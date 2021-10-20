use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use reqwest::{Client, StatusCode};
use tokio::sync::Semaphore;

use crate::{regexp::RegexWrapper, utils::get_path_ext, Opts};

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("glob error")]
    Glob(#[from] glob::GlobError),
    #[error("io error")]
    IO(#[from] std::io::Error),
}

/// Process markdown, downlaod and replace.
pub async fn process_markdown(opts: Opts) -> Result<(), ProcessError> {
    // find files
    let options = glob::MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };
    let path = Path::new(&opts.input).join("**/*.md");

    // collect urls
    let mut set = HashSet::new();
    let mut file_list = Vec::new();
    let regex = RegexWrapper::new();
    for entry in glob::glob_with(&path.to_string_lossy(), options).expect("invalid glob pattern") {
        let path = entry?;
        let content = std::fs::read_to_string(&path)?;
        regex.collect_urls(content, &mut set);
        file_list.push(path);
    }
    tracing::info!(
        "scanned {} links in {} markdown files",
        set.len(),
        file_list.len()
    );

    // download them
    let result_mapping = download_images(
        set,
        opts.output_dir,
        opts.link_prefix,
        Duration::from_secs(opts.timeout_sec as u64),
        opts.current_limit,
    )
    .await;
    tracing::info!("downloaded {} images", result_mapping.len());

    // replace them back
    for path in file_list {
        let contents = std::fs::read_to_string(&path)?;
        let new_contents = regex.replace_urls(contents, &result_mapping);
        std::fs::write(&path, new_contents)?;
    }
    tracing::info!("rewritten all markdown files done");

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("invalid status code: {0}")]
    InvalidStatusCode(StatusCode),
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
}

/// Download images to output folder and return the result of new url.
/// You may make sure the output_dir already exists.
async fn download_images(
    urls: HashSet<String>,
    output_dir: String,
    prefix: String,
    timeout: Duration,
    current_limit: u32,
) -> HashMap<String, String> {
    let semaphore = Arc::new(Semaphore::new(current_limit as usize));
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.81 Safari/537.36")
        .timeout(timeout)
        .build()
        .expect("unable to build reqwest client");
    let mut join_handles = Vec::with_capacity(urls.len());
    let results = Arc::new(Mutex::new(HashMap::with_capacity(urls.len())));

    for url in urls.into_iter() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("unable to acquire semaphore");
        let (client, output_dir, prefix, results) = (
            client.clone(),
            output_dir.clone(),
            prefix.clone(),
            results.clone(),
        );
        let join = tokio::spawn(async move {
            // 1. download image
            let ret = download_single(client, &url).await;
            if let Err(e) = ret {
                tracing::error!("downloading single image {} with error {}", &url, e);
                return;
            }
            let content = ret.unwrap();

            // 2. save image
            let save = save_single(&output_dir, &content, &url);
            if let Err(e) = save {
                tracing::error!("saving single image {} with error {}", &url, e);
                return;
            }
            let link = PathBuf::from(prefix).join(save.unwrap());
            let mut results = results.lock().expect("unable to lock results");
            results.insert(
                url,
                link.into_os_string()
                    .into_string()
                    .expect("unable to convert string"),
            );

            // 3. drop permit
            drop(permit);
        });
        join_handles.push(join);
    }
    for j in join_handles {
        let _ = j.await;
    }

    Arc::try_unwrap(results)
        .expect("unable to get arc inner")
        .into_inner()
        .expect("unable to get mutex inner")
}

async fn download_single(client: Client, url: &str) -> Result<Bytes, DownloadError> {
    tracing::info!("downloading {}", url);
    let req = client.get(url).build()?;
    let ret = client.execute(req).await?;
    if ret.status() != StatusCode::OK {
        return Err(DownloadError::InvalidStatusCode(ret.status()));
    }
    let content = ret.bytes().await.map_err(Into::into);
    content
}

fn save_single(output_dir: &str, content: &Bytes, url: &str) -> Result<String, DownloadError> {
    tracing::info!("saving {}", url);
    let mut file_name = sha1::Sha1::from(url.as_bytes()).hexdigest();
    if let Some(ext) = get_path_ext(url) {
        file_name.push_str(ext);
    }
    let path = Path::new(&output_dir).join(&file_name);
    let mut f = std::fs::File::create(path)?;
    f.write_all(content)?;
    Ok(file_name)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_download_images() {
        use super::download_images;
        use std::time::Duration;

        let _ = std::fs::create_dir_all("/tmp/images");
        let ret = download_images(
            [
                "https://i.v2ex.co/R7yApIA5s.jpeg".to_string(),
                "https://i.v2ex.co/BU0hPU5qs.jpeg".to_string(),
            ]
            .into_iter()
            .collect(),
            "/tmp/images".to_string(),
            "/images".to_string(),
            Duration::from_secs(20),
            20,
        )
        .await;
        assert_eq!(ret.len(), 2);
        assert_eq!(
            ret.get("https://i.v2ex.co/R7yApIA5s.jpeg").unwrap(),
            "/images/e3d093edf299313347493eed24e85330ea22eafc.jpeg"
        );
        assert_eq!(
            ret.get("https://i.v2ex.co/BU0hPU5qs.jpeg").unwrap(),
            "/images/e49534e68a0621048244877cf8ed30d2c93cb810.jpeg"
        );
    }

    #[allow(unused)]
    async fn test_process_markdown() {
        use super::{process_markdown, Opts};
        let opts = Opts {
            input: "/tmp/mds".to_string(),
            output_dir: "/tmp/images".to_string(),
            timeout_sec: 20,
            current_limit: 50,
            link_prefix: "/images".to_string(),
        };
        assert!(process_markdown(opts).await.is_ok());
    }
}

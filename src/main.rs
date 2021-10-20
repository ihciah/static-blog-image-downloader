use clap::Parser;
use tracing_subscriber::FmtSubscriber;

mod downloader;
use downloader::process_markdown;

mod utils;
mod regexp;

#[derive(Parser)]
#[clap(version = "1.0", author = "ihciah <ihciah@gmail.com>")]
pub struct Opts {
    #[clap(short, long, default_value = "source")]
    pub(crate) input: String,
    #[clap(short, long, default_value = "public/images")]
    pub(crate) output_dir: String,
    #[clap(short, long, default_value = "/images")]
    pub(crate) link_prefix: String,
    #[clap(short, long, parse(try_from_str), default_value = "60")]
    pub(crate) timeout_sec: u32,
    #[clap(short, long, parse(try_from_str), default_value = "50")]
    pub(crate) current_limit: u32,
}

#[tokio::main]
async fn main() {
    let opts: Opts = Opts::parse();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    tracing::info!(
        "will download image for markdown files in {} to {} with link prefix {}, timeout is {} sec",
        opts.input,
        opts.output_dir,
        opts.link_prefix,
        opts.timeout_sec,
    );

    let _ = std::fs::create_dir_all(&opts.output_dir);

    if let Err(e) = process_markdown(opts).await {
        tracing::error!("process markdown in error: {}", e);
    }
    tracing::info!("image downloader finished");
}

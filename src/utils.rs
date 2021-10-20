/// Extract path extension with dot.
pub fn get_path_ext(url: &str) -> Option<&str> {
    let dot_pos = url.rfind('.')?;
    let suffix = &url[dot_pos..];
    if suffix.chars().skip(1).all(|c| c.is_alphanumeric()) {
        return Some(suffix);
    }
    None
}
use std::path::Path;

pub fn ensure_dir_exists(dir_path: &str) -> std::io::Result<()> {
    let path = Path::new(dir_path);
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}

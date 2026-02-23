use std::path::{Path, PathBuf};
pub fn validate_file_path(workdir: &str, target: &str) -> Result<PathBuf, String> {
    if workdir.is_empty() {
        return Err("workdir must not be empty".into());
    }
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return Err(format!("absolute target path not allowed: {:?}", target));
    }
    let abs_workdir = std::env::current_dir()
        .map_err(|e| format!("getting cwd: {}", e))?
        .join(workdir);
    let abs_workdir = normalize_path(&abs_workdir);
    let joined = abs_workdir.join(target);
    let cleaned = normalize_path(&joined);
    let workdir_str = abs_workdir.to_string_lossy().to_string();
    let cleaned_str = cleaned.to_string_lossy().to_string();
    if cleaned_str != workdir_str && !cleaned_str.starts_with(&format!("{}/", workdir_str)) {
        return Err(format!(
            "path traversal detected: {:?} escapes workdir {:?}",
            target, workdir_str
        ));
    }
    Ok(cleaned)
}
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            c => components.push(c),
        }
    }
    let mut result = PathBuf::new();
    for c in components {
        result.push(c.as_os_str());
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    #[test]
    fn test_valid_path() {
        let dir = TempDir::new().unwrap();
        let result = validate_file_path(dir.path().to_str().unwrap(), "output.txt");
        assert!(result.is_ok());
    }
    #[test]
    fn test_traversal_rejected() {
        let dir = TempDir::new().unwrap();
        let traversal = ["..", "..", "..", "tmp", "x"].join(&std::path::MAIN_SEPARATOR.to_string());
        let result = validate_file_path(dir.path().to_str().unwrap(), &traversal);
        assert!(result.is_err());
    }
    #[test]
    fn test_absolute_rejected() {
        let dir = TempDir::new().unwrap();
        let result = validate_file_path(dir.path().to_str().unwrap(), "/tmp/test");
        assert!(result.is_err());
    }
    #[test]
    fn test_empty_workdir() {
        let result = validate_file_path("", "output.txt");
        assert!(result.is_err());
    }
    #[test]
    fn test_workdir_itself() {
        let dir = TempDir::new().unwrap();
        let result = validate_file_path(dir.path().to_str().unwrap(), ".");
        assert!(result.is_ok());
    }
}

use std::path::Path;

pub fn ensure_gitignore_entry(repo_root: &Path) -> std::io::Result<()> {
    let gitignore_path = repo_root.join(".gitignore");
    let entry = ".cxpak/";

    if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path)?;
        if content.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
        let separator = if content.ends_with('\n') { "" } else { "\n" };
        std::fs::write(&gitignore_path, format!("{content}{separator}{entry}\n"))
    } else {
        std::fs::write(&gitignore_path, format!("{entry}\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_creates_gitignore_with_cxpak() {
        let dir = TempDir::new().unwrap();
        ensure_gitignore_entry(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains(".cxpak/"));
    }

    #[test]
    fn test_appends_to_existing_gitignore() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        ensure_gitignore_entry(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("target/"));
        assert!(content.contains(".cxpak/"));
    }

    #[test]
    fn test_idempotent_if_already_present() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n.cxpak/\n").unwrap();
        ensure_gitignore_entry(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(content.matches(".cxpak/").count(), 1);
    }
}

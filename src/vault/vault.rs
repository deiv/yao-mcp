use crate::vault::error::VaultError;
use path_absolutize::Absolutize;
use path_trav::PathTrav;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Vault {
    vault_path: PathBuf,
}

impl Vault {
    /// Create a new vault
    pub fn new(vault_path: PathBuf) -> Result<Self, ()> {
        let vault_path = vault_path.clone();

        Ok(Self { vault_path })
    }

    /// Get vault path
    pub fn vault_path(&self) -> &PathBuf {
        &self.vault_path
    }

    pub async fn read_note(&self, path: &str) -> Result<String, VaultError> {
        self.read_file(path.as_ref()).await
    }

    pub async fn write_note(&self, path: &str, content: &str) -> Result<(), VaultError> {
        self.write_file(path.as_ref(), content).await
    }

    pub async fn modify_note(&self, path: &str, content: &str) -> Result<(), VaultError> {
        self.write_file(path.as_ref(), content).await
    }

    async fn read_file(&self, path: &Path) -> Result<String, VaultError> {
        let resolved_file_path = self.resolve_path_from_vault_root(path)?;
        let content = tokio::fs::read_to_string(&resolved_file_path)
            .await
            .map_err(VaultError::io)?;

        Ok(content)
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<(), VaultError> {
        let resolved_file_path = self.resolve_path_from_vault_root(path)?;
        // TODO: create not existing directories
        tokio::fs::write(&resolved_file_path, content)
            .await
            .map_err(VaultError::io)?;

        Ok(())
    }

    fn resolve_path_from_vault_root(&self, path: &Path) -> Result<PathBuf, VaultError> {
        // we allow absolute paths as our vault behaves like a chroot
        let normalized_path = if path.is_absolute() {
            let mut components = path.components();
            components.next();
            components.as_path()
        } else {
            path
        };

        match self.vault_path.is_path_trav(&normalized_path) {
            Ok(true) => Err(VaultError::invalid_path_traversal(path)),

            Ok(false) | Err(ErrorKind::NotFound) => {
                match normalized_path.absolutize_virtually(self.vault_path.as_path()) {
                    Ok(resolved_path) => Ok(PathBuf::from(resolved_path)),
                    Err(err) => {
                        Err(VaultError::invalid_path(format!(
                            "Invalid path {:?}: {:?}",
                            path, err
                        )))
                    }
                }
            }

            Err(err) => Err(VaultError::invalid_path(format!(
                "Invalid path {:?}: {:?}",
                path, err
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::error::VaultError::InvalidPath;
    use std::env::set_current_dir;
    use std::fs::File;
    use tempfile::TempDir;

    const FILE_NO_TRAVERSAL: &str = "testfolder/subfolder/test-note.md";
    const FILE_WITH_TRAVERSAL: &str = "./../subfolder/test-note.md";

    const FILE_WITH_TRAVERSAL_AND_EXISTS: &str = "existing-file.md";

    #[tokio::test]
    async fn should_resolve_path_from_vault_root() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        let vault = Vault::new(PathBuf::from(temp_path)).unwrap();

        /* no traversal */
        let path = Path::new(FILE_NO_TRAVERSAL);
        let result = vault.resolve_path_from_vault_root(path);
        assert!(result.is_ok());
        assert_eq!(temp_path.join(FILE_NO_TRAVERSAL), result.unwrap());

        /* no traversal and absolute */
        let path = Path::new("/").join(FILE_NO_TRAVERSAL);
        let result = vault.resolve_path_from_vault_root(path.as_path());
        assert!(result.is_ok());
        assert_eq!(temp_path.join(FILE_NO_TRAVERSAL), result.unwrap());

        /* not existing traversal */
        let path = Path::new(FILE_WITH_TRAVERSAL);
        let result = vault.resolve_path_from_vault_root(path);
        match result.err().unwrap() {
            InvalidPath { reason } => assert_eq!(
                format!(
                    "Invalid path \"{}\": Kind(InvalidInput)",
                    FILE_WITH_TRAVERSAL
                ),
                reason
            ),
            _ => assert!(false),
        }

        /* existing traversal */
        let temp_dir_traversal = TempDir::new().unwrap();
        let temp_dir_traversal_path = temp_dir_traversal.path();
        let file_path_traversal =
            File::create(Path::new(temp_dir_traversal_path).join(FILE_WITH_TRAVERSAL_AND_EXISTS));
        assert!(file_path_traversal.is_ok());

        set_current_dir(temp_path).expect("unable to change current dir");
        let temp_dir_traversal = temp_dir_traversal_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let path_traversal = format!(
            "../{}/{}",
            temp_dir_traversal, FILE_WITH_TRAVERSAL_AND_EXISTS);
        let result = vault.resolve_path_from_vault_root(Path::new(&path_traversal));

        match result.err().unwrap() {
            InvalidPath { reason } => assert_eq!(
                format!(
                    "Invalid Path: path traversal detected: \"../{}/{}\"",
                    temp_dir_traversal, FILE_WITH_TRAVERSAL_AND_EXISTS
                ),
                reason
            ),
            _ => assert!(false),
        }
    }
}

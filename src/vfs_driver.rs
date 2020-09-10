use error::*;

/// File system implementations mus implement this trait
pub trait VfsDriver: Debug + Sync + Send + 'static {
    /// Iterates over all entries of this directory path
    fn read_dir(&self, path: &str) -> VfsResult<Box<dyn Iterator<Item = String>>>;
    /// Opens the file at this path for reading
    fn open_file(&self, path: &str) -> VfsResult<Box<dyn SeekAndRead>>;
    /// Returns the file metadata for the file at this path
    fn metadata(&self, path: &str) -> VfsResult<VfsMetadata>;
    /// Returns true if a file or directory at path exists, false otherwise
    fn exists(&self, path: &str) -> bool;
    /// Removes the file at this path (optional)
    fn remove_file(&self, path: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// Removes the directory at this path
    fn remove_dir(&self, path: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// Copies the src path to the destination path within the same filesystem (optional)
    fn copy_file(&self, _src: &str, _dest: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// Moves the src path to the destination path within the same filesystem (optional)
    fn move_file(&self, _src: &str, _dest: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// Moves the src directory to the destination path within the same filesystem (optional)
    fn move_dir(&self, _src: &str, _dest: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
}

impl<T: FileSystem> From<T> for VfsNode {
    fn from(filesystem: T) -> Self {
        VfsPath::new(filesystem)
    }
}

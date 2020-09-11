use error::*;

/// File system implementations mus implement this trait
pub trait VfsDriver: Debug + Sync + Send + 'static {
    /// Returns a handle which updates the progress and returns the loaded data.
    /// This will return the raw data meaning if it's compressed it will be left as such.
    /// See `load_file` to load that will try to decompress it as well
    fn load_file_raw(&self, path: &str) -> VfsResult<Box<dyn VfsHandle>>;

    /// Returns a handle which updates the progress and returns the loaded data. This will try to
    /// decompress the data as well if an appropriate decompresser can be found.
    fn load_file(&self, path: &str) -> VfsResult<Box<dyn VfsHandle>>;
}


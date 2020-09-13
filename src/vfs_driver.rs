use crate::RecvMsg;
use crate::{InternalError, VfsError};

/// File system implementations must implement this trait
pub trait VfsDriver: Sync + Send {
    /// This is used to figure out if a certain mount can be done
    fn can_mount(&self, target: &str, source: &str) -> Result<(), VfsError>;
    /// Used when creating an instance of the driver with a path to load from
    fn new_from_path(&self, path: &str) -> Result<Box<dyn VfsDriver>, VfsError>;
    /// Returns a handle which updates the progress and returns the loaded data. This will try to
    /// decompress the data as well if an appropriate decompresser can be found.
    fn load_file(
        &self,
        path: &str,
        msg: &crossbeam_channel::Sender<RecvMsg>,
    ) -> Result<(), InternalError>;
}

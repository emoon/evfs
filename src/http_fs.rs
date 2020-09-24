use crate::{EntryType, InternalError, RecvMsg, VfsDriver, VfsError};
use std::path::Path;

pub struct HttpFs {
    url: String,
}

impl HttpFs {
    pub fn new() -> HttpFs {
        HttpFs { url: String::new() }
    }
}

impl VfsDriver for HttpFs {
    fn is_remote(&self) -> bool {
        true
    }

    fn can_mount(&self, _target: &str, source: &str) -> Result<(), VfsError> {
        // source has to start with http:// or https://
        if source.starts_with("http://") || source.starts_with("https://") {
            Ok(())
        } else {
            Err(VfsError::UnsupportedMount {
                mount: source.into(),
            })
        }
    }

    fn new_from_path(&self, url: &str) -> Result<Box<dyn VfsDriver>, VfsError> {
        Ok(Box::new(HttpFs { url: url.into() }))
    }

    ///
    /// Read a file from the local filesystem.
    /// TODO: Make the 5 meg size configurable
    fn load_file(
        &self,
        path: &str,
        send_msg: &crossbeam_channel::Sender<RecvMsg>,
    ) -> Result<Box<[u8]>, InternalError> {
        let path = Path::new(&self.url).join(path);
        let t = path.to_string_lossy();
        let p = t.to_string();

        // TODO: Proper progress
        send_msg.send(RecvMsg::ReadProgress(0.0))?;
        let bytes = reqwest::blocking::get(&p).unwrap().bytes().unwrap();
        send_msg.send(RecvMsg::ReadProgress(1.0))?;

        // TODO: better way to do this?
        let mut data = vec![0; bytes.len()];
        data.copy_from_slice(bytes.as_ref());

        //send_msg.send(RecvMsg::ReadDone(output_data.into_boxed_slice()))?;

        Ok(data.into_boxed_slice())
    }

    /// This is used to figure out if a certain mount can be done
    fn has_entry(&self, _path: &str) -> EntryType {
        // TODO: Fix unwrap
        /*
        let read_file = File::open(&self.filename).unwrap();
        let mut archive = zip::ZipArchive::new(read_file).unwrap();

        if archive.by_name(path).is_ok() {
            EntryType::File
        } else {
            EntryType::NotFound
        }
        */

        // TODO: Currently assuming this is true
        EntryType::File
    }

    // local fs can't decompress anything
    fn can_decompress(&self, _data: &[u8]) -> bool {
        false
    }

    // local fs support any file ext
    fn supports_file_ext(&self, file_ext: &str) -> bool {
        file_ext == "zip"
    }
}

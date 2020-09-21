use crate::{EntryType, InternalError, RecvMsg, VfsDriver, VfsError};
use std::fs::File;
use std::io::Read;
use zip;

pub struct ZipFs {
    filename: String,
}

impl ZipFs {
    pub fn new() -> ZipFs {
        ZipFs {
            filename: String::new(),
        }
    }
}

impl VfsDriver for ZipFs {
    fn can_mount(&self, _target: &str, source: &str) -> Result<(), VfsError> {
        let metadata = std::fs::metadata(source)?;

        // Currently file has to end with .zip
        if metadata.is_file() && source.ends_with(".zip") {
            Ok(())
        } else {
            Err(VfsError::UnsupportedMount {
                mount: source.into(),
            })
        }
    }

    fn new_from_path(&self, filename: &str) -> Result<Box<dyn VfsDriver>, VfsError> {
        Ok(Box::new(ZipFs {
            filename: filename.into(),
        }))
    }

    ///
    /// Read a file from the local filesystem.
    /// TODO: Make the 5 meg size configurable
    fn load_file(
        &self,
        path: &str,
        send_msg: &crossbeam_channel::Sender<RecvMsg>,
    ) -> Result<Box<[u8]>, InternalError> {
        let read_file = File::open(&self.filename)?;
        // TODO: We should cache the archive and not reopen it
        // TODO: Handle error better here
        let mut archive = zip::ZipArchive::new(read_file).unwrap();
        let mut file = archive.by_name(path).unwrap();
        let len = file.size() as usize;
        let mut output_data = vec![0u8; len];

        // if file is small than 10k we just unpack it directly without progress
        if len < 10 * 1024 {
            send_msg.send(RecvMsg::ReadProgress(0.0))?;
            file.read_to_end(&mut output_data)?;
        } else {
            // above 10k we read in 10 chunks
            let loop_count = 10;
            let block_len = len / loop_count;
            let mut percent = 0.0;
            let percent_step = 1.0 / loop_count as f32;

            for i in 0..loop_count {
                let block_offset = i * block_len;
                let read_amount = usize::min(len - block_offset, block_len);
                file.read_exact(&mut output_data[block_offset..block_offset + read_amount])?;
                send_msg.send(RecvMsg::ReadProgress(percent))?;
                percent += percent_step;
            }
        }

        //send_msg.send(RecvMsg::ReadDone(output_data.into_boxed_slice()))?;

        Ok(output_data.into_boxed_slice())
    }

    /// This is used to figure out if a certain mount can be done
    fn has_entry(&self, path: &str) -> EntryType {
        // TODO: Fix unwrap
        let read_file = File::open(&self.filename).unwrap();
        let mut archive = zip::ZipArchive::new(read_file).unwrap();

        if archive.by_name(path).is_ok() {
            EntryType::File
        } else {
            EntryType::NotFound
        }
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

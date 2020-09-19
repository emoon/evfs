use crate::{EntryType, InternalError, RecvMsg, VfsDriver, VfsError};
use log::*;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Clone)]
pub struct LocalFs {
    root: String,
}

impl LocalFs {
    pub fn new() -> LocalFs {
        LocalFs {
            root: String::new(),
        }
    }
}

impl VfsDriver for LocalFs {
    fn can_mount(&self, _target: &str, source: &str) -> Result<(), VfsError> {
        // special case for source of current dir
        if source == "" {
            return Ok(());
        }

        let metadata = std::fs::metadata(source)?;

        if metadata.is_file() {
            Err(VfsError::UnsupportedMount {
                mount: source.into(),
            })
        } else {
            Ok(())
        }
    }

    fn new_from_path(&self, path: &str) -> Result<Box<dyn VfsDriver>, VfsError> {
        Ok(Box::new(LocalFs { root: path.into() }))
    }

    ///
    /// Read a file from the local filesystem.
    /// TODO: Make the 5 meg size configurable
    fn load_file(
        &self,
        path: &str,
        send_msg: &crossbeam_channel::Sender<RecvMsg>,
    ) -> Result<Box<[u8]>, InternalError> {
        let path = Path::new(&self.root).join(path);

        let metadata = std::fs::metadata(&path)?;
        let len = metadata.len() as usize;
        let mut file = File::open(&path)?;
        let mut output_data = vec![0u8; len];

        trace!("vfs: reading from {:#?}", path);

        // if file is small than 5 meg we just load it fully directly to memory
        if len < 5 * 1024 * 1024 {
            send_msg.send(RecvMsg::ReadProgress(0.0))?;
            file.read_to_end(&mut output_data)?;
        } else {
            // above 5 meg we read in 10 chunks
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

    fn has_entry(&self, path: &str) -> EntryType {
        let path = Path::new(&self.root).join(path);

        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.is_file() {
                EntryType::File
            } else {
                EntryType::Directory
            }
        } else {
            EntryType::NotFound
        }
    }

    // local fs can't decompress anything
    fn can_decompress(&self, _data: &[u8]) -> bool {
        false
    }

    // local fs support any file ext
    fn supports_file_ext(&self, _file_ext: &str) -> bool {
        true
    }
}

use crossbeam_channel;
use crossbeam_channel::unbounded;
use log::{trace, error};
use std::fs::File;
use std::io::Read;
use std::thread;
use thiserror::Error;

pub enum RecvMsg {
    ReadProgress(f32),
    ReadDone(Box<[u8]>),
    Error(VfsError),
}

pub enum SendMsg {
    // TODO: Proper error
    //Error(String),
    /// Send messages
    ReadFile(String),
}

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("File Error)")]
    FileError(#[from] std::io::Error),
    #[error("Send Error")]
    SendError(#[from] crossbeam_channel::SendError<RecvMsg>),
}

#[derive(Error, Debug)]
pub enum VfsError {
    #[error("File Error)")]
    FileError(#[from] std::io::Error),
}


pub struct Handle {
    pub recv: crossbeam_channel::Receiver<RecvMsg>,
}

pub struct VfsDriver {
    send: crossbeam_channel::Sender<SendMsg>,
    recv: crossbeam_channel::Receiver<RecvMsg>,
    _path: String,
}

impl VfsDriver {
    fn new(path: &str) -> VfsDriver {
        let (thread_send, main_recv) = unbounded::<RecvMsg>();
        let (main_send, thread_recv) = unbounded::<SendMsg>();

        let _ = thread::spawn(move || {
            while let Ok(msg) = thread_recv.recv() {
                Self::handle_msg(&msg, &thread_send);
            }
        });

        VfsDriver {
            send: main_send,
            recv: main_recv,
            _path: path.into(),
        }
    }

    fn handle_msg(msg: &SendMsg, thread_send: &crossbeam_channel::Sender<RecvMsg>) {
        let res = match msg {
            SendMsg::ReadFile(path) => Self::read_file(thread_send.clone(), &path),
        };

        // We need to do some special handling of errors here depending on the error in question.
        // If we have a IoError we just propagate it back to the main thread otherwise we will just log
        // a warning here as we have no way to notify the otherside

        if let Err(e) = res {
            match e {
                InternalError::FileError(e) => {
                    let file_error = format!("{:#?}", e);
                    if let Err(send_err) = thread_send.send(RecvMsg::Error(e.into())) {
                        error!("evfs: Unable to send file error {:#?} to main thread due to {:#?}", file_error, send_err);
                    }
                }

                _ => (),
            }
        }
    }

    ///
    /// Read a file to memory. Path in should be a fully resolved path. This code will still
    /// hande if the file doesn't exist and will return an error that the other side will need to handle.
    ///
    fn read_file(send_msg: crossbeam_channel::Sender<RecvMsg>, path: &str) -> Result<(), InternalError> {
        let metadata = std::fs::metadata(path)?;
        let len = metadata.len() as usize;
        let mut file = File::open(path)?;
        let mut output_data = vec![0u8; len];

        trace!("vfs: reading from {}", path);

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

        send_msg.send(RecvMsg::ReadDone(output_data.into_boxed_slice()))?;

        Ok(())
    }
}

struct Mount {
    path: String,
    driver: VfsDriver,
}

pub struct Evfs {
    drivers: Vec<Mount>,
    pub mounts: Vec<VfsDriver>,
}

impl Evfs {
    pub fn new() -> Evfs {
        Evfs {
            drivers: Vec::new(),
            //mounts: HashMap::new()
            mounts: Vec::new(),
        }
    }

    pub fn install_driver(driver: VfsDriver) {

    }

    /// TODO: Error handling
    pub fn mount(&mut self, _root: &str, filesys: &str) {
        // TODO: select the correct vfs system here
        self.mounts.push(VfsDriver::new(filesys));
    }

    /// TODO: Error handling, etc, correct path, etc
    pub fn read_file(&self, path: &str) -> Handle {
        // testing
        let vfs = &self.mounts[0];
        vfs.send.send(SendMsg::ReadFile(path.into())).unwrap();
        Handle {
            recv: vfs.recv.clone(),
        }
    }
}

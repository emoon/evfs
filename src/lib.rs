use crossbeam_channel;
use crossbeam_channel::unbounded;
use log::*;
use thiserror::Error;

use std::sync::Arc;
use std::thread;

//mod error;
mod vfs_driver;

//use error::VfsError;
use vfs_driver::VfsDriver;

pub enum RecvMsg {
    ReadProgress(f32),
    ReadDone(Box<[u8]>),
    Error(VfsError),
}

pub enum SendMsg {
    // TODO: Proper error
    //Error(String),
    /// Send messages
    LoadFile(
        String,
        Arc<Box<dyn VfsDriver>>,
        crossbeam_channel::Sender<RecvMsg>,
    ),
}

#[cfg(feature = "local-fs")]
pub mod local_fs;
#[cfg(feature = "local-fs")]
pub use local_fs::LocalFs;

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

pub struct Evfs {
    drivers: Vec<Box<dyn VfsDriver>>,
    pub mounts: Vec<Arc<Box<dyn VfsDriver>>>,
    _msg_thread: thread::JoinHandle<()>,
    main_send: crossbeam_channel::Sender<SendMsg>,
}

fn handle_error(res: Result<(), InternalError>, msg: &crossbeam_channel::Sender<RecvMsg>) {
    if let Err(e) = res {
        match e {
            InternalError::FileError(e) => {
                let file_error = format!("{:#?}", e);
                if let Err(send_err) = msg.send(RecvMsg::Error(e.into())) {
                    error!(
                        "evfs: Unable to send file error {:#?} to main thread due to {:#?}",
                        file_error, send_err
                    );
                }
            }

            _ => (),
        }
    }
}

fn handle_msg(msg: &SendMsg) {
    match msg {
        SendMsg::LoadFile(path, loader, msg) => {
            let res = loader.load_file(path, msg);
            handle_error(res, msg);
        }
    }
}

impl Evfs {
   #[allow(unused_mut)]
   pub fn new() -> Evfs {
        let (main_send, thread_recv) = unbounded::<SendMsg>();

        // Setup 2 worker threads
        // TODO: Configure number of worker threads
        // and consider spliting io from unpacking

        let worker_threads = threadpool::Builder::new()
            .num_threads(2)
            .thread_name("evfs_worker_thread".into())
            .build();

        let msg_thread = thread::Builder::new()
            .name("evfs_msg_thread".to_string())
            .spawn(move || {
                while let Ok(msg) = thread_recv.recv() {
                    worker_threads.execute(move || {
                        handle_msg(&msg);
                    });
                }
            })
            .unwrap();

        let mut drivers: Vec<Box<dyn VfsDriver>> = Vec::new();

        #[cfg(feature = "local-fs")]
        drivers.push(Box::new(LocalFs::new()));

        Evfs {
            drivers,
            mounts: Vec::new(),
            _msg_thread: msg_thread,
            main_send,
        }
    }

    pub fn install_driver(&mut self, driver: Box<dyn VfsDriver>) {
        self.drivers.push(driver);
    }

    /// TODO: Error handling
    pub fn mount(&mut self, _root: &str, filesys: &str) -> Result<(), VfsError> {
        // TODO: select the correct vfs system here
        self.mounts
            .push(Arc::new(self.drivers[0].new_from_path(filesys)?));
        Ok(())
    }

    /// TODO: Error handling, etc, correct path, etc
    pub fn load_file(&self, path: &str) -> Handle {
        let (thread_send, main_recv) = unbounded::<RecvMsg>();
        // testing
        let driver = self.mounts[0].clone();
        self.main_send
            .send(SendMsg::LoadFile(path.into(), driver, thread_send))
            .unwrap();

        Handle { recv: main_recv }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "local-fs")]
    fn load_local_file() {
        use super::*;
        use std::{thread, time};

        let mut vfs = Evfs::new();
        vfs.mount("/test", "").unwrap();
        let handle = vfs.load_file("Cargo.toml");

        for _ in 0..10 {
            match handle.recv.try_recv() {
                Ok(data) => {
                    match data {
                        RecvMsg::ReadProgress(p) => println!("ReadProgress {}", p),
                        RecvMsg::ReadDone(_data) => {
                            println!("File read done!");
                            //break;
                        }

                        RecvMsg::Error(e) => {
                            panic!("main: error {:#?}", e);
                        }
                    }
                }

                _ => (),
            }

            thread::sleep(time::Duration::from_millis(10));
        }
    }
}

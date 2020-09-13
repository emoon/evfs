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
    /// Errors from std::io::Error
    #[error("File Error)")]
    FileError(#[from] std::io::Error),
    /// If trying to mount an invalid path
    #[error("The mount point `{path}` is invalid. It has to start with a /")]
    InvalidRootPath {
        /// The invalid path
        path: String,
    },
    /// If trying to mount an invalid path
    #[error("The mount `{mount}` is invalid for this driver. Has to be a directory")]
    UnsupportedMount {
        /// The invalid path
        mount: String,
    },

    /// If trying to mount an invalid path
    #[error("There is no driver that supports the current mount")]
    NoDriverSupport {},

    /// If trying to mount an invalid path
    #[error("No mount for `{path}` was found. Have you forgot to mount the path?")]
    NoMountFound {
        /// The invalid path
        path: String,
    },
}

pub struct Handle {
    pub recv: crossbeam_channel::Receiver<RecvMsg>,
}

pub struct Mount {
    source: String,
    target: String,
    driver: Arc<Box<dyn VfsDriver>>,
}

pub struct Evfs {
    drivers: Vec<Box<dyn VfsDriver>>,
    pub mounts: Vec<Mount>,
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

    /// Mount a path in the virtual file system
    pub fn mount(&mut self, target: &str, source: &str) -> Result<(), VfsError> {
        for (i, driver) in self.drivers.iter().enumerate() {
            if driver.can_mount(target, source).is_ok() {
                let t;
                // special case for ""
                if source == "" {
                    t = std::env::current_dir()?;
                } else {
                    t = std::fs::canonicalize(source)?;
                }

                let full_path = t.to_string_lossy();
                self.mounts.push(Mount {
                    target: target.into(),
                    source: full_path.to_string(),
                    driver: Arc::new(self.drivers[i].new_from_path(&full_path)?),
                });

                return Ok(());
            }
        }

        Err(VfsError::NoDriverSupport {})
    }

    /// TODO: Error handling, etc, correct path, etc
    pub fn load_file(&self, path: &str) -> Result<Handle, VfsError> {
        for mount in &self.mounts {
            if path.starts_with(&mount.target) {
                let driver = mount.driver.clone();
                let (thread_send, main_recv) = unbounded::<RecvMsg>();
                let full_path = path.replace(&mount.target, &mount.source);
                self.main_send
                    .send(SendMsg::LoadFile(full_path.into(), driver, thread_send))
                    .unwrap();

                return Ok(Handle { recv: main_recv });
            }
        }

        Err(VfsError::NoMountFound { path: path.into() })
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
        let handle = vfs.load_file("/test/Cargo.toml").unwrap();

        for _ in 0..10 {
            match handle.recv.try_recv() {
                Ok(data) => match data {
                    RecvMsg::ReadProgress(p) => println!("ReadProgress {}", p),
                    RecvMsg::ReadDone(_data) => {
                        println!("File read done!");
                    }

                    RecvMsg::Error(e) => {
                        panic!("main: error {:#?}", e);
                    }
                },

                _ => (),
            }

            thread::sleep(time::Duration::from_millis(10));
        }
    }
}

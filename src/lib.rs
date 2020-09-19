use crossbeam_channel;
use crossbeam_channel::unbounded;
use log::*;
use thiserror::Error;

use std::path::Path;
use std::sync::Arc;
use std::thread;

//mod error;
mod vfs_driver;

//use error::VfsError;
use vfs_driver::{EntryType, VfsDriver};

pub enum RecvMsg {
    ReadProgress(f32),
    ReadDone(Box<[u8]>),
    Error(VfsError),
}

type Mounts = Vec<Mount>;
type ArcDriver = Arc<Box<dyn VfsDriver>>;

#[derive(Clone)]
pub struct Mount {
    source: String,
    target: String,
    driver: ArcDriver,
}

pub enum SendMsg {
    // TODO: Proper error
    //Error(String),
    /// Send messages
    LoadFile(String, Mounts, Vec<ArcDriver>, crossbeam_channel::Sender<RecvMsg>),
}

#[cfg(feature = "local-fs")]
pub mod local_fs;
#[cfg(feature = "local-fs")]
pub use local_fs::LocalFs;

#[cfg(feature = "zip-fs")]
pub mod zip_fs;
#[cfg(feature = "zip-fs")]
pub use zip_fs::ZipFs;

#[derive(Error, Debug)]
pub enum InternalError {
    /// If trying to mount an invalid path
    #[error("The path `{path}` is not found in mount")]
    PathNotFound {
        /// The invalid path
        path: String,
    },
    /// If trying to mount an invalid path
    #[error("The path `{path}` is a directory and not a file")]
    NotFile {
        /// The invalid path
        path: String,
    },
    /// If trying to mount an invalid path
    #[error("Unable to find decompressor for `{path}`")]
    DecompressorNotFound {
        /// The invalid path
        path: String,
    },
    /// If no mount was found
    #[error("Invalid mount `{path}`")]
    InvalidMount {
        /// The invalid path
        path: String,
    },
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

pub struct Evfs {
    drivers: Vec<Arc<Box<dyn VfsDriver>>>,
    pub mounts: Mounts,
    _msg_thread: thread::JoinHandle<()>,
    main_send: crossbeam_channel::Sender<SendMsg>,
}

fn handle_error(res: Result<(), InternalError>, msg: &crossbeam_channel::Sender<RecvMsg>) {
    if let Err(e) = res {
    	println!("error {:#?}", e);
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

/// Find the initial mount point for a file
fn get_intital_mount(path: &str, mount_points: &Mounts) -> Option<usize> {
    // first find a mount point for this file. The way we do it is to strip down the path more and more
    // such as if a starting point is /hello/this/has/many/dirs/file
    // /hello/this/has/many/dirs
    // /hello/this/has/many
    // /hello/this/has
    // ..
    // In order to find the top-level mount point as it's possible to overlay file-systems

    let dir = Path::new(path);

    while let Some(dir) = dir.parent() {
        for (i, mount) in mount_points.iter().enumerate() {
            let t = dir.to_string_lossy();
            if mount.target == t {
                return Some(i);
            }
        }
    }

    None
}

// Looks for file entry for a driver
fn find_entry(driver: &ArcDriver, path: &str) -> (usize, EntryType) {
    // Early check if driver has path, then we can return directly
    if driver.has_entry(path) == EntryType::File {
        return (path.len(), EntryType::File);
    }

    let dir = Path::new(path);

    while let Some(dir) = dir.parent() {
        let t = dir.to_string_lossy();
        let entry = driver.has_entry(&t);

        match entry {
            EntryType::File => return (t.len(), EntryType::File),
            EntryType::Directory => return (t.len(), EntryType::Directory),
            _ => (),
        }
    }

	(0, EntryType::NotFound)
}

fn find_driver(current_path: &str, file_data: &[u8], drivers: &Vec<ArcDriver>) -> Option<ArcDriver> {
	// TODO: Figure out how to deal with finding by data or ext
	for driver in drivers {
		if driver.supports_file_ext(current_path) {
			return Some(driver.clone());
		}

		if driver.can_decompress(file_data) {
			return Some(driver.clone());
		}
	}

	None
}

fn load_file(
    mount: &Mount,
    path: &str,
    drivers: &Vec<ArcDriver>,
    send_msg: &crossbeam_channel::Sender<RecvMsg>,
) -> Result<(), InternalError> {
	// used for "sliding window" of the path
	let path_len = path.len();
	let mut start_path = 0;
	let mut end_path = path_len;
	let mut driver = mount.driver.clone();

    // first strip the first pArcDriver of the path. We want to go from something like
    // this is safe to do as we have found this mount point

	// max 100 depth for saftey and not lock-up this code in case of error
    for _ in 0..100 {
    	let current_path = &path[start_path..end_path];

    	// Search for the entry with the current mount
		let (path_size, entry_type) = find_entry(&mount.driver, current_path);

		// Validate that some part of the path was actually found
		match entry_type {
			EntryType::NotFound => return Err(InternalError::PathNotFound { path: path.to_owned() }),
			EntryType::Directory => return Err(InternalError::NotFile { path: path.to_owned() }),
			_ => (),
		}

		let file_data = driver.load_file(current_path, send_msg)?;

		// if we are at the end path we can return the file
		// TODO: Auto-detect and try to decompress
		if end_path == path_size {
        	send_msg.send(RecvMsg::ReadDone(file_data))?;
        	return Ok(());
		}

		// if we aren't at end here we have a multifile and need to find a decompressor file tho current file
		if let Some(new_driver) = find_driver(current_path, &file_data, drivers) {
			driver = new_driver;
		} else {
			return Err(InternalError::DecompressorNotFound { path: path.to_owned() });
		}

		// set path window
		start_path = path_size;
		end_path = path_len;
    }

	Err(InternalError::DecompressorNotFound { path: path.to_owned() })
}

fn handle_msg(msg: &SendMsg) {
    match msg {
        SendMsg::LoadFile(path, mounts, drivers, msg) => {
            let res;
            let driver_index = get_intital_mount(path, mounts);

            if let Some(driver_index) = driver_index {
            	let driver = &mounts[driver_index];
            	println!("{}", path);
            	println!("{}", driver.target);
            	println!("{}", driver.target.len());
                res = load_file(driver, &path[driver.target.len() + 1..], drivers, msg);
            } else {
                res = Err(InternalError::InvalidMount {
                    path: path.to_owned(),
                });
            }

            //let res = loader.load_file(path, msg);
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

        let mut drivers: Vec<ArcDriver> = Vec::new();

        #[cfg(feature = "local-fs")]
        drivers.push(Arc::new(Box::new(LocalFs::new())));

        #[cfg(feature = "zip-fs")]
        drivers.push(Arc::new(Box::new(ZipFs::new())));

        Evfs {
            drivers,
            mounts: Vec::new(),
            _msg_thread: msg_thread,
            main_send,
        }
    }

    pub fn install_driver(&mut self, driver: ArcDriver) {
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
    pub fn load_file(&self, path: &str) -> Handle {
        let mounts = self.mounts.clone();
        let drivers = self.drivers.clone();
        let (thread_send, main_recv) = unbounded::<RecvMsg>();

        self.main_send
            .send(SendMsg::LoadFile(path.into(), mounts, drivers, thread_send))
            .unwrap();

        Handle { recv: main_recv }
    }
}

#[cfg(test)]
mod tests {
    /*
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
         */

    #[test]
    #[cfg(feature = "zip-fs")]
    fn load_zip_file() {
        use super::*;
        use std::{thread, time};

        let mut vfs = Evfs::new();
        vfs.mount("/data", "").unwrap();
        //vfs.mount("/data", "data/test_data.zip").unwrap();
        let handle = vfs.load_file("/data/Cargo.toml");

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

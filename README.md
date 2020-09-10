## 📁 evfs

evfs is a [Virtual File System](https://en.wikipedia.org/wiki/Virtual_file_system) written in [Rust](https://www.rust-lang.org)

The purpose of this library is to allow the user to setup a virtual file system that works similar to how a [POSIX](https://en.wikipedia.org/wiki/POSIX) file system works. Here are some examples

```Rust
    vfs.mount("/temp", "/usr/foo/temp");
    let handle = vfs.load_file("/temp/some_file");
```

The example above creates a mount `/temp` that points to `/usr/foo/temp` so when loading the file `some_file` from `/temp` it will actually load from `/usr/foo/temp/some_file`

```Rust
    vfs.mount("/assets", "data/data.zip");
    let handle = vfs.load_file("/assets/main_menu.png");
```

In this code we mount a [zip file](https://en.wikipedia.org/wiki/Zip_(file_format)) and read a file from it. If at some point we don't want to use zip files anymore all of the code reading the data can stay the same and only the mount has to change.

```Rust
    vfs.mount("/music", "ftp://some_music_sever.com/music");
    let handle = vfs.load("/music/awesome.flac");
```

evfs will (optionally) support loading over the net as well. In this case we read a file from an [FTP server](https://en.wikipedia.org/wiki/File_Transfer_Protocol)

## Async

evfs always uses async for loading but does not rely on Rust `async` to keep things simple. When loading a file the user will always get a handle back and is responsible to check the status of it. It's also optionally possible to get the progress of how much a file has been loaded to allow updates in UIs and such.

## Caching

Something that evfs also will support is to have different levels of caching. So for example files that happen to be read from a remote source can be cached locally. Still when using evfs you will still write the code as loading from a remote source, but if the file is present in the cache it will be loaded from there instead.

## Current status

evfs is in very early development and isn't useable yet.

## Licence

evfs is licensed under the [MIT](https://en.wikipedia.org/wiki/MIT_License) licence

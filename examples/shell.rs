//! A simple shell demo for embedded-sdmmc
//!
//! Presents a basic command prompt which implements some basic MS-DOS style
//! shell commands.
//!
//! Note that `embedded_sdmmc` itself does not care about 'paths' - only
//! accessing files and directories on on disk, relative to some previously
//! opened directory. A 'path' is an operating-system level construct, and can
//! vary greatly (see MS-DOS paths vs POSIX paths). This example, however,
//! implements an MS-DOS style Path API over the top of embedded-sdmmc. Feel
//! free to copy it if it suits your particular application.
//!
//! The four primary partitions are scanned on the given disk image on start-up.
//! Any valid FAT16 or FAT32 volumes are mounted, and given volume labels from
//! `A:` to `D:`, like MS-DOS. Also like MS-DOS, file and directory names use
//! the `8.3` format, like `FILENAME.TXT`. Long filenames are not supported.
//!
//! Unlike MS-DOS, this application uses the POSIX `/` as the directory
//! separator.
//!
//! Every volume has its own *current working directory*. The shell has one
//! *current volume* selected but it remembers the *current working directory*
//! for the unselected volumes.
//!
//! A path comprises:
//!
//! * An optional volume specifier, like `A:`
//!   * If the volume specifier is not given, the current volume is used.
//! * An optional `/` to indicate this is an absolute path, not a relative path
//!   * If this is a relative path, traversal starts at the Current Working
//!     Directory for the volume
//! * An optional sequence of directory names, each followed by a `/`
//! * An optional final filename
//!   * If this is missing, then `.` is the default (which selects the
//!     containing directory)
//!
//! An *expanded path* has all optional components, and works independently of
//! whichever volume is currently selected, or the current working directory
//! within that volume. The empty path (`""`) is invalid, but commands may
//! assume that in the absence of a path argument they are to use the current
//! working directory on the current volume.
//!
//! As an example, imagine that volume `A:` is the current volume, and we have
//! these current working directories:
//!
//! * `A:` has a CWD of `/CATS`
//! * `B:` has a CWD of `/DOGS`
//!
//! The following path expansions would occur:
//!
//! | Given Path                  | Volume  | Absolute | Directory Names    | Final Filename | Expanded Path                  |
//! | --------------------------- | ------- | -------- | ------------------ | -------------- | ------------------------------ |
//! | `NAMES.CSV`                 | Current | No       | `[]`               | `NAMES.CSV`    | `A:/CATS/NAMES.CSV`            |
//! | `./NAMES.CSV`               | Current | No       | `[.]`              | `NAMES.CSV`    | `A:/CATS/NAMES.CSV`            |
//! | `BACKUP.000/`               | Current | No       | `[BACKUP.000]`     | None           | `A:/CATS/BACKUP.000/.`         |
//! | `BACKUP.000/NAMES.CSV`      | Current | No       | `[BACKUP.000]`     | `NAMES.CSV`    | `A:/CATS/BACKUP.000/NAMES.CSV` |
//! | `/BACKUP.000/NAMES.CSV`     | Current | Yes      | `[BACKUP.000]`     | `NAMES.CSV`    | `A:/BACKUP.000/NAMES.CSV`      |
//! | `../BACKUP.000/NAMES.CSV`   | Current | No       | `[.., BACKUP.000]` | `NAMES.CSV`    | `A:/BACKUP.000/NAMES.CSV`      |
//! | `A:NAMES.CSV`               | `A:`    | No       | `[]`               | `NAMES.CSV`    | `A:/CATS/NAMES.CSV`            |
//! | `A:./NAMES.CSV`             | `A:`    | No       | `[.]`              | `NAMES.CSV`    | `A:/CATS/NAMES.CSV`            |
//! | `A:BACKUP.000/`             | `A:`    | No       | `[BACKUP.000]`     | None           | `A:/CATS/BACKUP.000/.`         |
//! | `A:BACKUP.000/NAMES.CSV`    | `A:`    | No       | `[BACKUP.000]`     | `NAMES.CSV`    | `A:/CATS/BACKUP.000/NAMES.CSV` |
//! | `A:/BACKUP.000/NAMES.CSV`   | `A:`    | Yes      | `[BACKUP.000]`     | `NAMES.CSV`    | `A:/BACKUP.000/NAMES.CSV`      |
//! | `A:../BACKUP.000/NAMES.CSV` | `A:`    | No       | `[.., BACKUP.000]` | `NAMES.CSV`    | `A:/BACKUP.000/NAMES.CSV`      |
//! | `B:NAMES.CSV`               | `B:`    | No       | `[]`               | `NAMES.CSV`    | `B:/DOGS/NAMES.CSV`            |
//! | `B:./NAMES.CSV`             | `B:`    | No       | `[.]`              | `NAMES.CSV`    | `B:/DOGS/NAMES.CSV`            |
//! | `B:BACKUP.000/`             | `B:`    | No       | `[BACKUP.000]`     | None           | `B:/DOGS/BACKUP.000/.`         |
//! | `B:BACKUP.000/NAMES.CSV`    | `B:`    | No       | `[BACKUP.000]`     | `NAMES.CSV`    | `B:/DOGS/BACKUP.000/NAMES.CSV` |
//! | `B:/BACKUP.000/NAMES.CSV`   | `B:`    | Yes      | `[BACKUP.000]`     | `NAMES.CSV`    | `B:/BACKUP.000/NAMES.CSV`      |
//! | `B:../BACKUP.000/NAMES.CSV` | `B:`    | No       | `[.., BACKUP.000]` | `NAMES.CSV`    | `B:/BACKUP.000/NAMES.CSV`      |

use std::io::prelude::*;

use embedded_sdmmc::{
    Error as EsError, RawDirectory, RawVolume, ShortFileName, VolumeIdx, VolumeManager,
};

use crate::linux::{Clock, LinuxBlockDevice};

type Error = EsError<std::io::Error>;

mod linux;

/// Represents a path on a volume within `embedded_sdmmc`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
struct Path(str);

impl std::ops::Deref for Path {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Path {
    /// Create a new Path from a string slice.
    ///
    /// The `Path` borrows the string slice. No validation is performed on the
    /// path.
    fn new<S: AsRef<str> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const str as *const Path) }
    }

    /// Does this path specify a volume?
    fn volume(&self) -> Option<char> {
        let mut char_iter = self.chars();
        match (char_iter.next(), char_iter.next()) {
            (Some(volume), Some(':')) => Some(volume),
            _ => None,
        }
    }

    /// Is this an absolute path?
    fn is_absolute(&self) -> bool {
        let tail = self.without_volume();
        tail.starts_with('/')
    }

    /// Iterate through the directory components.
    ///
    /// This will exclude the final path component (i.e. it will not include the
    /// 'basename').
    fn iterate_dirs(&self) -> impl Iterator<Item = &str> {
        let path = self.without_volume();
        let path = path.strip_prefix('/').unwrap_or(path);
        if let Some((directories, _basename)) = path.rsplit_once('/') {
            directories.split('/')
        } else {
            "".split('/')
        }
    }

    /// Iterate through all the components.
    ///
    /// This will include the final path component (i.e. it will include the
    /// 'basename').
    fn iterate_components(&self) -> impl Iterator<Item = &str> {
        let path = self.without_volume();
        let path = path.strip_prefix('/').unwrap_or(path);
        path.split('/')
    }

    /// Get the final component of this path (the 'basename').
    fn basename(&self) -> Option<&str> {
        if let Some((_, basename)) = self.rsplit_once('/') {
            if basename.is_empty() {
                None
            } else {
                Some(basename)
            }
        } else {
            let path = self.without_volume();
            Some(path)
        }
    }

    /// Return this [`Path`], but without a leading volume.
    fn without_volume(&self) -> &Path {
        if let Some((volume, tail)) = self.split_once(':') {
            // only support single char drive letters
            if volume.chars().count() == 1 {
                return Path::new(tail);
            }
        }
        self
    }
}

impl PartialEq<str> for Path {
    fn eq(&self, other: &str) -> bool {
        let s: &str = self;
        s == other
    }
}

struct VolumeState {
    directory: RawDirectory,
    volume: RawVolume,
    path: Vec<String>,
}

struct Context {
    volume_mgr: VolumeManager<LinuxBlockDevice, Clock, 8, 8, 4>,
    volumes: [Option<VolumeState>; 4],
    current_volume: usize,
}

impl Context {
    fn current_path(&self) -> Vec<String> {
        let Some(s) = &self.volumes[self.current_volume] else {
            return vec![];
        };
        s.path.clone()
    }

    /// Print some help text
    fn help(&mut self) -> Result<(), Error> {
        println!("Commands:");
        println!("\thelp                -> this help text");
        println!("\t<volume>:           -> change volume/partition");
        println!("\tstat                -> print volume manager status");
        println!("\tdir [<path>]        -> do a directory listing");
        println!("\ttree [<path>]       -> do a recursive directory listing");
        println!("\tcd ..               -> go up a level");
        println!("\tcd <path>           -> change into directory <path>");
        println!("\tcat <path>          -> print a text file");
        println!("\thexdump <path>      -> print a binary file");
        println!("\tmkdir <path>        -> create an empty directory");
        println!("\tquit                -> exits the program");
        println!();
        println!("Paths can be:");
        println!();
        println!("\t* Bare names, like `FILE.DAT`");
        println!("\t* Relative, like `../SOMEDIR/FILE.DAT` or `./FILE.DAT`");
        println!("\t* Absolute, like `B:/SOMEDIR/FILE.DAT`");
        Ok(())
    }

    /// Print volume manager status
    fn stat(&mut self) -> Result<(), Error> {
        println!("Status:\n{:#?}", self.volume_mgr);
        Ok(())
    }

    /// Print a directory listing
    fn dir(&mut self, path: &Path) -> Result<(), Error> {
        println!("Directory listing of {:?}", path);
        let dir = self.resolve_existing_directory(path)?;
        let mut dir = dir.to_directory(&mut self.volume_mgr);
        dir.iterate_dir(|entry| {
            println!(
                "{:12} {:9} {} {} {:08X?} {:?}",
                entry.name, entry.size, entry.ctime, entry.mtime, entry.cluster, entry.attributes
            );
        })?;
        Ok(())
    }

    /// Print a recursive directory listing for the given path
    fn tree(&mut self, path: &Path) -> Result<(), Error> {
        println!("Directory listing of {:?}", path);
        let dir = self.resolve_existing_directory(path)?;
        // tree_dir will close this directory, always
        self.tree_dir(dir)
    }

    /// Print a recursive directory listing for the given open directory.
    ///
    /// Will close the given directory.
    fn tree_dir(&mut self, dir: RawDirectory) -> Result<(), Error> {
        let mut dir = dir.to_directory(&mut self.volume_mgr);
        let mut children = Vec::new();
        dir.iterate_dir(|entry| {
            println!(
                "{:12} {:9} {} {} {:08X?} {:?}",
                entry.name, entry.size, entry.ctime, entry.mtime, entry.cluster, entry.attributes
            );
            if entry.attributes.is_directory()
                && entry.name != ShortFileName::this_dir()
                && entry.name != ShortFileName::parent_dir()
            {
                children.push(entry.name.clone());
            }
        })?;
        // Be sure to close this, no matter what happens
        let dir = dir.to_raw_directory();
        for child in children {
            println!("Entering {}", child);
            let child_dir = match self.volume_mgr.open_dir(dir, &child) {
                Ok(child_dir) => child_dir,
                Err(e) => {
                    self.volume_mgr.close_dir(dir).expect("close open dir");
                    return Err(e);
                }
            };
            let result = self.tree_dir(child_dir);
            println!("Returning from {}", child);
            if let Err(e) = result {
                self.volume_mgr.close_dir(dir).expect("close open dir");
                return Err(e);
            }
        }
        self.volume_mgr.close_dir(dir).expect("close open dir");
        Ok(())
    }

    /// Change into `<dir>`
    ///
    /// * An arg of `..` goes up one level
    /// * A relative arg like `../FOO` goes up a level and then into the `FOO`
    ///   sub-folder, starting from the current directory on the current volume
    /// * An absolute path like `B:/FOO` changes the CWD on Volume 1 to path
    ///   `/FOO`
    fn cd(&mut self, full_path: &Path) -> Result<(), Error> {
        let volume_idx = self.resolve_volume(full_path)?;
        let d = self.resolve_existing_directory(full_path)?;
        let Some(s) = &mut self.volumes[volume_idx] else {
            self.volume_mgr.close_dir(d).expect("close open dir");
            return Err(Error::NoSuchVolume);
        };
        self.volume_mgr
            .close_dir(s.directory)
            .expect("close open dir");
        s.directory = d;
        if full_path.is_absolute() {
            s.path.clear();
        }
        for fragment in full_path.iterate_components().filter(|s| !s.is_empty()) {
            if fragment == ".." {
                s.path.pop();
            } else {
                s.path.push(fragment.to_owned());
            }
        }
        Ok(())
    }

    /// print a text file
    fn cat(&mut self, filename: &Path) -> Result<(), Error> {
        let (dir, filename) = self.resolve_filename(filename)?;
        let mut dir = dir.to_directory(&mut self.volume_mgr);
        let mut f = dir.open_file_in_dir(filename, embedded_sdmmc::Mode::ReadOnly)?;
        let mut data = Vec::new();
        while !f.is_eof() {
            let mut buffer = vec![0u8; 65536];
            let n = f.read(&mut buffer)?;
            // read n bytes
            data.extend_from_slice(&buffer[0..n]);
            println!("Read {} bytes, making {} total", n, data.len());
        }
        if let Ok(s) = std::str::from_utf8(&data) {
            println!("{}", s);
        } else {
            println!("I'm afraid that file isn't UTF-8 encoded");
        }
        Ok(())
    }

    /// print a binary file
    fn hexdump(&mut self, filename: &Path) -> Result<(), Error> {
        let (dir, filename) = self.resolve_filename(filename)?;
        let mut dir = dir.to_directory(&mut self.volume_mgr);
        let mut f = dir.open_file_in_dir(filename, embedded_sdmmc::Mode::ReadOnly)?;
        let mut data = Vec::new();
        while !f.is_eof() {
            let mut buffer = vec![0u8; 65536];
            let n = f.read(&mut buffer)?;
            // read n bytes
            data.extend_from_slice(&buffer[0..n]);
            println!("Read {} bytes, making {} total", n, data.len());
        }
        for (idx, chunk) in data.chunks(16).enumerate() {
            print!("{:08x} | ", idx * 16);
            for b in chunk {
                print!("{:02x} ", b);
            }
            for _padding in 0..(16 - chunk.len()) {
                print!("   ");
            }
            print!("| ");
            for b in chunk {
                print!(
                    "{}",
                    if b.is_ascii_graphic() {
                        *b as char
                    } else {
                        '.'
                    }
                );
            }
            println!();
        }
        Ok(())
    }

    /// create a directory
    fn mkdir(&mut self, dir_name: &Path) -> Result<(), Error> {
        let (dir, filename) = self.resolve_filename(dir_name)?;
        let mut dir = dir.to_directory(&mut self.volume_mgr);
        dir.make_dir_in_dir(filename)
    }

    fn process_line(&mut self, line: &str) -> Result<(), Error> {
        if line == "help" {
            self.help()?;
        } else if line == "A:" || line == "a:" {
            self.current_volume = 0;
        } else if line == "B:" || line == "b:" {
            self.current_volume = 1;
        } else if line == "C:" || line == "c:" {
            self.current_volume = 2;
        } else if line == "D:" || line == "d:" {
            self.current_volume = 3;
        } else if line == "dir" {
            self.dir(Path::new("."))?;
        } else if let Some(path) = line.strip_prefix("dir ") {
            self.dir(Path::new(path.trim()))?;
        } else if line == "tree" {
            self.tree(Path::new("."))?;
        } else if let Some(path) = line.strip_prefix("tree ") {
            self.tree(Path::new(path.trim()))?;
        } else if line == "stat" {
            self.stat()?;
        } else if let Some(path) = line.strip_prefix("cd ") {
            self.cd(Path::new(path.trim()))?;
        } else if let Some(path) = line.strip_prefix("cat ") {
            self.cat(Path::new(path.trim()))?;
        } else if let Some(path) = line.strip_prefix("hexdump ") {
            self.hexdump(Path::new(path.trim()))?;
        } else if let Some(path) = line.strip_prefix("mkdir ") {
            self.mkdir(Path::new(path.trim()))?;
        } else {
            println!("Unknown command {line:?} - try 'help' for help");
        }
        Ok(())
    }

    /// Resolves an existing directory.
    ///
    /// Converts a string path into a directory handle.
    ///
    /// * Bare names (no leading `.`, `/` or `N:/`) are mapped to the current
    ///   directory in the current volume.
    /// * Relative names, like `../SOMEDIR` or `./SOMEDIR`, traverse
    ///   starting at the current volume and directory.
    /// * Absolute, like `B:/SOMEDIR/OTHERDIR` start at the given volume.
    fn resolve_existing_directory(&mut self, full_path: &Path) -> Result<RawDirectory, Error> {
        let (dir, fragment) = self.resolve_filename(full_path)?;
        let mut work_dir = dir.to_directory(&mut self.volume_mgr);
        work_dir.change_dir(fragment)?;
        Ok(work_dir.to_raw_directory())
    }

    /// Either get the volume from the path, or pick the current volume.
    fn resolve_volume(&self, path: &Path) -> Result<usize, Error> {
        match path.volume() {
            None => Ok(self.current_volume),
            Some('A' | 'a') => Ok(0),
            Some('B' | 'b') => Ok(1),
            Some('C' | 'c') => Ok(2),
            Some('D' | 'd') => Ok(3),
            Some(_) => Err(Error::NoSuchVolume),
        }
    }

    /// Resolves a filename.
    ///
    /// Converts a string path into a directory handle and a name within that
    /// directory (that may or may not exist).
    ///
    /// * Bare names (no leading `.`, `/` or `N:/`) are mapped to the current
    ///   directory in the current volume.
    /// * Relative names, like `../SOMEDIR/SOMEFILE` or `./SOMEDIR/SOMEFILE`, traverse
    ///   starting at the current volume and directory.
    /// * Absolute, like `B:/SOMEDIR/SOMEFILE` start at the given volume.
    fn resolve_filename<'path>(
        &mut self,
        full_path: &'path Path,
    ) -> Result<(RawDirectory, &'path str), Error> {
        let volume_idx = self.resolve_volume(full_path)?;
        let Some(s) = &mut self.volumes[volume_idx] else {
            return Err(Error::NoSuchVolume);
        };
        let mut work_dir = if full_path.is_absolute() {
            // relative to root
            self.volume_mgr
                .open_root_dir(s.volume)?
                .to_directory(&mut self.volume_mgr)
        } else {
            // relative to CWD
            self.volume_mgr
                .open_dir(s.directory, ".")?
                .to_directory(&mut self.volume_mgr)
        };

        for fragment in full_path.iterate_dirs() {
            work_dir.change_dir(fragment)?;
        }
        Ok((
            work_dir.to_raw_directory(),
            full_path.basename().unwrap_or("."),
        ))
    }

    /// Convert a volume index to a letter
    fn volume_to_letter(volume: usize) -> char {
        match volume {
            0 => 'A',
            1 => 'B',
            2 => 'C',
            3 => 'D',
            _ => panic!("Invalid volume ID"),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        for v in self.volumes.iter_mut() {
            if let Some(v) = v {
                println!("Closing directory {:?}", v.directory);
                self.volume_mgr
                    .close_dir(v.directory)
                    .expect("Closing directory");
                println!("Closing volume {:?}", v.volume);
                self.volume_mgr
                    .close_volume(v.volume)
                    .expect("Closing volume");
            }
            *v = None;
        }
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    println!("Opening '{filename}'...");
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let stdin = std::io::stdin();

    let mut ctx = Context {
        volume_mgr: VolumeManager::new_with_limits(lbd, Clock, 100),
        volumes: [None, None, None, None],
        current_volume: 0,
    };

    let mut current_volume = None;
    for volume_no in 0..4 {
        match ctx.volume_mgr.open_raw_volume(VolumeIdx(volume_no)) {
            Ok(volume) => {
                println!("Volume # {}: found", Context::volume_to_letter(volume_no));
                match ctx.volume_mgr.open_root_dir(volume) {
                    Ok(root_dir) => {
                        ctx.volumes[volume_no] = Some(VolumeState {
                            directory: root_dir,
                            volume,
                            path: vec![],
                        });
                        if current_volume.is_none() {
                            current_volume = Some(volume_no);
                        }
                    }
                    Err(e) => {
                        println!("Failed to open root directory: {e:?}");
                        ctx.volume_mgr.close_volume(volume).expect("close volume");
                    }
                }
            }
            Err(e) => {
                println!("Failed to open volume {volume_no}: {e:?}");
            }
        }
    }

    match current_volume {
        Some(n) => {
            // Default to the first valid partition
            ctx.current_volume = n;
        }
        None => {
            println!("No volumes found in file. Sorry.");
            return Ok(());
        }
    };

    loop {
        print!("{}:/", Context::volume_to_letter(ctx.current_volume));
        print!("{}", ctx.current_path().join("/"));
        print!("> ");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        stdin.read_line(&mut line)?;
        let line = line.trim();
        if line == "quit" {
            break;
        } else if let Err(e) = ctx.process_line(line) {
            println!("Error: {:?}", e);
        }
    }

    println!("Bye!");
    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************

//! A simple shell demo for embedded-sdmmc
//!
//! Presents a basic command prompt which implements some basic MS-DOS style shell commands.

use std::io::prelude::*;

use embedded_sdmmc::{Error, RawDirectory, RawVolume, VolumeIdx, VolumeManager};

use crate::linux::{Clock, LinuxBlockDevice};

mod linux;

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

    fn process_line(&mut self, line: &str) -> Result<(), Error<std::io::Error>> {
        if line == "help" {
            println!("Commands:");
            println!("\thelp                -> this help text");
            println!("\t<volume>:           -> change volume/partition");
            println!("\tdir                 -> do a directory listing");
            println!("\tstat                -> print volume manager status");
            println!("\tcat <file>          -> print a text file");
            println!("\thexdump <file>      -> print a binary file");
            println!("\tcd ..               -> go up a level");
            println!("\tcd <dir>            -> change into <dir>");
            println!("\tmkdir <dir>         -> create a directory called <dir>");
            println!("\tquit                -> exits the program");
        } else if line == "0:" {
            self.current_volume = 0;
        } else if line == "1:" {
            self.current_volume = 1;
        } else if line == "2:" {
            self.current_volume = 2;
        } else if line == "3:" {
            self.current_volume = 3;
        } else if line == "stat" {
            println!("Status:\n{:#?}", self.volume_mgr);
        } else if line == "dir" {
            let Some(s) = &self.volumes[self.current_volume] else {
                println!("That volume isn't available");
                return Ok(());
            };
            self.volume_mgr.iterate_dir(s.directory, |entry| {
                println!(
                    "{:12} {:9} {} {} {:X?} {:?}",
                    entry.name,
                    entry.size,
                    entry.ctime,
                    entry.mtime,
                    entry.cluster,
                    entry.attributes
                );
            })?;
        } else if let Some(arg) = line.strip_prefix("cd ") {
            let arg = arg.trim();
            let Some(s) = &mut self.volumes[self.current_volume] else {
                println!("This volume isn't available");
                return Ok(());
            };
            let d = self.volume_mgr.open_dir(s.directory, arg)?;
            self.volume_mgr.close_dir(s.directory)?;
            s.directory = d;
            if arg == ".." {
                s.path.pop();
            } else {
                s.path.push(arg.to_owned());
            }
        } else if let Some(arg) = line.strip_prefix("cat ") {
            let arg = arg.trim();
            let Some(s) = &mut self.volumes[self.current_volume] else {
                println!("This volume isn't available");
                return Ok(());
            };
            let mut f = self
                .volume_mgr
                .open_file_in_dir(s.directory, arg, embedded_sdmmc::Mode::ReadOnly)?
                .to_file(&mut self.volume_mgr);
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
        } else if let Some(arg) = line.strip_prefix("hexdump ") {
            let arg = arg.trim();
            let Some(s) = &mut self.volumes[self.current_volume] else {
                println!("This volume isn't available");
                return Ok(());
            };
            let mut f = self
                .volume_mgr
                .open_file_in_dir(s.directory, arg, embedded_sdmmc::Mode::ReadOnly)?
                .to_file(&mut self.volume_mgr);
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
        } else if let Some(arg) = line.strip_prefix("mkdir ") {
            let arg = arg.trim();
            let Some(s) = &mut self.volumes[self.current_volume] else {
                println!("This volume isn't available");
                return Ok(());
            };
            // make the dir
            self.volume_mgr.make_dir_in_dir(s.directory, arg)?;
        } else {
            println!("Unknown command {line:?} - try 'help' for help");
        }
        Ok(())
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

fn main() -> Result<(), Error<std::io::Error>> {
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
                println!("Volume # {}: found", volume_no,);
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
        print!("{}:/", ctx.current_volume);
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

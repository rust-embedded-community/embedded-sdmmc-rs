//! A simple shell demo for embedded-sdmmc
//!
//! Presents a basic command prompt which implements some basic MS-DOS style shell commands.

use std::io::prelude::*;

use embedded_sdmmc::{Directory, Error, Volume, VolumeIdx, VolumeManager};

use crate::linux::{Clock, LinuxBlockDevice};

mod linux;

struct State {
    directory: Directory,
    volume: Volume,
}

fn main() -> Result<(), Error<std::io::Error>> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    println!("Opening '{filename}'...");
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let mut volume_mgr: VolumeManager<LinuxBlockDevice, Clock, 8, 8, 4> =
        VolumeManager::new_with_limits(lbd, Clock, 100);
    let stdin = std::io::stdin();
    let mut volumes: [Option<State>; 4] = [None, None, None, None];

    let mut current_volume = None;
    for volume_no in 0..4 {
        match volume_mgr.open_volume(VolumeIdx(volume_no)) {
            Ok(volume) => {
                println!("Volume # {}: found", volume_no,);
                match volume_mgr.open_root_dir(volume) {
                    Ok(root_dir) => {
                        volumes[volume_no] = Some(State {
                            directory: root_dir,
                            volume,
                        });
                        if current_volume.is_none() {
                            current_volume = Some(volume_no);
                        }
                    }
                    Err(e) => {
                        println!("Failed to open root directory: {e:?}");
                        volume_mgr.close_volume(volume).expect("close volume");
                    }
                }
            }
            Err(e) => {
                println!("Failed to open volume {volume_no}: {e:?}");
            }
        }
    }

    let Some(mut current_volume) = current_volume else {
        println!("No volumes found in file. Sorry.");
        return Ok(());
    };

    loop {
        print!("{}:> ", current_volume);
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        stdin.read_line(&mut line)?;
        let line = line.trim();
        if line == "quit" {
            break;
        } else if line == "help" {
            println!("Commands:");
            println!("\thelp                -> this help text");
            println!("\t<volume>:           -> change volume/partition");
            println!("\tdir                 -> do a directory listing");
            println!("\tquit                -> exits the program");
        } else if line == "0:" {
            current_volume = 0;
        } else if line == "1:" {
            current_volume = 1;
        } else if line == "2:" {
            current_volume = 2;
        } else if line == "3:" {
            current_volume = 3;
        } else if line == "stat" {
            println!("Status:\n{volume_mgr:#?}");
        } else if line == "dir" {
            let Some(s) = &volumes[current_volume] else {
                println!("That volume isn't available");
                continue;
            };
            let r = volume_mgr.iterate_dir(s.directory, |entry| {
                println!(
                    "{:12} {:9} {} {}",
                    entry.name,
                    entry.size,
                    entry.mtime,
                    if entry.attributes.is_directory() {
                        "<DIR>"
                    } else {
                        ""
                    }
                );
            });
            handle("iterating directory", r);
        } else if let Some(arg) = line.strip_prefix("cd ") {
            let Some(s) = &mut volumes[current_volume] else {
                println!("This volume isn't available");
                continue;
            };
            match volume_mgr.open_dir(s.directory, arg) {
                Ok(d) => {
                    let r = volume_mgr.close_dir(s.directory);
                    handle("closing old directory", r);
                    s.directory = d;
                }
                Err(e) => {
                    handle("changing directory", Err(e));
                }
            }
        } else {
            println!("Unknown command {line:?} - try 'help' for help");
        }
    }

    for (idx, s) in volumes.into_iter().enumerate() {
        if let Some(state) = s {
            println!("Unmounting {idx}...");
            let r = volume_mgr.close_dir(state.directory);
            handle("closing directory", r);
            let r = volume_mgr.close_volume(state.volume);
            handle("closing volume", r);
            println!("Unmounted {idx}!");
        }
    }

    println!("Bye!");
    Ok(())
}

fn handle(operation: &str, r: Result<(), Error<std::io::Error>>) {
    if let Err(e) = r {
        println!("Error {operation}: {e:?}");
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************

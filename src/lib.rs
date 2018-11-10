//! embedded-sdmmc: A SD/MMC Library written in Embedded Rust

#![no_std]

/// Represents a standard 512 byte block/sector.
pub struct Block {
    _contents: [u8; 512],
}

/// Represents a block device which is <= 2 TiB in size.
pub trait BlockDevice {
    type Error;
    fn read(&mut self, block: &mut Block, block_idx: u32) -> Result<(), Self::Error>;
    fn write(&mut self, block: &Block, block_idx: u32) -> Result<(), Self::Error>;
}

pub struct Controller {
    _x: ()
}

pub struct Card {
    _x: (),
}

pub struct Volume {
   _name: [u8; 11],
}

pub struct Directory<'a> {
   _parent: &'a Volume,
}

pub struct DirEntry {
   pub name: [u8; 11],
   pub mtine: u32,
   pub ctime: u32,
   pub attributes: u8,
}

pub struct File<'a> {
   _parent: &'a Volume,
   _offset: u32,
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

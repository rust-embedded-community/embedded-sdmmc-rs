//! FAT16/FAT32 file system implementation
//!
//! Implements the File Allocation Table file system. Supports FAT16 and FAT32 volumes.

/// Number of entries reserved at the start of a File Allocation Table
pub const RESERVED_ENTRIES: u32 = 2;

/// Indentifies the supported types of FAT format
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FatType {
    /// FAT16 Format
    Fat16,
    /// FAT32 Format
    Fat32,
}

pub(crate) struct BlockCache {
    block: Block,
    idx: Option<BlockIdx>,
}
impl BlockCache {
    pub fn empty() -> Self {
        BlockCache {
            block: Block::new(),
            idx: None,
        }
    }
    pub(crate) fn read<D>(
        &mut self,
        block_device: &D,
        block_idx: BlockIdx,
        reason: &str,
    ) -> Result<&Block, Error<D::Error>>
    where
        D: BlockDevice,
    {
        if Some(block_idx) != self.idx {
            self.idx = Some(block_idx);
            block_device
                .read(core::slice::from_mut(&mut self.block), block_idx, reason)
                .map_err(Error::DeviceError)?;
        }
        Ok(&self.block)
    }
}

mod bpb;
mod info;
mod ondiskdirentry;
mod volume;

pub use bpb::Bpb;
pub use info::{Fat16Info, Fat32Info, FatSpecificInfo, InfoSector};
pub use ondiskdirentry::OnDiskDirEntry;
pub use volume::{parse_volume, FatVolume, VolumeName};

use crate::{Block, BlockDevice, BlockIdx, Error};

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod test {

    use super::*;
    use crate::{Attributes, BlockIdx, ClusterId, DirEntry, ShortFileName, Timestamp};

    fn parse(input: &str) -> Vec<u8> {
        let mut output = Vec::new();
        for line in input.lines() {
            let line = line.trim();
            if !line.is_empty() {
                // 32 bytes per line
                for index in 0..32 {
                    let start = index * 2;
                    let end = start + 1;
                    let piece = &line[start..=end];
                    let value = u8::from_str_radix(piece, 16).unwrap();
                    output.push(value);
                }
            }
        }
        output
    }

    /// This is the first block of this directory listing.
    /// total 19880
    /// -rw-r--r-- 1 jonathan jonathan   10841 2016-03-01 19:56:36.000000000 +0000  bcm2708-rpi-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   11120 2016-03-01 19:56:34.000000000 +0000  bcm2708-rpi-b-plus.dtb
    /// -rw-r--r-- 1 jonathan jonathan   10871 2016-03-01 19:56:36.000000000 +0000  bcm2708-rpi-cm.dtb
    /// -rw-r--r-- 1 jonathan jonathan   12108 2016-03-01 19:56:36.000000000 +0000  bcm2709-rpi-2-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   12575 2016-03-01 19:56:36.000000000 +0000  bcm2710-rpi-3-b.dtb
    /// -rw-r--r-- 1 jonathan jonathan   17920 2016-03-01 19:56:38.000000000 +0000  bootcode.bin
    /// -rw-r--r-- 1 jonathan jonathan     136 2015-11-21 20:28:30.000000000 +0000  cmdline.txt
    /// -rw-r--r-- 1 jonathan jonathan    1635 2015-11-21 20:28:30.000000000 +0000  config.txt
    /// -rw-r--r-- 1 jonathan jonathan   18693 2016-03-01 19:56:30.000000000 +0000  COPYING.linux
    /// -rw-r--r-- 1 jonathan jonathan    2505 2016-03-01 19:56:38.000000000 +0000  fixup_cd.dat
    /// -rw-r--r-- 1 jonathan jonathan    6481 2016-03-01 19:56:38.000000000 +0000  fixup.dat
    /// -rw-r--r-- 1 jonathan jonathan    9722 2016-03-01 19:56:38.000000000 +0000  fixup_db.dat
    /// -rw-r--r-- 1 jonathan jonathan    9724 2016-03-01 19:56:38.000000000 +0000  fixup_x.dat
    /// -rw-r--r-- 1 jonathan jonathan     110 2015-11-21 21:32:06.000000000 +0000  issue.txt
    /// -rw-r--r-- 1 jonathan jonathan 4046732 2016-03-01 19:56:40.000000000 +0000  kernel7.img
    /// -rw-r--r-- 1 jonathan jonathan 3963140 2016-03-01 19:56:38.000000000 +0000  kernel.img
    /// -rw-r--r-- 1 jonathan jonathan    1494 2016-03-01 19:56:34.000000000 +0000  LICENCE.broadcom
    /// -rw-r--r-- 1 jonathan jonathan   18974 2015-11-21 21:32:06.000000000 +0000  LICENSE.oracle
    /// drwxr-xr-x 2 jonathan jonathan    8192 2016-03-01 19:56:54.000000000 +0000  overlays
    /// -rw-r--r-- 1 jonathan jonathan  612472 2016-03-01 19:56:40.000000000 +0000  start_cd.elf
    /// -rw-r--r-- 1 jonathan jonathan 4888200 2016-03-01 19:56:42.000000000 +0000  start_db.elf
    /// -rw-r--r-- 1 jonathan jonathan 2739672 2016-03-01 19:56:40.000000000 +0000  start.elf
    /// -rw-r--r-- 1 jonathan jonathan 3840328 2016-03-01 19:56:44.000000000 +0000  start_x.elf
    /// drwxr-xr-x 2 jonathan jonathan    8192 2015-12-05 21:55:06.000000000 +0000 'System Volume Information'
    #[test]
    fn test_dir_entries() {
        #[derive(Debug)]
        enum Expected {
            Lfn(bool, u8, [char; 13]),
            Short(DirEntry),
        }
        let raw_data = r#"
        626f6f7420202020202020080000699c754775470000699c7547000000000000 boot       ...i.uGuG..i.uG......
        416f007600650072006c000f00476100790073000000ffffffff0000ffffffff Ao.v.e.r.l...Ga.y.s.............
        4f5645524c4159532020201000001b9f6148614800001b9f6148030000000000 OVERLAYS   .....aHaH....aH......
        422d0070006c00750073000f00792e006400740062000000ffff0000ffffffff B-.p.l.u.s...y..d.t.b...........
        01620063006d00320037000f0079300038002d0072007000690000002d006200 .b.c.m.2.7...y0.8.-.r.p.i...-.b.
        42434d3237307e31445442200064119f614861480000119f61480900702b0000 BCM270~1DTB .d..aHaH....aH..p+..
        4143004f005000590049000f00124e0047002e006c0069006e00000075007800 AC.O.P.Y.I....N.G...l.i.n...u.x.
        434f5059494e7e314c494e2000000f9f6148614800000f9f6148050005490000 COPYIN~1LIN ....aHaH....aH...I..
        4263006f006d000000ffff0f0067ffffffffffffffffffffffff0000ffffffff Bc.o.m.......g..................
        014c004900430045004e000f0067430045002e00620072006f00000061006400 .L.I.C.E.N...gC.E...b.r.o...a.d.
        4c4943454e437e3142524f200000119f614861480000119f61480800d6050000 LICENC~1BRO ....aHaH....aH......
        422d0062002e00640074000f001962000000ffffffffffffffff0000ffffffff B-.b...d.t....b.................
        01620063006d00320037000f0019300039002d0072007000690000002d003200 .b.c.m.2.7....0.9.-.r.p.i...-.2.
        42434d3237307e34445442200064129f614861480000129f61480f004c2f0000 BCM270~4DTB .d..aHaH....aH..L/..
        422e0064007400620000000f0059ffffffffffffffffffffffff0000ffffffff B..d.t.b.....Y..................
        01620063006d00320037000f0059300038002d0072007000690000002d006200 .b.c.m.2.7...Y0.8.-.r.p.i...-.b.
        "#;
        let results = [
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str_mixed_case("boot").unwrap(),
                mtime: Timestamp::from_calendar(2015, 11, 21, 19, 35, 18).unwrap(),
                ctime: Timestamp::from_calendar(2015, 11, 21, 19, 35, 18).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::VOLUME),
                cluster: ClusterId(0),
                size: 0,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                1,
                [
                    'o', 'v', 'e', 'r', 'l', 'a', 'y', 's', '\u{0000}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("OVERLAYS").unwrap(),
                mtime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 54).unwrap(),
                ctime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 54).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::DIRECTORY),
                cluster: ClusterId(3),
                size: 0,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '-', 'p', 'l', 'u', 's', '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '8', '-', 'r', 'p', 'i', '-', 'b',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("BCM270~1.DTB").unwrap(),
                mtime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 34).unwrap(),
                ctime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 34).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: ClusterId(9),
                size: 11120,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                1,
                [
                    'C', 'O', 'P', 'Y', 'I', 'N', 'G', '.', 'l', 'i', 'n', 'u', 'x',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("COPYIN~1.LIN").unwrap(),
                mtime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 30).unwrap(),
                ctime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 30).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: ClusterId(5),
                size: 18693,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    'c', 'o', 'm', '\u{0}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'L', 'I', 'C', 'E', 'N', 'C', 'E', '.', 'b', 'r', 'o', 'a', 'd',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("LICENC~1.BRO").unwrap(),
                mtime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 34).unwrap(),
                ctime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 34).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: ClusterId(8),
                size: 1494,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '-', 'b', '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '9', '-', 'r', 'p', 'i', '-', '2',
                ],
            ),
            Expected::Short(DirEntry {
                name: ShortFileName::create_from_str("BCM270~4.DTB").unwrap(),
                mtime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 36).unwrap(),
                ctime: Timestamp::from_calendar(2016, 3, 1, 19, 56, 36).unwrap(),
                attributes: Attributes::create_from_fat(Attributes::ARCHIVE),
                cluster: ClusterId(15),
                size: 12108,
                entry_block: BlockIdx(0),
                entry_offset: 0,
            }),
            Expected::Lfn(
                true,
                2,
                [
                    '.', 'd', 't', 'b', '\u{0000}', '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                    '\u{ffff}', '\u{ffff}', '\u{ffff}', '\u{ffff}',
                ],
            ),
            Expected::Lfn(
                false,
                1,
                [
                    'b', 'c', 'm', '2', '7', '0', '8', '-', 'r', 'p', 'i', '-', 'b',
                ],
            ),
        ];

        let data = parse(raw_data);
        for (part, expected) in data.chunks(OnDiskDirEntry::LEN).zip(results.iter()) {
            let on_disk_entry = OnDiskDirEntry::new(part);
            match expected {
                Expected::Lfn(start, index, contents) if on_disk_entry.is_lfn() => {
                    let (calc_start, calc_index, calc_contents) =
                        on_disk_entry.lfn_contents().unwrap();
                    assert_eq!(*start, calc_start);
                    assert_eq!(*index, calc_index);
                    assert_eq!(*contents, calc_contents);
                }
                Expected::Short(expected_entry) if !on_disk_entry.is_lfn() => {
                    let parsed_entry = on_disk_entry.get_entry(FatType::Fat32, BlockIdx(0), 0);
                    assert_eq!(*expected_entry, parsed_entry);
                }
                _ => {
                    panic!(
                        "Bad dir entry, expected:\n{:#?}\nhad\n{:#?}",
                        expected, on_disk_entry
                    );
                }
            }
        }
    }

    #[test]
    fn test_bpb() {
        // Taken from a Raspberry Pi bootable SD-Card
        const BPB_EXAMPLE: [u8; 512] = hex!(
            "EB 3C 90 6D 6B 66 73 2E 66 61 74 00 02 10 01 00
             02 00 02 00 00 F8 20 00 3F 00 FF 00 00 00 00 00
             00 E0 01 00 80 01 29 BB B0 71 77 62 6F 6F 74 20
             20 20 20 20 20 20 46 41 54 31 36 20 20 20 0E 1F
             BE 5B 7C AC 22 C0 74 0B 56 B4 0E BB 07 00 CD 10
             5E EB F0 32 E4 CD 16 CD 19 EB FE 54 68 69 73 20
             69 73 20 6E 6F 74 20 61 20 62 6F 6F 74 61 62 6C
             65 20 64 69 73 6B 2E 20 20 50 6C 65 61 73 65 20
             69 6E 73 65 72 74 20 61 20 62 6F 6F 74 61 62 6C
             65 20 66 6C 6F 70 70 79 20 61 6E 64 0D 0A 70 72
             65 73 73 20 61 6E 79 20 6B 65 79 20 74 6F 20 74
             72 79 20 61 67 61 69 6E 20 2E 2E 2E 20 0D 0A 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
             00 00 00 00 00 00 00 00 00 00 00 00 00 00 55 AA"
        );
        let bpb = Bpb::create_from_bytes(&BPB_EXAMPLE).unwrap();
        assert_eq!(bpb.footer(), Bpb::FOOTER_VALUE);
        assert_eq!(bpb.oem_name(), b"mkfs.fat");
        assert_eq!(bpb.bytes_per_block(), 512);
        assert_eq!(bpb.blocks_per_cluster(), 16);
        assert_eq!(bpb.reserved_block_count(), 1);
        assert_eq!(bpb.num_fats(), 2);
        assert_eq!(bpb.root_entries_count(), 512);
        assert_eq!(bpb.total_blocks16(), 0);
        assert_eq!(bpb.fat_size16(), 32);
        assert_eq!(bpb.total_blocks32(), 122_880);
        assert_eq!(bpb.footer(), 0xAA55);
        assert_eq!(bpb.volume_label(), b"boot       ");
        assert_eq!(bpb.fat_size(), 32);
        assert_eq!(bpb.total_blocks(), 122_880);
        assert_eq!(bpb.fat_type, FatType::Fat16);
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************

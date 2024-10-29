//! The Volume Manager implementation.
//!
//! The volume manager handles partitions and open files on a block device.

use core::cell::RefCell;
use core::convert::TryFrom;
use core::ops::DerefMut;

use byteorder::{ByteOrder, LittleEndian};
use heapless::Vec;

use crate::{
    debug, fat,
    filesystem::{
        Attributes, ClusterId, DirEntry, DirectoryInfo, FileInfo, HandleGenerator, Mode,
        RawDirectory, RawFile, TimeSource, ToShortFileName, MAX_FILE_SIZE,
    },
    trace, Block, BlockCache, BlockCount, BlockDevice, BlockIdx, Error, RawVolume, ShortFileName,
    Volume, VolumeIdx, VolumeInfo, VolumeType, PARTITION_ID_FAT16, PARTITION_ID_FAT16_LBA,
    PARTITION_ID_FAT32_CHS_LBA, PARTITION_ID_FAT32_LBA,
};

/// Wraps a block device and gives access to the FAT-formatted volumes within
/// it.
///
/// Tracks which files and directories are open, to prevent you from deleting
/// a file or directory you currently have open.
#[derive(Debug)]
pub struct VolumeManager<
    D,
    T,
    const MAX_DIRS: usize = 4,
    const MAX_FILES: usize = 4,
    const MAX_VOLUMES: usize = 1,
> where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    time_source: T,
    data: RefCell<VolumeManagerData<D, MAX_DIRS, MAX_FILES, MAX_VOLUMES>>,
}

impl<D, T> VolumeManager<D, T, 4, 4>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    /// Create a new Volume Manager using a generic `BlockDevice`. From this
    /// object we can open volumes (partitions) and with those we can open
    /// files.
    ///
    /// This creates a `VolumeManager` with default values
    /// MAX_DIRS = 4, MAX_FILES = 4, MAX_VOLUMES = 1. Call `VolumeManager::new_with_limits(block_device, time_source)`
    /// if you need different limits.
    pub fn new(block_device: D, time_source: T) -> VolumeManager<D, T, 4, 4, 1> {
        // Pick a random starting point for the IDs that's not zero, because
        // zero doesn't stand out in the logs.
        Self::new_with_limits(block_device, time_source, 5000)
    }
}

impl<D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
    T: TimeSource,
    <D as BlockDevice>::Error: core::fmt::Debug,
{
    /// Create a new Volume Manager using a generic `BlockDevice`. From this
    /// object we can open volumes (partitions) and with those we can open
    /// files.
    ///
    /// You can also give an offset for all the IDs this volume manager
    /// generates, which might help you find the IDs in your logs when
    /// debugging.
    pub fn new_with_limits(
        block_device: D,
        time_source: T,
        id_offset: u32,
    ) -> VolumeManager<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES> {
        debug!("Creating new embedded-sdmmc::VolumeManager");
        VolumeManager {
            time_source,
            data: RefCell::new(VolumeManagerData {
                block_cache: BlockCache::new(block_device),
                id_generator: HandleGenerator::new(id_offset),
                open_volumes: Vec::new(),
                open_dirs: Vec::new(),
                open_files: Vec::new(),
            }),
        }
    }

    /// Temporarily get access to the underlying block device.
    pub fn device<F>(&self, f: F) -> T
    where
        F: FnOnce(&mut D) -> T,
    {
        let mut data = self.data.borrow_mut();
        let result = f(data.block_cache.block_device());
        result
    }

    /// Get a volume (or partition) based on entries in the Master Boot Record.
    ///
    /// We do not support GUID Partition Table disks. Nor do we support any
    /// concept of drive letters - that is for a higher layer to handle.
    pub fn open_volume(
        &self,
        volume_idx: VolumeIdx,
    ) -> Result<Volume<D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, Error<D::Error>> {
        let v = self.open_raw_volume(volume_idx)?;
        Ok(v.to_volume(self))
    }

    /// Get a volume (or partition) based on entries in the Master Boot Record.
    ///
    /// We do not support GUID Partition Table disks. Nor do we support any
    /// concept of drive letters - that is for a higher layer to handle.
    ///
    /// This function gives you a `RawVolume` and you must close the volume by
    /// calling `VolumeManager::close_volume`.
    pub fn open_raw_volume(&self, volume_idx: VolumeIdx) -> Result<RawVolume, Error<D::Error>> {
        const PARTITION1_START: usize = 446;
        const PARTITION2_START: usize = PARTITION1_START + PARTITION_INFO_LENGTH;
        const PARTITION3_START: usize = PARTITION2_START + PARTITION_INFO_LENGTH;
        const PARTITION4_START: usize = PARTITION3_START + PARTITION_INFO_LENGTH;
        const FOOTER_START: usize = 510;
        const FOOTER_VALUE: u16 = 0xAA55;
        const PARTITION_INFO_LENGTH: usize = 16;
        const PARTITION_INFO_STATUS_INDEX: usize = 0;
        const PARTITION_INFO_TYPE_INDEX: usize = 4;
        const PARTITION_INFO_LBA_START_INDEX: usize = 8;
        const PARTITION_INFO_NUM_BLOCKS_INDEX: usize = 12;

        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;

        if data.open_volumes.is_full() {
            return Err(Error::TooManyOpenVolumes);
        }

        for v in data.open_volumes.iter() {
            if v.idx == volume_idx {
                return Err(Error::VolumeAlreadyOpen);
            }
        }

        let (part_type, lba_start, num_blocks) = {
            trace!("Reading partition table");
            let block = data
                .block_cache
                .read(BlockIdx(0))
                .map_err(Error::DeviceError)?;
            // We only support Master Boot Record (MBR) partitioned cards, not
            // GUID Partition Table (GPT)
            if LittleEndian::read_u16(&block[FOOTER_START..FOOTER_START + 2]) != FOOTER_VALUE {
                return Err(Error::FormatError("Invalid MBR signature"));
            }
            let partition = match volume_idx {
                VolumeIdx(0) => {
                    &block[PARTITION1_START..(PARTITION1_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(1) => {
                    &block[PARTITION2_START..(PARTITION2_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(2) => {
                    &block[PARTITION3_START..(PARTITION3_START + PARTITION_INFO_LENGTH)]
                }
                VolumeIdx(3) => {
                    &block[PARTITION4_START..(PARTITION4_START + PARTITION_INFO_LENGTH)]
                }
                _ => {
                    return Err(Error::NoSuchVolume);
                }
            };
            // Only 0x80 and 0x00 are valid (bootable, and non-bootable)
            if (partition[PARTITION_INFO_STATUS_INDEX] & 0x7F) != 0x00 {
                return Err(Error::FormatError("Invalid partition status"));
            }
            let lba_start = LittleEndian::read_u32(
                &partition[PARTITION_INFO_LBA_START_INDEX..(PARTITION_INFO_LBA_START_INDEX + 4)],
            );
            let num_blocks = LittleEndian::read_u32(
                &partition[PARTITION_INFO_NUM_BLOCKS_INDEX..(PARTITION_INFO_NUM_BLOCKS_INDEX + 4)],
            );
            (
                partition[PARTITION_INFO_TYPE_INDEX],
                BlockIdx(lba_start),
                BlockCount(num_blocks),
            )
        };
        match part_type {
            PARTITION_ID_FAT32_CHS_LBA
            | PARTITION_ID_FAT32_LBA
            | PARTITION_ID_FAT16_LBA
            | PARTITION_ID_FAT16 => {
                let volume = fat::parse_volume(&mut data.block_cache, lba_start, num_blocks)?;
                let id = RawVolume(data.id_generator.generate());
                let info = VolumeInfo {
                    raw_volume: id,
                    idx: volume_idx,
                    volume_type: volume,
                };
                // We already checked for space
                data.open_volumes.push(info).unwrap();
                Ok(id)
            }
            _ => Err(Error::FormatError("Partition type not supported")),
        }
    }

    /// Open the volume's root directory.
    ///
    /// You can then read the directory entries with `iterate_dir`, or you can
    /// use `open_file_in_dir`.
    pub fn open_root_dir(&self, volume: RawVolume) -> Result<RawDirectory, Error<D::Error>> {
        debug!("Opening root on {:?}", volume);

        // Opening a root directory twice is OK
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;

        let directory_id = RawDirectory(data.id_generator.generate());
        let dir_info = DirectoryInfo {
            raw_volume: volume,
            cluster: ClusterId::ROOT_DIR,
            raw_directory: directory_id,
        };

        data.open_dirs
            .push(dir_info)
            .map_err(|_| Error::TooManyOpenDirs)?;

        debug!("Opened root on {:?}, got {:?}", volume, directory_id);

        Ok(directory_id)
    }

    /// Open a directory.
    ///
    /// You can then read the directory entries with `iterate_dir` and `open_file_in_dir`.
    ///
    /// Passing "." as the name results in opening the `parent_dir` a second time.
    pub fn open_dir<N>(
        &self,
        parent_dir: RawDirectory,
        name: N,
    ) -> Result<RawDirectory, Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        if data.open_dirs.is_full() {
            return Err(Error::TooManyOpenDirs);
        }

        // Find dir by ID
        let parent_dir_idx = data.get_dir_by_id(parent_dir)?;
        let volume_idx = data.get_volume_by_id(data.open_dirs[parent_dir_idx].raw_volume)?;
        let short_file_name = name.to_short_filename().map_err(Error::FilenameError)?;

        // Open the directory

        // Should we short-cut? (root dir doesn't have ".")
        if short_file_name == ShortFileName::this_dir() {
            let directory_id = RawDirectory(data.id_generator.generate());
            let dir_info = DirectoryInfo {
                raw_directory: directory_id,
                raw_volume: data.open_volumes[volume_idx].raw_volume,
                cluster: data.open_dirs[parent_dir_idx].cluster,
            };

            data.open_dirs
                .push(dir_info)
                .map_err(|_| Error::TooManyOpenDirs)?;

            return Ok(directory_id);
        }

        // ok we'll actually look for the directory then

        let dir_entry = match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(
                &mut data.block_cache,
                &data.open_dirs[parent_dir_idx],
                &short_file_name,
            )?,
        };

        debug!("Found dir entry: {:?}", dir_entry);

        if !dir_entry.attributes.is_directory() {
            return Err(Error::OpenedFileAsDir);
        }

        // We don't check if the directory is already open - directories hold
        // no cached state and so opening a directory twice is allowable.

        // Remember this open directory.
        let directory_id = RawDirectory(data.id_generator.generate());
        let dir_info = DirectoryInfo {
            raw_directory: directory_id,
            raw_volume: data.open_volumes[volume_idx].raw_volume,
            cluster: dir_entry.cluster,
        };

        data.open_dirs
            .push(dir_info)
            .map_err(|_| Error::TooManyOpenDirs)?;

        Ok(directory_id)
    }

    /// Close a directory. You cannot perform operations on an open directory
    /// and so must close it if you want to do something with it.
    pub fn close_dir(&self, directory: RawDirectory) -> Result<(), Error<D::Error>> {
        debug!("Closing {:?}", directory);
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;

        for (idx, info) in data.open_dirs.iter().enumerate() {
            if directory == info.raw_directory {
                data.open_dirs.swap_remove(idx);
                return Ok(());
            }
        }
        Err(Error::BadHandle)
    }

    /// Close a volume
    ///
    /// You can't close it if there are any files or directories open on it.
    pub fn close_volume(&self, volume: RawVolume) -> Result<(), Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;

        for f in data.open_files.iter() {
            if f.raw_volume == volume {
                return Err(Error::VolumeStillInUse);
            }
        }

        for d in data.open_dirs.iter() {
            if d.raw_volume == volume {
                return Err(Error::VolumeStillInUse);
            }
        }

        let volume_idx = data.get_volume_by_id(volume)?;

        data.open_volumes.swap_remove(volume_idx);

        Ok(())
    }

    /// Look in a directory for a named file.
    pub fn find_directory_entry<N>(
        &self,
        directory: RawDirectory,
        name: N,
    ) -> Result<DirEntry, Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        let directory_idx = data.get_dir_by_id(directory)?;
        let volume_idx = data.get_volume_by_id(data.open_dirs[directory_idx].raw_volume)?;
        match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => {
                let sfn = name.to_short_filename().map_err(Error::FilenameError)?;
                fat.find_directory_entry(
                    &mut data.block_cache,
                    &data.open_dirs[directory_idx],
                    &sfn,
                )
            }
        }
    }

    /// Call a callback function for each directory entry in a directory.
    ///
    /// <div class="warning">
    ///
    /// Do not attempt to call any methods on the VolumeManager or any of its
    /// handles from inside the callback. You will get a lock error because the
    /// object is already locked in order to do the iteration.
    ///
    /// </div>
    pub fn iterate_dir<F>(&self, directory: RawDirectory, func: F) -> Result<(), Error<D::Error>>
    where
        F: FnMut(&DirEntry),
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        let directory_idx = data.get_dir_by_id(directory)?;
        let volume_idx = data.get_volume_by_id(data.open_dirs[directory_idx].raw_volume)?;
        match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => {
                fat.iterate_dir(&mut data.block_cache, &data.open_dirs[directory_idx], func)
            }
        }
    }

    /// Open a file with the given full path. A file can only be opened once.
    pub fn open_file_in_dir<N>(
        &self,
        directory: RawDirectory,
        name: N,
        mode: Mode,
    ) -> Result<RawFile, Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        // This check is load-bearing - we do an unchecked push later.
        if data.open_files.is_full() {
            return Err(Error::TooManyOpenFiles);
        }

        let directory_idx = data.get_dir_by_id(directory)?;
        let volume_id = data.open_dirs[directory_idx].raw_volume;
        let volume_idx = data.get_volume_by_id(volume_id)?;
        let volume_info = &data.open_volumes[volume_idx];
        let sfn = name.to_short_filename().map_err(Error::FilenameError)?;

        let dir_entry = match &volume_info.volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(
                &mut data.block_cache,
                &data.open_dirs[directory_idx],
                &sfn,
            ),
        };

        let dir_entry = match dir_entry {
            Ok(entry) => {
                // we are opening an existing file
                Some(entry)
            }
            Err(_)
                if (mode == Mode::ReadWriteCreate)
                    | (mode == Mode::ReadWriteCreateOrTruncate)
                    | (mode == Mode::ReadWriteCreateOrAppend) =>
            {
                // We are opening a non-existant file, but that's OK because they
                // asked us to create it
                None
            }
            _ => {
                // We are opening a non-existant file, and that's not OK.
                return Err(Error::NotFound);
            }
        };

        // Check if it's open already
        if let Some(dir_entry) = &dir_entry {
            if data.file_is_open(volume_info.raw_volume, dir_entry) {
                return Err(Error::FileAlreadyOpen);
            }
        }

        let mode = solve_mode_variant(mode, dir_entry.is_some());

        match mode {
            Mode::ReadWriteCreate => {
                if dir_entry.is_some() {
                    return Err(Error::FileAlreadyExists);
                }
                let cluster = data.open_dirs[directory_idx].cluster;
                let att = Attributes::create_from_fat(0);
                let volume_idx = data.get_volume_by_id(volume_id)?;
                let entry = match &mut data.open_volumes[volume_idx].volume_type {
                    VolumeType::Fat(fat) => fat.write_new_directory_entry(
                        &mut data.block_cache,
                        &self.time_source,
                        cluster,
                        sfn,
                        att,
                    )?,
                };

                let file_id = RawFile(data.id_generator.generate());

                let file = FileInfo {
                    raw_file: file_id,
                    raw_volume: volume_id,
                    current_cluster: (0, entry.cluster),
                    current_offset: 0,
                    mode,
                    entry,
                    dirty: false,
                };

                // Remember this open file - can't be full as we checked already
                unsafe {
                    data.open_files.push_unchecked(file);
                }

                Ok(file_id)
            }
            _ => {
                // Safe to unwrap, since we actually have an entry if we got here
                let dir_entry = dir_entry.unwrap();

                if dir_entry.attributes.is_read_only() && mode != Mode::ReadOnly {
                    return Err(Error::ReadOnly);
                }

                if dir_entry.attributes.is_directory() {
                    return Err(Error::OpenedDirAsFile);
                }

                // Check it's not already open
                if data.file_is_open(volume_id, &dir_entry) {
                    return Err(Error::FileAlreadyOpen);
                }

                let mode = solve_mode_variant(mode, true);
                let raw_file = RawFile(data.id_generator.generate());

                let file = match mode {
                    Mode::ReadOnly => FileInfo {
                        raw_file,
                        raw_volume: volume_id,
                        current_cluster: (0, dir_entry.cluster),
                        current_offset: 0,
                        mode,
                        entry: dir_entry,
                        dirty: false,
                    },
                    Mode::ReadWriteAppend => {
                        let mut file = FileInfo {
                            raw_file,
                            raw_volume: volume_id,
                            current_cluster: (0, dir_entry.cluster),
                            current_offset: 0,
                            mode,
                            entry: dir_entry,
                            dirty: false,
                        };
                        // seek_from_end with 0 can't fail
                        file.seek_from_end(0).ok();
                        file
                    }
                    Mode::ReadWriteTruncate => {
                        let mut file = FileInfo {
                            raw_file,
                            raw_volume: volume_id,
                            current_cluster: (0, dir_entry.cluster),
                            current_offset: 0,
                            mode,
                            entry: dir_entry,
                            dirty: false,
                        };
                        match &mut data.open_volumes[volume_idx].volume_type {
                            VolumeType::Fat(fat) => fat.truncate_cluster_chain(
                                &mut data.block_cache,
                                file.entry.cluster,
                            )?,
                        };
                        file.update_length(0);
                        match &data.open_volumes[volume_idx].volume_type {
                            VolumeType::Fat(fat) => {
                                file.entry.mtime = self.time_source.get_timestamp();
                                fat.write_entry_to_disk(&mut data.block_cache, &file.entry)?;
                            }
                        };

                        file
                    }
                    _ => return Err(Error::Unsupported),
                };

                // Remember this open file - can't be full as we checked already
                unsafe {
                    data.open_files.push_unchecked(file);
                }

                Ok(raw_file)
            }
        }
    }

    /// Delete a closed file with the given filename, if it exists.
    pub fn delete_file_in_dir<N>(
        &self,
        directory: RawDirectory,
        name: N,
    ) -> Result<(), Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        let dir_idx = data.get_dir_by_id(directory)?;
        let dir_info = &data.open_dirs[dir_idx];
        let volume_idx = data.get_volume_by_id(dir_info.raw_volume)?;
        let sfn = name.to_short_filename().map_err(Error::FilenameError)?;

        let dir_entry = match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => fat.find_directory_entry(&mut data.block_cache, dir_info, &sfn),
        }?;

        if dir_entry.attributes.is_directory() {
            return Err(Error::DeleteDirAsFile);
        }

        if data.file_is_open(dir_info.raw_volume, &dir_entry) {
            return Err(Error::FileAlreadyOpen);
        }

        let volume_idx = data.get_volume_by_id(dir_info.raw_volume)?;
        match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => {
                fat.delete_directory_entry(&mut data.block_cache, dir_info, &sfn)?
            }
        }

        Ok(())
    }

    /// Get the volume label
    ///
    /// Will look in the BPB for a volume label, and if nothing is found, will
    /// search the root directory for a volume label.
    pub fn get_root_volume_label(
        &self,
        raw_volume: RawVolume,
    ) -> Result<Option<crate::VolumeName>, Error<D::Error>> {
        debug!("Reading volume label for {:?}", raw_volume);
        // prefer the one in the BPB - it's easier to get
        let data = self.data.try_borrow().map_err(|_| Error::LockError)?;
        let volume_idx = data.get_volume_by_id(raw_volume)?;
        match &data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => {
                if !fat.name.name().is_empty() {
                    debug!(
                        "Got volume label {:?} for {:?} from BPB",
                        fat.name, raw_volume
                    );
                    return Ok(Some(fat.name.clone()));
                }
            }
        }
        drop(data);

        // Nothing in the BPB, let's do it the slow way
        let root_dir = self.open_root_dir(raw_volume)?.to_directory(self);
        let mut maybe_volume_name = None;
        root_dir.iterate_dir(|de| {
            if maybe_volume_name.is_none()
                && de.attributes == Attributes::create_from_fat(Attributes::VOLUME)
            {
                maybe_volume_name = Some(unsafe { de.name.clone().to_volume_label() })
            }
        })?;

        debug!(
            "Got volume label {:?} for {:?} from root",
            maybe_volume_name, raw_volume
        );

        Ok(maybe_volume_name)
    }

    /// Read from an open file.
    pub fn read(&self, file: RawFile, buffer: &mut [u8]) -> Result<usize, Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        let file_idx = data.get_file_by_id(file)?;
        let volume_idx = data.get_volume_by_id(data.open_files[file_idx].raw_volume)?;

        // Calculate which file block the current offset lies within
        // While there is more to read, read the block and copy in to the buffer.
        // If we need to find the next cluster, walk the FAT.
        let mut space = buffer.len();
        let mut read = 0;
        while space > 0 && !data.open_files[file_idx].eof() {
            let mut current_cluster = data.open_files[file_idx].current_cluster;
            let (block_idx, block_offset, block_avail) = data.find_data_on_disk(
                volume_idx,
                &mut current_cluster,
                data.open_files[file_idx].entry.cluster,
                data.open_files[file_idx].current_offset,
            )?;
            data.open_files[file_idx].current_cluster = current_cluster;
            trace!("Reading file ID {:?}", file);
            let block = data
                .block_cache
                .read(block_idx)
                .map_err(Error::DeviceError)?;
            let to_copy = block_avail
                .min(space)
                .min(data.open_files[file_idx].left() as usize);
            assert!(to_copy != 0);
            buffer[read..read + to_copy]
                .copy_from_slice(&block[block_offset..block_offset + to_copy]);
            read += to_copy;
            space -= to_copy;
            data.open_files[file_idx]
                .seek_from_current(to_copy as i32)
                .unwrap();
        }
        Ok(read)
    }

    /// Write to a open file.
    pub fn write(&self, file: RawFile, buffer: &[u8]) -> Result<(), Error<D::Error>> {
        #[cfg(feature = "defmt-log")]
        debug!("write(file={:?}, buffer={:x}", file, buffer);

        #[cfg(feature = "log")]
        debug!("write(file={:?}, buffer={:x?}", file, buffer);

        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        // Clone this so we can touch our other structures. Need to ensure we
        // write it back at the end.
        let file_idx = data.get_file_by_id(file)?;
        let volume_idx = data.get_volume_by_id(data.open_files[file_idx].raw_volume)?;

        if data.open_files[file_idx].mode == Mode::ReadOnly {
            return Err(Error::ReadOnly);
        }

        data.open_files[file_idx].dirty = true;

        if data.open_files[file_idx].entry.cluster.0 < fat::RESERVED_ENTRIES {
            // file doesn't have a valid allocated cluster (possible zero-length file), allocate one
            data.open_files[file_idx].entry.cluster =
                match data.open_volumes[volume_idx].volume_type {
                    VolumeType::Fat(ref mut fat) => {
                        fat.alloc_cluster(&mut data.block_cache, None, false)?
                    }
                };
            debug!(
                "Alloc first cluster {:?}",
                data.open_files[file_idx].entry.cluster
            );
        }

        // Clone this so we can touch our other structures.
        let volume_idx = data.get_volume_by_id(data.open_files[file_idx].raw_volume)?;

        if (data.open_files[file_idx].current_cluster.1) < data.open_files[file_idx].entry.cluster {
            debug!("Rewinding to start");
            data.open_files[file_idx].current_cluster =
                (0, data.open_files[file_idx].entry.cluster);
        }
        let bytes_until_max =
            usize::try_from(MAX_FILE_SIZE - data.open_files[file_idx].current_offset)
                .map_err(|_| Error::ConversionError)?;
        let bytes_to_write = core::cmp::min(buffer.len(), bytes_until_max);
        let mut written = 0;

        while written < bytes_to_write {
            let mut current_cluster = data.open_files[file_idx].current_cluster;
            debug!(
                "Have written bytes {}/{}, finding cluster {:?}",
                written, bytes_to_write, current_cluster
            );
            let current_offset = data.open_files[file_idx].current_offset;
            let (block_idx, block_offset, block_avail) = match data.find_data_on_disk(
                volume_idx,
                &mut current_cluster,
                data.open_files[file_idx].entry.cluster,
                current_offset,
            ) {
                Ok(vars) => {
                    debug!(
                        "Found block_idx={:?}, block_offset={:?}, block_avail={}",
                        vars.0, vars.1, vars.2
                    );
                    vars
                }
                Err(Error::EndOfFile) => {
                    debug!("Extending file");
                    match data.open_volumes[volume_idx].volume_type {
                        VolumeType::Fat(ref mut fat) => {
                            if fat
                                .alloc_cluster(
                                    &mut data.block_cache,
                                    Some(current_cluster.1),
                                    false,
                                )
                                .is_err()
                            {
                                return Err(Error::DiskFull);
                            }
                            debug!("Allocated new FAT cluster, finding offsets...");
                            let new_offset = data
                                .find_data_on_disk(
                                    volume_idx,
                                    &mut current_cluster,
                                    data.open_files[file_idx].entry.cluster,
                                    data.open_files[file_idx].current_offset,
                                )
                                .map_err(|_| Error::AllocationError)?;
                            debug!("New offset {:?}", new_offset);
                            new_offset
                        }
                    }
                }
                Err(e) => return Err(e),
            };
            let to_copy = core::cmp::min(block_avail, bytes_to_write - written);
            let block = if block_offset != 0 {
                debug!("Reading for partial block write");
                data.block_cache
                    .read_mut(block_idx)
                    .map_err(Error::DeviceError)?
            } else {
                data.block_cache.blank_mut(block_idx)
            };
            block[block_offset..block_offset + to_copy]
                .copy_from_slice(&buffer[written..written + to_copy]);
            debug!("Writing block {:?}", block_idx);
            data.block_cache.write_back()?;
            written += to_copy;
            data.open_files[file_idx].current_cluster = current_cluster;

            let to_copy = to_copy as u32;
            let new_offset = data.open_files[file_idx].current_offset + to_copy;
            if new_offset > data.open_files[file_idx].entry.size {
                // We made it longer
                data.open_files[file_idx].update_length(new_offset);
            }
            data.open_files[file_idx]
                .seek_from_start(new_offset)
                .unwrap();
            // Entry update deferred to file close, for performance.
        }
        data.open_files[file_idx].entry.attributes.set_archive(true);
        data.open_files[file_idx].entry.mtime = self.time_source.get_timestamp();
        Ok(())
    }

    /// Close a file with the given raw file handle.
    pub fn close_file(&self, file: RawFile) -> Result<(), Error<D::Error>> {
        let flush_result = self.flush_file(file);
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        data.open_files.swap_remove(file_idx);
        flush_result
    }

    /// Flush (update the entry) for a file with the given raw file handle.
    pub fn flush_file(&self, file: RawFile) -> Result<(), Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        let file_id = data.get_file_by_id(file)?;

        if data.open_files[file_id].dirty {
            let volume_idx = data.get_volume_by_id(data.open_files[file_id].raw_volume)?;
            match &mut data.open_volumes[volume_idx].volume_type {
                VolumeType::Fat(fat) => {
                    debug!("Updating FAT info sector");
                    fat.update_info_sector(&mut data.block_cache)?;
                    debug!("Updating dir entry {:?}", data.open_files[file_id].entry);
                    if data.open_files[file_id].entry.size != 0 {
                        // If you have a length, you must have a cluster
                        assert!(data.open_files[file_id].entry.cluster.0 != 0);
                    }
                    fat.write_entry_to_disk(
                        &mut data.block_cache,
                        &data.open_files[file_id].entry,
                    )?;
                }
            };
        }
        Ok(())
    }

    /// Check if any files or folders are open.
    pub fn has_open_handles(&self) -> bool {
        let data = self.data.borrow();
        !(data.open_dirs.is_empty() || data.open_files.is_empty())
    }

    /// Consume self and return BlockDevice and TimeSource
    pub fn free(self) -> (D, T) {
        let data = self.data.into_inner();
        (data.block_cache.free(), self.time_source)
    }

    /// Check if a file is at End Of File.
    pub fn file_eof(&self, file: RawFile) -> Result<bool, Error<D::Error>> {
        let data = self.data.try_borrow().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        Ok(data.open_files[file_idx].eof())
    }

    /// Seek a file with an offset from the start of the file.
    pub fn file_seek_from_start(&self, file: RawFile, offset: u32) -> Result<(), Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        data.open_files[file_idx]
            .seek_from_start(offset)
            .map_err(|_| Error::InvalidOffset)?;
        Ok(())
    }

    /// Seek a file with an offset from the current position.
    pub fn file_seek_from_current(
        &self,
        file: RawFile,
        offset: i32,
    ) -> Result<(), Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        data.open_files[file_idx]
            .seek_from_current(offset)
            .map_err(|_| Error::InvalidOffset)?;
        Ok(())
    }

    /// Seek a file with an offset back from the end of the file.
    pub fn file_seek_from_end(&self, file: RawFile, offset: u32) -> Result<(), Error<D::Error>> {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        data.open_files[file_idx]
            .seek_from_end(offset)
            .map_err(|_| Error::InvalidOffset)?;
        Ok(())
    }

    /// Get the length of a file
    pub fn file_length(&self, file: RawFile) -> Result<u32, Error<D::Error>> {
        let data = self.data.try_borrow().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        Ok(data.open_files[file_idx].length())
    }

    /// Get the current offset of a file
    pub fn file_offset(&self, file: RawFile) -> Result<u32, Error<D::Error>> {
        let data = self.data.try_borrow().map_err(|_| Error::LockError)?;
        let file_idx = data.get_file_by_id(file)?;
        Ok(data.open_files[file_idx].current_offset)
    }

    /// Create a directory in a given directory.
    pub fn make_dir_in_dir<N>(
        &self,
        directory: RawDirectory,
        name: N,
    ) -> Result<(), Error<D::Error>>
    where
        N: ToShortFileName,
    {
        let mut data = self.data.try_borrow_mut().map_err(|_| Error::LockError)?;
        let data = data.deref_mut();

        // This check is load-bearing - we do an unchecked push later.
        if data.open_dirs.is_full() {
            return Err(Error::TooManyOpenDirs);
        }

        let parent_directory_idx = data.get_dir_by_id(directory)?;
        let parent_directory_info = &data.open_dirs[parent_directory_idx];
        let volume_id = data.open_dirs[parent_directory_idx].raw_volume;
        let volume_idx = data.get_volume_by_id(volume_id)?;
        let volume_info = &data.open_volumes[volume_idx];
        let sfn = name.to_short_filename().map_err(Error::FilenameError)?;

        debug!("Creating directory '{}'", sfn);
        debug!(
            "Parent dir is in cluster {:?}",
            parent_directory_info.cluster
        );

        // Does an entry exist with this name?
        let maybe_dir_entry = match &volume_info.volume_type {
            VolumeType::Fat(fat) => {
                fat.find_directory_entry(&mut data.block_cache, parent_directory_info, &sfn)
            }
        };

        match maybe_dir_entry {
            Ok(entry) if entry.attributes.is_directory() => {
                return Err(Error::DirAlreadyExists);
            }
            Ok(_entry) => {
                return Err(Error::FileAlreadyExists);
            }
            Err(Error::NotFound) => {
                // perfect, let's make it
            }
            Err(e) => {
                // Some other error - tell them about it
                return Err(e);
            }
        };

        let att = Attributes::create_from_fat(Attributes::DIRECTORY);

        // Need mutable access for this
        match &mut data.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => {
                debug!("Making dir entry");
                let mut new_dir_entry_in_parent = fat.write_new_directory_entry(
                    &mut data.block_cache,
                    &self.time_source,
                    parent_directory_info.cluster,
                    sfn,
                    att,
                )?;
                if new_dir_entry_in_parent.cluster == ClusterId::EMPTY {
                    new_dir_entry_in_parent.cluster =
                        fat.alloc_cluster(&mut data.block_cache, None, false)?;
                    // update the parent dir with the cluster of the new dir
                    fat.write_entry_to_disk(&mut data.block_cache, &new_dir_entry_in_parent)?;
                }
                let new_dir_start_block = fat.cluster_to_block(new_dir_entry_in_parent.cluster);
                debug!("Made new dir entry {:?}", new_dir_entry_in_parent);
                let now = self.time_source.get_timestamp();
                let fat_type = fat.get_fat_type();
                // A blank block
                let block = data.block_cache.blank_mut(new_dir_start_block);
                // make the "." entry
                let dot_entry_in_child = DirEntry {
                    name: crate::ShortFileName::this_dir(),
                    mtime: now,
                    ctime: now,
                    attributes: att,
                    // point at ourselves
                    cluster: new_dir_entry_in_parent.cluster,
                    size: 0,
                    entry_block: new_dir_start_block,
                    entry_offset: 0,
                };
                debug!("New dir has {:?}", dot_entry_in_child);
                let mut offset = 0;
                block[offset..offset + fat::OnDiskDirEntry::LEN]
                    .copy_from_slice(&dot_entry_in_child.serialize(fat_type)[..]);
                offset += fat::OnDiskDirEntry::LEN;
                // make the ".." entry
                let dot_dot_entry_in_child = DirEntry {
                    name: crate::ShortFileName::parent_dir(),
                    mtime: now,
                    ctime: now,
                    attributes: att,
                    // point at our parent
                    cluster: match fat_type {
                        fat::FatType::Fat16 => {
                            // On FAT16, indicate parent is root using Cluster(0)
                            if parent_directory_info.cluster == ClusterId::ROOT_DIR {
                                ClusterId::EMPTY
                            } else {
                                parent_directory_info.cluster
                            }
                        }
                        fat::FatType::Fat32 => parent_directory_info.cluster,
                    },
                    size: 0,
                    entry_block: new_dir_start_block,
                    entry_offset: fat::OnDiskDirEntry::LEN_U32,
                };
                debug!("New dir has {:?}", dot_dot_entry_in_child);
                block[offset..offset + fat::OnDiskDirEntry::LEN]
                    .copy_from_slice(&dot_dot_entry_in_child.serialize(fat_type)[..]);

                data.block_cache.write_back()?;

                for block_idx in new_dir_start_block
                    .range(BlockCount(u32::from(fat.blocks_per_cluster)))
                    .skip(1)
                {
                    let _block = data.block_cache.blank_mut(block_idx);
                    data.block_cache.write_back()?;
                }
            }
        };

        Ok(())
    }
}

/// The mutable data the VolumeManager needs to hold
///
/// Kept separate so its easier to wrap it in a RefCell
#[derive(Debug)]

struct VolumeManagerData<
    D,
    const MAX_DIRS: usize = 4,
    const MAX_FILES: usize = 4,
    const MAX_VOLUMES: usize = 1,
> where
    D: BlockDevice,
{
    id_generator: HandleGenerator,
    block_cache: BlockCache<D>,
    open_volumes: Vec<VolumeInfo, MAX_VOLUMES>,
    open_dirs: Vec<DirectoryInfo, MAX_DIRS>,
    open_files: Vec<FileInfo, MAX_FILES>,
}

impl<D, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>
    VolumeManagerData<D, MAX_DIRS, MAX_FILES, MAX_VOLUMES>
where
    D: BlockDevice,
{
    /// Check if a file is open
    ///
    /// Returns `true` if it's open, `false`, otherwise.
    fn file_is_open(&self, raw_volume: RawVolume, dir_entry: &DirEntry) -> bool {
        for f in self.open_files.iter() {
            if f.raw_volume == raw_volume
                && f.entry.entry_block == dir_entry.entry_block
                && f.entry.entry_offset == dir_entry.entry_offset
            {
                return true;
            }
        }
        false
    }

    fn get_volume_by_id<E>(&self, raw_volume: RawVolume) -> Result<usize, Error<E>>
    where
        E: core::fmt::Debug,
    {
        for (idx, v) in self.open_volumes.iter().enumerate() {
            if v.raw_volume == raw_volume {
                return Ok(idx);
            }
        }
        Err(Error::BadHandle)
    }

    fn get_dir_by_id<E>(&self, raw_directory: RawDirectory) -> Result<usize, Error<E>>
    where
        E: core::fmt::Debug,
    {
        for (idx, d) in self.open_dirs.iter().enumerate() {
            if d.raw_directory == raw_directory {
                return Ok(idx);
            }
        }
        Err(Error::BadHandle)
    }

    fn get_file_by_id<E>(&self, raw_file: RawFile) -> Result<usize, Error<E>>
    where
        E: core::fmt::Debug,
    {
        for (idx, f) in self.open_files.iter().enumerate() {
            if f.raw_file == raw_file {
                return Ok(idx);
            }
        }
        Err(Error::BadHandle)
    }

    /// This function turns `desired_offset` into an appropriate block to be
    /// read. It either calculates this based on the start of the file, or
    /// from the given start point - whichever is better.
    ///
    /// Returns:
    ///
    /// * the index for the block on the disk that contains the data we want,
    /// * the byte offset into that block for the data we want, and
    /// * how many bytes remain in that block.
    fn find_data_on_disk(
        &mut self,
        volume_idx: usize,
        start: &mut (u32, ClusterId),
        file_start: ClusterId,
        desired_offset: u32,
    ) -> Result<(BlockIdx, usize, usize), Error<D::Error>>
    where
        D: BlockDevice,
    {
        let bytes_per_cluster = match &self.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => fat.bytes_per_cluster(),
        };
        // do we need to be before our start point?
        if desired_offset < start.0 {
            // user wants to go backwards - start from the beginning of the file
            // because the FAT is only a singly-linked list.
            start.0 = 0;
            start.1 = file_start;
        }
        // How many clusters forward do we need to go?
        let offset_from_cluster = desired_offset - start.0;
        // walk through the FAT chain
        let num_clusters = offset_from_cluster / bytes_per_cluster;
        for _ in 0..num_clusters {
            start.1 = match &self.open_volumes[volume_idx].volume_type {
                VolumeType::Fat(fat) => fat.next_cluster(&mut self.block_cache, start.1)?,
            };
            start.0 += bytes_per_cluster;
        }
        // How many blocks in are we now?
        let offset_from_cluster = desired_offset - start.0;
        assert!(offset_from_cluster < bytes_per_cluster);
        let num_blocks = BlockCount(offset_from_cluster / Block::LEN_U32);
        let block_idx = match &self.open_volumes[volume_idx].volume_type {
            VolumeType::Fat(fat) => fat.cluster_to_block(start.1),
        } + num_blocks;
        let block_offset = (desired_offset % Block::LEN_U32) as usize;
        let available = Block::LEN - block_offset;
        Ok((block_idx, block_offset, available))
    }
}

/// Transform mode variants (ReadWriteCreate_Or_Append) to simple modes ReadWriteAppend or
/// ReadWriteCreate
fn solve_mode_variant(mode: Mode, dir_entry_is_some: bool) -> Mode {
    let mut mode = mode;
    if mode == Mode::ReadWriteCreateOrAppend {
        if dir_entry_is_some {
            mode = Mode::ReadWriteAppend;
        } else {
            mode = Mode::ReadWriteCreate;
        }
    } else if mode == Mode::ReadWriteCreateOrTruncate {
        if dir_entry_is_some {
            mode = Mode::ReadWriteTruncate;
        } else {
            mode = Mode::ReadWriteCreate;
        }
    }
    mode
}

// ****************************************************************************
//
// Unit Tests
//
// ****************************************************************************

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::Handle;
    use crate::Timestamp;

    struct DummyBlockDevice;

    struct Clock;

    #[derive(Debug)]
    enum Error {
        Unknown,
    }

    impl TimeSource for Clock {
        fn get_timestamp(&self) -> Timestamp {
            // TODO: Return actual time
            Timestamp {
                year_since_1970: 0,
                zero_indexed_month: 0,
                zero_indexed_day: 0,
                hours: 0,
                minutes: 0,
                seconds: 0,
            }
        }
    }

    impl BlockDevice for DummyBlockDevice {
        type Error = Error;

        /// Read one or more blocks, starting at the given block index.
        fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
            // Actual blocks taken from an SD card, except I've changed the start and length of partition 0.
            static BLOCKS: [Block; 3] = [
                Block {
                    contents: [
                        0xfa, 0xb8, 0x00, 0x10, 0x8e, 0xd0, 0xbc, 0x00, 0xb0, 0xb8, 0x00, 0x00,
                        0x8e, 0xd8, 0x8e, 0xc0, // 0x000
                        0xfb, 0xbe, 0x00, 0x7c, 0xbf, 0x00, 0x06, 0xb9, 0x00, 0x02, 0xf3, 0xa4,
                        0xea, 0x21, 0x06, 0x00, // 0x010
                        0x00, 0xbe, 0xbe, 0x07, 0x38, 0x04, 0x75, 0x0b, 0x83, 0xc6, 0x10, 0x81,
                        0xfe, 0xfe, 0x07, 0x75, // 0x020
                        0xf3, 0xeb, 0x16, 0xb4, 0x02, 0xb0, 0x01, 0xbb, 0x00, 0x7c, 0xb2, 0x80,
                        0x8a, 0x74, 0x01, 0x8b, // 0x030
                        0x4c, 0x02, 0xcd, 0x13, 0xea, 0x00, 0x7c, 0x00, 0x00, 0xeb, 0xfe, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x040
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x050
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x060
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x070
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x080
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x090
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0B0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0F0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x100
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x110
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x120
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x130
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x140
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x150
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x160
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x170
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x180
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x190
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x4c, 0xca, 0xde, 0x06,
                        0x00, 0x00, 0x00, 0x04, // 0x1B0
                        0x01, 0x04, 0x0c, 0xfe, 0xc2, 0xff, 0x01, 0x00, 0x00, 0x00, 0x33, 0x22,
                        0x11, 0x00, 0x00, 0x00, // 0x1C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x55, 0xaa, // 0x1F0
                    ],
                },
                Block {
                    contents: [
                        0xeb, 0x58, 0x90, 0x6d, 0x6b, 0x66, 0x73, 0x2e, 0x66, 0x61, 0x74, 0x00,
                        0x02, 0x08, 0x20, 0x00, // 0x000
                        0x02, 0x00, 0x00, 0x00, 0x00, 0xf8, 0x00, 0x00, 0x10, 0x00, 0x04, 0x00,
                        0x00, 0x08, 0x00, 0x00, // 0x010
                        0x00, 0x20, 0x76, 0x00, 0x80, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x02, 0x00, 0x00, 0x00, // 0x020
                        0x01, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x030
                        0x80, 0x01, 0x29, 0x0b, 0xa8, 0x89, 0x27, 0x50, 0x69, 0x63, 0x74, 0x75,
                        0x72, 0x65, 0x73, 0x20, // 0x040
                        0x20, 0x20, 0x46, 0x41, 0x54, 0x33, 0x32, 0x20, 0x20, 0x20, 0x0e, 0x1f,
                        0xbe, 0x77, 0x7c, 0xac, // 0x050
                        0x22, 0xc0, 0x74, 0x0b, 0x56, 0xb4, 0x0e, 0xbb, 0x07, 0x00, 0xcd, 0x10,
                        0x5e, 0xeb, 0xf0, 0x32, // 0x060
                        0xe4, 0xcd, 0x16, 0xcd, 0x19, 0xeb, 0xfe, 0x54, 0x68, 0x69, 0x73, 0x20,
                        0x69, 0x73, 0x20, 0x6e, // 0x070
                        0x6f, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f, 0x74, 0x61, 0x62, 0x6c,
                        0x65, 0x20, 0x64, 0x69, // 0x080
                        0x73, 0x6b, 0x2e, 0x20, 0x20, 0x50, 0x6c, 0x65, 0x61, 0x73, 0x65, 0x20,
                        0x69, 0x6e, 0x73, 0x65, // 0x090
                        0x72, 0x74, 0x20, 0x61, 0x20, 0x62, 0x6f, 0x6f, 0x74, 0x61, 0x62, 0x6c,
                        0x65, 0x20, 0x66, 0x6c, // 0x0A0
                        0x6f, 0x70, 0x70, 0x79, 0x20, 0x61, 0x6e, 0x64, 0x0d, 0x0a, 0x70, 0x72,
                        0x65, 0x73, 0x73, 0x20, // 0x0B0
                        0x61, 0x6e, 0x79, 0x20, 0x6b, 0x65, 0x79, 0x20, 0x74, 0x6f, 0x20, 0x74,
                        0x72, 0x79, 0x20, 0x61, // 0x0C0
                        0x67, 0x61, 0x69, 0x6e, 0x20, 0x2e, 0x2e, 0x2e, 0x20, 0x0d, 0x0a, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x0F0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x100
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x110
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x120
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x130
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x140
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x150
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x160
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x170
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x180
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x190
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1A0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1B0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1C0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1D0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x00, 0x00, // 0x1E0
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                        0x00, 0x00, 0x55, 0xaa, // 0x1F0
                    ],
                },
                Block {
                    contents: hex!(
                        "52 52 61 41 00 00 00 00 00 00 00 00 00 00 00 00
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
                         00 00 00 00 72 72 41 61 FF FF FF FF FF FF FF FF
                         00 00 00 00 00 00 00 00 00 00 00 00 00 00 55 AA"
                    ),
                },
            ];
            println!(
                "Reading block {} to {}",
                start_block_idx.0,
                start_block_idx.0 as usize + blocks.len()
            );
            for (idx, block) in blocks.iter_mut().enumerate() {
                let block_idx = start_block_idx.0 as usize + idx;
                if block_idx < BLOCKS.len() {
                    *block = BLOCKS[block_idx].clone();
                } else {
                    return Err(Error::Unknown);
                }
            }
            Ok(())
        }

        /// Write one or more blocks, starting at the given block index.
        fn write(&self, _blocks: &[Block], _start_block_idx: BlockIdx) -> Result<(), Self::Error> {
            unimplemented!();
        }

        /// Determine how many blocks this device can hold.
        fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
            Ok(BlockCount(2))
        }
    }

    #[test]
    fn partition0() {
        let c: VolumeManager<DummyBlockDevice, Clock, 2, 2> =
            VolumeManager::new_with_limits(DummyBlockDevice, Clock, 0xAA00_0000);

        let v = c.open_raw_volume(VolumeIdx(0)).unwrap();
        let expected_id = RawVolume(Handle(0xAA00_0000));
        assert_eq!(v, expected_id);
        assert_eq!(
            &c.data.borrow().open_volumes[0],
            &VolumeInfo {
                raw_volume: expected_id,
                idx: VolumeIdx(0),
                volume_type: VolumeType::Fat(crate::FatVolume {
                    lba_start: BlockIdx(1),
                    num_blocks: BlockCount(0x0011_2233),
                    blocks_per_cluster: 8,
                    first_data_block: BlockCount(15136),
                    fat_start: BlockCount(32),
                    second_fat_start: Some(BlockCount(32 + 0x0000_1D80)),
                    name: fat::VolumeName::create_from_str("Pictures").unwrap(),
                    free_clusters_count: None,
                    next_free_cluster: None,
                    cluster_count: 965_788,
                    fat_specific_info: fat::FatSpecificInfo::Fat32(fat::Fat32Info {
                        first_root_dir_cluster: ClusterId(2),
                        info_location: BlockIdx(1) + BlockCount(1),
                    })
                })
            }
        );
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************

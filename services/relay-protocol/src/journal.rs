//! The ordering owner's encrypted full-match archive. Sequence offsets stay on
//! disk; the in-memory deque is only a bounded cache of this same stream.

use super::sequenced::{DEPARTURE_EVENT, FRAME_END_EVENT};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const GLOBAL_QUOTA: u64 = 8 * 1024 * 1024 * 1024;
// Commands one source may archive. Its checksum sample every 15 frames alone
// takes roughly a day at 60 FPS; past this its batches get Backpressure.
const SOURCE_COMMAND_QUOTA: u64 = 32 * 1024 * 1024;
const FRAME_END_BYTES: u64 = 4 + 13 + 8;
const DEPARTURE_BYTES: u64 = 4 + 15 + 8;
const MAX_RECORD_BYTES: usize = 22 + 40 + super::ORDERING_BATCH_CAPACITY;

pub(super) struct JournalDirectory {
    root: PathBuf,
    // Bytes per room, logical and never a disk preallocation: a live room
    // holds its whole quota, a finished one only what it wrote. Abandoned
    // journals found after restart hold their size and are never overwritten.
    reserved: BTreeMap<[u8; 16], u64>,
    total: u64,
    // Ended or partly created archives whose exact files are still on disk.
    // Each keeps its reservation until a retried removal succeeds.
    retiring: BTreeSet<[u8; 16]>,
    retry_at: u64,
}

impl JournalDirectory {
    pub(super) fn open(root: &Path) -> io::Result<Self> {
        if !root.is_absolute() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(root)?;
        let metadata = fs::symlink_metadata(root)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
        }
        let mut reserved = BTreeMap::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.len() != 32 || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                continue;
            }
            let mut room = [0; 16];
            for (index, byte) in room.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&name[index * 2..index * 2 + 2], 16)
                    .map_err(|_| io::ErrorKind::InvalidData)?;
            }
            let mut bytes = entry.metadata()?.len();
            if entry.file_type()?.is_dir() {
                for file in fs::read_dir(entry.path())? {
                    bytes += file?.metadata()?.len();
                }
            }
            reserved.insert(room, bytes);
        }
        let total = reserved.values().sum();
        Ok(Self {
            root: root.to_owned(),
            reserved,
            total,
            retiring: BTreeSet::new(),
            retry_at: 0,
        })
    }

    fn path(&self, room: &[u8; 16]) -> PathBuf {
        let name: String = room.iter().map(|byte| format!("{byte:02x}")).collect();
        self.root.join(name)
    }

    /// Admits a room only while its whole quota fits beside every other
    /// reservation: its clock records for the full match lifetime and each
    /// source's command allowance.
    pub(super) fn create(
        &mut self,
        room: [u8; 16],
        tick_hz: u16,
        sources: u32,
    ) -> io::Result<Journal> {
        if self.reserved.contains_key(&room) {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let reserved_tail = (24 * 60 * 60 * u64::from(tick_hz) + 2) * FRAME_END_BYTES
            + u64::from(super::MAX_PEERS) * DEPARTURE_BYTES;
        let quota = reserved_tail + u64::from(sources) * SOURCE_COMMAND_QUOTA;
        if self.total + quota > GLOBAL_QUOTA {
            return Err(io::ErrorKind::StorageFull.into());
        }
        let path = self.path(&room);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&path)?;
        self.reserved.insert(room, quota);
        self.total += quota;
        let files = private_new(&path.join("stream.data"))
            .and_then(|data| Ok((data, private_new(&path.join("stream.index"))?)));
        let (data, index) = match files {
            Ok(files) => files,
            Err(error) => {
                // Nothing was written yet: retire it like an ended archive.
                self.retire(room);
                return Err(error);
            }
        };
        Ok(Journal {
            room,
            data,
            index,
            entries: 0,
            used: 0,
            quota,
            reserved_tail,
            pending: 0,
            failed: false,
        })
    }

    pub(super) fn cleanup(&mut self, journal: Journal) -> io::Result<()> {
        // Only a clean, legitimately ended match may retire its exact files.
        // Never remove an I/O-failed archive or recursively delete a directory.
        if journal.failed {
            return Err(io::ErrorKind::Other.into());
        }
        // Windows can defer deletion until the last file handle closes. Close
        // both files before removing them and their containing directory.
        let room = journal.room;
        drop(journal);
        self.retire(room);
        Ok(())
    }

    /// A finished room takes no more commands; it keeps the bytes its
    /// terminal replay grace serves plus the frame ends of `unmerged` frames
    /// a federated relay sealed before every departure reached it.
    pub(super) fn settle(&mut self, journal: &mut Journal, unmerged: u32) {
        let tail = journal
            .reserved_tail
            .min(u64::from(unmerged) * FRAME_END_BYTES);
        let keep = journal.used + tail;
        if journal.quota == keep {
            return;
        }
        if let Some(bytes) = self.reserved.get_mut(&journal.room) {
            self.total -= *bytes - keep;
            *bytes = keep;
        }
        journal.quota = keep;
        journal.reserved_tail = tail;
        journal.pending = 0;
    }

    fn retire(&mut self, room: [u8; 16]) {
        self.retiring.insert(room);
        self.remove_retiring();
    }

    /// Retries removals that failed, such as a transient disk error, at most
    /// once a second.
    pub(super) fn retry(&mut self, now_ms: u64) {
        if self.retiring.is_empty() || now_ms < self.retry_at {
            return;
        }
        self.retry_at = now_ms.saturating_add(1000);
        self.remove_retiring();
    }

    fn remove_retiring(&mut self) {
        for room in std::mem::take(&mut self.retiring) {
            let path = self.path(&room);
            let removed = present(fs::remove_file(path.join("stream.data")))
                .and_then(|()| present(fs::remove_file(path.join("stream.index"))))
                .and_then(|()| present(fs::remove_dir(&path)));
            if removed.is_ok() {
                if let Some(bytes) = self.reserved.remove(&room) {
                    self.total -= bytes;
                }
            } else {
                self.retiring.insert(room);
            }
        }
    }
}

/// A file or directory an earlier attempt already removed counts as removed.
fn present(removal: io::Result<()>) -> io::Result<()> {
    match removal {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
thread_local! {
    // Fails this thread's next index creation, as a full disk would.
    pub(super) static FAIL_NEXT_INDEX: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn private_new(path: &Path) -> io::Result<File> {
    #[cfg(test)]
    if path.ends_with("stream.index") && FAIL_NEXT_INDEX.replace(false) {
        return Err(io::ErrorKind::StorageFull.into());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(super) struct Journal {
    room: [u8; 16],
    data: File,
    index: File,
    entries: u64,
    used: u64,
    quota: u64,
    reserved_tail: u64,
    pending: u64,
    failed: bool,
}

impl Journal {
    pub(super) fn reserve_batch(&mut self, bytes: usize) -> bool {
        self.reserve_entries(&[bytes])
    }

    /// Reserve a whole control transaction before accepting its first marker;
    /// a pause cannot consume the space needed for its eventual resume.
    pub(super) fn reserve_entries(&mut self, lengths: &[usize]) -> bool {
        let Some(bytes) = lengths.iter().try_fold(0u64, |total, &length| {
            total.checked_add(u64::try_from(length).ok()?.checked_add(12)?)
        }) else {
            return false;
        };
        if self.failed || self.used + self.reserved_tail + self.pending + bytes > self.quota {
            return false;
        }
        self.pending += bytes;
        true
    }

    /// One checked append transaction for a frame or canonical control barrier. A partial write is
    /// left intact for inspection, never truncated or treated as eviction.
    pub(super) fn commit(&mut self, records: &[Vec<u8>]) -> io::Result<()> {
        if self.failed {
            return Err(io::ErrorKind::Other.into());
        }
        let result = self.commit_inner(records);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn commit_inner(&mut self, records: &[Vec<u8>]) -> io::Result<()> {
        let mut data = Vec::new();
        let mut offsets = Vec::with_capacity(records.len() * 8);
        let mut offset = self.data.seek(SeekFrom::End(0))?;
        let mut mandatory = 0;
        let mut optional = 0;
        for record in records {
            if record.len() > MAX_RECORD_BYTES {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let bytes = record.len() as u64 + 12;
            if matches!(record.first(), Some(&FRAME_END_EVENT | &DEPARTURE_EVENT)) {
                mandatory += bytes;
            } else {
                optional += bytes;
            }
            offsets.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&(record.len() as u32).to_le_bytes());
            data.extend_from_slice(record);
            offset += 4 + record.len() as u64;
        }
        if optional > self.pending || mandatory > self.reserved_tail {
            return Err(io::ErrorKind::StorageFull.into());
        }
        self.data.write_all(&data)?;
        self.index.seek(SeekFrom::End(0))?;
        self.index.write_all(&offsets)?;
        // These are unbuffered File writes. Restart migration is deliberately
        // unsupported; existing files are refused rather than resumed unsafely.
        self.entries += records.len() as u64;
        self.used += data.len() as u64 + offsets.len() as u64;
        self.pending -= optional;
        self.reserved_tail -= mandatory;
        Ok(())
    }

    pub(super) fn read(&mut self, sequence: u64, count: usize) -> io::Result<Vec<Vec<u8>>> {
        if self.failed || sequence == 0 || sequence > self.entries + 1 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let result = self.read_inner(sequence, count);
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn read_inner(&mut self, sequence: u64, count: usize) -> io::Result<Vec<Vec<u8>>> {
        let count = (self.entries + 1 - sequence).min(count as u64) as usize;
        let mut offsets = vec![0; count * 8];
        self.index.seek(SeekFrom::Start((sequence - 1) * 8))?;
        self.index.read_exact(&mut offsets)?;
        let mut output = Vec::with_capacity(count);
        for (index, offset) in offsets.chunks_exact(8).enumerate() {
            self.data.seek(SeekFrom::Start(u64::from_le_bytes(
                offset.try_into().unwrap(),
            )))?;
            let mut length = [0; 4];
            self.data.read_exact(&mut length)?;
            let length = u32::from_le_bytes(length) as usize;
            if !(13..=MAX_RECORD_BYTES).contains(&length) {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let mut bytes = vec![0; length];
            self.data.read_exact(&mut bytes)?;
            if u64::from_le_bytes(bytes[1..9].try_into().unwrap()) != sequence + index as u64 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            output.push(bytes);
        }
        Ok(output)
    }
}

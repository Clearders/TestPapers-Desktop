//! Minimal ZIP writer/reader for deterministic, bounded, uncompressed workspace artifacts.
//!
//! Storing rather than deflating is intentional: it keeps this security-sensitive archive parser
//! small, prevents decompression bombs, and is sufficient for DOCX and `.tpbackup` containers.

use std::collections::BTreeSet;
use std::fmt;

const LOCAL_FILE_HEADER: u32 = 0x0403_4b50;
const CENTRAL_DIRECTORY_HEADER: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY: u32 = 0x0605_4b50;
const UTF8_FLAG: u16 = 1 << 11;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ZipEntry {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

impl ZipEntry {
    pub(crate) fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ZipLimits {
    pub(crate) max_entries: usize,
    pub(crate) max_entry_bytes: u64,
    pub(crate) max_total_bytes: u64,
}

impl Default for ZipLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_entry_bytes: 2 * 1024 * 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ZipError {
    InvalidPath(String),
    DuplicatePath(String),
    TooManyEntries,
    EntryTooLarge(String),
    ArchiveTooLarge,
    UnsupportedArchive(&'static str),
    Truncated,
    Corrupt(String),
}

impl fmt::Display for ZipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(path) => write!(formatter, "unsafe archive path: {path}"),
            Self::DuplicatePath(path) => write!(formatter, "duplicate archive path: {path}"),
            Self::TooManyEntries => formatter.write_str("archive has too many entries"),
            Self::EntryTooLarge(path) => write!(formatter, "archive entry is too large: {path}"),
            Self::ArchiveTooLarge => formatter.write_str("archive is too large"),
            Self::UnsupportedArchive(reason) => {
                write!(formatter, "unsupported ZIP archive: {reason}")
            }
            Self::Truncated => formatter.write_str("truncated ZIP archive"),
            Self::Corrupt(reason) => write!(formatter, "corrupt ZIP archive: {reason}"),
        }
    }
}

pub(crate) fn write_stored_zip(entries: &[ZipEntry]) -> Result<Vec<u8>, ZipError> {
    if entries.len() > u16::MAX as usize {
        return Err(ZipError::TooManyEntries);
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    let mut central = Vec::new();

    for entry in entries {
        validate_archive_path(&entry.path)?;
        if !seen.insert(entry.path.clone()) {
            return Err(ZipError::DuplicatePath(entry.path.clone()));
        }
        let name = entry.path.as_bytes();
        let name_len =
            u16::try_from(name.len()).map_err(|_| ZipError::InvalidPath(entry.path.clone()))?;
        let size = u32::try_from(entry.bytes.len())
            .map_err(|_| ZipError::EntryTooLarge(entry.path.clone()))?;
        let offset = u32::try_from(output.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
        let checksum = crc32(&entry.bytes);

        push_u32(&mut output, LOCAL_FILE_HEADER);
        push_u16(&mut output, 20);
        push_u16(&mut output, UTF8_FLAG);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u16(&mut output, 0);
        push_u32(&mut output, checksum);
        push_u32(&mut output, size);
        push_u32(&mut output, size);
        push_u16(&mut output, name_len);
        push_u16(&mut output, 0);
        output.extend_from_slice(name);
        output.extend_from_slice(&entry.bytes);

        push_u32(&mut central, CENTRAL_DIRECTORY_HEADER);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, UTF8_FLAG);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, checksum);
        push_u32(&mut central, size);
        push_u32(&mut central, size);
        push_u16(&mut central, name_len);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name);
    }

    let central_offset = u32::try_from(output.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
    let central_size = u32::try_from(central.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
    output.extend_from_slice(&central);
    push_u32(&mut output, END_OF_CENTRAL_DIRECTORY);
    push_u16(&mut output, 0);
    push_u16(&mut output, 0);
    push_u16(&mut output, entries.len() as u16);
    push_u16(&mut output, entries.len() as u16);
    push_u32(&mut output, central_size);
    push_u32(&mut output, central_offset);
    push_u16(&mut output, 0);
    Ok(output)
}

pub(crate) fn read_stored_zip(bytes: &[u8], limits: ZipLimits) -> Result<Vec<ZipEntry>, ZipError> {
    let mut cursor = 0_usize;
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    let mut total_size = 0_u64;
    let mut found_central_directory = false;

    while cursor < bytes.len() {
        let signature = read_u32(bytes, cursor)?;
        if signature == CENTRAL_DIRECTORY_HEADER || signature == END_OF_CENTRAL_DIRECTORY {
            found_central_directory = true;
            break;
        }
        if signature != LOCAL_FILE_HEADER {
            return Err(ZipError::Corrupt(format!(
                "unexpected signature at byte {cursor}"
            )));
        }
        if entries.len() >= limits.max_entries {
            return Err(ZipError::TooManyEntries);
        }
        ensure_range(bytes, cursor, 30)?;
        let flags = read_u16(bytes, cursor + 6)?;
        let method = read_u16(bytes, cursor + 8)?;
        if flags & 1 != 0 {
            return Err(ZipError::UnsupportedArchive("encrypted ZIP entries"));
        }
        if flags & (1 << 3) != 0 {
            return Err(ZipError::UnsupportedArchive("data descriptors"));
        }
        if method != 0 {
            return Err(ZipError::UnsupportedArchive("compressed entries"));
        }
        let expected_crc = read_u32(bytes, cursor + 14)?;
        let compressed_size = read_u32(bytes, cursor + 18)? as usize;
        let uncompressed_size = read_u32(bytes, cursor + 22)? as usize;
        if compressed_size != uncompressed_size {
            return Err(ZipError::Corrupt("stored entry size mismatch".into()));
        }
        let name_len = read_u16(bytes, cursor + 26)? as usize;
        let extra_len = read_u16(bytes, cursor + 28)? as usize;
        let name_start = cursor.checked_add(30).ok_or(ZipError::ArchiveTooLarge)?;
        ensure_range(bytes, name_start, name_len + extra_len)?;
        let path = std::str::from_utf8(&bytes[name_start..name_start + name_len])
            .map_err(|_| ZipError::InvalidPath("non-UTF-8".into()))?
            .to_owned();
        validate_archive_path(&path)?;
        if !seen.insert(path.clone()) {
            return Err(ZipError::DuplicatePath(path));
        }

        let size = uncompressed_size as u64;
        if size > limits.max_entry_bytes {
            return Err(ZipError::EntryTooLarge(path));
        }
        total_size = total_size
            .checked_add(size)
            .ok_or(ZipError::ArchiveTooLarge)?;
        if total_size > limits.max_total_bytes {
            return Err(ZipError::ArchiveTooLarge);
        }
        let data_start = name_start
            .checked_add(name_len + extra_len)
            .ok_or(ZipError::ArchiveTooLarge)?;
        ensure_range(bytes, data_start, uncompressed_size)?;
        let data = &bytes[data_start..data_start + uncompressed_size];
        if crc32(data) != expected_crc {
            return Err(ZipError::Corrupt(format!("CRC mismatch for {path}")));
        }
        entries.push(ZipEntry::new(path, data.to_vec()));
        cursor = data_start
            .checked_add(uncompressed_size)
            .ok_or(ZipError::ArchiveTooLarge)?;
    }

    if !found_central_directory {
        return Err(ZipError::Truncated);
    }
    Ok(entries)
}

pub(crate) fn validate_archive_path(path: &str) -> Result<(), ZipError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.as_bytes().get(1) == Some(&b':')
    {
        return Err(ZipError::InvalidPath(path.into()));
    }
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(ZipError::InvalidPath(path.into()));
        }
    }
    Ok(())
}

fn ensure_range(bytes: &[u8], start: usize, len: usize) -> Result<(), ZipError> {
    let end = start.checked_add(len).ok_or(ZipError::ArchiveTooLarge)?;
    if end > bytes.len() {
        return Err(ZipError::Truncated);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ZipError> {
    ensure_range(bytes, offset, 2)?;
    Ok(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().expect("checked range"),
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ZipError> {
    ensure_range(bytes, offset, 4)?;
    Ok(u32::from_le_bytes(
        bytes[offset..offset + 4].try_into().expect("checked range"),
    ))
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_stored_entries() {
        let entries = vec![
            ZipEntry::new("manifest.json", br#"{"version":1}"#.to_vec()),
            ZipEntry::new("blobs/aa/hash", b"binary".to_vec()),
        ];
        let archive = write_stored_zip(&entries).unwrap();
        assert_eq!(
            read_stored_zip(&archive, ZipLimits::default()).unwrap(),
            entries
        );
    }

    #[test]
    fn rejects_traversal_duplicates_and_crc_tampering() {
        assert!(matches!(
            write_stored_zip(&[ZipEntry::new("../secret", Vec::new())]),
            Err(ZipError::InvalidPath(_))
        ));
        assert!(matches!(
            write_stored_zip(&[
                ZipEntry::new("same", Vec::new()),
                ZipEntry::new("same", Vec::new())
            ]),
            Err(ZipError::DuplicatePath(_))
        ));

        let mut archive = write_stored_zip(&[ZipEntry::new("item", b"abc".to_vec())]).unwrap();
        let data_offset = 30 + "item".len();
        archive[data_offset] ^= 1;
        assert!(matches!(
            read_stored_zip(&archive, ZipLimits::default()),
            Err(ZipError::Corrupt(_))
        ));
    }

    #[test]
    fn enforces_uncompressed_size_limits() {
        let archive = write_stored_zip(&[ZipEntry::new("large", vec![0; 32])]).unwrap();
        let limits = ZipLimits {
            max_entries: 1,
            max_entry_bytes: 8,
            max_total_bytes: 8,
        };
        assert!(matches!(
            read_stored_zip(&archive, limits),
            Err(ZipError::EntryTooLarge(_))
        ));
    }
}

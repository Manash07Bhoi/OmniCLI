use std::{fs, io::Read, path::Path};

use anyhow::Result;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use serde::Serialize;
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

use crate::error::ArchiveError;

/// An entry in an archive listing.
#[derive(Debug, Serialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub size_bytes: u64,
    pub compressed_size: Option<u64>,
    pub is_dir: bool,
}

/// List the contents of an archive without extracting it.
pub fn list_archive(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    if !archive.exists() {
        return Err(ArchiveError::NotFound(archive.display().to_string()));
    }

    let mut f = fs::File::open(archive)?;
    let mut magic = [0u8; 8];
    let n = f.read(&mut magic)?;
    drop(f);

    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if n >= 4 && magic[0] == 0x50 && magic[1] == 0x4B {
        list_zip(archive)
    } else if n >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
        list_tar_gz(archive)
    } else if n >= 6 && magic[0] == 0xfd && magic[1] == b'7' && magic[2] == b'z' {
        list_tar_xz(archive)
    } else if n >= 3 && magic[0] == 0x42 && magic[1] == 0x5A && magic[2] == 0x68 {
        list_tar_bz2(archive)
    } else if name.ends_with(".tar") {
        list_tar_plain(archive)
    } else {
        Err(ArchiveError::UnsupportedFormat(format!(
            "Cannot detect format of {}",
            archive.display()
        )))
    }
}

fn list_zip(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        entries.push(ArchiveEntry {
            name: entry.name().to_owned(),
            size_bytes: entry.size(),
            compressed_size: Some(entry.compressed_size()),
            is_dir: entry.is_dir(),
        });
    }
    Ok(entries)
}

fn list_tar_gz(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let gz = GzDecoder::new(file);
    let mut tar = TarArchive::new(gz);
    let mut entries = Vec::new();
    for entry in tar.entries()? {
        let entry = entry?;
        let hdr = entry.header();
        entries.push(ArchiveEntry {
            name: entry.path()?.display().to_string(),
            size_bytes: hdr.size()?,
            compressed_size: None,
            is_dir: hdr.entry_type().is_dir(),
        });
    }
    Ok(entries)
}

fn list_tar_xz(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let xz = XzDecoder::new(file);
    let mut tar = TarArchive::new(xz);
    let mut entries = Vec::new();
    for entry in tar.entries()? {
        let entry = entry?;
        let hdr = entry.header();
        entries.push(ArchiveEntry {
            name: entry.path()?.display().to_string(),
            size_bytes: hdr.size()?,
            compressed_size: None,
            is_dir: hdr.entry_type().is_dir(),
        });
    }
    Ok(entries)
}

fn list_tar_bz2(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let bz = BzDecoder::new(file);
    let mut tar = TarArchive::new(bz);
    let mut entries = Vec::new();
    for entry in tar.entries()? {
        let entry = entry?;
        let hdr = entry.header();
        entries.push(ArchiveEntry {
            name: entry.path()?.display().to_string(),
            size_bytes: hdr.size()?,
            compressed_size: None,
            is_dir: hdr.entry_type().is_dir(),
        });
    }
    Ok(entries)
}

fn list_tar_plain(archive: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut tar = TarArchive::new(file);
    let mut entries = Vec::new();
    for entry in tar.entries()? {
        let entry = entry?;
        let hdr = entry.header();
        entries.push(ArchiveEntry {
            name: entry.path()?.display().to_string(),
            size_bytes: hdr.size()?,
            compressed_size: None,
            is_dir: hdr.entry_type().is_dir(),
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::create_archive;
    use tempfile::tempdir;

    #[test]
    fn test_list_zip() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("readme.txt");
        fs::write(&f, b"readme content").unwrap();

        let archive = dir.path().join("test.zip");
        create_archive(&archive, &[f]).unwrap();

        let entries = list_archive(&archive).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "readme.txt");
        assert_eq!(entries[0].size_bytes, 14);
    }

    #[test]
    fn test_list_tar_gz() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("file.txt");
        fs::write(&f, b"file data").unwrap();

        let archive = dir.path().join("test.tar.gz");
        create_archive(&archive, &[f]).unwrap();

        let entries = list_archive(&archive).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].is_dir);
    }

    #[test]
    fn test_list_tar_bz2() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("bz2file.txt");
        fs::write(&f, b"bzip2 file data").unwrap();

        let archive = dir.path().join("test.tar.bz2");
        create_archive(&archive, &[f]).unwrap();

        let entries = list_archive(&archive).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "bz2file.txt");
    }
}

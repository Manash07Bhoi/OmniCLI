use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use anyhow::Result;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use serde::Serialize;
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

use crate::error::ArchiveError;

/// Result of `omni archive extract`.
#[derive(Debug, Serialize)]
pub struct ExtractResult {
    pub archive: String,
    pub dest: String,
    pub files_extracted: u64,
    pub bytes_extracted: u64,
}

/// Detect the archive format by reading magic bytes from the file (not from the extension).
fn detect_format_by_magic(path: &Path) -> Result<&'static str, ArchiveError> {
    let mut f = fs::File::open(path)?;
    let mut magic = [0u8; 8];
    let n = f.read(&mut magic)?;

    // ZIP: PK\x03\x04
    if n >= 4 && magic[0] == 0x50 && magic[1] == 0x4B && magic[2] == 0x03 && magic[3] == 0x04 {
        return Ok("zip");
    }
    // GZIP: \x1f\x8b
    if n >= 2 && magic[0] == 0x1f && magic[1] == 0x8b {
        return Ok("tar.gz");
    }
    // XZ: \xfd7zXZ\x00
    if n >= 6
        && magic[0] == 0xfd
        && magic[1] == b'7'
        && magic[2] == b'z'
        && magic[3] == b'X'
        && magic[4] == b'Z'
    {
        return Ok("tar.xz");
    }
    // BZIP2: BZh (0x42 0x5A 0x68)
    if n >= 3 && magic[0] == 0x42 && magic[1] == 0x5A && magic[2] == 0x68 {
        return Ok("tar.bz2");
    }
    // TAR (uncompressed): detect by extension fallback
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name.ends_with(".tar") {
        return Ok("tar");
    }

    Err(ArchiveError::UnsupportedFormat(format!(
        "Cannot detect format of {}. Supported: zip, tar, tar.gz, tar.xz, tar.bz2",
        path.display()
    )))
}

/// Extract an archive to `dest` (defaults to a directory next to the archive).
pub fn extract_archive(archive: &Path, dest: Option<&Path>) -> Result<ExtractResult, ArchiveError> {
    if !archive.exists() {
        return Err(ArchiveError::NotFound(archive.display().to_string()));
    }

    let dest_path = match dest {
        Some(d) => d.to_owned(),
        None => {
            // Strip the archive extension(s) to get the default extraction directory.
            let name = archive
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("extracted");
            let stripped = strip_archive_ext(name);
            archive.parent().unwrap_or(Path::new(".")).join(stripped)
        }
    };

    fs::create_dir_all(&dest_path)?;

    let fmt = detect_format_by_magic(archive)?;
    let (files_extracted, bytes_extracted) = match fmt {
        "zip" => extract_zip(archive, &dest_path)?,
        "tar.gz" => extract_tar_gz(archive, &dest_path)?,
        "tar.xz" => extract_tar_xz(archive, &dest_path)?,
        "tar.bz2" => extract_tar_bz2(archive, &dest_path)?,
        "tar" => extract_tar_plain(archive, &dest_path)?,
        other => {
            return Err(ArchiveError::UnsupportedFormat(other.to_owned()));
        }
    };

    Ok(ExtractResult {
        archive: archive.display().to_string(),
        dest: dest_path.display().to_string(),
        files_extracted,
        bytes_extracted,
    })
}

/// Guard against zip-slip: ensure the resolved entry path stays inside `dest`.
/// Rejects absolute paths and any path components containing "..".
fn safe_join(dest: &Path, entry_name: &str) -> Result<std::path::PathBuf, ArchiveError> {
    // Normalise path separators (Windows archives use backslash).
    let name = entry_name.replace('\\', "/");

    // Reject absolute paths and path-traversal attempts.
    for component in name.split('/') {
        if component == ".." {
            return Err(ArchiveError::Corrupt(format!(
                "Zip-slip: path traversal detected in archive entry: {entry_name}"
            )));
        }
    }
    if name.starts_with('/') {
        return Err(ArchiveError::Corrupt(format!(
            "Zip-slip: absolute path in archive entry: {entry_name}"
        )));
    }

    Ok(dest.join(&name))
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;
    let mut files_extracted = 0u64;
    let mut bytes_extracted = 0u64;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let out_path = safe_join(dest, entry.name())?;

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out_path)?;
            bytes_extracted += io::copy(&mut entry, &mut out_file)?;
            files_extracted += 1;
        }
    }
    Ok((files_extracted, bytes_extracted))
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::open(archive)?;
    let gz = GzDecoder::new(file);
    let mut tar = TarArchive::new(gz);
    let mut files_extracted = 0u64;
    let mut bytes_extracted = 0u64;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size()?;
        if entry.header().entry_type().is_file() {
            entry.unpack_in(dest)?;
            files_extracted += 1;
            bytes_extracted += size;
        } else {
            entry.unpack_in(dest)?;
        }
    }
    Ok((files_extracted, bytes_extracted))
}

fn extract_tar_xz(archive: &Path, dest: &Path) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::open(archive)?;
    let xz = XzDecoder::new(file);
    let mut tar = TarArchive::new(xz);
    let mut files_extracted = 0u64;
    let mut bytes_extracted = 0u64;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size()?;
        if entry.header().entry_type().is_file() {
            entry.unpack_in(dest)?;
            files_extracted += 1;
            bytes_extracted += size;
        } else {
            entry.unpack_in(dest)?;
        }
    }
    Ok((files_extracted, bytes_extracted))
}

fn extract_tar_bz2(archive: &Path, dest: &Path) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::open(archive)?;
    let bz = BzDecoder::new(file);
    let mut tar = TarArchive::new(bz);
    let mut files_extracted = 0u64;
    let mut bytes_extracted = 0u64;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size()?;
        if entry.header().entry_type().is_file() {
            entry.unpack_in(dest)?;
            files_extracted += 1;
            bytes_extracted += size;
        } else {
            entry.unpack_in(dest)?;
        }
    }
    Ok((files_extracted, bytes_extracted))
}

fn extract_tar_plain(archive: &Path, dest: &Path) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut tar = TarArchive::new(file);
    let mut files_extracted = 0u64;
    let mut bytes_extracted = 0u64;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let size = entry.header().size()?;
        if entry.header().entry_type().is_file() {
            entry.unpack_in(dest)?;
            files_extracted += 1;
            bytes_extracted += size;
        } else {
            entry.unpack_in(dest)?;
        }
    }
    Ok((files_extracted, bytes_extracted))
}

fn strip_archive_ext(name: &str) -> &str {
    for ext in &[".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tar", ".zip"] {
        if let Some(stripped) = name.strip_suffix(ext) {
            return stripped;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create::create_archive;
    use tempfile::tempdir;

    #[test]
    fn test_extract_zip_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("hello.txt");
        fs::write(&src, b"hello world").unwrap();

        let archive = dir.path().join("test.zip");
        create_archive(&archive, &[src]).unwrap();

        let out_dir = dir.path().join("extracted");
        let result = extract_archive(&archive, Some(&out_dir)).unwrap();
        assert_eq!(result.files_extracted, 1);
        assert_eq!(result.bytes_extracted, 11);
        assert_eq!(fs::read(out_dir.join("hello.txt")).unwrap(), b"hello world");
    }

    #[test]
    fn test_extract_tar_gz_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("data.txt");
        fs::write(&src, b"data content here").unwrap();

        let archive = dir.path().join("test.tar.gz");
        create_archive(&archive, &[src]).unwrap();

        let out_dir = dir.path().join("out");
        let result = extract_archive(&archive, Some(&out_dir)).unwrap();
        assert_eq!(result.files_extracted, 1);
        assert_eq!(
            fs::read(out_dir.join("data.txt")).unwrap(),
            b"data content here"
        );
    }

    #[test]
    fn test_extract_tar_bz2_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("bz2test.txt");
        fs::write(&src, b"bzip2 compressed content").unwrap();

        let archive = dir.path().join("test.tar.bz2");
        create_archive(&archive, &[src]).unwrap();

        let out_dir = dir.path().join("bz2_out");
        let result = extract_archive(&archive, Some(&out_dir)).unwrap();
        assert_eq!(result.files_extracted, 1);
        assert_eq!(
            fs::read(out_dir.join("bz2test.txt")).unwrap(),
            b"bzip2 compressed content"
        );
    }

    #[test]
    fn test_extract_not_found() {
        let result = extract_archive(Path::new("/nonexistent/archive.zip"), None);
        assert!(matches!(result, Err(ArchiveError::NotFound(_))));
    }
}

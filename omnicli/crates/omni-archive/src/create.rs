use std::{
    fs,
    io::{self, Write, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::Result;
use bzip2::write::BzEncoder;
use flate2::{write::GzEncoder, Compression};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use tar::Builder as TarBuilder;
use walkdir::WalkDir;
use xz2::write::XzEncoder;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::error::ArchiveError;

/// Supported output formats, inferred from extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
    TarXz,
    TarBz2,
    Tar,
    #[allow(dead_code)] // Phase-2: 7z support via external library
    SevenZip,
}

impl ArchiveFormat {
    /// Detect format from file extension.
    pub fn from_path(path: &Path) -> Result<Self, ArchiveError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Ok(Self::TarGz)
        } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
            Ok(Self::TarXz)
        } else if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
            Ok(Self::TarBz2)
        } else if name.ends_with(".tar") {
            Ok(Self::Tar)
        } else if name.ends_with(".zip") {
            Ok(Self::Zip)
        } else if name.ends_with(".7z") {
            Err(ArchiveError::UnsupportedFormat(
                "7z creation is not yet supported in Phase 1. Use .zip or .tar.gz instead."
                    .to_owned(),
            ))
        } else {
            Err(ArchiveError::UnsupportedFormat(format!(
                "Cannot infer format from: {}. Use .zip, .tar.gz, .tar.xz, .tar.bz2, or .tar.",
                path.display()
            )))
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarGz => "tar.gz",
            Self::TarXz => "tar.xz",
            Self::TarBz2 => "tar.bz2",
            Self::Tar => "tar",
            Self::SevenZip => "7z",
        }
    }
}

/// Result of `omni archive create`.
#[derive(Debug, Serialize)]
pub struct CreateResult {
    pub output: String,
    pub format: String,
    pub files_added: u64,
    pub bytes_uncompressed: u64,
    pub archive_size_bytes: u64,
}

/// Create an archive at `output` from the given `inputs` (files or directories).
pub fn create_archive(output: &Path, inputs: &[PathBuf]) -> Result<CreateResult, ArchiveError> {
    let format = ArchiveFormat::from_path(output)?;

    // Only show progress bar when writing to an interactive terminal.
    let pb = if omni_core::is_tty() {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} compressing {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        bar
    } else {
        ProgressBar::hidden()
    };

    let (files_added, bytes_uncompressed) = match format {
        ArchiveFormat::Zip => create_zip(output, inputs, &pb)?,
        ArchiveFormat::TarGz => create_tar(output, inputs, &pb, TarCompression::Gz)?,
        ArchiveFormat::TarXz => create_tar(output, inputs, &pb, TarCompression::Xz)?,
        ArchiveFormat::TarBz2 => create_tar(output, inputs, &pb, TarCompression::Bz2)?,
        ArchiveFormat::Tar => create_tar(output, inputs, &pb, TarCompression::None)?,
        ArchiveFormat::SevenZip => unreachable!(),
    };

    pb.finish_and_clear();

    let archive_size_bytes = fs::metadata(output).map(|m| m.len()).unwrap_or(0);

    Ok(CreateResult {
        output: output.display().to_string(),
        format: format.as_str().to_owned(),
        files_added,
        bytes_uncompressed,
        archive_size_bytes,
    })
}

enum TarCompression {
    None,
    Gz,
    Xz,
    Bz2,
}

fn create_zip(
    output: &Path,
    inputs: &[PathBuf],
    pb: &ProgressBar,
) -> Result<(u64, u64), ArchiveError> {
    let file = fs::File::create(output)?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut files_added = 0u64;
    let mut bytes_uncompressed = 0u64;

    for input in inputs {
        if input.is_file() {
            let name = input
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");
            pb.set_message(name.to_owned());
            zip.start_file(name, options)?;
            let mut f = fs::File::open(input)?;
            bytes_uncompressed += io::copy(&mut f, &mut zip)?;
            files_added += 1;
        } else if input.is_dir() {
            for entry in WalkDir::new(input).follow_links(false) {
                let entry = entry.map_err(|e| ArchiveError::Other(e.into()))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let rel = entry
                    .path()
                    .strip_prefix(input)
                    .map_err(|e| ArchiveError::Other(e.into()))?;
                let name = rel.to_string_lossy();
                pb.set_message(name.to_string());
                zip.start_file(name.as_ref(), options)?;
                let mut f = fs::File::open(entry.path())?;
                bytes_uncompressed += io::copy(&mut f, &mut zip)?;
                files_added += 1;
            }
        }
    }

    zip.finish()?;
    Ok((files_added, bytes_uncompressed))
}

fn create_tar(
    output: &Path,
    inputs: &[PathBuf],
    pb: &ProgressBar,
    compression: TarCompression,
) -> Result<(u64, u64), ArchiveError> {
    let out_file = fs::File::create(output)?;

    let mut files_added = 0u64;
    let mut bytes_uncompressed = 0u64;

    match compression {
        TarCompression::None => {
            let mut tar = TarBuilder::new(BufWriter::new(out_file));
            for input in inputs {
                add_to_tar(&mut tar, input, pb, &mut files_added, &mut bytes_uncompressed)?;
            }
            tar.finish()?;
        }
        TarCompression::Gz => {
            let encoder = GzEncoder::new(BufWriter::new(out_file), Compression::best());
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(&mut tar, input, pb, &mut files_added, &mut bytes_uncompressed)?;
            }
            tar.into_inner()?.finish()?;
        }
        TarCompression::Xz => {
            let encoder = XzEncoder::new(BufWriter::new(out_file), 6);
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(&mut tar, input, pb, &mut files_added, &mut bytes_uncompressed)?;
            }
            tar.into_inner()?.finish()?;
        }
        TarCompression::Bz2 => {
            let encoder = BzEncoder::new(BufWriter::new(out_file), bzip2::Compression::best());
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(&mut tar, input, pb, &mut files_added, &mut bytes_uncompressed)?;
            }
            tar.into_inner()?.finish()?;
        }
    }

    Ok((files_added, bytes_uncompressed))
}

fn add_to_tar<W: Write>(
    tar: &mut TarBuilder<W>,
    input: &Path,
    pb: &ProgressBar,
    files_added: &mut u64,
    bytes_uncompressed: &mut u64,
) -> Result<(), ArchiveError> {
    if input.is_file() {
        let name = input.file_name().and_then(|n| n.to_str()).unwrap_or("file");
        pb.set_message(name.to_owned());
        let size = fs::metadata(input).map(|m| m.len()).unwrap_or(0);
        tar.append_path_with_name(input, name)?;
        *bytes_uncompressed += size;
        *files_added += 1;
    } else if input.is_dir() {
        for entry in WalkDir::new(input).follow_links(false) {
            let entry = entry.map_err(|e| ArchiveError::Other(e.into()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(input)
                .map_err(|e| ArchiveError::Other(e.into()))?;
            pb.set_message(rel.display().to_string());
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            tar.append_path_with_name(entry.path(), rel)?;
            *bytes_uncompressed += size;
            *files_added += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_zip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("hello.txt");
        fs::write(&src, b"hello world").unwrap();

        let out = dir.path().join("test.zip");
        let result = create_archive(&out, &[src]).unwrap();
        assert!(out.exists());
        assert_eq!(result.files_added, 1);
        assert_eq!(result.bytes_uncompressed, 11);
        assert!(result.archive_size_bytes > 0);
    }

    #[test]
    fn test_create_tar_gz() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        fs::write(&src, b"content").unwrap();

        let out = dir.path().join("test.tar.gz");
        let result = create_archive(&out, &[src]).unwrap();
        assert!(out.exists());
        assert_eq!(result.files_added, 1);
    }

    #[test]
    fn test_create_tar_bz2() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("file.txt");
        fs::write(&src, b"bzip2 content here").unwrap();

        let out = dir.path().join("test.tar.bz2");
        let result = create_archive(&out, &[src]).unwrap();
        assert!(out.exists());
        assert_eq!(result.files_added, 1);
        assert_eq!(result.format, "tar.bz2");
    }

    #[test]
    fn test_format_detection_error() {
        let result = ArchiveFormat::from_path(Path::new("file.unknown"));
        assert!(matches!(result, Err(ArchiveError::UnsupportedFormat(_))));
    }
}

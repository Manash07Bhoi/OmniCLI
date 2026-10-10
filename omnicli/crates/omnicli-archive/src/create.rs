use std::{
    fs,
    io::{self, BufWriter, Write},
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

/// Result of `omnicli archive create`.
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
    let pb = if omnicli_core::is_tty() {
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
        ArchiveFormat::SevenZip => {
            return Err(ArchiveError::UnsupportedFormat(
                "7z creation is not supported. Use .zip, .tar.gz, .tar.xz, .tar.bz2, or .tar."
                    .to_owned(),
            ))
        }
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

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for input in inputs {
        if input.is_file() {
            let name = input
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    ArchiveError::Other(anyhow::anyhow!(
                        "Cannot derive an archive entry name from {}",
                        input.display()
                    ))
                })?
                .to_owned();
            record_entry(&mut seen, &name)?;
            pb.set_message(name.clone());
            zip.start_file(&name, options)?;
            let mut f = fs::File::open(input)?;
            bytes_uncompressed += io::copy(&mut f, &mut zip)?;
            files_added += 1;
        } else if input.is_dir() {
            // Directory inputs keep their top-level name so the archive mirrors
            // the layout the user asked for (`omnicli archive create out.zip src` →
            // `src/...`), instead of flattening every file into the archive root.
            let root_name = input
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| {
                    ArchiveError::Other(anyhow::anyhow!(
                        "Cannot derive a directory name from {}",
                        input.display()
                    ))
                })?
                .to_owned();

            for entry in WalkDir::new(input).follow_links(false) {
                let entry = entry.map_err(|e| ArchiveError::Other(e.into()))?;
                let rel = entry
                    .path()
                    .strip_prefix(input)
                    .map_err(|e| ArchiveError::Other(e.into()))?;
                let name = if rel.as_os_str().is_empty() {
                    root_name.clone()
                } else {
                    format!("{root_name}/{}", rel.to_string_lossy().replace('\\', "/"))
                };

                if entry.file_type().is_dir() {
                    // Explicit directory entries preserve empty directories.
                    record_dir_entry(&mut seen, &name);
                    zip.add_directory(format!("{name}/"), options)?;
                    continue;
                }
                if !entry.file_type().is_file() {
                    // Symlinks and special files are not archived.
                    continue;
                }

                record_entry(&mut seen, &name)?;
                pb.set_message(name.clone());
                zip.start_file(&name, options)?;
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
                add_to_tar(
                    &mut tar,
                    input,
                    pb,
                    &mut files_added,
                    &mut bytes_uncompressed,
                )?;
            }
            tar.finish()?;
        }
        TarCompression::Gz => {
            let encoder = GzEncoder::new(BufWriter::new(out_file), Compression::best());
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(
                    &mut tar,
                    input,
                    pb,
                    &mut files_added,
                    &mut bytes_uncompressed,
                )?;
            }
            tar.into_inner()?.finish()?;
        }
        TarCompression::Xz => {
            let encoder = XzEncoder::new(BufWriter::new(out_file), 6);
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(
                    &mut tar,
                    input,
                    pb,
                    &mut files_added,
                    &mut bytes_uncompressed,
                )?;
            }
            tar.into_inner()?.finish()?;
        }
        TarCompression::Bz2 => {
            let encoder = BzEncoder::new(BufWriter::new(out_file), bzip2::Compression::best());
            let mut tar = TarBuilder::new(encoder);
            for input in inputs {
                add_to_tar(
                    &mut tar,
                    input,
                    pb,
                    &mut files_added,
                    &mut bytes_uncompressed,
                )?;
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
        // Directory inputs keep their top-level name (see `create_zip`).
        let root_name = input
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                ArchiveError::Other(anyhow::anyhow!(
                    "Cannot derive a directory name from {}",
                    input.display()
                ))
            })?
            .to_owned();

        for entry in WalkDir::new(input).follow_links(false) {
            let entry = entry.map_err(|e| ArchiveError::Other(e.into()))?;
            let rel = entry
                .path()
                .strip_prefix(input)
                .map_err(|e| ArchiveError::Other(e.into()))?;
            let name = if rel.as_os_str().is_empty() {
                root_name.clone()
            } else {
                format!("{root_name}/{}", rel.to_string_lossy().replace('\\', "/"))
            };

            if entry.file_type().is_dir() {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_mode(0o755);
                header.set_cksum();
                tar.append_data(&mut header, &name, io::empty())?;
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            pb.set_message(name.clone());
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            tar.append_path_with_name(entry.path(), &name)?;
            *bytes_uncompressed += size;
            *files_added += 1;
        }
    }
    Ok(())
}

/// Record an entry name, refusing duplicates.
///
/// Two inputs that would map onto the same archive path (e.g. `a/report.txt` and
/// `b/report.txt` passed as two directories both named `report.txt`) previously
/// produced an archive that silently lost one of them on extraction.
fn record_entry(
    seen: &mut std::collections::HashSet<String>,
    name: &str,
) -> Result<(), ArchiveError> {
    let normalized = name.trim_end_matches('/').to_owned();
    if !seen.insert(normalized.clone()) {
        return Err(ArchiveError::Other(anyhow::anyhow!(
            "duplicate archive entry '{normalized}': two inputs would overwrite each other. \
             Archive the conflicting paths separately, or rename one of them."
        )));
    }
    Ok(())
}

/// Record a directory entry name. Directories do not conflict with the files
/// they contain, so only an exact directory duplicate is rejected.
fn record_dir_entry(seen: &mut std::collections::HashSet<String>, name: &str) {
    seen.insert(format!("{}/", name.trim_end_matches('/')));
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

    #[test]
    fn test_seven_zip_creation_reports_support_error() {
        let result = ArchiveFormat::from_path(Path::new("bundle.7z"));
        assert!(matches!(result, Err(ArchiveError::UnsupportedFormat(_))));
    }

    #[test]
    fn test_zip_preserves_directory_name_and_nesting() {
        use crate::list::list_archive;

        let dir = tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), b"fn main() {}").unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();

        let out = dir.path().join("bundle.zip");
        create_archive(&out, std::slice::from_ref(&root)).unwrap();

        let names: Vec<String> = list_archive(&out)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(
            names.iter().any(|n| n == "project/src/main.rs"),
            "nested path must keep the directory prefix, got {names:?}"
        );
        assert!(
            names.iter().any(|n| n.starts_with("project/empty")),
            "empty directories must be preserved, got {names:?}"
        );
    }

    #[test]
    fn test_tar_gz_preserves_directory_name_and_nesting() {
        use crate::list::list_archive;

        let dir = tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), b"fn main() {}").unwrap();

        let out = dir.path().join("bundle.tar.gz");
        create_archive(&out, &[root]).unwrap();

        let names: Vec<String> = list_archive(&out)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(
            names.iter().any(|n| n == "project/src/main.rs"),
            "nested path must keep the directory prefix, got {names:?}"
        );
    }

    #[test]
    fn test_colliding_entry_names_are_rejected() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        fs::write(a.join("same.txt"), b"from a").unwrap();
        fs::write(b.join("same.txt"), b"from b").unwrap();

        // Both directories would contribute an entry called "same.txt" if
        // passed as files; passing the files themselves reproduces the old
        // flattening collision.
        let out = dir.path().join("collide.zip");
        let err = create_archive(&out, &[a.join("same.txt"), b.join("same.txt")]).unwrap_err();
        assert!(
            err.to_string().contains("duplicate archive entry"),
            "expected a duplicate-entry error, got: {err}"
        );
    }
}

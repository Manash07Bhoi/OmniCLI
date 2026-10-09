use std::{
    fs,
    io::{self, Read},
    path::{Component, Path},
};

use anyhow::Result;
use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;
use serde::Serialize;
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

use crate::error::ArchiveError;

/// Result of `omnicli archive extract`.
#[derive(Debug, Serialize)]
pub struct ExtractResult {
    pub archive: String,
    pub dest: String,
    pub files_extracted: u64,
    pub bytes_extracted: u64,
    /// Entries skipped because they were unsafe (traversal, absolute paths,
    /// links, devices) or hit an extraction limit.
    pub entries_skipped: u64,
    pub warnings: Vec<String>,
}

/// Resource limits applied to every extraction so a malicious or corrupt
/// archive cannot exhaust disk space.
///
/// The defaults are deliberately generous for real-world archives; tests use
/// tighter values via [`extract_archive_with_limits`].
#[derive(Debug, Clone, Copy)]
pub struct ExtractLimits {
    /// Maximum number of entries processed.
    pub max_entries: u64,
    /// Maximum total uncompressed bytes written to disk.
    pub max_total_bytes: u64,
    /// Maximum uncompressed:compressed ratio, i.e. the decompression-bomb guard.
    pub max_ratio: u64,
}

impl Default for ExtractLimits {
    fn default() -> Self {
        Self {
            max_entries: 1_000_000,
            max_total_bytes: 64 * 1024 * 1024 * 1024, // 64 GiB
            max_ratio: 10_000,
        }
    }
}

/// Detect the archive format by reading magic bytes from the file (not from the extension).
fn detect_format_by_magic(path: &Path) -> Result<&'static str, ArchiveError> {
    let mut f = fs::File::open(path)?;
    let mut magic = [0u8; 8];
    let n = f.read(&mut magic)?;

    // ZIP: PK\x03\x04 (also accept an empty archive: PK\x05\x06)
    if n >= 4
        && magic[0] == 0x50
        && magic[1] == 0x4B
        && (magic[2] == 0x03 || magic[2] == 0x05)
        && (magic[3] == 0x04 || magic[3] == 0x06)
    {
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
    // TAR (uncompressed): magic "ustar" at offset 257.
    let mut ustar = [0u8; 6];
    let mut f = fs::File::open(path)?;
    if f.seek_read(&mut ustar, 257) && &ustar == b"ustar\0" {
        return Ok("tar");
    }
    // Fall back to the extension for tars that predate the ustar magic.
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

/// Small helper so the ustar probe reads without pulling in `Seek` everywhere.
trait SeekRead {
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> bool;
}
impl SeekRead for fs::File {
    fn seek_read(&mut self, buf: &mut [u8], offset: u64) -> bool {
        use std::io::{Seek, SeekFrom};
        self.seek(SeekFrom::Start(offset)).is_ok() && self.read_exact(buf).is_ok()
    }
}

/// Extract an archive to `dest` (defaults to a directory next to the archive).
///
/// Extraction is hardened:
/// * every entry path is validated against the destination (no `..`, no absolute paths);
/// * symlinks, hardlinks, device nodes and FIFOs are skipped and reported;
/// * entry count and total uncompressed size are capped;
/// * a decompression ratio above the bomb threshold aborts the extraction.
pub fn extract_archive(archive: &Path, dest: Option<&Path>) -> Result<ExtractResult, ArchiveError> {
    extract_archive_with_limits(archive, dest, ExtractLimits::default())
}

/// [`extract_archive`] with explicit resource limits.
pub fn extract_archive_with_limits(
    archive: &Path,
    dest: Option<&Path>,
    limits: ExtractLimits,
) -> Result<ExtractResult, ArchiveError> {
    if !archive.exists() {
        return Err(ArchiveError::NotFound(archive.display().to_string()));
    }
    if archive.is_dir() {
        return Err(ArchiveError::Corrupt(format!(
            "{} is a directory, not an archive",
            archive.display()
        )));
    }

    let dest_path = match dest {
        Some(d) => {
            if d.as_os_str().is_empty() {
                return Err(ArchiveError::Corrupt(
                    "extraction destination must not be empty".to_owned(),
                ));
            }
            d.to_owned()
        }
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
    let compressed_size = archive.metadata().map(|m| m.len()).unwrap_or(1).max(1);

    let mut guard = Guard::new(compressed_size, limits);
    let stats = match fmt {
        "zip" => extract_zip(archive, &dest_path, &mut guard)?,
        "tar.gz" => {
            let file = fs::File::open(archive)?;
            extract_tar(
                TarArchive::new(GzDecoder::new(file)),
                &dest_path,
                &mut guard,
            )?
        }
        "tar.xz" => {
            let file = fs::File::open(archive)?;
            extract_tar(
                TarArchive::new(XzDecoder::new(file)),
                &dest_path,
                &mut guard,
            )?
        }
        "tar.bz2" => {
            let file = fs::File::open(archive)?;
            extract_tar(
                TarArchive::new(BzDecoder::new(file)),
                &dest_path,
                &mut guard,
            )?
        }
        "tar" => {
            let file = fs::File::open(archive)?;
            extract_tar(TarArchive::new(file), &dest_path, &mut guard)?
        }
        other => {
            return Err(ArchiveError::UnsupportedFormat(other.to_owned()));
        }
    };

    Ok(ExtractResult {
        archive: archive.display().to_string(),
        dest: dest_path.display().to_string(),
        files_extracted: stats.files,
        bytes_extracted: stats.bytes,
        entries_skipped: stats.skipped,
        warnings: guard.warnings,
    })
}

/// Running totals plus the limits that stop hostile archives.
struct Guard {
    compressed_size: u64,
    entries: u64,
    bytes_written: u64,
    limits: ExtractLimits,
    warnings: Vec<String>,
}

impl Guard {
    fn new(compressed_size: u64, limits: ExtractLimits) -> Self {
        Self {
            compressed_size,
            entries: 0,
            bytes_written: 0,
            limits,
            warnings: Vec::new(),
        }
    }

    fn account_entry(&mut self) -> Result<(), ArchiveError> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(ArchiveError::Corrupt(format!(
                "archive contains more than {} entries; refusing to extract",
                self.limits.max_entries
            )));
        }
        Ok(())
    }

    /// Record `n` bytes that were written to disk, enforcing both the absolute
    /// cap and the compression-ratio cap.
    fn account_bytes(&mut self, n: u64) -> Result<(), ArchiveError> {
        self.bytes_written = self.bytes_written.saturating_add(n);
        if self.bytes_written > self.limits.max_total_bytes {
            return Err(ArchiveError::Corrupt(format!(
                "archive expands beyond the {} limit; refusing to extract",
                omnicli_core::format_bytes(self.limits.max_total_bytes)
            )));
        }
        let ratio = self.bytes_written / self.compressed_size.max(1);
        if ratio > self.limits.max_ratio {
            return Err(ArchiveError::Corrupt(format!(
                "archive compression ratio ({ratio}:1) exceeds the {}:1 limit; \
                 this looks like a decompression bomb",
                self.limits.max_ratio
            )));
        }
        Ok(())
    }

    fn skip(&mut self, entry: &str, reason: &str) {
        self.warnings.push(format!("skipped '{entry}': {reason}"));
    }
}

struct ExtractCounts {
    files: u64,
    bytes: u64,
    skipped: u64,
}

/// Validate and resolve an archive entry path inside `dest`.
///
/// Rejects absolute paths, drive prefixes, and any `..` component — the classic
/// zip-slip / tar-slip escape. Returns the safe absolute-ish join target.
fn safe_join(dest: &Path, entry_name: &str) -> Result<std::path::PathBuf, ArchiveError> {
    // Normalise separators: some writers use backslashes even for tar/zip.
    let name = entry_name.replace('\\', "/");

    if name.starts_with('/') {
        return Err(ArchiveError::Corrupt(format!(
            "unsafe absolute path in archive entry: {entry_name}"
        )));
    }
    // Windows drive prefix such as C:/… would escape the destination.
    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return Err(ArchiveError::Corrupt(format!(
            "unsafe drive-relative path in archive entry: {entry_name}"
        )));
    }

    for component in name.split('/') {
        if component == ".." {
            return Err(ArchiveError::Corrupt(format!(
                "path traversal detected in archive entry: {entry_name}"
            )));
        }
    }

    // Second line of defence: the resolved path must stay under `dest`.
    let candidate = dest.join(&name);
    let mut depth = 0usize;
    for component in candidate.components() {
        match component {
            Component::ParentDir => {
                depth = depth.saturating_sub(1);
            }
            Component::CurDir => {}
            _ => depth += 1,
        }
        if depth == 0 {
            return Err(ArchiveError::Corrupt(format!(
                "path escapes the extraction root: {entry_name}"
            )));
        }
    }

    Ok(candidate)
}

fn extract_zip(
    archive: &Path,
    dest: &Path,
    guard: &mut Guard,
) -> Result<ExtractCounts, ArchiveError> {
    let file = fs::File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut skipped = 0u64;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        guard.account_entry()?;

        let raw_name = entry.name().to_owned();

        // Reject symlinks and other non-regular entries outright.
        let mode = entry.unix_mode().unwrap_or(0o644);
        if mode & 0o170000 == 0o120000 {
            guard.skip(&raw_name, "symbolic link entries are not extracted");
            skipped += 1;
            continue;
        }

        let out_path = safe_join(dest, &raw_name)?;

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = fs::File::create(&out_path)?;
        let copied = io::copy(&mut entry, &mut out_file)?;
        drop(out_file);
        guard.account_bytes(copied)?;
        bytes += copied;
        files += 1;

        // Preserve the permission bits, but never the setuid/setgid/sticky bits.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let safe_mode = mode & 0o777;
            let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(safe_mode));
        }
    }

    Ok(ExtractCounts {
        files,
        bytes,
        skipped,
    })
}

/// Shared tar extraction path for every compression flavour.
///
/// Uses an explicit `unpack_in` return value check (rather than ignoring it) so a
/// rejected entry is reported instead of silently skipped, and refuses to create
/// links or special files.
fn extract_tar<R: Read>(
    mut tar: TarArchive<R>,
    dest: &Path,
    guard: &mut Guard,
) -> Result<ExtractCounts, ArchiveError> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut skipped = 0u64;

    for entry in tar.entries()? {
        let mut entry = entry?;
        guard.account_entry()?;

        let entry_type = entry.header().entry_type();
        let path_string = entry
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        if entry_type.is_symlink() || entry_type.is_hard_link() {
            guard.skip(&path_string, "link entries are not extracted");
            skipped += 1;
            continue;
        }
        if !entry_type.is_file() && !entry_type.is_dir() {
            guard.skip(&path_string, "unsupported entry type");
            skipped += 1;
            continue;
        }

        let size = entry.header().size().unwrap_or(0);

        // `unpack_in` performs its own traversal check and returns Ok(false) when
        // it refuses an entry.
        let unpacked = entry.unpack_in(dest).map_err(|e| {
            ArchiveError::Corrupt(format!("failed to extract '{path_string}': {e}"))
        })?;

        if !unpacked {
            guard.skip(&path_string, "entry would escape the destination directory");
            skipped += 1;
            continue;
        }

        if entry_type.is_file() {
            guard.account_bytes(size)?;
            bytes += size;
            files += 1;
        }
    }

    Ok(ExtractCounts {
        files,
        bytes,
        skipped,
    })
}

fn strip_archive_ext(name: &str) -> &str {
    for ext in &[
        ".tar.gz", ".tgz", ".tar.xz", ".txz", ".tar.bz2", ".tbz2", ".tar", ".zip",
    ] {
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
        assert_eq!(result.entries_skipped, 0);
        assert!(result.warnings.is_empty());
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
    fn test_extract_tar_xz_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("xz.txt");
        fs::write(&src, b"xz compressed").unwrap();

        let archive = dir.path().join("test.tar.xz");
        create_archive(&archive, &[src]).unwrap();

        let out_dir = dir.path().join("xz_out");
        let result = extract_archive(&archive, Some(&out_dir)).unwrap();
        assert_eq!(result.files_extracted, 1);
        assert_eq!(fs::read(out_dir.join("xz.txt")).unwrap(), b"xz compressed");
    }

    #[test]
    fn test_extract_nested_directories_roundtrip() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), b"deep").unwrap();

        let archive = dir.path().join("nested.tar.gz");
        create_archive(&archive, &[dir.path().join("a")]).unwrap();

        let out_dir = dir.path().join("nested_out");
        let result = extract_archive(&archive, Some(&out_dir)).unwrap();
        assert!(result.files_extracted >= 1);
        assert!(out_dir.join("a/b/deep.txt").exists());
    }

    #[test]
    fn test_extract_not_found() {
        let result = extract_archive(Path::new("/nonexistent/archive.zip"), None);
        assert!(matches!(result, Err(ArchiveError::NotFound(_))));
    }

    #[test]
    fn test_extract_directory_is_rejected() {
        let dir = tempdir().unwrap();
        let err = extract_archive(dir.path(), Some(&dir.path().join("out"))).unwrap_err();
        assert!(err.to_string().contains("is a directory"), "got: {err}");
    }

    #[test]
    fn test_detect_format_rejects_non_archive() {
        let dir = tempdir().unwrap();
        let junk = dir.path().join("junk.bin");
        fs::write(&junk, b"not an archive at all").unwrap();
        assert!(detect_format_by_magic(&junk).is_err());
    }

    #[test]
    fn test_detect_format_by_magic_ignores_misleading_extension() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("payload.txt");
        fs::write(&src, b"payload").unwrap();
        // A real tar.gz that claims to be a .zip must still be detected as gzip.
        let lying = dir.path().join("payload.zip");
        create_archive(&dir.path().join("real.tar.gz"), &[src]).unwrap();
        fs::copy(dir.path().join("real.tar.gz"), &lying).unwrap();
        assert_eq!(detect_format_by_magic(&lying).unwrap(), "tar.gz");
    }

    // ── zip-slip / tar-slip defence ──────────────────────────────────────────

    #[test]
    fn test_safe_join_rejects_parent_traversal() {
        let dest = Path::new("/tmp/dest");
        assert!(safe_join(dest, "../evil.txt").is_err());
        assert!(safe_join(dest, "a/../../evil.txt").is_err());
        assert!(safe_join(dest, "..\\evil.txt").is_err());
    }

    #[test]
    fn test_safe_join_rejects_absolute_paths() {
        let dest = Path::new("/tmp/dest");
        assert!(safe_join(dest, "/etc/passwd").is_err());
        assert!(safe_join(dest, "C:/Windows/system32/evil.dll").is_err());
    }

    #[test]
    fn test_safe_join_accepts_normal_paths() {
        let dest = Path::new("/tmp/dest");
        assert_eq!(
            safe_join(dest, "a/b/c.txt").unwrap(),
            Path::new("/tmp/dest/a/b/c.txt")
        );
        assert_eq!(
            safe_join(dest, "./a.txt").unwrap(),
            Path::new("/tmp/dest/a.txt")
        );
        // Backslash separators are normalised rather than rejected.
        assert_eq!(
            safe_join(dest, "a\\b.txt").unwrap(),
            Path::new("/tmp/dest/a/b.txt")
        );
    }

    #[test]
    fn test_zip_slip_entry_is_rejected_during_extraction() {
        use std::io::Write;
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("evil.zip");

        {
            let file = fs::File::create(&archive_path).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            zw.start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"pwned").unwrap();
            zw.finish().unwrap();
        }

        let out = dir.path().join("out");
        let err = extract_archive(&archive_path, Some(&out)).unwrap_err();
        assert!(
            err.to_string().contains("path traversal") || err.to_string().contains("escape"),
            "expected a traversal rejection, got: {err}"
        );
        assert!(
            !dir.path().join("escaped.txt").exists(),
            "the escaping file must never be written"
        );
    }

    #[test]
    fn test_tar_symlink_entries_are_skipped() {
        // The `zip` crate masks `unix_permissions` to 0o777, so a symlink-marked
        // zip cannot be produced through its high-level writer. TAR can.
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("link.tar");

        {
            let file = fs::File::create(&archive_path).unwrap();
            let mut builder = tar::Builder::new(file);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            builder
                .append_link(&mut header, "escape", "/etc/passwd")
                .unwrap();
            builder.finish().unwrap();
        }

        let out = dir.path().join("out");
        let result = extract_archive(&archive_path, Some(&out)).unwrap();
        assert_eq!(result.files_extracted, 0);
        assert_eq!(result.entries_skipped, 1);
        assert!(
            !out.join("escape").exists(),
            "symlink entries must never be materialised"
        );
        assert!(result.warnings.iter().any(|w| w.contains("link")));
    }

    #[test]
    fn test_zip_directory_prefix_survives_roundtrip() {
        // Regression: `archive create` used to flatten directory inputs, so
        // extracting produced a different tree than the one that was archived.
        let dir = tempdir().unwrap();
        let root = dir.path().join("project");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), b"fn main() {}").unwrap();
        fs::write(root.join("README.md"), b"docs").unwrap();

        let archive = dir.path().join("project.zip");
        create_archive(&archive, &[root]).unwrap();

        let out = dir.path().join("out");
        let result = extract_archive(&archive, Some(&out)).unwrap();
        assert_eq!(result.files_extracted, 2);
        assert_eq!(
            fs::read(out.join("project/src/main.rs")).unwrap(),
            b"fn main() {}"
        );
        assert_eq!(fs::read(out.join("project/README.md")).unwrap(), b"docs");
    }

    #[test]
    fn test_entry_count_limit_is_enforced() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let archive = dir.path().join("many.zip");
        create_archive(&archive, &[a, b]).unwrap();

        let out = dir.path().join("out");
        let err = extract_archive_with_limits(
            &archive,
            Some(&out),
            ExtractLimits {
                max_entries: 1,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("more than 1 entries"),
            "expected the entry cap to fire, got: {err}"
        );
    }

    #[test]
    fn test_total_bytes_limit_is_enforced() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("payload.bin");
        fs::write(&src, vec![7u8; 4096]).unwrap();
        let archive = dir.path().join("payload.tar.gz");
        create_archive(&archive, &[src]).unwrap();

        let out = dir.path().join("out");
        let err = extract_archive_with_limits(
            &archive,
            Some(&out),
            ExtractLimits {
                max_total_bytes: 1024,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("expands beyond"),
            "expected the byte cap to fire, got: {err}"
        );
    }

    #[test]
    fn test_compression_ratio_limit_is_enforced() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("zeros.bin");
        fs::write(&src, vec![0u8; 1024 * 1024]).unwrap();
        let archive = dir.path().join("zeros.tar.gz");
        create_archive(&archive, &[src]).unwrap();

        let out = dir.path().join("out");
        let err = extract_archive_with_limits(
            &archive,
            Some(&out),
            ExtractLimits {
                max_ratio: 2,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("compression ratio"),
            "expected the ratio guard to fire, got: {err}"
        );
    }

    #[test]
    fn test_default_limits_allow_normal_archives() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("payload.bin");
        fs::write(&src, vec![0u8; 1024 * 1024]).unwrap();
        let archive = dir.path().join("payload.tar.gz");
        create_archive(&archive, &[src]).unwrap();

        let out = dir.path().join("out");
        let result = extract_archive(&archive, Some(&out)).unwrap();
        assert_eq!(result.files_extracted, 1);
        assert_eq!(result.bytes_extracted, 1024 * 1024);
    }
}

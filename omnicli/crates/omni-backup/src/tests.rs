#[cfg(test)]
mod backup_tests {
    use crate::{backup_create, backup_restore, backup_verify, restore::list_snapshots};
    use std::fs;
    use tempfile::TempDir;

    fn setup_source(dir: &TempDir) {
        fs::write(dir.path().join("file_a.txt"), b"Hello from OmniCLI backup").unwrap();
        fs::write(dir.path().join("file_b.txt"), b"Another file with content").unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("nested.txt"), b"Nested file content").unwrap();
    }

    // ── create ────────────────────────────────────────────────────────────────

    #[test]
    fn backup_create_produces_snapshot() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        setup_source(&src);

        let result = backup_create(src.path(), dst.path(), "test-job", true).unwrap();

        assert_eq!(result.job_name, "test-job");
        assert!(result.files_total >= 3, "should count at least 3 files");
        assert!(result.bytes_transferred > 0);
        assert!(!result.snapshot_id.is_empty());
    }

    #[test]
    fn backup_create_nonexistent_source_returns_error() {
        let dst = TempDir::new().unwrap();
        let fake_src = std::path::Path::new("/tmp/this-path-does-not-exist-omni-test");
        let r = backup_create(fake_src, dst.path(), "fail-job", true);
        assert!(r.is_err());
    }

    #[test]
    fn backup_create_incremental_skips_unchanged() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        setup_source(&src);

        // First backup
        let r1 = backup_create(src.path(), dst.path(), "incr-job", true).unwrap();
        assert!(r1.files_new >= 3);

        // Second backup — nothing changed, all files should be unchanged
        let r2 = backup_create(src.path(), dst.path(), "incr-job", true).unwrap();
        assert_eq!(r2.files_new, 0, "no new files on unchanged source");
        assert!(r2.files_unchanged >= 3);
    }

    #[test]
    fn backup_create_detects_changed_files() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        setup_source(&src);

        backup_create(src.path(), dst.path(), "change-job", true).unwrap();

        // Modify a file
        fs::write(src.path().join("file_a.txt"), b"Modified content!").unwrap();

        let r2 = backup_create(src.path(), dst.path(), "change-job", true).unwrap();
        assert_eq!(r2.files_new, 1, "one modified file should be re-backed-up");
    }

    // ── restore ───────────────────────────────────────────────────────────────

    #[test]
    fn backup_restore_recreates_files() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        let restore_target = TempDir::new().unwrap();
        setup_source(&src);

        let created = backup_create(src.path(), dst.path(), "restore-job", true).unwrap();
        let snapshot_id = created.snapshot_id.clone();

        let result = backup_restore(dst.path(), &snapshot_id, restore_target.path()).unwrap();
        assert_eq!(result.files_restored, created.files_total);

        // Verify restored files exist
        assert!(restore_target.path().join("file_a.txt").exists());
        assert!(restore_target.path().join("file_b.txt").exists());
        assert!(restore_target
            .path()
            .join("subdir")
            .join("nested.txt")
            .exists());
    }

    #[test]
    fn restore_wrong_snapshot_id_returns_error() {
        let dst = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let r = backup_restore(dst.path(), "nonexistent-snapshot-id", target.path());
        assert!(r.is_err());
    }

    // ── verify ────────────────────────────────────────────────────────────────

    #[test]
    fn backup_verify_passes_on_intact_backup() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        setup_source(&src);

        let created = backup_create(src.path(), dst.path(), "verify-job", true).unwrap();
        let result = backup_verify(dst.path(), &created.snapshot_id).unwrap();

        assert_eq!(
            result.files_corrupt, 0,
            "intact backup should verify successfully"
        );
        assert_eq!(result.files_checked as u64, created.files_total);
        assert_eq!(result.files_corrupt, 0);
    }

    // ── list_snapshots ────────────────────────────────────────────────────────

    #[test]
    fn list_snapshots_empty_for_unknown_job() {
        let dst = TempDir::new().unwrap();
        let snapshots = list_snapshots(dst.path()).unwrap();
        assert!(snapshots.is_empty());
    }
}

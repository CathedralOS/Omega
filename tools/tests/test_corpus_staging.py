#!/usr/bin/env python3
"""Exercise the corpus's real Rust staging code without linking the compiler.

Two module instances have separate session globals but the same process id,
which deterministically models separate PID namespaces sharing temporary storage.
Run with the repository's pinned rustc available:

    python3 tools/tests/test_corpus_staging.py -v
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SUPPORT = ROOT / "omega-rust/omega/compilation/driver/tests/support/sealed_fixture_staging.rs"

PROBE = r'''
use std::{fs, path::{Path, PathBuf}};

fn fixture(repository: &Path, contents: &str) -> PathBuf {
    let root = repository.join("tests/omega/pass/staging");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("main.txt"), contents).unwrap();
    root
}

// Model a timed-out worker holding its build path while the coordinator exits.
fn detached_cleanup() {
    let old_root = first::temporary_session_root().to_path_buf();
    let old_build = old_root.join("build-0");
    fs::create_dir(&old_build).unwrap();
    let (resume_sender, resume_receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        resume_receiver.recv().unwrap();
        fs::remove_dir_all(old_build).unwrap();
    });
    first::discard_staged_snapshots(1);
    assert!(old_root.exists(), "a live worker must retain the reservation");
    let new_root = second::temporary_session_root().to_path_buf();
    assert_ne!(old_root, new_root, "a live worker's root must not be reused");
    let new_build = new_root.join("build-0");
    fs::create_dir(&new_build).unwrap();
    fs::write(new_build.join("peer-marker"), "peer").unwrap();
    resume_sender.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(fs::read_to_string(new_build.join("peer-marker")).unwrap(), "peer");
    first::discard_staged_snapshots(0);
    assert!(!old_root.exists());
    assert!(new_build.join("peer-marker").exists());
    second::discard_staged_snapshots(0);
    assert!(!new_root.exists());
    println!("detached worker cleanup preserves the peer: passed");
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("allocate") => { first::staging_session_root(); return; }
        Some("detached") => { detached_cleanup(); return; }
        _ => {}
    }
    let temporary = std::env::temp_dir();
    // A retained candidate belongs to another run, even with our process id.
    let occupied = temporary.join(format!("omega-corpus-session-{}-0", std::process::id()));
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("marker"), "retained").unwrap();
    // A file also occupies a candidate; allocation must skip it, not remove it.
    let occupied_file = temporary.join(format!("omega-corpus-session-{}-1", std::process::id()));
    fs::write(&occupied_file, "retained file").unwrap();

    // Simultaneous workers must all observe the same once-allocated source root.
    let start = std::sync::Barrier::new(17);
    let first_root = std::thread::scope(|scope| {
        let second_worker = scope.spawn(|| {
            start.wait();
            second::staging_session_root()
        });
        let workers: Vec<_> = (0..16)
            .map(|_| scope.spawn(|| {
                start.wait();
                first::staging_session_root()
            }))
            .collect();
        let roots: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
        assert!(roots.iter().all(|root| *root == roots[0]));
        assert_ne!(roots[0], second_worker.join().unwrap(), "same-pid sessions must not share roots");
        roots[0].to_path_buf()
    });

    let first_repository = temporary.join("first-repository");
    let second_repository = temporary.join("second-repository");
    let first_fixture = fixture(&first_repository, "first authored result");
    let second_fixture = fixture(&second_repository, "second authored result");
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::write(first_fixture.join("executable"), "authored executable").unwrap();
        fs::set_permissions(first_fixture.join("executable"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let first_source = first::stage_sealed_snapshot(&first_repository, &first_fixture);
    let first_build = first::fresh_staging_build_dir(&first_repository, &first_fixture);
    fs::create_dir_all(&first_build).unwrap();
    fs::write(first_build.join("marker"), "first build").unwrap();
    let second_source = second::stage_sealed_snapshot(&second_repository, &second_fixture);
    let second_build = second::fresh_staging_build_dir(&second_repository, &second_fixture);
    fs::create_dir_all(&second_build).unwrap();
    fs::write(second_build.join("marker"), "second build").unwrap();

    assert_ne!(first_source, second_source, "same-pid sessions must not share snapshots");
    assert_ne!(first_build, second_build, "same-pid sessions must not share build staging");
    assert_eq!(fs::read_to_string(first_source.join("main.txt")).unwrap(), "first authored result");
    assert_eq!(fs::read_to_string(second_source.join("main.txt")).unwrap(), "second authored result");
    assert_eq!(fs::read_to_string(first_build.join("marker")).unwrap(), "first build");
    assert_eq!(fs::read_to_string(second_build.join("marker")).unwrap(), "second build");
    assert!(!first_build.starts_with(first::staging_session_root()));
    assert_eq!(first_source.strip_prefix(first::staging_session_root()).unwrap(),
               Path::new("tests/omega/pass/staging"));
    assert_eq!(first::staging_session_root(), first_root);
    assert_eq!(first_source.canonicalize().unwrap(), first_source);
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&first_source).unwrap().permissions().mode() & 0o777, 0o555);
        assert_eq!(fs::metadata(first_source.join("main.txt")).unwrap().permissions().mode() & 0o777, 0o444);
        assert_eq!(fs::metadata(first_source.join("executable")).unwrap().permissions().mode() & 0o777, 0o555);
        assert_eq!(fs::metadata(&first_fixture).unwrap().permissions().mode() & 0o200, 0o200);
    }

    // Repeated preparation still refreshes authored bytes and clears build output.
    fs::write(first_fixture.join("main.txt"), "edited authored result").unwrap();
    assert_eq!(first::stage_sealed_snapshot(&first_repository, &first_fixture), first_source);
    assert_eq!(fs::read_to_string(first_source.join("main.txt")).unwrap(), "edited authored result");
    assert_eq!(first::fresh_staging_build_dir(&first_repository, &first_fixture), first_build);
    assert!(!first_build.exists());

    first::discard_staged_snapshots(0);
    assert!(!first_source.exists());
    assert_eq!(fs::read_to_string(second_source.join("main.txt")).unwrap(), "second authored result");
    assert_eq!(fs::read_to_string(second_build.join("marker")).unwrap(), "second build");
    second::discard_staged_snapshots(0);
    assert!(!second_source.exists());
    assert!(!second_build.exists());
    assert_eq!(fs::read_to_string(occupied.join("marker")).unwrap(), "retained");
    assert_eq!(fs::read_to_string(occupied_file).unwrap(), "retained file");
    println!("same-pid snapshots, build staging, sealing, refresh, worker sharing and cleanup: passed");
}
'''


def build_probe(source, executable, support=SUPPORT):
    """Compile the actual support module twice, without mocks or compiler crates."""
    source.write_text(
        f'#[path = {json.dumps(str(support), ensure_ascii=False)}] mod first;\n'
        f'#[path = {json.dumps(str(support), ensure_ascii=False)}] mod second;\n' + PROBE,
        encoding="utf-8",
    )
    result = subprocess.run(
        ["rustc", "--edition=2024", str(source), "-o", str(executable)],
        capture_output=True, text=True, cwd=ROOT,
    )
    if result.returncode:
        raise RuntimeError(result.stderr)


class CorpusStaging(unittest.TestCase):
    def test_same_pid_sessions_preserve_authored_bytes_and_cleanup_ownership(self):
        self.assertIsNotNone(shutil.which("rustc"), "the pinned Rust toolchain is required")
        with tempfile.TemporaryDirectory(prefix="omega-staging-test-") as directory:
            root = Path(directory)
            executable = root / ("probe.exe" if os.name == "nt" else "probe")
            build_probe(root / "probe.rs", executable)
            temporary = root / "shared-temporary"
            temporary.mkdir()
            environment = dict(os.environ, TMPDIR=str(temporary), TMP=str(temporary), TEMP=str(temporary))
            result = subprocess.run(
                [str(executable)], env=environment, capture_output=True, text=True,
                timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("cleanup: passed", result.stdout)
            # Only authored inputs and deliberately occupied candidates remain.
            self.assertEqual(len(list(temporary.iterdir())), 4)
            detached = root / "detached-temporary"
            detached.mkdir()
            environment.update(TMPDIR=str(detached), TMP=str(detached), TEMP=str(detached))
            delayed = subprocess.run(
                [str(executable), "detached"], env=environment,
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(delayed.returncode, 0, delayed.stderr)
            self.assertIn("detached worker cleanup preserves the peer: passed", delayed.stdout)
            self.assertEqual(list(detached.iterdir()), [])
            unavailable = root / "not-a-directory"
            unavailable.write_text("retain this file", encoding="utf-8")
            environment.update(TMPDIR=str(unavailable), TMP=str(unavailable), TEMP=str(unavailable))
            failed = subprocess.run(
                [str(executable), "allocate"], env=environment,
                capture_output=True, text=True, timeout=30,
            )
            self.assertNotEqual(failed.returncode, 0)
            self.assertIn("could not be created", failed.stderr)
            self.assertEqual(unavailable.read_text(encoding="utf-8"), "retain this file")


if __name__ == "__main__":
    unittest.main()

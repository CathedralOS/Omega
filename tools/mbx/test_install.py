#!/usr/bin/env python3
"""Check installer replacement and reuse without compiling or changing PATH tools."""
import hashlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[2]
ARCHIVE = Path(sys.argv.pop(1)).resolve(strict=True) if len(sys.argv) > 1 else None


class InstallerTests(unittest.TestCase):
    def setUp(self):
        if ARCHIVE is None:
            self.fail("pass the pinned upstream source archive as the first argument")
        self.temp = tempfile.TemporaryDirectory(prefix="omega-mbx-installer-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fake = self.root / "bin"
        self.fake.mkdir()
        self.version = (REPO / "tools/mbx-version").read_text().strip()
        self.events = self.root / "events"
        bootstrap = f'''#!/usr/bin/env bash
set -eu
test "$1" = build
test "$CARGO_HOME" = "$MBX_INSTALL_WORK/cargo"
test "$MBX_CACHE_DIR" = "$MBX_INSTALL_WORK/cache"
test ! -e unexpected.rs
grep -q checksum-hash-algorithm crates/mbx-cache-rustc/src/lib.rs
grep -q '{self.version}' crates/mbx/Cargo.toml
mkdir -p "$CARGO_TARGET_DIR/release"
printf '#!/bin/sh\\nprintf "mbx {self.version}\\\\n"\\n' > "$CARGO_TARGET_DIR/release/mbx"
chmod +x "$CARGO_TARGET_DIR/release/mbx"
echo build >> '{self.events}'
'''.encode()
        release = self.root / "release.tar.gz"
        with tarfile.open(release, "w:gz") as archive:
            member = tarfile.TarInfo("mbx")
            member.size = len(bootstrap)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(bootstrap))
        self.shim("rustup", "#!/bin/sh\necho nightly-test-x86_64-unknown-linux-gnu\n")
        self.shim("uname", "#!/bin/sh\nif [ \"$1\" = -s ]; then echo Linux; else echo x86_64; fi\n")
        self.shim("curl", f'''#!{sys.executable}
import hashlib, pathlib, shutil, sys
args=sys.argv[1:]; output=pathlib.Path(args[args.index('-o')+1]); url=next(a for a in args if a.startswith('https://'))
with pathlib.Path({str(self.events)!r}).open('a') as f: f.write('download\\n')
release=pathlib.Path({str(release)!r})
if 'codeload.github.com' in url:
 if pathlib.Path({str(self.root / 'corrupt')!r}).exists(): output.write_bytes(b'corrupt')
 else: shutil.copyfile({str(ARCHIVE)!r},output)
elif url.endswith('SHA256SUMS'):
 output.write_text(hashlib.sha256(release.read_bytes()).hexdigest()+'  mbx-x86_64-unknown-linux-gnu.tar.gz\\n')
else: shutil.copyfile(release,output)
''')
        self.destination = self.root / "installed"
        self.env = dict(os.environ)
        self.env.update(PATH=str(self.fake) + os.pathsep + os.environ["PATH"],
                        MBX_INSTALL_WORK=str(self.root / "work"))

    def shim(self, name, text):
        path = self.fake / name
        path.write_text(text)
        path.chmod(0o755)

    def install(self):
        return subprocess.run(
            ["bash", str(REPO / "tools/mbx/install.sh"), str(self.destination)],
            env=self.env, capture_output=True, text=True,
        )

    def test_source_identity_receipt_makes_installation_idempotent(self):
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stderr)
        binary = self.destination / "mbx"
        receipt = (self.destination / "mbx.source").read_text().splitlines()
        self.assertIn(self.version, receipt[0])
        self.assertEqual(receipt[1], hashlib.sha256(binary.read_bytes()).hexdigest())
        events = self.events.read_text()
        second = self.install()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(self.events.read_text(), events, "reuse downloaded or rebuilt")
        self.assertEqual(second.stdout.strip(), f"mbx {self.version}")

    def test_damaged_source_keeps_the_existing_installation(self):
        self.destination.mkdir()
        binary = self.destination / "mbx"
        binary.write_text("existing executable\n")
        (self.root / "corrupt").touch()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum mismatch", result.stderr)
        self.assertEqual(binary.read_text(), "existing executable\n")
        self.assertFalse((self.destination / "mbx.source").exists())
        self.assertNotIn("build", self.events.read_text().splitlines())

    def test_same_version_with_changed_binary_requires_a_rebuild(self):
        first = self.install()
        self.assertEqual(first.returncode, 0, first.stderr)
        binary = self.destination / "mbx"
        original = binary.read_text()
        binary.write_text(original + "# locally changed\n")
        second = self.install()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(binary.read_text(), original)
        self.assertEqual(self.events.read_text().splitlines().count("build"), 2)

    def test_stale_source_is_neither_compiled_nor_deleted(self):
        previous = self.root / "work/source"
        previous.mkdir(parents=True)
        stale = previous / "unexpected.rs"
        stale.write_text("caller-owned diagnostic source\n")
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(stale.read_text(), "caller-owned diagnostic source\n")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Execute the bundled directory record/copy code without Windows system calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile


def declaration(root, relative, prefix):
    """Stage an entire production declaration, including its contracts/body."""
    text = (root / relative).read_text()
    starts = list(re.finditer(r"^" + re.escape(prefix), text, re.MULTILINE))
    assert len(starts) == 1, (relative, prefix, len(starts))
    start = starts[0].start()
    opening = text.index("{", start)
    depth, end = 1, opening + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start:end] + "\n"


PROGRAM = r'''
machine program() crashes Trap {
    let mut native: WindowsNativeDirectoryEntryInfo;
    native.name_bytes = 8192;
    let decoded: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8256);
    transition decoded { WindowsDirectoryStep::Name { length } -> decoded_length(native, length) _ -> wrong() }
    machine decoded_length(native: WindowsNativeDirectoryEntryInfo, length: u64) crashes Trap {
        transition length == 4096 { true -> malformed(native) false -> wrong() }
    }
    machine malformed(mut native: WindowsNativeDirectoryEntryInfo) crashes Trap {
        let short: WindowsDirectoryStep = windows_directory_decode_entry(&native, 63);
        let long: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8257);
        let payload: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8254);
        native.name_bytes = 3;
        let odd: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8256);
        native.name_bytes = 0;
        let empty: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8256);
        native.name_bytes = 8194;
        let oversized: WindowsDirectoryStep = windows_directory_decode_entry(&native, 8256);
        native.name_bytes = 2;
        native.control = 1;
        let chained: WindowsDirectoryStep = windows_directory_decode_entry(&native, 66);
        transition error(short, 122) && error(long, 122) && error(payload, 31)
            && error(odd, 31) && error(empty, 31) && error(oversized, 31) && error(chained, 31) {
            true -> dots(native) false -> wrong()
        }
    }
    machine error(result: WindowsDirectoryStep, code: u32) -> bool {
        transition result { WindowsDirectoryStep::Error { code as found } -> (found == code) _ -> false }
    }
    machine dots(mut native: WindowsNativeDirectoryEntryInfo) crashes Trap {
        native.characters[0] = 46; native.characters[1] = 46;
        transition windows_directory_entry_dot(&native, 1) && windows_directory_entry_dot(&native, 2)
            && !windows_directory_entry_dot(&native, 3) && !windows_directory_entry_dot(&native, 0) {
            true -> dot_prefix(native) false -> wrong()
        }
    }
    machine dot_prefix(mut native: WindowsNativeDirectoryEntryInfo) crashes Trap {
        native.characters[1] = 120;
        transition !windows_directory_entry_dot(&native, 2) { true -> names() false -> wrong() }
    }
    machine names() crashes Trap {
        let mut bytes: [u8; 12288];
        fill(&mut bytes, 0);
        let legal: WindowsDirectoryEntryResult = windows_directory_encoded_entry(&bytes, 765, 16);
        transition legal { WindowsDirectoryEntryResult::Entry { entry } -> short_name(bytes, entry) _ -> wrong() }
    }
    machine fill(bytes: &mut [u8; 12288], position: u64) {
        transition position < 12288 { true -> put(bytes, position) false -> filled() }
        machine put(bytes: &mut [u8; 12288], position: u64) requires position < 12288 {
            // U+4E00: three UTF-8 bytes for each admitted UTF-16 unit.
            bytes[position] = octet(position % 3);
            transition { _ -> fill(bytes, position + 1) }
        }
        machine octet(remainder: u64) -> u8 {
            transition remainder { 0 -> 228 1 -> 184 _ -> 128 }
        }
        machine filled() {}
    }
    machine short_name(bytes: [u8; 12288], entry: WindowsDirectoryEntry) crashes Trap {
        transition entry.name_len == 765 && entry.attributes == 16 && same(&bytes, &entry.name, 765, 0) {
            true -> maximum(bytes) false -> wrong()
        }
    }
    machine maximum(bytes: [u8; 12288]) crashes Trap {
        let result: WindowsDirectoryEntryResult = windows_directory_encoded_entry(&bytes, 12288, 0);
        transition result { WindowsDirectoryEntryResult::Entry { entry } -> full_name(bytes, entry) _ -> wrong() }
    }
    machine full_name(bytes: [u8; 12288], entry: WindowsDirectoryEntry) crashes Trap {
        transition entry.name_len == 12288 && entry.attributes == 0 && same(&bytes, &entry.name, 12288, 0) {
            true -> overflow(bytes, entry) false -> wrong()
        }
    }
    machine same(bytes: &[u8; 12288], name: &[u8; 12288], count: u64, position: u64) -> bool
    requires count <= 12288 {
        transition position < count { true -> byte(bytes, name, count, position) false -> true }
        machine byte(bytes: &[u8; 12288], name: &[u8; 12288], count: u64, position: u64) -> bool
        requires count <= 12288 && position < count {
            transition bytes[position] == name[position] { true -> (same(bytes, name, count, position + 1)) false -> false }
        }
    }
    machine overflow(bytes: [u8; 12288], entry: WindowsDirectoryEntry) crashes Trap {
        let result: WindowsDirectoryEntryResult = windows_directory_encoded_entry(&bytes, 12289, 0);
        transition result { WindowsDirectoryEntryResult::Error { code } -> overflow_code(entry, code) _ -> wrong() }
    }
    machine overflow_code(entry: WindowsDirectoryEntry, code: u32) crashes Trap {
        transition code == 122 { true -> facade(entry) false -> wrong() }
    }
    machine facade(mut entry: WindowsDirectoryEntry) crashes Trap {
        entry.name_len = 765; entry.attributes = 16;
        let directory: DirEntryResult = windows_filesystem_directory_entry(entry);
        entry.attributes = 0;
        let file: DirEntryResult = windows_filesystem_directory_entry(entry);
        entry.attributes = 1040;
        let link: DirEntryResult = windows_filesystem_directory_entry(entry);
        entry.name_len = 12288;
        let maximum: DirEntryResult = windows_filesystem_directory_entry(entry);
        entry.name_len = 0;
        let empty: DirEntryResult = windows_filesystem_directory_entry(entry);
        entry.name_len = 12289;
        let oversized: DirEntryResult = windows_filesystem_directory_entry(entry);
        transition classified(directory, 765, true, false) && classified(file, 765, false, true)
            && classified(link, 765, false, false) && classified(maximum, 12288, false, false)
            && refused(empty) && refused(oversized) { true -> done() false -> wrong() }
    }
    machine classified(result: DirEntryResult, length: u64, directory: bool, file: bool) -> bool {
        transition result { DirEntryResult::Ok { entry } -> (entry.name_len == length
            && entry.is_dir == directory && entry.is_file == file
            && entry.name[0] == 228 && entry.name[764] == 128) _ -> false }
    }
    machine refused(result: DirEntryResult) -> bool {
        transition result { DirEntryResult::Error { kind as error_kind } -> is_other_kind(error_kind) _ -> false }
        machine is_other_kind(kind: ErrorKind) -> bool { transition kind { ErrorKind::Other -> true _ -> false } }
    }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--receipt", type=Path, required=True)
    options = parser.parse_args()
    if (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        raise SystemExit("This qualification needs a Linux x86-64 native host.")
    root, omega = options.repository.resolve(), options.omega.resolve()
    std = "source/library/std/"
    win = std + "targets/windows_x86_64/"
    declarations = [
        (std + "filesystem.omg", "public data ErrorKind ["),
        (std + "filesystem.omg", "public data DirEntry ["),
        (std + "filesystem.omg", "public data DirEntryResult {"),
        (std + "windows_filesystem_host.omg", "public data WindowsDirectoryEntry ["),
        (std + "windows_filesystem_host.omg", "public data WindowsDirectoryEntryResult {"),
        (win + "windows_filesystem_directory_native.omg", "data WindowsNativeDirectoryEntryInfo {"),
        (win + "windows_filesystem_directory.omg", "data WindowsDirectoryStep {"),
        *[(win + "windows_filesystem_directory_entry.omg", "machine " + name + "(") for name in (
            "windows_directory_decode_entry", "windows_directory_entry_dot", "windows_directory_encoded_entry",
            "windows_directory_copy_entry", "windows_directory_copy_name")],
        (win + "filesystem/directory.omg", "machine windows_filesystem_directory_entry("),
    ]
    parts = [declaration(root, path, prefix) for path, prefix in declarations]
    receipt = {"source_head": subprocess.check_output(
                   ["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip(),
               "producer_sha256": hashlib.sha256(omega.read_bytes()).hexdigest(),
               "production_declarations": [dict(path=path, prefix=prefix,
                   sha256=hashlib.sha256(part.encode()).hexdigest())
                   for (path, prefix), part in zip(declarations, parts)], "observations": []}
    environment = {**os.environ, "OMEGA_LIBRARY_ROOT": str(root / "source/library")}
    with tempfile.TemporaryDirectory(prefix="omega-directory-names-") as temporary:
        project = Path(temporary)
        (project / "production.omg").write_text("use omega::language::core::widen_u32_to_u64;\n"
            "use omega::language::core::narrow_i32_to_u64_exact;\n" + "\n".join(parts))
        (project / "main.omg").write_text(PROGRAM)
        (project / "build.omg").write_text('machine build(builder: &mut Build) { '
            'Build::application(builder, "directory_names"); '
            'Build::bind(builder, linux_x86_64::ProgramEntry, program); '
            'Build::bind(builder, linux_arm64::ProgramEntry, program); '
            'Build::bind(builder, macos_arm64::ProgramEntry, program); '
            'Build::bind(builder, windows_x86_64::ProgramEntry, program); }\n')
        for arguments in (("packages", "update", "--project", project, "--dangerously-accept-all"),
                          ("run", "--both", project / "main.omg")):
            observed = subprocess.run([str(omega), *map(str, arguments)], env=environment,
                stdin=subprocess.DEVNULL, capture_output=True, timeout=240)
            row = dict(arguments=list(map(str, arguments)), exit=observed.returncode,
                       stdout=observed.stdout.decode(errors="replace"), stderr=observed.stderr.decode(errors="replace"))
            receipt["observations"].append(row)
            options.receipt.write_text(json.dumps(receipt, indent=2) + "\n")
            assert observed.returncode == 0, row
        assert b"native exit 0\n" in observed.stderr and b"interp exit: 0\n" in observed.stderr, row
    print("directory record, bounded copy and portable entry: native 0, Psi 0")


if __name__ == "__main__":
    main()

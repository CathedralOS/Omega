//! Ordered bootstrap source closure.
//!
//! `mbx xtask source-closure MANIFEST OUTPUT [--prefix ENTRY]` checks every
//! member row and the manifest directory's source inventory, then writes the
//! concatenated bytes atomically. A refusal leaves an existing output in place.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const HEADERS: &[(&str, &str)] = &[
    ("GammaSourceClosureV1", ".gamma"),
    ("DeltaSourceClosureV1", ".delta"),
    ("EpsilonSourceClosureV1", ".epsilon"),
];
const SOURCE_SUFFIXES: &[&str] = &[".beta", ".gamma", ".delta", ".epsilon", ".omg"];

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    if let Err(message) = execute(args) {
        eprintln!("{message}");
        return Err(1);
    }
    Ok(())
}

fn execute(args: &[String]) -> Result<(), String> {
    let (manifest, output, prefix) = match args {
        [manifest, output] => (manifest, output, None),
        [manifest, output, flag, prefix] if flag == "--prefix" => (manifest, output, Some(prefix)),
        _ => {
            return Err(refuse(
                "usage: mbx xtask source-closure MANIFEST OUTPUT [--prefix ENTRY]",
            ));
        }
    };
    publish(
        Path::new(manifest),
        Path::new(output),
        prefix.map(Path::new),
    )
}

fn publish(manifest: &Path, output: &Path, prefix: Option<&Path>) -> Result<(), String> {
    ensure_regular(manifest, "manifest")?;
    let (body, suffix) = materialize(manifest)?;
    let packed = match prefix {
        None => body,
        Some(prefix) => {
            if path_suffix(prefix) != suffix {
                return Err(refuse(format!(
                    "prefix suffix is not {suffix}: {}",
                    prefix.display()
                )));
            }
            ensure_regular(prefix, "prefix")?;
            let mut combined = source_bytes(prefix, &prefix.display().to_string())?;
            combined.extend(body);
            combined
        }
    };
    write_atomic(output, &packed)
}

fn materialize(manifest: &Path) -> Result<(Vec<u8>, String), String> {
    let manifest_bytes = fs::read(manifest).map_err(refuse)?;
    if !manifest_bytes.is_ascii() {
        return Err(refuse("manifest is not ASCII"));
    }
    if manifest_bytes
        .iter()
        .any(|byte| !matches!(byte, 9 | 10 | 13 | 32..=126))
    {
        return Err(refuse("manifest contains a forbidden byte"));
    }
    let text = String::from_utf8(manifest_bytes).map_err(refuse)?;
    let lines = split_lines(&text);
    let header = lines.first().copied().unwrap_or("");
    let Some(suffix) = HEADERS
        .iter()
        .find(|(name, _)| *name == header)
        .map(|(_, suffix)| (*suffix).to_owned())
    else {
        let names = HEADERS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(refuse(format!("manifest header must be one of: {names}")));
    };
    if lines.len() == 1 {
        return Err(refuse("manifest has no members"));
    }

    let mut packed = Vec::new();
    let mut prior_identity: Option<&str> = None;
    let mut paths = BTreeSet::new();
    for (line_number, line) in lines.iter().copied().enumerate().skip(1) {
        let line_number = line_number + 1;
        let fields: Vec<&str> = line.split(' ').collect();
        if fields.len() != 5 || fields[0] != "member" || fields.iter().any(|field| field.is_empty())
        {
            return Err(refuse(format!(
                "malformed member row at line {line_number}"
            )));
        }
        let identity = fields[1];
        let length_text = fields[2];
        let expected_digest = fields[3];
        let spelling = fields[4];
        if !is_identity(identity) {
            return Err(refuse(format!(
                "invalid member identity at line {line_number}"
            )));
        }
        if prior_identity.is_some_and(|prior| identity <= prior) {
            return Err(refuse(format!(
                "member identities are not in strict order at line {line_number}"
            )));
        }
        prior_identity = Some(identity);
        if !is_decimal(length_text) {
            return Err(refuse(format!(
                "noncanonical member length at line {line_number}"
            )));
        }
        if !is_identity(expected_digest) {
            return Err(refuse(format!(
                "invalid member digest at line {line_number}"
            )));
        }
        if !paths.insert(spelling.to_owned()) {
            return Err(refuse(format!(
                "duplicate member path at line {line_number}"
            )));
        }
        let path = member_path(parent_dir(manifest), spelling, &suffix)?;
        let data = source_bytes(&path, spelling)?;
        if data.len().to_string() != length_text {
            return Err(refuse(format!(
                "member length changed for {spelling}: expected {length_text}, found {}",
                data.len()
            )));
        }
        let digest = sha256_hex(&data);
        if digest != expected_digest {
            return Err(refuse(format!(
                "member digest changed for {spelling}: expected {expected_digest}, found {digest}"
            )));
        }
        packed.extend(data);
    }
    check_inventory(parent_dir(manifest), &paths)?;
    Ok((packed, suffix))
}

fn member_path(root: &Path, spelling: &str, suffix: &str) -> Result<PathBuf, String> {
    let (display, absolute, parts) = pure_posix(spelling);
    if display != spelling
        || absolute
        || parts.is_empty()
        || parts
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err(refuse(format!("noncanonical member path: {spelling}")));
    }
    if spelling
        .bytes()
        .any(|byte| byte == b'\\' || byte == b':' || !(33..=126).contains(&byte))
    {
        return Err(refuse(format!("noncanonical member path: {spelling}")));
    }
    if path_suffix(Path::new(spelling)) != suffix {
        return Err(refuse(format!("member suffix is not {suffix}: {spelling}")));
    }
    let mut candidate = root.to_path_buf();
    for part in &parts {
        candidate.push(part);
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(refuse(format!(
                    "member path traverses a symbolic link: {spelling}"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(refuse(error)),
        }
    }
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.is_file() => Ok(candidate),
        Ok(_) | Err(_) => Err(refuse(format!("member is not a regular file: {spelling}"))),
    }
}

fn check_inventory(root: &Path, members: &BTreeSet<String>) -> Result<(), String> {
    let found = inventory(root)?;
    if &found == members {
        return Ok(());
    }
    let difference: Vec<&String> = found.symmetric_difference(members).collect();
    Err(refuse(format!(
        "source inventory differs from manifest: {}",
        python_list(difference)
    )))
}

fn inventory(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).map_err(refuse)? {
            entries.push(entry.map_err(refuse)?);
        }
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(refuse)?;
            let symlink = metadata.file_type().is_symlink();
            let is_directory = if symlink {
                fs::metadata(&path).is_ok_and(|followed| followed.is_dir())
            } else {
                metadata.is_dir()
            };
            if is_directory {
                if symlink {
                    return Err(refuse(format!(
                        "source inventory traverses a symbolic link: {}",
                        entry.file_name().to_string_lossy()
                    )));
                }
                directories.push(path);
                continue;
            }
            if SOURCE_SUFFIXES.contains(&path_suffix(&path).as_str()) {
                found.insert(posix_relative(root, &path));
            }
        }
    }
    Ok(found)
}

fn source_bytes(path: &Path, spelling: &str) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(refuse)?;
    if let Some(offset) = bytes
        .iter()
        .position(|byte| !matches!(byte, 9 | 10 | 13 | 32..=126))
    {
        return Err(refuse(format!(
            "forbidden byte in {spelling} at offset {offset}"
        )));
    }
    Ok(bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = parent_dir(path);
    fs::create_dir_all(parent).map_err(refuse)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "closure".to_owned());
    let mut attempts = 0u32;
    let temporary = loop {
        attempts += 1;
        if attempts > 100 {
            return Err(refuse("could not create a temporary closure file"));
        }
        let candidate = parent.join(format!(".{file_name}.{}.{attempts}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    let _ = fs::remove_file(&candidate);
                    return Err(refuse(error));
                }
                break candidate;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(refuse(error)),
        }
    };
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(refuse(error));
    }
    Ok(())
}

fn ensure_regular(path: &Path, label: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => Ok(()),
        Ok(_) => Err(refuse(format!("{label} is not a regular file"))),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Err(refuse(format!("{label} is not a regular file")))
        }
        Err(error) => Err(refuse(error)),
    }
}

fn pure_posix(spelling: &str) -> (String, bool, Vec<&str>) {
    let absolute = spelling.starts_with('/');
    let parts: Vec<&str> = spelling
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    let display = if absolute {
        format!("/{}", parts.join("/"))
    } else if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    };
    (display, absolute, parts)
}

fn path_suffix(path: &Path) -> String {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return String::new();
    };
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            format!(".{extension}")
        }
        _ => String::new(),
    }
}

fn posix_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(index) = rest.find(['\n', '\r']) {
            lines.push(&rest[..index]);
            if rest[index..].starts_with("\r\n") {
                rest = &rest[index + 2..];
            } else {
                rest = &rest[index + 1..];
            }
        } else {
            lines.push(rest);
            break;
        }
    }
    lines
}

fn is_identity(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_decimal(text: &str) -> bool {
    let mut bytes = text.bytes();
    match bytes.next() {
        Some(b'0') => text.len() == 1,
        Some(b'1'..=b'9') => bytes.all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0xf) as usize] as char);
    }
    hex
}

fn python_list(items: Vec<&String>) -> String {
    let quoted = items
        .into_iter()
        .map(|item| format!("'{item}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{quoted}]")
}

fn refuse(message: impl std::fmt::Display) -> String {
    format!("source closure: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-closure-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct Sample {
        manifest: PathBuf,
        first: PathBuf,
        first_bytes: Vec<u8>,
        second_bytes: Vec<u8>,
    }

    fn write_sample(directory: &Path, header: &str) -> Sample {
        let suffix = HEADERS.iter().find(|(name, _)| *name == header).unwrap().1;
        let closure = directory.join("closure");
        let first = closure.join(format!("parts/first{suffix}"));
        fs::create_dir_all(first.parent().unwrap()).unwrap();
        let first_bytes = b"; first\r\n".to_vec();
        fs::write(&first, &first_bytes).unwrap();
        let second = closure.join(format!("second{suffix}"));
        let second_bytes = b"\t; second\n".to_vec();
        fs::write(&second, &second_bytes).unwrap();
        let manifest = closure.join("compiler.sources");
        write_manifest(
            &manifest,
            header,
            &[
                member_row(1, "parts/first", &first_bytes, suffix),
                member_row(2, "second", &second_bytes, suffix),
            ],
        );
        Sample {
            manifest,
            first,
            first_bytes,
            second_bytes,
        }
    }

    fn member_row(identity: u64, spelling: &str, bytes: &[u8], suffix: &str) -> String {
        format!(
            "member {:064x} {} {} {spelling}{suffix}",
            identity,
            bytes.len(),
            sha256_hex(bytes)
        )
    }

    fn write_manifest(path: &Path, header: &str, rows: &[String]) {
        let mut text = String::from(header);
        text.push('\n');
        for row in rows {
            text.push_str(row);
            text.push('\n');
        }
        fs::write(path, text).unwrap();
    }

    fn preserved(output: &Path, existing: bool) {
        if existing {
            assert_eq!(fs::read(output).unwrap(), b"keep prior output");
        } else {
            assert!(!output.exists());
        }
    }

    #[test]
    fn exact_order_follows_the_manifest_not_the_paths() {
        for (header, _) in HEADERS {
            let fixture = Fixture::new();
            let sample = write_sample(&fixture.0, header);
            let suffix = path_suffix(&sample.first);
            write_manifest(
                &sample.manifest,
                header,
                &[
                    member_row(1, "second", &sample.second_bytes, &suffix),
                    member_row(2, "parts/first", &sample.first_bytes, &suffix),
                ],
            );
            let output = fixture.0.join("output");
            publish(&sample.manifest, &output, None).unwrap();
            let mut expected = sample.second_bytes.clone();
            expected.extend(sample.first_bytes);
            assert_eq!(fs::read(output).unwrap(), expected);
        }
    }

    #[test]
    fn prefix_is_prepended_and_refusals_keep_the_previous_output() {
        let fixture = Fixture::new();
        let sample = write_sample(&fixture.0, "GammaSourceClosureV1");
        let prefix = fixture.0.join("entry.gamma");
        let prefix_bytes = b"; exact entry\r\n\t".to_vec();
        fs::write(&prefix, &prefix_bytes).unwrap();
        let output = fixture.0.join("output");
        publish(&sample.manifest, &output, Some(&prefix)).unwrap();
        let mut expected = prefix_bytes;
        expected.extend(&sample.first_bytes);
        expected.extend(&sample.second_bytes);
        assert_eq!(fs::read(&output).unwrap(), expected);

        let cases: &[(&str, fn(&Sample, &Path))] = &[
            ("missing", |_, prefix| {
                fs::remove_file(prefix).unwrap();
            }),
            ("control", |_, prefix| fs::write(prefix, [0u8]).unwrap()),
            ("nonascii", |_, prefix| fs::write(prefix, [0x80u8]).unwrap()),
            ("stale", |sample, _| {
                fs::write(&sample.first, b"changed").unwrap();
            }),
            ("wrong-suffix", |_, prefix| {
                fs::remove_file(prefix).unwrap();
                fs::write(prefix.with_extension("delta"), b"; wrong\n").unwrap();
            }),
        ];
        for (name, alter) in cases {
            let fixture = Fixture::new();
            let sample = write_sample(&fixture.0, "GammaSourceClosureV1");
            let prefix = fixture.0.join("entry.gamma");
            fs::write(&prefix, b"; exact entry\r\n\t").unwrap();
            let prefix_arg = if *name == "wrong-suffix" {
                fixture.0.join("entry.delta")
            } else {
                prefix.clone()
            };
            alter(&sample, &prefix);
            for existing in [false, true] {
                let output = fixture.0.join("output");
                if existing {
                    fs::write(&output, b"keep prior output").unwrap();
                } else if output.exists() {
                    fs::remove_file(&output).unwrap();
                }
                let error = publish(&sample.manifest, &output, Some(&prefix_arg)).unwrap_err();
                assert!(error.starts_with("source closure:"), "{name}: {error}");
                preserved(&output, existing);
            }
        }
    }

    #[test]
    fn malformed_manifests_and_inventories_refuse_before_replacing_output() {
        let output_name = "output";
        let cases = [
            "header",
            "empty",
            "malformed",
            "reordered",
            "duplicate-identity",
            "duplicate-path",
            "length-spelling",
            "length",
            "digest",
            "missing",
            "extra",
            "foreign",
            "wrong-language",
            "dot",
            "double-slash",
            "inner-dot",
            "parent",
            "absolute",
            "backslash",
            "drive",
            "source-byte",
            "manifest-ascii",
            "manifest-byte",
        ];
        for case in cases {
            let fixture = Fixture::new();
            let sample = write_sample(&fixture.0, "DeltaSourceClosureV1");
            let spelling = "parts/first.delta";
            let mut rows = vec![
                member_row(1, "parts/first", &sample.first_bytes, ".delta"),
                member_row(2, "second", &sample.second_bytes, ".delta"),
            ];
            let mut header = "DeltaSourceClosureV1";
            match case {
                "header" => header = "UnknownSourceClosureV1",
                "empty" => rows.clear(),
                "malformed" => rows[0] = "member broken".to_owned(),
                "reordered" => rows.reverse(),
                "duplicate-identity" => {
                    rows[1] = rows[1].replacen(&format!("{:064x}", 2), &format!("{:064x}", 1), 1);
                }
                "duplicate-path" => {
                    rows[1] = member_row(2, "parts/first", &sample.first_bytes, ".delta");
                }
                "length-spelling" => rows[0] = rows[0].replacen(" 9 ", " 09 ", 1),
                "length" => rows[0] = rows[0].replacen(" 9 ", " 8 ", 1),
                "digest" => {
                    rows[0] = member_row(1, "parts/first", b"different", ".delta");
                }
                "missing" => fs::remove_file(&sample.first).unwrap(),
                "extra" => fs::write(
                    sample.manifest.parent().unwrap().join("unlisted.delta"),
                    b"",
                )
                .unwrap(),
                "foreign" => {
                    fs::write(
                        sample.manifest.parent().unwrap().join("unlisted.gamma"),
                        b"",
                    )
                    .unwrap();
                }
                "wrong-language" => {
                    let foreign = sample.first.with_extension("gamma");
                    fs::rename(&sample.first, &foreign).unwrap();
                    let bytes = fs::read(&foreign).unwrap();
                    rows[0] = member_row(1, "parts/first", &bytes, ".gamma");
                }
                "dot" => rows[0] = rows[0].replace(spelling, &format!("./{spelling}")),
                "double-slash" => rows[0] = rows[0].replace(spelling, "parts//first.delta"),
                "inner-dot" => rows[0] = rows[0].replace(spelling, "parts/./first.delta"),
                "parent" => rows[0] = rows[0].replace(spelling, &format!("../{spelling}")),
                "absolute" => {
                    rows[0] = rows[0].replace(spelling, &sample.first.display().to_string())
                }
                "backslash" => rows[0] = rows[0].replace(spelling, "parts\\first.delta"),
                "drive" => rows[0] = rows[0].replace(spelling, &format!("C:/{spelling}")),
                "source-byte" => {
                    fs::write(&sample.first, [0x01u8]).unwrap();
                    rows[0] = member_row(1, "parts/first", &[0x01], ".delta");
                }
                "manifest-ascii" | "manifest-byte" => {}
                _ => unreachable!("{case}"),
            }
            write_manifest(&sample.manifest, header, &rows);
            if case == "manifest-ascii" {
                let mut bytes = fs::read(&sample.manifest).unwrap();
                bytes.push(0xff);
                fs::write(&sample.manifest, bytes).unwrap();
            } else if case == "manifest-byte" {
                let mut bytes = fs::read(&sample.manifest).unwrap();
                bytes.push(0x0b);
                fs::write(&sample.manifest, bytes).unwrap();
            }
            let output = fixture.0.join(output_name);
            fs::write(&output, b"keep prior output").unwrap();
            let error = publish(&sample.manifest, &output, None).unwrap_err();
            assert!(error.starts_with("source closure:"), "{case}: {error}");
            assert_eq!(fs::read(&output).unwrap(), b"keep prior output", "{case}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_refuse_before_replacing_output() {
        let fixture = Fixture::new();
        let sample = write_sample(&fixture.0, "EpsilonSourceClosureV1");
        let output = fixture.0.join("output");
        fs::write(&output, b"keep prior output").unwrap();
        let moved = fixture.0.join("moved-file");
        fs::rename(&sample.first, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &sample.first).unwrap();
        let error = publish(&sample.manifest, &output, None).unwrap_err();
        assert!(error.contains("symbolic link"), "{error}");
        assert_eq!(fs::read(&output).unwrap(), b"keep prior output");

        let fixture = Fixture::new();
        let sample = write_sample(&fixture.0, "EpsilonSourceClosureV1");
        let output = fixture.0.join("output");
        let moved = fixture.0.join("moved-dir");
        fs::rename(sample.first.parent().unwrap(), &moved).unwrap();
        std::os::unix::fs::symlink(&moved, sample.first.parent().unwrap()).unwrap();
        let error = publish(&sample.manifest, &output, None).unwrap_err();
        assert!(error.contains("symbolic link"), "{error}");
        assert!(!output.exists());

        let fixture = Fixture::new();
        let sample = write_sample(&fixture.0, "EpsilonSourceClosureV1");
        let output = fixture.0.join("output");
        fs::write(&output, b"keep prior output").unwrap();
        let moved = fixture.0.join("manifest-real");
        fs::rename(&sample.manifest, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &sample.manifest).unwrap();
        let error = publish(&sample.manifest, &output, None).unwrap_err();
        assert!(error.contains("manifest is not a regular file"), "{error}");
        assert_eq!(fs::read(&output).unwrap(), b"keep prior output");
    }

    #[test]
    fn selected_compiler_closures_match_their_pinned_bytes() {
        let root = crate::repo_root();
        let cases = [
            (
                "tests/bootstrap/epsilon-source-closure/fixture.sources",
                89usize,
                "528f65b2e2d9666db1c1f3930c9f5784bbfc1497e3b7225b26cb3eee34d2924c",
                None,
            ),
            (
                "bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources",
                818946,
                "30f8b6de9e15312b4445c2194cbdb857bf5b2e27f08caa59281c5b0170fc088f",
                None,
            ),
            (
                "bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources",
                617354,
                "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e",
                None,
            ),
            (
                "bootstrap/interpreted/3_delta/implementation/implementation.gamma.sources",
                151525,
                "de94946dd946c90c3311dba52aa4622a3c80a18866d5483531de1312cc62e0c6",
                Some("bootstrap/interpreted/3_delta/delta_compiler.gamma"),
            ),
            (
                "bootstrap/interpreted/3_delta/implementation/implementation.gamma.sources",
                151292,
                "4e82f36aa94a5cad71297687e2643982d88942bda4750c877fe8d633308fb720",
                Some("tests/delta/staged-compiler/development_driver.gamma"),
            ),
            (
                "bootstrap/interpreted/3_delta/support/support.gamma.sources",
                2998,
                "cfdf07cf8010eba2fd7da47e6936ea1e237f637f4ded5791c272e03096d70255",
                None,
            ),
            (
                "bootstrap/proofs/checker/implementation/implementation.gamma.sources",
                62357,
                "382531b910429c7111c2bc9e7dcce7e96394c45fa0a89b624debd061b69422b2",
                None,
            ),
            (
                "bootstrap/proofs/beta_encoding/theory/theory.gamma.sources",
                130363,
                "632871b5c22c22a0ba397ad4dc6054af797a364f789570e6b25be08840b41ddf",
                None,
            ),
        ];
        let fixture = Fixture::new();
        for (manifest, length, digest, prefix) in cases {
            let output = fixture.0.join("output");
            let prefix = prefix.map(|path| root.join(path));
            publish(&root.join(manifest), &output, prefix.as_deref())
                .unwrap_or_else(|error| panic!("{manifest}: {error}"));
            let bytes = fs::read(&output).unwrap();
            assert_eq!(bytes.len(), length, "{manifest}");
            assert_eq!(sha256_hex(&bytes), digest, "{manifest}");
        }
    }
}

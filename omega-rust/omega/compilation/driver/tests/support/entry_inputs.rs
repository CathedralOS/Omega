//! Authored invocation inputs and original-backing oracles for corpus fixtures.
//! These bytes request construction; the public native pipeline still admits
//! each exact row, type, placement and initialized field before publication.

use std::path::Path;

fn rows(path: &Path) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (position, contents) = line
            .trim()
            .split_once('=')
            .ok_or("entry RAM row needs position=hex-bytes")?;
        let position: u32 = position
            .parse()
            .map_err(|_| "entry RAM position needs u32")?;
        if rows.iter().any(|(prior, _)| *prior == position)
            || contents.len() % 2 != 0
            || !contents.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("entry RAM row has duplicate position or invalid hex bytes".into());
        }
        let bytes = contents
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).expect("ASCII checked"), 16)
                    .expect("hex digits checked")
            })
            .collect();
        rows.push((position, bytes));
    }
    Ok(rows)
}

pub(super) fn load(
    root: &Path,
) -> Result<Vec<compilation_products::PrivateEntryRamInitialization>, Vec<diagnostics::Diagnostic>>
{
    rows(&root.parent().unwrap_or(root).join("entry_ram.txt"))
        .map(|rows| {
            rows.into_iter()
                .map(
                    |(position, contents)| compilation_products::PrivateEntryRamInitialization {
                        position,
                        contents,
                    },
                )
                .collect()
        })
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
}

pub(super) fn observe(
    directory: &Path,
    prefix: &str,
    observations: &[(u32, Vec<u8>)],
) -> Vec<String> {
    if !directory.join("expected_entry_ram.txt").exists() {
        return Vec::new();
    }
    let matches = rows(&directory.join("expected_entry_ram.txt")).is_ok_and(|expected| {
        !expected.is_empty()
            && expected.len() == observations.len()
            && expected.iter().all(|row| observations.contains(row))
    });
    vec![format!(
        "{prefix}entry-ram:{}",
        if matches { "match" } else { "differs" }
    )]
}

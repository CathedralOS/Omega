//! Canonical relative paths and symlink targets: the portable component
//! rules every retained entry must satisfy.

use crate::staged_output_tree::{MAX_STAGED_OUTPUT_PATH_BYTES, diagnostics};
use diagnostics::Diagnostic;
use std::path::{Component, Path};

pub(crate) fn canonical_relative_path(
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut output = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(diagnostics(format!(
                "build staged-output entry `{}` has a non-canonical relative path",
                diagnostic_path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            diagnostics(format!(
                "build staged-output entry `{}` has a non-UTF-8 path component",
                diagnostic_path.display()
            ))
        })?;
        validate_portable_component(component.as_bytes(), diagnostic_path)?;
        if !output.is_empty() {
            output.push(b'/');
        }
        output.extend_from_slice(component.as_bytes());
    }
    if output.is_empty() {
        return Err(diagnostics("build staged-output entry has an empty path"));
    }
    Ok(output)
}

pub(crate) fn canonical_symlink_target(
    target: &Path,
    link_relative_path: &[u8],
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let target = target.to_str().ok_or_else(|| {
        diagnostics(format!(
            "build staged-output symlink `{}` has a non-UTF-8 target",
            diagnostic_path.display()
        ))
    })?;
    let bytes = target.as_bytes();
    if bytes.is_empty() || bytes.starts_with(b"/") || bytes.contains(&b'\\') || bytes.contains(&0) {
        return Err(diagnostics(format!(
            "build staged-output symlink `{}` must have a nonempty relative slash-separated target",
            diagnostic_path.display()
        )));
    }
    let mut resolved_depth = link_relative_path.split(|byte| *byte == b'/').count() - 1;
    for component in bytes.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            return Err(diagnostics(format!(
                "build staged-output symlink `{}` has a non-canonical target",
                diagnostic_path.display()
            )));
        }
        if component == b".." {
            if resolved_depth == 0 {
                return Err(diagnostics(format!(
                    "build staged-output symlink `{}` escapes the Output root",
                    diagnostic_path.display()
                )));
            }
            resolved_depth -= 1;
        } else {
            validate_portable_component(component, diagnostic_path)?;
            resolved_depth += 1;
        }
    }
    Ok(bytes.to_vec())
}

pub(crate) fn validate_portable_component(
    component: &[u8],
    diagnostic_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    if component.is_empty()
        || component == b"."
        || component == b".."
        || component.iter().any(|byte| {
            *byte < 0x20
                || matches!(
                    *byte,
                    b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
                )
        })
        || matches!(component.last(), Some(b'.' | b' '))
    {
        return Err(diagnostics(format!(
            "build staged-output entry `{}` has a non-portable path component",
            diagnostic_path.display()
        )));
    }
    let stem = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let reserved_device = [b"CON".as_slice(), b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && matches!(stem[3], b'1'..=b'9'))
        || stem.eq_ignore_ascii_case(b"CONIN$")
        || stem.eq_ignore_ascii_case(b"CONOUT$");
    if reserved_device {
        return Err(diagnostics(format!(
            "build staged-output entry `{}` uses a reserved portable device name",
            diagnostic_path.display()
        )));
    }
    Ok(())
}

pub(crate) fn reserve_path_bytes(
    current: usize,
    additional: usize,
) -> Result<usize, Vec<Diagnostic>> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_STAGED_OUTPUT_PATH_BYTES)
        .ok_or_else(|| {
            diagnostics(format!(
                "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_PATH_BYTES}-byte path and symlink-target ceiling"
            ))
        })
}

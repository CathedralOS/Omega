//! Validate Cargo's effective compiler codegen profile before producing a binary.

use std::io;

pub fn optimization_level() -> io::Result<String> {
    let mut optimization = std::env::var("OPT_LEVEL").map_err(io::Error::other)?;
    let flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    if flags.split('\u{1f}').any(|option| option.starts_with('@')) {
        return Err(io::Error::other(
            "cannot verify compiler optimization through a rustflags response file",
        ));
    }
    let mut options = flags.split('\u{1f}');
    while let Some(option) = options.next() {
        let codegen = match option {
            "-C" | "--codegen" => options.next(),
            _ => option
                .strip_prefix("-C")
                .or_else(|| option.strip_prefix("--codegen=")),
        };
        if let Some(level) = codegen.and_then(|value| value.strip_prefix("opt-level=")) {
            optimization = level.to_owned();
        }
    }
    if !matches!(optimization.as_str(), "1" | "2" | "3" | "s" | "z") {
        return Err(io::Error::other(format!(
            "Omega requires an optimized compiler; effective opt-level={optimization}. \
             Remove opt-level=0 profile/rustflags overrides and rebuild. \
             Debug information is available with CARGO_PROFILE_DEV_DEBUG=2."
        )));
    }
    Ok(optimization)
}

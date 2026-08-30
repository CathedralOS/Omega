//! Construct the compiler-owned bounded Git fetch invocation.

use crate::limits::LocalSourceLimits;
use std::ffi::OsString;

pub(crate) fn bounded_git_fetch_arguments(
    fetch_locator: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Vec<OsString> {
    let first_inadmissible_blob_size = limits
        .max_bytes
        .checked_add(1)
        .expect("compiler-owned Git source byte ceiling leaves room for one sentinel byte");
    vec![
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--depth=1"),
        OsString::from("--no-tags"),
        OsString::from("--no-recurse-submodules"),
        OsString::from(format!(
            "--filter=blob:limit={first_inadmissible_blob_size}"
        )),
        OsString::from("--"),
        OsString::from(fetch_locator),
        OsString::from(requested_rev),
    ]
}

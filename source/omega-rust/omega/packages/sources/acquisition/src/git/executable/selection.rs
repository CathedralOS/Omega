//! Compiler-selected primary system Git candidates.

#[cfg(target_os = "macos")]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &[
        "/Library/Developer/CommandLineTools/usr/bin/git",
        "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &["/usr/bin/git", "/usr/local/bin/git"]
}

#[cfg(windows)]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &[
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        r"C:\Program Files (x86)\Git\cmd\git.exe",
    ]
}

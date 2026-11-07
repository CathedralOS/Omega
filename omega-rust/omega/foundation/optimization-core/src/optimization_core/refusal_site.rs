// `CompilerCheckSite`: `#[track_caller]` source location of an internal
// compiler custody or shape check.
//
// We retain the Rust file and line as data on stage refusal variants so two
// distinct internal checks compare unequal and developers inspecting an error
// value can read `site.location()`, while `Debug` and `Display` withhold the
// compiler source path unless `OMEGA_COMPILER_CHECK_SITES` is set in the
// environment.

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CompilerCheckSite {
    location: &'static std::panic::Location<'static>, // Rust source location of the refusing check.
}

impl CompilerCheckSite {
    #[must_use]
    #[track_caller]
    pub fn caller() -> Self {
        Self {
            location: std::panic::Location::caller(),
        }
    }

    #[must_use]
    pub const fn location(self) -> &'static std::panic::Location<'static> {
        self.location
    }
}

fn sites_requested() -> bool {
    std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
}

impl std::fmt::Debug for CompilerCheckSite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if sites_requested() {
            return write!(
                formatter,
                "{}:{}",
                self.location.file(),
                self.location.line()
            );
        }
        formatter.write_str("withheld; set OMEGA_COMPILER_CHECK_SITES to reveal it")
    }
}

impl std::fmt::Display for CompilerCheckSite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, formatter)
    }
}

#[cfg(test)]
mod tests;

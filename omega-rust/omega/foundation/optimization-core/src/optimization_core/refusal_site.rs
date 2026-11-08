//! Optimizer module role: producer-side explanation. Where in the compiler's
//! own source an internal check refused.
//!
//! A stage whose hundreds of internal custody and shape checks share one
//! refusal variant cannot say which check fired from the variant alone, so
//! each such stage keeps one `#[track_caller]` constructor and retains the
//! Rust file and line of the check. That location is a fact about the
//! compiler, not about the author's program: a refusal rendered as
//! `SourceCustodyMismatch { site: Location { file:
//! "omega-rust/omega/compilation/pipeline/03_.../aggregate_return.rs", line:
//! 41 } }` hands an Omega author a compiler implementation path and no
//! authored coordinate at all.
//!
//! This carrier keeps the location as data and withholds it from rendering.
//! `Debug` and `Display` name the file and line only when a developer opens
//! the channel with `OMEGA_COMPILER_CHECK_SITES`, the same opt-in observation
//! convention as `OMEGA_STRUCT_TRACE`; otherwise they print the hint and
//! nothing else, because every stage error that carries a site renders through
//! `Debug`. The refusal an author reads names the authored coordinates its own
//! stage attached — machine and operation — and a developer holding the error
//! value reads [`CompilerCheckSite::location`] directly.
//!
//! The site carries no trust and enters no canonical record. Refusals raised
//! at two different checks stay unequal, so retaining the site keeps refusal
//! identity exactly as precise as printing it did.

/// The compiler source location of the internal check that refused.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CompilerCheckSite {
    location: &'static std::panic::Location<'static>,
}

impl CompilerCheckSite {
    /// The caller of the stage refusal constructor that built this site.
    ///
    /// Every caller in the chain is `#[track_caller]` up to the stage's one
    /// refusal constructor, so this names the refusing check rather than the
    /// constructor that spells the variant.
    #[must_use]
    #[track_caller]
    pub fn caller() -> Self {
        Self {
            location: std::panic::Location::caller(),
        }
    }

    /// The retained location, for a developer channel that has decided a
    /// compiler path belongs in its output.
    #[must_use]
    pub const fn location(self) -> &'static std::panic::Location<'static> {
        self.location
    }
}

/// Whether a developer asked for compiler check sites in rendered refusals.
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

//! The four outcomes of a compiler request and their specified halt values.

use diagnostics::{Diagnostic, ExceededProvision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerRequestOutcome {
    Complete,
    Reject,
    Incomplete(ExceededProvision),
    /// The compiler detected a defect in itself, such as its own stage output
    /// failing an independent validator. It says nothing about the source and
    /// grants no artifact authority.
    InternalFailure,
}

impl CompilerRequestOutcome {
    pub fn of_refusal(diagnostics: &[Diagnostic]) -> Self {
        let mut errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.is_error());
        let Some(first) = errors.next().and_then(|error| error.exceeded_provision) else {
            return Self::Reject;
        };
        if errors.all(|error| error.exceeded_provision.is_some()) {
            Self::Incomplete(first)
        } else {
            Self::Reject
        }
    }

    pub const fn halt_value(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::Reject => 1,
            Self::Incomplete(_) => 2,
            Self::InternalFailure => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diagnostics::CoverageForm;

    #[test]
    fn bounded_refusal_retains_the_first_provision_among_errors() {
        let first = CoverageForm::EntrySignature.exceeded(13);
        let diagnostics = [
            Diagnostic::warning("unused declaration"),
            Diagnostic::incomplete("first uncovered entry", first),
            Diagnostic::note("while checking the entry"),
            Diagnostic::incomplete("uncovered call", CoverageForm::CallForm.exceeded(23)),
        ];
        assert_eq!(
            CompilerRequestOutcome::of_refusal(&diagnostics),
            CompilerRequestOutcome::Incomplete(first)
        );
    }

    #[test]
    fn a_definitive_error_or_absence_of_errors_rejects() {
        let error = Diagnostic::error("invalid entry");
        let limit =
            Diagnostic::incomplete("uncovered entry", CoverageForm::EntrySignature.exceeded(13));
        for diagnostics in [
            vec![error.clone(), limit.clone()],
            vec![limit, error],
            vec![Diagnostic::warning("unused declaration")],
            vec![],
        ] {
            assert_eq!(
                CompilerRequestOutcome::of_refusal(&diagnostics),
                CompilerRequestOutcome::Reject
            );
        }
    }

    #[test]
    fn compiler_halt_values_follow_the_request_contract() {
        use CompilerRequestOutcome::*;
        let provision = CoverageForm::EntrySignature.exceeded(13);
        assert_eq!(
            [Complete, Reject, Incomplete(provision), InternalFailure]
                .map(CompilerRequestOutcome::halt_value),
            [0, 1, 2, 3]
        );
    }
}

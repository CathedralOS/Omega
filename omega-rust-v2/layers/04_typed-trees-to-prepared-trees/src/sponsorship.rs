//! L4 can admit a bounded preparation using a certified ceiling for the whole
//! demand. Work and peak live cells are independent checks. Interpreter steps
//! are neither Psi operations nor a measurement of live Psi storage.

use omega_typer::{BoundarySlot, SourcePackage, TypeError};
use std::sync::Arc;

use crate::{PreparedTrees, TargetSemantics};

/// Root-selected limits for one preparation of a package.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparationGrant {
    pub work_budget: u64,
    pub peak_live_cells: u64,
}

/// A checked maximum can establish admission before execution. These numbers
/// describe a ceiling. Actual usage requires its own versioned measurement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedResources {
    pub checked_subject: Arc<[u8]>,
    pub work_schedule: u32,
    pub maximum_work: u64,
    pub maximum_live_cells: u64,
}

/// The checker owns construction. Tie the checked package and all selected
/// inputs to the same immutable compilation, including target realizations.
pub struct CheckedPreparationBound<'input, 'text> {
    source: &'input SourcePackage<'text>,
    slots: &'input [BoundarySlot],
    target: Option<&'input TargetSemantics>,
    resources: CertifiedResources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparationResourceReceipt {
    pub grant: PreparationGrant,
    pub bound: CertifiedResources,
}

#[derive(Debug)]
pub enum SponsoredPreparationError {
    MissingBound,
    DifferentSubject,
    UnsupportedWorkSchedule(u32),
    InsufficientWork { required: u64, granted: u64 },
    InsufficientLiveCells { required: u64, granted: u64 },
    Preparation(TypeError),
}

impl std::fmt::Display for SponsoredPreparationError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::MissingBound => f.write_str("bounded preparation needs a checked resource bound"),
            Self::DifferentSubject => f.write_str("the checked resource bound belongs to different preparation inputs"),
            Self::UnsupportedWorkSchedule(version) => write!(f, "unsupported Psi work schedule {version}"),
            Self::InsufficientWork { required, granted } => write!(f, "preparation requires a work ceiling of {required}, exceeding the grant of {granted}"),
            Self::InsufficientLiveCells { required, granted } => {
                write!(f, "preparation requires a live-cell ceiling of {required}, exceeding the grant of {granted}")
            }
            Self::Preparation(error) => f.write_str(&error.message),
        }
    }
}

impl std::error::Error for SponsoredPreparationError {}

/// Use a checked upper bound covering all demands in this preparation. The
/// caller must supply a new grant for a new invocation, including cached work.
pub fn prepare_source_package_under_grant<'text>(
    source: &SourcePackage<'text>,
    slots: &[BoundarySlot],
    target: Option<&TargetSemantics>,
    grant: PreparationGrant,
    bound: Option<&CheckedPreparationBound<'_, '_>>,
) -> Result<PreparedTrees<'text>, SponsoredPreparationError> {
    crate::PreparationRound::begin_under_grant(source, slots, target, grant, bound)?
        .finish_without_external_evidence()
        .and_then(crate::PreparationProgress::into_complete)
        .map_err(SponsoredPreparationError::Preparation)
}

impl<'input, 'text> crate::PreparationRound<'input, 'text> {
    /// Admit the grant once, before any evaluation. The same checked ceiling
    /// covers every subsequent round, including calls that need separate proofs.
    pub fn begin_under_grant(
        source: &'input SourcePackage<'text>,
        slots: &'input [BoundarySlot],
        target: Option<&'input TargetSemantics>,
        grant: PreparationGrant,
        bound: Option<&CheckedPreparationBound<'_, '_>>,
    ) -> Result<Self, SponsoredPreparationError> {
        let checked = bound.ok_or(SponsoredPreparationError::MissingBound)?;
        let same_target = match (checked.target, target) {
            (None, None) => true,
            (Some(first), Some(second)) => std::ptr::eq(first, second),
            _ => false,
        };
        if !std::ptr::eq(checked.source, source) || checked.slots != slots || !same_target {
            return Err(SponsoredPreparationError::DifferentSubject);
        }
        let bound = &checked.resources;
        if bound.work_schedule != 1 {
            return Err(SponsoredPreparationError::UnsupportedWorkSchedule(bound.work_schedule));
        }
        if bound.maximum_work > grant.work_budget {
            return Err(SponsoredPreparationError::InsufficientWork {
                required: bound.maximum_work,
                granted: grant.work_budget,
            });
        }
        if bound.maximum_live_cells > grant.peak_live_cells {
            return Err(SponsoredPreparationError::InsufficientLiveCells {
                required: bound.maximum_live_cells,
                granted: grant.peak_live_cells,
            });
        }
        let mut round = Self::begin(source, slots, target).map_err(SponsoredPreparationError::Preparation)?;
        round.state.resource_receipt = Some(PreparationResourceReceipt { grant, bound: bound.clone() });
        Ok(round)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_resolver::{ParsedSource, StageSource};
    use omega_typer::resolve_source_package;

    fn resolve(text: &str) -> SourcePackage<'_> {
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        resolve_source_package(&[StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: "main.omg",
                text,
                tokens: &tokens,
                tree: &tree,
            },
        }])
        .unwrap()
    }

    #[test]
    fn admission_uses_each_certified_ceiling_for_the_exact_package() {
        let source = resolve("const VALUE:u8 = 7u8;");
        // This consumer fixture supplies a conservative verifier output. Its
        // bounds are deliberately larger than a literal needs.
        let bound = CheckedPreparationBound {
            source: &source,
            slots: &[],
            target: None,
            resources: CertifiedResources {
                checked_subject: Arc::from(&b"literal-seven"[..]),
                work_schedule: 1,
                maximum_work: 16,
                maximum_live_cells: 4,
            },
        };
        let grant = PreparationGrant {
            work_budget: 16,
            peak_live_cells: 4,
        };
        let run = |grant| prepare_source_package_under_grant(&source, &[], None, grant, Some(&bound));
        let prepared = run(grant).unwrap();
        assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
        assert_eq!(prepared.resource_receipt().unwrap().bound, bound.resources);
        assert!(matches!(
            run(PreparationGrant { work_budget: 15, ..grant }),
            Err(SponsoredPreparationError::InsufficientWork { required: 16, granted: 15 })
        ));
        assert!(matches!(
            run(PreparationGrant { peak_live_cells: 3, ..grant }),
            Err(SponsoredPreparationError::InsufficientLiveCells { required: 4, granted: 3 })
        ));
        assert!(matches!(
            prepare_source_package_under_grant(&source, &[], None, grant, None),
            Err(SponsoredPreparationError::MissingBound)
        ));
        let different = resolve("const VALUE:u8 = 8u8;");
        assert!(matches!(
            prepare_source_package_under_grant(&different, &[], None, grant, Some(&bound)),
            Err(SponsoredPreparationError::DifferentSubject)
        ));
    }
    #[test]
    fn sponsored_preparation_uses_the_checked_rank_and_keeps_the_certified_ceiling() {
        let source = resolve(
            "machine count(n:u64)->u64 terminates by n {
            match n == 0u64 {true -> 7u64, false -> count(n - 1u64),}
        } const VALUE:u64=count(3u64);",
        );
        let bound = CheckedPreparationBound {
            source: &source,
            slots: &[],
            target: None,
            resources: CertifiedResources {
                checked_subject: Arc::from(&b"checked-countdown-three"[..]),
                work_schedule: 1,
                maximum_work: 1000,
                maximum_live_cells: 100,
            },
        };
        let grant = PreparationGrant {
            work_budget: 1000,
            peak_live_cells: 100,
        };
        let round = crate::PreparationRound::begin_under_grant(&source, &[], None, grant, Some(&bound)).unwrap();
        let omega_typer::TypingProgress::Complete(trees) = round.typing() else {
            panic!("the result type is already closed")
        };
        assert!(
            round
                .evaluate(crate::CheckedPreparationEvidence::default())
                .is_err()
        );
        let evidence = crate::invocation::termination::tests::countdown_evidence(trees);
        let evaluated = round.evaluate(evidence).unwrap();
        let crate::PreparationStep::Finished(crate::PreparationProgress::Complete(prepared)) = round.advance(evaluated).unwrap() else {
            panic!("the sponsored countdown freezes its result");
        };
        assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
        assert_eq!(
            prepared.resource_receipt().unwrap(),
            &PreparationResourceReceipt { grant, bound: bound.resources }
        );
    }
}

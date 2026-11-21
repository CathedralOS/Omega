//! L4 can admit a bounded preparation using a certified ceiling for the whole
//! demand. Work and peak live cells are independent checks. Interpreter steps
//! are neither Psi operations nor a measurement of live Psi storage.

use omega_typed::BoundarySlot;
use omega_typer::SourcePackage;

use crate::{CertifiedResources, PreparationFailure, PreparationGrant, PreparationResourceReceipt, PreparedTrees, TargetSemantics};

/// The checker owns construction. Tie the checked package and all selected
/// inputs to the same immutable compilation, including target realizations.
pub struct CheckedPreparationBound<'input, 'text> {
    source: &'input SourcePackage<'text>,
    slots: &'input [BoundarySlot],
    target: Option<&'input TargetSemantics>,
    resources: CertifiedResources,
}

#[derive(Debug)]
pub enum SponsoredPreparationError {
    MissingBound,
    DifferentSubject,
    UnsupportedWorkSchedule(u32),
    InsufficientWork { required: u64, granted: u64 },
    InsufficientLiveCells { required: u64, granted: u64 },
    Preparation(PreparationFailure),
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
            Self::Preparation(error) => f.write_str(&error.diagnostic().message),
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
        round.record_resource_receipt(PreparationResourceReceipt { grant, bound: bound.clone() });
        Ok(round)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_resolver::{ParsedSource, StageSource};
    use omega_typer::resolve_source_package;
    use std::sync::Arc;

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
        spec_corpus::observe_handoff!(
            "preparation-resource-receipts/refusal",
            assert!(matches!(
                run(PreparationGrant { work_budget: 15, ..grant }),
                Err(SponsoredPreparationError::InsufficientWork { required: 16, granted: 15 })
            ))
        );
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
        let Err(SponsoredPreparationError::Preparation(PreparationFailure::Preparation(refusal))) =
            prepare_source_package_under_grant(&source, &[], None, grant, Some(&bound))
        else {
            panic!("a resource grant alone cannot establish recursive termination");
        };
        assert_eq!(refusal.cause.reason(), Some(crate::PreparationReason::CyclicInvocation));
        let round_refusal = round.evaluate_with_cause(crate::CheckedPreparationEvidence::default()).err().unwrap();
        assert_eq!(refusal, round_refusal);
        let evidence = crate::invocation::termination::tests::countdown_evidence(trees);
        let evaluated = round.evaluate(evidence).unwrap();
        let crate::PreparationStep::Finished(crate::PreparationProgress::Complete(prepared)) = round.advance(evaluated).unwrap() else {
            panic!("the sponsored countdown freezes its result");
        };
        assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
        spec_corpus::observe_handoff!(
            "preparation-resource-receipts/producer",
            assert_eq!(
                prepared.resource_receipt().unwrap(),
                &PreparationResourceReceipt { grant, bound: bound.resources }
            )
        );
    }

    const COUNTDOWN: &str = "machine count(n:u64)->u64 terminates by n {
            match n == 0u64 {true -> 7u64, false -> count(n - 1u64),}
        } const VALUE:u64=count(3u64);";

    // Stands in for the checker's output, which is the only other constructor.
    fn checked<'input, 'text>(
        source: &'input SourcePackage<'text>,
        slots: &'input [BoundarySlot],
        target: Option<&'input TargetSemantics>,
        subject: &[u8],
        (work_schedule, maximum_work, maximum_live_cells): (u32, u64, u64),
    ) -> CheckedPreparationBound<'input, 'text> {
        CheckedPreparationBound {
            source,
            slots,
            target,
            resources: CertifiedResources {
                checked_subject: Arc::from(subject),
                work_schedule,
                maximum_work,
                maximum_live_cells,
            },
        }
    }

    fn outcome(result: Result<PreparedTrees<'_>, SponsoredPreparationError>) -> String {
        match result {
            Ok(prepared) => format!("frozen {:?}", prepared.constant(&prepared.typed().constants[0].declaration)),
            Err(SponsoredPreparationError::Preparation(failure)) => format!("evaluated, then refused: {:?}", failure.reason()),
            Err(error) => format!("refused before evaluation: {error}"),
        }
    }

    /// Without the checked rank, evaluating the countdown refuses with
    /// `CyclicInvocation`. A resource or subject refusal on the same bytes
    /// therefore shows that admission read that input before evaluation.
    #[test]
    fn admission_reads_each_input_before_evaluating_the_countdown() {
        let source = resolve(COUNTDOWN);
        let other_source = resolve(
            "machine count(n:u64)->u64 terminates by n {
            match n == 0u64 {true -> 7u64, false -> count(n - 1u64),}
        } const VALUE:u64=count(4u64);",
        );
        let slot = [BoundarySlot {
            requirement: omega_typer::RequirementSlot {
                declaration: source.declarations[0].clone(),
                trait_arguments: vec![],
            },
            arguments: vec![],
            direction: omega_typed::SlotDirection::EnvironmentToProgram,
        }];
        let identity = |realization: &[u8]| crate::TargetIdentity {
            profile: "windows_x86_64".into(),
            semantics_version: 1,
            realization: Arc::from(realization),
        };
        let target = TargetSemantics::new(identity(b"sponsored target"), [], []).unwrap();
        let other_target = TargetSemantics::new(identity(b"other target"), [], []).unwrap();
        let grant = PreparationGrant {
            work_budget: 1000,
            peak_live_cells: 100,
        };
        let fits = (1, 1000, 100);
        let subject = b"checked-countdown-three".as_slice();
        let run =
            |bound: Option<&CheckedPreparationBound<'_, '_>>, target, grant| outcome(prepare_source_package_under_grant(&source, &[], target, grant, bound));
        let exact = checked(&source, &[], None, subject, fits);
        let on_target = checked(&source, &[], Some(&target), subject, fits);
        let outcomes = [
            run(Some(&exact), None, grant),
            run(Some(&on_target), Some(&target), grant),
            run(Some(&exact), None, PreparationGrant { work_budget: 999, ..grant }),
            run(Some(&exact), None, PreparationGrant { peak_live_cells: 99, ..grant }),
            run(Some(&checked(&source, &[], None, subject, (2, 1000, 100))), None, grant),
            run(Some(&checked(&other_source, &[], None, subject, fits)), None, grant),
            run(Some(&checked(&source, &slot, None, subject, fits)), None, grant),
            run(Some(&on_target), None, grant),
            run(Some(&on_target), Some(&other_target), grant),
            run(None, None, grant),
        ];
        let cyclic = "evaluated, then refused: Some(CyclicInvocation)";
        let different = "refused before evaluation: the checked resource bound belongs to different preparation inputs";
        spec_corpus::observe_handoff!(
            "preparation-resource-receipts/before-evaluation",
            assert_eq!(
                outcomes.each_ref().map(String::as_str),
                [
                    cyclic,
                    cyclic,
                    "refused before evaluation: preparation requires a work ceiling of 1000, exceeding the grant of 999",
                    "refused before evaluation: preparation requires a live-cell ceiling of 100, exceeding the grant of 99",
                    "refused before evaluation: unsupported Psi work schedule 2",
                    different,
                    different,
                    different,
                    different,
                    "refused before evaluation: bounded preparation needs a checked resource bound",
                ]
            )
        );
        // A refused admission returns no round, so no checked evidence can be evaluated.
        let short = PreparationGrant { work_budget: 999, ..grant };
        assert!(matches!(
            crate::PreparationRound::begin_under_grant(&source, &[], None, short, Some(&exact)),
            Err(SponsoredPreparationError::InsufficientWork { required: 1000, granted: 999 })
        ));
        let round = crate::PreparationRound::begin_under_grant(&source, &[], None, grant, Some(&exact)).unwrap();
        let omega_typer::TypingProgress::Complete(trees) = round.typing() else {
            panic!("the result type is already closed")
        };
        let evaluated = round
            .evaluate(crate::invocation::termination::tests::countdown_evidence(trees))
            .unwrap();
        let crate::PreparationStep::Finished(crate::PreparationProgress::Complete(prepared)) = round.advance(evaluated).unwrap() else {
            panic!("the admitted countdown freezes its result");
        };
        assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
    }

    /// A later reader gets each value from its own input: the grant from the
    /// root, the subject, schedule and maxima from the checker. Each run differs
    /// from the first in one input, and the receipt differs in that field only.
    #[test]
    fn the_receipt_keeps_the_grant_apart_from_each_certified_maximum() {
        let source = resolve("const VALUE:u8 = 7u8;");
        type Fields = (u64, u64, Vec<u8>, u32, u64, u64);
        let read = |receipt: &PreparationResourceReceipt| -> Fields {
            let PreparationResourceReceipt {
                grant: PreparationGrant { work_budget, peak_live_cells },
                bound:
                    CertifiedResources {
                        checked_subject,
                        work_schedule,
                        maximum_work,
                        maximum_live_cells,
                    },
            } = receipt;
            (
                *work_budget,
                *peak_live_cells,
                checked_subject.to_vec(),
                *work_schedule,
                *maximum_work,
                *maximum_live_cells,
            )
        };
        let run = |(work_budget, peak_live_cells): (u64, u64), subject: &[u8], maxima: (u32, u64, u64)| {
            let bound = checked(&source, &[], None, subject, maxima);
            let prepared = prepare_source_package_under_grant(&source, &[], None, PreparationGrant { work_budget, peak_live_cells }, Some(&bound)).unwrap();
            assert_eq!(prepared.integer_constant(&prepared.typed().constants[0].declaration), Some(&7.into()));
            read(
                prepared
                    .resource_receipt()
                    .expect("a sponsored package keeps its receipt"),
            )
        };
        let seven = b"literal-seven".as_slice();
        let receipts = [
            run((20, 6), seven, (1, 16, 4)),
            run((21, 6), seven, (1, 16, 4)),
            run((20, 7), seven, (1, 16, 4)),
            run((20, 6), b"literal-seven-other", (1, 16, 4)),
            run((20, 6), seven, (1, 15, 4)),
            run((20, 6), seven, (1, 16, 3)),
        ];
        let other = b"literal-seven-other".to_vec();
        spec_corpus::observe_handoff!(
            "preparation-resource-receipts/fields",
            assert_eq!(
                receipts,
                [
                    (20, 6, seven.to_vec(), 1, 16, 4),
                    (21, 6, seven.to_vec(), 1, 16, 4),
                    (20, 7, seven.to_vec(), 1, 16, 4),
                    (20, 6, other, 1, 16, 4),
                    (20, 6, seven.to_vec(), 1, 15, 4),
                    (20, 6, seven.to_vec(), 1, 16, 3),
                ]
            )
        );
        let unsponsored = crate::prepare_source_package(&source).unwrap();
        assert_eq!(unsponsored.resource_receipt(), None);
    }

    /// The width `N` is evaluated in a first round and typing resumes. The
    /// grant admitted at entry still covers, and is kept through, that round.
    #[test]
    fn one_admission_covers_a_resumed_typing_round() {
        let source = resolve("const N:u64=M; const M:u64=4; machine take(x:[u8; N])->[u8;4]{x}");
        let grant = PreparationGrant {
            work_budget: 64,
            peak_live_cells: 8,
        };
        let bound = checked(&source, &[], None, b"width-four", (1, 32, 8));
        let round = crate::PreparationRound::begin_under_grant(&source, &[], None, grant, Some(&bound)).unwrap();
        assert!(matches!(round.typing(), omega_typer::TypingProgress::Pending(_)));
        let prepared = prepare_source_package_under_grant(&source, &[], None, grant, Some(&bound)).unwrap();
        spec_corpus::observe_handoff!(
            "preparation-resource-receipts/resumed-round",
            assert_eq!(
                prepared.resource_receipt(),
                Some(&PreparationResourceReceipt {
                    grant,
                    bound: bound.resources.clone()
                })
            )
        );
    }
}

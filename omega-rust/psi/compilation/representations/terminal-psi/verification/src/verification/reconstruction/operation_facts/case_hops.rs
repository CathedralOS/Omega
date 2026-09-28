//! Payload observations through `Case` steps require proved active cases.
//!
//! A sum overlays every case's payload at one place, so a type-correct payload
//! path says nothing about which overlay is live. Each `Case` step of an
//! observed canonical path must hold for this exact root and prefix in the
//! incoming facts, after all-predecessor intersection and write invalidation.
//! A write to the sum or any prefix retires every fact below. The observation
//! itself introduces no membership. `StructuralCaseLeafCopy` and
//! case-qualified scalar field reads share this one judgment.
//!
//! A case holds in one of two ways:
//!
//! - Its membership is a current fact: a dominating membership test's true
//!   edge or a case dispatch supplied it. A kernel-checked certificate cites it.
//! - Every other declared case of that sum is refuted. A sum always holds
//!   exactly one declared case, so the last unrefuted case is active. This is
//!   how an exhaustive transition's untested final arm reads its payload.
//!
//! A refutation is narrow on purpose. A membership observation publishes
//! `v == true -> Membership(s, c)` for its own Boolean result `v`, and write
//! invalidation retires that implication if the discriminator can change. While
//! it survives, `v` is still exactly "the current case is `c`", so a known
//! `v == false` refutes `c`. The implication alone is one-directional. Only an
//! implication whose premise is the result of a `StructuralCaseMembership`
//! operation of this machine, over the same place and case, counts; another
//! producer's implication (a callee guarantee, say) refutes nothing. The
//! exhaustion step has no kernel rule: it is this module's procedural judgment,
//! bound in the trusted-surface ledger beside the membership observation row.
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, ScalarTerm, StructuralCaseId,
    StructuralCaseSubject, ValueId,
};
use terminal_psi::{OperationKind, StructuralTypeShape, TerminalMachine, TerminalModule};

use super::super::machine_context::MachineReconstructionContext;

/// Whether every `Case` step of `path` below `source` is proved active by the
/// incoming facts. A path without `Case` steps names no overlay and holds.
pub(super) fn active(
    module: &TerminalModule,
    machine: &TerminalMachine,
    context: &MachineReconstructionContext<'_>,
    axioms: &[Proposition],
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
) -> bool {
    path.iter().enumerate().all(|(position, segment)| {
        let CanonicalStructuralPathSegment::Case(case) = segment else {
            return true;
        };
        let subject = StructuralCaseSubject::new(source, path[..position].to_vec());
        let membership = Proposition::StructuralCaseMembership {
            subject: subject.clone(),
            case: *case,
        };
        let Ok(converted) = proof_admission::check_predicate_denotations_with_value_equalities(
            context.proposition_context(),
            &membership,
            &[],
            axioms,
        ) else {
            return false;
        };
        if certified(context, &converted) {
            return true;
        }
        // Exhaustion: every other declared case of this exact sum is refuted.
        let Some(cases) = declared_cases(module, machine, source, &path[..=position]) else {
            return false;
        };
        cases.iter().filter(|other| *other != case).all(|other| {
            refuted(
                module,
                machine,
                axioms,
                converted.semantic_axioms(),
                &subject,
                *other,
            )
        })
    })
}

/// A kernel-checked citation of the goal's membership fact, directly or as the
/// conclusion of an implication whose premise converted to `Truth`.
fn certified(
    context: &MachineReconstructionContext<'_>,
    converted: &proof_admission::CheckedPredicateDenotations<'_>,
) -> bool {
    let certificate = converted
        .semantic_axioms()
        .iter()
        .enumerate()
        .find_map(|(index, fact)| {
            let cited = ProofNode {
                conclusion: fact.clone(),
                rule: ProofRule::SemanticAxiom { index },
            };
            if fact == converted.goal() {
                return Some(cited);
            }
            if let Proposition::Implication {
                premise,
                conclusion,
            } = fact
                && **premise == Proposition::Truth
                && **conclusion == *converted.goal()
            {
                return Some(ProofNode {
                    conclusion: converted.goal().clone(),
                    rule: ProofRule::ImplicationElimination {
                        implication: Box::new(cited),
                        premise: Box::new(ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::Truth),
                        }),
                    },
                });
            }
            None
        });
    certificate.is_some_and(|certificate| {
        converted
            .check_certificate(context.proposition_context(), &certificate)
            .is_ok()
    })
}

/// The declared cases of the sum that `sum_path` (ending in a `Case` step)
/// qualifies below `source`.
fn declared_cases(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: PlaceId,
    sum_path: &[CanonicalStructuralPathSegment],
) -> Option<Vec<StructuralCaseId>> {
    let root_type =
        crate::validation::structural::operations::caller_structural_root_type(machine, source)?;
    let carrier = terminal_semantics::case_payload_carrier(
        module.structural_types.iter(),
        root_type,
        sum_path,
    )?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier.sum)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return None;
    };
    Some(cases.iter().map(|case| case.id).collect())
}

/// Whether a surviving membership observation of `case` on `subject` is known
/// false. The original implication's premise must be the exact Boolean result
/// of that observation; its converted form must have a false premise.
fn refuted(
    module: &TerminalModule,
    machine: &TerminalMachine,
    axioms: &[Proposition],
    converted: &[Proposition],
    subject: &StructuralCaseSubject,
    case: StructuralCaseId,
) -> bool {
    let target = Proposition::StructuralCaseMembership {
        subject: subject.clone(),
        case,
    };
    axioms.iter().zip(converted).any(|(original, converted)| {
        let Proposition::Implication {
            premise,
            conclusion,
        } = original
        else {
            return false;
        };
        let Proposition::Equal(ScalarTerm::Value { id, .. }, ScalarTerm::Boolean(true)) =
            premise.as_ref()
        else {
            return false;
        };
        **conclusion == target
            && observes(module, machine, *id, subject, case)
            && matches!(converted, Proposition::Implication { premise, conclusion }
                if **premise == Proposition::Falsehood && **conclusion == target)
    })
}

/// Whether `value` is the result of this machine's membership observation of
/// `case` on exactly `subject`'s place.
fn observes(
    module: &TerminalModule,
    machine: &TerminalMachine,
    value: ValueId,
    subject: &StructuralCaseSubject,
    case: StructuralCaseId,
) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| {
            let OperationKind::StructuralCaseMembership {
                source,
                path,
                case: observed,
            } = &operation.kind
            else {
                return false;
            };
            *source == subject.root()
                && *observed == case
                && operation
                    .result
                    .scalar_ref()
                    .is_some_and(|result| result.id == value)
                && crate::validation::canonical_field_path(module, machine, *source, path)
                    .is_some_and(|prefix| prefix == subject.path())
        })
}

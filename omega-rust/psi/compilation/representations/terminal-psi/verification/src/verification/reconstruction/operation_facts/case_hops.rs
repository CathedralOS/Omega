//! Payload observations through `Case` steps require proved active cases.
//!
//! A sum overlays every case's payload at one place, so a type-correct payload
//! path says nothing about which overlay is live. Each `Case` step of an
//! observed canonical path must hold for this exact root and prefix in the
//! incoming facts, after all-predecessor intersection and write invalidation.
//! A write to the sum or any prefix retires every fact below. The observation
//! itself introduces no membership. Borrowed argument paths, reference
//! formation, successor loans, `StructuralCaseLeafCopy` and case-qualified
//! scalar field reads share this one judgment.
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
//!
//! Conversion sees only this sum's surviving membership premises and their
//! equality dependencies. Normalizing the whole accumulated machine history
//! makes an unrelated earlier computation spend this observation's proof
//! budget. Selecting original premises changes no authority: conversion and
//! certificate checking still decide the same membership judgment.
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, Proposition, PropositionContext, ScalarTerm,
    StructuralCaseId, StructuralCaseSubject, ValueId,
};
use terminal_psi::{OperationKind, StructuralTypeShape, TerminalMachine, TerminalModule};

use super::super::machine_context::MachineReconstructionContext;

/// Check edge operands before their roots are rebound to target parameters.
/// The chosen conditional edge contributes its Boolean truth, not an asserted
/// payload membership. The same current-fact judgment used by operations must
/// derive every case in the path, after preceding writes invalidated facts.
pub(in crate::verification::reconstruction) fn validate_successors(
    module: &TerminalModule,
    machine: &TerminalMachine,
    context: &MachineReconstructionContext<'_>,
    axioms: &[Proposition],
    terminator: &terminal_psi::Terminator,
) -> Result<(), crate::ModuleError> {
    let has_case = |argument: &terminal_psi::StructuralArgument| {
        argument
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Case(_)))
    };
    let validate = |edge, arguments: &[terminal_psi::StructuralArgument], facts: &[Proposition]| {
        for argument in arguments.iter().filter(|argument| has_case(argument)) {
            let invalid = || crate::ModuleError::InvalidStructuralSuccessorArgument {
                edge,
                place: argument.place,
            };
            let path = crate::validation::canonical_field_path(
                module,
                machine,
                argument.place,
                &argument.path,
            )
            .ok_or_else(invalid)?;
            if !active(module, machine, context, facts, argument.place, &path) {
                return Err(invalid());
            }
        }
        Ok(())
    };
    match terminator {
        terminal_psi::Terminator::Jump {
            edge,
            structural_arguments,
            ..
        } => validate(*edge, structural_arguments, axioms),
        terminal_psi::Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            for (successor, positive) in [(when_true, true), (when_false, false)] {
                if !successor.structural_arguments.iter().any(has_case) {
                    continue;
                }
                let mut selected = axioms.to_vec();
                selected.push(Proposition::Equal(
                    context.value_term(*condition),
                    ScalarTerm::Boolean(positive),
                ));
                validate(successor.edge, &successor.structural_arguments, &selected)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

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
        let selected = membership_axioms(axioms, &subject);
        let Ok(converted) = proof_admission::check_predicate_denotations_with_value_equalities(
            context.proposition_context(),
            &membership,
            &[],
            &selected,
        ) else {
            return false;
        };
        if certified(context.proposition_context(), &converted) {
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
                &selected,
                converted.semantic_axioms(),
                &subject,
                *other,
            )
        })
    })
}

/// Select existing premises, never manufacture a membership or recover an
/// invalidated observation from operation history. Preserve roster order and
/// every definition of a reached value so conversion's equation choice stays
/// unchanged. A reached storage observation conservatively includes storage
/// equations too; the value-only visitor cannot distinguish their identities.
fn membership_axioms(axioms: &[Proposition], subject: &StructuralCaseSubject) -> Vec<Proposition> {
    let is_membership = |fact: &Proposition| {
        matches!(fact, Proposition::StructuralCaseMembership { subject: actual, .. }
            if actual == subject)
    };
    let mut selected = vec![false; axioms.len()];
    let mut values = Vec::new();
    let mut storage_equations = false;
    for (position, axiom) in axioms.iter().enumerate() {
        if is_membership(axiom) {
            selected[position] = true;
        } else if let Proposition::Implication {
            premise,
            conclusion,
        } = axiom
            && is_membership(conclusion)
        {
            selected[position] = true;
            storage_equations |= !premise.visit_value_ids(|value| {
                if !values.contains(&value) {
                    values.push(value);
                }
            });
        }
    }
    loop {
        let mut changed = false;
        for (position, axiom) in axioms.iter().enumerate() {
            if selected[position] {
                continue;
            }
            let Proposition::Equal(left, _) = axiom else {
                continue;
            };
            let reached = match left {
                ScalarTerm::Value { id, .. } => values.contains(id),
                ScalarTerm::BooleanField { .. }
                | ScalarTerm::IntegerField { .. }
                | ScalarTerm::IeeeFloatField { .. } => storage_equations,
                _ => false,
            };
            if !reached {
                continue;
            }
            selected[position] = true;
            changed = true;
            storage_equations |= !axiom.visit_value_ids(|value| {
                if !values.contains(&value) {
                    values.push(value);
                }
            });
        }
        if !changed {
            break;
        }
    }
    axioms
        .iter()
        .zip(selected)
        .filter(|(_, selected)| *selected)
        .map(|(axiom, _)| axiom.clone())
        .collect()
}

/// A kernel-checked citation of the goal's membership fact, directly or as the
/// conclusion of an implication whose premise converted to `Truth`.
fn certified(
    context: &PropositionContext,
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
    certificate
        .is_some_and(|certificate| converted.check_certificate(context, &certificate).is_ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ScalarType, StructuralPlaceKind};

    fn fixture() -> (
        PropositionContext,
        StructuralCaseSubject,
        Proposition,
        Vec<Proposition>,
    ) {
        let place = PlaceId::new(1).unwrap();
        let subject = StructuralCaseSubject::new(place, Vec::new());
        let membership = Proposition::StructuralCaseMembership {
            subject: subject.clone(),
            case: StructuralCaseId::new(1).unwrap(),
        };
        let value =
            |ordinal| ScalarTerm::value(ValueId::new(ordinal).unwrap(), ScalarType::Boolean);
        let context = PropositionContext::from_value_types_and_places(
            (1..=3).map(|ordinal| (ValueId::new(ordinal).unwrap(), ScalarType::Boolean)),
            [(
                place,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
        )
        .unwrap();
        let axioms = vec![
            Proposition::Equal(value(2), ScalarTerm::Boolean(true)),
            Proposition::Equal(value(1), value(2)),
            Proposition::Implication {
                premise: Box::new(Proposition::Equal(value(1), ScalarTerm::Boolean(true))),
                conclusion: Box::new(membership.clone()),
            },
        ];
        (context, subject, membership, axioms)
    }

    fn proves(
        context: &PropositionContext,
        subject: &StructuralCaseSubject,
        goal: &Proposition,
        axioms: &[Proposition],
    ) -> bool {
        let selected = membership_axioms(axioms, subject);
        let Ok(converted) = proof_admission::check_predicate_denotations_with_value_equalities(
            context,
            goal,
            &[],
            &selected,
        ) else {
            return false;
        };
        certified(context, &converted)
    }

    #[test]
    fn membership_ignores_unrelated_history_without_raising_conversion_budget() {
        let (context, subject, goal, mut axioms) = fixture();
        let expected = axioms.clone();
        let irrelevant = Proposition::Equal(
            ScalarTerm::value(ValueId::new(3).unwrap(), ScalarType::Boolean),
            ScalarTerm::Boolean(true),
        );
        axioms.extend(std::iter::repeat_n(irrelevant, 1500));
        assert!(matches!(
            proof_admission::check_predicate_denotations_with_value_equalities(
                &context,
                &goal,
                &[],
                &axioms
            ),
            Err(proof_admission::PredicateDenotationError::ResourceLimitExceeded)
        ));
        assert_eq!(membership_axioms(&axioms, &subject), expected);
        assert!(proves(&context, &subject, &goal, &axioms));
    }

    #[test]
    fn selection_preserves_equation_order_and_rejects_missing_or_cyclic_support() {
        let (context, subject, goal, mut axioms) = fixture();
        // Both definitions survive; selecting only a convenient true equation
        // would change the denotation checker's first-definition discipline.
        axioms.insert(
            0,
            Proposition::Equal(
                ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
                ScalarTerm::Boolean(false),
            ),
        );
        assert_eq!(membership_axioms(&axioms, &subject), axioms);
        assert!(!proves(&context, &subject, &goal, &axioms));
        axioms.remove(0);
        axioms.remove(0);
        assert!(!proves(&context, &subject, &goal, &axioms));
        axioms.insert(
            0,
            Proposition::Equal(
                ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
                ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
            ),
        );
        assert!(!proves(&context, &subject, &goal, &axioms));
    }

    #[test]
    fn retired_or_different_membership_cannot_be_recreated_from_value_truth() {
        let (context, subject, goal, mut axioms) = fixture();
        axioms.pop();
        assert!(!proves(&context, &subject, &goal, &axioms));
        let other = StructuralCaseSubject::new(PlaceId::new(2).unwrap(), Vec::new());
        axioms.push(Proposition::StructuralCaseMembership {
            subject: other,
            case: StructuralCaseId::new(1).unwrap(),
        });
        assert!(membership_axioms(&axioms, &subject).is_empty());
        assert!(!proves(&context, &subject, &goal, &axioms));
    }

    #[test]
    fn membership_dependencies_retain_live_storage_equations() {
        let (context, subject, goal, mut axioms) = fixture();
        let field = ScalarTerm::BooleanField {
            root: subject.root(),
            path: vec![CanonicalStructuralPathSegment::Field(
                semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            )],
        };
        axioms[0] = Proposition::Equal(
            ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
            field.clone(),
        );
        axioms.push(Proposition::Equal(field, ScalarTerm::Boolean(true)));
        assert_eq!(membership_axioms(&axioms, &subject), axioms);
        assert!(proves(&context, &subject, &goal, &axioms));
    }
}

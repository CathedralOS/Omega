//! Values that proof and custody sidecars name without listing direct uses.
//!
//! Propositions carried by contracts, crash routes, site guards, and call
//! crash continuations keep the exact value identities they mention: they are
//! proof terms, not uses. Recorded source-call joins keep the scalar
//! environment they capture. A rewrite that removes a value identity must
//! leave every one of these carriers intact.

use semantic_vocabulary::ValueId;
use std::collections::BTreeSet;
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, OperationKind as O};

pub(crate) fn retain_proposition(
    proposition: &semantic_vocabulary::Proposition,
    retained_values: &mut BTreeSet<ValueId>,
) {
    proposition.visit_value_ids(|value| {
        retained_values.insert(value);
    });
}

/// Erased-argument lanes carry caller-side proof terms: retain every identity
/// they name so a rewrite cannot strip a formal's evidence.
pub(crate) fn retain_erased_arguments(
    terms: &[semantic_vocabulary::ScalarTerm],
    retained_values: &mut BTreeSet<ValueId>,
) {
    for term in terms {
        term.visit_value_ids(|value| retained_values.insert(value));
    }
}

/// The erased-argument lane of a call kind that carries one.
pub(crate) fn call_erased_arguments(kind: &O) -> &[semantic_vocabulary::ScalarTerm] {
    match kind {
        O::Call {
            erased_arguments, ..
        }
        | O::CallUnit {
            erased_arguments, ..
        }
        | O::CallStructuralScalar {
            erased_arguments, ..
        }
        | O::CallStructuralWithScalarArguments {
            erased_arguments, ..
        } => erased_arguments,
        _ => &[],
    }
}

pub(crate) fn retain_crash_routes(
    routes: &[CrashRouteBucket],
    retained_values: &mut BTreeSet<ValueId>,
) {
    for bucket in routes {
        for alternative in &bucket.alternatives {
            if let CrashRouteGuard::Predicate(term) = alternative {
                retain_proposition(term.proposition(), retained_values);
            }
        }
    }
}

pub(crate) fn crash_continuations(kind: &O) -> &[CrashRouteBucket] {
    match kind {
        O::Call {
            crash_continuations,
            ..
        }
        | O::CallUnit {
            crash_continuations,
            ..
        }
        | O::CallStructuralScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicUnit {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterUnit {
            crash_continuations,
            ..
        }
        | O::CallStructural {
            crash_continuations,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            crash_continuations,
            ..
        } => crash_continuations,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    //! The retention helpers' own legs: every proposition-carried identity a
    //! rewrite must keep is collected transitively, `Truth` alternatives and
    //! literal-only propositions carry none, and the continuation selector
    //! covers every crash-continuation-bearing call kind and nothing else.
    use super::{crash_continuations, retain_crash_routes, retain_proposition};
    use semantic_vocabulary::{
        BoundaryMachineId, IntegerSign, IntegerType, MachineId, Proposition, PropositionId,
        ScalarTerm, ScalarType, ValueId,
    };
    use std::collections::BTreeSet;
    use terminal_psi::{
        CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard, OperationKind as O,
    };

    fn value(ordinal: u64) -> ValueId {
        ValueId::new(ordinal).unwrap()
    }

    fn bool_term(ordinal: u64) -> ScalarTerm {
        ScalarTerm::Value {
            id: value(ordinal),
            scalar_type: ScalarType::Boolean,
        }
    }

    fn int_term(ordinal: u64) -> ScalarTerm {
        ScalarTerm::Value {
            id: value(ordinal),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        }
    }

    fn predicate(proposition: Proposition) -> CrashRouteGuard {
        CrashRouteGuard::Predicate(CrashPredicateTerm::new(proposition))
    }

    fn truth_bucket() -> Vec<CrashRouteBucket> {
        vec![CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![CrashRouteGuard::Truth],
        }]
    }

    #[test]
    fn proposition_retention_collects_every_nested_value_identity() {
        // Positive: value identities nested inside compound propositions and
        // scalar-term operators all join the retained set. Negative: Truth,
        // Falsehood, Atom, and literal terms carry no value identity.
        let proposition = Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Atom(PropositionId::new(7).unwrap()),
            Proposition::Equal(bool_term(1), ScalarTerm::Boolean(true)),
            Proposition::Implication {
                premise: Box::new(Proposition::LessThan(int_term(2), int_term(3))),
                conclusion: Box::new(Proposition::Equal(
                    ScalarTerm::BooleanNot {
                        operand: Box::new(bool_term(4)),
                    },
                    ScalarTerm::Boolean(false),
                )),
            },
        ]);
        let mut retained = BTreeSet::new();
        retain_proposition(&proposition, &mut retained);
        assert_eq!(
            retained,
            BTreeSet::from([value(1), value(2), value(3), value(4)])
        );
    }

    #[test]
    fn crash_route_predicates_retain_named_values_while_truth_carries_none() {
        // Positive: every Predicate alternative's proposition contributes its
        // value identities across every bucket. Boundary: a `Truth`
        // alternative — the sole row of its canonical bucket — retains
        // nothing, and a route list without buckets retains nothing.
        let routes = vec![
            CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![
                    predicate(Proposition::Equal(bool_term(5), ScalarTerm::Boolean(true))),
                    predicate(Proposition::Conjunction(vec![
                        Proposition::Equal(int_term(6), int_term(7)),
                        Proposition::Falsehood,
                    ])),
                ],
            },
            CrashRouteBucket {
                cause: CrashCause::Abort,
                alternatives: vec![CrashRouteGuard::Truth],
            },
        ];
        let mut retained = BTreeSet::new();
        retain_crash_routes(&routes, &mut retained);
        assert_eq!(retained, BTreeSet::from([value(5), value(6), value(7)]));

        let mut empty = BTreeSet::new();
        retain_crash_routes(&[], &mut empty);
        retain_crash_routes(&truth_bucket(), &mut empty);
        assert!(empty.is_empty());
    }

    #[test]
    fn crash_continuations_selects_every_call_kinds_buckets() {
        // Positive boundary: all nine operation kinds whose record carries
        // crash continuations yield that exact slice. Boundary: every other
        // kind — including `BoundaryCall`, a call shape without continuations —
        // yields none.
        let callee = MachineId::new(2).unwrap();
        let calls = [
            O::Call {
                erased_arguments: Vec::new(),
                callee,
                arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallUnit {
                erased_arguments: Vec::new(),
                callee,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallStructuralScalar {
                erased_arguments: Vec::new(),
                callee,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallDynamicScalar {
                descriptor_ordinal: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallDynamicParameterScalar {
                parameter_ordinal: 0,
                requirement_slot: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallDynamicUnit {
                descriptor_ordinal: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallDynamicParameterUnit {
                parameter_ordinal: 0,
                requirement_slot: 0,
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
            O::CallStructural {
                callee,
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
                selected_evidence: Vec::new(),
            },
            O::CallStructuralWithScalarArguments {
                erased_arguments: Vec::new(),
                callee,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: truth_bucket(),
            },
        ];
        for kind in &calls {
            let continuations = crash_continuations(kind);
            assert_eq!(
                continuations.len(),
                1,
                "every call kind yields its own continuation buckets"
            );
            assert_eq!(continuations[0].cause, CrashCause::Trap);
        }
        for kind in [
            O::BooleanConstant { value: true },
            O::BoundaryCall {
                boundary: BoundaryMachineId::new(1).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
            O::StoreDynamicDescriptor {
                descriptor_ordinal: 0,
            },
            O::EstablishScalarArray {
                elements: Vec::new(),
            },
        ] {
            assert!(
                crash_continuations(&kind).is_empty(),
                "a kind without crash continuations selects nothing"
            );
        }
    }
}

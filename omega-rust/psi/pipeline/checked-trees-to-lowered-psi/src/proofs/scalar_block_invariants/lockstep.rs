//! Strengthen guarded field bounds into counter/divisor lockstep families.
//!
//! A bounded exit guard `counter < N` combined with the body's exact updates
//! `divisor = divisor / d` and `counter = counter + 1` leaves the plain bound
//! `counter < N -> B <= divisor` non-inductive: the last in-guard iteration
//! already divided `divisor` once too often. The inductive predicate is the
//! lockstep family `counter < k -> B * d^(N-k) <= divisor` for `k` in `1..=N`;
//! the clause at `k = N` is the original bound and earlier clauses carry the
//! remaining quotients. These are still proposals — the retained roster
//! proves every actual arrival before the module grants authority, including
//! the vacuous `counter < 1` clause, which discharges through a checked
//! integer contradiction rather than a trusted prune. A rewrite only
//! replaces the conditional candidate it understood; anything else keeps its
//! original predicate for the same prove-or-drop boundary.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerValue, PlaceId, Proposition, ScalarTerm,
    StructuralFieldId, ValueId,
};
use terminal_psi::{ScalarBlockInvariant, TerminalModule, Terminator};

/// The largest counter bound expanded into a lockstep family. A cycle whose
/// guard exceeds this keeps its original candidate for the ordinary check.
const MAXIMUM_LOCKSTEP_BOUND: u128 = 64;

pub(super) fn strengthen(module: &TerminalModule, candidates: &mut [ScalarBlockInvariant]) {
    for candidate in candidates.iter_mut() {
        if let Some(predicate) = lockstep(module, candidate) {
            candidate.predicate = predicate;
        }
    }
}

/// Rewrite one `guards -> B <= divisor` candidate into its lockstep family
/// when the machine's backedge applies the exact `divisor /= d` and
/// `counter += 1` updates this family measures. Anything unrecognized leaves
/// the candidate untouched.
fn lockstep(module: &TerminalModule, candidate: &ScalarBlockInvariant) -> Option<Proposition> {
    let Proposition::Implication {
        premise,
        conclusion,
    } = &candidate.predicate
    else {
        return None;
    };
    let Proposition::LessOrEqual(
        bound @ ScalarTerm::Integer { .. },
        divisor_field @ ScalarTerm::IntegerField { .. },
    ) = conclusion.as_ref()
    else {
        return None;
    };
    let (_, base) = bound.integer_value()?;
    let IntegerValue::Unsigned(base) = base else {
        return None;
    };
    // The smallest strict counter bound among the retained guards is the
    // measured iteration count N; a non-strict `counter <= m` reads as the
    // equivalent `counter < m + 1`.
    let guards: &[Proposition] = match premise.as_ref() {
        Proposition::Conjunction(members) => members.as_slice(),
        other => std::slice::from_ref(other),
    };
    let mut counter_field = None;
    let mut bound_n = None;
    for guard in guards {
        let (left, right) = match guard {
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        let (
            ScalarTerm::IntegerField { .. },
            ScalarTerm::Integer {
                value: IntegerValue::Unsigned(n),
                ..
            },
        ) = (left, right)
        else {
            continue;
        };
        let strict = match guard {
            Proposition::LessThan(..) => *n,
            _ => n.checked_add(1)?,
        };
        if strict == 0 {
            continue;
        }
        if bound_n.is_none_or(|existing| strict < existing) {
            counter_field = Some(left.clone());
            bound_n = Some(strict);
        }
    }
    let counter_field = counter_field?;
    let bound_n = bound_n?;
    if bound_n > MAXIMUM_LOCKSTEP_BOUND {
        return None;
    }
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == candidate.machine)?;
    let divisor_leaf = field_leaf(divisor_field)?;
    let counter_leaf = field_leaf(&counter_field)?;
    // Only the blocks that flow back to the header can carry the per-iteration
    // update; an update anywhere else is not the lockstep this family names.
    let divisor_step = update_literal(machine, candidate.header, divisor_leaf, Update::Divide)?;
    let stride = update_literal(machine, candidate.header, counter_leaf, Update::Add)?;
    if stride != 1 || divisor_step == 0 {
        return None;
    }
    // `counter < k -> B * d^(N-k) <= divisor` for k in 1..=N. The bound literal
    // must stay inside the divisor field's own carrier; overflow keeps the
    // original candidate rather than proposing an out-of-type literal.
    let ScalarTerm::IntegerField {
        scalar_type: divisor_type,
        ..
    } = divisor_field
    else {
        unreachable!("matched above")
    };
    let ScalarTerm::IntegerField {
        scalar_type: counter_type,
        ..
    } = &counter_field
    else {
        unreachable!("matched above")
    };
    let mut clauses = Vec::new();
    for k in 1..=bound_n {
        let required =
            (0..bound_n - k).try_fold(base, |bound, _| bound.checked_mul(divisor_step))?;
        let bound_term =
            ScalarTerm::integer(*divisor_type, IntegerValue::Unsigned(required)).ok()?;
        let threshold = ScalarTerm::integer(*counter_type, IntegerValue::Unsigned(k)).ok()?;
        clauses.push(Proposition::Implication {
            premise: Box::new(Proposition::LessThan(counter_field.clone(), threshold)),
            conclusion: Box::new(Proposition::LessOrEqual(bound_term, divisor_field.clone())),
        });
    }
    Some(Proposition::Conjunction(clauses))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Update {
    Divide,
    Add,
}

fn field_leaf(term: &ScalarTerm) -> Option<(PlaceId, StructuralFieldId)> {
    let ScalarTerm::IntegerField { root, path, .. } = term else {
        return None;
    };
    let [CanonicalStructuralPathSegment::Field(field)] = path.as_slice() else {
        return None;
    };
    Some((*root, *field))
}

/// The literal operand of the field's backedge update: `field = field / d`
/// reads as `Update::Divide`, `field = field + s` as `Update::Add`. The store
/// and its defining operation must live in one block whose terminator
/// returns to the header, so the literal is the exact per-iteration step.
fn update_literal(
    machine: &terminal_psi::TerminalMachine,
    header: semantic_vocabulary::BlockId,
    leaf: (PlaceId, StructuralFieldId),
    update: Update,
) -> Option<u128> {
    let (root, field) = leaf;
    let mut found = None;
    for block in &machine.blocks {
        if !terminator_reaches(&block.terminator, header) {
            continue;
        }
        for store in &block.operations {
            let terminal_psi::OperationKind::StructuralScalarFieldStore {
                destination,
                path,
                field: stored,
                value,
                ..
            } = &store.kind
            else {
                continue;
            };
            if *destination != root || !path.is_empty() || *stored != field {
                continue;
            }
            let definition = block.operations.iter().find(|operation| {
                operation
                    .result
                    .scalar_ref()
                    .is_some_and(|result| result.id == *value)
            })?;
            // A divide's read must be the dividend; an add's literal may sit
            // on either side of the commutative step.
            let pairs: &[(ValueId, ValueId)] = match (&definition.kind, update) {
                (
                    terminal_psi::OperationKind::WrappingIntegerDivide { left, right, .. },
                    Update::Divide,
                ) => &[(*left, *right)],
                (terminal_psi::OperationKind::WrappingIntegerAdd { left, right }, Update::Add) => {
                    &[(*left, *right), (*right, *left)]
                }
                _ => continue,
            };
            let mut literal = None;
            for (operand, constant) in pairs {
                let reads_field = block.operations.iter().any(|operation| {
                    matches!(
                        &operation.kind,
                        terminal_psi::OperationKind::IntegerStructuralField {
                            source,
                            path,
                            field: read,
                            ..
                        } if operation
                            .result
                            .scalar_ref()
                            .is_some_and(|result| result.id == *operand)
                            && *source == root
                            && path.is_empty()
                            && *read == field
                    )
                });
                if !reads_field {
                    continue;
                }
                literal = block
                    .operations
                    .iter()
                    .find_map(|operation| match &operation.kind {
                        terminal_psi::OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(literal),
                        } if operation
                            .result
                            .scalar_ref()
                            .is_some_and(|result| result.id == *constant) =>
                        {
                            Some(*literal)
                        }
                        _ => None,
                    });
                if literal.is_some() {
                    break;
                }
            }
            let Some(literal) = literal else {
                continue;
            };
            if found.is_some_and(|existing| existing != literal) {
                return None;
            }
            found = Some(literal);
        }
    }
    found
}

fn terminator_reaches(terminator: &Terminator, header: semantic_vocabulary::BlockId) -> bool {
    match terminator {
        Terminator::Jump { target, .. } => *target == header,
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => when_true.target == header || when_false.target == header,
        Terminator::StructuralCase { cases, .. } => {
            cases.iter().any(|successor| successor.target == header)
        }
        _ => false,
    }
}

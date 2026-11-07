//! Range constraint intervals, dependent maxima and value admission.
//!
//! Intervals are a sufficient proof, not the complete meaning of a receiving
//! bound. Preserve the evaluated value's live relation environment when a
//! correlation such as `amount <= limit - used` supplies the tighter result.
//! Operand formation precedes admission; the requested range is never a premise.

use crate::CheckingInput;
use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
use crate::proof_contracts::arithmetic_domains::interval::Interval;
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// An already-formed integer value under the receiving validator's live facts.
/// Statement validation has conservatively retired every nested call's writes,
/// including destination selectors; transition arguments may carry their own
/// separately fenced environments. If their sufficient interval/relation proof
/// does not settle admission, the completed-value owner reconstructs the exact
/// occurrence with ordered effects and complete private arrivals. Neither route
/// publishes the requested bound as a premise.
#[derive(Clone, Copy)]
pub(crate) struct IntegerValueObservation<'a> {
    pub machine: &'a Machine,
    pub state: Option<&'a State>,
    pub expression: ExpressionHandle,
    pub environment: &'a ValueEnvironment,
    pub interval: Interval,
}

impl IntegerValueObservation<'_> {
    fn fits(self, program: &dyn CheckingInput, stated: Interval) -> bool {
        if stated
            .low
            .zip(stated.high)
            .is_some_and(|(low, high)| low > high)
        {
            return false;
        }
        let lower_proven = stated
            .low
            .is_none_or(|bound| self.interval.low.is_some_and(|low| low >= bound));
        let upper_proven = stated
            .high
            .is_none_or(|bound| self.interval.high.is_some_and(|high| high <= bound));
        if lower_proven && upper_proven {
            return true;
        }
        let Some(state) = self.state else {
            return false;
        };
        let minimum = stated.low.map(BigInt::from_i64);
        let maximum = stated.high.map(BigInt::from_i64);
        if super::live_integer_constraints::proves_range(
            program,
            self.machine,
            state,
            self.environment,
            self.expression,
            if lower_proven { None } else { minimum.as_ref() },
            if upper_proven { None } else { maximum.as_ref() },
        ) {
            return true;
        }
        let Some(statement_index) =
            crate::proof_contracts::contract_entailment::expression_statement_index(
                program,
                state,
                self.expression,
            )
        else {
            return false;
        };
        crate::proof_contracts::contract_entailment::completed_value_fits_integer_interval(
            program,
            self.machine,
            state,
            statement_index,
            self.expression,
            stated,
        )
    }
}

pub(crate) fn range_constraint_interval(
    program: &dyn CheckingInput,
    handle: TypeReferenceHandle,
) -> Option<Interval> {
    // Integral-looking IEEE endpoints are still floating bounds. Applying an
    // integer predecessor to their exclusive maximum would turn [0.0..1.0]
    // into [0..=0]. The floating obligation owner retains IEEE comparisons.
    if floating_range_carrier(program, handle) {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            range_constraint_interval(program, *referee)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                // A DECLARED domain's predicates bound the place exactly as a
                // bracketed range does: membership requires every predicate,
                // and a place declared `in D` establishes D at every write. The
                // derivation is shared with the index prover so a bound moved
                // off the revoked suffix reaches both readers.
                TypeConstraintNode::Domain(domain) => {
                    super::domain_intervals::declared_domain_predicate_bounds(program, domain).map(
                        |(minimum, maximum)| Interval {
                            low: minimum.to_i64(),
                            high: maximum.to_i64(),
                        },
                    )
                }
                _ => None,
            })
            .chain(range_constraint_interval(program, *base_type))
            .reduce(Interval::intersect),
        _ => None,
    }
}

/// An atomic resident update must preserve its declaration's interval at the
/// event. Ordinary record predicates use restoration windows instead. An
/// unstated end constrains nothing; a stated end needs the matching proven end.
pub(crate) fn check_where_interval_containment(
    program: &dyn CheckingInput,
    stated: Interval,
    value: IntegerValueObservation<'_>,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !value.fits(program, stated) {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} stores a value not provably within its data's `requires` facts: every read \
             trusts them (indexes, exact arithmetic), so the stored value must be proven to \
             honor them. Narrow the value with a dominating guard or a requires coupling",
        )));
        return false;
    }
    true
}

fn floating_range_carrier(program: &dyn CheckingInput, reference: TypeReferenceHandle) -> bool {
    crate::unwrapped_type_reference(program, reference)
        .and_then(|carrier| program.primitive_type_reference(carrier))
        .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64))
}

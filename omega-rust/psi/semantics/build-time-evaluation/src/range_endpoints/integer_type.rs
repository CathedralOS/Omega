//! Closed integer positions establish every range before snapshots erase types.
//!
//! A range refines values of the same integer carrier; a domain or arithmetic
//! policy is not interchangeable with that refinement. Only range shells over
//! an exact builtin leaf enter this route. Bounds retain their original source
//! selections, while their values read the completed working substitutions.

use numerics::bignum::BigInt;
use typed_trees::{
    TypedTrees,
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

pub(super) struct IntegerPosition {
    pub(super) primitive: PrimitiveType,
    ranges: Vec<(BigInt, BigInt)>,
}

impl IntegerPosition {
    pub(super) fn prepare(
        program: &TypedTrees,
        original: &TypedTrees,
        mut reference: TypeReferenceHandle,
        authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    ) -> Result<Self, String> {
        let mut ranges = Vec::new();
        let mut visited = Vec::new();
        loop {
            if !program
                .type_reference_table
                .contains_type_reference(reference)
                || visited.contains(&reference)
            {
                return Err("invalid or cyclic range endpoint integer type".to_owned());
            }
            visited.push(reference);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            else {
                break;
            };
            let rows = program.type_reference_table.constraints(*constraints);
            if rows.is_empty() || rows.len() != constraints.count() as usize {
                return Err("range endpoint integer type has invalid constraints".to_owned());
            }
            for constraint in rows {
                let TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } = constraint
                else {
                    return Err(
                        "range endpoint integer types admit only range refinements".to_owned()
                    );
                };
                for expression in [*minimum, *maximum] {
                    crate::admission::require_closed_integer_argument(
                        original, program, expression, authority,
                    )?;
                }
                let minimum = validation::closed_integer_range_bound(program, *minimum)
                    .ok_or("range endpoint type needs a closed minimum")?;
                let maximum =
                    validation::closed_integer_range_maximum(program, *maximum, *end_inclusive)
                        .ok_or("range endpoint type needs a closed maximum")?;
                ranges.push((minimum, maximum));
            }
            reference = *base_type;
        }
        let primitive =
            crate::const_generic_expressions::exact_probe_destination(program, reference)
                .filter(|primitive| primitive.accepts_integer_literal())
                .ok_or("range endpoint position requires an exact builtin integer carrier")?;
        Ok(Self { primitive, ranges })
    }

    pub(super) fn require_value(&self, value: &BigInt) -> Result<(), String> {
        // Snapshot integers use i64 bits even for u64. Check the decoded exact
        // value, never a signed compatibility interval or merely its kind.
        typed_trees::closed_numeric::land_integer(value, self.primitive)
            .ok_or("range endpoint value does not fit its declared integer carrier")?;
        for (minimum, maximum) in &self.ranges {
            if value < minimum || value > maximum {
                return Err(format!(
                    "range endpoint value `{value}` is outside declared range `{minimum}..={maximum}`"
                ));
            }
        }
        Ok(())
    }
}

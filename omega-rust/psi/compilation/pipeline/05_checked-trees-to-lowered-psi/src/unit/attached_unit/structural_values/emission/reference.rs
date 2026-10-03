//! Reference construction is determined by its storage contract, not its syntax
//! position. A stored carrier owns a reference value, never its backing. A
//! borrowed referent joins the existing place directly. Both consume the same
//! checked source projection and access; record fields need no special route.

use super::{Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation};
use super::{lookup_type_id, unsupported};
use crate::expression_preparation::bindings::{LoanAccess, ScalarBindings};
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan};

impl Emission<'_, '_, '_> {
    pub(super) fn reference_value(
        &mut self,
        source: &CheckedUnitStructuralArgumentPlan,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let (machine, state) = crate::expression_preparation::source_custody::authored_state(
            self.checked,
            self.state,
        )?;
        let argument = ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
            .borrowed_structural_argument(source, machine, self.checked.state_parameters(state))?;
        let referent_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        // A borrowed join names the referent itself, even when that referent
        // happens to contain another reference. Compare the relationship
        // before classifying the destination's outer shape.
        if self.structural_type == referent_type {
            if source.access != CheckedStructuralAccess::SharedBorrow {
                return unsupported("borrowed value changed its referent or access");
            }
            return if let Some(continuation) = continuation {
                self.complete_loan(argument, continuation, LoanAccess::Shared)?;
                Ok(continuation.place)
            } else {
                self.direct_borrow(argument)
            };
        }
        let declaration = self
            .structural_types
            .iter()
            .find(|declaration| declaration.id == self.structural_type)
            .ok_or(LoweringError::Unsupported(
                "reference value type is missing",
            ))?;
        if let StructuralTypeShape::Reference { referent, access } = &declaration.shape {
            if *referent != referent_type || *access != argument.access {
                return unsupported("reference carrier changed its referent or access");
            }
            let carrier = super::super::super::reference_results::emit_carrier(
                self.structural_type,
                argument,
                self.next_place,
                self.operations,
            )?;
            let place = carrier.id;
            self.temporary_places.push(carrier);
            if let Some(continuation) = continuation {
                self.complete_value(place, None, continuation)?;
            }
            return Ok(place);
        }

        unsupported("reference destination is neither its carrier nor referent")
    }
}

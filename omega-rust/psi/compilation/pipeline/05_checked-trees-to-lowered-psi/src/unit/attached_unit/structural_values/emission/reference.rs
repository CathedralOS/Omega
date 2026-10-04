//! Reference construction is determined by its storage contract, not its syntax
//! position. A stored carrier owns a reference value, never its backing. A
//! borrowed referent joins the existing place directly. Both consume the same
//! checked source projection and access; record fields need no special route.

use super::{Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation};
use super::{lookup_type_id, unsupported};
use crate::expression_preparation::bindings::{LoanAccess, ScalarBindings, StructuralLocalBinding};
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan};

impl Emission<'_, '_, '_> {
    pub(super) fn reference_value(
        &mut self,
        source: &CheckedUnitStructuralArgumentPlan,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let primitive_home = self.immutable_local_borrow_home(source)?;
        let (machine, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let mut locals = self.evaluation.structural_locals.clone();
        locals.extend(primitive_home);
        let argument = ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&locals)
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

    /// Immutable scalar locals are values until an address is observed. Rejoin
    /// their authored ordinal to the completed SSA value, then give that value
    /// one shared home using the same operation as addressable scalar ingress.
    fn immutable_local_borrow_home(
        &mut self,
        source: &CheckedUnitStructuralArgumentPlan,
    ) -> Result<Option<StructuralLocalBinding>, LoweringError> {
        let Some(facts::PlaceRoot::Symbol(symbol)) = source.structural_storage_root() else {
            return Ok(None);
        };
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let Some((statement, local)) = self.checked.typed.state_local_declaration(state, symbol)
        else {
            return Ok(None);
        };
        let Some(primitive) = self.checked.primitive_type_reference(local.type_reference) else {
            return Ok(None);
        };
        if local.is_mutable {
            return Ok(None);
        }
        if statement >= self.statement as usize
            || source.access != CheckedStructuralAccess::SharedBorrow
            || !source.path.is_empty()
            || self
                .checked
                .normalized_type_identity(local.type_reference)
                .as_str()
                != source.type_identity
        {
            return unsupported("immutable primitive borrow changed its source or access");
        }
        let scalar_type = crate::emission::scalar_types::terminal_scalar_type(primitive)?;
        let ordinal = crate::expression_preparation::source_custody::source_local_ordinal(
            self.checked,
            self.state,
            u32::try_from(statement).map_err(|_| {
                LoweringError::Unsupported("primitive local statement ordinal overflow")
            })?,
        )?;
        let bindings =
            self.evaluation
                .scalar_bindings
                .as_ref()
                .ok_or(LoweringError::Unsupported(
                    "immutable primitive borrow has no completed scalar namespace",
                ))?;
        let position = bindings.immutable_local_value(ordinal as usize, scalar_type)?;
        let value = self.values.get(position).ok_or(LoweringError::Unsupported(
            "immutable primitive borrow lost its completed scalar value",
        ))?;
        if value.scalar_type != scalar_type {
            return unsupported("immutable primitive borrow changes its completed scalar type");
        }
        let structural_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        let place = self.evaluation.immutable_primitive_home(
            symbol,
            value,
            structural_type,
            self.structural_types,
            self.next_place,
            self.operations,
            self.temporary_places,
        )?;
        Ok(Some(StructuralLocalBinding {
            result_binding_ordinal: None,
            root: facts::PlaceRoot::Symbol(symbol),
            structural_type,
            argument: terminal_psi::StructuralArgument {
                place,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
        }))
    }
}

//! Reference construction is determined by its storage contract, not its syntax
//! position. A stored carrier owns a reference value, never its backing. A
//! borrowed referent joins the existing place directly. Both consume the same
//! checked source projection and access; record fields need no special route.
//! A computed source retains its ordinary backing producer. Establish it at
//! this operand's evaluation point, then lend its home; flattening it into a
//! place-only plan loses both its evaluation schedule and its storage identity.

use super::{Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation};
use super::{lookup_type_id, unsupported};
use crate::expression_preparation::bindings::{LoanAccess, ScalarBindings};
use checked_trees::CheckedUnitStructuralArgumentPlan;

impl Emission<'_, '_, '_> {
    pub(super) fn reference_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        formation: super::super::super::reference_results::Formation<'_>,
        source: &CheckedUnitStructuralArgumentPlan,
        source_value: checked_trees::CheckedStructuralValueHandle,
        continuation: Option<&ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let has_primitive_home = self.evaluation.prepare_immutable_local_borrow(
            self.checked,
            self.state,
            source.structural_storage_root(),
            self.values,
            self.structural_types,
            self.next_place,
            self.operations,
            self.temporary_places,
        )?;
        let (machine, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let referent_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        let referent =
            terminal_psi::find_structural_type(&self.structural_types[..], referent_type).ok_or(
                LoweringError::Unsupported("reference referent type is missing"),
            )?;
        // Direct primitive storage needs its original mutable home. Existing
        // structural projections, parameters and immutable homes retain their
        // established join, including shared root qualification custody.
        let argument = if source_value.is_valid() {
            let node = self.structural_value_node(source_value);
            let root_type = lookup_type_id(
                self.type_ids,
                &validation::stored_structural_type_identity(
                    &self.checked.typed,
                    node.type_reference,
                    &[],
                    &[],
                ),
            )?;
            let completed = self.child_constructor_operand(
                identities,
                root_type,
                source_value,
                node.type_reference,
                &[],
            )?;
            // The loan and its later retirement name this exact home. Keep
            // its expression identity at establishment, not a reconstructed
            // initializer or a second evaluation during cleanup.
            let root = source
                .structural_storage_root()
                .ok_or(LoweringError::Unsupported(
                    "computed reference backing has no storage identity",
                ))?;
            self.evaluation.structural_locals.push(
                crate::expression_preparation::bindings::StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root,
                    argument: completed.argument.clone(),
                    structural_type: root_type,
                },
            );
            let mut argument = completed.argument;
            argument.path.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities, &source.path, root_type, self.structural_types,
            )?);
            argument.access =
                crate::expression_preparation::bindings::structural_paths::terminal_access(
                    source.access,
                );
            argument
        } else {
            let bindings = ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
                .with_structural_parameters(&self.evaluation.structural_parameters)
                .with_structural_locals(&self.evaluation.structural_locals)
                .with_structural_observations(self.structural_types)
                .with_primitive_storage(&self.evaluation.primitive_storage);
            match referent.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type)
                if !has_primitive_home
                    && matches!(
                        source.source,
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
                    ) =>
            {
                bindings.primitive_borrow(
                    identities,
                    self.checked,
                    source,
                    scalar_type,
                    self.structural_types,
                )?
            }
            _ => bindings.borrowed_structural_argument(
                identities,
                self.checked,
                source,
                machine,
                self.checked.state_parameters(state),
            )?,
        }
        };
        // A borrowed join names the referent itself, even when that referent
        // happens to contain another reference. Test the relationship before
        // classifying the destination's outer shape.
        if self.structural_type == referent_type {
            return if let Some(continuation) = continuation {
                self.complete_loan(argument, continuation, LoanAccess::Shared)?;
                Ok(continuation.place)
            } else {
                self.direct_borrow(argument)
            };
        }
        let declaration =
            terminal_psi::find_structural_type(&self.structural_types[..], self.structural_type)
                .ok_or(LoweringError::Unsupported(
                    "reference value type is missing",
                ))?;
        if let StructuralTypeShape::Reference { .. } = &declaration.shape {
            let carrier = super::super::super::reference_results::emit_carrier(
                formation,
                self.structural_type,
                argument,
                None,
                &self.structural_types[..],
                super::super::super::place_id(super::super::super::allocate_dense(
                    self.next_place,
                )?),
                self.operations,
            )?;
            let place = carrier.id;
            self.temporary_places.push(carrier);
            if let Some(continuation) = continuation {
                self.complete_value(identities, place, None, continuation)?;
            }
            return Ok(place);
        }

        unsupported("reference destination is neither its carrier nor referent")
    }
}

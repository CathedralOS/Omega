//! Reference construction is determined by its storage contract, not its syntax
//! position. A stored carrier owns a reference value, never its backing. A
//! borrowed referent joins the existing place directly. Both consume the same
//! checked source projection and access; record fields need no special route.
//! A computed source retains its ordinary backing producer. Establish it at
//! this operand's evaluation point, then lend its home; flattening it into a
//! place-only plan loses both its evaluation schedule and its storage identity.

use super::{Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation};
use super::{lookup_type_id, unsupported};
use crate::expression_preparation::bindings::structural_paths::{
    PreparedStructuralPathSegment, emit_captured_argument_path, prepare_structural_argument_path,
};
use crate::expression_preparation::bindings::{
    LoanAccess, ScalarBindings, StructuralScalarFieldBinding,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

impl Emission<'_, '_, '_> {
    pub(super) fn reference_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        formation: super::super::super::reference_results::Formation<'_>,
        source: &CheckedUnitStructuralArgumentPlan,
        source_value: checked_trees::CheckedStructuralValueHandle,
        storage: checked_trees::CheckedReferenceStorage,
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
        if source_value.is_valid() {
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
        }
        let bindings = ScalarBindings::new(self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_structural_observations(self.structural_types)
            .with_primitive_storage(&self.evaluation.primitive_storage);
        let argument = match referent.shape {
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
            _ => {
                let mut argument = bindings.borrowed_structural_home(
                    source,
                    machine,
                    self.checked.state_parameters(state),
                )?;
                let root_type = bindings.structural_argument_type(&argument).ok_or(
                    LoweringError::Unsupported("reference source lost its published selected type"),
                )?;
                let suffix = prepare_structural_argument_path(
                    identities,
                    self.checked,
                    source,
                    root_type,
                    self.structural_types,
                )?;
                let prepared = argument
                    .path
                    .iter()
                    .cloned()
                    .map(PreparedStructuralPathSegment::Static)
                    .chain(suffix)
                    .collect::<Vec<_>>();
                let fields = StructuralScalarFieldBinding::collect(
                    &self.evaluation.structural_parameters,
                    self.structural_types,
                );
                let source_count = self.values.len();
                let mut computation_ordinal = 0u32;
                for segment in &source.path {
                    match segment {
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(value) => {
                            let selector = self.scalar(
                                identities,
                                checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                                    expression: formation.expression,
                                    selector_ordinal: computation_ordinal,
                                },
                                *value,
                                source_count,
                            )?;
                            computation_ordinal = computation_ordinal.checked_add(1).ok_or(
                                LoweringError::Unsupported("reference selector ordinal overflow"),
                            )?;
                            self.values.push(selector);
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                            checked_trees::CheckedRuntimeIndex::StructuralField(_),
                        ) => {
                            let selected =
                                super::super::super::parameters::mint_runtime_index_selectors(
                                    identities,
                                    &[checked_trees::CheckedUnitStructuralArgumentPlan {
                                        path: vec![segment.clone()],
                                        ..source.clone()
                                    }],
                                    &fields,
                                    self.values.as_slice(),
                                    self.next_value,
                                    self.operations,
                                )?;
                            for id in selected {
                                let value = self
                                    .operations
                                    .iter()
                                    .rev()
                                    .filter_map(|operation| operation.result.scalar())
                                    .find(|value| value.id == id)
                                    .or_else(|| {
                                        self.values.iter().find(|value| value.id == id).copied()
                                    })
                                    .ok_or(LoweringError::Unsupported(
                                        "reference field selector lost its evaluated value",
                                    ))?;
                                self.values.push(value);
                            }
                        }
                        _ => {}
                    }
                }
                // Private selector control flow transports the earlier saved
                // values. Read their final identities after all evaluations.
                let captured = self.values[source_count..]
                    .iter()
                    .map(|value| value.id)
                    .collect::<Vec<_>>();
                self.values.truncate(source_count);
                let mut selectors = captured.into_iter();
                argument.place = self.evaluation.current_structural_place(argument.place);
                // A view lends from its current descriptor. Its live extent
                // and captured selectors belong to this reference formation's
                // operand frame, just as they do for an ordinary call operand.
                argument.path = emit_captured_argument_path(
                    &prepared,
                    argument.place,
                    Some(self.values.as_slice()),
                    &mut selectors,
                    Some(&mut *self.calls),
                    Some((&mut *self.next_value, &mut *self.operations)),
                )?;
                argument.access =
                    crate::expression_preparation::bindings::structural_paths::terminal_access(
                        source.access,
                    );
                argument
            }
        };
        // A borrowed join names the referent itself, even when that referent
        // happens to contain another reference. Test the relationship before
        // classifying the destination's outer shape.
        if self.structural_type == referent_type {
            let loan = match argument.access {
                terminal_psi::StructuralAccess::SharedBorrow => LoanAccess::Shared,
                terminal_psi::StructuralAccess::MutableBorrow => LoanAccess::Mutable,
                _ => return unsupported("reference alias requires readable loan access"),
            };
            return if let Some(continuation) = continuation {
                self.complete_loan(argument, continuation, loan)?;
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
            let storage = super::super::super::reference_results::publish_storage(
                storage,
                self.next_value,
                self.operations,
            )?
            .map(|(view, value)| {
                self.values.push(value);
                view
            });
            let carrier = super::super::super::reference_results::emit_carrier(
                formation,
                self.structural_type,
                argument,
                storage,
                &self.structural_types[..],
                super::super::super::place_id(super::super::super::allocate_dense(
                    self.next_place,
                )?),
                self.operations,
            )?;
            let place = carrier.id;
            self.temporary_places.push(carrier);
            return self.completed(identities, place, continuation);
        }

        unsupported("reference destination is neither its carrier nor referent")
    }
}

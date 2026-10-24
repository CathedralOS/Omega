//! Reference construction is determined by its storage contract, not its syntax
//! position. A stored carrier owns a reference value, never its backing. A
//! borrowed referent joins the existing place directly. Both consume the same
//! checked source projection and access; record fields need no special route.
//! A computed source retains its ordinary backing producer. Establish it at
//! this operand's evaluation point, then lend its home; flattening it into a
//! place-only plan loses both its evaluation schedule and its storage identity.
//! Once a destination is not the referent alias, the shared carrier producer
//! selects its declared representation. Repeating an outer Reference shape
//! test here would exclude fixed byte views, whose carrier is a descriptor.

use super::{Emission, LoweringError, PlaceId, StructuralTypeShape, ValueContinuation};
use super::{lookup_type_id, unsupported};
use crate::expression_preparation::bindings::LoanAccess;
use crate::expression_preparation::bindings::structural_paths::{
    PreparedStructuralPathSegment, prepare_structural_argument_path,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;

impl Emission<'_, '_, '_> {
    pub(super) fn reference_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        formation: super::super::super::reference_results::Formation<'_>,
        source: &CheckedUnitStructuralArgumentPlan,
        selectors: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
        source_value: checked_trees::CheckedStructuralValueHandle,
        storage: checked_trees::CheckedReferenceStorage,
        completion: super::CompletionOccurrence,
        continuation: Option<&mut ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        // Address demand belongs to the original source, including a scalar
        // formal. Reuse ordinary argument preparation so representation views
        // lend that home's bits rather than creating storage of the viewed type.
        self.evaluation.prepare_borrowed_argument(
            identities,
            self.checked,
            self.state,
            source,
            self.values,
            self.structural_types,
            self.domain_declarations,
            self.environment.next_place,
            self.operations,
            self.environment.private_places,
        )?;
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let referent_type = lookup_type_id(self.type_ids, &source.type_identity)?;
        let referent = terminal_psi::find_structural_type(self.structural_types, referent_type)
            .ok_or(LoweringError::Unsupported(
                "reference referent type is missing",
            ))?;
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
            // Backing completes through its own producer occurrence, not the
            // descriptor's child roster. The shared construction/call owner
            // publishes that exact home's claims before the loan is formed.
            let completed = self.child_constructor_operand(
                identities,
                root_type,
                source_value,
                node.type_reference,
            )?;
            // The loan and its later retirement name this exact home. Keep
            // its expression identity at establishment, not a reconstructed
            // initializer or a second evaluation during cleanup.
            let root = source
                .structural_storage_root()
                .ok_or(LoweringError::Unsupported(
                    "computed reference backing has no storage identity",
                ))?;
            crate::expression_preparation::bindings::retain_completed_structural_homes(
                &mut self.evaluation.structural_locals,
                &[
                    crate::expression_preparation::bindings::StructuralLocalBinding {
                        result_binding_ordinal: None,
                        root,
                        argument: completed.argument.clone(),
                        structural_type: root_type,
                    },
                ],
            )?;
        }
        let evaluations = self
            .checked
            .facts
            .values
            .scalar_computations
            .operands
            .span(selectors)
            .ok_or(LoweringError::Unsupported(
                "reference has stale selector evaluations",
            ))?;
        if source
            .computation_operands(&self.checked.facts.values.scalar_computations.operands)
            .any(|root| !evaluations.contains(&root))
        {
            return unsupported("reference path has no retained selector evaluation");
        }
        let bindings = self
            .evaluation
            .scalar_bindings
            .clone()
            .with_computation_operands(0, self.values.iter().map(|value| value.scalar_type))
            .with_structural_parameters(&self.evaluation.structural_parameters)
            .with_structural_locals(&self.evaluation.structural_locals)
            .with_view_locals(&self.evaluation.view_locals)
            .with_structural_observations(self.structural_types)
            .with_primitive_storage(&self.evaluation.primitive_storage);
        let argument = match referent.shape {
            StructuralTypeShape::PrimitiveScalar(_)
                if matches!(
                    source.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
                ) =>
            {
                bindings.scalar_parameter_borrow(
                    identities,
                    self.checked,
                    source,
                    self.structural_types,
                )?
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type)
                if matches!(
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
                    self.checked.state_parameters(state),
                )?
            }
            _ => {
                let argument = bindings
                    .borrowed_structural_home(source, self.checked.state_parameters(state))?;
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
                self.complete_projection(
                    identities,
                    formation.expression,
                    argument.place,
                    prepared,
                    crate::expression_preparation::bindings::structural_paths::terminal_access(
                        source.access,
                    ),
                )?
            }
        };
        // Destination access distinguishes an alias from a stored descriptor.
        // A slice descriptor can have the same shape as the collection it
        // reborrows; shape equality alone cannot turn its owned field into an
        // alias of the supplying descriptor. Conversely a borrowed join keeps
        // the referent itself even when that referent contains references.
        if self.result_access != terminal_psi::StructuralAccess::Owned {
            if self.structural_type != referent_type {
                return unsupported("reference alias changes its referent carrier");
            }
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
        // The offset is an operand of this carrier establishment, not a live
        // scalar result of the enclosing expression. Keeping it in `values`
        // would send an arm-private temporary to an already-sized value join.
        let (storage, _) = super::super::super::reference_results::publish_storage(
            storage,
            self.next_value,
            self.operations,
        )?;
        let carrier = super::super::super::reference_results::emit_carrier(
            identities,
            formation,
            self.structural_type,
            argument,
            storage,
            self.structural_types,
            self.evaluation,
            super::super::super::place_id(super::super::super::allocate_dense(
                self.environment.next_place,
            )?),
            self.operations,
        )?;
        let place = carrier.id;
        self.environment.private_places.push(carrier);
        self.completed(identities, place, completion, continuation)
    }
}

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
use crate::expression_preparation::bindings::StructuralLocalBinding;
use crate::expression_preparation::bindings::structural_paths::structural_access_can_supply;
use crate::shared::checked_facts::value_facts;
use crate::shared::terminal_identities::stored_structural_identity;
use crate::unit::attached_unit::structural_values::ValueOutcome;
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
    ) -> Result<ValueOutcome<PlaceId>, LoweringError> {
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
        let (_, state) = crate::shared::declarations::state_declaration(self.checked, self.state)?;
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
                &stored_structural_identity(self.checked, node.type_reference),
            )?;
            let root = source
                .structural_storage_root()
                .ok_or(LoweringError::Unsupported(
                    "computed reference backing has no storage identity",
                ))?;
            if self
                .evaluation
                .completed_source_binding(source_value, root, root_type, self.operations)?
                .is_none()
            {
                // A genuinely unproduced backing completes through its own
                // occurrence and publishes its claims before the loan forms.
                let ValueOutcome::Normal(completed) = self.child_constructor_operand(
                    identities,
                    root_type,
                    source_value,
                    node.type_reference,
                )?
                else {
                    return Ok(ValueOutcome::NoNormal);
                };
                crate::expression_preparation::bindings::retain_completed_structural_homes(
                    &mut self.evaluation.structural_locals,
                    &[StructuralLocalBinding {
                        result_binding_ordinal: None,
                        root,
                        argument: completed.argument,
                        structural_type: root_type,
                    }],
                )?;
            }
            // Cached reuse already proved the exact current root, argument
            // and type. Publishing that same binding with no result alias
            // would be an exact no-op; only a fresh producer publishes here.
        }
        let evaluations = value_facts(self.checked)
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
            .contextual_bindings()
            .with_computation_operands(0, self.values.iter().map(|value| value.scalar_type))
            .with_structural_observations(self.structural_types);
        let mut argument = match referent.shape {
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
                // A saved reference may name borrowed storage without owning a
                // descriptor slot. Reborrowing it follows the checked loan to
                // that storage, using the same path as an ordinary call.
                let argument = bindings.prepared_borrowed_argument(
                    identities,
                    self.checked,
                    source,
                    self.checked.state_parameters(state),
                    self.structural_types,
                )?;
                self.complete_projection(
                    identities,
                    formation.expression,
                    argument.source.known_place()?,
                    argument.path,
                    argument.access,
                )?
            }
        };
        // A borrowed destination aliases the referent only when that loan keeps
        // the referent's representation. Equal type identities do. So does an
        // unchanged descriptor presentation: a shared byte view and the
        // mutable view it reborrows are different type identities with the
        // same pointer and extent, and the destination access is only a lower
        // ceiling. Owned access is never that alias. A slice descriptor can
        // have the same shape as the collection it reborrows, and shape
        // equality cannot turn the owned field into the supplying referent.
        // Addressing inline backing, or any other carrier change, is a new
        // descriptor. The carrier producer below selects that representation.
        // Establishing a whole-collection view for an unchanged reborrow looks
        // for collection storage the existing descriptor does not have.
        let same_representation = self.structural_type == referent_type
            || terminal_semantics::borrowed_referent_presentation(
                self.structural_types.iter(),
                referent_type,
                self.structural_type,
            ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged);
        if self.result_access != terminal_psi::StructuralAccess::Owned && same_representation {
            if self.structural_type != referent_type {
                if !structural_access_can_supply(argument.access, self.result_access) {
                    return unsupported("reference alias changes its referent carrier");
                }
                // Present the destination ceiling. The place remains the
                // existing descriptor, so the window and backing stay put.
                argument.access = self.result_access;
            }
            let loan = match argument.access {
                terminal_psi::StructuralAccess::SharedBorrow => LoanAccess::Shared,
                terminal_psi::StructuralAccess::MutableBorrow => LoanAccess::Mutable,
                _ => return unsupported("reference alias requires readable loan access"),
            };
            return if let Some(continuation) = continuation {
                self.complete_loan(argument, continuation, loan)?;
                Ok(ValueOutcome::Normal(continuation.place))
            } else {
                self.direct_borrow(argument).map(ValueOutcome::Normal)
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
            .map(ValueOutcome::Normal)
    }
}

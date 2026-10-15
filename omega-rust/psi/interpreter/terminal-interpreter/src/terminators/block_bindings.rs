//! Snapshot selected successor operands before cleanup or parameter replacement.
//!
//! A state edge relocates a borrowed descriptor, not its referent. Mutable and
//! write-only aggregate loans use the same backing-preserving transport; their
//! declared source access independently limits what the destination may do.
//! Qualification requirements use the common argument binder on the captured
//! runtime memberships. A destination declaration never mints missing evidence.
//!
//! Cleanup preflight follows the ordered custody transaction on scratch ledgers.
//! It never runs hooks or writes backing. This keeps rejection before fuel and
//! mutation without treating an already-disposed parent's claim as child overlap;
//! the subsequent commit applies the same transitions to the live frame.
//! Projected ownership transfers the affine complement and authenticated linear
//! claims separately. Outgoing claims leave the source before hooks execute,
//! remain captured across suspension, and arrive only after residual cleanup.
//! An authored state's claim roster binds destination-local names by active
//! typed path. Capture all sources first, then check the complete bijection;
//! rebinding a name never creates an obligation. Internal continuation blocks
//! without binding rows simply relocate the identities they received.
//! Reentry retires old operation homes only after outgoing owned sources leave
//! custody, and before destinations are installed. Otherwise a captured call
//! result appears to leak its claim, or retirement removes a fresh binding.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, ClaimId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape,
};

use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::bind_active_structural_arguments;
use crate::custody::bind_affine_frontier;
use crate::custody::bind_arguments;
use crate::custody::resolve_single_structural_argument;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{LiveClaim, TerminalExecution};
use crate::values::{TerminalScalarValue, TerminalStructuralValue};

#[derive(Default)]
pub(crate) struct BlockBindings {
    scalars: BTreeMap<ValueId, TerminalScalarValue>,
    structural: BTreeMap<PlaceId, TerminalStructuralValue>,
    aggregate_backings: BTreeMap<PlaceId, crate::reference::AggregateBacking>,
    byte_sequences: BTreeMap<PlaceId, ByteSequenceBinding>,
    primitive_loans: BTreeMap<PlaceId, crate::primitive_storage::PrimitiveLoan>,
    element_views: BTreeMap<PlaceId, crate::element_views::ElementView>,
    owned_sources: BTreeSet<PlaceId>,
    claims: BTreeMap<ClaimId, LiveClaim>,
    /// Roots whose projected children moved on this edge. Their whole-root
    /// frontier entries are split by the caller, not removed as transfers.
    pub(super) projected_sources: BTreeSet<PlaceId>,
    affine_destinations: BTreeSet<StructuralAffineDiscard>,
    aggregate_payload: crate::record::storage::ConstructorStorage,
    identity_cursor: Option<u64>,
}

impl BlockBindings {
    pub(crate) fn from_scalars(scalars: Vec<(ValueId, TerminalScalarValue)>) -> Self {
        Self {
            scalars: scalars.into_iter().collect(),
            ..Self::default()
        }
    }

    /// All operands have been captured, including descriptors whose source
    /// places are also destinations. Updating one destination cannot affect
    /// another binding. Affine and borrowed descriptors retain their backing;
    /// owned unrestricted records publish independent staged payloads.
    /// Reserve staged identities after edge fuel succeeds but before hooks can
    /// allocate. Completion must not rewind the allocator to this saved cursor.
    pub(crate) fn reserve_identities(&self, execution: &mut TerminalExecution) {
        execution
            .local_structural_identities
            .commit_cursor(self.identity_cursor);
    }

    pub(crate) fn commit(
        self,
        execution: &mut TerminalExecution,
        target: BlockId,
    ) -> Result<(), TerminalInterpretError> {
        for source in self.owned_sources {
            execution
                .live_claims
                .retain(|_, claim| claim.place != Some(source));
            execution.structural_values.remove(&source);
            execution.aggregate_argument_backings.remove(&source);
            execution.byte_sequence_values.remove(&source);
            execution.scalar_array_values.remove(&source);
            execution.element_view_values.remove(&source);
            execution.primitive_loan_values.remove(&source);
            execution
                .live_affine_frontier
                .retain(|entry| entry.place != source);
        }
        // Captured values retain backing after their source homes end. Check
        // remaining old results before publishing any destination: unrelated
        // surviving custody still refuses reentry rather than being erased.
        execution.retire_reentered_results(target)?;
        self.aggregate_payload.publish(execution)?;
        for destination in self.structural.keys() {
            execution.aggregate_argument_backings.remove(destination);
            execution.scalar_array_values.remove(destination);
            execution.primitive_loan_values.remove(destination);
        }
        execution.live_claims.extend(self.claims);
        execution.values.extend(self.scalars);
        execution.structural_values.extend(self.structural);
        execution
            .aggregate_argument_backings
            .extend(self.aggregate_backings);
        execution.byte_sequence_values.extend(self.byte_sequences);
        execution.primitive_loan_values.extend(self.primitive_loans);
        execution.element_view_values.extend(self.element_views);
        execution
            .live_affine_frontier
            .extend(self.affine_destinations);
        Ok(())
    }

    pub(crate) fn prepare_cleanup_actions(
        &self,
        execution: &TerminalExecution,
        structural_arguments: &[StructuralArgument],
        actions: &[terminal_psi::TerminalAffineCleanupAction],
    ) -> Result<Vec<terminal_psi::TerminalAffineCleanupAction>, TerminalInterpretError> {
        use terminal_psi::TerminalAffineCleanupAction;
        // Capture each receiver once. The resulting schedule survives hook
        // suspension and is exactly the schedule preflight checks below.
        let mut selected = actions.to_vec();
        for (action, selected) in actions.iter().zip(&mut selected) {
            let path = match selected {
                TerminalAffineCleanupAction::DiscardResidual(discard) => &mut discard.path,
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => &mut cleanup.path,
                _ => continue,
            };
            if path.iter().any(|segment| segment.runtime_index().is_some()) {
                execution.select_runtime_projection(
                    action.place(),
                    execution.live_structural_type(action.place())?,
                    action.path(),
                    path,
                )?;
            }
        }
        let mut discarded = BTreeSet::new();
        let mut values = execution.structural_values.clone();
        let mut frontier = execution.live_affine_frontier.clone();
        let mut claims = execution.live_claims.clone();
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned
                && !argument.path.is_empty()
                && self.projected_sources.contains(&argument.place)
        }) {
            crate::custody::consume_projected_ownership(
                &execution.program.structural_types(),
                &values,
                &execution.structural_cases,
                &mut frontier,
                &mut claims,
                argument,
            )?;
        }
        // Source checking owns the recursive parent-before-child schedule;
        // overlapping hook receivers are not overlapping region destruction.
        // Preflight follows that same custody transaction without executing any
        // hook or changing backing. Checking each child against the unchanged
        // entry ledger mistakes its already-consumed parent's claim for overlap.
        for action in &selected {
            let place = action.place();
            let path = action.path();
            if self.owned_sources.contains(&place)
                || (path.is_empty()
                    && self.projected_sources.contains(&place)
                    && !matches!(action, TerminalAffineCleanupAction::RetireHome(_)))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            // Projected operands were captured before this schedule. Ending
            // their old frame home leaves that captured backing intact; a
            // whole-region disposal would consume the outgoing value instead.
            discarded.insert(place);
            match action {
                TerminalAffineCleanupAction::DiscardRoot(_) => {
                    let structural_type = crate::custody::verified_structural_place(
                        &execution.structural_values,
                        place,
                    )
                    .map(|value| value.structural_type)?;
                    if claims.values().any(|claim| claim.place == Some(place))
                        || frontier
                            .iter()
                            .filter(|entry| entry.place == place)
                            .ne(std::iter::once(&StructuralAffineDiscard {
                                place,
                                path: Vec::new(),
                                structural_type,
                            }))
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    crate::custody::remove_affine_root(&mut frontier, place);
                    values.remove(&place);
                }
                TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    crate::custody::validate_nominal_cleanup_action(
                        &execution.program.structural_types(),
                        &execution.structural_cases,
                        &execution.program.machines(),
                        &values,
                        &frontier,
                        &claims,
                        cleanup,
                    )?;
                    crate::custody::begin_nominal_cleanup(
                        &execution.program.structural_types(),
                        &values,
                        &execution.structural_cases,
                        &mut frontier,
                        &mut claims,
                        cleanup,
                    )?;
                    crate::custody::complete_nominal_cleanup(
                        &mut values,
                        &mut frontier,
                        &claims,
                        cleanup,
                    )?;
                }
                TerminalAffineCleanupAction::DiscardResidual(discard) => {
                    crate::custody::consume_affine_discard(
                        &execution.program.structural_types(),
                        &values,
                        &execution.structural_cases,
                        &mut frontier,
                        &claims,
                        discard,
                    )?;
                }
                TerminalAffineCleanupAction::RetireHome(_) => {
                    crate::custody::retire_cleanup_home(
                        &mut values,
                        &mut frontier,
                        &claims,
                        place,
                    )?;
                }
            }
        }
        // A swap may replace another transferred root. Replacing a different
        // live destination requires its explicit whole-root discard first.
        if self.structural.keys().any(|place| {
            execution.live_affine_discard_at(*place).is_some()
                && !self.owned_sources.contains(place)
                && !self.projected_sources.contains(place)
                && !discarded.contains(place)
        }) {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        // Destination names can equal names consumed elsewhere in a swap or
        // reentry, but cannot overwrite an obligation surviving the edge.
        claims.retain(|_, claim| {
            !claim
                .place
                .is_some_and(|place| self.owned_sources.contains(&place))
        });
        if self
            .claims
            .keys()
            .any(|identity| claims.contains_key(identity))
        {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        Ok(selected)
    }
}

impl TerminalExecution<'_> {
    /// Snapshot a selected edge's operands, including projected affine children.
    /// The caller commits each exact transfer and any residual cleanup before
    /// installing these bindings at the destination.
    pub(crate) fn prepare_block_bindings(
        &self,
        target: BlockId,
        arguments: &[ValueId],
        structural_arguments: &[StructuralArgument],
    ) -> Result<BlockBindings, TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        let block = machine
            .block(&target)
            .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
        let scalar_arguments = self.scalar_values_at(arguments)?;
        let scalars = bind_arguments(&block.parameters, &scalar_arguments)?;
        if block.structural_parameters.len() != structural_arguments.len() {
            return Err(TerminalInterpretError::StructuralArgumentCount {
                expected: block.structural_parameters.len(),
                actual: structural_arguments.len(),
            });
        }
        let mut resolved_arguments = Vec::with_capacity(structural_arguments.len());
        let mut aggregate_backings = BTreeMap::new();
        let contains_payloads = structural_arguments
            .iter()
            .any(|argument| self.scalar_array_values.contains_key(&argument.place));
        let mut descriptor_parameters = Vec::new();
        let mut descriptor_arguments = Vec::new();
        let mut packed_arguments = Vec::new();
        let mut primitive_loans = BTreeMap::new();
        let mut array_loans = BTreeMap::new();
        let mut byte_loans = BTreeMap::new();
        let mut local_array_roots = BTreeMap::new();
        let mut identity_cursor = self.local_structural_identities.cursor();
        let mut owned_sources = BTreeSet::new();
        let mut claims = BTreeMap::new();
        let mut projected_sources = BTreeSet::new();
        for (position, (parameter, argument)) in block
            .structural_parameters
            .iter()
            .zip(structural_arguments)
            .enumerate()
        {
            if usize::try_from(parameter.position).ok() != Some(position)
                || (parameter.access != StructuralAccess::Owned
                    && self
                        .live_claims
                        .values()
                        .any(|claim| claim.place == Some(argument.place)))
                || argument.access != parameter.access
            {
                return Err(malformed());
            }
            if parameter.access != StructuralAccess::Owned
                && matches!(
                    self.program
                        .structural_types()
                        .get(&parameter.structural_type)
                        .ok_or_else(malformed)?
                        .shape,
                    StructuralTypeShape::PrimitiveScalar(_)
                )
            {
                if !self.block_parameter_is_declared(target, parameter) {
                    return Err(malformed());
                }
                let loan =
                    self.capture_primitive_loan(parameter, argument, &mut identity_cursor)?;
                resolved_arguments.push(loan.referent.clone());
                primitive_loans.insert(parameter.place, loan);
                if contains_payloads {
                    descriptor_parameters.push(parameter.clone());
                    descriptor_arguments.push(argument.clone());
                }
                continue;
            }
            if let Some((referent, loan)) = self.prepare_scalar_array_loan(
                parameter,
                argument,
                &mut identity_cursor,
                &mut local_array_roots,
            )? {
                if !self.block_parameter_is_declared(target, parameter)
                    || self.live_affine_discard_at(parameter.place).is_some()
                    || array_loans.insert(parameter.place, loan).is_some()
                {
                    return Err(malformed());
                }
                resolved_arguments.push(referent);
                if contains_payloads {
                    descriptor_parameters.push(parameter.clone());
                    descriptor_arguments.push(argument.clone());
                }
                continue;
            }
            // A byte loan lends exactly what the same argument lends a call:
            // an array presented as a view, a bounded owner, a view field.
            if parameter.access != StructuralAccess::Owned
                && matches!(
                    self.program
                        .structural_types()
                        .get(&parameter.structural_type)
                        .ok_or_else(malformed)?
                        .shape,
                    StructuralTypeShape::ByteSequence(_)
                )
                && let Some((referent, binding)) = self.prepare_byte_sequence_loan(
                    parameter,
                    argument,
                    &mut identity_cursor,
                    &mut local_array_roots,
                )?
            {
                if !self.block_parameter_is_declared(target, parameter)
                    || self.live_affine_frontier.iter().any(|entry| {
                        entry.place == parameter.place
                            || (entry.place == argument.place && !entry.path.is_empty())
                    })
                    || byte_loans.insert(parameter.place, binding).is_some()
                {
                    return Err(malformed());
                }
                resolved_arguments.push(referent);
                if contains_payloads {
                    descriptor_parameters.push(parameter.clone());
                    descriptor_arguments.push(argument.clone());
                }
                continue;
            }
            if let Some(root) = self.scalar_array_values.get(&argument.place) {
                let source_matches =
                    self.owned_block_source(argument.place)
                        .is_some_and(|source| {
                            source.structural_type == root.structural_type
                                && source.access == StructuralAccess::Owned
                                && source.multiplicity == StructuralMultiplicity::Unrestricted
                        })
                        || self.owned_result_source(
                            argument.place,
                            root.structural_type,
                            StructuralMultiplicity::Unrestricted,
                        );
                let value = crate::record::storage::ConstructorValue::resolve(self, argument)?;
                if !self.block_parameter_is_declared(target, parameter)
                    || !source_matches
                    || parameter.access != StructuralAccess::Owned
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || parameter.structural_type != value.structural_type()
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || machine
                        .block_entry_claims
                        .iter()
                        .any(|claim| claim.input == parameter.place)
                    || self.live_claims.values().any(|claim| {
                        claim.place == Some(argument.place) || claim.place == Some(parameter.place)
                    })
                    || self.live_affine_frontier.iter().any(|entry| {
                        entry.place == argument.place || entry.place == parameter.place
                    })
                {
                    return Err(malformed());
                }
                // Packed arrays and selected leaves arrive in ordinary typed
                // storage. Snapshot every operand before edge cleanup/writes;
                // no source ownership or sibling payload is retired by a copy.
                packed_arguments.push((parameter, argument, value));
                continue;
            }
            let value =
                crate::custody::verified_structural_place(&self.structural_values, argument.place)?;
            // A projected owned argument binds the selected subtree, keeping
            // the root's opaque identity and the exact moved path. The caller
            // discharges the residual complement on the same edge.
            let mut projected_value = None;
            match parameter.access {
                StructuralAccess::Owned => {
                    if !self.block_parameter_is_declared(target, parameter) {
                        return Err(malformed());
                    }
                    if argument.path.is_empty()
                        || parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    {
                        let selected = resolve_single_structural_argument(
                            &self.program.structural_types(),
                            &self.structural_values,
                            argument,
                        )?;
                        if selected.structural_type != parameter.structural_type {
                            return Err(malformed());
                        }
                        let source_matches =
                            self.owned_block_source(argument.place)
                                .is_some_and(|source| {
                                    source.structural_type == value.structural_type
                                        && source.multiplicity == parameter.multiplicity
                                        && source.access == StructuralAccess::Owned
                                })
                                || self.owned_result_source(
                                    argument.place,
                                    value.structural_type,
                                    parameter.multiplicity,
                                );
                        if !source_matches {
                            return Err(malformed());
                        }
                        let carried = crate::custody::whole_value_frontier(
                            &self.program.structural_types(),
                            argument.place,
                            parameter.structural_type,
                            parameter.multiplicity,
                            self.live_claim_paths_at(argument.place),
                            &|current| {
                                self.stored_case_at_discard(&self.structural_values, current)
                            },
                        )?;
                        if self
                            .live_affine_frontier
                            .iter()
                            .filter(|entry| entry.place == argument.place)
                            .ne(carried.iter())
                            || projected_sources.contains(&argument.place)
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                            && !owned_sources.insert(argument.place)
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        let mut count = 0;
                        for (identity, claim) in &self.live_claims {
                            if claim.place != Some(argument.place) {
                                continue;
                            }
                            if parameter.multiplicity == StructuralMultiplicity::Unrestricted {
                                return Err(malformed());
                            }
                            let mut moved = claim.clone();
                            moved.place = Some(parameter.place);
                            if claims.insert(*identity, moved).is_some() {
                                return Err(malformed());
                            }
                            count += 1;
                        }
                        if parameter.multiplicity == StructuralMultiplicity::Linear && count == 0 {
                            return Err(malformed());
                        }
                        projected_value = Some(selected);
                    } else {
                        let root = value;
                        let view = resolve_single_structural_argument(
                            &self.program.structural_types(),
                            &self.structural_values,
                            &argument,
                        )?;
                        if view.structural_type != parameter.structural_type {
                            return Err(TerminalInterpretError::StructuralArgumentType {
                                place: parameter.place,
                                expected: parameter.structural_type,
                                actual: view.structural_type,
                            });
                        }
                        let source_matches =
                            self.owned_block_source(argument.place)
                                .is_some_and(|source| {
                                    source.structural_type == root.structural_type
                                        && source.multiplicity == StructuralMultiplicity::Affine
                                        && source.access == StructuralAccess::Owned
                                })
                                || self.owned_result_source(
                                    argument.place,
                                    root.structural_type,
                                    StructuralMultiplicity::Affine,
                                );
                        if !source_matches
                            || !matches!(
                                parameter.multiplicity,
                                StructuralMultiplicity::Affine | StructuralMultiplicity::Linear
                            )
                        {
                            return Err(malformed());
                        }
                        let selected_claims = crate::custody::projected_ownership_claims(
                            &self.live_claims,
                            argument,
                        )?;
                        let claims_match = match parameter.multiplicity {
                            StructuralMultiplicity::Linear => matches!(selected_claims.as_slice(),
                                [(_, claim)] if claim.path.is_empty()
                                    && claim.multiplicity == Some(StructuralMultiplicity::Linear)),
                            StructuralMultiplicity::Affine => selected_claims
                                .iter()
                                .all(|(_, claim)| !claim.path.is_empty()),
                            StructuralMultiplicity::Unrestricted => false,
                        };
                        if !claims_match || owned_sources.contains(&argument.place) {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        for (identity, mut claim) in selected_claims {
                            claim.place = Some(parameter.place);
                            if claims.insert(identity, claim).is_some() {
                                return Err(TerminalInterpretError::ClaimTransferMismatch);
                            }
                        }
                        projected_sources.insert(argument.place);
                        projected_value = Some(view);
                    }
                }
                terminal_psi::reading_borrow_pair_pattern!()
                    if matches!(self.program.structural_types().get(&parameter.structural_type),
                        Some(declaration) if matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                            || declaration.shape.is_borrowed_byte_view()) =>
                {
                    // A descriptor lends its window, not its affine storage.
                    // The common argument binder below preserves exact backing
                    // and attenuates access. Its source debt remains here until
                    // the independently verified cleanup frontier discharges it.
                    if !terminal_semantics::view_descriptor_parameter(
                        parameter,
                        self.program.structural_types().values(),
                    ) || !self.block_parameter_is_declared(target, parameter)
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == parameter.place
                                || (entry.place == argument.place && !entry.path.is_empty())
                        })
                    {
                        return Err(malformed());
                    }
                    let (view, backing) =
                        self.present_structural_argument(argument, parameter.structural_type)?;
                    if let Some(backing) = backing {
                        if aggregate_backings
                            .insert(parameter.place, backing)
                            .is_some()
                        {
                            return Err(malformed());
                        }
                    }
                    if view.structural_type != parameter.structural_type {
                        return Err(TerminalInterpretError::StructuralArgumentType {
                            place: parameter.place,
                            expected: parameter.structural_type,
                            actual: view.structural_type,
                        });
                    }
                    projected_value = Some(view);
                }
                // A loan moves no custody, so it may present any aggregate,
                // including one holding service bindings or claims, as the
                // exact view its path selects: the destination names the
                // same referent a call parameter would. A shared loan grants
                // no write; an exclusive one forwards only the authority its
                // exact source declaration holds, checked below. Fused
                // bindings have no storage to snapshot: either borrowed access
                // forwards the same admitted occurrence.
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
                    if self
                        .program
                        .structural_types()
                        .get(&parameter.structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_composite_shape()
                                || matches!(
                                    declaration.shape,
                                    StructuralTypeShape::PrimitiveScalar(_)
                                        | StructuralTypeShape::BindingCarrier
                                )
                        }) =>
                {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || self.live_affine_discard_at(parameter.place).is_some()
                        || (parameter.access != StructuralAccess::SharedBorrow
                            && (!self.block_parameter_is_declared(target, parameter)
                                || !self.exclusive_place_source(argument.place, parameter.access)))
                    {
                        return Err(malformed());
                    }
                    // A borrowed place binds the exact projected view: the
                    // argument path extends the root's stored path and
                    // must resolve to the parameter's declared referent
                    // type — a plain record subtree or one canonical
                    // primitive-scalar leaf. The loan moves no custody —
                    // the root either stays untracked or is still held as
                    // one whole frontier entry.
                    let (view, backing) =
                        self.present_structural_argument(argument, parameter.structural_type)?;
                    if let Some(backing) = backing {
                        if aggregate_backings
                            .insert(parameter.place, backing)
                            .is_some()
                        {
                            return Err(malformed());
                        }
                    }
                    if view.structural_type != parameter.structural_type {
                        return Err(TerminalInterpretError::StructuralArgumentType {
                            place: parameter.place,
                            expected: parameter.structural_type,
                            actual: view.structural_type,
                        });
                    }
                    if self
                        .live_affine_frontier
                        .iter()
                        .any(|entry| entry.place == argument.place && !entry.path.is_empty())
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    projected_value = Some(view);
                }
                StructuralAccess::SharedBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || self.live_affine_discard_at(parameter.place).is_some()
                    {
                        return Err(malformed());
                    }
                    if matches!(self.program.structural_types().get(&parameter.structural_type), Some(declaration)
                                            if declaration.shape.is_borrowed_byte_carrier())
                    {
                        // Borrowed byte views join whole: the argument names
                        // an untracked view root with no projection.
                        if !argument.path.is_empty()
                            || self.live_affine_discard_at(argument.place).is_some()
                        {
                            return Err(malformed());
                        }
                    } else {
                        return Err(malformed());
                    }
                }
                StructuralAccess::MutableBorrow => {
                    if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || !matches!(self.program.structural_types().get(&parameter.structural_type), Some(declaration)
                                                    if declaration.shape.is_borrowed_byte_carrier())
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == argument.place || entry.place == parameter.place
                        })
                    {
                        return Err(malformed());
                    }
                    self.mutable_byte_sequence_storage(argument.place)?;
                }
                _ => return Err(malformed()),
            }
            if contains_payloads {
                descriptor_parameters.push(parameter.clone());
                descriptor_arguments.push(argument.clone());
            }
            resolved_arguments.push(projected_value.unwrap_or_else(|| value.clone()));
        }
        // A destination may already name a source of this simultaneous move,
        // but may not overwrite any independently held claim or owned value.
        for parameter in &block.structural_parameters {
            if self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(parameter.place))
                && !owned_sources.contains(&parameter.place)
            {
                return Err(malformed());
            }
        }
        // Existing descriptor-only edges reuse their original declaration and
        // argument slices. Only mixed representations need a selected roster.
        let descriptor_parameters = if contains_payloads {
            descriptor_parameters.as_slice()
        } else {
            &block.structural_parameters
        };
        let descriptor_arguments = if contains_payloads {
            descriptor_arguments.as_slice()
        } else {
            structural_arguments
        };
        // Prepared byte loans are already bound. Direct descriptor rebindings
        // and owned byte copies keep their exact checks; owned record fields
        // are keyed by runtime referent identity, not SSA place.
        let mut byte_sequences = if byte_loans.is_empty() {
            self.bind_byte_sequence_arguments(
                descriptor_parameters,
                descriptor_arguments,
                &resolved_arguments,
            )?
        } else {
            let mut parameters = Vec::new();
            let mut arguments = Vec::new();
            let mut values = Vec::new();
            for ((parameter, argument), value) in descriptor_parameters
                .iter()
                .zip(descriptor_arguments)
                .zip(&resolved_arguments)
            {
                if !byte_loans.contains_key(&parameter.place) {
                    parameters.push(parameter.clone());
                    arguments.push(argument.clone());
                    values.push(value.clone());
                }
            }
            self.bind_byte_sequence_arguments(&parameters, &arguments, &values)?
        };
        for (place, binding) in byte_loans {
            if byte_sequences.insert(place, binding).is_some() {
                return Err(malformed());
            }
        }
        // An owned byte copy was just checked against every domain its formal
        // names, so the destination holds the copy under those domains, exactly
        // as an ordinary call's owned copy.
        for (parameter, value) in descriptor_parameters.iter().zip(&mut resolved_arguments) {
            if parameter.access == StructuralAccess::Owned
                && byte_sequences.contains_key(&parameter.place)
            {
                value.absorb_qualifications(&parameter.qualifications);
            }
        }
        let mut structural = bind_active_structural_arguments(
            descriptor_parameters,
            &resolved_arguments,
            &self.program.structural_types(),
            &self.structural_cases,
        )?;
        let mut bound_claims = BTreeMap::new();
        for parameter in descriptor_parameters {
            let catalogue = machine
                .block_entry_claims
                .iter()
                .filter(|binding| binding.input == parameter.place)
                .collect::<Vec<_>>();
            let mut active = Vec::new();
            for binding in &catalogue {
                if crate::structural_inputs::case_membership::case_projection_is_active(
                    &self.program.structural_types(),
                    &structural,
                    &self.structural_cases,
                    parameter.place,
                    &binding.path,
                )? {
                    active.push(*binding);
                }
            }
            for (identity, claim) in claims
                .iter()
                .filter(|(_, claim)| claim.place == Some(parameter.place))
            {
                let destination = if catalogue.is_empty() {
                    *identity
                } else {
                    let matching = active
                        .iter()
                        .position(|binding| binding.path == claim.path)
                        .ok_or(TerminalInterpretError::ClaimTransferMismatch)?;
                    let binding = active.swap_remove(matching);
                    let expected_multiplicity = if binding.path.is_empty() {
                        parameter.multiplicity
                    } else {
                        StructuralMultiplicity::Linear
                    };
                    if claim.multiplicity != Some(expected_multiplicity) {
                        return Err(TerminalInterpretError::ClaimTransferMismatch);
                    }
                    binding.claim
                };
                if bound_claims.insert(destination, claim.clone()).is_some() {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            if !active.is_empty() {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        if bound_claims.len() != claims.len() {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let claims = bound_claims;
        let mut affine_destinations = bind_affine_frontier(
            descriptor_parameters,
            &structural,
            &self.program.structural_types(),
        )?;
        // A whole affine value arrives with the claims it carried; its new
        // debt is the same complement of those linear regions.
        crate::custody::exclude_linear_regions(
            &self.program.structural_types(),
            &mut affine_destinations,
            claims
                .values()
                .filter_map(|claim| claim.place.map(|place| (place, claim.path.as_slice()))),
            &|current| self.stored_case_at_discard(&structural, current),
        )?;
        let mut element_views = self.bind_element_view_arguments(
            descriptor_parameters,
            descriptor_arguments,
            &resolved_arguments,
        )?;
        for (place, loan) in array_loans {
            if element_views.insert(place, loan).is_some() {
                return Err(malformed());
            }
        }
        let mut aggregate_payload = self.stage_owned_value_arguments(
            descriptor_parameters,
            &mut structural,
            &mut identity_cursor,
        )?;
        for (parameter, argument, value) in packed_arguments {
            let identity = self
                .local_structural_identities
                .allocate_staged(&mut identity_cursor)?;
            let captured =
                TerminalStructuralValue::minted(identity, parameter.structural_type, Vec::new());
            aggregate_payload.stage(
                self,
                argument,
                &value,
                &crate::values::StructuralRuntimePlace::from(&captured),
                false,
            )?;
            if structural.insert(parameter.place, captured).is_some() {
                return Err(malformed());
            }
        }
        Ok(BlockBindings {
            scalars,
            structural,
            aggregate_backings,
            byte_sequences,
            primitive_loans,
            element_views,
            owned_sources,
            claims,
            projected_sources,
            affine_destinations,
            aggregate_payload,
            identity_cursor,
        })
    }

    /// Exclusive forwarding preserves the current referent and its source debt.
    /// Runtime identities alone carry no permission: recover it from the exact
    /// current parameter or independently admitted owned result declaration.
    fn exclusive_place_source(&self, place: PlaceId, requested: StructuralAccess) -> bool {
        let Some(value) = self.structural_values.get(&place) else {
            return false;
        };
        let multiplicity = if let Some(source) = self.owned_block_source(place) {
            if source.structural_type != value.structural_type
                || !crate::reference::access_covers(source.access, requested)
                || (source.access != StructuralAccess::Owned
                    && source.multiplicity != StructuralMultiplicity::Unrestricted)
            {
                return false;
            }
            source.multiplicity
        } else if self.owned_result_source(
            place,
            value.structural_type,
            StructuralMultiplicity::Affine,
        ) {
            StructuralMultiplicity::Affine
        } else if self.owned_result_source(
            place,
            value.structural_type,
            StructuralMultiplicity::Unrestricted,
        ) {
            StructuralMultiplicity::Unrestricted
        } else {
            return false;
        };
        let root = StructuralAffineDiscard {
            place,
            path: Vec::new(),
            structural_type: value.structural_type,
        };
        let expected = match multiplicity {
            StructuralMultiplicity::Unrestricted => None,
            StructuralMultiplicity::Affine => Some(&root),
            _ => return false,
        };
        self.live_affine_frontier
            .iter()
            .filter(|entry| entry.place == place)
            .eq(expected)
    }

    fn block_parameter_is_declared(
        &self,
        target: BlockId,
        parameter: &StructuralParameterDeclaration,
    ) -> bool {
        self.owned_block_source(parameter.place) == Some(parameter)
            && self
                .program
                .machines()
                .get(&self.current_machine)
                .is_some_and(|machine| {
                    machine.structural_places.iter().any(|declaration| {
                        declaration.id == parameter.place
                            && declaration.kind
                                == StructuralPlaceKind::BlockParameter {
                                    block: target,
                                    position: parameter.position,
                                }
                    })
                })
    }

    pub(crate) fn owned_result_matches(
        &self,
        place: PlaceId,
        parameter: &StructuralParameterDeclaration,
    ) -> bool {
        self.owned_result_source(place, parameter.structural_type, parameter.multiplicity)
    }

    /// A projected argument's source is checked against the root's declared
    /// type and affine ownership, not the parameter's leaf type.
    fn owned_result_source(
        &self,
        place: PlaceId,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> bool {
        let Some(machine) = self.program.machines().get(&self.current_machine) else {
            return false;
        };
        let Some(declaration) = machine.structural_place(place) else {
            return false;
        };
        // A declared trivial affine local is an exact owned source for its
        // empty-record type: the establishment op carries custody and the
        // declaration carries the type, so no producer row exists.
        if let StructuralPlaceKind::TrivialAffineLocal {
            structural_type: declared,
            ..
        } = declaration.kind
        {
            return declared == structural_type && multiplicity == StructuralMultiplicity::Affine;
        }
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type: declared,
        } = declaration.kind
        else {
            return false;
        };
        // A successful operation publishes one owned result declaration.
        // The runtime carrier and the frontier above establish its current
        // custody; surrounding constructor/call/copy syntax is irrelevant.
        // Descriptor custody is ownership of that value, not of its backing.
        // The edge's runtime payload and loan roster must travel with it; a
        // plain-shape predicate cannot decide whether its declaration is owned.
        declared == structural_type
            && machine.operations().any(|operation| {
                operation.id == producer
                    && operation.result.structural().is_some_and(|result| {
                        result.place == place
                            && result.structural_type == structural_type
                            && result.multiplicity == multiplicity
                    })
            })
    }

    pub(crate) fn owned_block_source(
        &self,
        place: PlaceId,
    ) -> Option<&StructuralParameterDeclaration> {
        let machine = self.program.machines().get(&self.current_machine)?;
        let declaration = machine.structural_place(place)?;
        match declaration.kind {
            StructuralPlaceKind::Parameter { position } => machine
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == place && parameter.position == position),
            StructuralPlaceKind::BlockParameter { block, position } => machine
                .block(&block)?
                .structural_parameters
                .get(position as usize)
                .filter(|parameter| parameter.place == place && parameter.position == position),
            _ => None,
        }
    }
}

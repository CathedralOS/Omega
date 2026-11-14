/*
    Block terminators, simultaneous edge bindings, nominal cleanup schedules,
    and structural return transport in the Terminal Psi engine.

    We capture outgoing edge operands before running disposal schedules, install
    successor block parameters simultaneously, and validate return preflight
    before restoring caller frames.
*/

pub(crate) mod block_bindings {
    use crate::boundary::structural_inputs::case_membership::{
        case_projection_with_checked_membership_is_active, stored_case_at_projection,
    };
    use crate::engine::custody::{
        bind_active_retained_arguments, bind_active_structural_arguments, bind_affine_frontier,
        bind_arguments, projected_ownership_claims, resolve_single_structural_argument,
        validate_nominal_cleanup_action, verified_structural_place, whole_path_discard,
    };
    use crate::storage::runtime_rows::RuntimeRows;
    use crate::storage::scalar_storage::ScalarCell;
    use std::collections::{BTreeMap, BTreeSet};

    use semantic_vocabulary::{
        BlockId, ClaimId, PlaceId, StructuralPlaceKind, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeShape,
    };

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
    use crate::engine::execution::{LiveClaim, TerminalExecution};
    use crate::storage::byte_sequences::binding::ByteSequenceBinding;
    use terminal_semantics::{borrowed_storage_parameter, view_descriptor_parameter};

    pub(crate) struct BlockBindings {
        scalars: RuntimeRows<ValueId, TerminalScalarValue>,
        structural: RuntimeRows<PlaceId, TerminalStructuralValue>,
        aggregate_backings: RuntimeRows<PlaceId, crate::storage::reference::AggregateBacking>,
        byte_sequences: RuntimeRows<PlaceId, ByteSequenceBinding>,
        primitive_loans: RuntimeRows<PlaceId, crate::storage::primitive_storage::PrimitiveLoan>,
        element_views: RuntimeRows<PlaceId, crate::storage::element_views::ElementView>,
        owned_sources: BTreeSet<PlaceId>,
        claims: BTreeMap<ClaimId, LiveClaim>,
        pub(crate) projected_sources: BTreeSet<PlaceId>,
        affine_destinations: BTreeSet<StructuralAffineDiscard>,
        aggregate_payload: crate::storage::record::storage::ConstructorStorage,
        identity_cursor: Option<u64>,
    }

    impl Default for BlockBindings {
        fn default() -> Self {
            Self {
                scalars: RuntimeRows::new(None),
                structural: RuntimeRows::new(None),
                aggregate_backings: RuntimeRows::new(None),
                byte_sequences: RuntimeRows::new(None),
                primitive_loans: RuntimeRows::new(None),
                element_views: RuntimeRows::new(None),
                owned_sources: BTreeSet::new(),
                claims: BTreeMap::new(),
                projected_sources: BTreeSet::new(),
                affine_destinations: BTreeSet::new(),
                aggregate_payload: crate::storage::record::storage::ConstructorStorage::new(None),
                identity_cursor: None,
            }
        }
    }

    impl BlockBindings {
        pub(crate) fn retain_storage(
            &self,
            retained: &mut crate::storage::structural_storage::RetainedStorage,
        ) {
            for value in self.structural.values() {
                retained.value(value);
            }
            for backing in self.aggregate_backings.values() {
                retained.aggregate_backing(backing);
            }
            for view in self.element_views.values() {
                view.retain_storage(retained);
            }
            for loan in self.primitive_loans.values() {
                retained.primitive_endpoint(&loan.storage.endpoint);
            }
            for binding in self.byte_sequences.values() {
                retained.byte_binding(binding);
            }
            self.aggregate_payload.retain_storage(retained);
        }

        pub(crate) fn reserve_identities(&self, execution: &mut TerminalExecution) {
            execution
                .local_structural_identities
                .commit_cursor(self.identity_cursor);
        }

        pub(crate) fn commit(
            mut self,
            execution: &mut TerminalExecution,
            target: BlockId,
        ) -> Result<(), TerminalInterpretError> {
            for source in &self.owned_sources {
                execution
                    .live_claims
                    .retain(|_, claim| claim.place != Some(*source));
                if let Some(retired) = execution.structural_values.remove(source) {
                    let origin = execution
                        .reference_referents
                        .get(&crate::boundary::values::StructuralRuntimePlace::from(
                            &retired,
                        ))
                        .cloned();
                    execution
                        .retired_structural_homes
                        .push((*source, retired, origin));
                }
                execution.aggregate_argument_backings.remove(source);
                execution.byte_sequence_values.remove(source);
                execution.scalar_array_values.remove(source);
                execution.element_view_values.remove(source);
                execution.primitive_loan_values.remove(source);
                execution
                    .live_affine_frontier
                    .retain(|entry| entry.place != *source);
            }
            execution.retire_reentered_results(target)?;
            for destination in self.structural.keys() {
                if let Some(previous) = execution.structural_values.remove(destination) {
                    let origin = execution
                        .reference_referents
                        .get(&crate::boundary::values::StructuralRuntimePlace::from(
                            &previous,
                        ))
                        .cloned();
                    execution.retired_structural_homes.push((
                        *destination,
                        previous.clone(),
                        origin,
                    ));
                    execution.queue_retired_descriptor_home((&previous).into());
                }
                execution.aggregate_argument_backings.remove(destination);
                execution.scalar_array_values.remove(destination);
                execution.primitive_loan_values.remove(destination);
                execution.element_view_values.remove(destination);
                execution.byte_sequence_values.remove(destination);
            }
            execution.collect_retired_successor_storage(&self);
            if self
                .structural
                .keys()
                .any(|place| execution.scalar_array_values.retains_retired(place))
            {
                return Err(malformed());
            }
            self.aggregate_payload.publish(execution)?;
            execution.live_claims.extend(self.claims);
            execution.values.append(&mut self.scalars)?;
            execution.structural_values.append(&mut self.structural)?;
            execution
                .aggregate_argument_backings
                .append(&mut self.aggregate_backings)?;
            execution
                .byte_sequence_values
                .append(&mut self.byte_sequences)?;
            execution
                .primitive_loan_values
                .append(&mut self.primitive_loans)?;
            execution
                .element_view_values
                .append(&mut self.element_views)?;
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
        ) -> Result<Vec<terminal_psi::TerminalAffineCleanupAction>, TerminalInterpretError>
        {
            use terminal_psi::TerminalAffineCleanupAction;
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
            let mut values = execution.structural_values.try_clone()?;
            for (place, home) in execution.linear_homes.iter() {
                if values.insert(*place, home.clone())?.is_some() {
                    return Err(malformed());
                }
            }
            let mut scalar_arrays: BTreeSet<_> =
                execution.scalar_array_values.keys().copied().collect();
            let mut frontier = execution.live_affine_frontier.clone();
            let mut claims = execution.live_claims.clone();
            for argument in structural_arguments.iter().filter(|argument| {
                argument.access == StructuralAccess::Owned
                    && !argument.path.is_empty()
                    && self.projected_sources.contains(&argument.place)
            }) {
                crate::engine::custody::consume_projected_ownership(
                    &execution.structural_types(),
                    &values,
                    execution.structural_cases.as_map(),
                    &mut frontier,
                    &mut claims,
                    argument,
                )?;
            }
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
                discarded.insert(place);
                match action {
                    TerminalAffineCleanupAction::DiscardRoot(_) => {
                        let structural_type =
                            verified_structural_place(&execution.structural_values, place)
                                .map(|value| value.structural_type)?;
                        if crate::engine::custody::place_is_claimed(&claims, place)
                            || frontier
                                .iter()
                                .filter(|entry| entry.place == place)
                                .ne(std::iter::once(&whole_path_discard(place, structural_type)))
                        {
                            return Err(TerminalInterpretError::AffineFrontierMismatch);
                        }
                        crate::engine::custody::remove_affine_root(&mut frontier, place);
                        values.remove(&place);
                    }
                    TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                        validate_nominal_cleanup_action(
                            &execution.structural_types(),
                            execution.structural_cases.as_map(),
                            &execution.program.machines(),
                            &values,
                            &frontier,
                            &claims,
                            cleanup,
                        )?;
                        crate::engine::custody::begin_nominal_cleanup(
                            &execution.structural_types(),
                            &values,
                            execution.structural_cases.as_map(),
                            &mut frontier,
                            &mut claims,
                            cleanup,
                        )?;
                        crate::engine::custody::complete_nominal_cleanup(
                            &mut values,
                            &mut scalar_arrays,
                            &mut frontier,
                            &claims,
                            cleanup,
                        )?;
                    }
                    TerminalAffineCleanupAction::DiscardResidual(discard) => {
                        crate::engine::custody::consume_affine_discard(
                            &execution.structural_types(),
                            &values,
                            execution.structural_cases.as_map(),
                            &mut frontier,
                            &claims,
                            discard,
                        )?;
                    }
                    TerminalAffineCleanupAction::RetireHome(_) => {
                        crate::engine::custody::retire_cleanup_home(
                            &mut values,
                            &mut scalar_arrays,
                            &mut frontier,
                            &claims,
                            place,
                        )?;
                    }
                }
            }
            if self.structural.keys().any(|place| {
                execution.live_affine_discard_at(*place).is_some()
                    && !self.owned_sources.contains(place)
                    && !self.projected_sources.contains(place)
                    && !discarded.contains(place)
            }) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
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
        pub(crate) fn prepare_block_bindings(
            &self,
            target: BlockId,
            scalar_arguments: &[TerminalScalarValue],
            structural_arguments: &[StructuralArgument],
        ) -> Result<BlockBindings, TerminalInterpretError> {
            let machine = self.program.verified_machine(self.current_machine)?;
            let block = machine
                .block(&target)
                .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
            let scalars =
                bind_arguments(&block.parameters, scalar_arguments, self.account.as_ref())?;
            if block.structural_parameters.len() != structural_arguments.len() {
                return Err(TerminalInterpretError::StructuralArgumentCount {
                    expected: block.structural_parameters.len(),
                    actual: structural_arguments.len(),
                });
            }
            let mut resolved_arguments = Vec::with_capacity(structural_arguments.len());
            let mut aggregate_backings = RuntimeRows::new(self.account.as_ref());
            let contains_payloads = structural_arguments
                .iter()
                .any(|argument| self.scalar_array_values.contains_key(&argument.place));
            let mut descriptor_parameters = Vec::new();
            let mut descriptor_arguments = Vec::new();
            let mut packed_arguments = Vec::new();
            let mut primitive_loans = RuntimeRows::new(self.account.as_ref());
            let mut array_loans = RuntimeRows::new(self.account.as_ref());
            let mut byte_loans = RuntimeRows::new(self.account.as_ref());
            let mut local_array_roots = RuntimeRows::new(self.account.as_ref());
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
                self.require_owned_value_claims(argument)?;
                if usize::try_from(parameter.position).ok() != Some(position)
                    || argument.access != parameter.access
                {
                    return Err(malformed());
                }
                if parameter.access != StructuralAccess::Owned
                    && matches!(
                        self.structural_declaration(parameter.structural_type)?
                            .shape,
                        StructuralTypeShape::PrimitiveScalar(_)
                    )
                {
                    if !self.block_parameter_is_declared(target, parameter) {
                        return Err(malformed());
                    }
                    let loan = self
                        .capture_primitive_loan(parameter, argument, &mut identity_cursor)
                        .map_err(|error| {
                            error.in_argument_preparation("capture primitive state arrival")
                        })?;
                    resolved_arguments.push(loan.referent.clone());
                    primitive_loans.insert(parameter.place, loan)?;
                    if contains_payloads {
                        descriptor_parameters.push(parameter.clone());
                        descriptor_arguments.push(argument.clone());
                    }
                    continue;
                }
                if let Some((referent, loan)) = self
                    .prepare_scalar_array_loan(
                        parameter,
                        argument,
                        &mut identity_cursor,
                        &mut local_array_roots,
                    )
                    .map_err(|error| error.in_argument_preparation("capture array state arrival"))?
                {
                    if !self.block_parameter_is_declared(target, parameter)
                        || self.live_affine_discard_at(parameter.place).is_some()
                        || array_loans.insert(parameter.place, loan)?.is_some()
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
                if parameter.access != StructuralAccess::Owned
                    && matches!(
                        self.structural_declaration(parameter.structural_type)?
                            .shape,
                        StructuralTypeShape::ByteSequence(_)
                    )
                    && let Some((referent, binding)) = self
                        .prepare_byte_sequence_loan(
                            parameter,
                            argument,
                            &mut identity_cursor,
                            &mut local_array_roots,
                        )
                        .map_err(|error| {
                            error.in_argument_preparation("capture byte state arrival")
                        })?
                {
                    if !self.block_parameter_is_declared(target, parameter)
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == parameter.place
                                || (entry.place == argument.place && !entry.path.is_empty())
                        })
                        || byte_loans.insert(parameter.place, binding)?.is_some()
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
                    let source_matches = self.owned_source_for(
                        argument.place,
                        root.structural_type,
                        StructuralMultiplicity::Unrestricted,
                    );
                    let value =
                        crate::storage::record::storage::ConstructorValue::resolve(self, argument)?;
                    if !self.block_parameter_is_declared(target, parameter)
                        || !source_matches
                        || parameter.access != StructuralAccess::Owned
                        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        || parameter.structural_type != value.structural_type()
                        || machine
                            .block_entry_claims
                            .iter()
                            .any(|claim| claim.input == parameter.place)
                        || self.live_claims.values().any(|claim| {
                            claim.place == Some(argument.place)
                                || claim.place == Some(parameter.place)
                        })
                        || self.live_affine_frontier.iter().any(|entry| {
                            entry.place == argument.place || entry.place == parameter.place
                        })
                    {
                        return Err(malformed());
                    }
                    packed_arguments.push((parameter, argument, value));
                    continue;
                }
                let value = verified_structural_place(&self.structural_values, argument.place)?;
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
                                &self.structural_types(),
                                &self.structural_values,
                                argument,
                            )?;
                            let selected = self.present_owned_descriptor(
                                selected,
                                parameter.structural_type,
                                parameter.multiplicity,
                            )?;
                            let source_matches = self.owned_source_for(
                                argument.place,
                                value.structural_type,
                                parameter.multiplicity,
                            );
                            if !source_matches {
                                return Err(malformed());
                            }
                            let carried = crate::engine::custody::whole_value_frontier(
                                &self.structural_types(),
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
                            if parameter.multiplicity == StructuralMultiplicity::Linear
                                && count == 0
                            {
                                return Err(malformed());
                            }
                            projected_value = Some(selected);
                        } else {
                            let root = value;
                            let view = resolve_single_structural_argument(
                                &self.structural_types(),
                                &self.structural_values,
                                argument,
                            )?;
                            if view.structural_type != parameter.structural_type {
                                return Err(TerminalInterpretError::StructuralArgumentType {
                                    place: parameter.place,
                                    expected: parameter.structural_type,
                                    actual: view.structural_type,
                                });
                            }
                            let source_matches = self.owned_source_for(
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
                            let selected_claims =
                                projected_ownership_claims(&self.live_claims, argument)?;
                            let claims_match = match parameter.multiplicity {
                                StructuralMultiplicity::Linear => {
                                    matches!(selected_claims.as_slice(),
                                [(_, claim)] if claim.path.is_empty()
                                    && claim.multiplicity == Some(StructuralMultiplicity::Linear))
                                }
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
                        if view_descriptor_parameter(
                            parameter,
                            self.structural_types().values(),
                        ) =>
                    {
                        if !view_descriptor_parameter(parameter, self.structural_types().values())
                            || !self.block_parameter_is_declared(target, parameter)
                            || self.live_affine_frontier.iter().any(|entry| {
                                entry.place == parameter.place
                                    || (entry.place == argument.place && !entry.path.is_empty())
                            })
                        {
                            return Err(malformed());
                        }
                        projected_value = Some(self.install_presented_descriptor(
                            argument,
                            parameter,
                            &mut aggregate_backings,
                        )?);
                    }
                    StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
                        if borrowed_storage_parameter(
                            parameter,
                            self.structural_types().values(),
                        ) =>
                    {
                        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                            || self.live_affine_discard_at(parameter.place).is_some()
                            || !self.borrowed_argument_source(argument, parameter.access)
                            || (parameter.access != StructuralAccess::SharedBorrow
                                && !self.block_parameter_is_declared(target, parameter))
                        {
                            return Err(malformed());
                        }
                        projected_value = Some(self.install_presented_descriptor(
                            argument,
                            parameter,
                            &mut aggregate_backings,
                        )?);
                    }
                    StructuralAccess::SharedBorrow => {
                        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
                            || self.live_affine_discard_at(parameter.place).is_some()
                        {
                            return Err(malformed());
                        }
                        if matches!(self.structural_types().get(&parameter.structural_type), Some(declaration)
                                            if declaration.shape.is_borrowed_byte_carrier())
                        {
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
                            || !matches!(self.structural_types().get(&parameter.structural_type), Some(declaration)
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
            for parameter in &block.structural_parameters {
                if crate::engine::custody::place_is_claimed(&self.live_claims, parameter.place)
                    && !owned_sources.contains(&parameter.place)
                {
                    return Err(malformed());
                }
            }
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
            let mut byte_sequences = if byte_loans.is_empty() {
                self.bind_byte_sequence_arguments(
                    descriptor_parameters,
                    descriptor_arguments,
                    &resolved_arguments,
                )
                .map_err(|error| error.in_argument_preparation("bind byte state arrivals"))?
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
                self.bind_byte_sequence_arguments(&parameters, &arguments, &values)
                    .map_err(|error| error.in_argument_preparation("bind byte state arrivals"))?
            };
            if byte_loans
                .keys()
                .any(|place| byte_sequences.contains_key(place))
            {
                return Err(malformed());
            }
            byte_sequences.append(&mut byte_loans)?;
            for (parameter, value) in descriptor_parameters.iter().zip(&mut resolved_arguments) {
                if parameter.access == StructuralAccess::Owned
                    && byte_sequences.contains_key(&parameter.place)
                {
                    value.absorb_qualifications(&parameter.qualifications);
                }
            }
            let provenance = self.structural_argument_provenance(descriptor_arguments)?;
            let mut structural = bind_active_retained_arguments(
                descriptor_parameters,
                descriptor_arguments,
                &resolved_arguments,
                &provenance,
                &|argument_index, rows| {
                    let source = &descriptor_arguments[argument_index];
                    let mut active = Vec::new();
                    for row in rows {
                        let is_active = case_projection_with_checked_membership_is_active(
                        &self.structural_types(),
                        resolved_arguments[argument_index].structural_type,
                        &row.path,
                        &|prefix| {
                            // Conditions name the selected value, while a Referent
                            // crosses the original descriptor's captured backing.
                            // Resolve before cleanup or destination rebinding; this
                            // metadata query grants no payload read/write access.
                            let mut selected = Vec::with_capacity(source.path.len() + prefix.len());
                            selected.extend_from_slice(&source.path);
                            selected.extend_from_slice(prefix);
                            let carrier = self.resolve_case_guard_carrier(source.place, &selected)?;
                            Ok(stored_case_at_projection(
                                &self.structural_types(),
                                &carrier,
                                self.structural_cases.as_map(),
                                &[],
                            ))
                        },
                        )?;
                        if is_active {
                            active.push(row.clone());
                        }
                    }
                    Ok(active)
                },
                &self.structural_types(),
                self.account.as_ref(),
            )
            .map_err(|error| {
            if std::env::var_os("OMEGA_STATE_REFERENCE_DIAGNOSTICS").is_some() {
                for (((parameter, source), value), origin) in descriptor_parameters
                    .iter()
                    .zip(descriptor_arguments)
                    .zip(&resolved_arguments)
                    .zip(&provenance)
                {
                    eprintln!(
                        "STATE arrival refused: error={error:?} parameter={parameter:?} source={source:?} value={value:?} source_parameter={:?} reference_projection={:?}",
                        origin.parameter,
                        origin.reference_projection,
                    );
                }
            }
            error.in_argument_preparation("bind captured state arrivals")
        })?;
            let mut bound_claims = BTreeMap::new();
            for parameter in descriptor_parameters {
                let catalogue = machine
                    .block_entry_claims
                    .iter()
                    .filter(|binding| binding.input == parameter.place)
                    .collect::<Vec<_>>();
                let mut active = Vec::new();
                for binding in &catalogue {
                    if crate::boundary::structural_inputs::case_membership::case_projection_is_active(
                    &self.structural_types(),
                    &structural,
                    self.structural_cases.as_map(),
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
                        let expected_multiplicity =
                            LiveClaim::path_multiplicity(&binding.path, parameter.multiplicity);
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
            let mut affine_destinations =
                bind_affine_frontier(descriptor_parameters, &structural, &self.structural_types())?;
            crate::engine::custody::exclude_linear_regions(
                &self.structural_types(),
                &mut affine_destinations,
                claims
                    .values()
                    .filter_map(|claim| claim.place.map(|place| (place, claim.path.as_slice()))),
                &|current| self.stored_case_at_discard(&structural, current),
            )?;
            let mut element_views = self
                .bind_element_view_arguments(
                    descriptor_parameters,
                    descriptor_arguments,
                    &resolved_arguments,
                )
                .map_err(|error| error.in_argument_preparation("bind element state arrivals"))?;
            if array_loans
                .keys()
                .any(|place| element_views.contains_key(place))
            {
                return Err(malformed());
            }
            element_views.append(&mut array_loans)?;
            let mut aggregate_payload = self
                .stage_owned_value_arguments(
                    descriptor_parameters,
                    &mut structural,
                    &mut identity_cursor,
                )
                .map_err(|error| error.in_argument_preparation("stage owned state arrivals"))?;
            for (parameter, argument, value) in packed_arguments {
                let identity = self
                    .local_structural_identities
                    .allocate_staged(&mut identity_cursor)?;
                let (whole, projected) = value.memberships();
                let mut captured = TerminalStructuralValue::minted(
                    identity,
                    parameter.structural_type,
                    whole.to_vec(),
                );
                captured.projected_qualifications = projected.to_vec();
                if let crate::storage::record::storage::ConstructorValue::PackedScalars {
                    elements,
                    ..
                } = &value
                {
                    self.validate_scalar_array_contents_qualifications(
                        parameter.structural_type,
                        elements.iter().map(ScalarCell::scalar_type),
                        &parameter.qualifications,
                    )?;
                    captured.absorb_qualifications(&parameter.qualifications);
                }
                let admitted = bind_active_structural_arguments(
                    std::slice::from_ref(parameter),
                    std::slice::from_ref(&captured),
                    &self.structural_types(),
                    self.structural_cases.as_map(),
                    self.account.as_ref(),
                )?;
                let captured = admitted
                    .get(&parameter.place)
                    .cloned()
                    .ok_or_else(malformed)?;
                aggregate_payload.stage(
                    self,
                    argument,
                    &value,
                    &crate::boundary::values::StructuralRuntimePlace::from(&captured),
                    false,
                )?;
                if structural.insert(parameter.place, captured)?.is_some() {
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

        fn borrowed_argument_source(
            &self,
            argument: &StructuralArgument,
            requested: StructuralAccess,
        ) -> bool {
            let place = argument.place;
            let Some(value) = self.structural_values.get(&place) else {
                return false;
            };
            let multiplicity = if let Some(source) = self.owned_block_source(place) {
                if source.structural_type != value.structural_type
                    || !crate::storage::reference::access_covers(source.access, requested)
                    || (source.access != StructuralAccess::Owned
                        && source.multiplicity != StructuralMultiplicity::Unrestricted)
                {
                    return false;
                }
                source.multiplicity
            } else if self.owned_source_for(
                place,
                value.structural_type,
                StructuralMultiplicity::Linear,
            ) {
                StructuralMultiplicity::Linear
            } else if self.owned_source_for(
                place,
                value.structural_type,
                StructuralMultiplicity::Affine,
            ) {
                StructuralMultiplicity::Affine
            } else if self.owned_source_for(
                place,
                value.structural_type,
                StructuralMultiplicity::Unrestricted,
            ) {
                StructuralMultiplicity::Unrestricted
            } else {
                return false;
            };
            match multiplicity {
                StructuralMultiplicity::Unrestricted => self
                    .live_affine_frontier
                    .iter()
                    .all(|entry| entry.place != place),
                StructuralMultiplicity::Affine | StructuralMultiplicity::Linear => self
                    .reference_carrier_access(place, value.structural_type, &argument.path)
                    .is_ok_and(|access| {
                        crate::storage::reference::access_covers(access, requested)
                    }),
            }
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

        pub(crate) fn owned_source_for(
            &self,
            place: PlaceId,
            structural_type: StructuralTypeId,
            multiplicity: StructuralMultiplicity,
        ) -> bool {
            self.owned_block_source(place).is_some_and(|source| {
                source.structural_type == structural_type
                    && source.access == StructuralAccess::Owned
                    && source.multiplicity == multiplicity
            }) || self.owned_result_source(place, structural_type, multiplicity)
        }

        fn install_presented_descriptor(
            &self,
            argument: &StructuralArgument,
            parameter: &StructuralParameterDeclaration,
            aggregate_backings: &mut RuntimeRows<
                PlaceId,
                crate::storage::reference::AggregateBacking,
            >,
        ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
            let (view, backing) =
                self.present_structural_argument(argument, parameter.structural_type)?;
            if let Some(backing) = backing
                && aggregate_backings
                    .insert(parameter.place, backing)?
                    .is_some()
            {
                return Err(malformed());
            }
            if view.structural_type != parameter.structural_type {
                return Err(TerminalInterpretError::StructuralArgumentType {
                    place: parameter.place,
                    expected: parameter.structural_type,
                    actual: view.structural_type,
                });
            }
            Ok(view)
        }

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
            if let StructuralPlaceKind::TrivialAffineLocal {
                structural_type: declared,
                ..
            } = declaration.kind
            {
                return declared == structural_type
                    && multiplicity == StructuralMultiplicity::Affine;
            }
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type: declared,
            } = declaration.kind
            else {
                return false;
            };
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
}
pub(crate) mod completion {
    use super::*;
    use crate::boundary::errors::malformed;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::execution::FailedReturn;
    use terminal_psi::{NominalAffineCleanup, TerminalAffineCleanupAction};

    impl TerminalExecution<'_> {
        pub(crate) fn reject_no_normal_return(&self) -> Result<(), TerminalInterpretError> {
            if let Some(SuspendedCall {
                result: SuspendedCallResult::NoNormal { callee, edge },
                ..
            }) = self.call_stack.last()
            {
                return Err(TerminalInterpretError::NoNormalInvocationReturned {
                    callee: *callee,
                    edge: *edge,
                });
            }
            Ok(())
        }

        fn retire_home_companions(&mut self, place: PlaceId) {
            self.aggregate_argument_backings.remove(&place);
            self.byte_sequence_values.remove(&place);
            self.element_view_values.remove(&place);
            self.primitive_loan_values.remove(&place);
        }

        pub(crate) fn continue_cleanup_actions(
            &mut self,
            mut remaining: std::vec::IntoIter<TerminalAffineCleanupAction>,
            continuation: CleanupContinuation,
        ) -> Result<TerminatorFlow, TerminalInterpretError> {
            while let Some(action) = remaining.next() {
                self.collect_retired_descriptor_storage(Some(&continuation));
                let retired = match &action {
                    TerminalAffineCleanupAction::RetireHome(place) => self
                        .structural_values
                        .get(place)
                        .or_else(|| self.linear_homes.get(place))
                        .cloned(),
                    _ => None,
                };
                let nominal = commit_cleanup_action(
                    &self.program.structural_types(),
                    self.structural_cases.as_map(),
                    &self.program.machines(),
                    self.program.verified_machine(self.current_machine)?,
                    &mut self.structural_values,
                    &mut self.linear_homes,
                    &mut self.scalar_array_values,
                    &mut self.reference_referents,
                    &mut self.structural_element_view_fields,
                    &mut self.live_affine_frontier,
                    &mut self.live_claims,
                    &mut self.retired_structural_homes,
                    &action,
                )?;
                if let TerminalAffineCleanupAction::RetireHome(place) = &action {
                    if let Some(retired) = retired {
                        let origin = self
                            .reference_referents
                            .get(&crate::boundary::values::StructuralRuntimePlace::from(
                                &retired,
                            ))
                            .cloned();
                        self.retired_structural_homes
                            .push((*place, retired.clone(), origin));
                        self.queue_retired_descriptor_home((&retired).into());
                    }
                    self.retire_home_companions(*place);
                }
                self.collect_retired_descriptor_storage(Some(&continuation));
                if let Some(completed) = nominal {
                    return self.start_nominal_cleanup(completed, remaining, continuation);
                }
            }
            if matches!(&continuation, CleanupContinuation::Return(_)) {
                if !self.live_affine_frontier.is_empty() || !self.live_claims.is_empty() {
                    return Err(TerminalInterpretError::UnsettledReturnFrontier {
                        affine: self.live_affine_frontier.iter().cloned().collect(),
                        claims: self.live_claims.keys().copied().collect(),
                    });
                }
                let retired: Vec<_> = self
                    .structural_values
                    .values()
                    .chain(self.linear_homes.values())
                    .map(Into::into)
                    .collect();
                self.retire_plain_locals();
                self.structural_values.clear();
                self.linear_homes.clear();
                for root in retired {
                    self.queue_retired_descriptor_home(root);
                }
                self.collect_retired_descriptor_storage(Some(&continuation));
            }
            match continuation {
                CleanupContinuation::Return(result) => self.finish_return(result),
                CleanupContinuation::Successor { target, bindings } => {
                    bindings.commit(self, target)?;
                    self.current = target;
                    self.next_operation = 0;
                    Ok(TerminatorFlow::Continue)
                }
            }
        }

        fn start_nominal_cleanup(
            &mut self,
            completed: (NominalAffineCleanup, TerminalStructuralValue),
            remaining: std::vec::IntoIter<TerminalAffineCleanupAction>,
            continuation: CleanupContinuation,
        ) -> Result<TerminatorFlow, TerminalInterpretError> {
            let callee = self.program.verified_machine(completed.0.cleanup_machine)?;
            let entry = callee.entry;
            let target = completed.0.cleanup_machine;
            let receiver = completed
                .0
                .cleanup_receiver
                .map(|place| (place, completed.1.clone()));
            self.suspend_with(
                SuspendedCallResult::NominalCleanups {
                    completed,
                    remaining,
                    continuation,
                },
                Vec::new(),
            )?;
            if let Some((place, value)) = receiver {
                self.structural_values.insert(place, value)?;
            }
            self.current_machine = target;
            self.current = entry;
            self.next_operation = 0;
            Ok(TerminatorFlow::Continue)
        }

        fn restore_return_caller(&mut self, caller: SuspendedCall) -> SuspendedCallResult {
            self.access_view_occurrences = caller.access_view_occurrences;
            self.values = caller.values;
            self.structural_values = caller.structural_values;
            self.linear_homes = caller.linear_homes;
            self.aggregate_argument_backings = caller.aggregate_argument_backings;
            self.scalar_array_values = caller.scalar_array_values;
            self.element_view_values = caller.element_view_values;
            self.byte_sequence_values = caller.byte_sequence_values;
            self.primitive_loan_values = caller.primitive_loan_values;
            self.live_affine_frontier = caller.live_affine_frontier;
            self.live_claims = caller.live_claims;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            caller.result
        }

        pub(crate) fn finish_return(
            &mut self,
            mut completed: CompletedReturn,
        ) -> Result<TerminatorFlow, TerminalInterpretError> {
            if let Err(error) = self.complete_activation_storage() {
                self.failed_return = Some(FailedReturn {
                    _completed: completed,
                    _expected: None,
                    _loan_completions: Vec::new(),
                });
                return Err(error);
            }
            let Some(mut caller) = self.call_stack.pop() else {
                if completed.destination.is_some() {
                    self.failed_return = Some(FailedReturn {
                        _completed: completed,
                        _expected: None,
                        _loan_completions: Vec::new(),
                    });
                    return Err(malformed());
                }
                if let Err(error) = self
                    .require_entry_results_complete()
                    .and_then(|()| self.require_scalar_entry_checks_complete())
                    .and_then(|()| self.charge_completed_result(&completed))
                {
                    self.failed_return = Some(FailedReturn {
                        _completed: completed,
                        _expected: None,
                        _loan_completions: Vec::new(),
                    });
                    return Err(error);
                }
                if let TerminalExecutionResult::Structural(returned) = &mut completed.value {
                    returned.observation_home = self.forward_host_atom(&mut returned.value);
                }
                self.returned_claims = completed.returned_claims;
                self.returned_byte_sequence = completed.byte_sequence;
                self.returned_element_view = completed.element_view;
                self.result = Some(completed.value.clone());
                return Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
                    completed.value,
                )));
            };
            let loan_completions = std::mem::take(&mut caller.loan_completions);
            let expected = self.restore_return_caller(caller);
            if let Err(error) = self.install_return(&mut completed, &expected, &loan_completions) {
                self.failed_return = Some(FailedReturn {
                    _completed: completed,
                    _expected: Some(expected),
                    _loan_completions: loan_completions,
                });
                return Err(error);
            }
            if let SuspendedCallResult::NominalCleanups {
                remaining,
                continuation,
                ..
            } = expected
            {
                return self.continue_cleanup_actions(remaining, continuation);
            }
            Ok(TerminatorFlow::Continue)
        }

        fn install_return(
            &mut self,
            completed: &mut CompletedReturn,
            expected: &SuspendedCallResult,
            loan_completions: &[terminal_psi::LoanCompletion],
        ) -> Result<(), TerminalInterpretError> {
            if let SuspendedCallResult::NoNormal { callee, edge } = expected {
                return Err(TerminalInterpretError::NoNormalInvocationReturned {
                    callee: *callee,
                    edge: *edge,
                });
            }
            if let SuspendedCallResult::ExternalCompletion { boundary, edge } = expected {
                return Err(TerminalInterpretError::ExternalCompletionProviderReturned {
                    boundary: *boundary,
                    edge: *edge,
                });
            }
            if let Some(claims) = completed.rebound_claims.take() {
                self.live_claims = claims;
            }
            self.complete_loan_custody(loan_completions)?;
            match expected {
                SuspendedCallResult::NoNormal { callee, edge } => {
                    Err(TerminalInterpretError::NoNormalInvocationReturned {
                        callee: *callee,
                        edge: *edge,
                    })
                }
                SuspendedCallResult::ExternalCompletion { boundary, edge } => {
                    Err(TerminalInterpretError::ExternalCompletionProviderReturned {
                        boundary: *boundary,
                        edge: *edge,
                    })
                }
                SuspendedCallResult::NominalCleanups {
                    completed: cleanup, ..
                } => {
                    if completed.value != TerminalExecutionResult::Unit {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    crate::engine::custody::complete_nominal_cleanup(
                        &mut self.structural_values,
                        &mut self.scalar_array_values,
                        &mut self.live_affine_frontier,
                        &self.live_claims,
                        &cleanup.0,
                    )?;
                    if !self.structural_values.contains_key(&cleanup.0.place) {
                        self.retire_home_companions(cleanup.0.place);
                        self.queue_retired_descriptor_home((&cleanup.1).into());
                    }
                    Ok(())
                }
                SuspendedCallResult::Unit => {
                    if completed.value != TerminalExecutionResult::Unit {
                        return Err(malformed());
                    }
                    Ok(())
                }
                SuspendedCallResult::Scalar(destination) => {
                    let TerminalExecutionResult::Scalar(value) = &completed.value else {
                        return Err(malformed());
                    };
                    self.values.insert(*destination, *value)?;
                    self.observe_completed_entry_result(*destination)
                }
                SuspendedCallResult::Structural {
                    result,
                    profile_completion,
                    ..
                } => {
                    if completed.destination.as_ref() != Some(result) {
                        return Err(malformed());
                    }
                    match &completed.value {
                        TerminalExecutionResult::Structural(returned) => {
                            if let Some(pending) = profile_completion {
                                self.complete_resource_profile_call(pending, &returned.value)?;
                            }
                            if self
                                .structural_values
                                .insert(result.place, returned.value.clone())?
                                .is_some()
                            {
                                return Err(malformed());
                            }
                            if let Some(binding) = &completed.byte_sequence {
                                self.byte_sequence_values
                                    .insert(result.place, binding.clone())?;
                            }
                            if let Some(binding) = &completed.element_view {
                                self.element_view_values
                                    .insert(result.place, binding.clone())?;
                            }
                        }
                        TerminalExecutionResult::ScalarArray(returned) => {
                            self.scalar_array_values
                                .publish(result.place, returned.value.clone())?;
                        }
                        _ => return Err(malformed()),
                    }
                    let carried = crate::engine::custody::whole_value_frontier(
                        &self.structural_types(),
                        result.place,
                        result.structural_type,
                        result.multiplicity,
                        self.live_claim_paths_at(result.place),
                        &|current| self.stored_case_at_discard(&self.structural_values, current),
                    )?;
                    if carried
                        .iter()
                        .any(|entry| self.live_affine_frontier.contains(entry))
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    self.live_affine_frontier.extend(carried);
                    Ok(())
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;
        use crate::engine::calls::frames::tests::empty_program;
        use crate::engine::execution::ResultClaimOrigin;
        use crate::storage::byte_sequences::{
            ByteSequenceView, binding::ByteSequenceBinding, storage::ByteSequenceStorage,
        };
        use evaluation_accounting::{
            BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
            TerminalStorageScheduleIdentity,
        };
        use semantic_vocabulary::{ClaimId, StructuralTypeId};
        use terminal_psi::{
            StructuralMultiplicity, StructuralOperationResult, StructuralResultClaimBinding,
        };

        #[test]
        fn refused_return_descriptor_retains_detached_backing() {
            check_refused_return(false);
            check_refused_return(true);
        }

        fn check_refused_return(retain_header: bool) {
            let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
            };
            let sponsor = BuildEvaluationSponsor::new_with_measurement(
                BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 2, 100, 100, 100).unwrap(),
                measurement,
            )
            .unwrap();
            let account = sponsor.begin_invocation(measurement, 100).unwrap();
            let mut execution = TerminalExecution::start_with_program(
                empty_program(),
                &[],
                crate::TerminalStructuralInputs::default(),
                None,
                Some(account.clone()),
            )
            .unwrap();
            let claim = ClaimId::new(1).unwrap();
            let claims = vec![StructuralResultClaimBinding {
                claim,
                path: Vec::new(),
            }];
            let structural_type = StructuralTypeId::new(1).unwrap();
            let result = StructuralOperationResult {
                place: PlaceId::new(1).unwrap(),
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: claims.clone(),
            };
            execution
                .suspend_with(
                    SuspendedCallResult::Structural {
                        result: result.clone(),
                        claim_origin: ResultClaimOrigin::CalleeReturn,
                        expected_reference_backings: Vec::new(),
                        preserved_projected_qualifications: Vec::new(),
                        profile_completion: None,
                    },
                    Vec::new(),
                )
                .unwrap();
            let retained_header = if retain_header {
                execution
                    .call_stack
                    .last_mut()
                    .unwrap()
                    ._frame_storage
                    .take()
            } else {
                None
            };
            let mut outgoing = CompletedReturn::plain(TerminalExecutionResult::Structural(
                TerminalStructuralResult {
                    value: TerminalStructuralValue::minted(42, structural_type, Vec::new()),
                    claims: vec![claim],
                    observation_home: None,
                },
            ));
            outgoing.destination = Some(result.clone());
            outgoing.returned_claims = claims.clone();
            outgoing.byte_sequence = Some(ByteSequenceBinding::Immutable(ByteSequenceView::new(
                ByteSequenceStorage::copy(b"retained", Some(&account)).unwrap(),
            )));
            let error = execution
                .finish_return(outgoing)
                .err()
                .expect("descriptor refusal");
            assert!(
                format!("{error:?}").contains("BudgetExhausted(LiveStorage)"),
                "{error:?}"
            );
            assert_eq!(
                sponsor.live_payload_bytes(),
                8,
                "detached return stays with failed activation"
            );
            assert_eq!(sponsor.live_storage_cells(), 2);
            assert_eq!(
                execution.structural_values.len(),
                usize::from(!retain_header)
            );
            assert!(execution.byte_sequence_values.is_empty());
            let failed = execution
                .failed_return
                .as_ref()
                .expect("failed return owner");
            let retained = &failed._completed;
            assert_eq!(retained.returned_claims, claims);
            assert_eq!(retained.destination.as_ref(), Some(&result));
            let TerminalExecutionResult::Structural(value) = &retained.value else {
                panic!("returned carrier lost");
            };
            assert_eq!(value.claims, [claim]);
            assert_eq!(value.value.opaque_identity, 42);
            assert!(
                matches!(failed._expected.as_ref(), Some(SuspendedCallResult::Structural { result: retained, .. }) if retained == &result)
            );
            assert!(failed._loan_completions.is_empty());
            let Some(ByteSequenceBinding::Immutable(bytes)) = &retained.byte_sequence else {
                panic!("returned bytes lost");
            };
            assert_eq!(bytes.bytes(), b"retained");
            drop(execution);
            assert_eq!(sponsor.live_payload_bytes(), 0);
            assert_eq!(sponsor.live_storage_cells(), u64::from(retain_header));
            drop(retained_header);
            assert_eq!(sponsor.live_storage_cells(), 0);
        }
    }
}
pub(crate) mod structural_return {
    use super::*;
    use crate::boundary::errors::malformed;
    use crate::engine::custody::verified_structural_place;
    use crate::storage::scalar_array::TerminalScalarArrayResult;

    impl TerminalExecution<'_> {
        pub(crate) fn settle_return_structural(
            &mut self,
            terminator: &Terminator,
            meter: &mut WorkMeter,
        ) -> Result<TerminatorFlow, TerminalInterpretError> {
            self.reject_no_normal_return()?;
            let Terminator::ReturnStructural {
                source,
                returned_claims,
                cleanup_actions,
                ..
            } = terminator
            else {
                unreachable!("dispatched settle_return_structural")
            };
            let signature = self
                .program
                .machines()
                .get(&self.current_machine)
                .and_then(|machine| machine.result.structural())
                .cloned()
                .ok_or(malformed())?;
            self.require_owned_value_claims(&StructuralArgument {
                place: *source,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })?;
            let active_returned_claims = self.active_structural_claims(*source, returned_claims)?;
            if cleanup_actions
                .iter()
                .any(|action| action.place() == *source)
            {
                return Err(malformed());
            }
            let destination = match self.call_stack.last() {
                Some(SuspendedCall {
                    structural_values,
                    scalar_array_values,
                    element_view_values,
                    byte_sequence_values,
                    live_affine_frontier,
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            claim_origin,
                            preserved_projected_qualifications,
                            ..
                        },
                    ..
                }) if result.structural_type == signature.structural_type
                    && result.multiplicity == signature.multiplicity
                    && claim_origin
                        .result_qualifications_match(result, &signature.qualifications)
                    && claim_origin.result_projected_qualifications_match(
                        result,
                        &signature.projected_qualifications,
                        preserved_projected_qualifications,
                    )
                    && !structural_values.contains_key(&result.place)
                    && !scalar_array_values.contains_key(&result.place)
                    && !element_view_values.contains_key(&result.place)
                    && !byte_sequence_values.contains_key(&result.place)
                    && live_affine_frontier
                        .iter()
                        .all(|entry| entry.place != result.place) =>
                {
                    Some(result.clone())
                }
                Some(_) => return Err(malformed()),
                None => None,
            };
            let mut identity_cursor = self.local_structural_identities.cursor();
            let mut completed = if self.scalar_array_values.contains_key(source) {
                let mut value = self.scalar_array_snapshot(
                    &StructuralArgument {
                        place: *source,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    },
                    signature.structural_type,
                )?;
                if value.structural_type != signature.structural_type
                    || signature.multiplicity != StructuralMultiplicity::Unrestricted
                    || !signature.projected_qualifications.is_empty()
                    || !returned_claims.is_empty()
                    || !self.live_claims.is_empty()
                    || destination.as_ref().is_some_and(|result| {
                        !result.projected_qualifications.is_empty() || !result.claims.is_empty()
                    })
                {
                    return Err(malformed());
                }
                self.validate_scalar_array_qualifications(&value, &signature.qualifications)?;
                value.qualifications.clone_from(&signature.qualifications);
                if let Some(destination) = &destination {
                    self.validate_scalar_array_qualifications(&value, &destination.qualifications)?;
                    value.qualifications.clone_from(&destination.qualifications);
                }
                CompletedReturn::plain(TerminalExecutionResult::ScalarArray(
                    TerminalScalarArrayResult { value },
                ))
            } else {
                let mut value =
                    verified_structural_place(&self.structural_values, *source).cloned()?;
                let byte_sequence = self.byte_sequence_values.get(source).cloned();
                let element_view = self.element_view_values.get(source).cloned();
                if byte_sequence.as_ref().is_some_and(|binding| {
                    binding.access() == StructuralAccess::SharedBorrow
                        && !binding.matches_shared_home(&value)
                }) || element_view
                    .as_ref()
                    .is_some_and(|view| !view.matches_descriptor_home(&value))
                {
                    return Err(malformed());
                }
                value.absorb_qualifications(&signature.qualifications);
                if let Some(destination) = &destination {
                    value.absorb_qualifications(&destination.qualifications);
                }
                self.validate_reference_return(&signature, &value)
                    .map_err(|error| {
                        error.in_reference_formation("validate returned references")
                    })?;
                if let Some(SuspendedCall {
                    result:
                        SuspendedCallResult::Structural {
                            preserved_projected_qualifications,
                            ..
                        },
                    ..
                }) = self.call_stack.last()
                {
                    for row in preserved_projected_qualifications {
                        if crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                        &self.structural_types(), &value, self.structural_cases.as_map(), &row.path,
                    )? {
                        value.projected_qualifications.push(row.clone());
                    }
                    }
                    value.projected_qualifications.sort();
                    value.projected_qualifications.dedup();
                }
                if let Some(SuspendedCall {
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            claim_origin: crate::engine::execution::ResultClaimOrigin::CalleeReturn,
                            ..
                        },
                    ..
                }) = self.call_stack.last()
                {
                    let mut receiving = Vec::new();
                    for row in &result.projected_qualifications {
                        if crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                        &self.structural_types(), &value, self.structural_cases.as_map(), &row.path,
                    )? {
                        if !value.projected_qualifications.contains(row) {
                            return Err(malformed());
                        }
                        receiving.push(row.clone());
                    }
                    }
                    value.projected_qualifications = receiving;
                }
                if value.structural_type != signature.structural_type
                    || signature
                        .qualifications
                        .iter()
                        .any(|domain| !value.qualifications.contains(domain))
                {
                    return Err(malformed());
                }
                self.validate_complete_owned_value_claims(*source, value.structural_type, &[])?;
                let actual_claims = self
                    .live_claims
                    .iter()
                    .filter(|(_, live)| live.place == Some(*source))
                    .map(|(claim, live)| terminal_psi::StructuralResultClaimBinding {
                        claim: *claim,
                        path: live.path.clone(),
                    })
                    .collect::<Vec<_>>();
                let unreturned = self
                    .live_claims
                    .keys()
                    .filter(|claim| {
                        !active_returned_claims
                            .iter()
                            .any(|binding| binding.claim == **claim)
                    })
                    .copied()
                    .collect::<Vec<_>>();
                if actual_claims != active_returned_claims || !unreturned.is_empty() {
                    return Err(TerminalInterpretError::StructuralReturnClaimsMismatch {
                        source: *source,
                        expected: active_returned_claims,
                        actual: actual_claims,
                        unreturned,
                    });
                }
                let borrowed_descriptor = signature.multiplicity == StructuralMultiplicity::Affine
                    && signature.qualifications.is_empty()
                    && terminal_semantics::view_descriptor_projected_qualifications(
                        self.structural_types().values(),
                        signature.structural_type,
                        &signature.projected_qualifications,
                    )
                    && signature
                        .projected_qualifications
                        .iter()
                        .all(|row| value.projected_qualifications.contains(row))
                    && active_returned_claims.is_empty()
                    && value.qualifications.is_empty()
                    && terminal_semantics::view_descriptor_projected_qualifications(
                        self.structural_types().values(),
                        value.structural_type,
                        &value.projected_qualifications,
                    )
                    && self
                        .program
                        .machines()
                        .get(&self.current_machine)
                        .and_then(|machine| terminal_semantics::arrival_parameter(machine, *source))
                        .is_some_and(|parameter| {
                            parameter.access == StructuralAccess::MutableBorrow
                                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                && terminal_semantics::view_descriptor_parameter(
                                    parameter,
                                    self.structural_types().values(),
                                )
                                && terminal_semantics::borrowed_referent_presentation(
                                    self.structural_types().values(),
                                    parameter.structural_type,
                                    signature.structural_type,
                                ) == Some(
                                    terminal_semantics::BorrowedReferentPresentation::Unchanged,
                                )
                                && parameter.qualifications.is_empty()
                                && terminal_semantics::view_descriptor_projected_qualifications(
                                    self.structural_types().values(),
                                    parameter.structural_type,
                                    &parameter.projected_qualifications,
                                )
                        })
                    && (element_view
                        .as_ref()
                        .is_some_and(|view| view.access() == StructuralAccess::MutableBorrow)
                        || (self
                            .structural_types()
                            .get(&signature.structural_type)
                            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
                            && matches!(byte_sequence, Some(ref binding) if binding.access() == StructuralAccess::MutableBorrow
                            && binding.validate_mutable_referent(self, &value).is_ok())));
                let carried = crate::engine::custody::whole_value_frontier(
                    &self.structural_types(),
                    *source,
                    signature.structural_type,
                    if borrowed_descriptor {
                        StructuralMultiplicity::Unrestricted
                    } else {
                        signature.multiplicity
                    },
                    self.live_claim_paths_at(*source),
                    &|current| self.stored_case_at_discard(&self.structural_values, current),
                )?;
                if self
                    .live_affine_frontier
                    .iter()
                    .filter(|entry| entry.place == *source)
                    .ne(carried.iter())
                {
                    return Err(
                        TerminalInterpretError::StructuralReturnAffineFrontierMismatch {
                            source: *source,
                            expected: carried.into_iter().collect(),
                            actual: self
                                .live_affine_frontier
                                .iter()
                                .filter(|entry| entry.place == *source)
                                .cloned()
                                .collect(),
                        },
                    );
                }
                let rebound_claims = match self.call_stack.last() {
                    Some(SuspendedCall {
                        live_claims,
                        result:
                            SuspendedCallResult::Structural {
                                result,
                                claim_origin,
                                ..
                            },
                        ..
                    }) => Some(match claim_origin {
                        crate::engine::execution::ResultClaimOrigin::CalleeReturn
                        | crate::engine::execution::ResultClaimOrigin::CheckedProvider
                        | crate::engine::execution::ResultClaimOrigin::TargetFamilyReturn {
                            ..
                        } => {
                            let mut active_result = result.clone();
                            active_result.claims =
                                self.active_structural_claims(*source, &result.claims)?;
                            rebind_structural_result_claims(
                                live_claims,
                                &self.live_claims,
                                *source,
                                &active_result,
                                &active_returned_claims,
                            )?
                        }
                    }),
                    Some(_) => return Err(malformed()),
                    None => None,
                };
                if let Some(SuspendedCall {
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            claim_origin:
                                crate::engine::execution::ResultClaimOrigin::TargetFamilyReturn {
                                    head,
                                    member,
                                },
                            ..
                        },
                    ..
                }) = self.call_stack.last()
                {
                    if *member != self.current_machine || !self.program.is_target_family_head(*head)
                    {
                        return Err(malformed());
                    }
                    if result
                        .projected_qualifications
                        .iter()
                        .any(|row| !value.projected_qualifications.contains(row))
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "family result projected qualification lacks transported member evidence",
                        ));
                    }
                    value.qualifications.clone_from(&result.qualifications);
                    value
                        .projected_qualifications
                        .clone_from(&result.projected_qualifications);
                }
                if let Some(SuspendedCall {
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            claim_origin:
                                crate::engine::execution::ResultClaimOrigin::CheckedProvider,
                            ..
                        },
                    ..
                }) = self.call_stack.last()
                {
                    let mut active = Vec::new();
                    for row in &result.projected_qualifications {
                        if crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                        &self.structural_types(), &value, self.structural_cases.as_map(), &row.path,
                    )? {
                        if !value.projected_qualifications.contains(row) {
                            return Err(malformed());
                        }
                        active.push(row.clone());
                    }
                    }
                    value.projected_qualifications = active;
                }
                let byte_sequence = byte_sequence.map(|binding| {
                    if binding.access() == StructuralAccess::SharedBorrow {
                        binding.with_descriptor_home(value.clone())
                    } else {
                        binding
                    }
                });
                if borrowed_descriptor && element_view.is_some() {
                    value.opaque_identity = self
                        .local_structural_identities
                        .allocate_staged(&mut identity_cursor)?;
                    value.path.clear();
                }
                let element_view =
                    element_view.map(|view| view.with_descriptor_home(value.clone()));
                let scalar_case = if self.call_stack.is_empty()
                    && signature.qualifications.is_empty()
                    && signature.projected_qualifications.is_empty()
                    && returned_claims.is_empty()
                {
                    self.snapshot_scalar_case(&value)?
                } else {
                    None
                };
                CompletedReturn {
                    returned_claims: active_returned_claims.clone(),
                    value: if let Some(value) = scalar_case {
                        TerminalExecutionResult::ScalarCase(value)
                    } else {
                        TerminalExecutionResult::Structural(TerminalStructuralResult {
                            value,
                            claims: active_returned_claims
                                .iter()
                                .map(|binding| binding.claim)
                                .collect(),
                            observation_home: None,
                        })
                    },
                    destination: None,
                    rebound_claims,
                    byte_sequence,
                    element_view,
                }
            };
            completed.destination = destination;
            let cleanup_actions = block_bindings::BlockBindings::default()
                .prepare_cleanup_actions(self, &[], cleanup_actions)?;
            if let Err(error) = meter.charge_terminator(terminator) {
                return meter_status(error).map(TerminatorFlow::Yield);
            }
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            if let Some(retired) = self.structural_values.remove(source) {
                let origin = self
                    .reference_referents
                    .get(&crate::boundary::values::StructuralRuntimePlace::from(
                        &retired,
                    ))
                    .cloned();
                self.retired_structural_homes
                    .push((*source, retired, origin));
            }
            self.scalar_array_values.remove(source);
            self.byte_sequence_values.remove(source);
            self.element_view_values.remove(source);
            self.live_affine_frontier
                .retain(|entry| entry.place != *source);
            for binding in &active_returned_claims {
                self.live_claims.remove(&binding.claim);
            }
            self.continue_cleanup_actions(
                cleanup_actions.into_iter(),
                CleanupContinuation::Return(completed),
            )
            .map_err(|error| error.in_reference_formation("complete structural return"))
        }

        fn snapshot_scalar_case(
            &self,
            value: &crate::boundary::values::TerminalStructuralValue,
        ) -> Result<Option<TerminalScalarCaseResult>, TerminalInterpretError> {
            if !value.qualifications.is_empty() || !value.projected_qualifications.is_empty() {
                return Ok(None);
            }
            let declaration = self.structural_declaration(value.structural_type)?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return Ok(None);
            };
            let root = crate::boundary::values::StructuralRuntimePlace::from(value);
            let Some(contents) = self.structural_cases.get(&root) else {
                return Ok(None);
            };
            if contents.structural_type != value.structural_type {
                return Err(malformed());
            }
            let selected =
                terminal_psi::find_structural_case(cases, contents.case).ok_or_else(malformed)?;
            if selected.fields.iter().any(|field| {
                field.relevance.is_erased() || field.field_type.scalar_type().is_none()
            }) {
                return Ok(None);
            }
            let fields = selected.fields.iter().map(|field| {
                let cell = crate::boundary::values::StructuralScalarRuntimeField::case_payload(
                    root.clone(),
                    field.id,
                );
                self.structural_scalar_fields
                    .get(&cell)
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?
                    .map(|scalar| (field.id, scalar))
                    .ok_or_else(malformed)
            });
            let snapshot = TerminalScalarCaseResult::collect(
                declaration,
                contents.case,
                selected.fields.len(),
                fields,
                self.account.as_ref(),
            )?;
            Ok(Some(snapshot))
        }
    }
}

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::results::meter_status;
use crate::boundary::results::{
    TerminalCrash, TerminalCrashSite, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarCaseResult, TerminalStructuralResult,
};
use crate::boundary::values::TerminalScalarValue;
use crate::engine::custody::{
    commit_cleanup_action, consume_affine_projection, consume_projected_ownership,
    rebind_structural_result_claims,
};
use crate::engine::execution::{
    CleanupContinuation, CompletedReturn, SuspendedCall, SuspendedCallResult, TerminalExecution,
    TerminatorFlow,
};
use crate::storage::runtime_rows::RuntimeRows;
use psi_work::WorkMeter;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ValueId};
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralPathSegment, TerminalMachineResult, Terminator,
};

struct SelectedSuccessor<'edge> {
    edge: EdgeId,
    target: BlockId,
    arguments: &'edge [ValueId],
    structural_arguments: &'edge [StructuralArgument],
    cleanup_actions: &'edge [terminal_psi::TerminalAffineCleanupAction],
}

impl TerminalExecution<'_> {
    pub(crate) fn settle_invoke_no_normal(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::InvokeNoNormal {
            edge,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            ..
        } = terminator
        else {
            return Err(malformed());
        };
        let scalar_arguments = arguments
            .iter()
            .map(|argument| self.scalar_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.begin_no_normal_call(
            *callee,
            *edge,
            &scalar_arguments,
            structural_arguments,
            claim_transfers,
        )?;
        Ok(TerminatorFlow::Continue)
    }

    fn live_claim_paths_at(
        &self,
        place: PlaceId,
    ) -> impl Iterator<Item = &[StructuralPathSegment]> + '_ {
        self.live_claims
            .values()
            .filter(move |claim| claim.place == Some(place))
            .map(|claim| claim.path.as_slice())
    }

    fn stored_case_at_discard(
        &self,
        structural_values: &RuntimeRows<PlaceId, crate::boundary::values::TerminalStructuralValue>,
        discard: &StructuralAffineDiscard,
    ) -> Option<semantic_vocabulary::StructuralCaseId> {
        crate::boundary::structural_inputs::case_membership::stored_case_at_projection(
            &self.structural_types(),
            structural_values.get(&discard.place)?,
            self.structural_cases.as_map(),
            &discard.path,
        )
    }

    pub(crate) fn consume_affine_projection(
        &self,
        frontier: &mut std::collections::BTreeSet<StructuralAffineDiscard>,
        argument: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        consume_affine_projection(
            &self.structural_types(),
            &self.structural_values,
            self.structural_cases.as_map(),
            frontier,
            argument,
        )
    }

    pub(crate) fn settle_jump(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Jump {
            edge,
            target,
            arguments,
            structural_arguments,
            cleanup_actions,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_jump")
        };
        self.settle_successor(
            SelectedSuccessor {
                edge: *edge,
                target: *target,
                arguments,
                structural_arguments,
                cleanup_actions,
            },
            terminator,
            meter,
        )
    }

    fn settle_successor(
        &mut self,
        successor: SelectedSuccessor<'_>,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let scalars = self.scalar_values_at(successor.arguments)?;
        self.settle_successor_with_scalars(successor, &scalars, terminator, meter)
    }

    fn settle_successor_with_scalars(
        &mut self,
        successor: SelectedSuccessor<'_>,
        scalars: &[TerminalScalarValue],
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let SelectedSuccessor {
            edge,
            target,
            arguments: _,
            structural_arguments,
            cleanup_actions,
        } = successor;
        let mut selected_arguments = structural_arguments.to_vec();
        for (declared, selected) in structural_arguments.iter().zip(&mut selected_arguments) {
            if declared
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
            {
                self.select_runtime_projection(
                    declared.place,
                    self.live_structural_type(declared.place)?,
                    &declared.path,
                    &mut selected.path,
                )?;
            }
        }
        let structural_arguments = selected_arguments.as_slice();
        let bindings = self
            .prepare_block_bindings(target, scalars, structural_arguments)
            .map_err(|error| {
                error.in_argument_preparation("state arrival storage and memberships")
            })?;
        let cleanup_actions =
            bindings.prepare_cleanup_actions(self, structural_arguments, cleanup_actions)?;
        if let Err(error) = meter.charge_edge(edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        bindings.reserve_identities(self);
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned
                && !argument.path.is_empty()
                && bindings.projected_sources.contains(&argument.place)
        }) {
            consume_projected_ownership(
                &self.program.structural_types(),
                &self.structural_values,
                self.structural_cases.as_map(),
                &mut self.live_affine_frontier,
                &mut self.live_claims,
                argument,
            )?;
        }
        let remaining = cleanup_actions;
        self.continue_cleanup_actions(
            remaining.into_iter(),
            CleanupContinuation::Successor { target, bindings },
        )
    }

    pub(crate) fn settle_conditional(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = terminator
        else {
            unreachable!("dispatched settle_conditional")
        };
        let condition = self.scalar_value(*condition)?;
        let TerminalScalarValue::Boolean(condition) = condition else {
            return Err(malformed());
        };
        let successor = if condition { when_true } else { when_false };
        self.settle_successor(
            SelectedSuccessor {
                edge: successor.edge,
                target: successor.target,
                arguments: &successor.arguments,
                structural_arguments: &successor.structural_arguments,
                cleanup_actions: &successor.cleanup_actions,
            },
            terminator,
            meter,
        )
    }

    pub(crate) fn settle_structural_case(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = terminator
        else {
            unreachable!("dispatched settle_structural_case")
        };
        for (position, segment) in path.iter().enumerate() {
            if let StructuralPathSegment::Case(case) = segment
                && self.observe_structural_case(*source, &path[..position])? != *case
            {
                return Err(malformed());
            }
        }
        let active_case = self.observe_structural_case(*source, path)?;
        let projected =
            self.resolve_live_projection(*source, path, StructuralAccess::SharedBorrow)?;
        let payload_root = crate::boundary::values::StructuralRuntimePlace::from(&projected);
        let successor = cases
            .iter()
            .find(|successor| successor.case == active_case)
            .ok_or(malformed())?;
        let target = self
            .program
            .verified_machine(self.current_machine)?
            .block(&successor.target)
            .ok_or(malformed())?;
        if target.parameters.len() != successor.payload_fields.len() {
            return Err(malformed());
        }
        let mut bindings = Vec::with_capacity(target.parameters.len());
        for (parameter, field) in target.parameters.iter().zip(&successor.payload_fields) {
            let scalar = self
                .structural_scalar_fields
                .get(
                    &crate::boundary::values::StructuralScalarRuntimeField::case_payload(
                        payload_root.clone(),
                        *field,
                    ),
                )
                .ok_or(malformed())?
                .read()?;
            if scalar.scalar_type() != parameter.scalar_type {
                return Err(malformed());
            }
            bindings.push(scalar);
        }
        self.settle_successor_with_scalars(
            SelectedSuccessor {
                edge: successor.edge,
                target: successor.target,
                arguments: &[],
                structural_arguments: &successor.structural_arguments,
                cleanup_actions: &successor.cleanup_actions,
            },
            &bindings,
            terminator,
            meter,
        )
    }

    pub(crate) fn settle_return(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        self.reject_no_normal_return()?;
        let Terminator::Return {
            value,
            cleanup_actions,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return")
        };
        let machine = self.program.verified_machine(self.current_machine)?;
        if !matches!(machine.result, TerminalMachineResult::Scalar(_)) {
            return Err(malformed());
        }
        let cleanup_actions = block_bindings::BlockBindings::default().prepare_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let result = self.scalar_value(*value)?;
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
            CleanupContinuation::Return(CompletedReturn::plain(TerminalExecutionResult::Scalar(
                result,
            ))),
        )
    }

    pub(crate) fn settle_return_unit(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        self.reject_no_normal_return()?;
        let Terminator::ReturnUnit {
            cleanup_actions, ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_unit")
        };
        let machine = self.program.verified_machine(self.current_machine)?;
        if machine.result != TerminalMachineResult::Unit {
            return Err(malformed());
        }
        let cleanup_actions = block_bindings::BlockBindings::default().prepare_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
            CleanupContinuation::Return(CompletedReturn::plain(TerminalExecutionResult::Unit)),
        )
    }

    pub(crate) fn settle_crash(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_crash")
        };
        if frontier_lower_bound != &self.live_claims.keys().copied().collect::<Vec<_>>() {
            return Err(malformed());
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let crash = TerminalCrash {
            site: TerminalCrashSite::Edge(*edge),
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        };
        self.crash = Some(crash.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Crashed(
            crash,
        )))
    }

    pub(crate) fn settle_external_completion(
        &mut self,
        terminator: &Terminator,
        meter: &mut WorkMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ExternalCompletion {
            edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            frontier_lower_bound,
            ..
        } = terminator
        else {
            return Err(malformed());
        };
        let declaration = self.program.verified_boundary_machine(boundary)?;
        if declaration.completion != terminal_psi::BoundaryCompletion::External(*kind)
            || arguments.len() != 1
            || !structural_arguments.is_empty()
            || frontier_lower_bound
                .windows(2)
                .any(|claims| claims[0] >= claims[1])
            || frontier_lower_bound
                .iter()
                .any(|claim| !self.live_claims.contains_key(claim))
        {
            return Err(malformed());
        }
        let actual = self.scalar_value(arguments[0])?;
        crate::engine::custody::bind_boundary_arguments(&declaration.scalar_parameters, &[actual])?;
        crate::engine::custody::validate_boundary_requirements(declaration, |_| None)?;
        if let Some(callee) = crate::engine::calls::target_families::completion_callee(
            &self.program,
            self.target_machines.as_ref(),
            self.boundary_supplies.get(boundary).copied(),
            *boundary,
        )? {
            if let Err(error) = meter.charge_terminator(terminator) {
                return meter_status(error).map(TerminatorFlow::Yield);
            }
            self.begin_external_completion_call(
                callee,
                *boundary,
                *edge,
                &[actual],
                structural_arguments,
            )?;
            return Ok(TerminatorFlow::Continue);
        }
        let demand = crate::engine::capabilities::TerminalExternalCompletionDemand {
            boundary: *boundary,
            kind: *kind,
        };
        if !self
            .external_bindings
            .get(boundary)
            .is_some_and(|admission| {
                admission.answers(
                    self.program.identity.semantic(),
                    self.process_domain.as_ref(),
                    demand,
                )
            })
        {
            return Err(TerminalInterpretError::UnboundExternalCompletions {
                missing: vec![demand],
            });
        }
        let TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Signed(value),
        } = actual
        else {
            return Err(malformed());
        };
        if scalar_type
            != semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .map_err(|_| malformed())?
        {
            return Err(malformed());
        }
        let return_code = i32::try_from(value).map_err(|_| malformed())?;
        match kind {
            semantic_vocabulary::ExternalCompletionKind::ProcessExit => {}
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let completion = crate::boundary::results::TerminalExternalCompletion {
            machine: self.current_machine,
            block: self.current,
            edge: *edge,
            boundary: *boundary,
            kind: *kind,
            return_code,
            frontier_lower_bound: frontier_lower_bound.clone(),
        };
        self.external_completion = Some(completion.clone());
        Ok(TerminatorFlow::Yield(
            TerminalExecutionStatus::ExternalCompletion(completion),
        ))
    }
}

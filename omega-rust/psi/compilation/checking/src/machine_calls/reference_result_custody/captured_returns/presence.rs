//! Presence of actual captured descriptor leaves. A completed value and its
//! immutable storage cut select the leaf; declaration shape alone cannot
//! make an inactive case present or revive overwritten descriptor contents.
use super::{
    Captures, CheckedReferenceOriginProjection, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment, ExpressionHandle,
    StoredCarrierSource, SymbolHandle, TypeReferenceHandle, is_mutable_structural_parameter,
    loan_place_segments, stored_carrier_sources,
};

impl Captures<'_> {
    pub(super) fn call_paths(
        &self,
        call: &checked_trees::FlowCallFact,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
        let parameters = crate::call_target_parameters(self.program, call.target_symbol)?;
        let sources = self.target_sources(call.target_symbol, active)?;
        let operands = self
            .facts
            .flow
            .control
            .operands
            .span(call.argument_operands())?;
        let mut paths = Vec::new();
        for source in sources {
            let parameter_index = match source.source.root {
                checked_trees::CheckedReferenceOriginRoot::Parameter { index } => index,
                checked_trees::CheckedReferenceOriginRoot::Invocation { .. } => {
                    if !paths.contains(&source.path) {
                        paths.push(source.path);
                    }
                    continue;
                }
            };
            let (position, parameter) = parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| is_mutable_structural_parameter(self.program, parameter))
                .nth(parameter_index as usize)?;
            let operand = operands.get(position)?;
            let present = if crate::reference_shell(self.program, parameter.type_reference)
                .is_some()
            {
                true
            } else {
                // Presence belongs to the input's stored descriptor, not the
                // pointee projection after crossing it. A runtime family is
                // never used as a selector for an owned input slot.
                let path = source
                    .source
                    .path
                    .iter()
                    .take_while(|projection| {
                        !matches!(
                            projection,
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::Referent
                            )
                        )
                    })
                    .map(|projection| match projection {
                        CheckedReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                        CheckedReferenceOriginProjection::Element => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                self.expression_leaf_present(
                    self.program.state_by_symbol(operand.state_symbol)?,
                    operand.statement_index,
                    operand.expression,
                    parameter.type_reference,
                    &path,
                    Some(operand),
                    active,
                )?
            };
            if present && !paths.contains(&source.path) {
                paths.push(source.path);
            }
        }
        Some(paths)
    }

    pub(super) fn expression_leaf_present(
        &self,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        path: &[CheckedUnitStructuralPathSegment],
        capture: Option<&checked_trees::FlowOperandFact>,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        // Whole calls already have a completed flow occurrence. Their result
        // need not be materialized as another value-graph root (a store can
        // consume it directly). Nested calls selected through constructors
        // reach the same call relation from the graph below.
        let flow = self.flow(state.symbol)?;
        let mut calls = self
            .facts
            .flow
            .control
            .calls
            .span(flow.calls)?
            .iter()
            .filter(|call| {
                call.statement_index == statement_index && call.authored_expression == expression
            });
        if let Some(call) = calls.next() {
            if calls.next().is_some() {
                return None;
            }
            return Some(
                self.call_paths(call, active)?
                    .iter()
                    .any(|candidate| candidate.as_slice() == path),
            );
        }
        let values = &self.facts.values.structural_values;
        let root = values
            .root_for_expression(
                state.symbol,
                u32::try_from(statement_index).ok()?,
                expression,
            )
            .map(|root| root.root)
            .or_else(|| {
                values.nodes.iter().find_map(|(handle, node)| {
                    (node.state == state.symbol
                        && node.statement_index as usize == statement_index
                        && node.expression == expression
                        && self.program.normalized_type_identity(node.type_reference)
                            == self.program.normalized_type_identity(reference))
                    .then_some(handle)
                })
            });
        let Some(root) = root else {
            let mut place = crate::canonical_place_from_expression_in_state(
                self.program,
                state.symbol,
                statement_index,
                expression,
            )?;
            place.extend_segments(&loan_place_segments(self.program, reference, path)?);
            return self.place_leaf_present(
                state,
                statement_index,
                place.root,
                &place.segments,
                capture,
                active,
            );
        };
        let mut present = false;
        for source in stored_carrier_sources(self.program, self.facts, root, path)? {
            let supplied = match source {
                StoredCarrierSource::Borrow { .. } => true,
                // Image storage has no activation-relative loan to publish.
                // The descriptor is present, and its literal producer retains
                // the actual backing and qualifications independently. This
                // query asks only whether a caller-relative capture is owed.
                StoredCarrierSource::Image { .. } => false,
                StoredCarrierSource::Place(source) => match source.source {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                        let (_, local) = self.program.state_local_declaration(state, symbol)?;
                        self.place_leaf_present(
                            state,
                            statement_index,
                            facts::PlaceRoot::Symbol(symbol),
                            &loan_place_segments(self.program, local.type_reference, &source.path)?,
                            capture,
                            active,
                        )?
                    }
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                        let parameter = self
                            .program
                            .state_parameters(state)
                            .iter()
                            .filter(|parameter| {
                                is_mutable_structural_parameter(self.program, parameter)
                            })
                            .nth(parameter_index as usize)?;
                        self.ingress_present(state, parameter.symbol)?
                    }
                    _ => return None,
                },
                StoredCarrierSource::Call { value, path } => {
                    let CheckedStructuralValueKind::Call { source_call } =
                        values.nodes.get(value).kind
                    else {
                        return None;
                    };
                    if !self.facts.flow.control.calls.is_valid(source_call) {
                        return None;
                    }
                    self.call_paths(self.facts.flow.control.calls.get(source_call), active)?
                        .contains(&path)
                }
            };
            present |= supplied;
        }
        Some(present)
    }

    pub(super) fn place_leaf_present(
        &self,
        state: &typed_trees::state::State,
        before: usize,
        root: facts::PlaceRoot,
        segments: &[facts::PlaceSegment],
        capture: Option<&checked_trees::FlowOperandFact>,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        let control = &self.facts.flow.control;
        let mut place = crate::CanonicalPlace {
            root,
            segments: segments.to_vec(),
        };
        let mut observation = capture;
        let mut sources = Vec::new();
        // A copied whole value retains its original inline bytes. A pointer
        // preserves only its descriptor, so never carry this link backwards
        // through an earlier reference boundary.
        while let Some(capture) = observation {
            let source = capture.value_source;
            if !source.is_valid()
                || crate::reference_shell(self.program, capture.type_reference).is_some()
            {
                break;
            }
            if !control.operands.is_valid(source) || sources.contains(&source) {
                return None;
            }
            let base = crate::canonical_place_from_expression_in_state(
                self.program,
                capture.state_symbol,
                capture.statement_index,
                capture.expression,
            )?;
            if place.root != base.root || !place.segments.starts_with(&base.segments) {
                return None;
            }
            let suffix = &place.segments[base.segments.len()..];
            if (0..suffix.len()).any(|length| {
                crate::project_type_reference_from_segments(
                    self.program,
                    capture.type_reference,
                    &suffix[..length],
                )
                .is_some_and(|reference| crate::reference_shell(self.program, reference).is_some())
            }) {
                break;
            }
            let earlier = control.operands.get(source);
            let mut prior = crate::canonical_place_from_expression_in_state(
                self.program,
                earlier.state_symbol,
                earlier.statement_index,
                earlier.expression,
            )?;
            prior.segments.extend_from_slice(suffix);
            place = prior;
            observation = Some(earlier);
            sources.push(source);
        }
        let flow = self.flow(state.symbol)?;
        let snapshot = if let Some(capture) = observation {
            capture.storage_snapshot
        } else {
            let order = self
                .program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            let position = order.position(before).or_else(|| {
                (before
                    == self
                        .program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len())
                .then_some(order.indices().len())
            })?;
            if let Some(previous) = order.indices()[..position].last() {
                control
                    .statements
                    .span(flow.statements)?
                    .iter()
                    .find(|statement| statement.statement_index == *previous)?
                    .storage_snapshot
            } else {
                Default::default()
            }
        };
        let Some(stored) = crate::completed_storage::at_snapshot(
            self.program,
            control,
            snapshot,
            &place,
            crate::completed_storage::StoredObservation::Binding,
        ) else {
            return self.ingress_present(state, place.root.as_symbol()?);
        };
        let captures = stored.captures()?;
        let mut present = false;
        for selected in captures {
            let value = control.operands.get(selected.capture);
            if crate::reference_shell(self.program, value.type_reference).is_some() {
                let values = crate::completed_storage::reference_captures_at_path(
                    self.program,
                    control,
                    selected.capture,
                    &selected.projection,
                    snapshot,
                );
                let Some(values) = values else {
                    // Only a complete admitted input may supply an abstract
                    // pointee namespace. An unknown local never does.
                    let referents = control.operand_referents.span(value.referents)?;
                    if referents.is_empty() {
                        return None;
                    }
                    for referent in referents {
                        present |= self.ingress_present(state, referent.root.as_symbol()?)?;
                    }
                    continue;
                };
                for descriptor in values.captures()? {
                    let descriptor_value = control.operands.get(descriptor.capture);
                    if descriptor.projection.is_empty()
                        && crate::reference_shell(self.program, descriptor_value.type_reference)
                            .is_some()
                    {
                        // Presence of an installed descriptor is not a new
                        // backing address or loan. Empty known dependencies
                        // owe no activation-relative capture (image storage).
                        present |= !control
                            .retained_roots(descriptor_value.retention)?
                            .is_empty();
                    } else {
                        let (path, _) = self.structural_path(
                            descriptor_value.type_reference,
                            &descriptor.projection,
                        )?;
                        present |= self.expression_leaf_present(
                            self.program
                                .state_by_symbol(descriptor_value.state_symbol)?,
                            descriptor_value.statement_index,
                            descriptor_value.expression,
                            descriptor_value.type_reference,
                            &path,
                            Some(descriptor_value),
                            active,
                        )?;
                    }
                }
            } else {
                // Actual aggregate structure decides case presence; a type's
                // declared reference field cannot create an active payload.
                let (path, _) = self.structural_path(value.type_reference, &selected.projection)?;
                present |= self.expression_leaf_present(
                    self.program.state_by_symbol(value.state_symbol)?,
                    value.statement_index,
                    value.expression,
                    value.type_reference,
                    &path,
                    Some(value),
                    active,
                )?;
            }
        }
        Some(present)
    }

    pub(super) fn ingress_present(
        &self,
        state: &typed_trees::state::State,
        symbol: SymbolHandle,
    ) -> Option<bool> {
        let parameter = self
            .program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)?;
        let flow = self.flow(state.symbol)?;
        super::super::stored_carrier_ingress_is_complete(
            self.program,
            self.facts,
            flow.machine_symbol,
            state.symbol,
            symbol,
            parameter.type_reference,
        )
        .then_some(true)
    }
}

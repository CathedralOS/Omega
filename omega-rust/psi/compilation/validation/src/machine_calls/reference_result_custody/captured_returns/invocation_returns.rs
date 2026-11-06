//! Invocation completion joins the finite target result relation to this edge's
//! completed operands and exact returned-loan group. Destination formals never
//! cross into the caller namespace without substitution; declaration lifetime
//! candidates do not establish a present result or choose its backing.

use super::super::{
    MAX_REFERENCE_ORIGIN_LEAVES, declared_carrier_references, origin_place_segments,
};
use super::{
    CapturedReturnOrigin, CapturedReturnPresentation, Captures, CheckedReferenceOriginProjection,
    CheckedReturnedReferencePlan, CheckedUnitStructuralPathSegment, SymbolHandle,
    TypeReferenceHandle, loan_owner_path, loan_place_segments,
};

impl Captures<'_> {
    /// A completing invocation consumes the complete target relation. Its formal roots are substituted through this
    /// occurrence's completed operands before joining source-state custody.
    pub(super) fn invocation_return_sources(
        &self,
        state: &typed_trees::state::State,
        transition_target: typed_trees::statement::TransitionTargetHandle,
        call: &checked_trees::FlowCallFact,
        loans: &[checked_trees::BorrowReturnedLoanFact],
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        // A forwarding body consumes the same checked return relation as its
        // other callers. Lifetime-only frame summaries cannot replace that
        // relation or recover the origins of an abstract requirement's result.
        let relative = self.target_sources(call.target_symbol, active)?;
        let leaves = self.reference_leaves(state)?;
        if self.trace_return_state(state) {
            eprintln!(
                "CHECKSITE completion-invocation-origins state={:?} call={:?} relative={:?} leaves={:?}",
                state.symbol,
                call,
                relative,
                leaves
                    .iter()
                    .map(|leaf| (&leaf.path, &leaf.source))
                    .collect::<Vec<_>>()
            );
        }
        let return_type = crate::call_target_return_type(self.program, call.target_symbol)?;
        // This declaration roster supplies result paths and types only. Its
        // placeholder source does not establish a parameter or permission;
        // target_sources and the exact returned loans own those obligations.
        let target_leaves = super::super::record_carrier_sources(
            self.program,
            return_type,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: 0,
            },
        )?;
        let parameters = crate::call_target_parameters(self.program, call.target_symbol)?;
        let operands = self
            .facts
            .flow
            .control
            .operands
            .span(call.argument_operands())?;
        if parameters.len() != operands.len() {
            return None;
        }
        let mut origins = Vec::new();
        let mut independent = Vec::new();
        for candidate in relative {
            let leaf = leaves.iter().find(|leaf| leaf.path == candidate.path)?;
            if candidate.source.access != leaf.source.access {
                return None;
            }
            let result_path =
                loan_place_segments(self.program, state.return_type, &candidate.path)?;
            let checked_trees::CheckedReferenceOriginRoot::Parameter { index } =
                &candidate.source.root
            else {
                independent.push(candidate);
                continue;
            };
            let (position, parameter) = parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    super::super::is_mutable_structural_parameter(self.program, parameter)
                })
                .nth(*index as usize)?;
            let operand = operands.get(position)?;
            if operand.state_symbol != state.symbol
                || operand.statement_index != call.statement_index
            {
                return None;
            }
            let target_leaf = target_leaves
                .iter()
                .find(|leaf| leaf.path == candidate.path)?;
            let input_origin = self.checked_input_origin(
                parameter,
                &candidate.source,
                target_leaf.source.type_identity,
            );
            if input_origin.is_none() && self.trace_return_state(state) {
                eprintln!(
                    "CHECKSITE completion-input-origin state={:?} parameter={:?} candidate={:?} result_type={:?}",
                    state.symbol, parameter, candidate, target_leaf.source.type_identity
                );
            }
            let (relative, presentation) = input_origin?;
            let inputs = self.argument_return_sources(
                state,
                parameter.type_reference,
                call.argument_operands().handles().nth(position)?,
                call.invocation_storage,
                &relative,
                &presentation,
                active,
                &mut Vec::new(),
            );
            if inputs.is_none() && self.trace_return_state(state) {
                eprintln!(
                    "CHECKSITE completion-input-sources state={:?} parameter={:?} operand={:?} relative={:?}",
                    state.symbol, parameter, operand, relative
                );
            }
            for input in inputs? {
                let origin = CapturedReturnOrigin {
                    result_path: result_path.clone(),
                    source: input.source,
                    extent: if candidate.source.extent
                        == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                        || input.extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                    {
                        semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                    } else {
                        semantic_vocabulary::ReferenceOriginExtent::Projected
                    },
                    presentation: Some(input.presentation),
                };
                if !origins.contains(&origin) {
                    origins.push(origin);
                    if origins.len() > MAX_REFERENCE_ORIGIN_LEAVES {
                        return None;
                    }
                }
            }
        }
        let site_loans: Vec<_> = loans
            .iter()
            .filter(|loan| {
                loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                    && loan.statement_index == call.statement_index
                    && loan.transition_target == transition_target
            })
            .cloned()
            .collect();
        let mut output = Vec::new();
        for leaf in leaves {
            let result_path = loan_place_segments(self.program, state.return_type, &leaf.path)?;
            let owner_path = crate::corresponding_loan_owner_path(
                self.program,
                state.return_type,
                &loan_owner_path(self.program, state.return_type, &leaf.path)?,
            )?;
            let mut captured = false;
            for candidate in independent
                .iter()
                .filter(|candidate| candidate.path == leaf.path)
            {
                let checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } =
                    &candidate.source.root
                else {
                    return None;
                };
                if candidate.source.entry_reference_hops != 0 {
                    return None;
                }
                let mut supplied = false;
                for loan in site_loans.iter().filter(|loan| {
                    self.returned_loan_owner_path(state, loan).as_deref()
                        == Some(owner_path.as_slice())
                        && self
                            .facts
                            .borrow
                            .incoming_references
                            .is_valid(loan.incoming_reference)
                }) {
                    let incoming = self
                        .facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    if incoming.state_symbol != state.symbol
                        || !matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Invocation { expression, target }
                            if expression == call.authored_expression
                                && crate::call_target_entry_symbol(self.program, target) == call.target_symbol)
                        || leaf.source.access.borrow_access_kind()? != loan.kind
                    {
                        continue;
                    }
                    let (invocation_carrier, descriptor_path) =
                        self.invocation_carrier(state, incoming)?;
                    let prefix = descriptor_path;
                    if self.program.normalized_type_identity(invocation_carrier) != *carrier_type
                        || !candidate.source.path.starts_with(&prefix)
                    {
                        continue;
                    }
                    let mut projection = super::super::origin_place_projection(
                        self.program,
                        invocation_carrier,
                        candidate.source.path.iter().cloned(),
                    )?;
                    if projection.type_identity(self.program) != candidate.source.type_identity {
                        return None;
                    }
                    for mut source in self.return_origin(
                        state,
                        loan,
                        &leaf,
                        self.facts.borrow.access_segments.span(loan.segments)?,
                        &site_loans,
                    )? {
                        if candidate.source.extent
                            == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                        {
                            source.extent =
                                semantic_vocabulary::ReferenceOriginExtent::WithinRegion;
                        }
                        let returned = CheckedReturnedReferencePlan {
                            path: leaf.path.clone(),
                            source,
                        };
                        if !output.contains(&returned) {
                            output.push(returned);
                        }
                        supplied = true;
                    }
                }
                if !supplied {
                    return None;
                }
                captured = true;
            }
            for origin in origins
                .iter()
                .filter(|origin| origin.result_path == result_path)
            {
                let mut covered = false;
                for loan in site_loans.iter().filter(|loan| {
                    self.returned_loan_owner_path(state, loan).as_deref()
                        == Some(owner_path.as_slice())
                        && self
                            .facts
                            .borrow
                            .incoming_references
                            .is_valid(loan.incoming_reference)
                }) {
                    let incoming = self
                        .facts
                        .borrow
                        .incoming_references
                        .get(loan.incoming_reference);
                    if incoming.state_symbol != state.symbol
                        || loan.root != facts::PlaceRoot::Expression(call.authored_expression)
                        || !matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Invocation { expression, target }
                            if expression == call.authored_expression
                                && crate::call_target_entry_symbol(self.program, target) == call.target_symbol)
                        || leaf.source.access.borrow_access_kind()? != loan.kind
                    {
                        continue;
                    }
                    let (carrier, descriptor) = self.invocation_carrier(state, incoming)?;
                    if self.program.normalized_type_identity(carrier)
                        != self.program.normalized_type_identity(return_type)
                        || descriptor
                            != leaf
                                .path
                                .iter()
                                .cloned()
                                .map(CheckedReferenceOriginProjection::Exact)
                                .collect::<Vec<_>>()
                    {
                        return None;
                    }
                    let source = self.captured_parameter_origin(state, &leaf, origin)?;
                    let returned = CheckedReturnedReferencePlan {
                        path: leaf.path.clone(),
                        source,
                    };
                    if !output.contains(&returned) {
                        output.push(returned);
                    }
                    covered = true;
                }
                for loan in site_loans.iter().filter(|loan| {
                    loan.root == facts::PlaceRoot::Symbol(origin.source.root)
                        && self.returned_loan_owner_path(state, loan).as_deref()
                            == Some(owner_path.as_slice())
                        && self.origin_matches_returned_loan(state, loan, origin)
                }) {
                    if leaf.source.access.borrow_access_kind()? != loan.kind {
                        return None;
                    }
                    covered = true;
                    for source in self.return_origin_with_frame(
                        state,
                        loan,
                        &leaf,
                        self.facts.borrow.access_segments.span(loan.segments)?,
                        &site_loans,
                        Some(&origins),
                    )? {
                        let returned = CheckedReturnedReferencePlan {
                            path: leaf.path.clone(),
                            source,
                        };
                        if !output.contains(&returned) {
                            output.push(returned);
                        }
                    }
                }
                if !covered {
                    if self.trace_return_state(state) {
                        eprintln!(
                            "CHECKSITE completion-uncovered-origin state={:?} leaf={:?} source={:?} extent={:?} loans={:?} loan_segments={:?}",
                            state.symbol,
                            leaf.path,
                            origin.source,
                            origin.extent,
                            site_loans,
                            site_loans
                                .iter()
                                .map(|loan| self.facts.borrow.access_segments.span(loan.segments))
                                .collect::<Vec<_>>()
                        );
                    }
                    return None;
                }
                captured = true;
            }
            // Neither a lifetime debt nor a missing body source proves that a
            // descriptor is absent. The complete occurrence relation must
            // establish that no activation-relative capture is owed here.
            if !captured && self.call_paths(call, active)?.contains(&leaf.path) {
                return None;
            }
        }
        Some(output)
    }

    fn checked_input_origin(
        &self,
        parameter: &typed_trees::signature::StateParameter,
        origin: &checked_trees::CheckedReferenceOriginSourcePlan,
        result_type: typed_trees::type_identity::TypeIdentityId,
    ) -> Option<(crate::LocalWriteSource, CapturedReturnPresentation)> {
        let ingress = super::super::stored_loan_parts(self.program, parameter.type_reference)
            .map_or(parameter.type_reference, |(referent, _)| referent);
        let mut entered = parameter.type_reference;
        let mut path = origin.path.as_slice();
        for _ in 0..origin.entry_reference_hops {
            let (
                CheckedReferenceOriginProjection::Exact(CheckedUnitStructuralPathSegment::Referent),
                remaining,
            ) = path.split_first()?
            else {
                return None;
            };
            entered = crate::reference_shell(self.program, entered)?.referee;
            path = remaining;
        }
        if origin.entry_reference_hops != 0
            && self.program.normalized_type_identity(entered)
                != self.program.normalized_type_identity(ingress)
        {
            return None;
        }
        // Substitute addresses only up to the first checked interpretation.
        // Following fields belong to that presentation's declaration and must
        // retain their typed coordinates, rather than become backing fields.
        let interpretation = path
            .iter()
            .position(|projection| {
                matches!(
                    projection,
                    CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::RepresentationView { .. }
                    )
                )
            })
            .unwrap_or(path.len());
        let (physical, presentation) = path.split_at(interpretation);
        let mut projected =
            super::super::origin_place_projection(self.program, ingress, physical.iter().cloned())?;
        let source_type = projected.type_identity(self.program);
        if !source_type.is_valid()
            || (presentation.is_empty() && source_type != origin.type_identity)
        {
            return None;
        }
        let source = crate::LocalWriteSource {
            invocation_output: None,
            invocation_result: None,
            is_image_storage: false,
            recasts: Vec::new(),
            representation: None,
            path: self.program.symbols.name(parameter.symbol).to_owned(),
            collection_coarse: false,
            root: parameter.symbol,
            segments: projected.segments,
        };
        Some((
            source,
            CapturedReturnPresentation {
                path: presentation.to_vec(),
                source_type,
                endpoint_type: origin.type_identity,
                result_type,
                extent: origin.extent,
            },
        ))
    }

    pub(super) fn captured_input_sources(
        &self,
        state: &typed_trees::state::State,
        reference: TypeReferenceHandle,
        operand_handle: arena::Handle<checked_trees::FlowOperandFact>,
        current: arena::Handle<checked_trees::FlowStorageSnapshot>,
        relative: &crate::LocalWriteSource,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<crate::LocalWriteSource>> {
        let actual = self.facts.flow.control.operands.get(operand_handle);
        // Pointer identity may come from an earlier capture; its pointee's
        // descriptor slots are still read at this invocation's consumer cut.
        let mut completed = *self.completed_operand_source(actual)?;
        completed.storage_snapshot = current;
        let operand = &completed;
        let mut relative = relative.clone();
        let selected = operand;
        let outer_reference = crate::reference_shell(self.program, reference).is_some();
        // A write-only input exposes its pointer, not the descriptors inside
        // its pointee. Keeping that outer type leaves pointer forwarding intact.
        let carrier =
            super::super::input_carrier_type(self.program, reference).unwrap_or(reference);
        let mut boundary = None;
        for leaf in declared_carrier_references(self.program, carrier)? {
            let segments = origin_place_segments(self.program, carrier, leaf.path.iter().cloned())?;
            // Forwarding the outer reference preserves that captured pointer.
            // A reference stored inside its record is a different pointer: read
            // that slot at this call's storage cut, including preceding writes.
            if outer_reference && segments.is_empty() {
                continue;
            }
            if relative.segments.starts_with(&segments) {
                if boundary.is_some() {
                    return None;
                }
                boundary = Some((leaf, segments));
            }
        }
        if let Some((leaf, segments)) = boundary {
            let path = leaf
                .path
                .iter()
                .map(|projection| match projection {
                    CheckedReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                    CheckedReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if !self.expression_leaf_present(
                state,
                operand.statement_index,
                operand.expression,
                reference,
                &path,
                Some(operand),
                active,
            )? {
                return Some(Vec::new());
            }
            relative.segments = relative
                .segments
                .strip_prefix(segments.as_slice())?
                .to_vec();
            let expected =
                crate::project_type_reference_from_segments(self.program, carrier, &segments)?;
            let actual_path = loan_place_segments(self.program, actual.type_reference, &path)?;
            return self.completed_descriptor_sources(
                state,
                operand_handle,
                &actual_path,
                expected,
                &relative,
                current,
                &mut Vec::new(),
            );
        }
        if !outer_reference {
            return None;
        }
        let referents = self
            .facts
            .flow
            .control
            .operand_referents
            .span(selected.referents)?;
        if referents.is_empty() {
            // An opaque incoming descriptor can retain a complete value/store
            // source without concrete raw referents. Consume that same checked
            // descriptor relation as aggregate leaves, at this operand's cut;
            // the exact returned-loan join below still owns its authority.
            return self.completed_descriptor_sources(
                state,
                operand_handle,
                &[],
                reference,
                &relative,
                current,
                &mut Vec::new(),
            );
        }
        let mut output = Vec::new();
        for referent in referents {
            let root = referent.root.as_symbol()?;
            let source = crate::LocalWriteSource {
                invocation_output: None,
                invocation_result: None,
                is_image_storage: false,
                recasts: Vec::new(),
                representation: None,
                path: self.program.symbols.name(root).to_owned(),
                collection_coarse: false,
                root,
                segments: self
                    .facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(referent.segments)?
                    .to_vec(),
            };
            let source = source.substitute_storage(self.program, &relative)?;
            if !output.contains(&source) {
                output.push(source);
            }
        }
        Some(output)
    }

    /// Follow the producer's identity-preserving whole-value links. A local
    /// copy can retain an incoming carrier without authoring new child values;
    /// its former local home supplies neither backing nor loan authority.
    pub(super) fn completed_operand_source<'fact>(
        &'fact self,
        mut operand: &'fact checked_trees::FlowOperandFact,
    ) -> Option<&'fact checked_trees::FlowOperandFact> {
        let mut visited = Vec::new();
        loop {
            let source = if operand.value_source.is_valid() {
                operand.value_source
            } else if crate::reference_shell(self.program, operand.type_reference).is_none()
                && self
                    .facts
                    .flow
                    .control
                    .operand_retentions
                    .is_valid(operand.retention)
            {
                let retained = self
                    .facts
                    .flow
                    .control
                    .operand_retentions
                    .get(operand.retention);
                // For an owned carrier this is its completed copied value.
                // For a reference it denotes the pointee instead, so that
                // association must never substitute descriptor identity.
                if !retained.stored_projection.is_empty() {
                    break;
                }
                retained.stored_value
            } else {
                break;
            };
            if !source.is_valid() {
                break;
            }
            if visited.contains(&source) || !self.facts.flow.control.operands.is_valid(source) {
                return None;
            }
            visited.push(source);
            let value = self.facts.flow.control.operands.get(source);
            if !crate::storage_origin_types_match(
                self.program,
                operand.type_reference,
                value.type_reference,
            ) {
                return None;
            }
            operand = value;
        }
        Some(operand)
    }

    pub(super) fn completed_operand_leaf<'fact>(
        &'fact self,
        mut operand: &'fact checked_trees::FlowOperandFact,
        mut path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<&'fact checked_trees::FlowOperandFact> {
        while !path.is_empty() {
            operand = self.completed_operand_source(operand)?;
            let mut selected = None;
            for child in self
                .facts
                .flow
                .control
                .operand_children
                .span(operand.constructed_children())?
            {
                let (destination, _) = super::super::source_paths::structural_path(
                    self.program,
                    operand.type_reference,
                    self.facts
                        .flow
                        .control
                        .operand_child_paths
                        .span(child.path)?,
                )?;
                if !destination.is_empty() && path.starts_with(&destination) {
                    if selected.is_some() || !self.facts.flow.control.operands.is_valid(child.value)
                    {
                        return None;
                    }
                    selected = Some((
                        self.facts.flow.control.operands.get(child.value),
                        destination.len(),
                    ));
                }
            }
            let (child, length) = selected?;
            if child.state_symbol != operand.state_symbol
                || child.statement_index != operand.statement_index
            {
                return None;
            }
            operand = child;
            path = &path[length..];
        }
        Some(operand)
    }
}

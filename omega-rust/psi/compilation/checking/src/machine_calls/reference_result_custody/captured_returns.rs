//! Executable result custody consumes the borrow producer's normal-return rows.
//! These rows describe current backing, including replacements; a typed local
//! initializer describes neither the return epoch nor an operand capture.
//! Missing rows are not an empty result. Only the retained value/store graph
//! can establish an inactive descriptor slot. Source admission keeps its
//! independent declaration-level lifetime analysis and does not call this query.

use super::{
    StoredCarrierSource, is_mutable_structural_parameter, loan_owner_path, loan_place_segments,
    record_carrier_sources, reference_access_can_supply, stored_carrier_sources, stored_loan_parts,
};
use checked_trees::{
    CheckFacts, CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReturnedReferencePlan, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Complete callee-relative origins at normal return, or unavailable capture.
pub fn returned_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
) -> Option<Vec<CheckedReturnedReferencePlan>> {
    Captures { program, facts }.returned(state, &mut Vec::new())
}

pub fn call_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    target: SymbolHandle,
) -> Option<Vec<CheckedReturnedReferencePlan>> {
    Captures { program, facts }.target_sources(target, &mut Vec::new())
}

/// A terminal call has no expression-local holder. Its normal-return capture
/// owns the same ingress relation as an expression return, selected by the
/// exact flow occurrence rather than reconstructed from argument syntax.
pub(super) fn terminal_call_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<Vec<CheckedReturnedReferencePlan>> {
    let captures = Captures { program, facts };
    let flow = captures.flow(state.symbol)?;
    let mut occurrences = facts
        .flow
        .control
        .calls
        .span(flow.calls)?
        .iter()
        .filter(|candidate| {
            candidate.statement_index == call.statement_index
                && !candidate.authored_expression.is_valid()
        });
    if occurrences.next()? != call || occurrences.next().is_some() {
        return None;
    }
    captures.normal_returns(state, &mut vec![state.symbol], Some(call.statement_index))
}

/// Actual captures can remove inactive formal case slots, never create loans.
pub(super) fn call_result_paths(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    occurrence: &checked_trees::FlowCallFact,
) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
    Captures { program, facts }.call_paths(occurrence, &mut Vec::new())
}

struct Captures<'capture> {
    program: &'capture dyn crate::CheckingInput,
    facts: &'capture CheckFacts,
}

impl Captures<'_> {
    fn target_sources(
        &self,
        target: SymbolHandle,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        if let Some(slot) = self.program.dynamic_slot_by_symbol(target) {
            return super::declared_returns::returned_sources(
                self.program,
                &slot.parameters,
                slot.return_type,
            )
            .ok();
        }
        self.returned(crate::call_target_state(self.program, target)?, active)
    }

    fn returned(
        &self,
        state: &typed_trees::state::State,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        if active.contains(&state.symbol) || active.len() >= super::MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        active.push(state.symbol);
        let result = self.normal_returns(state, active, None);
        active.pop();
        result
    }

    fn normal_returns(
        &self,
        state: &typed_trees::state::State,
        active: &mut Vec<SymbolHandle>,
        selected_statement: Option<usize>,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        let mut states = self
            .facts
            .borrow
            .states
            .iter()
            .filter_map(|(_, candidate)| {
                (candidate.state_symbol == state.symbol).then_some(candidate)
            });
        let borrowed = states.next()?;
        if states.next().is_some() {
            return None;
        }
        let loans = self
            .facts
            .borrow
            .returned_loans
            .span(borrowed.returned_loans)?;
        let leaves = record_carrier_sources(
            self.program,
            state.return_type,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: state.symbol,
            },
        )?;
        let statements = typed_trees::statement::reachable_state_statements(
            self.program
                .statement_table
                .statements(state.statement_nodes),
        );
        let mut output = Vec::new();
        let mut completed = false;
        for (statement_index, statement) in statements.iter().enumerate() {
            if selected_statement.is_some_and(|selected| selected != statement_index) {
                continue;
            }
            let expression = match statement {
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    Some(*expression)
                }
                StatementNode::Transition(transition) => {
                    if matches!(transition.exit, TransitionExit::Crash(_)) {
                        continue;
                    }
                    if transition.continuation.is_valid() {
                        return None;
                    }
                    match self
                        .program
                        .statement_table
                        .transition_target(transition.target)
                    {
                        TransitionTargetNode::Value(expression) => Some(*expression),
                        TransitionTargetNode::Named { path, .. } => {
                            if let Some(destination) = self.program.state_by_symbol(path.symbol)
                                && self.flow(destination.symbol)?.machine_symbol
                                    == self.flow(state.symbol)?.machine_symbol
                                && !self.normal_completion_possible(destination)?
                            {
                                // A state transfer into a proved nonreturning subgraph
                                // contributes no machine result. Missing capture rows on
                                // any potentially normal route still decline below.
                                continue;
                            }
                            None
                        }
                        TransitionTargetNode::Terminal => return None,
                    }
                }
                _ => continue,
            };
            completed = true;
            for leaf in &leaves {
                let owner_path = loan_owner_path(self.program, state.return_type, &leaf.path)?;
                let mut captured = false;
                for loan in loans
                    .iter()
                    .filter(|loan| loan.statement_index == statement_index)
                {
                    if self.facts.borrow.owner_segments.span(loan.owner_path)? != owner_path {
                        continue;
                    }
                    if leaf.source.access.borrow_access_kind()? != loan.kind {
                        return None;
                    }
                    captured = true;
                    for origin in self.return_origins(state, loan, leaf, expression, loans)? {
                        let source = CheckedReturnedReferencePlan {
                            path: leaf.path.clone(),
                            source: origin,
                        };
                        if !output.contains(&source) {
                            output.push(source);
                            if output.len() > super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                }
                if !captured {
                    let present = if let Some(expression) = expression {
                        self.expression_leaf_present(
                            state,
                            statement_index,
                            expression,
                            state.return_type,
                            &leaf.path,
                            None,
                            active,
                        )?
                    } else {
                        let flow = self.flow(state.symbol)?;
                        let mut calls = self
                            .facts
                            .flow
                            .control
                            .calls
                            .span(flow.calls)?
                            .iter()
                            .filter(|call| {
                                call.statement_index == statement_index
                                    && !call.authored_expression.is_valid()
                            });
                        let call = calls.next()?;
                        if calls.next().is_some() {
                            return None;
                        }
                        self.call_paths(call, active)?.contains(&leaf.path)
                    };
                    // A live leaf without its formation is incomplete, not empty.
                    if present {
                        return None;
                    }
                }
            }
        }
        completed.then_some(output)
    }

    /// Conservative normal-completion reachability over admitted state control.
    /// Coverage has already ruled out implicit misses; explicit crash edges
    /// never return. Internal transfers retain their exact declaration owner.
    /// Ordinary results and external calls remain potentially normal, without
    /// using a failed origin query as evidence that a result is absent.
    fn normal_completion_possible(&self, initial: &typed_trees::state::State) -> Option<bool> {
        let owner = self.flow(initial.symbol)?.machine_symbol;
        let mut pending = vec![initial.symbol];
        let mut visited = Vec::new();
        while let Some(symbol) = pending.pop() {
            if visited.contains(&symbol) {
                continue;
            }
            visited.push(symbol);
            let state = self.program.state_by_symbol(symbol)?;
            if self.flow(symbol)?.machine_symbol != owner {
                return Some(true);
            }
            let statements = typed_trees::statement::reachable_state_statements(
                self.program
                    .statement_table
                    .statements(state.statement_nodes),
            );
            let mut has_transition = false;
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    StatementNode::Expression(_) if index + 1 == statements.len() => {
                        return Some(true);
                    }
                    StatementNode::Transition(transition) => {
                        has_transition = true;
                        if matches!(transition.exit, TransitionExit::Crash(_)) {
                            continue;
                        }
                        for target in transition.targets() {
                            match self.program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named { path, .. } => {
                                    let Some(destination) =
                                        self.program.state_by_symbol(path.symbol)
                                    else {
                                        return Some(true);
                                    };
                                    if self.flow(destination.symbol)?.machine_symbol != owner {
                                        return Some(true);
                                    }
                                    pending.push(destination.symbol);
                                }
                                TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                    return Some(true);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // An admitted body without an explicit control edge may complete
            // implicitly as Unit; lack of capture rows cannot erase that path.
            if !has_transition {
                return Some(true);
            }
        }
        Some(false)
    }

    /// Loan rows own lifetime and access. The completed value owns evaluated
    /// coordinates: a selector's later value cannot retarget its captured loan.
    /// Both records must describe this exact return occurrence and result leaf.
    fn return_origins(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        expression: Option<ExpressionHandle>,
        loans: &[checked_trees::BorrowReturnedLoanFact],
    ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
        let segments = self.facts.borrow.access_segments.span(loan.segments)?;
        if let Some(source) = self.return_origin(state, loan, leaf, segments, loans) {
            return Some(source);
        }
        // Incoming stored descriptors have a separate opaque origin identity.
        // A root address alone cannot replace their exact owner-path relation.
        if loan.incoming_reference.is_valid() {
            return None;
        }
        let operand =
            self.completed_return_operand(state, loan.statement_index, expression?, &leaf.path)?;
        if operand.expression != loan.formation_expression {
            return None;
        }
        let referents = self
            .facts
            .flow
            .control
            .operand_referents
            .span(operand.referents)?;
        if referents.is_empty() {
            return None;
        }
        let owner_path = self.facts.borrow.owner_segments.span(loan.owner_path)?;
        let mut output = Vec::new();
        for referent in referents {
            let captured = self
                .facts
                .flow
                .control
                .operand_referent_segments
                .span(referent.segments)?;
            // Cover the complete evaluated origin set. A capture under another
            // root must have its own lifetime row for the same result leaf;
            // selecting just the convenient alternative would lose custody.
            if !loans.iter().any(|candidate| {
                candidate.statement_index == loan.statement_index
                    && candidate.formation_expression == operand.expression
                    && candidate.root == referent.root
                    && candidate.kind == loan.kind
                    && !candidate.incoming_reference.is_valid()
                    && self.facts.borrow.owner_segments.span(candidate.owner_path)
                        == Some(owner_path)
                    && self
                        .facts
                        .borrow
                        .access_segments
                        .span(candidate.segments)
                        .is_some_and(|segments| captured_address_refines(segments, captured))
            }) {
                return None;
            }
            if referent.root == loan.root && captured_address_refines(segments, captured) {
                for origin in self.return_origin(state, loan, leaf, captured, loans)? {
                    if !output.contains(&origin) {
                        output.push(origin);
                    }
                }
            }
        }
        (!output.is_empty()).then_some(output)
    }

    fn completed_return_operand(
        &self,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
        mut path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<&checked_trees::FlowOperandFact> {
        let flow = self.flow(state.symbol)?;
        let control = &self.facts.flow.control;
        let mut exits = control.exits.span(flow.exits)?.iter().filter(|exit| {
            exit.machine_symbol == flow.machine_symbol
                && exit.state_symbol == state.symbol
                && exit.statement_index == statement_index
        });
        let exit = exits.next()?;
        if exits.next().is_some() || !control.operands.is_valid(exit.returned_value) {
            return None;
        }
        let mut operand = control.operands.get(exit.returned_value);
        if operand.state_symbol != state.symbol
            || operand.statement_index != statement_index
            || operand.expression != expression
            || self
                .program
                .normalized_type_identity(operand.type_reference)
                != self.program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        while !path.is_empty() {
            let mut selected = None;
            for child in control.operand_children.span(operand.children)? {
                let (destination, _) = self.structural_path(
                    operand.type_reference,
                    control.operand_child_paths.span(child.path)?,
                )?;
                if !destination.is_empty() && path.starts_with(&destination) {
                    if selected.is_some() || !control.operands.is_valid(child.value) {
                        return None;
                    }
                    selected = Some((control.operands.get(child.value), destination.len()));
                }
            }
            let (child, length) = selected?;
            if child.state_symbol != state.symbol || child.statement_index != statement_index {
                return None;
            }
            operand = child;
            path = &path[length..];
        }
        Some(operand)
    }

    fn return_origin(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        segments: &[facts::PlaceSegment],
        loans: &[checked_trees::BorrowReturnedLoanFact],
    ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
        let incoming = if loan.incoming_reference.is_valid() {
            if !self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
            {
                return None;
            }
            let incoming = self
                .facts
                .borrow
                .incoming_references
                .get(loan.incoming_reference);
            if incoming.state_symbol != state.symbol
                || loan.root != facts::PlaceRoot::Symbol(incoming.parameter_symbol)
            {
                return None;
            }
            Some(incoming)
        } else {
            None
        };
        let (parameter_index, parameter) = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| is_mutable_structural_parameter(self.program, parameter))
            .enumerate()
            .find(|(_, parameter)| loan.root == facts::PlaceRoot::Symbol(parameter.symbol))?;
        let mut path = Vec::new();
        let mut incoming_segments = Vec::new();
        let mut selected = parameter.type_reference;
        if let Some(incoming) = incoming {
            let carrier = super::input_carrier_type(self.program, parameter.type_reference)?;
            if carrier != incoming.carrier_type {
                return None;
            }
            let owner = self.facts.borrow.owner_segments.span(incoming.owner_path)?;
            let incoming_leaf = super::declared_carrier_references(self.program, carrier)?
                .into_iter()
                .find(|source| {
                    super::origin_owner_path(self.program, carrier, source.path.iter().cloned())
                        .as_deref()
                        == Some(owner)
                })?;
            if !reference_access_can_supply(incoming_leaf.access, leaf.source.access) {
                return None;
            }
            if stored_loan_parts(self.program, parameter.type_reference).is_some() {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
            }
            let incoming_path = super::origin_place_segments(
                self.program,
                carrier,
                incoming_leaf.path.iter().cloned(),
            )?;
            selected =
                crate::project_type_reference_from_segments(self.program, carrier, &incoming_path)?;
            if incoming_leaf.crosses_reference {
                selected = stored_loan_parts(self.program, selected)?.0;
            }
            path.extend(incoming_leaf.path);
            if incoming_leaf.crosses_reference {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
            }
            incoming_segments = incoming_path;
        } else {
            // Incoming borrow authority is independent of its descriptor ABI:
            // slices supply element storage even though they are not stored
            // pointer-shaped references.
            let shell = crate::reference_shell(self.program, selected)?;
            if !reference_access_can_supply(shell.access.into(), leaf.source.access) {
                return None;
            }
            if self.program.normalized_type_identity(selected) != leaf.source.type_identity {
                selected = shell.referee;
            }
        }
        // Returned loans retain address selections across each parent view.
        // Compose those frozen windows before turning the address into typed
        // structural projections; a window offset is not an array dimension.
        let mut segments = segments.to_vec();
        facts::compose_captured_view_segments(&mut segments)?;
        let parameter_index = u32::try_from(parameter_index).ok()?;
        let frame = crate::CallFrameResolver::new(self.program)?;
        let result_path = loan_place_segments(self.program, state.return_type, &leaf.path)?;
        let origins = frame.reference_result_leaf_origins(state.symbol);
        let has_presentation = origins.as_ref().is_some_and(|origins| {
            origins.iter().any(|origin| {
                origin.result_path == result_path
                    && (!origin.source.recasts.is_empty() || origin.source.representation.is_some())
            })
        }) || self.facts.values.structural_values.nodes.iter().any(
            |(_, value)| {
                value.expression == loan.formation_expression
                    && matches!(
                        value.kind,
                        CheckedStructuralValueKind::Reference {
                            storage: checked_trees::CheckedReferenceStorage::View { .. },
                            ..
                        }
                    )
            },
        );
        // A captured loan that already reaches the returned type needs no
        // body-summary refinement. Its frozen coordinates remain authoritative
        // even when a later rebind makes the frame summary unavailable. The
        // body relation below is needed only to refine a wider lifetime region
        // or recover a checked presentation of another endpoint type.
        // A retained interpretation still needs its codec, even if its target
        // type equals the original endpoint. Type equality cannot erase a view.
        if !has_presentation
            && let Some((suffix, endpoint)) = self.origin_path(selected, &segments)
            && self.program.normalized_type_identity(endpoint) == leaf.source.type_identity
        {
            path.extend(suffix);
            return Some(vec![CheckedReferenceOriginSourcePlan {
                parameter_index,
                path,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                type_identity: leaf.source.type_identity,
                access: leaf.source.access,
            }]);
        }
        if let Some(origins) = origins {
            // This is a state-wide union, not a menu from which one loan may
            // select a convenient alternative. Every candidate for the leaf
            // must have actual returned custody, even if another normal exit
            // supplies it. An unresolved root cannot disappear behind a known
            // source. Per-loan publication below then authenticates every
            // covered candidate's declaration path and presented endpoint.
            let owner_path = self.facts.borrow.owner_segments.span(loan.owner_path)?;
            for origin in origins
                .iter()
                .filter(|origin| origin.result_path == result_path)
            {
                if !origin.source.root.is_valid()
                    || !loans.iter().any(|candidate| {
                        candidate.root == facts::PlaceRoot::Symbol(origin.source.root)
                            && candidate.kind == loan.kind
                            && self.facts.borrow.owner_segments.span(candidate.owner_path)
                                == Some(owner_path)
                            && self
                                .facts
                                .borrow
                                .access_segments
                                .span(candidate.segments)
                                .is_some_and(|segments| {
                                    frame_origin_fits_loan(&origin.source, segments)
                                })
                    })
                {
                    return None;
                }
            }
            let matching = origins
                .iter()
                .filter(|origin| {
                    origin.result_path == result_path
                        && loan.root == facts::PlaceRoot::Symbol(origin.source.root)
                        && frame_origin_fits_loan(&origin.source, &segments)
                })
                .collect::<Vec<_>>();
            if !matching.is_empty() {
                let mut output = Vec::new();
                for origin in matching {
                    let (suffix, presented) =
                        self.frame_origin_path(selected, &incoming_segments, &origin.source)?;
                    if self.program.normalized_type_identity(presented) != leaf.source.type_identity
                    {
                        return None;
                    }
                    let mut complete = path.clone();
                    complete.extend(suffix);
                    let source = CheckedReferenceOriginSourcePlan {
                        parameter_index,
                        path: complete,
                        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                        type_identity: leaf.source.type_identity,
                        access: leaf.source.access,
                    };
                    if !output.contains(&source) {
                        output.push(source);
                    }
                }
                return (!output.is_empty()).then_some(output);
            }
        }
        // Loan formation can capture an input region without a public typed
        // subprojection, notably for a bodyless call's returned descriptor.
        // Keep the captured region as a region: inventing an element or field
        // here would pretend the public signature selected a private address.
        // The original loan/root/access joins above remain mandatory, and
        // completion must check the actual returned footprint's containment.
        let (suffix, mut endpoint) = self.origin_path(selected, &segments)?;
        path.extend(suffix);
        if path.is_empty()
            && stored_loan_parts(self.program, parameter.type_reference).is_none()
            && crate::reference_shell(self.program, parameter.type_reference)
                .is_some_and(|shell| shell.referee == endpoint)
        {
            // A view formal presents its descriptor, not a bare unsized slice.
            // With no subprojection, keep the same endpoint identity as its
            // declared region and executable argument presentation.
            endpoint = parameter.type_reference;
        }
        Some(vec![CheckedReferenceOriginSourcePlan {
            parameter_index,
            path,
            extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
            type_identity: self.program.normalized_type_identity(endpoint),
            access: leaf.source.access,
        }])
    }

    /// The complete body relation refines a lifetime loan's storage region.
    /// These are origin summaries, not executable callee selectors. Re-views
    /// additionally require their actual value formation and retained codec;
    /// equal byte geometry cannot rename the original backing declaration.
    fn frame_origin_path(
        &self,
        mut selected: TypeReferenceHandle,
        incoming: &[facts::PlaceSegment],
        origin: &crate::LocalWriteSource,
    ) -> Option<(Vec<CheckedReferenceOriginProjection>, TypeReferenceHandle)> {
        if origin.recasts.is_empty() {
            if origin.representation.is_some() {
                return None;
            }
            return self.origin_path(selected, origin.segments.strip_prefix(incoming)?);
        }
        let mut path = Vec::new();
        for (ordinal, recast) in origin.recasts.iter().enumerate() {
            if recast.root != origin.root {
                return None;
            }
            let selectors = if ordinal == 0 {
                recast.segments.strip_prefix(incoming)?
            } else {
                let (previous, selectors) = recast.representation.as_ref()?;
                if self.program.normalized_type_identity(*previous)
                    != self.program.normalized_type_identity(selected)
                {
                    return None;
                }
                selectors
            };
            let (projection, endpoint) = self.origin_path(selected, selectors)?;
            path.extend(projection);
            selected = endpoint;
            if let Some(shell) = crate::reference_shell(self.program, selected) {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
                selected = shell.referee;
            }
            let mut formations =
                self.facts
                    .values
                    .structural_values
                    .nodes
                    .iter()
                    .filter_map(|(_, value)| {
                        if value.expression != recast.formation_expression {
                            return None;
                        }
                        let CheckedStructuralValueKind::Reference {
                            storage,
                            backing_type,
                            ..
                        } = &value.kind
                        else {
                            return None;
                        };
                        Some((value, *storage, *backing_type))
                    });
            let (formation, storage, backing_type) = formations.next()?;
            if formations.next().is_some()
                || self.program.normalized_type_identity(backing_type)
                    != self.program.normalized_type_identity(selected)
                || !matches!(storage, checked_trees::CheckedReferenceStorage::View { .. })
                || storage != crate::reference_view_storage(self.program, recast.target_type)?
                || self.program.normalized_type_identity(
                    crate::reference_shell(self.program, formation.type_reference)?.referee,
                ) != self.program.normalized_type_identity(recast.target_type)
            {
                return None;
            }
            // The arena occurrence remains in its original body after a call
            // substitution. Its source identity is not the rebased caller root.
            self.program.state_by_symbol(formation.state)?;
            path.push(CheckedReferenceOriginProjection::Exact(
                CheckedUnitStructuralPathSegment::RepresentationView {
                    type_identity: self.program.normalized_type_identity(recast.target_type),
                    storage,
                },
            ));
            selected = recast.target_type;
        }
        let (presentation, selectors) = origin.representation.as_ref()?;
        if self.program.normalized_type_identity(*presentation)
            != self.program.normalized_type_identity(selected)
        {
            return None;
        }
        let (suffix, endpoint) = self.origin_path(selected, selectors)?;
        path.extend(suffix);
        Some((path, endpoint))
    }

    fn call_paths(
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
            let parameter_index = source.source.parameter_index;
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

    fn expression_leaf_present(
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
                StoredCarrierSource::Borrow { .. } | StoredCarrierSource::Image => true,
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

    fn flow(&self, state: SymbolHandle) -> Option<&checked_trees::FlowStateFact> {
        let mut states = self
            .facts
            .flow
            .control
            .states
            .iter()
            .filter_map(|(_, flow)| (flow.state_symbol == state).then_some(flow));
        let flow = states.next()?;
        states.next().is_none().then_some(flow)
    }

    fn place_leaf_present(
        &self,
        state: &typed_trees::state::State,
        before: usize,
        root: facts::PlaceRoot,
        segments: &[facts::PlaceSegment],
        capture: Option<&checked_trees::FlowOperandFact>,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        // Constructors and fresh borrows retain their shape directly. Only a
        // place reconstructed from preceding stores depends on an unchanged
        // statement-entry observation. Unrelated earlier writes do not erase
        // this captured value; overlapping or unknown writes stop the walk.
        if let Some(capture) = capture {
            if capture.entry_origin_unknown_write {
                return None;
            }
            for write in self
                .facts
                .flow
                .control
                .operand_origin_writes
                .span(capture.entry_origin_writes)?
            {
                let path = self
                    .facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(write.segments)?;
                if write.root == root && paths_overlap(path, segments) {
                    return None;
                }
            }
        }
        let flow = self.flow(state.symbol)?;
        let semantic = &self.facts.semantic;
        for statement in self
            .facts
            .flow
            .control
            .statements
            .span(flow.statements)?
            .iter()
            .rev()
            .filter(|statement| statement.statement_index < before)
        {
            if semantic.places.is_valid(statement.stored_destination) {
                let destination = semantic.places.get(statement.stored_destination);
                let destination_path = semantic.place_segments.span(destination.segments)?;
                if destination.root == root && segments.starts_with(destination_path) {
                    if !self
                        .facts
                        .flow
                        .control
                        .operands
                        .is_valid(statement.stored_value)
                    {
                        return None;
                    }
                    let stored = self.facts.flow.control.operands.get(statement.stored_value);
                    let (path, _) = self.structural_path(
                        stored.type_reference,
                        &segments[destination_path.len()..],
                    )?;
                    return self.expression_leaf_present(
                        state,
                        stored.statement_index,
                        stored.expression,
                        stored.type_reference,
                        &path,
                        Some(stored),
                        active,
                    );
                }
            }
            if statement.completed_unknown_write {
                return None;
            }
            for write in self
                .facts
                .flow
                .control
                .operand_origin_writes
                .span(statement.completed_writes)?
            {
                let path = self
                    .facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(write.segments)?;
                if write.root == root && paths_overlap(path, segments) {
                    return None;
                }
            }
        }
        // Opaque ingress may contain this slot, but it must have the complete
        // retained input roster. A local without a produced store is missing
        // capture data, not proof of either presence or an empty value.
        self.ingress_present(state, root.as_symbol()?)
    }

    fn ingress_present(
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
        super::stored_carrier_ingress_is_complete(
            self.program,
            self.facts,
            flow.machine_symbol,
            state.symbol,
            symbol,
            parameter.type_reference,
        )
        .then_some(true)
    }

    /// Runtime selections describe one typed element, not a finite list of
    /// possible addresses. Exact formation and bounds remain in the loan rows.
    fn origin_path(
        &self,
        mut selected: TypeReferenceHandle,
        mut segments: &[facts::PlaceSegment],
    ) -> Option<(Vec<CheckedReferenceOriginProjection>, TypeReferenceHandle)> {
        let mut output = Vec::new();
        // Cross-call loan summaries erase callee-local selectors. The absent
        // coordinate still denotes one typed element; it is not a dangling
        // expression or a surviving range expression. Bounds and the evaluated
        // address remain with the invocation's captured descriptor.
        let is_element_selection = |expression: ExpressionHandle| {
            !expression.is_valid()
                || self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                    && self
                        .program
                        .expression_table
                        .range_node(expression)
                        .is_none()
        };
        loop {
            let selection = segments.iter().position(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Index { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                )
            });
            let prefix_length = selection.unwrap_or(segments.len());
            let (prefix, endpoint) = self.structural_path(selected, &segments[..prefix_length])?;
            output.extend(
                prefix
                    .into_iter()
                    .map(CheckedReferenceOriginProjection::Exact),
            );
            selected = endpoint;
            let Some(position) = selection else {
                return Some((output, selected));
            };
            selected = crate::peel_declared_constraints(self.program, selected);
            if let Some(shell) = crate::reference_shell(self.program, selected) {
                output.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
                selected = crate::peel_declared_constraints(self.program, shell.referee);
            }
            let (TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type }) =
                self.program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            selected = *element_type;
            let (projection, consumed) = match segments[position] {
                facts::PlaceSegment::FixedIndex { index } => (
                    CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(index).ok()?),
                    ),
                    1,
                ),
                facts::PlaceSegment::Index { expression } if is_element_selection(expression) => {
                    (CheckedReferenceOriginProjection::Element, 1)
                }
                facts::PlaceSegment::FixedRange { start, end } => {
                    // A view window and its element selector share one array
                    // dimension. This is provenance, not a new address: the
                    // descriptor retains its captured window, selector and
                    // bound. Never consult the selector's later value here.
                    let projection = match segments.get(position.checked_add(1)?)? {
                        facts::PlaceSegment::Index { expression }
                            if start < end && is_element_selection(*expression) =>
                        {
                            CheckedReferenceOriginProjection::Element
                        }
                        facts::PlaceSegment::FixedIndex { index }
                            if *index < end.checked_sub(start)? =>
                        {
                            CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::FixedIndex(
                                    u64::try_from(start.checked_add(*index)?).ok()?,
                                ),
                            )
                        }
                        _ => return None,
                    };
                    (projection, 2)
                }
                _ => return None,
            };
            output.push(projection);
            segments = &segments[position + consumed..];
        }
    }

    fn structural_path(
        &self,
        mut selected: TypeReferenceHandle,
        segments: &[facts::PlaceSegment],
    ) -> Option<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle)> {
        let mut path = Vec::new();
        let mut payload = None;
        for segment in segments {
            selected = crate::peel_declared_constraints(self.program, selected);
            if let Some(shell) = crate::reference_shell(self.program, selected) {
                selected = shell.referee;
                path.push(CheckedUnitStructuralPathSegment::Referent);
            }
            match segment {
                facts::PlaceSegment::Case { variant } => {
                    let data = self.program.closed_data_definition(selected)?;
                    let case =
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Variant(case)
                                    if case.symbol == *variant =>
                                {
                                    Some(case)
                                }
                                _ => None,
                            })?;
                    path.push(CheckedUnitStructuralPathSegment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    ));
                    payload = Some(case);
                }
                facts::PlaceSegment::Field { symbol } => {
                    let field = if let Some(case) = payload.take() {
                        self.program
                            .data_payload_fields(case)
                            .iter()
                            .find(|field| field.symbol == *symbol)?
                    } else {
                        let data = self.program.closed_data_definition(selected)?;
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.symbol == *symbol =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })?
                    };
                    path.push(CheckedUnitStructuralPathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ));
                    selected = field.type_reference;
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    let TypeReferenceNode::FixedArray { element_type, .. } =
                        self.program.type_reference_table.type_reference(selected)
                    else {
                        return None;
                    };
                    selected = *element_type;
                    path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(*index).ok()?,
                    ));
                }
                _ => return None,
            }
        }
        Some((path, selected))
    }
}

/// Match a lifetime capture in its original declaration namespace, retaining
/// the exact presentation coordinate when the loan already captured that view.
fn frame_origin_fits_loan(origin: &crate::LocalWriteSource, loan: &[facts::PlaceSegment]) -> bool {
    // Lifetime capture can conservatively lend a whole input region. The
    // complete body relation narrows its declaration-level origin only within
    // that captured region; it cannot pick an unrelated field or storage root.
    let within = |candidate: &[facts::PlaceSegment]| {
        captured_address_refines(candidate, loan)
            || candidate
                .get(..loan.len())
                .is_some_and(|prefix| captured_address_refines(loan, prefix))
    };
    if within(&origin.segments) {
        return true;
    }
    // A call's declared lifetime may conservatively lend the original region
    // while its checked body returns a narrower presented subregion. Those
    // selectors belong to different declarations. Match the loan against the
    // original predecessor, never against same-spelled fields in the view.
    // Publication separately authenticates every view issuer and backing type;
    // this comparison grants neither a presentation nor a new storage root.
    origin
        .recasts
        .first()
        .is_some_and(|formation| formation.root == origin.root && within(&formation.segments))
}

/// Compare exact evaluated coordinates. Dynamic selectors may refine, but
/// static fields, cases, windows and later array dimensions stay fixed.
fn captured_address_refines(
    segments: &[facts::PlaceSegment],
    captured: &[facts::PlaceSegment],
) -> bool {
    use facts::PlaceSegment;
    let mut segments = segments.to_vec();
    if facts::compose_captured_view_segments(&mut segments).is_none() {
        return false;
    }
    let mut remaining = segments.as_slice();
    for &coordinate in captured {
        remaining = match (remaining, coordinate) {
            (
                [
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::Index { .. },
                    suffix @ ..,
                ],
                PlaceSegment::FixedIndex { index },
            ) if *start <= index && index < *end => suffix,
            ([PlaceSegment::Index { .. }, suffix @ ..], PlaceSegment::FixedIndex { .. }) => suffix,
            ([expected, suffix @ ..], actual) if *expected == actual => suffix,
            _ => return false,
        };
    }
    remaining.is_empty()
}

fn paths_overlap(left: &[facts::PlaceSegment], right: &[facts::PlaceSegment]) -> bool {
    for (left, right) in left.iter().zip(right) {
        if left == right {
            continue;
        }
        return !matches!(
            (left, right),
            (
                facts::PlaceSegment::Field { .. },
                facts::PlaceSegment::Field { .. }
            ) | (
                facts::PlaceSegment::Case { .. },
                facts::PlaceSegment::Case { .. }
            ) | (
                facts::PlaceSegment::FixedIndex { .. },
                facts::PlaceSegment::FixedIndex { .. }
            )
        );
    }
    true
}

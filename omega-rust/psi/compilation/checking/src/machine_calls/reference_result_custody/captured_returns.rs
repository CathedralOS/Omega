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
    CheckFacts, CheckedReferenceResultSourcePlan, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
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
) -> Option<Vec<CheckedReferenceResultSourcePlan>> {
    Captures { program, facts }.returned(state, &mut Vec::new())
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
    fn returned(
        &self,
        state: &typed_trees::state::State,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReferenceResultSourcePlan>> {
        if active.contains(&state.symbol) || active.len() >= super::MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        active.push(state.symbol);
        let result = self.normal_returns(state, active);
        active.pop();
        result
    }

    fn normal_returns(
        &self,
        state: &typed_trees::state::State,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReferenceResultSourcePlan>> {
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
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let mut output = Vec::new();
        let mut completed = false;
        for (statement_index, statement) in statements.iter().enumerate() {
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
                        TransitionTargetNode::Named { .. } => None,
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
                        let source = CheckedReferenceResultSourcePlan {
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
    ) -> Option<Vec<CheckedUnitStructuralArgumentPlan>> {
        let segments = self.facts.borrow.access_segments.span(loan.segments)?;
        if let Some(source) = self.return_origin(state, loan, leaf, segments) {
            return Some(vec![source]);
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
                let origin = self.return_origin(state, loan, leaf, captured)?;
                if !output.contains(&origin) {
                    output.push(origin);
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
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
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
        let mut selected = parameter.type_reference;
        if let Some(incoming) = incoming {
            let carrier = super::input_carrier_type(self.program, parameter.type_reference)?;
            if carrier != incoming.carrier_type {
                return None;
            }
            let owner = self.facts.borrow.owner_segments.span(incoming.owner_path)?;
            let incoming_leaf = record_carrier_sources(
                self.program,
                carrier,
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(parameter_index).ok()?,
                },
            )?
            .into_iter()
            .find(|source| {
                loan_owner_path(self.program, carrier, &source.path).as_deref() == Some(owner)
            })?;
            if !reference_access_can_supply(incoming_leaf.source.access, leaf.source.access) {
                return None;
            }
            if crate::reference_shell(self.program, parameter.type_reference).is_some() {
                path.push(CheckedUnitStructuralPathSegment::Referent);
            }
            let incoming_path = loan_place_segments(self.program, carrier, &incoming_leaf.path)?;
            selected =
                crate::project_type_reference_from_segments(self.program, carrier, &incoming_path)?;
            if matches!(
                incoming_leaf.source.path.last(),
                Some(CheckedUnitStructuralPathSegment::Referent)
            ) {
                selected = stored_loan_parts(self.program, selected)?.0;
            }
            path.extend(incoming_leaf.source.path);
        } else {
            let (referent, access) = stored_loan_parts(self.program, selected)?;
            if !reference_access_can_supply(access, leaf.source.access) {
                return None;
            }
            if self.program.normalized_type_identity(selected) != leaf.source.type_identity {
                selected = referent;
            }
        }
        // Returned loans retain address selections across each parent view.
        // Compose those frozen windows before turning the address into typed
        // structural projections; a window offset is not an array dimension.
        let mut segments = segments.to_vec();
        facts::compose_captured_view_segments(&mut segments)?;
        let (suffix, selected) = self.structural_path(selected, &segments)?;
        path.extend(suffix);
        if self.program.normalized_type_identity(selected) != leaf.source.type_identity {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: leaf.source.type_identity,
            access: leaf.source.access,
        })
    }

    fn call_paths(
        &self,
        call: &checked_trees::FlowCallFact,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<Vec<CheckedUnitStructuralPathSegment>>> {
        let destination = self.program.state_by_symbol(call.target_symbol)?;
        let sources = self.returned(destination, active)?;
        let operands = self
            .facts
            .flow
            .control
            .operands
            .span(call.argument_operands())?;
        let mut paths = Vec::new();
        for source in sources {
            let CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
                source.source.source
            else {
                return None;
            };
            let (position, parameter) = self
                .program
                .state_parameters(destination)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| is_mutable_structural_parameter(self.program, parameter))
                .nth(parameter_index as usize)?;
            let operand = operands.get(position)?;
            let present = if stored_loan_parts(self.program, parameter.type_reference).is_some() {
                true
            } else {
                let path = match source.source.path.split_last() {
                    Some((CheckedUnitStructuralPathSegment::Referent, path)) => path,
                    _ => source.source.path.as_slice(),
                };
                self.expression_leaf_present(
                    self.program.state_by_symbol(operand.state_symbol)?,
                    operand.statement_index,
                    operand.expression,
                    parameter.type_reference,
                    path,
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
                StoredCarrierSource::Borrow { .. } => true,
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

/// Compare a completed address with the conservative coordinates of the same
/// evaluated loan. Dynamic selectors admit a refinement, not reevaluation;
/// every static field, case, window and later array dimension stays fixed.
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

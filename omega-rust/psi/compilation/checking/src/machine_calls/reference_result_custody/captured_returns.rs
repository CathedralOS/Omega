//! Executable result custody consumes the borrow producer's normal-return rows.
//! These rows describe current backing, including replacements; a typed local
//! initializer describes neither the return epoch nor an operand capture.
//! Missing rows are not an empty result. Only the retained value/store graph
//! can establish an inactive descriptor slot. Source admission keeps its
//! independent declaration-level lifetime analysis and does not call this query.
//! Cross-invocation summaries erase executable selectors, not their dimensions:
//! a runtime window remains a region of the input collection, whereas an index
//! selects one element. Region provenance never establishes an exact descriptor
//! extent or equality between independently captured windows.
//! A placed formation retains that original backing region separately from
//! its accessor presentation. It is not a codec projection: the source-checked
//! formation and the returned live association own its placement authority.
//! Forwarded descriptors still retain their exact existing interpretation.

mod descriptor_sources;
mod incoming_origins;
mod named_returns;
mod presence;

use super::{
    StoredCarrierSource, is_mutable_structural_parameter, loan_owner_path, loan_place_segments,
    record_carrier_sources, reference_access_can_supply, stored_carrier_sources, stored_loan_parts,
};
use checked_trees::{
    CheckFacts, CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
    CheckedReferenceResultSourcePlan, CheckedReturnedReferencePlan, CheckedStructuralValueKind,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::ReferenceOriginExtent;
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
    let machine = program.machine_by_symbol(flow.machine_symbol)?;
    let target = crate::transition_call_target(
        program,
        machine,
        state,
        call.statement_index,
        call.call_ordinal,
    )?;
    if captures.transition_call(state, call.statement_index, target)? != call {
        return None;
    }
    captures.normal_returns(
        state,
        &mut vec![state.symbol],
        Some((call.statement_index, target)),
    )
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

/// A formed interpretation can present a different type without renaming its
/// backing. The origin transports the original storage; the returned carrier
/// transports the independently checked presentation and its live association.
struct FrameOrigin {
    path: Vec<CheckedReferenceOriginProjection>,
    backing: TypeReferenceHandle,
    presentation: TypeReferenceHandle,
    extent: ReferenceOriginExtent,
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
        selected_site: Option<(usize, typed_trees::statement::TransitionTargetHandle)>,
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
        let leaves = self.reference_leaves(state)?;
        let statements = typed_trees::statement::reachable_state_statements(
            self.program
                .statement_table
                .statements(state.statement_nodes),
        );
        let mut output = Vec::new();
        let mut completed = false;
        for (statement_index, statement) in statements.iter().enumerate() {
            if selected_site.is_some_and(|(selected, _)| selected != statement_index) {
                continue;
            }
            let mut sites = Vec::new();
            match statement {
                StatementNode::Expression(expression)
                    if statement_index + 1 == statements.len() =>
                {
                    sites.push((Default::default(), Some(*expression), None));
                }
                StatementNode::Transition(transition) => {
                    if matches!(transition.exit, TransitionExit::Crash(_)) {
                        continue;
                    }
                    for target in transition.targets() {
                        if selected_site.is_some_and(|(_, selected)| selected != target) {
                            continue;
                        }
                        match self.program.statement_table.transition_target(target) {
                            TransitionTargetNode::Value(expression) => {
                                sites.push((target, Some(*expression), None));
                            }
                            TransitionTargetNode::Named { .. } => {
                                let call = self.transition_call(state, statement_index, target)?;
                                if let Some(destination) =
                                    crate::call_target_state(self.program, call.target_symbol)
                                    && self.flow(destination.symbol)?.machine_symbol
                                        == self.flow(state.symbol)?.machine_symbol
                                    && !crate::normal_completion_possible(
                                        self.program,
                                        destination,
                                    )?
                                {
                                    continue;
                                }
                                // The source state captured this edge's actual
                                // descriptors. Successor formal ordinals are
                                // another namespace, and recursively copying
                                // them both misroutes reordered inputs and
                                // mistakes a returning loop for missing data.
                                sites.push((target, None, Some(call)));
                            }
                            TransitionTargetNode::Terminal => return None,
                        }
                    }
                }
                _ => continue,
            }
            for (transition_target, expression, call) in sites {
                completed = true;
                if let Some(call) = call {
                    for source in
                        self.named_return_sources(state, transition_target, call, loans, active)?
                    {
                        if !output.contains(&source) {
                            output.push(source);
                            if output.len() > super::MAX_REFERENCE_ORIGIN_LEAVES {
                                return None;
                            }
                        }
                    }
                    continue;
                }
                let expression = expression?;
                for leaf in &leaves {
                    let owner_path = loan_owner_path(self.program, state.return_type, &leaf.path)?;
                    let mut captured = false;
                    for loan in loans.iter().filter(|loan| {
                        loan.statement_index == statement_index
                            && loan.transition_target == transition_target
                    }) {
                        if self.facts.borrow.owner_segments.span(loan.owner_path)? != owner_path {
                            continue;
                        }
                        if leaf.source.access.borrow_access_kind()? != loan.kind {
                            return None;
                        }
                        captured = true;
                        for origin in
                            self.return_origins(state, loan, leaf, Some(expression), loans)?
                        {
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
                        let present = self.expression_leaf_present(
                            state,
                            statement_index,
                            expression,
                            state.return_type,
                            &leaf.path,
                            None,
                            active,
                        )?;
                        // Missing formation never establishes an inactive
                        // payload. Every normal alternative owes its own row
                        // or positive completed-value evidence of absence.
                        if present {
                            return None;
                        }
                    }
                }
            }
        }
        completed.then_some(output)
    }

    fn transition_call(
        &self,
        state: &typed_trees::state::State,
        statement_index: usize,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Option<&checked_trees::FlowCallFact> {
        let flow = self.flow(state.symbol)?;
        let machine = self.program.machine_by_symbol(flow.machine_symbol)?;
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
                    && crate::transition_call_target(
                        self.program,
                        machine,
                        state,
                        statement_index,
                        call.call_ordinal,
                    ) == Some(target)
            });
        let call = calls.next()?;
        calls.next().is_none().then_some(call)
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
        if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency {
            // Lifetime custody does not authenticate a projected input address.
            return None;
        }
        let values = &self.facts.values.structural_values;
        if let Some(expression) = expression {
            let root = values
                .root_for_expression(
                    state.symbol,
                    u32::try_from(loan.statement_index).ok()?,
                    expression,
                )
                .map(|root| root.root);
            if let Some(root) = root {
                let sources = stored_carrier_sources(self.program, self.facts, root, &leaf.path)?;
                if sources.iter().any(|source| match source {
                    StoredCarrierSource::Image { value } => {
                        let image = values.nodes.get(*value);
                        image.state == state.symbol
                            && image.statement_index as usize == loan.statement_index
                            && image.expression == loan.formation_expression
                    }
                    _ => false,
                }) {
                    // This exact returned loan formed an immutable image
                    // descriptor. It contributes no caller-relative origin;
                    // all non-image loans still traverse the normal path.
                    return Some(Vec::new());
                }
            }
        }
        let segments = self.facts.borrow.access_segments.span(loan.segments)?;
        if let Some(source) = self.return_origin(state, loan, leaf, segments, loans) {
            return Some(source);
        }
        // Incoming stored descriptors have a separate opaque origin identity.
        // A root address alone cannot replace their exact owner-path relation.
        if loan.incoming_reference.is_valid() {
            return None;
        }
        let operand = self.completed_return_operand(
            state,
            loan.statement_index,
            loan.transition_target,
            expression?,
            &leaf.path,
        )?;
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
                    && candidate.transition_target == loan.transition_target
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
        transition_target: typed_trees::statement::TransitionTargetHandle,
        expression: ExpressionHandle,
        path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<&checked_trees::FlowOperandFact> {
        let flow = self.flow(state.symbol)?;
        let control = &self.facts.flow.control;
        let mut exits = control.exits.span(flow.exits)?.iter().filter(|exit| {
            exit.machine_symbol == flow.machine_symbol
                && exit.state_symbol == state.symbol
                && exit.statement_index == statement_index
                && exit.transition_target == transition_target
        });
        let exit = exits.next()?;
        if exits.next().is_some() || !control.operands.is_valid(exit.returned_value) {
            return None;
        }
        let operand = control.operands.get(exit.returned_value);
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
        self.completed_operand_leaf(operand, path)
    }

    fn return_origin(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        segments: &[facts::PlaceSegment],
        loans: &[checked_trees::BorrowReturnedLoanFact],
    ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
        self.return_origin_with_frame(state, loan, leaf, segments, loans, None)
    }

    fn return_origin_with_frame(
        &self,
        state: &typed_trees::state::State,
        loan: &checked_trees::BorrowReturnedLoanFact,
        leaf: &CheckedReferenceResultSourcePlan,
        segments: &[facts::PlaceSegment],
        loans: &[checked_trees::BorrowReturnedLoanFact],
        captured: Option<&[crate::ReferenceResultLeafOrigin]>,
    ) -> Option<Vec<CheckedReferenceOriginSourcePlan>> {
        if loan.incoming_reference.is_valid()
            && self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
        {
            let incoming = self
                .facts
                .borrow
                .incoming_references
                .get(loan.incoming_reference);
            if let checked_trees::IncomingReferenceOrigin::Invocation { expression, target } =
                incoming.origin
            {
                if incoming.state_symbol != state.symbol
                    || loan.root != facts::PlaceRoot::Expression(expression)
                    || !matches!(self.program.expression_table.expression(expression), typed_trees::expression::ExpressionNode::Call(call) if call.target_symbol == target)
                {
                    return None;
                }
                let shell = crate::reference_shell(self.program, incoming.carrier_type)?;
                if !reference_access_can_supply(shell.access.into(), leaf.source.access) {
                    return None;
                }
                let (suffix, endpoint, extent) = self.origin_path(shell.referee, segments)?;
                let identity = self.program.normalized_type_identity(endpoint);
                if identity != leaf.source.type_identity {
                    return None;
                }
                return Some(vec![CheckedReferenceOriginSourcePlan {
                    root: checked_trees::CheckedReferenceOriginRoot::Invocation {
                        carrier_type: self.program.normalized_type_identity(incoming.carrier_type),
                    },
                    path: std::iter::once(CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::Referent,
                    ))
                    .chain(suffix)
                    .collect(),
                    extent,
                    type_identity: identity,
                    access: leaf.source.access,
                }]);
            }
        }
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
        if loan.incoming_reference.is_valid() {
            let selected_incoming = self.incoming_leaf(state, loan)?;
            let carrier = selected_incoming.carrier;
            let incoming_leaf = selected_incoming.reference;
            if !reference_access_can_supply(incoming_leaf.access, leaf.source.access) {
                return None;
            }
            if stored_loan_parts(self.program, parameter.type_reference).is_some() {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
            }
            let incoming_path = selected_incoming.segments;
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
        let inferred;
        let origins = if let Some(captured) = captured {
            Some(captured)
        } else {
            inferred = frame.reference_result_leaf_origins(state.symbol);
            inferred.as_deref()
        };
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
        // A captured loan that already reaches the returned type or its typed
        // collection region needs no body-summary refinement. Keep a window's
        // actual collection identity and WithinRegion extent, not equality to
        // the returned descriptor or another opaque window. Its captured root
        // and prefix remain authoritative even if a frame summary is unavailable.
        // The body relation below refines wider lifetime regions or recovers a
        // checked presentation of another endpoint type.
        // A retained interpretation still needs its codec, even if its target
        // type equals the original endpoint. Type equality cannot erase a view.
        if captured.is_none()
            && !has_presentation
            && let Some((suffix, endpoint, extent)) = self.origin_path(selected, &segments)
        {
            let identity = self.program.normalized_type_identity(endpoint);
            if let Some(extent) = self.result_origin_extent(
                endpoint,
                state.return_type,
                &result_path,
                leaf.source.type_identity,
                extent,
            ) {
                path.extend(suffix);
                return Some(vec![CheckedReferenceOriginSourcePlan {
                    root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                        index: parameter_index,
                    },
                    path,
                    extent,
                    type_identity: identity,
                    access: leaf.source.access,
                }]);
            }
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
                            && self.origin_matches_returned_loan(state, candidate, &origin.source)
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
                        && self.origin_matches_returned_loan(state, loan, &origin.source)
                })
                .collect::<Vec<_>>();
            if !matching.is_empty() {
                let mut output = Vec::new();
                for origin in matching {
                    let captured =
                        self.frame_origin_path(selected, &incoming_segments, &origin.source)?;
                    let source_identity = self.program.normalized_type_identity(captured.backing);
                    let extent = self.result_origin_extent(
                        captured.presentation,
                        state.return_type,
                        &result_path,
                        leaf.source.type_identity,
                        captured.extent,
                    )?;
                    let mut complete = path.clone();
                    complete.extend(captured.path);
                    let source = CheckedReferenceOriginSourcePlan {
                        root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                            index: parameter_index,
                        },
                        path: complete,
                        extent,
                        type_identity: source_identity,
                        access: leaf.source.access,
                    };
                    if !output.contains(&source) {
                        output.push(source);
                    }
                }
                return (!output.is_empty()).then_some(output);
            }
        }
        if captured.is_some() {
            return None;
        }
        // Loan formation can capture an input region without a public typed
        // subprojection, notably for a bodyless call's returned descriptor.
        // Keep the captured region as a region: inventing an element or field
        // here would pretend the public signature selected a private address.
        // The original loan/root/access joins above remain mandatory, and
        // completion must check the actual returned footprint's containment.
        let (suffix, mut endpoint, extent) = self.origin_path(selected, &segments)?;
        // A captured window still needs a compatible slice presentation when
        // no body relation is available. A wider opaque lifetime region, in
        // contrast, has no window conversion to establish here.
        if extent == ReferenceOriginExtent::WithinRegion
            && self.program.normalized_type_identity(endpoint) != leaf.source.type_identity
            && !self.collection_region_presents_result(endpoint, state.return_type, &result_path)
        {
            return None;
        }
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
            root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                index: parameter_index,
            },
            path,
            extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
            type_identity: self.program.normalized_type_identity(endpoint),
            access: leaf.source.access,
        }])
    }

    /// An exact collection projection may supply a slice without an authored
    /// range. Its source remains the array/collection, not the slice type. The
    /// checked formation owns the concrete descriptor; completion authenticates
    /// containment in this region instead of asserting equal referent families.
    fn result_origin_extent(
        &self,
        source: TypeReferenceHandle,
        result: TypeReferenceHandle,
        result_path: &[facts::PlaceSegment],
        result_identity: typed_trees::type_identity::TypeIdentityId,
        extent: ReferenceOriginExtent,
    ) -> Option<ReferenceOriginExtent> {
        let source_identity = self.program.normalized_type_identity(source);
        if !source_identity.is_valid() {
            return None;
        }
        if source_identity == result_identity {
            return Some(extent);
        }
        // Different endpoint types require a checked slice presentation with
        // the same complete element identity; unrelated regions do not fit.
        self.collection_region_presents_result(source, result, result_path)
            .then_some(ReferenceOriginExtent::WithinRegion)
    }

    /// A window retains its collection's storage identity while presenting a
    /// slice descriptor. This is ordinary element storage, not a representation
    /// conversion: element qualifications must agree. Loan joins authenticate
    /// access and ancestry; checked formation and completion retain the actual
    /// window and its bounds. A codec-backed re-view still takes the separately
    /// authenticated recast route in `frame_origin_path`.
    fn collection_region_presents_result(
        &self,
        source: TypeReferenceHandle,
        result: TypeReferenceHandle,
        result_path: &[facts::PlaceSegment],
    ) -> bool {
        let Some(result) =
            crate::project_type_reference_from_segments(self.program, result, result_path)
                .and_then(|result| crate::slice_view_shell(self.program, result, &[]))
        else {
            return false;
        };
        let source =
            crate::reference_shell(self.program, source).map_or(source, |shell| shell.referee);
        let source = crate::peel_declared_constraints(self.program, source);
        let (TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type }) =
            self.program.type_reference_table.type_reference(source)
        else {
            return false;
        };
        let source_element = self.program.normalized_type_identity(*element_type);
        let result_element = self.program.normalized_type_identity(result.element);
        source_element.is_valid() && result_element.is_valid() && source_element == result_element
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
    ) -> Option<FrameOrigin> {
        if origin.recasts.is_empty() {
            if origin.representation.is_some() {
                return None;
            }
            let (path, backing, extent) =
                self.origin_path(selected, origin.segments.strip_prefix(incoming)?)?;
            return Some(FrameOrigin {
                path,
                backing,
                presentation: backing,
                extent,
            });
        }
        let mut path = Vec::new();
        let mut extent = ReferenceOriginExtent::Projected;
        let mut placed_backing = TypeReferenceHandle::invalid();
        for (ordinal, recast) in origin.recasts.iter().enumerate() {
            if recast.root != origin.root || placed_backing.is_valid() {
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
            let (projection, endpoint, selection_extent) = self.origin_path(selected, selectors)?;
            if selection_extent == ReferenceOriginExtent::WithinRegion {
                extent = ReferenceOriginExtent::WithinRegion;
            }
            path.extend(projection);
            selected = endpoint;
            if let Some(shell) = crate::reference_shell(self.program, selected) {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
                selected = shell.referee;
            }
            if let crate::ReferenceViewFormation::ExtentView(_) = recast.formation {
                let (storage, view) = recast.formation.extent_view_types(self.program)?;
                let storage = crate::reference_shell(self.program, storage)?.referee;
                if self.program.normalized_type_identity(
                    self.program
                        .type_reference_table
                        .peeled_type_reference(selected),
                ) != self.program.normalized_type_identity(
                    self.program
                        .type_reference_table
                        .peeled_type_reference(storage),
                ) || self.program.normalized_type_identity(view)
                    != self.program.normalized_type_identity(recast.target_type)
                {
                    return None;
                }
                // A sealed View has no authored reference expression. Its
                // exact result relation keeps the descriptor's loan region
                // separate from the accessor presentation, just as an authored
                // placed formation does. Completion still checks the actual
                // returned association; this is no codec or Extent projection.
                placed_backing = selected;
                extent = ReferenceOriginExtent::WithinRegion;
                selected = recast.target_type;
                continue;
            }
            let mut formations =
                self.facts
                    .values
                    .structural_values
                    .nodes
                    .iter()
                    .filter_map(|(_, value)| {
                        if value.expression != recast.formation.expression() {
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
            let authored =
                crate::reference_formation_source(self.program, recast.formation.expression())?;
            let expected_storage = if authored.interpretation.is_placed() {
                let view = self.program.closed_data_definition(recast.target_type)?;
                let mut plans = self
                    .program
                    .placed_view_plans
                    .iter()
                    .filter(|plan| plan.data_symbol == view.symbol);
                let (Some(plan), None) = (plans.next(), plans.next()) else {
                    return None;
                };
                authored
                    .interpretation
                    .placed_storage(self.program, selected, plan)?
            } else {
                crate::reference_view_storage(self.program, selected, recast.target_type)?
            };
            if formations.next().is_some()
                || self.program.normalized_type_identity(backing_type)
                    != self.program.normalized_type_identity(selected)
                || storage != expected_storage
                || self.program.normalized_type_identity(
                    crate::reference_shell(self.program, formation.type_reference)?.referee,
                ) != self.program.normalized_type_identity(recast.target_type)
            {
                return None;
            }
            // The arena occurrence remains in its original body after a call
            // substitution. Its source identity is not the rebased caller root.
            self.program.state_by_symbol(formation.state)?;
            if authored.interpretation.is_placed() {
                // Placed accessors are not an ordinary structural re-view of
                // the backing declaration. Keep the exact original region;
                // formation owns the placement and completion retains that
                // association. A codec path would invent ordinary field access.
                placed_backing = selected;
                extent = ReferenceOriginExtent::WithinRegion;
            }
            if matches!(storage, checked_trees::CheckedReferenceStorage::View { .. }) {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::RepresentationView {
                        // The path names the registered physical declaration.
                        // Qualification stays on the checked result contract;
                        // it is not another storage shape in the Terminal roster.
                        type_identity: crate::structural_type_identity(
                            self.program,
                            recast.target_type,
                            &[],
                            &[],
                        ),
                        storage,
                    },
                ));
            }
            selected = recast.target_type;
        }
        let (presentation, selectors) = origin.representation.as_ref()?;
        if self.program.normalized_type_identity(*presentation)
            != self.program.normalized_type_identity(selected)
        {
            return None;
        }
        if placed_backing.is_valid() {
            // Accessor projections are capabilities with their own event
            // contract, not typed subregions of the original schema.
            if !selectors.is_empty() {
                return None;
            }
            return Some(FrameOrigin {
                path,
                backing: placed_backing,
                presentation: selected,
                extent,
            });
        }
        let (suffix, endpoint, selection_extent) = self.origin_path(selected, selectors)?;
        if selection_extent == ReferenceOriginExtent::WithinRegion {
            extent = ReferenceOriginExtent::WithinRegion;
        }
        path.extend(suffix);
        Some(FrameOrigin {
            path,
            backing: endpoint,
            presentation: endpoint,
            extent,
        })
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

    fn reference_leaves(
        &self,
        state: &typed_trees::state::State,
    ) -> Option<Vec<CheckedReferenceResultSourcePlan>> {
        record_carrier_sources(
            self.program,
            state.return_type,
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: state.symbol,
            },
        )
    }

    /// Runtime element selections describe one typed element; erased windows
    /// describe a region of the collection. Neither enumerates addresses.
    /// Exact formation and bounds remain in the invocation's captured loan.
    fn origin_path(
        &self,
        mut selected: TypeReferenceHandle,
        mut segments: &[facts::PlaceSegment],
    ) -> Option<(
        Vec<CheckedReferenceOriginProjection>,
        TypeReferenceHandle,
        ReferenceOriginExtent,
    )> {
        let mut output = Vec::new();
        // An erased Index still denotes one element. Erased range expressions
        // have their own OpaqueRange kind, so an invalid handle cannot collapse
        // a window into its element type.
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
                        | facts::PlaceSegment::OpaqueRange
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
                return Some((output, selected, ReferenceOriginExtent::Projected));
            };
            selected = crate::peel_declared_constraints(self.program, selected);
            // A window that ends the path narrows a collection without selecting an
            // element. Its origin is the collection it narrows, presented as
            // that collection is: a view formal presents its descriptor, as
            // the unprojected region below does. The descriptor retains the
            // captured window and bound. A fixed array keeps its own storage
            // type here; its returned slice is a separate descriptor, never an
            // assertion that the array and slice types are interchangeable.
            let trailing_window = position + 1 == segments.len()
                && match segments[position] {
                    facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                        true
                    }
                    facts::PlaceSegment::Index { expression } => !is_element_selection(expression),
                    _ => false,
                };
            if trailing_window {
                let collection = crate::reference_shell(self.program, selected)
                    .map_or(selected, |shell| {
                        crate::peel_declared_constraints(self.program, shell.referee)
                    });
                return matches!(
                    self.program.type_reference_table.type_reference(collection),
                    TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
                )
                .then_some((output, selected, ReferenceOriginExtent::WithinRegion));
            }
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
                facts::PlaceSegment::OpaqueRange => {
                    match segments.get(position.checked_add(1)?)? {
                        facts::PlaceSegment::Index { expression }
                            if is_element_selection(*expression) => {}
                        facts::PlaceSegment::FixedIndex { .. } => {}
                        _ => return None,
                    }
                    (CheckedReferenceOriginProjection::Element, 2)
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
            // Both summaries retain a region in this same collection
            // dimension. This is provenance coverage, never equality of
            // windows: publication keeps WithinRegion, and formation retains
            // the descriptor's actual offset and extent. Static parents and
            // pointer/field boundaries still have to match exactly.
            || match (candidate.split_last(), loan.split_last()) {
                (Some((facts::PlaceSegment::OpaqueRange, candidate)),
                 Some((facts::PlaceSegment::OpaqueRange, loan))) => captured_address_refines(loan, candidate),
                _ => false,
            }
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
            // Equal summary kinds do not identify equal evaluated windows.
            ([expected, suffix @ ..], actual)
                if *expected == actual && actual != PlaceSegment::OpaqueRange =>
            {
                suffix
            }
            _ => return false,
        };
    }
    remaining.is_empty()
}

pub(super) fn paths_overlap(left: &[facts::PlaceSegment], right: &[facts::PlaceSegment]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{captured_address_refines, paths_overlap};
    use facts::PlaceSegment;

    #[test]
    fn opaque_windows_overlap_without_exact_address_identity() {
        let unknown = [PlaceSegment::OpaqueRange];
        let fixed = [PlaceSegment::FixedRange { start: 1, end: 4 }];
        assert!(paths_overlap(&unknown, &unknown));
        assert!(paths_overlap(&unknown, &fixed));
        assert!(!captured_address_refines(&unknown, &unknown));
        assert!(!captured_address_refines(&unknown, &fixed));
        assert!(!captured_address_refines(&fixed, &unknown));
        assert!(captured_address_refines(&fixed, &fixed));
    }
}

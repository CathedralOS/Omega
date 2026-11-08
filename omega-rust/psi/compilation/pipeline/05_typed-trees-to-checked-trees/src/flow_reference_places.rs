/*
    Rebase a live reference before tracing the value currently in its referent.
*/

use super::operand_contexts::binding_origin_is_stable;
use super::operand_contexts::entry_origin_is_stable;
use crate::published::places::place_at;
use validation::CanonicalPlace;

use super::canonical_place_segments_may_overlap;
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::PlaceRoot;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{machine::Machine, statement::StatementNode};

mod result_candidates {
    /*
        Reference-result geometry adapts the shared chronological result relation.

        Completed Flow operands can select stored descriptor leaves from an owned
        input. The older raw storage query has only a borrowed input address and
        declines that descriptor load. Both consumers use the same complete body
        relation; neither replays a declaration initializer or creates loan authority.
        Callee-local selectors never become caller coordinates. Closed-array
        geometry retains every possible slot when an immutable actual cannot narrow
        the selection; recast presentations still require their own correspondence.

        An unchanged place-rebase is not a referent. A reference binding contributes
        storage only when its initializer or a distinct rebase names that storage.
        A dynamic index denotes every fixed slot, and every slot must resolve; one
        unresolved alternative fails the query.
    */
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use validation::CanonicalPlace;

    use facts::{PlaceRoot, PlaceSegment};
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::statement::StatementNode;

    /*
        The candidate storage places a reference-typed local names at
        `statement_index`, when it was bound from a checked reference result and
        has not been rebound since. `None` keeps the conservative treatment.
    */
    pub(crate) fn reference_result_candidates_before_statement(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        local_symbol: SymbolHandle,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        let state = program.state_by_symbol(state_symbol)?;
        let statements = program.statement_table.statements(state.statement_nodes);
        let prefix = statements.get(..statement_index)?;
        let (binding_index, local) = program
            .state_local_declaration(state, local_symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)?;
        if !crate::checks::contracts::is_readable_mutable_reference(program, local.type_reference) {
            return None;
        }
        if prefix[binding_index + 1..].iter().any(|statement| {
            matches!(statement, StatementNode::Assignment(assignment)
                if place_at(program, state_symbol, statement_index, assignment.target)
                .is_some_and(|place| place.root == PlaceRoot::Symbol(local_symbol) && place.segments.is_empty()))
        }) {
            return None;
        }
        let call = program.expression_table.call_node(local.initial_value)?;
        call_result_candidates(
            program,
            state_symbol,
            binding_index,
            local.initial_value,
            call,
            call_frames,
            false,
        )
    }

    fn call_result_candidates(
        program: &dyn ::validation::CheckingInput,
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_site: ExpressionHandle,
        call: &TableCallExpression,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        include_shared: bool,
    ) -> Option<Vec<CanonicalPlace>> {
        let mut candidates = Vec::new();
        let arguments = program.expression_table.expression_handles(call.arguments);
        for source in call_result_leaf_sources(program, call, call_frames, &[])? {
            if !source.result_path.is_empty() || source.collection_coarse {
                return None;
            }
            match source.backing {
                ReferenceResultBacking::CallerArgument {
                    argument_index,
                    parameter_type,
                } => {
                    if !parameter_grants_readable_loan(program, parameter_type, include_shared) {
                        return None;
                    }
                    let actual = *arguments.get(argument_index)?;
                    for mut storage in storage_places(
                        program,
                        caller_state_symbol,
                        statement_index,
                        actual,
                        call_frames,
                        include_shared,
                    )? {
                        storage.segments.extend_from_slice(&source.segments);
                        crate::shared::collections::push_unique(&mut candidates, storage);
                    }
                }
                ReferenceResultBacking::InvocationResult(_) => {
                    crate::shared::collections::push_unique(
                        &mut candidates,
                        CanonicalPlace {
                            root: PlaceRoot::Expression(call_site),
                            segments: source.segments.clone(),
                        },
                    );
                }
            }
        }
        (!candidates.is_empty()).then_some(candidates)
    }

    /*
        Backing storage of mutable or shared reference expressions. Proof reads
        the referent; this mapping alone does not establish loan authority.
    */
    pub(crate) fn reference_expression_storage_places_including_shared(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        storage_places(program, state, statement_index, expression, frames, true)
    }

    fn storage_places(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        include_shared: bool,
    ) -> Option<Vec<CanonicalPlace>> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => {
                return storage_places(
                    program,
                    state,
                    statement_index,
                    borrow.target,
                    frames,
                    include_shared,
                );
            }
            ExpressionNode::Call(call) => {
                return call_result_candidates(
                    program,
                    state,
                    statement_index,
                    expression,
                    call,
                    frames,
                    include_shared,
                );
            }
            ExpressionNode::Match(dispatch) => {
                let mut candidates = Vec::new();
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    for storage in storage_places(
                        program,
                        state,
                        statement_index,
                        arm.value,
                        frames,
                        include_shared,
                    )? {
                        crate::shared::collections::push_unique(&mut candidates, storage);
                    }
                }
                return (!candidates.is_empty()).then_some(candidates);
            }
            _ => {}
        }
        let place = place_at(program, state, statement_index, expression)?;
        storage_places_for_canonical_place(
            program,
            state,
            statement_index,
            &place,
            frames,
            include_shared,
            16,
        )
    }

    /*
        Storage a transition or call argument denotes. Reference bindings resolve
        to their referents; an unresolved reference is not itself storage.
    */
    pub(crate) fn argument_storage_places(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Vec<CanonicalPlace>> {
        storage_places_for_canonical_place(program, state, statement_index, place, frames, true, 16)
    }

    fn storage_places_for_canonical_place(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        frames: Option<&validation::CallFrameResolver<'_>>,
        include_shared: bool,
        depth: usize,
    ) -> Option<Vec<CanonicalPlace>> {
        if depth == 0 {
            return None;
        }
        if place
            .segments
            .iter()
            .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
        {
            let root = CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            };
            let root_type =
                validation::canonical_place_type_reference(program, state, statement_index, &root)?;
            let paths = validation::completed_storage::finite_projection_paths(
                program,
                root_type,
                &place.segments,
            )?;
            let mut candidates = Vec::new();
            for segments in paths {
                let fixed = CanonicalPlace {
                    root: place.root,
                    segments,
                };
                for storage in storage_places_for_canonical_place(
                    program,
                    state,
                    statement_index,
                    &fixed,
                    frames,
                    include_shared,
                    depth - 1,
                )? {
                    crate::shared::collections::push_unique(&mut candidates, storage);
                }
            }
            return (!candidates.is_empty()).then_some(candidates);
        }

        if let PlaceRoot::Expression(expression) = place.root {
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) => {
                    let symbol =
                        validation::effective_member_symbol(program, member.receiver, member);
                    let mut segments = Vec::new();
                    validation::push_field_place_segments(program, &mut segments, symbol);
                    segments.extend_from_slice(&place.segments);
                    if let Some(mut receiver) =
                        place_at(program, state, statement_index, member.receiver)
                    {
                        receiver.segments.extend(segments);
                        if receiver.root != place.root || receiver.segments != place.segments {
                            return storage_places_for_canonical_place(
                                program,
                                state,
                                statement_index,
                                &receiver,
                                frames,
                                include_shared,
                                depth - 1,
                            );
                        }
                    }
                }
                ExpressionNode::Indexed(indexed) => {
                    let mut segments =
                        vec![validation::index_place_segment(program, indexed.index)];
                    segments.extend_from_slice(&place.segments);
                    if let Some(mut collection) =
                        place_at(program, state, statement_index, indexed.collection)
                    {
                        collection.segments.extend(segments);
                        if collection.root != place.root || collection.segments != place.segments {
                            return storage_places_for_canonical_place(
                                program,
                                state,
                                statement_index,
                                &collection,
                                frames,
                                include_shared,
                                depth - 1,
                            );
                        }
                    }
                }
                ExpressionNode::Borrow(borrow) => {
                    if let Some(mut target) =
                        place_at(program, state, statement_index, borrow.target)
                    {
                        target.segments.extend_from_slice(&place.segments);
                        if target.root != place.root || target.segments != place.segments {
                            return storage_places_for_canonical_place(
                                program,
                                state,
                                statement_index,
                                &target,
                                frames,
                                include_shared,
                                depth - 1,
                            );
                        }
                    }
                }
                ExpressionNode::Match(dispatch) => {
                    let mut candidates = Vec::new();
                    for arm in program.expression_table.match_arms(dispatch.arms) {
                        let mut arm_place = place_at(program, state, statement_index, arm.value)
                            .unwrap_or(CanonicalPlace {
                                root: PlaceRoot::Expression(arm.value),
                                segments: Vec::new(),
                            });
                        arm_place.segments.extend_from_slice(&place.segments);
                        for storage in storage_places_for_canonical_place(
                            program,
                            state,
                            statement_index,
                            &arm_place,
                            frames,
                            include_shared,
                            depth - 1,
                        )? {
                            crate::shared::collections::push_unique(&mut candidates, storage);
                        }
                    }
                    return (!candidates.is_empty()).then_some(candidates);
                }
                _ => {}
            }
        }

        let state_decl = program.state_by_symbol(state);
        let root_symbol = place.root.as_symbol();
        if let Some((binding_index, local)) = root_symbol
            .zip(state_decl)
            .and_then(|(root, decl)| program.state_local_declaration(decl, root))
            .filter(|&(ordinal, _)| ordinal < statement_index)
        {
            if parameter_grants_readable_loan(program, local.type_reference, include_shared) {
                let statements = program
                    .statement_table
                    .statements(state_decl?.statement_nodes);
                let prefix = statements.get(..statement_index)?;
                let rebound = prefix[binding_index + 1..].iter().any(|statement| {
                    matches!(statement, StatementNode::Assignment(assignment)
                    if place_at(program, state, statement_index, assignment.target)
                        .is_some_and(|target| {
                            target.root == place.root && target.segments.is_empty()
                        }))
                });
                if !rebound
                    && let Some(initial) = storage_places(
                        program,
                        state,
                        binding_index,
                        local.initial_value,
                        frames,
                        include_shared,
                    )
                {
                    let mut candidates = Vec::new();
                    for mut candidate in initial {
                        candidate.segments.extend_from_slice(&place.segments);
                        if candidate.root == place.root && candidate.segments == place.segments {
                            return None;
                        }
                        for storage in storage_places_for_canonical_place(
                            program,
                            state,
                            binding_index,
                            &candidate,
                            frames,
                            include_shared,
                            depth - 1,
                        )? {
                            crate::shared::collections::push_unique(&mut candidates, storage);
                        }
                    }
                    if !candidates.is_empty() {
                        return Some(candidates);
                    }
                }
                if let Some(storage) = crate::flow::rebase_exact_local_place(
                    program,
                    state,
                    statement_index,
                    place.clone(),
                    frames,
                ) && storage != *place
                    && matches!(storage.root, PlaceRoot::Symbol(_))
                {
                    return storage_places_for_canonical_place(
                        program,
                        state,
                        statement_index,
                        &storage,
                        frames,
                        include_shared,
                        depth - 1,
                    );
                }
                return None;
            }
            if !place.segments.is_empty() {
                if let Some(projections) = crate::flow::literal_expression_projections(
                    program,
                    local.initial_value,
                    local.type_reference,
                    &place.segments,
                    false,
                ) {
                    let mut candidates = Vec::new();
                    for projection in projections {
                        for mut storage in storage_places(
                            program,
                            state,
                            binding_index,
                            projection.expression,
                            frames,
                            include_shared,
                        )? {
                            storage.segments.extend_from_slice(&projection.remaining);
                            if storage.segments.is_empty()
                                && !place_is_readable_reference(
                                    program,
                                    state,
                                    binding_index,
                                    &storage,
                                    include_shared,
                                )
                            {
                                crate::shared::collections::push_unique(&mut candidates, storage);
                                continue;
                            }
                            for resolved in storage_places_for_canonical_place(
                                program,
                                state,
                                binding_index,
                                &storage,
                                frames,
                                include_shared,
                                depth - 1,
                            )? {
                                crate::shared::collections::push_unique(&mut candidates, resolved);
                            }
                        }
                    }
                    if !candidates.is_empty() {
                        return Some(candidates);
                    }
                }
                if let Some(mut source) =
                    place_at(program, state, binding_index, local.initial_value)
                {
                    source.segments.extend_from_slice(&place.segments);
                    if source.root != place.root || source.segments != place.segments {
                        return storage_places_for_canonical_place(
                            program,
                            state,
                            binding_index,
                            &source,
                            frames,
                            include_shared,
                            depth - 1,
                        );
                    }
                }
            }
        }

        if let Some(parameter) = root_symbol
            .zip(state_decl)
            .and_then(|(root, decl)| program.state_parameter_by_symbol(decl, root))
            && parameter_grants_readable_loan(program, parameter.type_reference, include_shared)
        {
            if let Some(storage) = crate::flow::rebase_exact_local_place(
                program,
                state,
                statement_index,
                place.clone(),
                frames,
            ) && storage != *place
                && matches!(storage.root, PlaceRoot::Symbol(_))
            {
                return storage_places_for_canonical_place(
                    program,
                    state,
                    statement_index,
                    &storage,
                    frames,
                    include_shared,
                    depth - 1,
                );
            }
            return Some(vec![place.clone()]);
        }

        if place.root.as_symbol().is_none()
            || place_is_readable_reference(program, state, statement_index, place, include_shared)
        {
            return None;
        }
        Some(vec![place.clone()])
    }

    fn place_is_readable_reference(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        include_shared: bool,
    ) -> bool {
        validation::canonical_place_type_reference(program, state, statement_index, place)
            .is_some_and(|reference| {
                parameter_grants_readable_loan(program, reference, include_shared)
            })
    }

    /*
        A checked body's actual storage route, before rebasing through caller
        locals. This value-origin query is not the lifetime/authority frontier:
        declarations can retain inputs absent from the body's actual return paths.
    */
    #[derive(Clone, Copy)]
    pub(in crate::flow) enum ReferenceResultSourceKind {
        Reference,
        Value {
            type_reference: typed_trees::types::TypeReferenceHandle,
        },
    }

    /*
        What a returned leaf's completed origin names. A caller route projects
        an actual's storage through its formal's declared type; an opaque result
        referent names backing the completed invocation owns, typed by the
        declared referee. Neither kind supplies new loan authority.
    */
    #[derive(Clone, Copy)]
    pub(in crate::flow) enum ReferenceResultBacking {
        CallerArgument {
            argument_index: usize,
            parameter_type: typed_trees::types::TypeReferenceHandle,
        },
        InvocationResult(validation::InvocationResultHome),
    }

    pub(in crate::flow) struct ReferenceResultSource {
        pub(in crate::flow) kind: ReferenceResultSourceKind,
        pub(in crate::flow) result_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        pub(in crate::flow) backing: ReferenceResultBacking,
        /*
            Indexed frame spellings coarsen to the containing collection even when
            authenticated structural selectors retain an exact fixed slot. Raw
            storage adapters need this limit; completed values consume the selectors.
        */
        collection_coarse: bool,
        pub(in crate::flow) segments: Vec<PlaceSegment>,
    }

    pub(in crate::flow) fn call_result_leaf_sources(
        program: &dyn ::validation::CheckingInput,
        call: &TableCallExpression,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        indices: &[Option<usize>],
    ) -> Option<Vec<ReferenceResultSource>> {
        let sources = call_result_captured_sources(program, call, call_frames, indices)?;
        sources
            .into_iter()
            .map(|source| {
                matches!(source.kind, ReferenceResultSourceKind::Reference).then_some(source)
            })
            .collect()
    }

    /*
        Preserve whole-value transport until the completed-input consumer can
        select its actual descriptor leaves. A declaration path identifies a slot,
        never the presence or address of the value carried in that slot.
    */
    pub(in crate::flow) fn call_result_captured_sources(
        program: &dyn ::validation::CheckingInput,
        call: &TableCallExpression,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        indices: &[Option<usize>],
    ) -> Option<Vec<ReferenceResultSource>> {
        let entered_target = validation::call_target_entry_symbol(
            program,
            validation::call_target_symbol(program, call),
        );
        let (parameters, stored_attached_receiver) =
            if let Some((callee, state)) = validation::find_state(program, entered_target) {
                (
                    program.state_parameters(state),
                    Some(callee.attached_data.is_some()),
                )
            } else {
                let (_, signature) =
                    validation::selected_requirement_signature(program, entered_target)?;
                (program.state_signature_parameters(signature), None)
            };
        if stored_attached_receiver.is_some_and(|attached| call.receiver.is_valid() != attached)
            || !call.carries_only_positional_arguments()
            || !call.selects_only_nominal_route()
            || expressions(program)
                .expression_handles(call.arguments)
                .len()
                != parameters.len()
        {
            return None;
        }
        let prepared_frames;
        let frames = match call_frames {
            Some(frames) => frames,
            None => {
                prepared_frames = validation::CallFrameResolver::new(program)?;
                &prepared_frames
            }
        };
        let transports = if indices.is_empty() {
            frames.reference_result_transports(entered_target)?
        } else {
            frames.reference_result_transports_for_indices(entered_target, indices)?
        };
        let mut sources = Vec::with_capacity(transports.len());
        for transport in transports {
            let (result_path, source, kind) = match transport {
                validation::ReferenceResultTransport::Reference(origin) => (
                    origin.result_path,
                    origin.source,
                    ReferenceResultSourceKind::Reference,
                ),
                validation::ReferenceResultTransport::Value {
                    result_path,
                    source,
                    type_reference,
                } => (
                    result_path,
                    source,
                    ReferenceResultSourceKind::Value { type_reference },
                ),
            };
            if source.is_image_storage
                || !source.recasts.is_empty()
                || source.representation.is_some()
                || !source.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        PlaceSegment::Field { .. }
                            | PlaceSegment::Case { .. }
                            | PlaceSegment::FixedIndex { .. }
                            | PlaceSegment::Index { .. }
                    )
                })
            {
                return None;
            }
            let result_path = crate::borrow::structural_owner_path(
                program,
                &crate::borrow::owner_path_from_place_segments(program, &result_path),
            )?;
            if let Some(home) = source.invocation_result {
                for segments in validation::completed_storage::finite_projection_paths(
                    program,
                    home.referent,
                    &source.segments,
                )? {
                    sources.push(ReferenceResultSource {
                        kind,
                        result_path: result_path.clone(),
                        backing: ReferenceResultBacking::InvocationResult(home),
                        collection_coarse: source.collection_coarse,
                        segments,
                    });
                }
                continue;
            }
            let (argument_index, parameter) = parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == source.root)?;
            for segments in validation::completed_storage::finite_projection_paths(
                program,
                parameter.type_reference,
                &source.segments,
            )? {
                sources.push(ReferenceResultSource {
                    kind,
                    result_path: result_path.clone(),
                    backing: ReferenceResultBacking::CallerArgument {
                        argument_index,
                        parameter_type: parameter.type_reference,
                    },
                    collection_coarse: source.collection_coarse,
                    segments,
                });
            }
        }
        (!sources.is_empty()).then_some(sources)
    }

    fn parameter_grants_readable_loan(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        include_shared: bool,
    ) -> bool {
        if !include_shared {
            return crate::checks::contracts::is_readable_mutable_reference(
                program,
                type_reference,
            );
        }
        let mut reference = type_reference;
        while reference.is_valid() {
            match program.type_reference_table.type_reference(reference) {
                typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                    reference = *base_type;
                }
                typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                    return *access != language_semantics::ReferenceAccess::WriteOnly;
                }
                _ => return false,
            }
        }
        false
    }
}

pub(in crate::flow) use result_candidates::{
    ReferenceResultBacking, ReferenceResultSourceKind, call_result_captured_sources,
    call_result_leaf_sources,
};

pub(crate) use result_candidates::{
    argument_storage_places, reference_expression_storage_places_including_shared,
    reference_result_candidates_before_statement,
};

/*
    A prepared referent store preserves the reference descriptor's binding.
    The receiving type, unlike the RHS expression's spelling, already records
    whether assignment selected the descriptor or its referent after typing.
*/
pub(crate) fn assignment_preserves_reference_binding(
    program: &dyn ::validation::CheckingInput,
    assignment: &typed_trees::statement::TableAssignment,
    declared: typed_trees::types::TypeReferenceHandle,
) -> bool {
    crate::checks::type_reference_is_reference(program, declared)
        && assignment.receiving_type.is_valid()
        && program.normalized_type_identity(assignment.receiving_type)
            != program.normalized_type_identity(declared)
}

/*
    The expression whose place a reference-typed local currently refers to:
    its initializer, or the value of the latest descriptor reassignment.
    A store through the reference changes its referent, not this route.
    Returns `None` when the symbol is not a reference-typed local or the route
    cannot be replayed.
*/
pub(crate) fn reference_route_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order
        .position(statement_index)
        .or_else(|| (statement_index == statements.len()).then_some(order.indices().len()))?;
    let (ordinal, declaration) = program.state_local_declaration(state, symbol)?;
    if !order
        .position(ordinal)
        .is_some_and(|position| position < end)
    {
        return None;
    }
    if !crate::checks::type_reference_is_reference(program, declaration.type_reference) {
        return None;
    }
    order.indices()[..end]
        .iter()
        .rev()
        .filter_map(|index| statements.get(*index))
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local.initial_value),
            StatementNode::Assignment(assignment) => {
                if assignment_preserves_reference_binding(
                    program,
                    assignment,
                    declaration.type_reference,
                ) {
                    return None;
                }
                let target = place_at(program, state.symbol, statement_index, assignment.target)?;
                (target.root == PlaceRoot::Symbol(symbol) && target.segments.is_empty())
                    .then_some(assignment.value)
            }
            _ => None,
        })
}

pub(crate) fn local_reference_storage_before_statement(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let root = place.root.as_symbol()?;
    let state = program.state_by_symbol(state.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order
        .position(index)
        .or_else(|| (index == statements.len()).then_some(order.indices().len()))?;
    let mut declarations = order.indices()[..end]
        .iter()
        .filter_map(|index| statements.get(*index))
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local),
            _ => None,
        });
    let Some(local) = declarations.next() else {
        return Some(place);
    };
    if declarations.next().is_some() {
        return None;
    }
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(local.type_reference);
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. }
    ) {
        return Some(place);
    }
    let (root, mut segments) = frames.local_reference_origin_before_statement(
        machine,
        statements.get(index)?,
        local.symbol,
    )?;
    segments.extend_from_slice(&place.segments);
    place.root = PlaceRoot::Symbol(root);
    place.segments = segments;
    Some(place)
}

pub(crate) fn local_reference_storage_at_call(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    observation: super::ValueObservation,
) -> Option<CanonicalPlace> {
    let resolved = local_reference_storage_before_statement(
        program,
        frames,
        machine,
        state,
        call.statement_index,
        place.clone(),
    )?;
    preserve_call_prefix_storage(
        program,
        frames,
        machine,
        flow,
        state,
        call,
        &[place, resolved.clone()],
        observation,
    )?;
    Some(resolved)
}

/*
    Every storage place a call actual names through a reference-result local:
    the finite candidate origins (`level.rooms[0]` .. `level.rooms[15]` for
    `level.room_mut(cell)`) with the actual's own projection applied, so
    `room.event` arrives as `level.rooms[i].event`. A caller-side proof over
    such an actual holds exactly when every candidate proves it -- the callee
    receives whichever storage the loan names. `None` keeps the single-origin
    or conservative treatment, including when an earlier operand's frame could
    have written a candidate.
*/
pub(crate) fn local_reference_candidate_storages_at_call(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    borrow: &checked_trees::BorrowFacts,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let root = place.root.as_symbol()?;
    let candidates = reference_result_candidates_before_statement(
        program,
        state.state_symbol,
        call.statement_index,
        root,
        Some(frames),
    )?;
    let mut storages: Vec<CanonicalPlace> = candidates
        .into_iter()
        .map(|mut candidate| {
            candidate.segments.extend_from_slice(&place.segments);
            candidate
        })
        .collect();
    storages.push(place);
    preserve_candidate_call_prefix_storage(
        program, frames, borrow, machine, flow, state, call, &storages,
    )?;
    storages.pop();
    Some(storages)
}

/*
    The prefix bound is the call's own position in the state's recorded
    execution order. Resolve it from the row's recorded identity — the
    `(statement_index, call_ordinal)` coordinate plus target and receiver
    symbols, the same fields `build_call_entry_contexts` uses — never the
    address the row occupies in this arena slice. A replayed copy of the
    same recorded row reaches the same bound; an absent or ambiguous
    identity declines rather than borrowing a same-shaped row's position.
*/
fn call_position_in_state(
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<usize> {
    let calls = flow.control.calls.span_or_empty(state.calls);
    let mut positions = calls
        .iter()
        .enumerate()
        .filter_map(|(position, candidate)| {
            (candidate.statement_index == call.statement_index
                && candidate.call_ordinal == call.call_ordinal
                && candidate.target_symbol == call.target_symbol
                && candidate.receiver_symbol == call.receiver_symbol)
                .then_some(position)
        });
    let position = positions.next()?;
    if positions.next().is_some() {
        return None;
    }
    Some(position)
}

/*
    Earlier operands may replace either the reference binding or its referent.
    The statement-prefix origin alone says nothing about those effects.
    Calls are retained in execution order, not authored preorder ordinal.
*/
fn preserve_call_prefix_storage(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    places: &[CanonicalPlace],
    observation: super::ValueObservation,
) -> Option<()> {
    let position = call_position_in_state(flow, state, call)?;
    let calls = flow.control.calls.span_or_empty(state.calls);
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let site = validation::find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            prior.statement_index,
            prior.call_ordinal,
        )?;
        match site {
            validation::CallSite::Statement(call) => {
                if !frames.call_reference_bindings_are_stable(machine, call) {
                    return None;
                }
            }
            validation::CallSite::Expression { expression, .. } => {
                if !frames.expression_reference_bindings_are_stable(machine, expression) {
                    return None;
                }
            }
            validation::CallSite::TransitionNamed { .. } => return None,
        };
        for place in places {
            let preserved = match observation {
                super::ValueObservation::Contents => entry_origin_is_stable,
                super::ValueObservation::Binding => binding_origin_is_stable,
            };
            preserved(
                program,
                &flow.control,
                prior.normal_return_writes,
                !prior.normal_return_writes_complete,
                place,
            )
            .then_some(())?;
        }
    }
    Some(())
}

/*
    The prefix gate for a candidate-bearing local asks the question flow's own
    invalidation already answered for every earlier operand call: which caller
    storage could the call reach? `signature_ceiling_places` names the declared
    ceiling of the prior call -- its exclusive actuals and `&mut` receiver,
    rebased through the same divergent-alias candidate map used here -- so a
    nested operand that cannot reach the traced slot or any candidate referent
    preserves the whole set. The resolver's reference-binding walk deliberately
    declines once a divergent binding exists in the prefix, so that gate cannot
    serve this path; the ceiling stays sound because every caller-visible write
    goes through an exclusive borrow it names.
*/
fn preserve_candidate_call_prefix_storage(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    borrow: &checked_trees::BorrowFacts,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    places: &[CanonicalPlace],
) -> Option<()> {
    let position = call_position_in_state(flow, state, call)?;
    let calls = flow.control.calls.span_or_empty(state.calls);
    let borrow_calls = borrow
        .first_state_borrow(machine.symbol, state.state_symbol)
        .map(|(_, owner)| borrow.calls.span_or_empty(owner.calls))
        .unwrap_or(&[]);
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let borrow_call = borrow_calls.iter().find(|candidate| {
            candidate.statement_index == prior.statement_index
                && candidate.call_ordinal == prior.call_ordinal
                && candidate.target_symbol == prior.target_symbol
        })?;
        let writes = crate::flow::signature_ceiling_places(
            program,
            machine.symbol,
            state.state_symbol,
            None,
            borrow_call,
            Some(frames),
        )?;
        if places.iter().any(|place| {
            writes.iter().any(|write| {
                place.root == write.root
                    && canonical_place_segments_may_overlap(
                        program,
                        &place.segments,
                        &write.segments,
                    )
            })
        }) {
            return None;
        }
    }
    Some(())
}

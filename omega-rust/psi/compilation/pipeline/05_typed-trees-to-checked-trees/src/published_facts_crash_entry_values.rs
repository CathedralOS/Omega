/*
    Relate saved crash operands to invocation-entry values.

    Ordinary calls and selected operators share this substitution boundary.
    A stable binding is insufficient when its contents contain mutable loans or
    interior authority. Reuse stable-observation validation before projecting
    fields; shared loans may retain immutable contents without owning them.
    State parameters resolve through every arrival that binds them: the
    invocation itself for the entry state, plus each named transition edge
    into the state, which must all produce the same entry-relative operand.
    A mutable binding additionally keeps that bound snapshot only while its
    storage is pristine: a write, an exclusive borrow, or a mutable receiver
    call before the read (or before a same-state forwarding edge
    that carries the storage into the next arrival) ends provenance. Field
    projections version the storage below the binding root, so a field read
    survives writes confined to disjoint siblings. The receiver is bound once
    at the invocation and never rebound, so a mutable receiver's field keeps
    its entry identity only while no statement that can precede the read can
    write through it — the window spans every state that still reaches the
    read's state, not one arrival's prefix.
    Mutable bindings with unstable contents, divergent arrivals and
    unresolvable cycles retain no entry identity. Substitution transports a
    proven origin, never re-reads an initializer after later operands execute.
    A guard leaf that reads only a member projection needs only that
    projection's provenance: whole-operand failure is not a reason to widen a
    surviving route to `Truth` when the read field's snapshot is intact.
    Member identity comes from the same contextual resolver the canonical
    place algebra uses, so a destructure-bound payload projection keeps its
    `subject.Case::field` steps and a produced operand names
    `receiver.Case::field` — same-named payload fields of different cases stay
    distinct. This is source provenance, not a Terminal certificate.
*/

use super::crash_calls::{field_entry_name, member_entry_name};
use crate::flow::expressions;
use crate::published::facts::crash_entry_values::literal_projection::literal_index;
use crate::values::integer_type;
use checked_trees::CrashPredicateExpression;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableNamePath};
use typed_trees::statement::TransitionTargetNode;
use typed_trees::types::TypeReferenceNode;
use validation::has_contents;

mod checked_projection {
    /*
        Saved constructor projections with checked, point-specific index values.

        Index arithmetic uses the ordinary scalar plan and evaluator, preserving
        selected operations, carrier width and arithmetic policy. Facts belong to
        the index's statement, not a later consumer: local aliases may lead back to
        an earlier snapshot. Writes within that statement conservatively refuse
        substitution because a sibling operand may have changed the observed value.
    */
    use checked_trees::{CheckedOperatorFacts, CrashPredicateExpression, FlowFacts, FlowStateFact};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;

    pub(in crate::published::facts) fn entry_value(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        semantic: &facts::FactPlan,
        flow: &FlowFacts,
        state_flow: &FlowStateFact,
        before_statement: usize,
        expression: ExpressionHandle,
    ) -> Option<CrashPredicateExpression> {
        let machine = program.machine(state_flow.machine_symbol)?;
        super::literal_projection::entry_value(
            program,
            machine.symbol,
            state_flow.state_symbol,
            before_statement,
            expression,
            0,
            &mut |state, statement_index, selector| {
                if !validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    selector,
                ) {
                    return None;
                }
                let primitive = program.primitive_type_reference(
                    validation::expression_result_type_reference(
                        program, machine, state, selector,
                    )?,
                )?;
                let lowered = crate::values::lower_unit_scalar_argument(
                    program,
                    operators,
                    state,
                    statement_index,
                    selector,
                    primitive,
                )?;
                let statements = program.statement_table.statements(state.statement_nodes);
                let statement = statements.get(statement_index)?;
                let parameters = program.state_parameters(state);
                let symbols = parameters
                    .iter()
                    .filter(|parameter| {
                        program
                            .primitive_type_reference(parameter.type_reference)
                            .is_some()
                    })
                    .map(|parameter| parameter.symbol)
                    .chain(
                        statements
                            .get(..statement_index)?
                            .iter()
                            .filter_map(|statement| match statement {
                                StatementNode::LocalData(local)
                                    if !local.is_mutable
                                        && program
                                            .primitive_type_reference(local.type_reference)
                                            .is_some() =>
                                {
                                    Some(local.symbol)
                                }
                                _ => None,
                            }),
                    )
                    .collect::<Vec<_>>();
                let context_span = flow
                    .state_statement(state_flow, statement_index)?
                    .entry_semantic_contexts;
                let contexts = flow
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(context_span);
                let value = crate::values::evaluate_checked_scalar(
                    &lowered,
                    &mut crate::values::PlaceScalarValues {
                        program,
                        parameters,
                        symbols: &symbols,
                        value_at_place: |place: &validation::CanonicalPlace, _observation| {
                            if super::statement_may_overwrite_place(
                                program,
                                machine.symbol,
                                statement,
                                place,
                            ) {
                                return None;
                            }
                            crate::values::scalar_value_at_place(
                                program,
                                semantic,
                                contexts
                                    .iter()
                                    .map(|reference| semantic.contexts.get(reference.context)),
                                place,
                            )
                        },
                    },
                )?;
                let facts::ScalarValue::Integer(value) = value else {
                    return None;
                };
                usize::try_from(value.to_u64()?).ok()
            },
        )
    }
}
mod literal_projection {
    /*
        Recover a constructed leaf's saved value without interpreting its helper.

        A fixed builtin projection can transport an initializer operand through a
        local array/record. Constructor evaluation and sibling crash obligations
        remain in ordinary checking; this only identifies the selected value. Each
        local hop moves back to its declaration prefix and requires stable contents
        and a pristine read projection — a disjoint sibling or element write does
        not dirty the selected leaf, while a write the read covers still ends
        provenance. The syntax-only entrance accepts literal selectors;
        checked_projection supplies point-specific scalar evidence for computed
        selectors. Authored indexing and unknown selectors never become builtin
        constructor substitutions.
    */
    use super::{MAX_ENTRY_PROVENANCE_DEPTH, PlaceSegment, entry_operand_at, member_hop_path};
    use checked_trees::CrashPredicateExpression;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;

    /*
        The caller supplies evaluated index evidence at the projection's own
        statement prefix. Following a saved local changes that prefix; consulting
        the eventual consumer's facts would replay a newer selector value.
    */
    pub(super) fn entry_value(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        before_statement: usize,
        expression: ExpressionHandle,
        depth: u32,
        selector: &mut impl FnMut(&State, usize, ExpressionHandle) -> Option<usize>,
    ) -> Option<CrashPredicateExpression> {
        let (machine, state) =
            validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
        projected_entry_value(
            program,
            machine,
            state,
            before_statement,
            expression,
            &[],
            depth,
            selector,
        )
    }

    pub(crate) fn literal_index(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        let literal = program.expression_table.integer_node(expression)?;
        if literal.landing().is_some_and(|landing| {
            landing.landed_type == numerics::literals::LandedIntegerType::Addr
                || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
        }) {
            return None;
        }
        usize::try_from(literal.value_u64()?).ok()
    }

    fn projected_entry_value(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        before_statement: usize,
        expression: ExpressionHandle,
        segments: &[facts::PlaceSegment],
        depth: u32,
        selector: &mut impl FnMut(&State, usize, ExpressionHandle) -> Option<usize>,
    ) -> Option<CrashPredicateExpression> {
        if depth >= MAX_ENTRY_PROVENANCE_DEPTH
            || !program.expression_table.expression_is_valid(expression)
        {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) => {
                if !validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) {
                    return None;
                }
                let element_index = selector(state, before_statement, indexed.index)?;
                let mut projection = vec![facts::PlaceSegment::FixedIndex {
                    index: element_index,
                }];
                projection.extend_from_slice(segments);
                projected_entry_value(
                    program,
                    machine,
                    state,
                    before_statement,
                    indexed.collection,
                    &projection,
                    depth + 1,
                    selector,
                )
            }
            ExpressionNode::Member(member) => {
                let (_, hop) = member_hop_path(program, member)?;
                let mut projection = hop
                    .into_iter()
                    .map(|segment| match segment {
                        PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
                        PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
                        PlaceSegment::FixedIndex(index) => {
                            Some(facts::PlaceSegment::FixedIndex { index })
                        }
                        PlaceSegment::FixedRange { start, end } => {
                            Some(facts::PlaceSegment::FixedRange { start, end })
                        }
                        PlaceSegment::Opaque => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                projection.extend_from_slice(segments);
                projected_entry_value(
                    program,
                    machine,
                    state,
                    before_statement,
                    member.receiver,
                    &projection,
                    depth + 1,
                    selector,
                )
            }
            ExpressionNode::Name(path)
                if validation::is_single_segment_name_path(program, path) =>
            {
                let (ordinal, local) = program
                    .state_local_declaration(state, path.symbol)
                    .filter(|&(ordinal, _)| ordinal < before_statement)?;
                let read_path: Vec<PlaceSegment> = segments
                    .iter()
                    .map(|segment| super::mutable::canonical_place_segment(program, segment))
                    .collect();
                if !validation::has_contents(
                    program,
                    local.type_reference,
                    &[],
                    validation::ContentsRequirement::StableObservation,
                ) || (local.is_mutable
                    && !super::storage_holds_bound_value(
                        program,
                        machine.symbol,
                        state,
                        ordinal + 1,
                        before_statement.saturating_add(1),
                        local.symbol,
                        &read_path,
                    ))
                {
                    return None;
                }
                let projections = crate::flow::literal_expression_projections(
                    program,
                    local.initial_value,
                    local.type_reference,
                    segments,
                    false,
                )?;
                let [projection] = projections.as_slice() else {
                    return None;
                };
                if projection.remaining.is_empty() {
                    entry_operand_at(
                        program,
                        machine.symbol,
                        state.symbol,
                        ordinal,
                        projection.expression,
                        depth + 1,
                    )
                } else {
                    projected_entry_value(
                        program,
                        machine,
                        state,
                        ordinal,
                        projection.expression,
                        &projection.remaining,
                        depth + 1,
                        selector,
                    )
                }
            }
            _ => None,
        }
    }
}
mod places {
    /*
        Entry provenance for canonical places uses the same binding and write window
        as authored operands. Projection identities are retained until the final
        crash-predicate presentation; no source expression is manufactured.
    */
    use super::mutable::PlaceSegment;
    use super::{entry_operand_symbol_at, field_entry_name};
    use checked_trees::{CheckedStructuralPredicatePathSegment, CrashPredicateExpression};
    use symbols::SymbolHandle;

    /*
        Entry identity for a captured place. The caller has authenticated the
        evaluated operand prefix separately, so only preceding statements belong
        to this provenance window, not the receiving invocation's future writes.
    */
    pub(crate) fn entry_place_before_statement(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<CrashPredicateExpression> {
        entry_place_at_write_boundary(program, machine, state, statement, statement, place)
    }

    fn entry_place_at_write_boundary(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        before_statement: usize,
        write_scan_end: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<CrashPredicateExpression> {
        let symbol = place.root.as_symbol()?;
        validation::canonical_place_type_reference(program, state_symbol, before_statement, place)?;
        let mut projection = Vec::with_capacity(place.segments.len());
        let mut members = Vec::with_capacity(place.segments.len());
        let mut selected_case = None;
        for segment in &place.segments {
            match *segment {
                facts::PlaceSegment::Case { variant } if selected_case.is_none() => {
                    if !variant.is_valid() {
                        return None;
                    }
                    selected_case = Some(variant);
                    projection.push(PlaceSegment::Case(variant));
                }
                facts::PlaceSegment::Field { symbol } => {
                    if facts::payload_variant_for_field(program, symbol) != selected_case {
                        return None;
                    }
                    let field = program.any_field_owner(symbol).map(|(_, field)| field)?;
                    projection.push(PlaceSegment::Field(symbol));
                    members.push(Projection::Member(field_entry_name(
                        program,
                        symbol,
                        field.name.as_str(),
                        None,
                    )));
                    selected_case = None;
                }
                facts::PlaceSegment::FixedIndex { index } if selected_case.is_none() => {
                    projection.push(PlaceSegment::FixedIndex(index));
                    members.push(Projection::Index(index));
                }
                _ => return None,
            }
        }
        if selected_case.is_some() {
            return None;
        }
        let root = entry_operand_symbol_at(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            write_scan_end,
            symbol,
            &projection,
            0,
        )?;
        Some(
            members
                .into_iter()
                .fold(root, |receiver, projection| match projection {
                    Projection::Member(member) => CrashPredicateExpression::Member {
                        receiver: Box::new(receiver),
                        member,
                    },
                    Projection::Index(index) => CrashPredicateExpression::Indexed {
                        collection: Box::new(receiver),
                        index: Box::new(CrashPredicateExpression::Integer(index.to_string())),
                    },
                }),
        )
    }

    /*
        Resolve a current observation to its unchanged invocation parameter and
        canonical structural path. The presentation comes only from the provenance
        replay above; every member is then authenticated against the exact entry
        type, including the case and field identities used by checked predicates.
    */
    pub(crate) fn entry_parameter_place(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        before_statement: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
        entry_parameter_place_at_write_boundary(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            before_statement.saturating_add(1),
            place,
        )
    }

    /*
        Invocation provenance at statement entry. The caller must authenticate the
        observed operand's evaluated write prefix before using this namespace.
    */
    pub(crate) fn entry_parameter_place_before_statement(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        before_statement: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
        entry_parameter_place_at_write_boundary(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            before_statement,
            place,
        )
    }

    fn entry_parameter_place_at_write_boundary(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        before_statement: usize,
        write_scan_end: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
        let expression = entry_place_at_write_boundary(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            write_scan_end,
            place,
        )?;
        let mut projections = Vec::new();
        let mut root = &expression;
        let position = loop {
            match root {
                CrashPredicateExpression::Parameter(position) => break *position,
                CrashPredicateExpression::Member { receiver, member } => {
                    projections.push(Projection::Member(member.clone()));
                    root = receiver;
                }
                CrashPredicateExpression::Indexed { collection, index } => {
                    let CrashPredicateExpression::Integer(index) = index.as_ref() else {
                        return None;
                    };
                    projections.push(Projection::Index(index.parse().ok()?));
                    root = collection;
                }
                _ => return None,
            }
        };
        let machine = program.machine(machine_symbol)?;
        let entry = validation::activation_target_state_index(program, machine, machine_symbol)?;
        let parameters = program.state_parameters(program.machine_states(machine).get(entry)?);
        let mut reference = parameters
            .get(usize::try_from(position).ok()?)?
            .type_reference;
        let mut path = Vec::new();
        let mut identities = Vec::new();
        for projection in projections.into_iter().rev() {
            match projection {
                Projection::Index(index) => {
                    path.push(CheckedStructuralPredicatePathSegment::FixedIndex(
                        u64::try_from(index).ok()?,
                    ));
                    identities.push(facts::PlaceSegment::FixedIndex { index });
                }
                Projection::Member(member) => {
                    let owner = program
                        .type_reference_table
                        .peeled_type_reference(reference);
                    match program.type_reference_table.type_reference(owner) {
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                        | typed_trees::types::TypeReferenceNode::Generic {
                            base_symbol: symbol,
                            ..
                        } if symbol.is_valid() => {}
                        _ => return None,
                    }
                    let data = crate::values::structural_data(program, reference)?;
                    let mut selected = None;
                    for declaration in program.data_members(data) {
                        let (variant, fields) = match declaration {
                            typed_trees::data::DataMember::Field(field) => {
                                (None, std::slice::from_ref(field))
                            }
                            typed_trees::data::DataMember::Variant(variant) => {
                                (Some(variant), program.data_payload_fields(variant))
                            }
                        };
                        for field in fields {
                            let presentation = field_entry_name(
                                program,
                                field.symbol,
                                field.name.as_str(),
                                variant.map(|variant| variant.name.as_str()),
                            );
                            if presentation == member {
                                if selected.is_some() || field.relevance.is_erased() {
                                    return None;
                                }
                                selected = Some((variant, field));
                            }
                        }
                    }
                    let (variant, field) = selected?;
                    if let Some(variant) = variant {
                        path.push(CheckedStructuralPredicatePathSegment::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ),
                        ));
                        identities.push(facts::PlaceSegment::Case {
                            variant: variant.symbol,
                        });
                    }
                    path.push(CheckedStructuralPredicatePathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ));
                    identities.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                }
            }
            let (_, resolved, endpoint, _) = crate::values::resolve_structural_parameter_path(
                program, parameters, position, &path,
            )?;
            if resolved != identities {
                return None;
            }
            reference = endpoint;
        }
        crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
        let root_type = crate::values::scalar::structural_fields::predicate_parameter_type(
            program,
            parameters[usize::try_from(position).ok()?].type_reference,
        )?;
        let (path, _) = crate::values::scalar::structural_fields::explicit_observation_path(
            program, root_type, &path,
        )?;
        Some((position, path))
    }

    /*
        Observation may read stable storage inside an owner whose whole value is
        not an observation (for example, an owner with a nominal cleanup hook).
        Every crossed descriptor validates its referee too: borrowed-storage
        classification alone deliberately treats shared descriptors as opaque.
    */
    pub(super) fn projection_requires_write_scan(
        program: &dyn ::validation::CheckingInput,
        root_type: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
    ) -> Option<bool> {
        let segments = path
            .iter()
            .map(|segment| match *segment {
                PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
                PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
                PlaceSegment::FixedIndex(index) => Some(facts::PlaceSegment::FixedIndex { index }),
                PlaceSegment::FixedRange { .. } | PlaceSegment::Opaque => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let mut mutable = false;
        for boundary in 0..segments.len() {
            if boundary > 0 && matches!(segments[boundary - 1], facts::PlaceSegment::Case { .. }) {
                continue;
            }
            let current = validation::project_type_reference_from_segments(
                program,
                root_type,
                &segments[..boundary],
            )?;
            mutable |= traversed_storage_requires_write_scan(program, current)?;
        }
        let endpoint =
            validation::project_type_reference_from_segments(program, root_type, &segments)?;
        validation::has_contents(
            program,
            endpoint,
            &[],
            validation::ContentsRequirement::StableObservation,
        )
        .then_some(mutable)
    }

    fn traversed_storage_requires_write_scan(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<bool> {
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Reference {
                access, referee, ..
            } => {
                let nested = traversed_storage_requires_write_scan(program, *referee)?;
                Some(nested || *access == language_core::ReferenceAccess::Mutable)
            }
            _ => (validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::BorrowedStorage,
            ) || validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::StableObservation,
            ))
            .then_some(false),
        }
    }

    enum Projection {
        Member(String),
        Index(usize),
    }
}
pub(super) use checked_projection::entry_value as checked_projected_entry_value;
pub(crate) use places::{
    entry_parameter_place, entry_parameter_place_before_statement, entry_place_before_statement,
};
mod mutable {
    /*
        Whether a mutable binding's current storage still holds the value its
        binding position established.

        A `mut` parameter is bound by the invocation (entry state) or by each
        named transition edge into its state; a `let mut` local is bound by its
        initializer. A read sees that bound snapshot only while no intervening
        statement can overwrite the storage or lend it exclusive access. The scan
        is deliberately syntactic and conservative: anything it cannot rule out
        dirties the snapshot, so provenance stays unknown rather than treating a
        later write as the saved actual.

        Field versions refine the same rule below the binding root: a read of
        `rec.value` keeps its bound snapshot across writes, exclusive borrows, and
        receiver-mutating calls confined to sibling fields such as `rec.other`.
        The rooted place path of every write is compared against the read path;
        disjoint sibling projections do not interfere — a case payload field is
        disjoint from its siblings in the same variant, and a write spelled under
        a different variant cannot execute while the bound snapshot's case still
        holds — and the same rule reaches statically fixed element selections: a
        `collection[1]` or `collection[1..3]` read survives a write provably
        outside its window. A dynamic index or any projection the scan cannot
        separate stays opaque and reaches everything at or below it.
    */
    use crate::flow::expressions;
    use crate::flow::state_statements;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{
        StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetNode,
    };

    /*
        One projection step below a binding's root. `Case` marks a sum's variant
        hop — the payload field itself is the following `Field` step, matching the
        canonical place spelling `facts::payload_variant_for_field` produces.
        `FixedIndex` and `FixedRange` carry the canonical algebra's normalized
        element identity for a statically known `collection[i]` or
        `collection[start..end]` selection, so a write confined to a provably
        disjoint element does not dirty the read. `Opaque` marks a position the
        scan cannot separate — a dynamic index or an unresolvable member — which
        interferes with every read at or below it.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum PlaceSegment {
        Field(SymbolHandle),
        Case(SymbolHandle),
        /* A statically selected `collection[i]` element. */
        FixedIndex(usize),
        /*
            A statically selected `collection[start..end]` window — half-open like
            the canonical `facts::PlaceSegment::FixedRange`; `start == end`
            selects no element.
        */
        FixedRange { start: usize, end: usize },
        Opaque,
    }

    /*
        The local scan's segment spelling of a canonical `facts::PlaceSegment`.
        The canonical producer normalizes fixed selections itself; an `Index`
        that still folds to a literal keeps the same element identity here, and a
        genuinely dynamic index stays opaque.
    */
    pub(super) fn canonical_place_segment(
        program: &dyn ::validation::CheckingInput,
        segment: &facts::PlaceSegment,
    ) -> PlaceSegment {
        match segment {
            facts::PlaceSegment::Field { symbol } => PlaceSegment::Field(*symbol),
            facts::PlaceSegment::Case { variant } => PlaceSegment::Case(*variant),
            facts::PlaceSegment::FixedIndex { index } => PlaceSegment::FixedIndex(*index),
            facts::PlaceSegment::FixedRange { start, end } => PlaceSegment::FixedRange {
                start: *start,
                end: *end,
            },
            facts::PlaceSegment::Index { expression } => fixed_index_segment(program, *expression),
            facts::PlaceSegment::OpaqueRange => PlaceSegment::Opaque,
        }
    }

    /*
        The element segment an `Indexed` step contributes to a rooted path. The
        index is folded by the same `constant_integer_value` normalization the
        canonical `FixedIndex`/`FixedRange` segments use; a `start..end` range
        index keeps its bounds as one half-open window. Anything not statically
        known stays `Opaque` — conservative, since element writes cannot be
        separated below the collection root.
    */
    pub(crate) fn fixed_index_segment(
        program: &dyn ::validation::CheckingInput,
        index: ExpressionHandle,
    ) -> PlaceSegment {
        let constant = |expression: ExpressionHandle| {
            expressions(program)
                .constant_integer_value(expression)
                .and_then(|value| usize::try_from(value).ok())
        };
        if let ExpressionNode::Range(range) = program.expression_table.expression(index)
            && let (Some(start), Some(end)) = (constant(range.start), constant(range.end))
            && let Some(end) = end.checked_add(usize::from(range.end_inclusive))
        {
            return PlaceSegment::FixedRange { start, end };
        }
        constant(index)
            .map(PlaceSegment::FixedIndex)
            .unwrap_or(PlaceSegment::Opaque)
    }

    /*
        A write to `write` reaches a read of `read` only when the two paths cannot
        be separated: equal segments until one path ends (either covers the
        other), or an opaque step. Distinct sibling fields never interfere. Two
        different variants share the payload slot but a case-qualified write only
        executes while the scrutinee holds its case — reaching a read under a
        different variant requires re-seating the whole binding, whose root write
        interferes on its own — so distinct `Case` hops separate the same way
        `canonical_place_segment_pair_may_overlap` rules them non-overlapping.
        Fixed element positions separate by the canonical window rules too:
        disjoint indices and disjoint half-open windows never overlap, and a
        fixed index outside a fixed window is unreachable. A segment kind the
        scan cannot order against its counterpart — a dynamic index, an opaque
        step, or a heterogeneous pair — stays interfering.
    */
    fn paths_interfere(write: &[PlaceSegment], read: &[PlaceSegment]) -> bool {
        for (write, read) in write.iter().zip(read.iter()) {
            match (write, read) {
                (PlaceSegment::Field(write), PlaceSegment::Field(read)) => {
                    if write != read {
                        return false;
                    }
                }
                (PlaceSegment::Case(write), PlaceSegment::Case(read)) => {
                    if write != read {
                        return false;
                    }
                }
                (PlaceSegment::FixedIndex(write), PlaceSegment::FixedIndex(read)) => {
                    if write != read {
                        return false;
                    }
                }
                (
                    PlaceSegment::FixedRange {
                        start: write_start,
                        end: write_end,
                    },
                    PlaceSegment::FixedRange {
                        start: read_start,
                        end: read_end,
                    },
                ) => {
                    if !(write_start < write_end
                        && read_start < read_end
                        && write_start < read_end
                        && read_start < write_end)
                    {
                        return false;
                    }
                }
                (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex(index))
                | (PlaceSegment::FixedIndex(index), PlaceSegment::FixedRange { start, end }) => {
                    if !(start < end && start <= index && index < end) {
                        return false;
                    }
                }
                _ => return true,
            }
        }
        true
    }

    /*
        The binding root plus projection steps of a place expression, mirroring
        `expression_root_symbol`'s rooting — `self.<field>` roots at the machine
        field — while retaining the segments a whole-symbol scan discards. A
        non-place expression has no root.
    */
    fn rooted_place_path(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, Vec<PlaceSegment>)> {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) => {
                rooted_place_path(program, machine_symbol, indexed.collection).map(
                    |(root, mut path)| {
                        path.push(fixed_index_segment(program, indexed.index));
                        (root, path)
                    },
                )
            }
            ExpressionNode::Borrow(borrow) => {
                rooted_place_path(program, machine_symbol, borrow.target)
            }
            ExpressionNode::Member(member) => {
                if let Some(path) = program.expression_table.name_path_node(member.receiver)
                    && path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol
                {
                    let symbol =
                        validation::effective_member_symbol(program, member.receiver, member);
                    return symbol.is_valid().then_some((symbol, Vec::new()));
                }
                rooted_place_path(program, machine_symbol, member.receiver).map(
                    |(root, mut path)| {
                        match super::member_hop_path(program, member) {
                            Some((_, hop)) => path.extend(hop),
                            None => path.push(PlaceSegment::Opaque),
                        }
                        (root, path)
                    },
                )
            }
            ExpressionNode::Name(path) => {
                validation::first_valid_name_path_symbol(path, &program.expression_table)
                    .map(|root| (root, Vec::new()))
            }
            _ => None,
        }
    }

    /*
        The written or exclusively borrowed place `expression` reaches `symbol`'s
        storage at a path the read cannot be separated from.
    */
    fn place_interferes(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        expression: ExpressionHandle,
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        rooted_place_path(program, machine_symbol, expression).is_some_and(|(root, write_path)| {
            root == symbol && paths_interfere(&write_path, read_path)
        })
    }

    /*
        Whether `statement` may overwrite the canonical `place` at all — a write,
        an exclusive borrow, or a receiver-mutating call the scan cannot separate
        from the read. Non-symbol roots cannot be reasoned about and stay
        conservative. A caller-side value proof uses this to refuse reads whose
        call-entry context postdates a sibling operand's effects.
    */
    pub(crate) fn statement_may_overwrite_place(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        place: &validation::CanonicalPlace,
    ) -> bool {
        let Some(symbol) = place.root.as_symbol() else {
            return true;
        };
        let read_path: Vec<PlaceSegment> = place
            .segments
            .iter()
            .map(|segment| canonical_place_segment(program, segment))
            .collect();
        statement_may_overwrite(program, machine_symbol, statement, symbol, &read_path)
    }

    /*
        No statement in `state[start..end]` may overwrite `symbol`'s storage at
        `read_path` or lend that projection exclusive access. An empty read path
        is the whole binding: every write rooted at `symbol` interferes. A
        malformed window cannot be cleared.
    */
    pub(super) fn storage_holds_bound_value(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        state: &typed_trees::state::State,
        start: usize,
        end: usize,
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        let Some(window) = state_statements(program, state.statement_nodes).get(start..end) else {
            return false;
        };
        !window.iter().any(|statement| {
            statement_may_overwrite(program, machine_symbol, statement, symbol, read_path)
        })
    }

    fn statement_may_overwrite(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        statement: &StatementNode,
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        match statement {
            StatementNode::Assembly(_) => false,
            StatementNode::Assignment(assignment) => {
                place_interferes(
                    program,
                    machine_symbol,
                    assignment.target,
                    symbol,
                    read_path,
                ) || expression_may_overwrite(
                    program,
                    machine_symbol,
                    assignment.target,
                    symbol,
                    read_path,
                ) || expression_may_overwrite(
                    program,
                    machine_symbol,
                    assignment.value,
                    symbol,
                    read_path,
                )
            }
            StatementNode::AssemblyFact(fact) => program
                .tables
                .proof_fact_expression_roots(fact.fact)
                .into_iter()
                .any(|expression| {
                    expression_may_overwrite(program, machine_symbol, expression, symbol, read_path)
                }),
            StatementNode::Expression(expression) => {
                expression_may_overwrite(program, machine_symbol, *expression, symbol, read_path)
            }
            StatementNode::LocalData(local) => expression_may_overwrite(
                program,
                machine_symbol,
                local.initial_value,
                symbol,
                read_path,
            ),
            StatementNode::Call(call) => {
                (call.receiver_root_symbol == symbol
                    && receiver_call_writes(program, call.target_symbol)
                    && call_receiver_path_interferes(program, call, read_path))
                    || call_arguments_may_overwrite(
                        program,
                        machine_symbol,
                        call.target_symbol,
                        program.statement_table.expression_handles(call.arguments),
                        symbol,
                        read_path,
                    )
                    || program
                        .statement_table
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| {
                            expression_may_overwrite(
                                program,
                                machine_symbol,
                                *argument,
                                symbol,
                                read_path,
                            )
                        })
            }
            StatementNode::Transition(transition) => {
                transition_may_overwrite(program, machine_symbol, transition, symbol, read_path)
            }
        }
    }

    /*
        A statement call's receiver place covers the read path. The receiver is a
        name path, not an expression: `x` covers the whole binding, `x.<field>`
        confines the write to the resolved leaf member, and a deeper or
        unresolvable projection cannot be separated below its root.
    */
    fn call_receiver_path_interferes(
        program: &dyn ::validation::CheckingInput,
        call: &TableCall,
        read_path: &[PlaceSegment],
    ) -> bool {
        let write_path: Vec<PlaceSegment> = match program
            .statement_table
            .name_path_members(call.receiver)
            .len()
        {
            0 | 1 => Vec::new(),
            2 if call.receiver_symbol.is_valid() => {
                vec![PlaceSegment::Field(call.receiver_symbol)]
            }
            _ => vec![PlaceSegment::Opaque],
        };
        paths_interfere(&write_path, read_path)
    }

    /*
        Guard, named-target arguments, and value targets of one transition. A
        nested exclusive borrow or receiver-mutating call inside any of them can
        reach `symbol`'s storage before the edge is taken.
    */
    fn transition_may_overwrite(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        transition: &TableTransition,
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        if let TransitionGuardNode::When(guard) = &transition.guard
            && expression_may_overwrite(program, machine_symbol, *guard, symbol, read_path)
        {
            return true;
        }
        transition.targets().any(
            |target| match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    let arguments = program.statement_table.expression_handles(*arguments);
                    let internal =
                        program
                            .machine_by_symbol(machine_symbol)
                            .is_some_and(|machine| {
                                validation::activation_target_state_index(
                                    program,
                                    machine,
                                    path.symbol,
                                )
                                .is_some()
                            });
                    (!internal
                        && call_arguments_may_overwrite(
                            program,
                            machine_symbol,
                            path.symbol,
                            arguments,
                            symbol,
                            read_path,
                        ))
                        || arguments.iter().any(|argument| {
                            expression_may_overwrite(
                                program,
                                machine_symbol,
                                *argument,
                                symbol,
                                read_path,
                            )
                        })
                }
                TransitionTargetNode::Value(value) => {
                    expression_may_overwrite(program, machine_symbol, *value, symbol, read_path)
                }
                TransitionTargetNode::Terminal => false,
            },
        )
    }

    /*
        `symbol`'s storage at `read_path` is reached by an exclusive borrow or by
        a call whose callee may write its receiver or a reference argument.
        Writes through aliases are conservatively covered by their exclusive
        borrow creation or the call that receives mutable access.
    */
    fn expression_may_overwrite(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        root: ExpressionHandle,
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        let mut pending = vec![root];
        while let Some(expression) = pending.pop() {
            if !program.expression_table.expression_is_valid(expression) {
                return true;
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => {
                    if borrow.access.is_exclusive()
                        && place_interferes(
                            program,
                            machine_symbol,
                            borrow.target,
                            symbol,
                            read_path,
                        )
                    {
                        return true;
                    }
                    pending.push(borrow.target);
                }
                ExpressionNode::Call(call) => {
                    if call_arguments_may_overwrite(
                        program,
                        machine_symbol,
                        call.target_symbol,
                        program.expression_table.expression_handles(call.arguments),
                        symbol,
                        read_path,
                    ) {
                        return true;
                    }
                    if call.receiver.is_valid()
                        && place_interferes(
                            program,
                            machine_symbol,
                            call.receiver,
                            symbol,
                            read_path,
                        )
                        && receiver_call_writes(program, call.target_symbol)
                    {
                        return true;
                    }
                    if call.receiver.is_valid() {
                        pending.push(call.receiver);
                    }
                    pending.extend(
                        expressions(program)
                            .expression_handles(call.arguments)
                            .iter()
                            .copied(),
                    );
                }
                ExpressionNode::Atomic(atomic) => {
                    if place_interferes(program, machine_symbol, atomic.value, symbol, read_path) {
                        return true;
                    }
                    pending.push(atomic.value);
                    if atomic.result.is_valid() {
                        pending.push(atomic.result);
                    }
                }
                node => {
                    program.subtree_children(node, typed_trees::SubtreeEdges::VALUES, |child| {
                        pending.push(child);
                        false
                    })
                }
            }
        }
        false
    }

    /*
        A call whose receiver is rooted at the tracked symbol writes that storage
        when the callee's `self` parameter is mutable. An unresolvable or
        self-less target cannot be cleared of writing.
    */
    fn receiver_call_writes(
        program: &dyn ::validation::CheckingInput,
        target_symbol: SymbolHandle,
    ) -> bool {
        match validation::call_target_parameters(program, target_symbol) {
            Some(parameters) => parameters
                .iter()
                .find(|_parameter| false)
                .is_none_or(|parameter| parameter.is_mutable),
            None => true,
        }
    }

    /*
        Passing an existing mutable reference grants the same write access as
        spelling a fresh exclusive borrow. Unknown callees cannot clear a place
        argument of writing; ordinary value parameters do not grant such access.
    */
    fn call_arguments_may_overwrite(
        program: &dyn ::validation::CheckingInput,
        machine_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        arguments: &[ExpressionHandle],
        symbol: SymbolHandle,
        read_path: &[PlaceSegment],
    ) -> bool {
        let argument_may_write = |argument: &ExpressionHandle| {
            rooted_place_path(program, machine_symbol, *argument).is_none_or(
                |(root, write_path)| root == symbol && paths_interfere(&write_path, read_path),
            )
        };
        let Some(parameters) = validation::call_target_parameters(program, target_symbol) else {
            return arguments.iter().any(argument_may_write);
        };
        parameters
            .iter()
            .zip(arguments)
            .any(|(parameter, argument)| {
                let mut reference = parameter.type_reference;
                let writes = loop {
                    match program.type_reference_table.type_reference(reference) {
                        typed_trees::types::TypeReferenceNode::Constrained {
                            base_type, ..
                        } => reference = *base_type,
                        typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                            break access.is_exclusive();
                        }
                        _ => break false,
                    }
                };
                writes && argument_may_write(argument)
            })
    }
}
pub(super) use mutable::statement_may_overwrite_place;
use mutable::{PlaceSegment, fixed_index_segment, storage_holds_bound_value};

/*
    Arrival provenance can revisit a state parameter through a transition
    cycle, so the fold carries a depth bound. Exhaustion is unproven
    provenance, never an affirmed origin.
*/
const MAX_ENTRY_PROVENANCE_DEPTH: u32 = 64;

pub(super) fn entry_operand(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    entry_operand_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        expression,
        0,
    )
}

/*
    Whether `operand`'s storage still proves the invocation-entry value at the
    place `leaf` projects below its own root. `leaf` is a place occurrence
    inside a crash guard and roots at one of the operator's parameter symbols;
    `operand` is the caller expression bound to that parameter. A bare `cell`
    leaf asks for the operand's whole storage — the same boundary
    `entry_operand` draws — while `cell.count` asks only about the `count`
    projection, so writes confined to disjoint sibling storage keep the
    provenance the guard actually reads. The operand's own field steps prepend
    to the leaf's projection, so a leaf `cell.count` bound to operand
    `pair.cell` asks about `pair.cell.count`. Anything the rooted-place scan
    cannot separate — an opaque projection, an unresolvable member, a
    non-shared borrow, or a non-place operand carrying a non-empty projection
    — refuses the same way the whole-operand path did.
*/
pub(super) fn operand_entry_provenance(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    operand: ExpressionHandle,
    leaf: ExpressionHandle,
) -> bool {
    let mut projection = Vec::new();
    let mut leaf_place = leaf;
    loop {
        if !program.expression_table.expression_is_valid(leaf_place) {
            return false;
        }
        match program.expression_table.expression(leaf_place) {
            ExpressionNode::Member(member) => {
                match member_hop_path(program, member) {
                    Some((_, hop)) => {
                        for segment in hop.into_iter().rev() {
                            projection.push(segment);
                        }
                    }
                    None => projection.push(PlaceSegment::Opaque),
                }
                leaf_place = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                projection.push(fixed_index_segment(program, indexed.index));
                leaf_place = indexed.collection;
            }
            ExpressionNode::Borrow(borrow) => leaf_place = borrow.target,
            ExpressionNode::Name(_) => break,
            _ => return false,
        }
    }
    projection.reverse();

    let mut operand_place = operand;
    loop {
        if !program.expression_table.expression_is_valid(operand_place) {
            return false;
        }
        match program.expression_table.expression(operand_place) {
            ExpressionNode::Member(member) => {
                let Some((_, hop)) = member_hop_path(program, member) else {
                    return false;
                };
                for segment in hop.into_iter().rev() {
                    projection.insert(0, segment);
                }
                operand_place = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if literal_projection::entry_value(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    operand_place,
                    0,
                    &mut |_, _, expression| literal_index(program, expression),
                )
                .is_some()
                {
                    return true;
                }
                if entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    indexed.index,
                    0,
                )
                .is_none()
                {
                    return false;
                }
                projection.insert(0, fixed_index_segment(program, indexed.index));
                operand_place = indexed.collection;
            }
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                operand_place = borrow.target
            }
            ExpressionNode::Name(path) => {
                return entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &projection,
                    0,
                )
                .is_some();
            }
            _ => {
                return projection.is_empty()
                    && entry_operand_at(
                        program,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        operand,
                        0,
                    )
                    .is_some();
            }
        }
    }
}

/*
    The operand's entry operand when only `leaf_projection` below its root is
    read — the expression-returning twin of `operand_entry_provenance`. A
    mutable binding written only outside the read projection still supplies
    the entry snapshot for that field, so a surviving route may keep the
    caller's per-field name instead of widening to `Truth`. The produced
    expression names only the operand's own member spine; the leaf's
    projection is applied by the substituting `Member` nodes above the
    `Parameter` leaf, never doubled here.
*/
pub(super) fn entry_operand_projected(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    operand: ExpressionHandle,
    leaf_projection: &[PlaceSegment],
) -> Option<CrashPredicateExpression> {
    entry_operand_projected_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        operand,
        leaf_projection,
        0,
    )
}

fn entry_operand_projected_at(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    operand: ExpressionHandle,
    leaf_projection: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH {
        return None;
    }
    let mut projection = leaf_projection.to_vec();
    let mut spine = Vec::new();
    let mut operand_place = operand;
    loop {
        if !program.expression_table.expression_is_valid(operand_place) {
            return None;
        }
        match program.expression_table.expression(operand_place) {
            ExpressionNode::Member(member) => {
                let (symbol, hop) = member_hop_path(program, member)?;
                spine.insert(
                    0,
                    OperandSpineStep::Member(member_entry_name(program, member, symbol)),
                );
                for segment in hop.into_iter().rev() {
                    projection.insert(0, segment);
                }
                operand_place = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if !builtin_index_meaning(program, machine_symbol, state_symbol, operand_place) {
                    return None;
                }
                if let Some(value) = literal_projection::entry_value(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    operand_place,
                    depth + 1,
                    &mut |_, _, expression| literal_index(program, expression),
                ) {
                    return Some(spine.iter().fold(value, |receiver, step| match step {
                        OperandSpineStep::Member(member) => CrashPredicateExpression::Member {
                            receiver: Box::new(receiver),
                            member: member.clone(),
                        },
                        OperandSpineStep::Indexed(index) => CrashPredicateExpression::Indexed {
                            collection: Box::new(receiver),
                            index: Box::new(index.clone()),
                        },
                    }));
                }
                let index = entry_operand_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    indexed.index,
                    depth + 1,
                )?;
                spine.insert(0, OperandSpineStep::Indexed(index));
                projection.insert(0, fixed_index_segment(program, indexed.index));
                operand_place = indexed.collection;
            }
            ExpressionNode::Borrow(borrow)
                if borrow.access == language_core::ReferenceAccess::Shared =>
            {
                operand_place = borrow.target;
            }
            ExpressionNode::Name(path) => {
                let root = entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &projection,
                    depth + 1,
                )?;
                return Some(spine.iter().fold(root, |receiver, step| match step {
                    OperandSpineStep::Member(member) => CrashPredicateExpression::Member {
                        receiver: Box::new(receiver),
                        member: member.clone(),
                    },
                    OperandSpineStep::Indexed(index) => CrashPredicateExpression::Indexed {
                        collection: Box::new(receiver),
                        index: Box::new(index.clone()),
                    },
                }));
            }
            _ => {
                return if projection.is_empty() {
                    entry_operand_at(
                        program,
                        machine_symbol,
                        state_symbol,
                        before_statement,
                        operand,
                        depth + 1,
                    )
                } else {
                    None
                };
            }
        }
    }
}

/*
    One step of the operand's own place spine, root-to-leaf: a resolved
    member keeps its authored (possibly case-qualified) name, while an index
    step carries the index's entry operand so the produced `Indexed` node
    replays the caller's read at the invocation's values.
*/
enum OperandSpineStep {
    Member(String),
    Indexed(CrashPredicateExpression),
}

/*
    The place steps one member hop adds below its receiver, in root-to-leaf
    order: the resolved field, preceded by its declaring case variant for a
    payload field — matching `facts::payload_variant_for_field`'s canonical
    spelling. Synthesized members — the destructure-bound payload projections
    `subject.Case::field` — retain no `member_symbol`, so identity comes from
    the shared contextual resolver the canonical place algebra uses, never
    from the first same-named field. `None` when the member does not resolve
    to a declared field.

    A builtin extent read (`s.len`) resolves no member symbol either, but its
    operand's entry observation is still exact: the binding's entry snapshot
    supplies the extent, and the by-name `Member` spelling the callee's own
    leaf carries (`Member { receiver, "len" }`) survives substitution
    unchanged — typing keeps the actual's `len` the same extent read as the
    formal's. The hop is one opaque projection: `paths_interfere` treats it
    as reaching the whole binding, so any write that could rebind or resize
    the receiver ends a mutable formal's stored-extent provenance, and the
    leaf/walks the opaque member already degrade the same way. The returned
    symbol stays invalid so `member_entry_name` keeps the authored `len`
    name. Other unresolvable members still admit no hop at all.
*/
fn member_hop_path(
    program: &dyn ::validation::CheckingInput,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<(SymbolHandle, Vec<PlaceSegment>)> {
    let symbol = validation::effective_member_symbol(program, member.receiver, member);
    if !symbol.is_valid() {
        return (member.case_variant.is_none()
            && CollectionMeasure::from_authored_spelling(member.member.as_str())
                == Some(CollectionMeasure::Length))
        .then(|| (SymbolHandle::invalid(), vec![PlaceSegment::Opaque]));
    }
    let mut path = Vec::with_capacity(2);
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        path.push(PlaceSegment::Case(variant));
    }
    path.push(PlaceSegment::Field(symbol));
    Some((symbol, path))
}

/*
    The `PlaceSegment` projection `members` spells below a formal's declared
    type. Each member resolves against the previous step's data declaration so
    the pristine-storage scan compares the same field symbols writes carry; a
    member that does not resolve to a plain data field — a case payload, an
    unresolvable or non-record step — is opaque and interferes with every
    write that reaches it. Resolution stops at the first opaque step: nothing
    below it can be separated anyway.
*/
pub(super) fn formal_member_projection(
    program: &dyn ::validation::CheckingInput,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
    members: &[String],
) -> Vec<PlaceSegment> {
    let mut projection = Vec::with_capacity(members.len());
    for member in members {
        let peeled = program
            .type_reference_table
            .peeled_type_reference(type_reference);
        let data_symbol = match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => *symbol,
            _ => SymbolHandle::invalid(),
        };
        let mut owners = program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == data_symbol);
        let field = owners.next().and_then(|data| {
            (owners.next().is_none())
                .then(|| {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|candidate| match candidate {
                            typed_trees::data::DataMember::Field(field)
                                if field.name.as_str() == member.as_str() =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })
                })
                .flatten()
        });
        let Some(field) = field else {
            projection.push(PlaceSegment::Opaque);
            break;
        };
        projection.push(PlaceSegment::Field(field.symbol));
        type_reference = field.type_reference;
    }
    projection
}

/*
    Resolve a canonical predicate's member names through the formal telescope.
    Opaque spelling cannot authenticate a place or its entry provenance.
*/
pub(super) fn formal_member_place_projection(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    members: &[String],
) -> Option<Vec<facts::PlaceSegment>> {
    formal_member_projection(program, reference, members)
        .into_iter()
        .map(|segment| match segment {
            PlaceSegment::Field(symbol) => Some(facts::PlaceSegment::Field { symbol }),
            PlaceSegment::Case(variant) => Some(facts::PlaceSegment::Case { variant }),
            PlaceSegment::FixedIndex(index) => Some(facts::PlaceSegment::FixedIndex { index }),
            PlaceSegment::FixedRange { .. } | PlaceSegment::Opaque => None,
        })
        .collect()
}

fn entry_operand_at(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
    depth: u32,
) -> Option<CrashPredicateExpression> {
    macro_rules! entry_child {
        ($expression:expr $(,)?) => {
            entry_operand_at(
                program,
                machine_symbol,
                state_symbol,
                before_statement,
                $expression,
                depth + 1,
            )
        };
    }
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH
        || !program.expression_table.expression_is_valid(expression)
    {
        return None;
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Indexed(_) | ExpressionNode::Member(_)
    ) && let Some(value) = literal_projection::entry_value(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        expression,
        depth,
        &mut |_, _, expression| literal_index(program, expression),
    ) {
        return Some(value);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            let mut agreed = None;
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let value = entry_child!(arm.value)?;
                if agreed.as_ref().is_some_and(|known| *known != value) {
                    return None;
                }
                agreed = Some(value);
            }
            agreed
        }

        ExpressionNode::Boolean(value) => Some(CrashPredicateExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => {
            Some(CrashPredicateExpression::Integer(value.to_string()))
        }
        ExpressionNode::Float(value) => {
            Some(CrashPredicateExpression::Float(value.text().to_owned()))
        }
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            let (machine, state) =
                validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
            let source_reference =
                validation::expression_result_type_reference(program, machine, state, cast.value)?;
            let source_type = program.primitive_type_reference(
                program
                    .type_reference_table
                    .peeled_type_reference(source_reference),
            )?;
            let target_type = program.primitive_type_reference(
                program
                    .type_reference_table
                    .peeled_type_reference(cast.target_type),
            )?;
            if !source_type.is_fixed_integer()
                || !target_type.is_fixed_integer()
                || source_type.scalar_byte_size() != target_type.scalar_byte_size()
            {
                return None;
            }
            let operand = entry_child!(cast.value)?;
            Some(CrashPredicateExpression::RepresentationView {
                source_type: integer_type(source_type)?,
                target_type: integer_type(target_type)?,
                operand: Box::new(operand),
            })
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() && cast.semantic_domain.is_empty() => {
            let (machine, state) =
                validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
            let source = program.primitive_type_reference(
                validation::expression_result_type_reference(program, machine, state, cast.value)?,
            )?;
            let target = program.primitive_type_reference(cast.target_type)?;
            if !validation::integer_widen_is_total(source, target) {
                return None;
            }
            Some(CrashPredicateExpression::IntegerWiden {
                source_type: source as u8,
                target_type: target as u8,
                domain: cast.domain as u8,
                operand: Box::new(entry_child!(cast.value)?),
            })
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(CrashPredicateExpression::Unary {
                operator: unary.operator as u8,
                operand: Box::new(entry_child!(unary.operand)?),
            })
        }
        ExpressionNode::Borrow(borrow)
            if borrow.access == language_core::ReferenceAccess::Shared =>
        {
            entry_child!(borrow.target)
        }
        ExpressionNode::Member(member) => {
            let (symbol, hop) = member_hop_path(program, member)?;
            let mut segments: Vec<PlaceSegment> = hop.into_iter().rev().collect();
            let mut member_names = vec![member_entry_name(program, member, symbol)];
            let mut base = member.receiver;
            loop {
                if !program.expression_table.expression_is_valid(base) {
                    return None;
                }
                match program.expression_table.expression(base) {
                    ExpressionNode::Member(inner) => {
                        let Some((symbol, hop)) = member_hop_path(program, inner) else {
                            break;
                        };
                        for segment in hop.into_iter().rev() {
                            segments.push(segment);
                        }
                        member_names.push(member_entry_name(program, inner, symbol));
                        base = inner.receiver;
                    }
                    _ => break,
                }
            }
            segments.reverse();
            member_names.reverse();
            let root = match program.expression_table.expression(base) {
                ExpressionNode::Name(path) => entry_operand_name_at(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    path,
                    &segments,
                    depth + 1,
                )?,
                _ => entry_child!(base)?,
            };
            Some(member_names.iter().fold(root, |receiver, member| {
                CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member: member.clone(),
                }
            }))
        }
        ExpressionNode::Indexed(indexed) => {
            if !builtin_index_meaning(program, machine_symbol, state_symbol, expression) {
                return None;
            }
            Some(CrashPredicateExpression::Indexed {
                collection: Box::new(entry_child!(indexed.collection)?),
                index: Box::new(entry_child!(indexed.index)?),
            })
        }
        ExpressionNode::Range(range) => Some(CrashPredicateExpression::Range {
            start: Box::new(entry_child!(range.start)?),
            end: Box::new(entry_child!(range.end)?),
            end_inclusive: range.end_inclusive,
        }),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Add
                    | typed_trees::expression::BinaryOperator::Subtract
                    | typed_trees::expression::BinaryOperator::Multiply
                    | typed_trees::expression::BinaryOperator::Divide
                    | typed_trees::expression::BinaryOperator::Modulo
                    | typed_trees::expression::BinaryOperator::BitwiseAnd
                    | typed_trees::expression::BinaryOperator::BitwiseOr
                    | typed_trees::expression::BinaryOperator::BitwiseXor
                    | typed_trees::expression::BinaryOperator::ShiftLeft
                    | typed_trees::expression::BinaryOperator::ShiftRight
                    | typed_trees::expression::BinaryOperator::And
                    | typed_trees::expression::BinaryOperator::Or
                    | typed_trees::expression::BinaryOperator::Equal
                    | typed_trees::expression::BinaryOperator::NotEqual
                    | typed_trees::expression::BinaryOperator::Less
                    | typed_trees::expression::BinaryOperator::LessOrEqual
                    | typed_trees::expression::BinaryOperator::Greater
                    | typed_trees::expression::BinaryOperator::GreaterOrEqual
            ) && (builtin_binary_meaning(program, machine_symbol, state_symbol, expression)
                || ieee_comparison_has_entry_meaning(
                    program,
                    machine_symbol,
                    state_symbol,
                    expression,
                )) =>
        {
            Some(CrashPredicateExpression::Binary {
                operator: binary.operator as u8,
                left: Box::new(entry_child!(binary.left)?),
                right: Box::new(entry_child!(binary.right)?),
            })
        }
        ExpressionNode::Name(path) => entry_operand_name_at(
            program,
            machine_symbol,
            state_symbol,
            before_statement,
            path,
            &[],
            depth,
        ),
        _ => None,
    }
}

/*
    A single-member name's entry operand. `field_path` is the field projection
    below this binding that the enclosing `Member` chain reads: for a mutable
    local or parameter the pristine-storage window only has to keep that
    projection unwritten, since sibling fields version independently.
*/
fn entry_operand_name_at(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    path: &TableNamePath,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH
        || !program.expression_table.name_path_is_single_member(path)
    {
        return None;
    }
    validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
    let symbol = path.symbol;
    entry_operand_symbol_at(
        program,
        machine_symbol,
        state_symbol,
        before_statement,
        before_statement.saturating_add(1),
        symbol,
        field_path,
        depth,
    )
}

/*
    The initializer of `symbol`'s local declaration that a read before
    `before_statement` still provably observes, with the declaration's ordinal.
    A writable local — or one whose scalar storage observes borrowed contents —
    keeps its bound snapshot only while no statement in
    `[ordinal + 1, write_scan_end)` could have overwritten the read projection
    or lent it exclusive access. The caller supplies the exact end of that write
    window: an unscoped expression includes its containing statement, while a
    captured operand separately authenticates that statement's prefix. `None`
    means the read cannot claim the binding's value, never that it changed.
*/
fn local_read_initializer(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
    before_statement: usize,
    write_scan_end: usize,
    field_path: &[PlaceSegment],
) -> Option<(usize, ExpressionHandle)> {
    let (ordinal, local) = program
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < before_statement)?;
    let mutable_storage =
        observation_requires_write_scan(program, local.type_reference, field_path)?;
    if (local.is_mutable || mutable_storage)
        && !storage_holds_bound_value(
            program,
            machine_symbol,
            state,
            ordinal + 1,
            write_scan_end,
            local.symbol,
            field_path,
        )
    {
        return None;
    }
    Some((ordinal, local.initial_value))
}

/*
    The saved initializer a local read at `before_statement` still observes, for
    callers that read that value instead of the entry identity. The local
    carrier rule — including the write window for a writable binding — stays
    here, so call-actual substitution and guard admission cannot drift apart on
    when a mutable local still holds what its initializer recorded.
*/
pub(crate) fn local_initial_value_before_statement(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    let (_, state) = validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
    let (_, initial_value) = local_read_initializer(
        program,
        machine_symbol,
        state,
        symbol,
        before_statement,
        before_statement.saturating_add(1),
        &[],
    )?;
    Some(initial_value)
}

/*
    Shared provenance for an already resolved binding. Expression callers
    validate their name syntax first; canonical places already retain the symbol.
*/
fn entry_operand_symbol_at(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    write_scan_end: usize,
    symbol: SymbolHandle,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    if depth >= MAX_ENTRY_PROVENANCE_DEPTH || !symbol.is_valid() {
        return None;
    }
    let (machine, state) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)?;
    if let Some((ordinal, initial_value)) = local_read_initializer(
        program,
        machine_symbol,
        state,
        symbol,
        before_statement,
        write_scan_end,
        field_path,
    ) {
        return entry_operand_projected_at(
            program,
            machine_symbol,
            state_symbol,
            ordinal,
            initial_value,
            field_path,
            depth + 1,
        );
    }
    state_parameter_entry_operand(
        program,
        machine,
        machine_symbol,
        state_symbol,
        symbol,
        before_statement,
        write_scan_end,
        field_path,
        depth,
    )
}

/*
    `self` names the containing machine (or its attached data), not the
    receiver's telescope row — the same resolution
    `structural_fields::exact_self_parameter` applies for authored contract
    predicates. Bind the name to the state's `is_self` parameter so receiver
    projections take the parameter path below.
*/

/*
    A sealed or builtin IEEE comparison is an entry predicate when its
    operands are. Primitive float equality is not the integer builtin, and
    the transition subject that captures it still has to carry the comparison
    the published crash guard names.
*/
fn ieee_comparison_has_entry_meaning(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        typed_trees::expression::BinaryOperator::Equal
            | typed_trees::expression::BinaryOperator::NotEqual
            | typed_trees::expression::BinaryOperator::Less
            | typed_trees::expression::BinaryOperator::LessOrEqual
            | typed_trees::expression::BinaryOperator::Greater
            | typed_trees::expression::BinaryOperator::GreaterOrEqual
    ) {
        return false;
    }
    let Some((machine, state)) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
    else {
        return false;
    };
    let left = validation::expression_result_type_reference(program, machine, state, binary.left);
    let right = validation::expression_result_type_reference(program, machine, state, binary.right);
    validation::has_builtin_or_sealed_float_comparison_meaning(
        program,
        machine.symbol,
        expression,
        binary.operator,
        &[left, right],
    )
}

fn builtin_binary_meaning(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return false;
    };
    let state = validation::find_state_in_machine(program, machine.symbol, state_symbol);
    validation::has_builtin_binary_expression_meaning(program, machine, state, expression)
}

/*
    Provenance may retain builtin collection selection only after source
    resolution authenticates this occurrence in its actual state namespace.
*/
fn builtin_index_meaning(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return false;
    };
    let Some(state) = validation::find_state_in_machine(program, machine.symbol, state_symbol)
    else {
        return false;
    };
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), expression)
}

/*
    A mutable reference can expose an entry observation of stable referent
    contents only while the selected storage remains unwritten. Nested loans
    and interior mutation still fail the ordinary contents classifier.
*/
fn observation_requires_write_scan(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    field_path: &[PlaceSegment],
) -> Option<bool> {
    if field_path.is_empty()
        && program
            .primitive_type_reference(
                program
                    .type_reference_table
                    .peeled_type_reference(type_reference),
            )
            .is_some_and(|primitive| primitive.accepts_float_literal())
    {
        return Some(false);
    }
    let stable = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable,
            referee,
            ..
        } => has_contents(
            program,
            *referee,
            &[],
            validation::ContentsRequirement::StableObservation,
        )
        .then_some(true),
        _ => has_contents(
            program,
            type_reference,
            &[],
            validation::ContentsRequirement::StableObservation,
        )
        .then_some(false),
    };
    if stable.is_some() || field_path.is_empty() {
        return stable;
    }
    places::projection_requires_write_scan(program, type_reference, field_path)
}

/*
    A state parameter's saved actual is whatever every arrival binds to it:
    the invocation for the entry state, and each named transition edge into
    the state positionally. An edge forwarding the same immutable parameter
    back to its own state is tautological; a mutable parameter's storage must
    still be pristine at the edge. Every remaining edge must resolve to one identical
    entry-relative operand, or provenance stays unknown rather than picking a
    winner. For a mutable parameter the read itself and every self-referential
    edge must keep the projected `field_path` pristine — the produced `Member`
    operand asserts only that projection is uniform across arrivals, never
    that the whole bound record is.
*/
fn state_parameter_entry_operand(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    parameter_symbol: SymbolHandle,
    _before_statement: usize,
    write_scan_end: usize,
    field_path: &[PlaceSegment],
    depth: u32,
) -> Option<CrashPredicateExpression> {
    let states = program.machine_states(machine);
    let state_index = program.state_position_by_symbol(machine.symbol, state_symbol)?;
    let state = &states[state_index];
    let parameters = program.state_parameters(state);
    let (parameter_ordinal, parameter) = parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == parameter_symbol)?;
    let entry_index = validation::activation_target_state_index(program, machine, machine.symbol)?;

    let mutable_storage =
        observation_requires_write_scan(program, parameter.type_reference, field_path)?;
    let mutable = parameter.is_mutable || mutable_storage;
    if mutable
        && !storage_holds_bound_value(
            program,
            machine_symbol,
            state,
            0,
            write_scan_end,
            parameter_symbol,
            field_path,
        )
    {
        return None;
    }
    let argument_index = parameters[..parameter_ordinal].len();
    let mut provenance = if state_index == entry_index {
        Some(CrashPredicateExpression::Parameter(
            u32::try_from(parameter_ordinal).ok()?,
        ))
    } else {
        None
    };
    for (source_symbol, statement_ordinal, argument) in
        named_transition_arguments(program, machine, state_index, argument_index)
    {
        if source_symbol == state_symbol
            && program.expression_table.expression_is_valid(argument)
            && let ExpressionNode::Name(forwarded) = program.expression_table.expression(argument)
            && forwarded.symbol == parameter_symbol
            && forwarded.head_symbol == parameter_symbol
            && expressions(program)
                .name_path_members(forwarded.members)
                .len()
                == 1
        {
            if mutable
                && !storage_holds_bound_value(
                    program,
                    machine_symbol,
                    state,
                    0,
                    statement_ordinal.saturating_add(1),
                    parameter_symbol,
                    field_path,
                )
            {
                return None;
            }
            continue;
        }
        let resolved = entry_operand_projected_at(
            program,
            machine_symbol,
            source_symbol,
            statement_ordinal,
            argument,
            field_path,
            depth + 1,
        )?;
        if let Some(existing) = provenance.as_ref() {
            if *existing != resolved {
                return None;
            }
        } else {
            provenance = Some(resolved);
        }
    }
    provenance
}

/*
    Positional arguments of every named transition edge into `state_index`,
    paired with the source state and the transition's own statement ordinal.
    Typed lowering flattens nested transition forms into statements; inspect
    ordinary targets and continuation targets, just as the termination graph
    does. Exits are not named arrivals; an edge short an
    argument contributes an invalid handle that fails resolution above.
*/
fn named_transition_arguments(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_index: usize,
    argument_index: usize,
) -> Vec<(SymbolHandle, usize, ExpressionHandle)> {
    let mut incoming = Vec::new();
    for source in program.machine_states(machine) {
        for (ordinal, statement) in crate::flow::state_statements(program, source.statement_nodes)
            .iter()
            .enumerate()
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            for target in transition.targets() {
                let TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
                else {
                    continue;
                };
                if validation::activation_target_state_index(program, machine, path.symbol)
                    != Some(state_index)
                {
                    continue;
                }
                incoming.push((
                    source.symbol,
                    ordinal,
                    program
                        .statement_table
                        .expression_handles(*arguments)
                        .get(argument_index)
                        .copied()
                        .unwrap_or_else(ExpressionHandle::invalid),
                ));
            }
        }
    }
    incoming
}

/*
    Entry substitution under whole-operand provenance: a formal `Parameter`
    resolves to the caller's entire argument, whatever member spine the guard
    applies above it. Literals and owner-scope names need no operand, and
    `Opaque`/`ContentConservation`/`Invalid` refuse — the transport rules this
    shares with the projected resolver stay in one walk
    (`substitute_entry_projected`), so this is only the operand lookup.
*/
pub(super) fn substitute_entry(
    expression: &CrashPredicateExpression,
    operands: &[Option<CrashPredicateExpression>],
) -> Option<CrashPredicateExpression> {
    substitute_entry_projected(expression, &mut |ordinal: u32, _members: &[String]| {
        operands.get(ordinal as usize)?.clone()
    })
}

/*
    Entry substitution with per-field operand provenance. `resolve(ordinal,
    members)` answers the operand's entry operand restricted to the member
    projection `members` — a contiguous `Member` spine above a `Parameter`
    leaf applies the authored projection to the operand's storage, so a
    binding whose unread sibling fields were written still supplies the read
    field's saved actual rather than widening the route to `Truth`. The
    `Member` spine re-applies its own names over the resolved root, so the
    resolver returns the operand's entry operand unprojected by `members`.
    Parameter-free nodes need no operand and transport under the same rule
    `substitute_entry` uses; `Opaque`, `ContentConservation` and a bare
    `Invalid` keep the same refusal here.
*/
pub(super) fn substitute_entry_projected(
    expression: &CrashPredicateExpression,
    resolve: &mut (impl FnMut(u32, &[String]) -> Option<CrashPredicateExpression> + ?Sized),
) -> Option<CrashPredicateExpression> {
    Some(match expression {
        CrashPredicateExpression::Parameter(ordinal) => resolve(*ordinal, &[])?,
        CrashPredicateExpression::Member { .. } => {
            let mut members = Vec::new();
            let mut base = expression;
            while let CrashPredicateExpression::Member { receiver, member } = base {
                members.push(member.clone());
                base = receiver;
            }
            members.reverse();
            let base = match base {
                CrashPredicateExpression::Parameter(ordinal) => {
                    resolve(*ordinal, members.as_slice())?
                }
                _ => substitute_entry_projected(base, resolve)?,
            };
            members
                .iter()
                .fold(base, |receiver, member| CrashPredicateExpression::Member {
                    receiver: Box::new(receiver),
                    member: member.clone(),
                })
        }
        CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Float(_)
        | CrashPredicateExpression::Name(_) => expression.clone(),
        CrashPredicateExpression::StructuralIdentity {
            type_commitment,
            left,
            right,
        } => {
            let mut left = substitute_entry_projected(left, resolve)?;
            let mut right = substitute_entry_projected(right, resolve)?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            CrashPredicateExpression::StructuralIdentity {
                type_commitment: *type_commitment,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => CrashPredicateExpression::Binary {
            operator: *operator,
            left: Box::new(substitute_entry_projected(left, resolve)?),
            right: Box::new(substitute_entry_projected(right, resolve)?),
        },
        CrashPredicateExpression::Unary { operator, operand } => CrashPredicateExpression::Unary {
            operator: *operator,
            operand: Box::new(substitute_entry_projected(operand, resolve)?),
        },
        CrashPredicateExpression::IntegerWiden {
            source_type,
            target_type,
            domain,
            operand,
        } => CrashPredicateExpression::IntegerWiden {
            source_type: *source_type,
            target_type: *target_type,
            domain: *domain,
            operand: Box::new(substitute_entry_projected(operand, resolve)?),
        },
        CrashPredicateExpression::RepresentationView {
            source_type,
            target_type,
            operand,
        } => CrashPredicateExpression::RepresentationView {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(substitute_entry_projected(operand, resolve)?),
        },
        CrashPredicateExpression::Indexed { collection, index } => {
            CrashPredicateExpression::Indexed {
                collection: Box::new(substitute_entry_projected(collection, resolve)?),
                index: Box::new(substitute_entry_projected(index, resolve)?),
            }
        }
        CrashPredicateExpression::Range {
            start,
            end,
            end_inclusive,
        } => CrashPredicateExpression::Range {
            start: Box::new(substitute_entry_projected(start, resolve)?),
            end: Box::new(substitute_entry_projected(end, resolve)?),
            end_inclusive: *end_inclusive,
        },
        CrashPredicateExpression::Call {
            target,
            receiver,
            arguments,
        } => CrashPredicateExpression::Call {
            target: target.clone(),
            receiver: Box::new(match receiver.as_ref() {
                CrashPredicateExpression::Invalid => CrashPredicateExpression::Invalid,
                receiver => substitute_entry_projected(receiver, resolve)?,
            }),
            arguments: arguments
                .iter()
                .map(|argument| substitute_entry_projected(argument, resolve))
                .collect::<Option<Vec<_>>>()?,
        },
        _ => return None,
    })
}

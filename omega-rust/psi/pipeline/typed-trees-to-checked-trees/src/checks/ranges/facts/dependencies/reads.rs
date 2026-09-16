use super::{ExpressionHandle, ExpressionNode, Machine, State, TypedTrees};
use crate::checks::ranges::facts::RangeCallContext;
use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_expression_in_state;
use crate::semantic_calls::CallSite;
use checked_trees::{
    CheckedOperatorFacts, CheckedOperatorResolutionStatus, CheckedValueOrigin,
    CheckedValueStatementRole,
};
use symbols::SymbolHandle;

pub(super) fn collect_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        // Literal leaves read no caller storage: their bytes are immutable
        // compile-time content, not a place a write could mutate.
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::Binary(binary) => {
            collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                calls,
                operators,
                reads,
                depth + 1,
            ) && collect_reads(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                calls,
                operators,
                reads,
                depth + 1,
            )
        }
        ExpressionNode::Unary(unary) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            unary.operand,
            calls,
            operators,
            reads,
            depth + 1,
        ),
        ExpressionNode::Cast(cast) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            cast.value,
            calls,
            operators,
            reads,
            depth + 1,
        ),
        // A borrow's value is a reference into its target place, so its
        // footprint is exactly the target's reads. For a call operand this is
        // also the storage the callee was handed; the checked access rows
        // record the same place once more and dedup absorbs it.
        ExpressionNode::Borrow(inner) => collect_reads(
            program,
            machine,
            state,
            statement_index,
            inner.target,
            calls,
            operators,
            reads,
            depth + 1,
        ),
        // A call's value is not a pure function of its argument expressions:
        // the callee may also read receiver `self` storage. Admit a footprint
        // only for the exact checked call occurrence at this statement — the
        // same flow/borrow join `structured_call_writes` uses — and only when
        // the typed call carries no machine, requirement, quotient, or
        // private-layout binder that could hand the callee storage the
        // operand scan cannot see. The checked operand accesses then
        // enumerate every caller place reachable through the arguments,
        // while a `self` target adds the canonical receiver place so an
        // implicit-self callee still reads the caller's machine storage.
        // Targets that do not resolve to a state (requirement symbols,
        // boundary signatures, intrinsics) keep the footprint incomplete.
        ExpressionNode::Call(call) => {
            let Some(calls) = calls else {
                return false;
            };
            let site = CallSite::Expression { expression, call };
            let Some(borrow_call) =
                calls.find_call(program, machine, state, statement_index, &site)
            else {
                return false;
            };
            if call.static_machine_parameter.is_valid()
                || call.static_requirement_dispatch.is_some()
                || !call.machine_arguments.is_empty()
                || call.quotient_operation.is_some()
                || call.private_layout_operation.is_some()
            {
                return false;
            }
            if call.receiver.is_valid()
                && !collect_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    call.receiver,
                    Some(calls),
                    operators,
                    reads,
                    depth + 1,
                )
            {
                return false;
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                if !collect_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    *argument,
                    Some(calls),
                    operators,
                    reads,
                    depth + 1,
                ) {
                    return false;
                }
            }
            for mut place in calls.operand_access_places(borrow_call) {
                if validate_place_read(program, machine, state, statement_index, &mut place)
                    .is_none()
                {
                    return false;
                }
                if !reads.contains(&place) {
                    reads.push(place);
                }
            }
            // Named calls may target the machine symbol rather than a state
            // symbol; the contract lookup resolves both forms to the exact
            // invocation while requirement, trait-signature, and conformance
            // targets resolve to symbols that own no state and stay opaque.
            let Some((target_machine_symbol, target_state_symbol)) =
                crate::proof::contract_target_from_state_symbol(program, borrow_call.target_symbol)
            else {
                return false;
            };
            let Some(target_state) = crate::semantic_calls::find_state_in_machine(
                program,
                target_machine_symbol,
                target_state_symbol,
            ) else {
                return false;
            };
            if program
                .state_parameters(target_state)
                .iter()
                .any(|parameter| parameter.is_self)
            {
                let Some(mut place) = crate::flow::canonical_receiver_place_for_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    &site,
                ) else {
                    return false;
                };
                if validate_place_read(program, machine, state, statement_index, &mut place)
                    .is_none()
                {
                    return false;
                }
                if !reads.contains(&place) {
                    reads.push(place);
                }
            }
            true
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => collect_place_read(
            program,
            machine,
            state,
            statement_index,
            expression,
            calls,
            operators,
            reads,
            depth,
        ),
        // Builtin `items[i]`/`items[a..b]` syntax projects element or window
        // storage and keeps the ordinary place footprint. Once an authored
        // `[]`/`[..]` declaration governs the occurrence the application is
        // call-shaped instead: its complete footprint is the operand reads
        // authenticated by the exact checked use row, so a missing or
        // unstable selection stays incomplete rather than pretending to be
        // element storage.
        ExpressionNode::Indexed(indexed) => {
            if has_builtin_index_meaning(
                program,
                machine,
                state,
                statement_index,
                expression,
                indexed,
            ) {
                collect_place_read(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    calls,
                    operators,
                    reads,
                    depth,
                )
            } else {
                collect_selected_index_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    indexed,
                    calls,
                    operators,
                    reads,
                    depth,
                )
            }
        }
        // `a..b`, `a..`, `..b`, and `..` evaluate only their present bounds.
        // An omitted open bound reads nothing; a present bound is an operand
        // position that must keep builtin bound meaning, exactly like the
        // selector scan requires of a point index.
        ExpressionNode::Range(range) => {
            (!range.start.is_valid()
                || collect_operand_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    range.start,
                    calls,
                    operators,
                    reads,
                    depth,
                ))
                && (!range.end.is_valid()
                    || collect_operand_reads(
                        program,
                        machine,
                        state,
                        statement_index,
                        range.end,
                        calls,
                        operators,
                        reads,
                        depth,
                    ))
        }
        // Every array literal element is evaluated at the literal site.
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .all(|element| {
                collect_operand_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    *element,
                    calls,
                    operators,
                    reads,
                    depth,
                )
            }),
        // Authored and synthesized erased-field initializers alike are
        // evaluated where the literal appears.
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| {
                collect_operand_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    field.value,
                    calls,
                    operators,
                    reads,
                    depth,
                )
            }),
        // A match reads its subject, every pattern compared against it, and
        // the selected arm's value. Unioning all arms is the conservative
        // footprint: an unselected arm can only shrink the storage the
        // result depends on, never grow it.
        ExpressionNode::Match(dispatch) => {
            collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                dispatch.subject,
                calls,
                operators,
                reads,
                depth,
            ) && program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .all(|arm| {
                    (match arm.pattern {
                        typed_trees::expression::MatchPattern::Value(pattern) => {
                            collect_operand_reads(
                                program,
                                machine,
                                state,
                                statement_index,
                                pattern,
                                calls,
                                operators,
                                reads,
                                depth,
                            )
                        }
                        typed_trees::expression::MatchPattern::Wildcard => true,
                    }) && collect_operand_reads(
                        program,
                        machine,
                        state,
                        statement_index,
                        arm.value,
                        calls,
                        operators,
                        reads,
                        depth,
                    )
                })
        }
        // `place.load(ordering)` is the one atomic observation whose complete
        // footprint is exactly its resident place: the desugar keeps that
        // place in `value` and leaves `result` empty. Every writing axis
        // wraps a stored operand or an instruction-shaped update in `value`,
        // which no operand scan can describe as reads. The ordering plan,
        // custody agreement, and empty result are rechecked so a writing
        // operation cannot borrow the load's place-shaped footprint.
        ExpressionNode::Atomic(atomic) => {
            let footprint_start = reads.len();
            matches!(
                atomic.ordering,
                language_core::atomic::AtomicOrderingPlan::Load(ordering)
                    if ordering.valid_for_load()
            ) && atomic.result_custody.is_valid_for(atomic.ordering)
                && !atomic.result_custody.requires_result_destination()
                && !atomic.result.is_valid()
                && collect_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    atomic.value,
                    calls,
                    operators,
                    reads,
                    depth + 1,
                )
                && reads.len() > footprint_start
        }
    }
}

/// A nested operand's reads count only when that operand subtree keeps
/// builtin bound meaning — the same floor `record_dependencies` applies to
/// the top-level expression — and its own read scan completes. The general
/// bound-meaning walk does not descend through the compound nodes handled
/// above, so each operand under them has to carry the floor independently:
/// an authored operator inside a window bound or a literal element stays
/// incomplete rather than pretending only its visible places were read.
fn collect_operand_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    operand: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    validation::has_builtin_bound_expression_meaning(program, machine, Some(state), operand)
        && collect_reads(
            program,
            machine,
            state,
            statement_index,
            operand,
            calls,
            operators,
            reads,
            depth + 1,
        )
}

/// A read place counts only when its root is current storage and every
/// segment carries resolved selector geometry. Normalization runs before the
/// current-root check so attached fields answer through the machine root.
/// The returned symbol is the pre-normalization root for callers that
/// distinguish a place's own root from a rebased copy identity.
fn validate_place_read(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    place: &mut CanonicalPlace,
) -> Option<SymbolHandle> {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    if !root.is_valid()
        || place
            .segments
            .iter()
            .any(|segment| crate::flow::place_segment_has_unresolved_identity(*segment))
    {
        return None;
    }
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, place);
    (root_is_current(program, machine, state, statement_index, place.root)
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::Index { .. }
            )
        }))
    .then_some(root)
}

pub(super) fn root_is_current(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    root: facts::PlaceRoot,
) -> bool {
    root == facts::PlaceRoot::Symbol(machine.symbol)
        || program.state_parameters(state).iter().any(|parameter| {
            root == facts::PlaceRoot::Symbol(parameter.symbol)
        })
        || program.statement_table.statements(state.statement_nodes).iter().take(statement_index).any(|statement| {
            matches!(statement, typed_trees::statement::StatementNode::LocalData(local) if root == facts::PlaceRoot::Symbol(local.symbol))
        })
}

/// A name, member, or builtin-indexed expression contributes its own storage
/// place plus the selector reads needed to address it.
fn collect_place_read(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if !collect_selector_reads(
        program,
        machine,
        state,
        statement_index,
        expression,
        calls,
        operators,
        reads,
        depth + 1,
    ) {
        return false;
    }
    let Some(mut place) = canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };
    let Some(root) = validate_place_read(program, machine, state, statement_index, &mut place)
    else {
        return false;
    };
    // Immutable integer copies read the same frozen value, not their
    // initializer's current storage. Preserve that existing identity
    // through copy chains without giving references snapshot semantics.
    if place.segments.is_empty()
        && let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && path.symbol == root
        && path.head_symbol == root
        && super::captures::is_integer_value(program, machine, state, expression)
        && let Some(value) = super::captures::integer_value_identity(program, state, expression)
        && root_is_current(
            program,
            machine,
            state,
            statement_index,
            facts::PlaceRoot::Symbol(value),
        )
    {
        place.root = facts::PlaceRoot::Symbol(value);
    }
    if !reads.contains(&place) {
        reads.push(place);
    }
    true
}

/// A selected `[]`/`[..]` application is a checked occurrence, not a place:
/// the exact `CheckedOperatorUseFact` at this statement authenticates which
/// declaration the occurrence resolved to, and the operand expressions
/// recovered from that row enumerate the caller storage the callee can
/// observe. Missing, ambiguous, or inconsistent selection custody leaves the
/// read set incomplete — the same evidence floor the checked-call join
/// applies. Each operand recurses through the ordinary read scan, so a
/// nested call or a selected nested indexing still has to prove its own
/// complete footprint.
fn collect_selected_index_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    indexed: &typed_trees::expression::TableIndexedExpression,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    use language_core::OperatorSpelling;

    let Some(operators) = operators else {
        return false;
    };
    // Use rows carry the enclosing statement's origin even for nested
    // operands, so this join covers selector and subexpression positions.
    // Custody must agree across every recorded row for this occurrence.
    let mut uses = operators.uses.iter().filter_map(|(_, selected)| {
        (selected.expression == expression
            && matches!(
                selected.origin,
                CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: index,
                    ..
                } if machine_symbol == machine.symbol
                    && state_symbol == state.symbol
                    && index == statement_index
            ))
        .then_some(selected)
    });
    let Some(selected) = uses.next() else {
        return false;
    };
    let spelling = if matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    };
    if selected.spelling != spelling
        || selected.status != CheckedOperatorResolutionStatus::Resolved
        || !selected.selected_operator_symbol.is_valid()
        || selected.candidate_count != operators.candidates(selected).len()
        || uses.any(|other| other != selected)
    {
        return false;
    }
    let Some(candidate) = operators.selected_candidate(selected) else {
        return false;
    };
    // The operand expressions recovered from the checked row are the only
    // caller storage the selected declaration can observe. Their count still
    // has to match the retained signature so a drifted row cannot rename
    // storage the operand scan never saw.
    let Some(operands) = selected.operands(program) else {
        return false;
    };
    if candidate.parameter_count != operands.len() {
        return false;
    }
    operands.iter().all(|operand| {
        collect_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            Some(operators),
            reads,
            depth + 1,
        )
    })
}

/// Check typed identities before contextual spelling recovery, and collect the
/// storage read to select this place. Do not read the whole collection merely
/// to address one element: parent replacement overlaps its child path already.
fn collect_selector_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            crate::lookup::first_valid_name_path_symbol(path, &program.expression_table).is_some()
                && (members.len() <= 1 || members.len() == symbols.len())
                && symbols.iter().all(|symbol| symbol.is_valid())
        }
        ExpressionNode::Member(member) => {
            member.member_symbol.is_valid()
                && collect_selector_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    member.receiver,
                    calls,
                    operators,
                    reads,
                    depth + 1,
                )
        }
        ExpressionNode::Indexed(indexed) => {
            // The general bound-meaning query treats places as symbolic leaves.
            // Inspect each selector explicitly before syntax-based constant
            // normalization may establish distinct element coordinates.
            has_builtin_index_meaning(
                program,
                machine,
                state,
                statement_index,
                expression,
                indexed,
            ) && validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                indexed.index,
            ) && collect_selector_reads(
                program,
                machine,
                state,
                statement_index,
                indexed.collection,
                calls,
                operators,
                reads,
                depth + 1,
            ) && collect_reads(
                program,
                machine,
                state,
                statement_index,
                indexed.index,
                calls,
                operators,
                reads,
                depth + 1,
            )
        }
        _ => false,
    }
}

fn has_builtin_index_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    indexed: &typed_trees::expression::TableIndexedExpression,
) -> bool {
    use crate::checks::ranges::types::expression_type_reference;
    use language_core::OperatorSpelling;

    // The occurrence's spelling follows the index shape production recorded:
    // a range index spells `[..]` and resolves against the decomposed
    // collection/start/end operand tuple, not a two-operand `[]` search that
    // cannot see a declared window operator (or lets a scalar `[]`
    // declaration masquerade as governing window syntax).
    let spelling = if indexed.index.is_valid()
        && matches!(
            program.expression_table.expression(indexed.index),
            ExpressionNode::Range(_)
        ) {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    };
    let operands = crate::operators::indexed_operand_types(
        program,
        indexed,
        CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: CheckedValueStatementRole::Expression,
        },
    );
    // Successful builtin projection requires array/slice storage geometry; a
    // nominal collection's authored index operation is not a primitive read.
    // A window's result is a slice view rather than an element projection,
    // so `[..]` judges the collection shell itself.
    let builtin_storage = if spelling == OperatorSpelling::Range {
        expression_type_reference(program, machine, state, indexed.collection)
            .and_then(|collection| validation::unwrapped_type_reference(program, collection))
            .is_some_and(|collection| {
                matches!(
                    program.type_reference_table.type_reference(collection),
                    typed_trees::types::TypeReferenceNode::FixedArray { .. }
                        | typed_trees::types::TypeReferenceNode::Slice { .. }
                )
            })
    } else {
        expression_type_reference(program, machine, state, expression).is_some()
    };
    builtin_storage
        && typed_trees::operator::resolve_indexed_spelling_for_operands(
            program, spelling, &operands,
        )
        .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &operands,
        )
}

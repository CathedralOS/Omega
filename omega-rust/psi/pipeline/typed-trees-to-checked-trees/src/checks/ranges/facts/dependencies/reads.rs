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
        // place in `value` and leaves `result` empty. Every writing axis is
        // instead an assignment carrier (`target = Atomic { .. }`) whose
        // `value` wraps a stored operand or an instruction-shaped update, so
        // the resident place is the carrier statement's `target`, not a child
        // of `value`. A writing axis is admitted only with its complete
        // footprint: the carrier at this statement supplying the resident
        // place, every stored-operand read, and — for the axes that observe a
        // prior — a `result` destination naming current local storage whose
        // symbol the update model reuses as its prior placeholder. The
        // ordering plan, custody agreement, and result shape are rechecked
        // per axis so a writing operation cannot borrow the load's
        // place-shaped footprint, a store of unproven operand reads stays
        // incomplete, and a compare-exchange whose failure path does not
        // reduce to the scalar prior model (the single-attempt observing
        // form, an illegal failure ordering, or a substituted update) never
        // claims a footprint either.
        ExpressionNode::Atomic(atomic) => {
            let footprint_start = reads.len();
            match atomic.ordering {
                language_core::atomic::AtomicOrderingPlan::Load(ordering) => {
                    ordering.valid_for_load()
                        && atomic.result_custody.is_valid_for(atomic.ordering)
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
                _ => collect_atomic_write_reads(
                    program,
                    machine,
                    state,
                    statement_index,
                    expression,
                    atomic,
                    calls,
                    operators,
                    reads,
                    depth,
                ),
            }
        }
    }
}

/// The footprint of a writing atomic (`store`, `swap`, fetch, or decisive
/// compare-exchange). All four exist only as an assignment carrier: the
/// parser desugars `place.store(v, ord)` and the `let r = place.op(..)`
/// forms into `target = Atomic { .. }`, so the resident place is that
/// carrier statement's `target`, never a child of `value`. The admitted
/// read set is every storage position the carrier names: the resident
/// place, the operand expressions the instruction consults besides the
/// resident place (`value` for store/swap, the fetch operand for
/// read-modify-write, the expected and replacement operands for a decisive
/// compare-exchange whose `value` is the exact prior-shaped update model),
/// and the `result` destination the carrier materializes into — a slot the
/// carrier owns, so a write reaching it can never slip past the recorded
/// label. The model's prior positions are placeholders for the
/// instruction-observed pre-write value; they are pinned to name the same
/// symbol `result` receives and are never scanned as local reads. Any
/// missing carrier, illegal ordering plan, non-scalar custody,
/// missing/substituted result destination, or unrecognized update shape
/// leaves the footprint incomplete.
fn collect_atomic_write_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    atomic: &typed_trees::expression::TableAtomicExpression,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    use language_core::atomic::AtomicOrderingPlan;
    // Canonical scalar custody is the only result form a scalar read set can
    // describe; the observing single-attempt custody names an outcome carrier
    // whose failure path may be an uncommitted attempt, never the resident
    // place's prior value.
    if !atomic.result_custody.is_valid_for(atomic.ordering)
        || atomic.result_custody.requires_result_destination()
    {
        return false;
    }
    let Some(resident) = atomic_carrier_target(program, state, statement_index, expression) else {
        return false;
    };
    let (operands, result_symbol): (Vec<ExpressionHandle>, Option<SymbolHandle>) = match atomic
        .ordering
    {
        AtomicOrderingPlan::Load(_) => return false,
        // `place.store(v, ordering)` reads the resident place and the
        // stored operand `v`; a store that also names a result is not the
        // desugar's shape.
        AtomicOrderingPlan::Store(ordering) => {
            if !ordering.valid_for_store() || atomic.result.is_valid() {
                return false;
            }
            (vec![atomic.value], None)
        }
        // `let r = place.swap(v, ordering)` reads the displaced prior into
        // `r` and writes `v`; `value` is the replacement operand.
        AtomicOrderingPlan::Swap(_) => {
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            (vec![atomic.value], Some(result_symbol))
        }
        // `let r = place.fetch_op(d, ordering)` models `prior OP d`. The
        // model's left operand must be the observed-prior placeholder
        // naming `r`; `d` is the instruction's only other operand read.
        AtomicOrderingPlan::ReadModifyWrite(_) => {
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            let Some(operand) = atomic_fetch_operand(
                program,
                machine,
                state,
                statement_index,
                atomic,
                result_symbol,
            ) else {
                return false;
            };
            (vec![operand], Some(result_symbol))
        }
        // A decisive compare-exchange reads the resident prior plus the
        // expected and replacement operands. Its `value` must be the
        // desugar's exact `prior + (prior == expected) * (replacement -
        // prior)` update model — anything else, including the
        // single-attempt observing form (whose uncommitted-attempt
        // failure path has no scalar prior), has no complete scalar
        // footprint.
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            if !failure.valid_compare_exchange_failure(success) {
                return false;
            }
            let Some(result_symbol) =
                atomic_local_name_symbol(program, machine, state, statement_index, atomic.result)
            else {
                return false;
            };
            let Some(operands) = atomic_compare_exchange_operands(
                program,
                machine,
                state,
                statement_index,
                atomic,
                result_symbol,
            ) else {
                return false;
            };
            (operands.to_vec(), Some(result_symbol))
        }
        AtomicOrderingPlan::CompareExchangeOnce { .. } => return false,
    };
    collect_place_read(
        program,
        machine,
        state,
        statement_index,
        resident,
        calls,
        operators,
        reads,
        depth + 1,
    ) && operands.iter().all(|operand| {
        collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            operators,
            reads,
            depth,
        )
    }) && result_symbol.is_none_or(|symbol| {
        // The result destination is a slot the carrier owns: join its exact
        // local place so no write reaching it slips past the recorded label.
        let place = CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        };
        if !reads.contains(&place) {
            reads.push(place);
        }
        true
    })
}

/// The resident place of a writing atomic is its carrier assignment's
/// target. An atomic not carried by the exact assignment at this statement
/// index has no provable resident place.
fn atomic_carrier_target(
    program: &TypedTrees,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let typed_trees::statement::StatementNode::Assignment(assignment) = statement else {
        return None;
    };
    (assignment.value == expression).then_some(assignment.target)
}

/// A single-member name's storage identity in this state: the resolved
/// symbol when the path carries one, else the nearest current local binding
/// the member text — the same name binding the desugar's generated result
/// and placeholder names receive, since those never carry resolved symbols.
/// The root must still be current storage at this statement; a member path,
/// an unresolved name, or a name bound only later is not a destination.
fn atomic_local_name_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if path.members.count() != 1 {
        return None;
    }
    let symbol = crate::lookup::first_valid_name_path_symbol(path, &program.expression_table)
        .or_else(|| {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .first()?;
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(statement_index)
                .rev()
                .find_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.name == *name =>
                    {
                        Some(local.symbol)
                    }
                    _ => None,
                })
        })?;
    root_is_current(
        program,
        machine,
        state,
        statement_index,
        facts::PlaceRoot::Symbol(symbol),
    )
    .then_some(symbol)
}

/// `expression` is the observed-prior placeholder: a single-member name
/// bound to the result destination's symbol, not an independent local read.
fn atomic_prior_placeholder(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    result_symbol: SymbolHandle,
) -> Option<()> {
    (atomic_local_name_symbol(program, machine, state, statement_index, expression)
        == Some(result_symbol))
    .then_some(())
}

/// The fetch model is `prior OP operand`; its left operand is the
/// observed-prior placeholder (naming the result destination, never scanned
/// as a read) and its right operand is the authored operand `d`. Operators
/// outside the sealed fetch family are not a stable instruction shape.
fn atomic_fetch_operand(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    atomic: &typed_trees::expression::TableAtomicExpression,
    result_symbol: SymbolHandle,
) -> Option<ExpressionHandle> {
    use typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(atomic.value) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::BitwiseXor
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseAnd
    ) {
        return None;
    }
    atomic_prior_placeholder(
        program,
        machine,
        state,
        statement_index,
        binary.left,
        result_symbol,
    )?;
    Some(binary.right)
}

/// The decisive compare-exchange model is
/// `prior + (prior == expected) * (replacement - prior)`: the resident prior
/// is read through three placeholder positions (all pinned to the result
/// symbol) and the remaining reads are exactly the expected and replacement
/// operands. Any other update shape has no complete scalar footprint.
fn atomic_compare_exchange_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    atomic: &typed_trees::expression::TableAtomicExpression,
    result_symbol: SymbolHandle,
) -> Option<[ExpressionHandle; 2]> {
    use typed_trees::expression::BinaryOperator;
    let ExpressionNode::Binary(sum) = program.expression_table.expression(atomic.value) else {
        return None;
    };
    if sum.operator != BinaryOperator::Add {
        return None;
    }
    atomic_prior_placeholder(
        program,
        machine,
        state,
        statement_index,
        sum.left,
        result_symbol,
    )?;
    let ExpressionNode::Binary(product) = program.expression_table.expression(sum.right) else {
        return None;
    };
    if product.operator != BinaryOperator::Multiply {
        return None;
    }
    let ExpressionNode::Binary(equal) = program.expression_table.expression(product.left) else {
        return None;
    };
    if equal.operator != BinaryOperator::Equal {
        return None;
    }
    atomic_prior_placeholder(
        program,
        machine,
        state,
        statement_index,
        equal.left,
        result_symbol,
    )?;
    let ExpressionNode::Binary(difference) = program.expression_table.expression(product.right)
    else {
        return None;
    };
    if difference.operator != BinaryOperator::Subtract {
        return None;
    }
    atomic_prior_placeholder(
        program,
        machine,
        state,
        statement_index,
        difference.right,
        result_symbol,
    )?;
    Some([equal.right, difference.left])
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
            // The typed member binder only covers `Name`/`Member`/`Indexed`/
            // `StructLiteral` receivers, so `(&x).f` reaches this scan with
            // `member_symbol` still unset — there the honest identity floor is
            // `effective_member_symbol`, the same contextual resolution the
            // canonical-place production stamps into the `Field` segment. A
            // member on a receiver the binder did walk keeps the authored
            // row's own identity: a symbol it saw but left unresolved cannot
            // be recovered from the receiver's spelling.
            (member.member_symbol.is_valid()
                || (matches!(
                    program.expression_table.expression(member.receiver),
                    ExpressionNode::Borrow(_)
                ) && crate::flow::effective_member_symbol(program, member.receiver, member)
                    .is_valid()))
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
        // An explicit `&`/`&mut` inside a place chain addresses exactly its
        // target's place: `(&x).f` reads `x.f`, and both canonical-place
        // productions already peel the borrow the same way. The peel admits
        // no new storage — the target's own selector scan still has to clear
        // the same floor, so `(&call()).f` or a borrow of foreign storage
        // stays incomplete.
        ExpressionNode::Borrow(inner) => collect_selector_reads(
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

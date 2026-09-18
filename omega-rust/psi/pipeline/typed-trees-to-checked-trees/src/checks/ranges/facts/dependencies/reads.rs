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
        // the typed call carries no machine-valued, requirement, quotient, or
        // private-layout binder that could hand the callee storage the
        // operand scan cannot see. A static type or const application is not
        // such a binder: it substitutes a declaration identity or a
        // compile-time value, so the applied call reads exactly what the same
        // call without the application reads. The checked operand accesses then
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
                || !static_application_carries_no_caller_storage(program, call)
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
                    statement_index,
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
        ExpressionNode::Name(_) => collect_place_read(
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
        ExpressionNode::Member(member) => collect_member_reads(
            program,
            machine,
            state,
            statement_index,
            expression,
            member,
            calls,
            operators,
            reads,
            depth,
        ),
        // Builtin `items[i]`/`items[a..b]` syntax projects element or window
        // storage and keeps the ordinary place footprint while the chain
        // bottoms out at caller storage. A collection that produces a
        // temporary instead — `compute(seed).a[i]`, `Pair { .. }.a[i]`,
        // `compute()[i]` — owns no element place at all, so its complete
        // footprint is whatever producing that collection read plus the
        // selector operand reads under the same builtin bound floor the
        // place scan applies. Once an authored `[]`/`[..]` declaration
        // governs the occurrence the application is call-shaped instead:
        // its complete footprint is the operand reads authenticated by the
        // exact checked use row, so a missing or unstable selection stays
        // incomplete rather than pretending to be element storage.
        ExpressionNode::Indexed(indexed) => {
            if has_builtin_index_meaning(
                program,
                machine,
                state,
                statement_index,
                expression,
                indexed,
            ) {
                // Place-rootedness is hereditary: the canonical root comes
                // from the leftmost leaf of the selector chain, so the whole
                // expression's root decides which footprint applies — the
                // same split `collect_member_reads` makes for temporary
                // receivers.
                if canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    expression,
                )
                .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Symbol(_)))
                {
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
                    collect_reads(
                        program,
                        machine,
                        state,
                        statement_index,
                        indexed.collection,
                        calls,
                        operators,
                        reads,
                        depth + 1,
                    ) && collect_operand_reads(
                        program,
                        machine,
                        state,
                        statement_index,
                        indexed.index,
                        calls,
                        operators,
                        reads,
                        depth,
                    )
                }
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

/// Whether a call's static application substitutes only storage-free
/// selections, so the checked operand accesses still enumerate the complete
/// caller footprint. A type argument names a declaration identity and a
/// const argument names a compile-time value; neither is a place, and
/// specialization "creates no runtime dictionary", so a statically applied
/// call reads exactly what the same call without the application reads. A
/// machine-valued argument is different in kind: it hands the callee a
/// callable body this occurrence never authenticated, whose own reads no
/// operand scan here describes. A nested static application or an evidence
/// projection can carry such a binder below the argument this scan sees, so
/// both stay unproven rather than being walked for a machine leaf.
fn static_application_carries_no_caller_storage(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    call.machine_arguments.iter().all(|argument| {
        if argument.application.is_some() || argument.evidence_projection.is_some() {
            return false;
        }
        if !argument.symbol.is_valid() {
            // A literal const selection retains no declaration symbol; every
            // other symbol-free argument is an unresolved selection.
            return argument.const_literal.is_some();
        }
        matches!(
            program.symbols.get(argument.symbol).kind,
            symbols::SymbolKind::BuiltinType
                | symbols::SymbolKind::Data
                | symbols::SymbolKind::Const
        )
    })
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

/// A nested operand's reads count when that operand subtree keeps builtin
/// bound meaning — the same floor `record_dependencies` applies to the
/// top-level expression — and its own read scan completes. The general
/// bound-meaning walk does not descend through the compound nodes handled
/// above, so each operand under them has to carry the floor independently.
/// When the floor fails, the operand is walked node by node instead: a
/// builtin arithmetic node keeps recursing into its operands, and an
/// authored arithmetic application is call-shaped, so it admits exactly
/// the operand reads its exact checked operator-use row authenticates
/// (`collect_selected_arithmetic_reads`). A cast or unary wrapper standing
/// above such an application has one immediate runtime child and no
/// overloadable token of its own, so the walk keeps descending through it
/// to whatever the child can prove. Every other non-builtin node — an
/// authored comparison, an unresolved selection — stays incomplete rather
/// than pretending only its visible places were read.
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
    if validation::has_builtin_bound_expression_meaning(program, machine, Some(state), operand) {
        return collect_reads(
            program,
            machine,
            state,
            statement_index,
            operand,
            calls,
            operators,
            reads,
            depth + 1,
        );
    }
    if depth >= 128 || !program.expression_table.expression_is_valid(operand) {
        return false;
    }
    // "Unary operations, borrows, casts, membership tests, and member access
    // have one immediate runtime child" (wiki/spec/language/expressions.md), and
    // the closed token vocabulary binds no spelling to either node — a cast
    // and a `!`/`~` are compiler-owned, never a selectable declaration — so a
    // wrapper reads exactly what its child reads and admits nothing the child
    // could not prove on its own. The general bound walk already passes
    // straight through both nodes; only the authored application below them
    // refused the floor, so the gate keeps walking down to that application
    // instead of stopping at the wrapper it does understand.
    let binary = match program.expression_table.expression(operand) {
        ExpressionNode::Cast(cast) => {
            return collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                cast.value,
                calls,
                operators,
                reads,
                depth + 1,
            );
        }
        ExpressionNode::Unary(unary) => {
            return collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                unary.operand,
                calls,
                operators,
                reads,
                depth + 1,
            );
        }
        ExpressionNode::Binary(binary) => binary,
        _ => return false,
    };
    let Some(spelling) =
        crate::operators::binary_operator_spelling(binary.operator).filter(|spelling| {
            use language_core::OperatorSpelling;
            matches!(
                spelling,
                OperatorSpelling::Add
                    | OperatorSpelling::Subtract
                    | OperatorSpelling::Multiply
                    | OperatorSpelling::Divide
                    | OperatorSpelling::Modulo
            )
        })
    else {
        return false;
    };
    if validation::has_builtin_binary_expression_meaning(program, machine, Some(state), operand) {
        return collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            binary.left,
            calls,
            operators,
            reads,
            depth + 1,
        ) && collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            binary.right,
            calls,
            operators,
            reads,
            depth + 1,
        );
    }
    collect_selected_arithmetic_reads(
        program,
        machine,
        state,
        statement_index,
        operand,
        spelling,
        calls,
        operators,
        reads,
        depth,
    )
}

/// A selected `+`/`-`/`*`/`/`/`%` application is a checked occurrence, not
/// builtin arithmetic: the exact `CheckedOperatorUseFact` at this statement
/// authenticates which declaration governs it, and that declaration can
/// observe only the operands it receives, so its footprint is the operands'
/// own reads — each recursing through the operand gate, so a nested
/// authored application still proves its own custody. A constant-shaped
/// application (`1u64 + 0u64`) is refused even with custody: the place
/// algebra folds such a selector syntactically (`index_place_segment`), so
/// the recorded coordinate would be builtin arithmetic's value rather than
/// whatever the selected declaration produces, and the read set would
/// claim disjointness the declaration never established.
fn collect_selected_arithmetic_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    spelling: language_core::OperatorSpelling,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    if program
        .expression_table
        .constant_integer_value(expression)
        .is_some()
    {
        return false;
    }
    let Some(operands) = selected_operator_operands(
        program,
        machine,
        state,
        statement_index,
        expression,
        spelling,
        operators,
    ) else {
        return false;
    };
    operands.iter().all(|operand| {
        collect_operand_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            operators,
            reads,
            depth + 1,
        )
    })
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

/// A member on a place chain reads its projected place. A member whose
/// receiver produces a temporary instead of naming storage — `compute().a`,
/// `Pair { .. }.a`, `compute().inner.a`, `match flag { .. }.a` — reads
/// exactly whatever producing that temporary read; the projection itself
/// touches no caller place, so the member expression's canonical place is
/// expression-rooted and the place path cannot describe it. That shape admits
/// only when the member still resolves to a declared field of the receiver's
/// exact type — the identity `temporary_member_symbol` recovers — and the
/// receiver's own read scan completes, so a missing call-occurrence custody
/// row or an unproven operand still leaves the set incomplete. A `Borrow`
/// receiver keeps the place floor either way: `(&x).f` is `x.f`, while a
/// borrow of a temporary has no statement-use place custody to lend the
/// projection.
fn collect_member_reads(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
    calls: Option<&RangeCallContext<'_>>,
    operators: Option<&CheckedOperatorFacts>,
    reads: &mut Vec<CanonicalPlace>,
    depth: usize,
) -> bool {
    let receiver_is_borrow = matches!(
        program.expression_table.expression(member.receiver),
        ExpressionNode::Borrow(_)
    );
    let place_shaped = receiver_is_borrow
        || canonical_place_from_expression_in_state(
            program,
            state.symbol,
            statement_index,
            expression,
        )
        .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Symbol(_)));
    if place_shaped {
        return collect_place_read(
            program,
            machine,
            state,
            statement_index,
            expression,
            calls,
            operators,
            reads,
            depth,
        );
    }
    temporary_member_symbol(program, machine, state, member).is_valid()
        && collect_reads(
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

/// The declared field identity of a member whose receiver produces a
/// temporary. `effective_member_symbol` already answers receivers that keep a
/// place position — names, calls, literals, indexed chains — plus any
/// binder-stamped `member_symbol`. A `match` receiver has neither: value
/// dispatch is a control-flow join the member binder never walked, so the
/// identity must come from the arms' declared result types instead, and the
/// same holds for a member receiver whose own receiver bottoms out below the
/// binder's reach — `match { .. }.inner.a` resolves `a` on the declared type
/// of the `inner` field the arm agreement supplied, so deeper chains keep
/// resolving while each hop names a declared field of the previous hop's
/// leaf. An arm whose result type cannot be recovered, leaves that disagree,
/// a case-qualified hop (whose variant disambiguation the leaf alone cannot
/// supply), or a member name no field of the agreed declaration owns all stay
/// unproven. The footprint itself still comes from the receiver's own read
/// scan — this answers only which field the projection selects.
fn temporary_member_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    temporary_member_symbol_at(program, machine, state, member, 0)
}

fn temporary_member_symbol_at(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    member: &typed_trees::expression::TableMemberExpression,
    depth: usize,
) -> SymbolHandle {
    if depth >= 128 {
        return SymbolHandle::invalid();
    }
    let direct = crate::flow::effective_member_symbol(program, member.receiver, member);
    if direct.is_valid() || member.case_variant.is_some() {
        return direct;
    }
    let Some(leaf) = temporary_receiver_leaf(program, machine, state, member.receiver, depth + 1)
    else {
        return direct;
    };
    crate::flow::resolve_member_symbol_from_type_symbol(program, leaf, member.member.as_str())
        .unwrap_or_else(SymbolHandle::invalid)
}

/// The declaration leaf a temporary-producing receiver's value stands on when
/// the member binder's contextual walk cannot type it. A `match` receiver's
/// leaf is the arm agreement: the checker's own result oracle is what
/// `validate_match_dispatch` consults for arm compatibility, so requiring
/// every arm's recovered leaf to name the same declaration is the
/// member-level statement of that agreement — judged on leaf symbols rather
/// than type handles, because separately authored references to one
/// declaration intern separately. A member receiver whose own receiver is
/// temporary recurses through the hop's declared type: `match { .. }.inner`
/// stands on the leaf of the `inner` field the agreement supplied. Receivers
/// the ordinary walk already types never reach this scan — the caller's
/// `effective_member_symbol` answer came first — and every other kind keeps
/// the leaf unproven.
fn temporary_receiver_leaf(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: ExpressionHandle,
    depth: usize,
) -> Option<SymbolHandle> {
    if depth >= 128 || !program.expression_table.expression_is_valid(receiver) {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Match(dispatch) => {
            let mut leaf = SymbolHandle::invalid();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                let symbol = validation::expression_result_type_reference(
                    program, machine, state, arm.value,
                )
                .map(|reference| program.type_reference_table.type_symbol(reference))
                .filter(|symbol| symbol.is_valid())?;
                if leaf.is_valid() && leaf != symbol {
                    return None;
                }
                leaf = symbol;
            }
            leaf.is_valid().then_some(leaf)
        }
        // An intermediate member hop carries the same honest-identity floor
        // as the leaf member, then stands on that field's declared type. A
        // case-qualified hop cannot be disambiguated by a leaf alone, so the
        // chain stays unproven rather than guessing at a variant.
        ExpressionNode::Member(inner) => {
            let field = temporary_member_symbol_at(program, machine, state, inner, depth + 1);
            if !field.is_valid() {
                return None;
            }
            crate::flow::symbol_type_symbol(program, field)
        }
        _ => None,
    }
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

    let spelling = if matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) {
        OperatorSpelling::Range
    } else {
        OperatorSpelling::Index
    };
    let Some(operands) = selected_operator_operands(
        program,
        machine,
        state,
        statement_index,
        expression,
        spelling,
        operators,
    ) else {
        return false;
    };
    operands.iter().all(|operand| {
        collect_reads(
            program,
            machine,
            state,
            statement_index,
            *operand,
            calls,
            operators,
            reads,
            depth + 1,
        )
    })
}

/// The operand expressions a selected operator application hands its
/// declaration, authenticated by the exact `CheckedOperatorUseFact` at this
/// statement occurrence. Use rows carry the enclosing statement's origin
/// even for nested operands, so this join covers selector and
/// subexpression positions; custody must agree across every recorded row
/// for the occurrence, resolve to a single valid declaration under the
/// expected spelling, and retain the candidate roster it was selected
/// from. The operand count still has to match the retained signature so a
/// drifted row cannot rename storage the operand scan never saw. Missing,
/// ambiguous, or inconsistent custody yields no operands — the same
/// evidence floor the checked-call join applies.
fn selected_operator_operands(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
    spelling: language_core::OperatorSpelling,
    operators: Option<&CheckedOperatorFacts>,
) -> Option<Vec<ExpressionHandle>> {
    let operators = operators?;
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
    let selected = uses.next()?;
    if selected.spelling != spelling
        || selected.status != CheckedOperatorResolutionStatus::Resolved
        || !selected.selected_operator_symbol.is_valid()
        || selected.candidate_count != operators.candidates(selected).len()
        || uses.any(|other| other != selected)
    {
        return None;
    }
    let candidate = operators.selected_candidate(selected)?;
    let operands = selected.operands(program)?;
    (candidate.parameter_count == operands.len()).then_some(operands)
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
            // `StructLiteral` receivers, and it deliberately never stamps a
            // case-qualified projection either: `subject.f@Case` reaches this
            // scan with `member_symbol` unset on every destructure-desugared
            // guard and target-argument read, and the unwalked hop shadows
            // every member stacked above it — `subject.inner@Case.v` leaves
            // `v` unstamped as well because the binder cannot type a
            // case-qualified receiver. For both shapes the honest identity
            // floor is `effective_member_symbol` — the same contextual
            // resolution the canonical-place production stamps into the
            // `Case`/`Field` segments, so the selector gate cannot admit a
            // place the walk below would reject. A member on a receiver the
            // binder did resolve keeps the authored row's own identity: a
            // symbol it saw but left unresolved cannot be recovered from the
            // receiver's spelling.
            (member.member_symbol.is_valid()
                || ((member.case_variant.is_some()
                    || member_receiver_escapes_binder(program, member.receiver, 0))
                    && crate::flow::effective_member_symbol(program, member.receiver, member)
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
            // Inspect each selector explicitly through the operand gate: it
            // keeps builtin bound meaning, or it proves an authored
            // arithmetic application through checked custody — and the gate
            // refuses a constant-shaped authored application before
            // syntax-based normalization can mint a fixed element
            // coordinate the selected declaration never established.
            has_builtin_index_meaning(
                program,
                machine,
                state,
                statement_index,
                expression,
                indexed,
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
            ) && collect_operand_reads(
                program,
                machine,
                state,
                statement_index,
                indexed.index,
                calls,
                operators,
                reads,
                depth,
            )
        }
        _ => false,
    }
}

/// Whether a member's receiver chain escaped the typed member binder, so an
/// unset `member_symbol` on the member marks a hop the binder never typed
/// rather than a name it saw and refused. The binder descends a member chain
/// only while each hop resolves on the previous hop's declared type: a
/// case-qualified hop is never stamped, so every member stacked above it —
/// and above any earlier unwalked hop, recursively — keeps the contextual
/// `effective_member_symbol` floor. An explicit `&`/`&mut` receiver peels
/// before the binder's coverage test, so it escapes the same way. A receiver
/// the binder did resolve (or whose own refusal was reachable) returns false:
/// an unset symbol above it stays a refusal that contextual lookup must not
/// repair — an unqualified `subject.f` naming a payload field, for example,
/// cannot borrow `f@Case`'s identity merely because the name happens to
/// resolve inside one variant.
fn member_receiver_escapes_binder(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(receiver) {
        return false;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Member(inner) => {
            inner.case_variant.is_some()
                || (!inner.member_symbol.is_valid()
                    && member_receiver_escapes_binder(program, inner.receiver, depth + 1))
        }
        ExpressionNode::Borrow(_) => true,
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

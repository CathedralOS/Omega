//! Discover each statement's call owners before composing its operation sequence.
//! Flow inventory is in completion order, while occurrence ordinals identify
//! source preorder. Discovery follows the latter so enclosing operand roots can
//! claim nested producers; retained operand plans still own execution order.
//! Exact call sites, not destination types or statement-family recognizers,
//! supply argument ownership. Selected exits keep their own control authority.

use super::super::{CheckFacts, CheckedScalarExpressionRole, StatementNode, SymbolHandle};
use super::collecting::{
    collect_argument_calls, collect_assignment_calls, collect_constructor_calls,
    collect_owned_root_calls, collect_range_calls, collect_root_calls,
    collect_structural_argument_calls,
};
use super::{ordered_statement_call, terminal_named_call};
use crate::execution::terminal_unit::control::{statement_sequence, structural_operands};

use crate::execution::terminal_unit::control::LocalConstructionTrace;

/// Sequence only the common body. Selected exit operands retain their own
/// computation/value roots and must not consume the body's call roster.
/// Trace the exact admission step and statement when occurrence custody fails.
pub(in crate::execution::terminal_unit) fn outer_calls_before_traced<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    statement_end: usize,
    trace: &LocalConstructionTrace,
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    trace.phase("outer calls: statement window");
    let statements = program.statement_table.statements(state.statement_nodes);
    // Selected execution retires the row of a call it replaced in the body;
    // the row keeps its handle for the certificates holding it and no longer
    // names an outer or nested call to sequence.
    let erased_statement = |statement: &StatementNode| matches!(statement, StatementNode::LocalData(local) if local.relevance.is_erased());
    // Proof initializers keep their flow evidence for checking, but neither
    // their outer call nor any nested operand call enters runtime sequencing.
    let omitted = |call: &checked_trees::FlowCallFact| {
        facts.flow.control.is_retired(state.symbol, call)
            || statements
                .get(call.statement_index)
                .is_some_and(erased_statement)
    };
    // A call whose target IS a compiler builtin slot with an empty write
    // frame -- the `min`/`max`/`sqrt` shorthand, which resolves straight to
    // the builtin, and the unnameable float rounding and classification
    // intrinsics selected execution settles a call into -- is an operand of
    // its enclosing scalar expression: the computation graph folds it rather
    // than recording a Call node, so it is never an outer or nested call to
    // sequence. Machine-control, port, and custody builtins keep their rows.
    //
    // An authored `F32::`/`F64::` boundary-operator spelling is deliberately
    // NOT folded here, even for the same operation. Psi records such a use
    // with no provider selection (CheckedNamedOperatorUseFact), because each
    // target joins its own ProviderPlan by requirement identity; whether the
    // selected target realizes the requirement as a folded instruction or as
    // a call into a provider machine is Omega's decision. Folding the
    // authored spelling in Psi would take that decision across the firewall,
    // so the two spellings diverge on purpose rather than by omission.
    let scalar_intrinsic = |call: &checked_trees::FlowCallFact| {
        program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .is_some_and(|function| function.has_empty_write_frame())
    };
    if statement_end > statements.len()
        || calls.iter().any(|call| {
            !scalar_intrinsic(call) && !omitted(call) && call.statement_index >= statement_end
        })
    {
        return None;
    }
    let mut consumed = Vec::new();
    let mut structural = Vec::<&checked_trees::FlowCallFact>::new();
    let mut outer = Vec::new();
    let owner = program.machine_by_symbol(machine)?;
    let mut scalar_local_count = 0u32;
    let control_prefix =
        statement_sequence::scalar_control(program, facts, owner, state).map(|(_, prefix)| prefix);
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .take(statement_end)
    {
        if erased_statement(statement) {
            continue;
        }
        // A stored range is evaluated by its LocalBinding consumer. The
        // expression coordinate may also retain the same endpoint occurrence;
        // it is not another scheduled construction at this statement.
        let local_range = if let Some(local) = statement.as_local_data()
            && !local.is_mutable
            && collect_range_calls(
                program,
                facts,
                machine,
                state.symbol,
                statement_index,
                local.initial_value,
                checked_trees::CheckedSubsliceSite::LocalBinding,
                calls,
                0,
                &mut consumed,
            )? {
            local.initial_value
        } else {
            typed_trees::expression::ExpressionHandle::invalid()
        };
        for (_, root) in facts
            .values
            .structural_values
            .roots
            .iter()
            .filter(|(_, root)| {
                root.state == state.symbol && root.statement_ordinal as usize == statement_index
            })
        {
            if root.machine != machine {
                return None;
            }
            if local_range.is_valid() && root.expression == local_range {
                continue;
            }
            let mut pending = vec![root.root];
            let mut visited = Vec::new();
            while let Some(value) = pending.pop() {
                let plans = &facts.values.structural_values;
                if !plans.nodes.is_valid(value) || visited.contains(&value) {
                    return None;
                }
                visited.push(value);
                let node = plans.nodes.get(value);
                match node.kind {
                    checked_trees::CheckedStructuralValueKind::QuotientCall {
                        request,
                        ref arguments,
                        ..
                    } => {
                        // The sealed request owns this exact captured source
                        // occurrence. Its selected ordinary call is emitted by
                        // the retained value graph, not scheduled a second time.
                        let mut occurrences =
                            facts.flow.control.calls.iter().filter(|(_, call)| {
                                call.authored_expression == request
                                    && calls.iter().any(|candidate| std::ptr::eq(candidate, *call))
                            });
                        let (handle, call) = occurrences.next()?;
                        if occurrences.next().is_some()
                            || call.statement_index != statement_index
                            || facts.flow.control.is_retired(state.symbol, call)
                            || program
                                .expression_table
                                .call_node(request)?
                                .quotient_operation
                                .is_none()
                        {
                            return None;
                        }
                        if !consumed.contains(&handle) {
                            consumed.push(handle);
                        }
                        pending.extend(arguments.iter().rev().copied());
                    }
                    checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                        if !facts.flow.control.calls.is_valid(source_call) {
                            return None;
                        }
                        let call = facts.flow.control.calls.get(source_call);
                        if call.authored_expression != node.expression
                            || call.statement_index != statement_index
                        {
                            return None;
                        }
                        // Owner and projection roots can both retain this
                        // captured occurrence. Discovery lists it once; the
                        // operand schedule, not the number of root references,
                        // owns its single execution and result establishment.
                        if consumed.contains(&source_call) {
                            continue;
                        }
                        consumed.push(source_call);
                        structural.push(call);
                    }
                    checked_trees::CheckedStructuralValueKind::Record { fields, .. }
                    | checked_trees::CheckedStructuralValueKind::StructuralCase {
                        fields, ..
                    } => {
                        for field in plans.record_fields.span(fields)? {
                            if let checked_trees::CheckedStructuralRecordFieldValue::Structural(
                                child,
                            ) = field.value
                            {
                                pending.push(child);
                            }
                        }
                    }
                    checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                        pending.extend(plans.dispatch_arms.span(arms)?.iter().map(|arm| arm.value));
                    }
                    checked_trees::CheckedStructuralValueKind::Projection { source, .. }
                    | checked_trees::CheckedStructuralValueKind::QuotientAdaptation {
                        source,
                        ..
                    } => {
                        pending.push(source);
                    }
                    checked_trees::CheckedStructuralValueKind::FixedArray { ref elements } => {
                        pending.extend(elements.iter().copied());
                    }
                    checked_trees::CheckedStructuralValueKind::Reference {
                        source_value, ..
                    } if source_value.is_valid() => {
                        pending.push(source_value);
                    }
                    checked_trees::CheckedStructuralValueKind::Reference { ref source, .. }
                    | checked_trees::CheckedStructuralValueKind::BorrowedSliceView {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::MutableSliceView {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                        ref source,
                        ..
                    }
                    | checked_trees::CheckedStructuralValueKind::Place(ref source) => {
                        collect_structural_argument_calls(
                            program,
                            facts,
                            state.symbol,
                            statement_index,
                            source,
                            calls,
                            0,
                            &mut Vec::new(),
                            &mut consumed,
                        )?;
                    }
                    checked_trees::CheckedStructuralValueKind::Case(_)
                    | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
                    | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
                    | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
                    | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. } => {}
                }
            }
        }
        collect_constructor_calls(
            program,
            facts,
            machine,
            state,
            statement_index,
            statement,
            calls,
            &mut consumed,
        )?;
        if control_prefix.is_some_and(|prefix| statement_index >= prefix) {
            for (_, root) in facts
                .values
                .scalar_computations
                .roots
                .iter()
                .filter(|(_, root)| {
                    root.state == state.symbol
                        && root.statement_ordinal as usize == statement_index
                        && matches!(
                            root.role,
                            CheckedScalarExpressionRole::Guard
                                | CheckedScalarExpressionRole::Return
                                | CheckedScalarExpressionRole::ContinuationReturn
                        )
                })
            {
                collect_owned_root_calls(
                    program,
                    facts,
                    machine,
                    state.symbol,
                    statement_index,
                    root,
                    calls,
                    &mut consumed,
                )?;
            }
            continue;
        }
        // A primitive local's initializer computation owns the calls it
        // evaluates, whether it binds an immutable local or initializes
        // mutable storage; only immutable locals take a binding ordinal.
        if let Some(local) = statement.as_local_data()
            && let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
        {
            let role = if local.is_mutable {
                CheckedScalarExpressionRole::StorageInitializer
            } else {
                let role = CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: scalar_local_count,
                };
                scalar_local_count = scalar_local_count.checked_add(1)?;
                role
            };
            if let Some(root) = facts.values.scalar_computations.root_at(
                state.symbol,
                u32::try_from(statement_index).ok()?,
                role,
            ) {
                let plans = &facts.values.scalar_computations;
                if !plans.is_authored_root(root, machine, local.initial_value, primitive_type) {
                    return None;
                }
                collect_root_calls(
                    program,
                    facts,
                    state.symbol,
                    statement_index,
                    root.root,
                    calls,
                    0,
                    &mut consumed,
                )?;
            }
        }
        if matches!(statement, StatementNode::Expression(_))
            && statement_index.checked_add(1)?
                == program
                    .statement_table
                    .statements(state.statement_nodes)
                    .len()
            && let Some(root) = facts.values.scalar_computations.root_at(
                state.symbol,
                u32::try_from(statement_index).ok()?,
                CheckedScalarExpressionRole::Return,
            )
        {
            if root.machine != machine {
                return None;
            }
            collect_root_calls(
                program,
                facts,
                state.symbol,
                statement_index,
                root.root,
                calls,
                0,
                &mut consumed,
            )?;
        }
        let Some(assignment) = statement.as_assignment() else {
            continue;
        };
        collect_assignment_calls(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            calls,
            &mut consumed,
        )?;
    }
    // Flow rows retain completion order: nested producers finish before their
    // enclosing invocation. Ownership discovery instead follows exact source
    // occurrence preorder, so an enclosing call can claim its operand calls
    // before they are considered independent statement operations. This does
    // not schedule execution; retained operand plans keep authored effect order.
    // Structural owners (value-walk calls and the structural operands of
    // scheduled calls) claim their argument calls before each visit, so a call
    // nested in a structural operand is owned before preorder reaches it.
    let mut occurrence_order = calls
        .iter()
        .filter(|call| !scalar_intrinsic(call) && !omitted(call))
        .collect::<Vec<_>>();
    occurrence_order.sort_by_key(|call| (call.statement_index, call.call_ordinal));
    if occurrence_order.windows(2).any(|pair| {
        (pair[0].statement_index, pair[0].call_ordinal)
            == (pair[1].statement_index, pair[1].call_ordinal)
    }) {
        return None;
    }
    let mut claimed_structural = 0;
    let expands = |call: &checked_trees::FlowCallFact| !scalar_intrinsic(call) && !omitted(call);
    for call in occurrence_order {
        claim_structural_operand_calls(
            program,
            facts,
            owner,
            state,
            calls,
            &mut structural,
            &mut claimed_structural,
            &mut consumed,
            &expands,
            trace,
        )?;
        if consumed
            .iter()
            .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), call))
            || structural.iter().any(|owned| std::ptr::eq(*owned, call))
        {
            continue;
        }
        // A target computation can precede the statement's value call, so
        // ordinal zero does not identify that call. Retained operand owners
        // remove their exact occurrences; only the statement's unowned outer
        // call is scheduled here. Its arguments then own their nested calls.
        trace.phase("outer calls: duplicate statement call");
        trace.statement(u32::try_from(call.statement_index).ok());
        if outer.iter().any(|prior: &&checked_trees::FlowCallFact| {
            prior.statement_index == call.statement_index
        }) {
            for unclaimed in outer.iter().copied().chain(std::iter::once(call)) {
                if let Some(reason) =
                    crate::execution::terminal_unit::calls::boundary_authority_refusal(
                        program, machine, state, unclaimed,
                    )
                {
                    trace.phase(reason);
                    break;
                }
            }
            return None;
        }
        outer.push(call);
        trace.phase("outer calls: structural operands");
        trace.statement(u32::try_from(call.statement_index).ok());
        for nested in structural_operands::for_call(program, facts, owner, state, call)? {
            // The value graph and canonical-place operand can name the same
            // captured occurrence. This is one catalog entry, not a second
            // invocation: the operand sequencer establishes its binding once.
            if structural.iter().any(|prior| std::ptr::eq(*prior, nested)) {
                continue;
            }
            structural.push(nested);
        }
        trace.phase("outer calls: ordered statement call");
        trace.statement(u32::try_from(call.statement_index).ok());
        let statement = program
            .statement_table
            .statement_at(state, call.statement_index)?;
        if matches!(statement, StatementNode::Expression(_)) {
            let (expression, authored) =
                ordered_statement_call(program, owner, state, call.statement_index)?;
            if call.authored_expression != expression
                || call.target_symbol != validation::call_target_symbol(program, authored)
            {
                return None;
            }
        }
        // The statement still owns its whole result operation. Destination
        // type and statement position do not decide argument ownership: the
        // exact call occurrence supplies its own positional operand roster.
        let result_expression = match statement {
            StatementNode::LocalData(local) => Some(local.initial_value),
            StatementNode::Assignment(assignment) => Some(assignment.value),
            _ => None,
        };
        if result_expression.is_some_and(|expression| call.authored_expression != expression) {
            return None;
        }
        // Non-completing named transitions remain selected control edges,
        // never unconditional calls emitted by this linear sequence.
        if let StatementNode::Transition(transition) = statement
            && terminal_named_call(program, owner, state, call.statement_index)
                != Some(call.target_symbol)
        {
            if matches!(
                program.statement_table.transition_target(transition.target),
                typed_trees::statement::TransitionTargetNode::Named { .. }
            ) {
                continue;
            }
            return None;
        }
        trace.phase("outer calls: exact call occurrence");
        trace.statement(u32::try_from(call.statement_index).ok());
        let site = validation::find_call_site_in_state(
            program,
            owner,
            state,
            call.statement_index,
            call.call_ordinal,
        )?;
        // Occurrence traversal marks every transition target as selected.
        // The exact terminal named occurrence instead has its control authority
        // from terminal_named_call: final Always, no preceding transition or
        // continuation, and an ordinary result matching the enclosing return.
        // Nested expression calls cannot inherit that completion proof.
        let terminal_completion = matches!(site, validation::CallSite::TransitionNamed { .. })
            && terminal_named_call(program, owner, state, call.statement_index)
                == Some(call.target_symbol);
        if !terminal_completion {
            validation::call_is_unconditional(
                program,
                machine,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
        }
        let target = match &site {
            validation::CallSite::Expression {
                expression,
                call: authored,
            } => {
                if *expression != call.authored_expression {
                    return None;
                }
                validation::call_target_symbol(program, authored)
            }
            validation::CallSite::Statement(authored) => authored.target_symbol,
            // The arm spells the callee as its machine or its entry state;
            // the call row names the resolved entry state either way.
            validation::CallSite::TransitionNamed { path, .. } => {
                validation::terminal_callable_target(program, owner, path.symbol)
                    .map_or(path.symbol, |(callable, _)| callable)
            }
        };
        if target != call.target_symbol {
            return None;
        }
        let arguments = validation::call_site_argument_expressions(program, &site);
        trace.phase("outer calls: argument calls");
        trace.statement(u32::try_from(call.statement_index).ok());
        collect_argument_calls(
            program,
            facts,
            machine,
            state.symbol,
            call,
            arguments,
            calls,
            &mut consumed,
        )?;
    }
    // The last scheduled call's structural operands claim theirs here.
    claim_structural_operand_calls(
        program,
        facts,
        owner,
        state,
        calls,
        &mut structural,
        &mut claimed_structural,
        &mut consumed,
        &expands,
        trace,
    )?;
    trace.phase("outer calls: unconsumed nested calls");
    trace.statement(None);
    if let Some(unconsumed) = calls
        .iter()
        .filter(|call| !scalar_intrinsic(call) && !omitted(call))
        .find(|call| {
            !consumed
                .iter()
                .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), *call))
                && !structural.iter().any(|nested| std::ptr::eq(*nested, *call))
                && !outer
                    .iter()
                    .any(|scheduled| std::ptr::eq(*scheduled, *call))
        })
    {
        // Name the statement family holding the first unconsumed nested call:
        // the admitted argument positions differ per family, so this is the
        // shape the composed route still lacks.
        trace.phase(match statements.get(unconsumed.statement_index) {
            Some(StatementNode::Assignment(_)) => {
                "outer calls: unconsumed nested call in an assignment"
            }
            Some(StatementNode::LocalData(_)) => {
                "outer calls: unconsumed nested call in a local initializer"
            }
            Some(StatementNode::Call(_)) => {
                "outer calls: unconsumed nested call in a call statement"
            }
            Some(StatementNode::Expression(_)) => {
                "outer calls: unconsumed nested call in an expression statement"
            }
            Some(StatementNode::Transition(_)) => {
                "outer calls: unconsumed nested call in a transition"
            }
            Some(StatementNode::RootBinding(_)) => {
                "outer calls: unconsumed nested call in a root binding"
            }
            Some(StatementNode::AssemblyFact(_)) => {
                "outer calls: unconsumed nested call in an assembly fact"
            }
            None => "outer calls: unconsumed nested call past the statement window",
        });
        trace.statement(u32::try_from(unconsumed.statement_index).ok());
        return None;
    }
    Some(outer)
}

/// Calls reached through the structural value walk (a record or case
/// initializer holding a call node) and the structural operands of scheduled
/// calls skip the outer-call loop's operand gathering. Claim each pending
/// one's structural operand calls into `structural` and its argument calls
/// into `consumed`, so transitively nested calls are owned before the loop
/// visits them and are not reported unconsumed. In
/// `combine(produce(&mut count), tick(&mut count), produce(&mut count)).value`
/// the value-walk call `combine` must claim `tick` before preorder reaches it.
#[allow(clippy::too_many_arguments)]
fn claim_structural_operand_calls<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    owner: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    structural: &mut Vec<&'a checked_trees::FlowCallFact>,
    claimed: &mut usize,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
    expands: &dyn Fn(&checked_trees::FlowCallFact) -> bool,
    trace: &LocalConstructionTrace,
) -> Option<()> {
    while let Some(call) = structural.get(*claimed).copied() {
        *claimed += 1;
        trace.phase("outer calls: structural operand expansion");
        trace.statement(u32::try_from(call.statement_index).ok());
        if expands(call) {
            for nested in structural_operands::for_call(program, facts, owner, state, call)? {
                if !structural.iter().any(|prior| std::ptr::eq(*prior, nested)) {
                    structural.push(nested);
                }
            }
        }
        trace.phase("outer calls: structural argument site");
        trace.statement(u32::try_from(call.statement_index).ok());
        let site = validation::find_call_site(
            program,
            owner.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let arguments = validation::call_site_argument_expressions(program, &site);
        trace.phase("outer calls: structural argument calls");
        collect_argument_calls(
            program,
            facts,
            owner.symbol,
            state.symbol,
            call,
            arguments,
            calls,
            consumed,
        )?;
    }
    Some(())
}

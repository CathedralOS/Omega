//! Which calls a statement's expressions contain: the recursive collector and
//! its per-kind operand walks, plus the dispatch-subject and
//! statement-computation ownership questions the walk answers.

use super::super::{CheckFacts, CheckedScalarExpressionRole, StatementNode, SymbolHandle};
use super::ComputationVisit;
use crate::execution::terminal_unit::control::structural_operands;

use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

/// Assignment targets and scalar values own their computation calls before
/// the separately scheduled whole-value call. Both the statement sequencer
/// and that call's operand owner consume the same exact occurrence ledger.
pub(super) fn collect_assignment_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    assignment: &typed_trees::statement::TableAssignment,
    calls: &[checked_trees::FlowCallFact],
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let owner = program.machine_by_symbol(machine)?;
    // A scalar computation owns its call occurrences whether its value
    // lands in a local, a field, or an indexed destination. Selectors own
    // theirs in the same way. Leaving either in the outer-call roster
    // makes the statement count an invocation the store already evaluates.
    let plans = &facts.values.scalar_computations;
    let selectors = validation::assignment_target_selectors(program, assignment.target);
    let values = if let Some(carrier) = validation::atomic_assignment_carrier(program, assignment) {
        carrier
            .operands
            .into_iter()
            .enumerate()
            .map(|(ordinal, operand)| {
                Some((
                    CheckedScalarExpressionRole::AtomicOperand {
                        operand_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    operand,
                ))
            })
            .collect::<Option<Vec<_>>>()?
    } else {
        vec![(
            CheckedScalarExpressionRole::AssignmentValue,
            assignment.value,
        )]
    };
    let operands = selectors
        .into_iter()
        .enumerate()
        .map(|(depth, expression)| {
            Some((
                CheckedScalarExpressionRole::AssignmentIndex {
                    depth: u32::try_from(depth).ok()?,
                },
                expression,
            ))
        })
        .chain(values.into_iter().map(Some));
    for operand in operands {
        let (role, expression) = operand?;
        let Some(root) = plans.root_at(state.symbol, u32::try_from(statement_index).ok()?, role)
        else {
            continue;
        };
        let primitive = match role {
            CheckedScalarExpressionRole::AssignmentIndex { .. } => {
                typed_trees::types::PrimitiveType::U64
            }
            CheckedScalarExpressionRole::AssignmentValue
            | CheckedScalarExpressionRole::AtomicOperand { .. } => {
                let reference = validation::declared_place_type_raw(
                    program,
                    owner,
                    Some(state),
                    assignment.target,
                )?;
                crate::values::assignment_target_primitive_type(program, reference)?
            }
            _ => return None,
        };
        if !plans.is_authored_root(root, machine, expression, primitive) {
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
            consumed,
        )?;
    }

    Some(())
}

/// Constructors retain their scalar evaluation roots independently of the
/// surrounding schedule. Ordinary statements and selected call arms consume
/// the same exact element, field, subject and pattern occurrence custody.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_constructor_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    calls: &[checked_trees::FlowCallFact],
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let construction_destination = match statement {
        StatementNode::LocalData(local) if !local.is_mutable => {
            Some((local.initial_value, local.type_reference))
        }
        StatementNode::Expression(expression) => Some((*expression, state.return_type)),
        _ => None,
    };
    // Structural dispatch operands and constructor fields own their calls.
    // This is the static call roster, not an instruction to evaluate every
    // arm: emission follows the structural value's selected control path.
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
                    CheckedScalarExpressionRole::StructuralValueField { .. }
                        | CheckedScalarExpressionRole::RecordField { .. }
                        | CheckedScalarExpressionRole::StructuralArrayElement { .. }
                        | CheckedScalarExpressionRole::StructuralScalarValue { .. }
                        | CheckedScalarExpressionRole::StructuralValueSubject { .. }
                        | CheckedScalarExpressionRole::StructuralValuePattern { .. }
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
            consumed,
        )?;
    }
    let constructions = construction_destination
        .into_iter()
        .map(|(expression, expected)| {
            (
                checked_trees::CheckedArrayConstructionSource::Statement,
                expression,
                expected,
            )
        });
    for (source, expression, expected) in constructions {
        let Some(elements) =
            validation::scalar_array_elements(program, machine, expression, expected)
        else {
            continue;
        };
        let plans = &facts.values.scalar_computations;
        for (element_index, (expression, primitive_type)) in
            elements.elements.into_iter().enumerate()
        {
            let role = CheckedScalarExpressionRole::ArrayElement {
                source,
                element_ordinal: u32::try_from(element_index).ok()?,
            };
            let Some(root) =
                plans.root_at(state.symbol, u32::try_from(statement_index).ok()?, role)
            else {
                continue;
            };
            if !plans.is_authored_root(root, machine, expression, primitive_type) {
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
                consumed,
            )?;
        }
    }
    Some(())
}

/// A separately scheduled invocation owns its scalar operand calls through
/// the same computation roots as an invocation in the statement sequence.
/// A transition guard retains its own earlier computation. The two schedules
/// share an occurrence ledger so neither can claim a call the other executes.
/// Every remaining occurrence at that statement must belong to those roots.
pub(in crate::execution::terminal_unit) fn call_owns_statement_computations(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    calls: &[checked_trees::FlowCallFact],
    predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> Option<()> {
    if facts.flow.control.is_retired(state, call)
        || !calls.iter().any(|candidate| std::ptr::eq(candidate, call))
    {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine,
        state,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    let mut consumed = Vec::new();
    let owner = program.machine_by_symbol(machine)?;
    let source_state = program
        .machine_states(owner)
        .iter()
        .find(|source| source.symbol == state)?;
    if let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statement_at(source_state, call.statement_index)
        && let typed_trees::statement::TransitionGuardNode::When(expression) = transition.guard
    {
        let plans = &facts.values.scalar_computations;
        let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state
                && root.statement_ordinal as usize == call.statement_index
                && root.role == CheckedScalarExpressionRole::Guard
        });
        if let Some(root) = roots.next() {
            if roots.next().is_some()
                || !plans.is_authored_root(
                    root,
                    machine,
                    expression,
                    typed_trees::types::PrimitiveType::Bool,
                )
            {
                return None;
            }
            collect_root_calls(
                program,
                facts,
                state,
                call.statement_index,
                root.root,
                calls,
                0,
                &mut consumed,
            )?;
            if consumed.iter().any(|handle| {
                let guard_call = facts.flow.control.calls.get(*handle);
                guard_call.call_ordinal >= call.call_ordinal
                    || validation::transition_call_target(
                        program,
                        owner,
                        source_state,
                        call.statement_index,
                        guard_call.call_ordinal,
                    )
                    .is_none_or(|target| target.is_valid())
            }) {
                return None;
            }
        }
    }
    let statement = program
        .statement_table
        .statement_at(source_state, call.statement_index)?;
    collect_constructor_calls(
        program,
        facts,
        machine,
        source_state,
        call.statement_index,
        statement,
        calls,
        &mut consumed,
    )?;
    if let Some(assignment) = statement.as_assignment() {
        collect_assignment_calls(
            program,
            facts,
            machine,
            source_state,
            call.statement_index,
            assignment,
            calls,
            &mut consumed,
        )?;
    }
    for nested in structural_operands::for_call(program, facts, owner, source_state, call)? {
        let mut occurrences = facts.flow.control.calls.iter().filter(|(_, candidate)| {
            std::ptr::eq(*candidate, nested)
                && calls.iter().any(|source| std::ptr::eq(source, nested))
        });
        let (handle, nested) = occurrences.next()?;
        if occurrences.next().is_some()
            || nested.statement_index != call.statement_index
            || nested.call_ordinal <= call.call_ordinal
            || facts.flow.control.is_retired(state, nested)
            || consumed.contains(&handle)
        {
            return None;
        }
        consumed.push(handle);
        let nested_site = validation::find_call_site(
            program,
            machine,
            state,
            nested.statement_index,
            nested.call_ordinal,
        )?;
        collect_argument_calls(
            program,
            facts,
            machine,
            state,
            nested,
            validation::call_site_argument_expressions(program, &nested_site),
            calls,
            &mut consumed,
        )?;
    }
    collect_argument_calls(
        program,
        facts,
        machine,
        state,
        call,
        arguments,
        calls,
        &mut consumed,
    )?;
    let scheduled_subject = |candidate: &checked_trees::FlowCallFact| {
        scheduled_dispatch_subject(
            program,
            facts,
            owner,
            source_state,
            call,
            candidate,
            calls,
            predecessors,
        )
    };
    // A computation may read the saved subject's fields, but must not also
    // execute the invocation that the predecessor sequence already scheduled.
    if consumed
        .iter()
        .any(|handle| scheduled_subject(facts.flow.control.calls.get(*handle)))
    {
        return None;
    }
    calls
        .iter()
        .filter(|candidate| candidate.statement_index == call.statement_index)
        .all(|candidate| {
            std::ptr::eq(candidate, call)
                || scheduled_subject(candidate)
                || facts.flow.control.is_retired(state, candidate)
                || program
                    .symbols
                    .builtin_function_for_symbol(candidate.target_symbol)
                    .is_some_and(|function| function.has_empty_write_frame())
                || consumed
                    .iter()
                    .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), candidate))
        })
        .then_some(())
}

/// Join a saved dispatch call or operand read to its one already scheduled
/// producer. Exact selection occurrence custody distinguishes parser copies
/// from independently authored calls with identical targets and arguments.
pub(super) fn scheduled_dispatch_subject(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    invocation: &checked_trees::FlowCallFact,
    candidate: &checked_trees::FlowCallFact,
    calls: &[checked_trees::FlowCallFact],
    predecessors: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> bool {
    let Some((statement, expression)) = validation::saved_dispatch_call(
        program,
        state,
        candidate.statement_index,
        candidate.authored_expression,
    ) else {
        return false;
    };
    let mut producers = calls.iter().filter(|producer| {
        producer.statement_index == statement && producer.authored_expression == expression
    });
    let Some(producer) = producers.next() else {
        return false;
    };
    if producers.next().is_some()
        || facts.flow.control.is_retired(state.symbol, producer)
        || producer.target_symbol != candidate.target_symbol
        || (producer.statement_index, producer.call_ordinal)
            >= (invocation.statement_index, invocation.call_ordinal)
    {
        return false;
    }
    let mut owners = 0;
    for operation in predecessors {
        let Some(planned) = operation.call() else {
            continue;
        };
        let mut occurrences = calls.iter().filter(|candidate| {
            candidate.statement_index == planned.coordinate.statement_index as usize
                && candidate.call_ordinal == planned.coordinate.call_ordinal as usize
        });
        let Some(prior) = occurrences.next() else {
            return false;
        };
        if occurrences.next().is_some() || facts.flow.control.is_retired(state.symbol, prior) {
            return false;
        }
        if std::ptr::eq(prior, producer) {
            owners += 1;
            continue;
        }
        if prior.statement_index != producer.statement_index {
            continue;
        }
        // Scalar operand invocations execute in the preceding call's retained
        // computation roots rather than as separate structural operations.
        let Some(site) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            prior.statement_index,
            prior.call_ordinal,
        ) else {
            return false;
        };
        let mut consumed = Vec::new();
        if collect_argument_calls(
            program,
            facts,
            machine.symbol,
            state.symbol,
            prior,
            validation::call_site_argument_expressions(program, &site),
            calls,
            &mut consumed,
        )
        .is_none()
        {
            return false;
        }
        if consumed
            .iter()
            .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), producer))
        {
            owners += 1;
        }
    }
    owners == 1
}

pub(super) fn collect_argument_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    arguments: &[typed_trees::expression::ExpressionHandle],
    calls: &[checked_trees::FlowCallFact],
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let register_operation = program
        .symbols
        .builtin_function_for_symbol(call.target_symbol)
        .and_then(|builtin| {
            language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                builtin.name(),
            )
        });
    // Compiler-owned register signatures supply the same scalar argument
    // roots as authored signatures. Their operands can contain real calls;
    // a physical instruction does not make those evaluations disappear.
    let (scalar_arguments, structural_arguments) = if let Some(operation) = register_operation {
        if arguments.len() != operation.input_carriers().len() {
            return None;
        }
        let scalar = arguments
            .iter()
            .zip(operation.input_carriers())
            .map(|(argument, name)| {
                Some((
                    *argument,
                    typed_trees::types::PrimitiveType::from_name(name.name())?,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        (scalar, Vec::new())
    } else {
        // Register operands own no nested call. An argument call left unclaimed
        // still refuses as an unconsumed nested call.
        if structural_operands::has_register_operands_only(program, call.target_symbol) {
            return Some(());
        }
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let formals = parameters.iter().collect::<Vec<_>>();
        if arguments.len() != formals.len() {
            return None;
        }
        let mut structural_ordinal = 0u32;
        let mut structural_arguments = Vec::new();
        for (argument, parameter) in arguments.iter().zip(formals.iter()) {
            if crate::execution::terminal_unit::strips_erased_parameter(parameter)?
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            {
                continue;
            }
            structural_arguments.push(*argument);
            collect_range_calls(
                program,
                facts,
                machine,
                state,
                call.statement_index,
                *argument,
                checked_trees::CheckedSubsliceSite::CallArgument {
                    call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                    argument_ordinal: structural_ordinal,
                },
                calls,
                u32::try_from(call.call_ordinal.checked_add(1)?).ok()?,
                consumed,
            )?;
            structural_ordinal = structural_ordinal.checked_add(1)?;
        }
        let scalar_arguments = arguments
            .iter()
            .zip(formals)
            .filter_map(|(argument, parameter)| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .map(|primitive| (*argument, primitive))
            })
            .collect::<Vec<_>>();
        (scalar_arguments, structural_arguments)
    };
    let mut roles = Vec::new();
    // All scalar operands belong to one invocation. Normalization can share a
    // computation between argument roots; retain its completed visit across
    // those roots while distinct nodes still face the exact occurrence check.
    let mut visits = Vec::new();
    for (_, root) in facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.state == state && root.statement_ordinal as usize == call.statement_index
        })
    {
        let plans = &facts.values.scalar_computations;
        let (expression, primitive) = match root.role {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal,
                argument_ordinal,
            }
            | CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal,
                argument_ordinal,
            } if call_ordinal as usize == call.call_ordinal => {
                *scalar_arguments.get(argument_ordinal as usize)?
            }
            CheckedScalarExpressionRole::CallSelector {
                call_ordinal,
                argument_ordinal,
                expression,
            } if call_ordinal as usize == call.call_ordinal => {
                let argument = *structural_arguments.get(argument_ordinal as usize)?;
                let place = validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    call.statement_index,
                    argument,
                )?;
                if !place.segments.iter().any(|segment| {
                    matches!(segment, facts::PlaceSegment::Index { expression: source }
                        if *source == expression)
                }) || !plans.nodes.is_valid(root.root)
                {
                    return None;
                }
                let primitive = plans.nodes.get(root.root).primitive_type;
                if !primitive.is_fixed_integer() {
                    return None;
                }
                (expression, primitive)
            }
            _ => continue,
        };
        if root.machine != machine || roles.contains(&root.role) {
            return None;
        }
        roles.push(root.role);
        if !plans.nodes.is_valid(root.root)
            || plans.nodes.get(root.root).authored_root != expression
            || plans.nodes.get(root.root).primitive_type != primitive
        {
            return None;
        }
        // Address operands own their nested invocations just as scalar actuals
        // do. Leaving a selector call in the outer roster either duplicates
        // its execution or refuses a statement whose operand already owns it.
        collect(
            program,
            facts,
            state,
            call.statement_index,
            root.root,
            calls,
            u32::try_from(call.call_ordinal.checked_add(1)?).ok()?,
            &mut visits,
            consumed,
        )?;
    }
    Some(())
}

/// A destination or invocation selects one endpoint site. Other coordinates
/// naming the same authored range are not additional executions.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_range_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
    site: checked_trees::CheckedSubsliceSite,
    calls: &[checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<bool> {
    let plans = &facts.values.scalar_computations;
    let mut computed = false;
    for (endpoint, role, ()) in
        crate::values::subslice_endpoints(program, &facts.operators, expression, site, |_| Some(()))
    {
        let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
            root.state == state && root.statement_ordinal as usize == statement && root.role == role
        });
        let Some(root) = roots.next() else { continue };
        if roots.next().is_some()
            || !plans.is_authored_root(
                root,
                machine,
                endpoint,
                typed_trees::types::PrimitiveType::U64,
            )
        {
            return None;
        }
        collect_root_calls(
            program,
            facts,
            state,
            statement,
            root.root,
            calls,
            minimum_call_ordinal,
            consumed,
        )?;
        computed = true;
    }
    Some(computed)
}

/// A structural source carries its evaluated scalar dependencies directly;
/// call accounting follows those operands, not parallel fact coordinates.
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_structural_argument_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: usize,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    calls: &[checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    visits: &mut Vec<ComputationVisit>,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    // Formation and argument admission already own storage, type and access.
    // This collector owns only executable projection operands. Anonymous
    // backing is retained as a structural value child (Reference::source_value),
    // not necessarily a statement root; its graph walk owns the producer call.
    // Requiring another root here rejects nested borrowed results and would
    // duplicate constructor accounting. Exact occurrence and unconsumed-call
    // checks still prevent losing or scheduling a call twice.
    for handle in argument.computation_operands() {
        collect(
            program,
            facts,
            state,
            statement,
            handle,
            calls,
            minimum_call_ordinal,
            visits,
            consumed,
        )?;
    }
    Some(())
}

/// The calls owned by one computation root in this machine — `None` when the
/// root belongs to another machine or the walk cannot consume them.
pub(super) fn collect_owned_root_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    root: &checked_trees::CheckedScalarComputationRoot,
    calls: &[checked_trees::FlowCallFact],
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    if root.machine != machine {
        return None;
    }
    collect_root_calls(
        program,
        facts,
        state,
        statement_index,
        root.root,
        calls,
        0,
        consumed,
    )
}

/// Collect a scalar-computation root's calls with a fresh visit list.
pub(super) fn collect_root_calls(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: usize,
    root: CheckedScalarComputationHandle,
    calls: &[checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    collect(
        program,
        facts,
        state,
        statement,
        root,
        calls,
        minimum_call_ordinal,
        &mut Vec::new(),
        consumed,
    )
}

pub(super) fn collect(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    statement: usize,
    handle: CheckedScalarComputationHandle,
    calls: &[checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    visits: &mut Vec<ComputationVisit>,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let plans = &facts.values.scalar_computations;
    if !plans.nodes.is_valid(handle) {
        return None;
    }
    // A computation is a DAG value, not one invocation per incoming edge.
    // Reuse a completed node's accounting; an unfinished visit is a cycle.
    // Distinct nodes still cannot claim the same captured call below.
    if let Some(visit) = visits.iter().find(|visit| visit.computation == handle) {
        return visit.completed.then_some(());
    }
    let visit_index = visits.len();
    visits.push(ComputationVisit {
        computation: handle,
        completed: false,
    });
    match &plans.nodes.get(handle).kind {
        CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
            ..
        }
        | CheckedScalarComputationKind::ByteSequenceLiteralComparison { subject, .. } => {
            collect_structural_argument_calls(
                program,
                facts,
                state,
                statement,
                subject,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
        }
        CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
            ..
        } => {
            for field in plans.case_fields.span(subject.fields)? {
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    field.value,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
        }
        CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
            collect(
                program,
                facts,
                state,
                statement,
                *left,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
            collect(
                program,
                facts,
                state,
                statement,
                *right,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
        }
        checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
            collect(
                program,
                facts,
                state,
                statement,
                *operand,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
        }
        CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
            collect(
                program,
                facts,
                state,
                statement,
                *subject,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
            for arm in plans.dispatch_arms.span(*arms)? {
                if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern {
                    collect(
                        program,
                        facts,
                        state,
                        statement,
                        pattern,
                        calls,
                        minimum_call_ordinal,
                        visits,
                        consumed,
                    )?;
                }
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    arm.value,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
        }
        CheckedScalarComputationKind::RetainedConstant { .. }
        | CheckedScalarComputationKind::Value(_)
        | CheckedScalarComputationKind::StructuralField { .. } => {}
        CheckedScalarComputationKind::ByteSequencePredicate {
            predicate,
            source_expression,
            subject,
            ..
        } => {
            collect_structural_argument_calls(
                program,
                facts,
                state,
                statement,
                subject,
                calls,
                minimum_call_ordinal,
                visits,
                consumed,
            )?;
            // This selected primitive owns its authored call occurrence even
            // though execution emits an observation, not a machine invocation.
            // Join the occurrence here, where every enclosing computation
            // routes its operands; statement binding shape does not decide it.
            let (selected, _) =
                typed_trees::byte_predicates::byte_predicate_call(program, *source_expression)?;
            if selected != *predicate {
                return None;
            }
            let authored = program.expression_table.call_node(*source_expression)?;
            let mut occurrences = facts.flow.control.calls.iter().filter(|(_, call)| {
                call.authored_expression == *source_expression
                    && calls.iter().any(|candidate| std::ptr::eq(candidate, *call))
            });
            if let Some((handle, call)) = occurrences.next() {
                if occurrences.next().is_some()
                    || call.statement_index != statement
                    || call.call_ordinal < minimum_call_ordinal as usize
                    || call.target_symbol != authored.target_symbol
                    || consumed.contains(&handle)
                    || facts.flow.control.is_retired(state, call)
                {
                    return None;
                }
                consumed.push(handle);
            }
        }
        CheckedScalarComputationKind::Call {
            source_call,
            call_ordinal,
            callee,
            arguments,
            structural_arguments,
            ..
        } => {
            if !facts.flow.control.calls.is_valid(*source_call) || consumed.contains(source_call) {
                return None;
            }
            let call = facts.flow.control.calls.get(*source_call);
            // A computation root over a retired call describes the authored
            // body, not the settled one; decline rather than sequence it.
            if *call_ordinal < minimum_call_ordinal
                || call.call_ordinal != *call_ordinal as usize
                || call.statement_index != statement
                || validation::exact_external_leaf_boundary_requirement(program, call.target_symbol)
                    .map_or(call.target_symbol, |(requirement, _)| requirement)
                    != callee.called_signature()
                || !call.authored_expression.is_valid()
                || !calls.iter().any(|candidate| std::ptr::eq(candidate, call))
                || facts.flow.control.is_retired(state, call)
            {
                return None;
            }
            consumed.push(*source_call);
            // A runtime implementation selector evaluates before every data operand.
            for operand in callee
                .selector()
                .into_iter()
                .chain(plans.operands.span(*arguments)?.iter().copied())
            {
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    operand,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
            for argument in plans.structural_arguments.span(*structural_arguments)? {
                if let checked_trees::CheckedScalarComputationStructuralArgument::Place(source) =
                    argument
                {
                    collect_structural_argument_calls(
                        program,
                        facts,
                        state,
                        statement,
                        source,
                        calls,
                        minimum_call_ordinal,
                        visits,
                        consumed,
                    )?;
                }
                if let checked_trees::CheckedScalarComputationStructuralArgument::Case(subject) =
                    argument
                {
                    for field in plans.case_fields.span(subject.fields)? {
                        collect(
                            program,
                            facts,
                            state,
                            statement,
                            field.value,
                            calls,
                            minimum_call_ordinal,
                            visits,
                            consumed,
                        )?;
                    }
                }
            }
        }
        CheckedScalarComputationKind::Apply { operands, .. }
        | CheckedScalarComputationKind::StorageObservation { operands, .. } => {
            for operand in plans.operands.span(*operands)? {
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    *operand,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
        }
        CheckedScalarComputationKind::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            for operand in [condition, when_true, when_false] {
                collect(
                    program,
                    facts,
                    state,
                    statement,
                    *operand,
                    calls,
                    minimum_call_ordinal,
                    visits,
                    consumed,
                )?;
            }
        }
    }
    visits[visit_index].completed = true;
    Some(())
}

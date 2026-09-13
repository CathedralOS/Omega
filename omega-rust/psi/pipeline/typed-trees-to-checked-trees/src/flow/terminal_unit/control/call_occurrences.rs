//! Scalar operand calls belong to computation roots; structural operands retain
//! their own result producers within the same authored statement.

use super::*;
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

pub(in crate::flow::terminal_unit) fn tail_call<'a>(
    program: &'a TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'a typed_trees::expression::TableCallExpression,
)> {
    let statements = program.statement_table.statements(state.statement_nodes);
    if !is_unit(program, state.return_type) || statement_index.checked_add(1)? != statements.len() {
        return None;
    }
    let StatementNode::Expression(expression) = statements.get(statement_index)? else {
        return None;
    };
    if !program.expression_table.expression_is_valid(*expression) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        return None;
    };
    Some((*expression, call))
}

/// A Unit-result call can occupy a statement without being the state's tail.
/// Tail consumers retain their existing final-statement check above.
pub(in crate::flow::terminal_unit) fn unit_statement_call<'a>(
    program: &'a TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'a typed_trees::expression::TableCallExpression,
)> {
    if let Some(call) = tail_call(program, state, statement_index) {
        return Some(call);
    }
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    if !program.expression_table.expression_is_valid(*expression) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        return None;
    };
    validation::unit_statement_call_is_supported(program, machine, state, *expression)
        .then_some((*expression, call))
}

pub(super) fn ordered_statement_call<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    &'program typed_trees::expression::TableCallExpression,
)> {
    if let Some(call) = unit_statement_call(program, machine, state, statement_index) {
        return Some(call);
    }
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
        return None;
    };
    if !crate::values::is_scalar_return_call(program, state, *expression) {
        // A final structural call is an ordinary result-producing operation.
        // Its value/claim custody is checked by statement sequencing, not by
        // treating all non-scalar expression statements as Unit calls.
        if statement_index.checked_add(1)?
            != program
                .statement_table
                .statements(state.statement_nodes)
                .len()
            || program.normalized_type_identity(crate::flow::call_target_return_type(
                program,
                call.target_symbol,
            )?) != program.normalized_type_identity(state.return_type)
            || checked_structural_result_type(
                program,
                &mut ShapeCollector::new(program),
                state.return_type,
                &machine_binders(program, machine),
            )
            .is_none()
        {
            return None;
        }
    }
    Some((*expression, call))
}

pub(in crate::flow::terminal_unit) fn outer_calls<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    let mut consumed = Vec::new();
    let mut structural = Vec::<&checked_trees::FlowCallFact>::new();
    let mut outer = Vec::new();
    let owner = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)?;
    let mut scalar_local_count = 0u32;
    let control_prefix =
        statement_sequence::scalar_control(program, facts, owner, state).map(|(_, prefix)| prefix);
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let Some(root) = u32::try_from(statement_index).ok().and_then(|ordinal| {
            facts
                .values
                .structural_values
                .root_at(state.symbol, ordinal)
        }) {
            if root.machine != machine {
                return None;
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
                    checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                        if !facts.flow.control.calls.is_valid(source_call) {
                            return None;
                        }
                        let call = facts.flow.control.calls.get(source_call);
                        if call.authored_expression != node.expression
                            || call.statement_index != statement_index
                            || consumed.contains(&source_call)
                        {
                            return None;
                        }
                        consumed.push(source_call);
                        structural.push(call);
                    }
                    checked_trees::CheckedStructuralValueKind::Record { fields, .. } => {
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
                    checked_trees::CheckedStructuralValueKind::Case(_)
                    | checked_trees::CheckedStructuralValueKind::Place(_) => {}
                }
            }
        }
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
                if root.machine != machine {
                    return None;
                }
                collect(
                    facts,
                    statement_index,
                    root.root,
                    calls,
                    0,
                    &mut Vec::new(),
                    &mut consumed,
                )?;
            }
            continue;
        }
        if let StatementNode::LocalData(local) = statement
            && !local.is_mutable
            && let Some(primitive_type) = program.primitive_type_reference(local.type_reference)
        {
            let role = CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: scalar_local_count,
            };
            scalar_local_count = scalar_local_count.checked_add(1)?;
            if let Some(root) = facts.values.scalar_computations.root_at(
                state.symbol,
                u32::try_from(statement_index).ok()?,
                role,
            ) {
                let plans = &facts.values.scalar_computations;
                if root.machine != machine
                    || !plans.nodes.is_valid(root.root)
                    || plans.nodes.get(root.root).authored_root != local.initial_value
                    || plans.nodes.get(root.root).primitive_type != primitive_type
                {
                    return None;
                }
                collect(
                    facts,
                    statement_index,
                    root.root,
                    calls,
                    0,
                    &mut Vec::new(),
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
            collect(
                facts,
                statement_index,
                root.root,
                calls,
                0,
                &mut Vec::new(),
                &mut consumed,
            )?;
        }
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
                            | CheckedScalarExpressionRole::StructuralValueSubject { .. }
                            | CheckedScalarExpressionRole::StructuralValuePattern { .. }
                    )
            })
        {
            if root.machine != machine {
                return None;
            }
            collect(
                facts,
                statement_index,
                root.root,
                calls,
                0,
                &mut Vec::new(),
                &mut consumed,
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
            })
            .chain(
                crate::values::call_array_constructions(
                    program,
                    &facts.flow,
                    owner,
                    state,
                    statement_index,
                )
                .into_iter()
                .map(|array| (array.source, array.expression, array.type_reference)),
            );
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
                if root.machine != machine
                    || !plans.nodes.is_valid(root.root)
                    || plans.nodes.get(root.root).authored_root != expression
                    || plans.nodes.get(root.root).primitive_type != primitive_type
                {
                    return None;
                }
                collect(
                    facts,
                    statement_index,
                    root.root,
                    calls,
                    0,
                    &mut Vec::new(),
                    &mut consumed,
                )?;
            }
        }
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let supported_destination = match program.expression_table.expression(assignment.target) {
            ExpressionNode::Member(_) => true,
            ExpressionNode::Name(name) => {
                name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && program
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1
            }
            _ => false,
        };
        if !supported_destination {
            continue;
        }
        let plans = &facts.values.scalar_computations;
        let Some(root) = plans.root_at(
            state.symbol,
            u32::try_from(statement_index).ok()?,
            CheckedScalarExpressionRole::AssignmentValue,
        ) else {
            continue;
        };
        let reference =
            validation::declared_place_type_raw(program, owner, Some(state), assignment.target)?;
        let reference = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => *referee,
            _ => reference,
        };
        let primitive = program.primitive_type_reference(reference)?;
        if root.machine != machine
            || !plans.nodes.is_valid(root.root)
            || plans.nodes.get(root.root).authored_root != assignment.value
            || plans.nodes.get(root.root).primitive_type != primitive
        {
            return None;
        }
        collect(
            facts,
            statement_index,
            root.root,
            calls,
            0,
            &mut Vec::new(),
            &mut consumed,
        )?;
    }
    for call in calls.iter().filter(|call| call.call_ordinal == 0) {
        if consumed
            .iter()
            .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), call))
        {
            continue;
        }
        if outer.iter().any(|prior: &&checked_trees::FlowCallFact| {
            prior.statement_index == call.statement_index
        }) {
            return None;
        }
        outer.push(call);
        let owner = program
            .machines()
            .iter()
            .find(|owner| owner.symbol == machine)?;
        for nested in structural_operands::for_call(program, facts, owner, state, call)? {
            if structural.iter().any(|prior| std::ptr::eq(*prior, nested)) {
                return None;
            }
            structural.push(nested);
        }
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .get(call.statement_index)?;
        if matches!(statement, StatementNode::Expression(_)) {
            let (expression, authored) =
                ordered_statement_call(program, owner, state, call.statement_index)?;
            if call.authored_expression != expression
                || call.target_symbol != authored.target_symbol
            {
                return None;
            }
        }
        if !facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| {
                root.state == state.symbol
                    && root.statement_ordinal as usize == call.statement_index
                    && matches!(
                        root.role,
                        CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                            | CheckedScalarExpressionRole::UnitCallArgument { .. }
                    )
            })
        {
            continue;
        }
        let (target, arguments) = match statement {
            StatementNode::Call(authored) => (
                authored.target_symbol,
                program
                    .statement_table
                    .expression_handles(authored.arguments),
            ),
            StatementNode::Expression(_) => {
                let (_, authored) =
                    ordered_statement_call(program, owner, state, call.statement_index)?;
                (
                    authored.target_symbol,
                    program
                        .expression_table
                        .expression_handles(authored.arguments),
                )
            }
            StatementNode::LocalData(local)
                if !local.is_mutable
                    && (call.statement_index == 0
                        || program
                            .primitive_type_reference(local.type_reference)
                            .is_some()
                        || statement_sequence::has_structural_result(
                            program, facts, owner, statement,
                        )) =>
            {
                if !program
                    .expression_table
                    .expression_is_valid(local.initial_value)
                    || call.authored_expression != local.initial_value
                {
                    return None;
                }
                let ExpressionNode::Call(authored) =
                    program.expression_table.expression(local.initial_value)
                else {
                    return None;
                };
                (
                    authored.target_symbol,
                    program
                        .expression_table
                        .expression_handles(authored.arguments),
                )
            }
            _ => return None,
        };
        if target != call.target_symbol {
            return None;
        }
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
    for call in &structural {
        let site = crate::find_call_site(
            program,
            machine,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let arguments = crate::call_site_argument_expressions(program, &site);
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
    if calls
        .iter()
        .filter(|call| call.call_ordinal != 0)
        .any(|call| {
            !consumed
                .iter()
                .any(|handle| std::ptr::eq(facts.flow.control.calls.get(*handle), call))
                && !structural.iter().any(|nested| std::ptr::eq(*nested, call))
        })
    {
        return None;
    }
    Some(outer)
}

fn collect_argument_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    call: &checked_trees::FlowCallFact,
    arguments: &[typed_trees::expression::ExpressionHandle],
    calls: &[checked_trees::FlowCallFact],
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let parameters = crate::call_target_parameters(program, call.target_symbol)?;
    let explicit_self = arguments.len()
        > parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let formals = parameters
        .iter()
        .filter(|parameter| !parameter.is_self || explicit_self)
        .collect::<Vec<_>>();
    if arguments.len() != formals.len() {
        return None;
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
    let mut roles = Vec::new();
    for (_, root) in facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            root.state == state && root.statement_ordinal as usize == call.statement_index
        })
    {
        let ordinal = match root.role {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal,
                argument_ordinal,
            }
            | CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal,
                argument_ordinal,
            } if call_ordinal as usize == call.call_ordinal => argument_ordinal,
            _ => continue,
        };
        if root.machine != machine || roles.contains(&root.role) {
            return None;
        }
        roles.push(root.role);
        let (expression, primitive) = scalar_arguments.get(ordinal as usize)?;
        let plans = &facts.values.scalar_computations;
        if !plans.nodes.is_valid(root.root)
            || plans.nodes.get(root.root).authored_root != *expression
            || plans.nodes.get(root.root).primitive_type != *primitive
        {
            return None;
        }
        collect(
            facts,
            call.statement_index,
            root.root,
            calls,
            1,
            &mut Vec::new(),
            consumed,
        )?;
    }
    Some(())
}

fn collect(
    facts: &CheckFacts,
    statement: usize,
    handle: CheckedScalarComputationHandle,
    calls: &[checked_trees::FlowCallFact],
    minimum_call_ordinal: u32,
    active: &mut Vec<CheckedScalarComputationHandle>,
    consumed: &mut Vec<arena::Handle<checked_trees::FlowCallFact>>,
) -> Option<()> {
    let plans = &facts.values.scalar_computations;
    if !plans.nodes.is_valid(handle) || active.contains(&handle) {
        return None;
    }
    active.push(handle);
    match &plans.nodes.get(handle).kind {
        CheckedScalarComputationKind::CaseMembership {
            subject:
                checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
                | checked_trees::CheckedScalarComputationStructuralArgument::Array { .. },
            ..
        } => {}
        CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
            ..
        } => {
            for field in plans.case_fields.span(subject.fields)? {
                collect(
                    facts,
                    statement,
                    field.value,
                    calls,
                    minimum_call_ordinal,
                    active,
                    consumed,
                )?;
            }
        }
        CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
            collect(
                facts,
                statement,
                *left,
                calls,
                minimum_call_ordinal,
                active,
                consumed,
            )?;
            collect(
                facts,
                statement,
                *right,
                calls,
                minimum_call_ordinal,
                active,
                consumed,
            )?;
        }
        CheckedScalarComputationKind::Qualification { operand, .. } => {
            collect(
                facts,
                statement,
                *operand,
                calls,
                minimum_call_ordinal,
                active,
                consumed,
            )?;
        }
        CheckedScalarComputationKind::Dispatch { subject, arms, .. } => {
            collect(
                facts,
                statement,
                *subject,
                calls,
                minimum_call_ordinal,
                active,
                consumed,
            )?;
            for arm in plans.dispatch_arms.span(*arms)? {
                if let checked_trees::CheckedScalarDispatchPattern::Value(pattern) = arm.pattern {
                    collect(
                        facts,
                        statement,
                        pattern,
                        calls,
                        minimum_call_ordinal,
                        active,
                        consumed,
                    )?;
                }
                collect(
                    facts,
                    statement,
                    arm.value,
                    calls,
                    minimum_call_ordinal,
                    active,
                    consumed,
                )?;
            }
        }
        CheckedScalarComputationKind::Value(_)
        | CheckedScalarComputationKind::StructuralField { .. } => {}
        CheckedScalarComputationKind::Call {
            source_call,
            call_ordinal,
            target_state,
            arguments,
            structural_arguments,
            ..
        } => {
            if !facts.flow.control.calls.is_valid(*source_call) || consumed.contains(source_call) {
                return None;
            }
            let call = facts.flow.control.calls.get(*source_call);
            if *call_ordinal < minimum_call_ordinal
                || call.call_ordinal != *call_ordinal as usize
                || call.statement_index != statement
                || call.target_symbol != *target_state
                || !call.authored_expression.is_valid()
                || !calls.iter().any(|candidate| std::ptr::eq(candidate, call))
            {
                return None;
            }
            consumed.push(*source_call);
            for operand in plans.operands.span(*arguments)? {
                collect(
                    facts,
                    statement,
                    *operand,
                    calls,
                    minimum_call_ordinal,
                    active,
                    consumed,
                )?;
            }
            for argument in plans.structural_arguments.span(*structural_arguments)? {
                if let checked_trees::CheckedScalarComputationStructuralArgument::Array {
                    elements,
                    ..
                } = argument
                {
                    for element in plans.operands.span(*elements)? {
                        collect(
                            facts,
                            statement,
                            *element,
                            calls,
                            minimum_call_ordinal,
                            active,
                            consumed,
                        )?;
                    }
                }
            }
        }
        CheckedScalarComputationKind::Apply { operands, .. } => {
            for operand in plans.operands.span(*operands)? {
                collect(
                    facts,
                    statement,
                    *operand,
                    calls,
                    minimum_call_ordinal,
                    active,
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
                    facts,
                    statement,
                    *operand,
                    calls,
                    minimum_call_ordinal,
                    active,
                    consumed,
                )?;
            }
        }
    }
    active.pop();
    Some(())
}

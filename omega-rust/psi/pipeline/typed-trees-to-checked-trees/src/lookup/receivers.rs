//! Complete projected receiver calls before validation and effect inference.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, statement::StatementNode};

mod projections;

pub(crate) fn projected_statement_receiver_place(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    before: usize,
    call: &typed_trees::statement::TableCall,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let members = program.statement_table.name_path_members(call.receiver);
    let mut receiver = projections::root(
        program,
        state,
        before,
        call.receiver_root_symbol,
        members.first()?,
    )?;
    let root = receiver.symbol;
    let mut segments = Vec::new();
    for member in members.iter().skip(1) {
        receiver = projections::field(program, receiver, member)?;
        segments.push(facts::PlaceSegment::Field {
            symbol: receiver.symbol,
        });
    }
    (receiver.symbol == call.receiver_symbol).then_some((root, segments))
}

pub(crate) fn resolve_projected_receiver_calls(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut updates = Vec::new();
    let mut expression_updates = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            for (index, statement) in statements.iter().enumerate() {
                let mut expressions = Vec::new();
                crate::monomorphization::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        program.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    let Some(receiver) = projections::expression(
                        program,
                        state,
                        index,
                        call.receiver,
                        &mut Vec::new(),
                    )?
                    else {
                        continue;
                    };
                    if receiver.depth == 0 {
                        continue;
                    }
                    let target = attached_target(
                        program,
                        receiver.type_reference,
                        call.target_symbol,
                        &call.target,
                        program.expression_table.source_span(expression),
                    )?;
                    if target.is_valid() {
                        expression_updates.push((expression, target));
                    }
                }
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                let members = program.statement_table.name_path_members(call.receiver);
                if members.len() < 2 || !call.receiver_root_symbol.is_valid() {
                    continue;
                }
                let Some(root) = projections::root(
                    program,
                    state,
                    index,
                    call.receiver_root_symbol,
                    &members[0],
                ) else {
                    continue;
                };
                let Some(receiver) = members[1..].iter().try_fold(root, |receiver, member| {
                    projections::field(program, receiver, member)
                }) else {
                    continue;
                };
                let endpoint = receiver.symbol;
                if call.receiver_symbol.is_valid() && call.receiver_symbol != endpoint {
                    return Err(vec![
                        Diagnostic::error(
                            "projected call receiver disagrees with its exact declared field",
                        )
                        .with_source_span(call.source_span),
                    ]);
                }
                let target = attached_target(
                    program,
                    receiver.type_reference,
                    call.target_symbol,
                    &call.target,
                    call.source_span,
                )?;
                updates.push((state.statement_nodes, index, endpoint, target));
            }
        }
    }
    for (statements, index, endpoint, target) in updates {
        let StatementNode::Call(call) =
            &mut program.statement_table.statements_mut(statements)[index]
        else {
            unreachable!("selected statement receiver changed shape")
        };
        call.receiver_symbol = endpoint;
        call.target_symbol = target;
    }
    for (expression, target) in expression_updates {
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression_mut(expression)
        else {
            unreachable!("selected expression call changed shape")
        };
        call.target_symbol = target;
    }
    Ok(())
}

fn attached_target(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    target: SymbolHandle,
    name: &typed_trees::name::Identifier,
    span: source::SourceSpan,
) -> Result<SymbolHandle, Vec<Diagnostic>> {
    let nominal = super::machine_symbol_from_type_reference_handle(program, type_reference);
    if !nominal.is_valid()
        || !program
            .data_definitions()
            .iter()
            .any(|definition| definition.symbol == nominal)
    {
        return Ok(target);
    }
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| machine.attached_data_symbol == nominal && nominal.is_valid())
        .flat_map(|machine| program.machine_states(machine))
        .filter(|state| state.name == *name)
        .map(|state| state.symbol);
    if target.is_valid() {
        if candidates.any(|candidate| candidate == target) {
            return Ok(target);
        }
        if program.machines().iter().any(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == target)
        }) {
            return Err(vec![
                Diagnostic::error(
                    "projected call target belongs to a different receiver declaration",
                )
                .with_source_span(span),
            ]);
        }
        return Ok(target);
    }
    let selected = candidates.next().unwrap_or_else(SymbolHandle::invalid);
    Ok(if candidates.next().is_none() {
        selected
    } else {
        SymbolHandle::invalid()
    })
}

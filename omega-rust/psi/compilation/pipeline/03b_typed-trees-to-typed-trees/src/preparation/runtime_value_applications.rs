//! Give executable data Value recipes an ordinary evaluation site.
//!
//! An authored local application binds its indices before its initializer.
//! Capture computed operands there once, so token binding, specialization,
//! contracts and effects see the same ordinary local as execution. Const and
//! proof recipes retain declaration staging; inferred result types already
//! belong to completed operands and must not be evaluated again here.

use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

type Operand = (
    HandleSpan<TypeReferenceHandle>,
    u32,
    ExpressionHandle,
    TypeReferenceHandle,
    SymbolHandle,
);

fn operands(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    seen: &mut Vec<TypeReferenceHandle>,
    result: &mut Vec<Operand>,
) {
    if !reference.is_valid() || seen.contains(&reference) {
        return;
    }
    seen.push(reference);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let Some(data) = program.unique_data_definition_by_symbol(*base_symbol) else {
                return;
            };
            let actuals = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(data);
            if actuals.len() != parameters.len() {
                return;
            }
            for (position, (parameter, actual)) in parameters.iter().zip(actuals).enumerate() {
                match parameter.kind {
                    TypeParameterKind::Value { type_reference } => {
                        if program.primitive_type_reference(type_reference).is_none() {
                            continue;
                        }
                        if let TypeReferenceNode::ConstExpression(expression) =
                            program.type_reference_table.type_reference(*actual)
                            && !matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Integer(_)
                                    | ExpressionNode::Boolean(_)
                                    | ExpressionNode::Float(_)
                            )
                        {
                            result.push((
                                *arguments,
                                position as u32,
                                *expression,
                                type_reference,
                                *base_symbol,
                            ));
                        }
                    }
                    TypeParameterKind::Type => operands(program, *actual, seen, result),
                    _ => {}
                }
            }
        }
        TypeReferenceNode::Reference { referee, .. } => operands(program, *referee, seen, result),
        TypeReferenceNode::Constrained { base_type, .. } => {
            operands(program, *base_type, seen, result)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            operands(program, *element_type, seen, result)
        }
        _ => {}
    }
}

pub(super) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut pending = Vec::new();
    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut positions = Vec::new();
            for (position, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                if local.type_is_inferred
                    || local.is_operand_capture
                    || local.relevance.is_erased()
                    || local.role != language_core::LocalBindingRole::Value
                    || !local.initial_value.is_valid()
                {
                    continue;
                }
                let mut found = Vec::new();
                operands(program, local.type_reference, &mut Vec::new(), &mut found);
                if !found.is_empty() {
                    positions.push(position);
                }
            }
            if !positions.is_empty() {
                pending.push((machine.clone(), state_index, state.clone(), positions));
            }
        }
    }
    for (machine, state_index, state, positions) in pending {
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec();
        let mut body = HandleSpan::empty();
        for (position, mut statement) in statements.into_iter().enumerate() {
            if positions.contains(&position) {
                let StatementNode::LocalData(local) = &mut statement else {
                    unreachable!();
                };
                let applied = program
                    .apply_callable_type(local.type_reference, &[], &[], machine.symbol)
                    .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
                let mut found = Vec::new();
                operands(program, applied, &mut Vec::new(), &mut found);
                for (ordinal, (arguments, offset, expression, carrier, declaration)) in
                    found.into_iter().enumerate()
                {
                    let parameters = program
                        .machine_by_symbol(declaration)
                        .map(|machine| program.machine_type_parameters(machine).to_vec())
                        .or_else(|| {
                            program
                                .unique_data_definition_by_symbol(declaration)
                                .map(|data| program.data_type_parameters(data).to_vec())
                        })
                        .expect("captured data application");
                    let substitutions = parameters
                        .iter()
                        .zip(
                            program
                                .type_reference_table
                                .type_reference_handles(arguments),
                        )
                        .map(|(parameter, actual)| (parameter.symbol, *actual))
                        .collect::<Vec<_>>();
                    let carrier = program
                        .apply_callable_type(carrier, &substitutions, &[], declaration)
                        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
                    let name = format!("__value_index#{}#{ordinal}", local.symbol.arena_index());
                    let children = program.symbols.append_generated_children_from(
                        state.symbol,
                        [(local.symbol, SymbolKind::Local, name.as_str())],
                    );
                    let symbol: SymbolHandle = symbols::SymbolTableBuilder::child_handles(children)
                        .next()
                        .expect("one Value index capture");
                    let name = typed_trees::name::Identifier::generated(name);
                    program.statement_table.push_statement(
                        &mut body,
                        StatementNode::LocalData(TableLocalData {
                            symbol,
                            name: name.clone(),
                            type_reference: carrier,
                            initial_value: expression,
                            ..Default::default()
                        }),
                    );
                    let reference = program
                        .type_reference_table
                        .insert(TypeReferenceNode::Named { symbol, name });
                    program
                        .type_reference_table
                        .set_type_reference_handle_at_offset(arguments, offset, reference);
                }
                local.type_reference = applied;
            }
            program.statement_table.push_statement(&mut body, statement);
            let source = state.statement_nodes.handle_at(position as u32);
            let target = body.handle_at(body.count() - 1);
            program
                .expression_table
                .relocate_statement_operand_captures(source, target);
        }
        program.machine_states_mut(&machine)[state_index].statement_nodes = body;
    }
    Ok(())
}

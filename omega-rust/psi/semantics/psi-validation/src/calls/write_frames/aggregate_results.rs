//! Body-derived reference leaves in an owned helper result. A return type
//! supplies structure, never a caller storage origin or permission to borrow it.

use super::path_instantiation::aggregate_arguments::{
    AggregateOrigins, ReferenceLeaf, reference_leaves_with_origins,
};
use super::stored_origins::{canonical_reference_origins, reference_leaves_before_statement};
use super::{
    Machine, StatementNode, SymbolHandle, TableCallExpression, TopLevelSymbols,
    TypeReferenceHandle, TypedTrees, machine_state_by_symbol, walk_state_write_prefix,
};
use psi_typed_trees::statement::{TransitionExit, TransitionGuardNode, TransitionTargetNode};

mod input_sources;

pub(super) fn call_result_origins(
    program: &TypedTrees,
    caller_machine: &Machine,
    call: &TableCallExpression,
    expected: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<AggregateOrigins> {
    // A stale or missing selected target cannot be repaired by its spelling.
    let (machine, state) = machine_state_by_symbol(program, call.target_symbol)?;
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if active_states.contains(&state.symbol)
        || machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
        || !program.machine_type_parameters(machine).is_empty()
        || !call.machine_arguments.is_empty()
        || call.receiver.is_valid() != machine.attached_data.is_some()
        || arguments.len()
            != parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        || !crate::type_references::type_references_match(program, state.return_type, expected)
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (result_statement, prefix) = statements.split_last()?;
    let result = match result_statement {
        StatementNode::Expression(result) => *result,
        StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always
                && transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            let TransitionTargetNode::Value(result) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            *result
        }
        _ => return None,
    };
    // Named/alternate return routes need a result relation over their graph;
    // an ordinary write summary alone cannot select their returned value.
    if prefix
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return None;
    }
    // The shared transfer freezes local aliases and carrier leaves. Incoming
    // reference bindings also anchor this exported relation and cannot be
    // exposed for replacement anywhere in the helper, including a terminal
    // sibling expression. Content writes through their owned suffixes remain
    // ordinary producer effects.
    if statements.iter().any(|statement| {
        super::statement_value_expression_roots(program, statement)
            .into_iter()
            .any(|expression| {
                super::local_aliases::expression_reborrows_stable_alias_binding(
                    program,
                    expression,
                    parameters,
                    &[],
                ) || super::local_aliases::expression_has_exclusive_borrow(
                    program,
                    expression,
                    &|target| {
                        super::reference_origins::declared_origin_root(program, machine, target)
                            .is_none()
                    },
                )
            })
    }) {
        return None;
    }
    for statement in statements {
        let source = match statement {
            StatementNode::LocalData(local)
                if super::type_reference_is_reference(program, local.type_reference) =>
            {
                local.initial_value
            }
            StatementNode::Assignment(assignment)
                if super::expression_may_rebind_mutable_alias(
                    program,
                    machine,
                    state,
                    assignment.value,
                ) =>
            {
                assignment.value
            }
            _ => continue,
        };
        if super::frame_place_path(program, source).is_some() {
            let source = match program.expression_table.expression(source) {
                super::ExpressionNode::Borrow(borrow) => borrow.target,
                _ => source,
            };
            super::reference_origins::declared_origin_root(program, machine, source)?;
        }
    }
    active_states.push(state.symbol);
    let result = (|| {
        // Include the terminal expression's producer writes and rebinding
        // fences. It cannot change the frozen local origins, so the resulting
        // context is also the context in which its leaves are constructed.
        let context = walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            active_states,
            &mut Vec::new(),
            None,
        )?;
        let returned = reference_leaves_with_origins(
            program,
            machine,
            result,
            state.return_type,
            "",
            symbols,
            active_states,
            &|expression, reference| {
                reference_leaves_before_statement(
                    program,
                    state,
                    result_statement,
                    expression,
                    reference,
                    Some(&context.stored),
                )
            },
        )?;
        let mut relative = AggregateOrigins {
            references: Vec::new(),
            cases: returned.cases,
        };
        for leaf in returned.references {
            for origin in canonical_reference_origins(
                program,
                &leaf.origin,
                &context.aliases,
                &context.stored,
            ) {
                relative.references.push(ReferenceLeaf {
                    local_suffix: leaf.local_suffix.clone(),
                    local_segments: leaf.local_segments.clone(),
                    origin,
                });
            }
        }
        Some(relative)
    })();
    active_states.pop();
    // Body recursion and finite repeated calls in caller syntax are distinct.
    // Finish the guarded body proof before substituting caller expressions;
    // any enclosing body guards remain active during that substitution.
    let relative = result?;
    let mut returned = AggregateOrigins {
        references: Vec::new(),
        cases: relative.cases,
    };
    for leaf in relative.references {
        let parameter = parameters.iter().find(|parameter| {
            leaf.origin.source.root == parameter.symbol
                || (parameter.is_self && leaf.origin.source.root == machine.symbol)
        })?;
        let actual = if parameter.is_self {
            call.receiver
        } else {
            let index = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|candidate| candidate.symbol == parameter.symbol)?;
            *arguments.get(index)?
        };
        for origin in input_sources::instantiate_source(
            program,
            caller_machine,
            machine,
            parameter,
            actual,
            &leaf.origin,
            symbols,
            active_states,
        )? {
            returned.references.push(ReferenceLeaf {
                local_suffix: leaf.local_suffix.clone(),
                local_segments: leaf.local_segments.clone(),
                origin,
            });
        }
    }
    Some(returned)
}

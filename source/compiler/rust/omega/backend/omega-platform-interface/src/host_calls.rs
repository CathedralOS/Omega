use omega_calling_conventions::HostAbiPlan;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_target::NativeTarget;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

mod collection;
mod lowering;
mod static_values;

use crate::{HostCall, HostCallArgument, HostCallArgumentKind, HostCallPlan, LoweredHostOperation};
use collection::collect_machine_host_calls;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::statement::StatementNode;

pub fn build_host_call_plan(
    program: &CheckedTrees,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
) -> Result<HostCallPlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_host_call_plan_with_workers(
        Arc::new(program.clone()),
        target,
        Arc::new(host_abi.clone()),
        workers.handle(),
    )
}

pub fn build_host_call_plan_with_workers(
    program: Arc<CheckedTrees>,
    target: NativeTarget,
    host_abi: Arc<HostAbiPlan>,
    workers: WorkerPoolHandle,
) -> Result<HostCallPlan, Diagnostic> {
    if program.machines().is_empty() {
        return Ok(HostCallPlan::default());
    }

    let machine_count = program.machines().len();
    let plan_capacity = host_call_plan_capacity(&program, &host_abi);
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines()
            .get(index)
            .expect("host-call worker index should be in range");
        let capacity = machine_host_call_plan_capacity(&program, &host_abi, machine);
        let mut machine_plan = HostCallPlan::with_capacity(
            capacity.call_count,
            capacity.call_count,
            capacity.operation_count,
            capacity.argument_count,
        );

        collect_machine_host_calls(&program, target, &host_abi, machine, &mut machine_plan)
            .map(|_| machine_plan)
    });

    let mut plan = HostCallPlan::with_capacity(
        plan_capacity.call_count,
        plan_capacity.call_count,
        plan_capacity.operation_count,
        plan_capacity.argument_count,
    );

    for machine_plan in machine_plans {
        merge_host_call_plan(&mut plan, machine_plan?);
    }

    Ok(plan)
}

#[derive(Debug, Clone, Copy, Default)]
struct HostCallPlanCapacity {
    call_count: usize,
    operation_count: usize,
    argument_count: usize,
}

fn host_call_plan_capacity(program: &CheckedTrees, host_abi: &HostAbiPlan) -> HostCallPlanCapacity {
    program
        .machines()
        .iter()
        .map(|machine| machine_host_call_plan_capacity(program, host_abi, machine))
        .fold(HostCallPlanCapacity::default(), |total, next| {
            HostCallPlanCapacity {
                call_count: total.call_count.saturating_add(next.call_count),
                operation_count: total.operation_count.saturating_add(next.operation_count),
                argument_count: total.argument_count.saturating_add(next.argument_count),
            }
        })
}

fn machine_host_call_plan_capacity(
    program: &CheckedTrees,
    host_abi: &HostAbiPlan,
    machine: &Machine,
) -> HostCallPlanCapacity {
    let max_lowering_operations = host_abi
        .platform_call_lowerings
        .iter()
        .map(|(_, lowering)| usize::try_from(lowering.operations.count()).unwrap_or(usize::MAX))
        .max()
        .unwrap_or(0);
    let mut capacity = HostCallPlanCapacity::default();

    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Call(call) = statement else {
                continue;
            };

            capacity.call_count = capacity.call_count.saturating_add(1);
            capacity.operation_count = capacity
                .operation_count
                .saturating_add(max_lowering_operations);
            capacity.argument_count = capacity.argument_count.saturating_add(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .len(),
            );
        }
    }

    capacity
}

fn merge_host_call_plan(target: &mut HostCallPlan, source: HostCallPlan) {
    target
        .unsupported_calls
        .reserve(source.unsupported_calls.len());
    target.calls.reserve(source.calls.len());
    target.operations.reserve(source.operations.len());
    target.arguments.reserve(source.arguments.len());

    let HostCallPlan {
        expressions,
        calls,
        unsupported_calls,
        operations,
        arguments,
    } = source;

    target
        .unsupported_calls
        .insert_many(unsupported_calls.into_items());

    for call in calls.into_items() {
        let operations =
            copy_lowered_host_operations(&mut target.operations, &operations, call.operations);
        let arguments = copy_host_call_arguments(target, &expressions, &arguments, call.arguments);
        target.calls.insert(HostCall {
            source_site: call.source_site,
            registration_operation: call.registration_operation,
            requirement_identity: call.requirement_identity,
            source_key: call.source_key,
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            lowering: call.lowering,
            data: call.data,
            operations,
            arguments,
            has_result: call.has_result,
        });
    }
}

fn copy_lowered_host_operations(
    target: &mut psi_arena::Arena<LoweredHostOperation>,
    source: &psi_arena::Arena<LoweredHostOperation>,
    span: psi_arena::HandleSpan<LoweredHostOperation>,
) -> psi_arena::HandleSpan<LoweredHostOperation> {
    target.insert_many(source.span_or_empty(span).iter().cloned())
}

fn copy_host_call_arguments(
    target: &mut HostCallPlan,
    source_expressions: &psi_checked_trees::expression::ExpressionTable,
    source_arguments: &psi_arena::Arena<HostCallArgument>,
    span: psi_arena::HandleSpan<HostCallArgument>,
) -> psi_arena::HandleSpan<HostCallArgument> {
    target
        .arguments
        .insert_many(source_arguments.span_or_empty(span).iter().map(|argument| {
            HostCallArgument {
                kind: match &argument.kind {
                    HostCallArgumentKind::Text(value) => {
                        HostCallArgumentKind::Text(Arc::clone(value))
                    }
                    HostCallArgumentKind::Integer(value) => HostCallArgumentKind::Integer(*value),
                    HostCallArgumentKind::Expression(expression) => {
                        HostCallArgumentKind::Expression(
                            target
                                .expressions
                                .copy_from(source_expressions, *expression),
                        )
                    }
                },
                formal: argument.formal,
                is_borrowed: argument.is_borrowed,
                expects_reference: argument.expects_reference,
                expects_address: argument.expects_address,
            }
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{PlatformCallData, build_host_abi_plan};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;

    #[test]
    fn collects_host_call_from_inherited_data_field_receiver() {
        let source = r#"
            data Game {
                console: Console;
                input: [u8; 256];
            }

            boundary trait Console {
                machine read_line(out_line: &mut [u8]);
            }

            machine Game::entry(&mut self) {
                self.console.read_line(&mut self.input);
            }

            data Main { game: Game; }

            machine Main::main(&mut self) {
                self.game.entry();
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);

        let plan = build_host_call_plan(&checked, target, &host_abi).expect("host calls");

        assert_eq!(plan.calls.len(), 1, "expected one host call: {plan:?}");
        let (_, call) = plan.calls.iter().next().expect("host call");
        assert_eq!(
            call.data,
            PlatformCallData::MutableOutputBuffer { byte_capacity: 256 }
        );
        assert_eq!(call.statement_index, 0);
        assert_eq!(call.call_ordinal, 0);
        assert!(matches!(
            call.source_site,
            Some(psi_checked_trees::NominalMachineUseSite::Statement(handle)) if handle.is_valid()
        ));
        assert!(call.registration_operation.is_valid());
        assert!(!call.requirement_identity.is_empty());
        let arguments = plan.arguments.span(call.arguments).expect("arguments");
        assert_eq!(arguments[0].formal.expect("formal").formal_ordinal, 0);
        assert_eq!(
            arguments[0].formal.expect("formal").native_parameter,
            omega_calling_conventions::callback_native_parameter_id(&call.requirement_identity, 0)
        );
    }

    #[test]
    fn host_call_plan_retains_non_utf8_literal_bytes() {
        let source = r#"
            boundary trait Console {
                machine write_line(text: &[u8]);
            }

            data Main { console: Console; }

            machine Main::main(&mut self) {
                self.console.write_line("\x80A");
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);

        let plan = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let (_, call) = plan.calls.iter().next().expect("write_line host call");
        let arguments = plan.arguments.span(call.arguments).expect("arguments");

        assert!(matches!(
            &arguments[0].kind,
            crate::HostCallArgumentKind::Text(bytes) if bytes.as_ref() == [0x80, b'A']
        ));
    }

    #[test]
    fn merge_preserves_exact_host_call_and_formal_identity() {
        let mut source = HostCallPlan::default();
        let requirement: Arc<str> = Arc::from("package::Registrar::register#exact");
        let mut call = HostCall {
            source_site: Some(psi_checked_trees::NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_arena_index(9),
            )),
            registration_operation: psi_symbols::SymbolHandle::from_arena_index(11),
            requirement_identity: requirement.clone(),
            lowering: psi_arena::Handle::from_arena_index(13),
            ..HostCall::default()
        };
        source.arguments.append_to_span(
            &mut call.arguments,
            HostCallArgument {
                kind: HostCallArgumentKind::Integer(7),
                formal: Some(crate::HostCallFormalArgumentIdentity {
                    formal_ordinal: 0,
                    native_parameter: omega_calling_conventions::callback_native_parameter_id(
                        &requirement,
                        0,
                    ),
                }),
                ..HostCallArgument::default()
            },
        );
        source.calls.insert(call.clone());

        let mut merged = HostCallPlan::default();
        merge_host_call_plan(&mut merged, source);

        let (_, retained) = merged.calls.iter().next().expect("merged call");
        assert_eq!(retained.source_site, call.source_site);
        assert_eq!(retained.registration_operation, call.registration_operation);
        assert_eq!(retained.requirement_identity, call.requirement_identity);
        assert_eq!(retained.lowering, call.lowering);
        assert_eq!(
            merged.arguments.span(retained.arguments).expect("argument")[0].formal,
            Some(crate::HostCallFormalArgumentIdentity {
                formal_ordinal: 0,
                native_parameter: omega_calling_conventions::callback_native_parameter_id(
                    &requirement,
                    0,
                ),
            })
        );
    }
}

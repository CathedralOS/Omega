use crate::abi::HostBindingMechanism;
use crate::control_flow::{OperationKind, StateFlow, StateKey};
use crate::data::NativeDataObject;
use crate::host_calls::HostCall;
use crate::host_calls::{HostCallArgument, HostCallArgumentKind};
use crate::layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use crate::object::{machine_storage_symbol_name, runtime_frame_storage_symbol_name};
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperationKind,
};
use crate::runtime_dispatch::loop_plan::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use crate::runtime_text::{RuntimeTextBuilderSegmentKind, RuntimeTextSource, RuntimeTextWriteKind};
use crate::state_guards::StateGuardLowering;
use crate::state_guards::StateGuardOperator;
use crate::state_schedule::{
    build_entry_state_schedule, scheduled_state_flow, scheduled_state_key,
};
use crate::target::{NativeTarget, ObjectFormat};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeAliasBinding {
    source_key: StateKey,
    parameter_name: ProgramName,
    expression: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            operands: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInstructionPlan {
    pub symbol: String,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub instructions: HandleSpan<SelectedInstruction>,
}

impl Default for FunctionInstructionPlan {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub kind: SelectedInstructionKind,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub source_statement: usize,
}

impl Default for SelectedInstruction {
    fn default() -> Self {
        Self {
            kind: SelectedInstructionKind::EnterFunction,
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    EnterFunction,
    EnterDispatchLoop {
        entry_dispatch_index: u32,
        terminal_dispatch_index: u32,
        current_state_slot: String,
        next_state_slot: String,
    },
    EnterDispatchCase {
        dispatch_index: u32,
        label: String,
    },
    EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering,
        operator: StateGuardOperator,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        has_storage: bool,
    },
    CompareRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    CompareRuntimeTextStorage {
        buffer_symbol: String,
        source_symbol: String,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorage {
        left_symbol: String,
        left_offset: usize,
        right_symbol: String,
        right_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorageValue {
        symbol: String,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    WriteRuntimeTextLiteralSegment {
        buffer_symbol: String,
        byte_offset: usize,
        literal: String,
    },
    AppendRuntimeTextStoredSuffix {
        buffer_symbol: String,
        buffer_offset: usize,
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer_symbol: String,
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
        literal: String,
    },
    WriteRuntimeMachineInteger {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeMachineString {
        byte_offset: usize,
        data_symbol: String,
        byte_length: usize,
    },
    ReadRuntimeTextLine {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
        byte_capacity: usize,
        syscall_number: u32,
        syscall_number_register: u8,
        supervisor_call: u16,
    },
    CopyRuntimeStorage {
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
        byte_count: usize,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    BeginPlatformCall {
        platform_call: String,
    },
    HostOperation {
        capability: String,
        operation: String,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionOperand {
    pub kind: InstructionOperandKind,
}

impl Default for InstructionOperand {
    fn default() -> Self {
        Self {
            kind: InstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionOperandKind {
    DataAddress { symbol: String },
    RuntimeMachineStringPointer { byte_offset: usize },
    RuntimeMachineStringLength { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}

pub fn build_instruction_plan(native_plan: &NativePlan) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        operands: Arena::new(),
    };

    let entry_instructions = select_entry_instructions(native_plan, &mut instruction_plan.operands);
    let instructions = instruction_plan
        .instructions
        .insert_many(entry_instructions);

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: native_plan.object.entry_symbol.clone(),
        machine: native_plan.entry_machine.clone().into(),
        state: native_plan.entry_state.clone().into(),
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
) -> Vec<SelectedInstruction> {
    let mut selected_instructions = Vec::new();
    let state_schedule_result = build_entry_state_schedule(native_plan);
    let can_inline_state_calls = state_schedule_result.is_ok()
        && native_plan
            .state_calls
            .calls
            .iter()
            .any(|(_, call)| call.required);
    let state_schedule =
        state_schedule_result.unwrap_or_else(|_| runtime_reachable_states(native_plan));

    selected_instructions.push(entry_instruction(native_plan));

    if native_plan.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            native_plan,
            operands,
            &mut selected_instructions,
        );
    } else if can_inline_state_calls {
        select_state_body_instructions(
            native_plan,
            native_plan.entry_key,
            operands,
            &mut selected_instructions,
            &mut Vec::new(),
        );
    } else {
        for scheduled_state in &state_schedule {
            if let Some(state_flow) = scheduled_state_flow(native_plan, scheduled_state) {
                let Some(machine_name) = machine_name_for_state(native_plan, state_flow) else {
                    continue;
                };
                select_state_host_calls(
                    native_plan,
                    machine_name,
                    &state_flow.name,
                    operands,
                    &mut selected_instructions,
                );
            }
        }
    }

    selected_instructions.push(exit_instruction(native_plan));
    selected_instructions
}

fn select_runtime_dispatch_loop_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: native_plan.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            current_state_slot: native_plan.runtime_dispatch_loop.current_state_slot.clone(),
            next_state_slot: native_plan.runtime_dispatch_loop.next_state_slot.clone(),
        },
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    });

    for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EnterDispatchCase {
                dispatch_index: dispatch_case.dispatch_index,
                label: dispatch_case.label.clone(),
            },
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });

        if let Some(runtime_body) = native_plan
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
            .map(|(_, body)| body)
            && let Some(operations) = native_plan
                .runtime_bodies
                .operations
                .span(runtime_body.operations)
        {
            let mut runtime_aliases = Vec::new();
            let mut runtime_static_values = Vec::new();

            for operation in operations {
                bind_runtime_operation_aliases(native_plan, operation, &mut runtime_aliases);

                select_runtime_storage_write_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    &runtime_aliases,
                    &mut runtime_static_values,
                    selected_instructions,
                );

                select_runtime_leaf_branch_expansions_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );
                select_runtime_straight_line_branch_expansions_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );

                if let Some(host_call) = host_call_for_statement(
                    native_plan,
                    operation.source_key,
                    operation.statement_index,
                ) {
                    if runtime_machine_string_descriptor_offset(native_plan, host_call).is_none()
                        && let Some((buffer_symbol, literal)) =
                            runtime_text_literal_write_for_host_call(native_plan, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer_symbol,
                                literal,
                            },
                            source_machine: host_call.machine.clone(),
                            source_state: host_call.state.clone(),
                            source_statement: host_call.statement_index,
                        });
                    }
                    select_host_call(native_plan, host_call, operands, selected_instructions);
                }
            }
        }

        if let Some(edges) = native_plan
            .runtime_dispatch_loop
            .edges
            .span(dispatch_case.edges)
        {
            for edge in edges {
                select_runtime_dispatch_edge(
                    edge,
                    &dispatch_case.machine,
                    &dispatch_case.state,
                    selected_instructions,
                );
            }
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::LeaveDispatchCase,
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::LeaveDispatchLoop,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    });
}

fn select_runtime_dispatch_edge(
    edge: &RuntimeDispatchLoopEdge,
    source_machine: &str,
    source_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator: edge.guard_operator,
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: edge.order,
    });

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

fn bind_runtime_operation_aliases(
    native_plan: &NativePlan,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    aliases: &mut Vec<RuntimeAliasBinding>,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::HostCall { .. }
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let Some(state_call) =
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        if argument.kind != crate::state_calls::StateCallArgumentKind::MutableAlias {
            continue;
        }

        let expression = strip_mutable_expression(resolve_runtime_alias_expression(
            &argument.expression,
            state_call.source_key,
            aliases,
        ));
        set_runtime_alias(
            aliases,
            RuntimeAliasBinding {
                source_key: state_call.target_key,
                parameter_name: argument.parameter_name.clone(),
                expression,
            },
        );
    }
}

fn set_runtime_alias(aliases: &mut Vec<RuntimeAliasBinding>, alias: RuntimeAliasBinding) {
    if let Some(existing_alias) = aliases.iter_mut().find(|existing_alias| {
        existing_alias.source_key == alias.source_key
            && existing_alias.parameter_name == alias.parameter_name
    }) {
        *existing_alias = alias;
    } else {
        aliases.push(alias);
    }
}

fn select_runtime_storage_write_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let RuntimeDispatchBodyOperationKind::Mutation { .. } = &operation.kind else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };

    select_runtime_mutation_writes(
        native_plan,
        dispatch_index,
        mutation.source_key,
        &operation.source_machine,
        &operation.source_state,
        mutation.statement_index,
        &mutation.target,
        &mutation.value,
        aliases,
        static_values,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_mutation_writes(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let resolved_target = resolve_runtime_alias_expression(target, source_key, aliases);

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target =
                append_place_suffix(&resolved_target, std::slice::from_ref(&field.name));
            select_runtime_mutation_writes(
                native_plan,
                dispatch_index,
                source_key,
                source_machine,
                source_state,
                statement_index,
                &field_target,
                &field.value,
                aliases,
                static_values,
                selected_instructions,
            );
        }
        return;
    }

    if let Expression::String(value) = value {
        select_runtime_string_descriptor_write(
            native_plan,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if let Some(instructions) = runtime_text_builder_write(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target,
        aliases,
    ) {
        for kind in instructions {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: statement_index,
            });
        }
        return;
    }

    let resolved_value = resolve_runtime_alias_expression(value, source_key, aliases);
    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        source_machine,
        source_state,
        &resolved_target,
        &resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: source_machine.to_owned().into(),
            source_state: source_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    let Some(value) = resolve_runtime_static_integer_value(
        native_plan,
        source_key,
        value,
        aliases,
        static_values,
    ) else {
        return;
    };
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        &resolved_target,
    ) else {
        return;
    };

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()).display_name(),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

fn runtime_text_builder_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
) -> Option<Vec<SelectedInstructionKind>> {
    runtime_text_builder_write_with_resolver(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &|expression| resolve_runtime_alias_expression(expression, source_key, aliases),
    )
}

fn runtime_text_builder_write_with_resolver(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
) -> Option<Vec<SelectedInstructionKind>> {
    let builder = native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)?;
    let segments = native_plan
        .runtime_text
        .builder_segments
        .span(builder.segments)?;
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer = runtime_text_input_buffer_for_text_place(native_plan, &resolved_target)?;
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_machine,
        source_state,
        &resolved_target,
    )?;
    if target_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
    {
        let Expression::String(prefix) = &prefix.expression else {
            return None;
        };
        let source = resolve_expression(&suffix.expression);
        let source_place = resolve_runtime_storage_place(
            native_plan,
            dispatch_index,
            source_machine,
            source_state,
            &source,
        )?;
        if source_place.byte_count != native_plan.target.pointer_size * 2 {
            return None;
        }
        return Some(vec![
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
                buffer_symbol: buffer.symbol.clone(),
                byte_offset: 0,
                literal: prefix.clone(),
            },
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
                buffer_symbol: buffer.symbol.clone(),
                buffer_offset: prefix.len(),
                source_symbol: source_place.symbol,
                source_offset: source_place.byte_offset,
                target_symbol: target_place.symbol,
                target_offset: target_place.byte_offset,
                length_delta: prefix.len(),
            },
        ]);
    }

    let mut instructions = Vec::new();
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let source = resolve_expression(&segment.expression);
                let source_place = resolve_runtime_storage_place(
                    native_plan,
                    dispatch_index,
                    source_machine,
                    source_state,
                    &source,
                )?;
                if source_place.byte_count != native_plan.target.pointer_size * 2 {
                    return None;
                }
                if source_place.symbol == target_place.symbol
                    && source_place.byte_offset == target_place.byte_offset
                {
                    instructions.push(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                        buffer_symbol: buffer.symbol.clone(),
                        target_symbol: target_place.symbol.clone(),
                        target_offset: target_place.byte_offset,
                    });
                    continue;
                }
                instructions.push(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                    buffer_symbol: buffer.symbol.clone(),
                    source_symbol: source_place.symbol,
                    source_offset: source_place.byte_offset,
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                });
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Expression::String(literal) = &segment.expression else {
                    return None;
                };
                instructions.push(SelectedInstructionKind::AppendRuntimeTextLiteral {
                    buffer_symbol: buffer.symbol.clone(),
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                    literal: literal.clone(),
                });
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return None,
        }
    }

    (!instructions.is_empty()).then_some(instructions)
}

fn runtime_storage_copy(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_machine,
        source_state,
        target,
    )?;
    let source_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_symbol: source_place.symbol,
        source_offset: source_place.byte_offset,
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

fn select_runtime_string_descriptor_write(
    native_plan: &NativePlan,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) else {
        return;
    };
    if byte_size != native_plan.target.pointer_size * 2 {
        return;
    }
    let Some(data_object) =
        string_literal_data_object(native_plan, source_key, statement_index, value)
    else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data_symbol: data_object.symbol.clone(),
            byte_length: value.len(),
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

fn string_literal_data_object<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == source_key
                && data_object.source_statement == statement_index
                && native_plan
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .is_some_and(|bytes| {
                        bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                    })
        })
        .map(|(_, data_object)| data_object)
}

fn resolve_runtime_static_integer_value(
    native_plan: &NativePlan,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &[(String, i64)],
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&native_plan.layouts, expression).or_else(|| {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }),
        Expression::Indexed(_) | Expression::Mutable(_) => {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

fn set_runtime_static_value(static_values: &mut Vec<(String, i64)>, target: String, value: i64) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        static_values.push((target, value));
    }
}

fn strip_mutable_expression(expression: Expression) -> Expression {
    match expression {
        Expression::Mutable(target) => *target,
        _ => expression,
    }
}

fn resolve_runtime_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => Expression::Mutable(Box::new(
            resolve_runtime_alias_expression(target, source_key, aliases),
        )),
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(
            omega_typed_program::expression::IndexedExpression {
                collection: resolve_runtime_alias_expression(
                    &indexed.collection,
                    source_key,
                    aliases,
                ),
                index: resolve_runtime_alias_expression(&indexed.index, source_key, aliases),
            },
        )),
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias.parameter_name == path[0])
            .map(|alias| append_place_suffix(&alias.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn select_runtime_leaf_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_leaf_branch_expansion(native_plan, expansion, selected_instructions);
    }
}

fn select_runtime_leaf_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let mut mutation_writes = Vec::new();
    select_runtime_leaf_branch_mutation_writes(native_plan, expansion, &mut mutation_writes);
    if mutation_writes.is_empty() {
        return;
    }

    if let Some((buffer_symbol, literal)) = runtime_text_literal_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CompareRuntimeTextLiteral {
                buffer_symbol,
                literal,
            },
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else if let Some(compare) = runtime_text_storage_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: compare,
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else if let Some(compare) = runtime_storage_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: compare,
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }
    selected_instructions.extend(mutation_writes);
}

fn select_runtime_straight_line_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_straight_line_branch_expansion(
            native_plan,
            expansion,
            selected_instructions,
        );
    }
}

fn select_runtime_straight_line_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if expansion.resolved_guard != omega_typed_program::statement::TransitionGuard::Always {
        return;
    }

    select_runtime_straight_line_branch_writes(native_plan, expansion, selected_instructions);
}

fn select_runtime_leaf_branch_mutation_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(target, bindings);
        let resolved_value = resolve_leaf_binding_expression(value, bindings);

        if let Some((byte_offset, byte_size, value)) = runtime_leaf_machine_integer_write(
            native_plan,
            expansion,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let Some(instructions) = runtime_text_builder_write_with_resolver(
            native_plan,
            expansion.dispatch_index,
            state_key_by_names(
                native_plan,
                &operation.source_machine,
                &operation.source_state,
            )
            .unwrap_or_default(),
            &operation.source_machine,
            &operation.source_state,
            operation.statement_index,
            &resolved_target,
            &|expression| resolve_leaf_binding_expression(expression, bindings),
        ) {
            for kind in instructions {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_machine: operation.source_machine.clone(),
                    source_state: operation.source_state.clone(),
                    source_statement: operation.statement_index,
                });
            }
            continue;
        }

        if let Some(copy) = runtime_leaf_storage_copy(
            native_plan,
            expansion,
            &operation.source_machine,
            &operation.source_state,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: copy,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
        }
    }
}

fn select_runtime_straight_line_branch_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                let resolved_target = resolve_straight_line_binding_expression(target, bindings);
                let resolved_value = resolve_straight_line_binding_expression(value, bindings);
                select_runtime_resolved_mutation_write(
                    native_plan,
                    expansion.dispatch_index,
                    &expansion.source_machine,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                target_machine,
                target_state,
                lowering: crate::state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                native_plan,
                expansion,
                operation,
                bindings,
                target_machine,
                target_state,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &crate::runtime_dispatch::branching::RuntimeStraightLineBranchOperation,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_machine: &str,
    target_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(state_call) = state_call_for_statement(
        native_plan,
        state_key_by_names(
            native_plan,
            &operation.source_machine,
            &operation.source_state,
        )
        .unwrap_or_default(),
        operation.statement_index,
    ) else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(native_plan, target_machine, target_state);
    let leaf_bindings = leaf_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter_name)| {
            let argument = arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: resolve_straight_line_binding_expression(
                    &argument.expression,
                    straight_line_bindings,
                ),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        })
        .collect::<Vec<_>>();

    let Some(operations) = state_operations(native_plan, target_machine, target_state) else {
        return;
    };
    for leaf_operation in operations {
        let Some(mutation) = state_mutation_for_statement(
            native_plan,
            state_key_by_names(native_plan, target_machine, target_state).unwrap_or_default(),
            leaf_operation.statement_index,
        ) else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(&mutation.target, &leaf_bindings);
        let resolved_value = resolve_leaf_binding_expression(&mutation.value, &leaf_bindings);
        select_runtime_resolved_mutation_write(
            native_plan,
            expansion.dispatch_index,
            &expansion.source_machine,
            target_machine,
            target_state,
            leaf_operation.statement_index,
            &resolved_target,
            &resolved_value,
            selected_instructions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_mutation_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) && let Some(value) = static_integer_value(&native_plan.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
    }
}

fn runtime_text_literal_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<(String, String)> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = match (&binary.left, &binary.right) {
        (text_place, Expression::String(literal)) => (text_place, literal),
        (Expression::String(literal), text_place) => (text_place, literal),
        _ => return None,
    };

    let buffer = runtime_text_input_buffer_for_text_place(native_plan, text_place)?;
    Some((buffer.symbol.clone(), literal.clone()))
}

fn runtime_text_storage_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }
    let operator = StateGuardOperator::Equal;

    let left_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.left,
    );
    let right_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.right,
    );
    let left_buffer = runtime_text_input_buffer_for_text_place(native_plan, &binary.left);
    let right_buffer = runtime_text_input_buffer_for_text_place(native_plan, &binary.right);
    let string_descriptor_size = native_plan.target.pointer_size * 2;

    if let (Some(source_place), Some(buffer)) = (left_place.clone(), right_buffer)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol: buffer.symbol.clone(),
            source_symbol: source_place.symbol,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    if let (Some(buffer), Some(source_place)) = (left_buffer, right_place)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol: buffer.symbol.clone(),
            source_symbol: source_place.symbol,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    None
}

fn runtime_storage_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    let operator = match binary.operator {
        omega_typed_program::expression::BinaryOperator::Equal => StateGuardOperator::Equal,
        omega_typed_program::expression::BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let left = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.left,
    );
    let right = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.right,
    );

    if let (Some(left), Some(right)) = (left.clone(), right.clone()) {
        if left.byte_count != right.byte_count {
            return None;
        }

        return Some(SelectedInstructionKind::CompareRuntimeStorage {
            left_symbol: left.symbol,
            left_offset: left.byte_offset,
            right_symbol: right.symbol,
            right_offset: right.byte_offset,
            byte_size: left.byte_count,
            operator,
        });
    }

    if let Some(place) = left
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.right)
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            symbol: place.symbol,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    if let Some(place) = right
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.left)
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            symbol: place.symbol,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    None
}

fn runtime_leaf_machine_integer_write(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &expansion.source_machine,
        target,
    )?;
    let value = static_integer_value(&native_plan.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_storage_copy(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    operation_machine: &str,
    operation_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    runtime_storage_copy(
        native_plan,
        expansion.dispatch_index,
        operation_machine,
        operation_state,
        target,
        value,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeStoragePlace {
    symbol: String,
    byte_offset: usize,
    byte_count: usize,
}

fn resolve_runtime_storage_place(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    if let Some((byte_offset, byte_count)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        expression,
    ) {
        return Some(RuntimeStoragePlace {
            symbol: machine_storage_symbol_name(&native_plan.entry_machine),
            byte_offset,
            byte_count,
        });
    }

    let expression = strip_mutable_expression(expression.clone());
    let normalized_expression;
    let expression = match &expression {
        Expression::Indexed(indexed) => {
            normalized_expression = Expression::Name(indexed_expression_path(indexed)?);
            &normalized_expression
        }
        _ => &expression,
    };
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let slot = native_plan
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_machine == source_machine
                && slot.source_state == source_state
                && slot.name == *root_name
        })
        .or_else(|| {
            native_plan
                .runtime_storage
                .frame_slots
                .iter()
                .find(|(_, slot)| slot.dispatch_index == dispatch_index && slot.name == *root_name)
        })
        .map(|(_, slot)| slot)?;
    let root_field = FieldLayout {
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_name: slot.type_name.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout(&native_plan.layouts, &root_field, suffix)?;

    Some(RuntimeStoragePlace {
        symbol: runtime_frame_storage_symbol_name(),
        byte_offset,
        byte_count: layout.size,
    })
}

fn resolve_leaf_binding_expression(
    expression: &Expression,
    bindings: &[RuntimeLeafBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_binding_expression(target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                binding.parameter_name == path[0]
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter_name == path[0])
            })
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn resolve_straight_line_binding_expression(
    expression: &Expression,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_straight_line_binding_expression(target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                binding.parameter_name == path[0]
                    && binding.kind == RuntimeStraightLineBranchBindingKind::TargetParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter_name == path[0])
            })
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn append_place_suffix(expression: &Expression, suffix: &[ProgramName]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Indexed(indexed) => {
            if let Some(mut indexed_path) = indexed_expression_path(indexed) {
                indexed_path.extend_from_slice(suffix);
                Expression::Name(indexed_path)
            } else {
                expression.clone()
            }
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
}

fn indexed_expression_path(
    indexed: &omega_typed_program::expression::IndexedExpression,
) -> Option<Vec<ProgramName>> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}

fn select_state_body_instructions(
    native_plan: &NativePlan,
    state_key: StateKey,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
    visiting: &mut Vec<StateKey>,
) {
    if visiting.contains(&state_key) {
        return;
    }

    visiting.push(state_key);

    let Some(state) = native_plan.control_flow.state_by_key(state_key) else {
        visiting.pop();
        return;
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };

    for operation in operations {
        if let Some(host_call) =
            host_call_for_statement(native_plan, state.key, operation.statement_index)
        {
            select_host_call(native_plan, host_call, operands, selected_instructions);
            continue;
        }

        let OperationKind::Call { .. } = &operation.kind else {
            continue;
        };
        let Some(state_call) =
            state_call_for_statement(native_plan, state.key, operation.statement_index)
        else {
            continue;
        };

        if state_call.target_machine.is_empty() {
            continue;
        }

        select_state_body_instructions(
            native_plan,
            state_call.target_key,
            operands,
            selected_instructions,
            visiting,
        );
    }

    visiting.pop();
}

fn select_state_host_calls(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if host_call.machine != machine_name || host_call.state != state_name {
            continue;
        }

        select_host_call(native_plan, host_call, operands, selected_instructions);
    }
}

fn host_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn state_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan crate::state_calls::StateCall> {
    native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_key == source_key && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn state_parameters(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Vec<ProgramName> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .map(|state| state.parameters.to_vec())
        .unwrap_or_default()
}

fn state_operations<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<&'plan [crate::control_flow::Operation]> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .and_then(|state| native_plan.control_flow.operations.span(state.operations))
}

fn state_key_by_names(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<StateKey> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| {
            states
                .iter()
                .find(|state| state.name == state_name)
                .map(|state| state.key)
        })
}

fn state_mutation_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan crate::state_storage::StateMutation> {
    native_plan
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

fn runtime_reachable_states(
    native_plan: &NativePlan,
) -> Vec<crate::state_schedule::ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(native_plan, &mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        push_scheduled_state(
            native_plan,
            &mut states,
            &state_call.source_machine,
            &state_call.source_state,
        );

        if !state_call.target_machine.is_empty() {
            push_scheduled_state(
                native_plan,
                &mut states,
                &state_call.target_machine,
                &state_call.target_state,
            );
        }
    }

    states
}

fn push_scheduled_state(
    native_plan: &NativePlan,
    states: &mut Vec<crate::state_schedule::ScheduledState>,
    machine: &str,
    state: &str,
) {
    let Some(key) = scheduled_state_key(native_plan, machine, state) else {
        return;
    };

    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(crate::state_schedule::ScheduledState { key });
}

fn machine_name_for_state<'plan>(
    native_plan: &'plan NativePlan,
    state_flow: &StateFlow,
) -> Option<&'plan str> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_flow.key.machine)
        .map(|(_, machine)| machine.name.as_str())
}

fn select_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall {
            platform_call: host_call.platform_call.clone(),
        },
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });

    if let Some(read_line) = runtime_text_line_read(native_plan, host_call) {
        selected_instructions.push(SelectedInstruction {
            kind: read_line,
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
        return;
    }

    let Some(operations) = native_plan.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for operation in operations {
        let operation_operands = select_host_operation_operands(
            native_plan,
            host_call,
            &operation.capability,
            &operation.operation,
        );
        let operation_operands = operands.insert_many(operation_operands);

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: operation.capability.clone(),
                operation: operation.operation.clone(),
                operands: operation_operands,
            },
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
    }

    if host_call_appends_newline(host_call)
        && runtime_machine_string_descriptor_offset(native_plan, host_call).is_some()
        && let Some(newline) = newline_data_object(native_plan)
    {
        let newline_operands = operands.insert_many(vec![
            operand(InstructionOperandKind::ImmediateInteger(1)),
            operand(InstructionOperandKind::DataAddress {
                symbol: newline.symbol.clone(),
            }),
            operand(InstructionOperandKind::ByteLength(1)),
        ]);
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: "Stdout".to_owned(),
                operation: "write".to_owned(),
                operands: newline_operands,
            },
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
    }
}

fn runtime_text_line_read(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<SelectedInstructionKind> {
    let crate::abi::PlatformCallData::MutableOutputBuffer { byte_capacity } = host_call.data else {
        return None;
    };
    let Some(HostBindingMechanism::Syscall {
        number: syscall_number,
        number_register: syscall_number_register,
        supervisor_call,
        ..
    }) = host_binding_mechanism(native_plan, "Stdin", "read")
    else {
        return None;
    };

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            buffer.source_key == host_call.source_key
                && buffer.statement_index == host_call.statement_index
        })
        .map(|(_, buffer)| buffer)?;
    let data_object = native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)?;
    let text_place = text_expression_for_buffer_target(&buffer.target)?;
    let target_place = resolve_runtime_storage_place(
        native_plan,
        0,
        &host_call.machine,
        &host_call.state,
        &text_place,
    )?;
    if target_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    Some(SelectedInstructionKind::ReadRuntimeTextLine {
        buffer_symbol: data_object.symbol.clone(),
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_capacity,
        syscall_number: *syscall_number,
        syscall_number_register: *syscall_number_register,
        supervisor_call: *supervisor_call,
    })
}

fn host_binding_mechanism<'plan>(
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBindingMechanism> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| &binding.mechanism)
}

fn select_host_operation_operands(
    native_plan: &NativePlan,
    host_call: &HostCall,
    capability: &str,
    operation: &str,
) -> Vec<InstructionOperand> {
    match (native_plan.target.object_format, capability, operation) {
        (ObjectFormat::Coff, "Stdout", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-11))]
        }
        (ObjectFormat::Coff, "Stdin", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-10))]
        }
        (_, "Stdin", "read" | "read_file") => {
            let Some(data_object) = find_data_object(native_plan, host_call) else {
                return Vec::new();
            };
            let byte_count = native_plan
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            let mut operands = Vec::new();
            if operation == "read" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(0)));
            }
            operands.push(operand(InstructionOperandKind::DataAddress {
                symbol: data_object.symbol.clone(),
            }));
            operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
            operands
        }
        (_, "Stdout", "write" | "write_file") => {
            let mut operands = Vec::new();
            if operation == "write" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(1)));
            }

            if let Some(data_object) = find_data_object(native_plan, host_call) {
                let byte_count = native_plan
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .map_or(0, |bytes| bytes.len());
                operands.push(operand(InstructionOperandKind::DataAddress {
                    symbol: data_object.symbol.clone(),
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
                return operands;
            }

            if let Some(data_object) =
                find_runtime_text_input_buffer_data_object(native_plan, host_call)
                && let Some(literal) = runtime_text_literal_for_host_call(native_plan, host_call)
                && runtime_machine_string_descriptor_offset(native_plan, host_call).is_none()
            {
                operands.push(operand(InstructionOperandKind::DataAddress {
                    symbol: data_object.symbol.clone(),
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(literal.len())));
                return operands;
            }

            if let Some(byte_offset) =
                runtime_machine_string_descriptor_offset(native_plan, host_call)
            {
                operands.push(operand(
                    InstructionOperandKind::RuntimeMachineStringPointer { byte_offset },
                ));
                operands.push(operand(
                    InstructionOperandKind::RuntimeMachineStringLength { byte_offset },
                ));
                return operands;
            }

            operands
        }
        (_, "Process", "exit" | "exit_group" | "exit_process") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(
                exit_code(host_call, native_plan),
            ))]
        }
        _ => Vec::new(),
    }
}

fn find_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == host_call.source_key
                && data_object.source_statement == host_call.statement_index
        })
        .map(|(_, data_object)| data_object)
}

fn newline_data_object(native_plan: &NativePlan) -> Option<&NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| data_object.symbol == "omega_newline")
        .map(|(_, data_object)| data_object)
}

fn find_runtime_text_input_buffer_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    let text_use = native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.source_key == host_call.source_key
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
                && text_use.source == RuntimeTextSource::StoredPlace
        })
        .map(|(_, text_use)| text_use)?;

    let text_slot = native_plan
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| {
            slot.place.display_name() == text_use.expression.display_name() && slot.has_input_buffer
        })
        .map(|(_, slot)| slot)?;

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_slot.place.display_name())
        })
        .map(|(_, buffer)| buffer)?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

fn host_call_appends_newline(host_call: &HostCall) -> bool {
    matches!(
        host_call.data,
        crate::abi::PlatformCallData::FirstTextArgument {
            append_newline: true
        }
    )
}

fn runtime_machine_string_descriptor_offset(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<usize> {
    let first_argument = native_plan
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())?;
    let HostCallArgumentKind::Expression(expression) = &first_argument.kind else {
        return None;
    };
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &host_call.machine,
        expression,
    )?;
    (byte_size == native_plan.target.pointer_size * 2).then_some(byte_offset)
}

fn text_place_for_buffer_target(
    target: &omega_typed_program::expression::Expression,
) -> Option<String> {
    text_expression_for_buffer_target(target).map(|expression| expression.display_name())
}

fn text_expression_for_buffer_target(
    target: &omega_typed_program::expression::Expression,
) -> Option<Expression> {
    match target {
        omega_typed_program::expression::Expression::Name(path) => {
            let mut text_path = path.clone();
            text_path.push(ProgramName::generated("text"));
            Some(omega_typed_program::expression::Expression::Name(text_path))
        }
        _ => None,
    }
}

fn runtime_text_input_buffer_for_text_place<'plan>(
    native_plan: &'plan NativePlan,
    text_place: &Expression,
) -> Option<&'plan NativeDataObject> {
    let text_place_name = text_place.display_name();
    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find_map(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_place_name)
                .then_some(buffer)
        })?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

fn runtime_text_literal_write_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<(String, String)> {
    let literal = runtime_text_literal_for_host_call(native_plan, host_call)?;
    let data_object = find_runtime_text_input_buffer_data_object(native_plan, host_call)?;
    Some((data_object.symbol.clone(), literal))
}

fn runtime_text_literal_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<String> {
    let append_newline = match host_call.data {
        crate::abi::PlatformCallData::FirstTextArgument { append_newline } => append_newline,
        crate::abi::PlatformCallData::MutableOutputBuffer { .. }
        | crate::abi::PlatformCallData::None => return None,
    };
    if !host_call_uses_runtime_text_input_buffer(native_plan, host_call) {
        return None;
    }

    let runtime_body = native_plan
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| {
            native_plan
                .runtime_bodies
                .operations
                .span(body.operations)
                .is_some_and(|operations| {
                    operations.iter().any(|operation| {
                        operation.source_key == host_call.source_key
                            && operation.statement_index == host_call.statement_index
                            && matches!(
                                operation.kind,
                                RuntimeDispatchBodyOperationKind::HostCall { .. }
                            )
                    })
                })
        })
        .map(|(_, body)| body)?;
    let operations = native_plan
        .runtime_bodies
        .operations
        .span(runtime_body.operations)?;
    let mut latest_static_text = None;

    for operation in operations {
        if operation.source_key == host_call.source_key
            && operation.statement_index == host_call.statement_index
            && matches!(
                operation.kind,
                RuntimeDispatchBodyOperationKind::HostCall { .. }
            )
        {
            break;
        }

        let Some(text_write) = runtime_text_write_for_operation(
            native_plan,
            operation.source_key,
            operation.statement_index,
        ) else {
            continue;
        };
        if text_write.kind != RuntimeTextWriteKind::StaticText {
            continue;
        }
        let Expression::String(value) = &text_write.value else {
            continue;
        };
        latest_static_text = Some(value.clone());
    }

    let mut literal = latest_static_text?;
    if append_newline {
        literal.push('\n');
    }
    Some(literal)
}

fn host_call_uses_runtime_text_input_buffer(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> bool {
    find_runtime_text_input_buffer_data_object(native_plan, host_call).is_some()
}

fn runtime_text_write_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan crate::runtime_text::RuntimeTextWrite> {
    native_plan
        .runtime_text
        .writes
        .iter()
        .find(|(_, write)| {
            write.source_key == source_key && write.statement_index == statement_index
        })
        .map(|(_, write)| write)
}

fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let expression = match expression {
        Expression::Mutable(target) => target.as_ref(),
        _ => expression,
    };
    let normalized_expression;
    let expression = match expression {
        Expression::Indexed(indexed) => {
            normalized_expression = Expression::Name(indexed_expression_path(indexed)?);
            &normalized_expression
        }
        _ => expression,
    };
    let Expression::Name(path) = expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let (machine_base_offset, root_field) =
        root_machine_field_layout(layouts, entry_machine, source_machine, root_name)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn root_machine_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_for_machine(layouts, entry_machine, source_machine, root_name)
        .or_else(|| {
            layouts
                .machine_layouts
                .iter()
                .find_map(|(_, machine_layout)| {
                    root_machine_field_layout_for_machine(
                        layouts,
                        entry_machine,
                        &machine_layout.name,
                        root_name,
                    )
                })
        })
}

fn root_machine_field_layout_for_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;
    Some((machine_base_offset, root_field))
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    layouts
        .fields
        .span(entry_layout.fields)?
        .iter()
        .find(|field| field.type_name == source_machine)
        .map(|field| field.offset)
}

fn resolve_nested_field_layout(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for field_name in suffix {
        let field_segment = parse_field_segment(field_name)?;
        let data_layout = layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.name == type_name)
            .map(|(_, data_layout)| data_layout)?;
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        let field = field_layout(layouts, *fields, field_segment.name)?;
        byte_offset += field.offset;
        type_name = &field.type_name;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let array = parse_array_type_name(type_name)?;
            if index >= array.length {
                return None;
            }
            let element_layout = TypeLayout {
                size: layout.size / array.length,
                alignment: layout.alignment,
            };
            byte_offset += element_layout.size * index;
            type_name = array.element_type_name;
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

struct FieldSegment<'name> {
    name: &'name str,
    index: Option<usize>,
}

fn parse_field_segment(segment: &str) -> Option<FieldSegment<'_>> {
    let Some((field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment {
            name: segment,
            index: None,
        });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment {
        name: field_name,
        index: Some(index),
    })
}

struct ArrayTypeName<'name> {
    element_type_name: &'name str,
    length: usize,
}

fn parse_array_type_name(type_name: &str) -> Option<ArrayTypeName<'_>> {
    let inner = type_name.strip_prefix('[')?.strip_suffix(']')?;
    let (element_type_name, length) = inner.split_once(';')?;
    Some(ArrayTypeName {
        element_type_name: element_type_name.trim(),
        length: length.trim().parse::<usize>().ok()?,
    })
}

fn field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name == field_name)
}

fn enum_variant_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let [type_name, variant_name] = path.as_slice() else {
        return None;
    };
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants } = &data_layout.shape else {
        return None;
    };
    variants
        .iter()
        .position(|variant| variant == variant_name)
        .and_then(|index| i64::try_from(index).ok())
}

fn static_integer_value(layouts: &LayoutPlan, expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => enum_variant_value(layouts, expression),
    }
}

fn exit_code(host_call: &HostCall, native_plan: &NativePlan) -> i64 {
    first_argument(host_call, native_plan)
        .and_then(|argument| match &argument.kind {
            HostCallArgumentKind::Integer(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(0)
}

fn first_argument<'plan>(
    host_call: &HostCall,
    native_plan: &'plan NativePlan,
) -> Option<&'plan HostCallArgument> {
    native_plan
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
}

fn entry_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    }
}

fn exit_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    }
}

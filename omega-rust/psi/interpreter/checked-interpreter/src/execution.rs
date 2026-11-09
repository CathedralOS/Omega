/*
    State-machine execution, transitions, statements, assignments, cleanup, and
    host boundary dispatch for the checked-tree interpreter.

    We step through states and guarded transitions, capture assignment
    destinations before evaluating replacements, run normal and replacement
    cleanup, and dispatch Console and boundary adapter calls.
*/

pub(crate) mod assignments {
    use super::replacement_cleanup::ReplacementStorage;
    use crate::interpreter::evaluator::{
        ArithmeticDomain, Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame,
        Halt, MutableRecordRecastWrite, ReferenceStorage, Value, expression_table, trap,
        type_reference_table,
    };
    use validation::{assignment_stored_value_type, declared_place_type_raw};

    enum AssignmentDestination {
        Cell(Cell),
        ArrayWindow {
            carrier: Cell,
            start: usize,
            end: usize,
            element_type: typed_trees::types::TypeReferenceHandle,
        },
        PackedByte {
            carrier: Cell,
            position: usize,
        },
        ScalarRecast {
            cell: Cell,
            recast: ReferenceStorage,
        },
        RecordRecast(MutableRecordRecastWrite),
    }

    impl Evaluator<'_> {
        pub(crate) fn assign(
            &mut self,
            statement_index: usize,
            assignment: &typed_trees::statement::TableAssignment,
            frame: &mut Frame,
        ) -> EvalResult<()> {
            let target = assignment.target;
            let replacement = assignment.value;
            let receiving = self
                .program
                .machine_by_symbol(frame.machine_symbol)
                .and_then(|machine| {
                    self.program
                        .state_in_machine(machine, frame.state_symbol)
                        .and_then(|state| {
                            assignment_stored_value_type(self.program, machine, state, assignment)
                        })
                })
                .ok_or_else(|| {
                    Halt::Unsupported("assignment has no selected receiving type".to_owned())
                })?;
            let coercion = if validation::reference_shell(self.program, receiving).is_none() {
                self.assignment_target_coercion(target, frame)
            } else {
                None
            };
            let destination = self.capture_assignment_destination(target, receiving, frame)?;
            let cleanup = self.capture_replacement_cleanup(
                statement_index,
                match &destination {
                    AssignmentDestination::Cell(cell) => {
                        Some(ReplacementStorage::Cell(cell.clone()))
                    }
                    AssignmentDestination::ArrayWindow {
                        carrier,
                        start,
                        end,
                        ..
                    } => Some(ReplacementStorage::ArrayWindow {
                        carrier: carrier.clone(),
                        start: *start,
                        end: *end,
                    }),
                    _ => None,
                },
                frame,
            )?;
            if let ExpressionNode::Atomic(atomic) = expression_table(self.program)
                .expression(replacement)
                .clone()
                && matches!(
                    atomic.ordering,
                    language_core::AtomicOrderingPlan::ReadModifyWrite(_)
                        | language_core::AtomicOrderingPlan::Swap(_)
                        | language_core::AtomicOrderingPlan::CompareExchange { .. }
                )
            {
                if !atomic.result.is_valid() {
                    return trap("atomic RMW carrier lost its result place");
                }
                let AssignmentDestination::Cell(cell) = &destination else {
                    return trap("atomic RMW requires an ordinary scalar place");
                };
                let prior = cell.borrow().clone();
                let result = self.resolve_place(atomic.result, frame)?;
                let result = self.deref_cell(result);
                *result.borrow_mut() = prior;
            }
            let value_type = type_reference_table(self.program).peeled_type_reference(receiving);
            let value = match &destination {
                AssignmentDestination::ArrayWindow { element_type, .. } => {
                    if matches!(
                        self.program.expression_table.expression(replacement),
                        ExpressionNode::ArrayLiteral(_)
                    ) {
                        self.eval_array_literal_at_element_type(replacement, *element_type, frame)?
                    } else {
                        self.eval_expression_at_type(replacement, value_type, frame)?
                    }
                }
                _ => self.eval_expression_at_type(replacement, value_type, frame)?,
            };
            let copy_array =
                matches!(value, Value::Array(_)) && self.declared_type_is_fixed_array(receiving);
            let value = if matches!(value, Value::Struct { .. })
                || copy_array
                || matches!(destination, AssignmentDestination::ArrayWindow { .. })
            {
                value.deep_clone_with(&|value| self.allocate_cell(value))?
            } else {
                value
            };
            if let AssignmentDestination::ArrayWindow { start, end, .. } = &destination {
                let Value::Array(elements) = &value else {
                    return trap("array window replacement is not an array value");
                };
                if elements.len() != end - start {
                    return trap("array window replacement has a different element count");
                }
            }
            self.execute_replacement_cleanup(cleanup)?;
            match destination {
                AssignmentDestination::ArrayWindow {
                    carrier,
                    start,
                    end,
                    ..
                } => self.write_array_window(carrier, start, end, value),
                AssignmentDestination::RecordRecast(write) => {
                    self.write_mutable_record_recast_target(write, value)
                }
                AssignmentDestination::ScalarRecast { cell, recast } => {
                    let primitive = recast.target().ok_or_else(|| {
                        Halt::Trap("record recast reached the scalar write seam".to_owned())
                    })?;
                    let value =
                        self.coerce_scalar_with(value, primitive, ArithmeticDomain::Exact)?;
                    match recast {
                        ReferenceStorage::Direct { source, .. } => {
                            *cell.borrow_mut() = self.eval_recast(value, Some(source))?;
                            Ok(())
                        }
                        ReferenceStorage::ByteRegion {
                            cells,
                            offset,
                            target,
                        } => self.write_scalar_byte_region(&cells, offset, target, value),
                        ReferenceStorage::AggregateByteRegion { .. }
                        | ReferenceStorage::AggregateTyped { .. } => {
                            trap("aggregate recast reached the scalar write seam")
                        }
                    }
                }
                destination => {
                    let value = match coercion {
                        Some((primitive, domain)) => {
                            self.coerce_scalar_with(value, primitive, domain)?
                        }
                        None => value,
                    };
                    match destination {
                        AssignmentDestination::Cell(cell) => {
                            *cell.borrow_mut() = value;
                            Ok(())
                        }
                        AssignmentDestination::PackedByte { carrier, position } => {
                            let byte = value.as_int().ok_or_else(|| {
                                Halt::Trap("carrier byte write value is not an integer".to_owned())
                            })? as u8;
                            let carrier = carrier.borrow();
                            let Value::Str(text) = &*carrier else {
                                return trap("carrier byte write lost its backing storage");
                            };
                            text.write_byte(position, byte).map_err(|length| {
                            Halt::Trap(format!(
                                "carrier byte write index {position} out of bounds (len {length})"
                            ))
                        })
                        }
                        _ => trap("recast destination reached an ordinary write"),
                    }
                }
            }
        }

        fn capture_assignment_destination(
            &mut self,
            target: ExpressionHandle,
            receiving: typed_trees::types::TypeReferenceHandle,
            frame: &mut Frame,
        ) -> EvalResult<AssignmentDestination> {
            if let Some((carrier, start, end, element_type)) =
                self.capture_array_window(target, frame)?
            {
                return Ok(AssignmentDestination::ArrayWindow {
                    carrier,
                    start,
                    end,
                    element_type,
                });
            }
            if let Some(write) = self.prepare_mutable_record_recast_write(target, frame)? {
                return Ok(AssignmentDestination::RecordRecast(write));
            }
            let cell = if let ExpressionNode::Indexed(indexed) =
                self.program.expression_table.expression(target).clone()
                && !matches!(
                    self.program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                let carrier = self.resolve_place(indexed.collection, frame)?;
                let carrier = self.deref_cell(carrier);
                let position = self.eval_index(indexed.index, frame)?;
                if matches!(&*carrier.borrow(), Value::Str(_)) {
                    return Ok(AssignmentDestination::PackedByte { carrier, position });
                }
                self.element_cell(&carrier, position)?
            } else {
                self.resolve_place(target, frame)?
            };
            if validation::reference_shell(self.program, receiving).is_some() {
                let mut cell = cell;
                let mut declared = self
                    .program
                    .machine_by_symbol(frame.machine_symbol)
                    .and_then(|machine| {
                        self.program
                            .state_in_machine(machine, frame.state_symbol)
                            .and_then(|state| {
                                declared_place_type_raw(self.program, machine, Some(state), target)
                            })
                    })
                    .ok_or_else(|| {
                        Halt::Unsupported(
                            "reference replacement has no declared destination type".to_owned(),
                        )
                    })?;
                while self.program.normalized_type_identity(declared)
                    != self.program.normalized_type_identity(receiving)
                {
                    let shell =
                        validation::reference_shell(self.program, declared).ok_or_else(|| {
                            Halt::Unsupported(
                                "reference replacement lost its destination projection".to_owned(),
                            )
                        })?;
                    let value = cell.borrow().clone();
                    let Value::Ref(reference) = value else {
                        return Err(Halt::Unsupported(
                            "reference destination has no retained outer borrow".to_owned(),
                        ));
                    };
                    cell = reference.cell;
                    declared = shell.referee;
                }
                return Ok(AssignmentDestination::Cell(cell));
            }
            if let Value::Ref(reference) = cell.borrow().clone()
                && let Some(storage) = reference.storage
            {
                return Ok(match storage {
                    recast @ (ReferenceStorage::Direct { .. }
                    | ReferenceStorage::ByteRegion { .. }) => AssignmentDestination::ScalarRecast {
                        cell: reference.cell,
                        recast,
                    },
                    recast @ (ReferenceStorage::AggregateByteRegion { projection, .. }
                    | ReferenceStorage::AggregateTyped { projection, .. }) => {
                        AssignmentDestination::RecordRecast(MutableRecordRecastWrite {
                            recast,
                            projection,
                        })
                    }
                });
            }
            Ok(AssignmentDestination::Cell(self.deref_cell(cell)))
        }
    }
}

pub(crate) mod boundary_adapter_dispatch {
    use crate::interpreter::evaluator::{
        EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value, trap,
    };
    impl<'program> Evaluator<'program> {
        pub(crate) fn try_selected_named_call(
            &mut self,
            receiver_symbol: SymbolHandle,
            target_symbol: SymbolHandle,
            machine_arguments: &[typed_trees::expression::StaticMachineArgument],
            arguments: &[ExpressionHandle],
            source: Option<validation::CallSite<'_>>,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if receiver_symbol.is_valid()
                && let Some(dispatch) = self.selected_boundary_adapter(
                    receiver_symbol,
                    target_symbol,
                    machine_arguments,
                )
            {
                return self
                    .run_boundary_adapter(dispatch, arguments, source, frame)
                    .map(Some);
            }
            self.try_operator_application(
                target_symbol,
                ExpressionHandle::invalid(),
                arguments,
                frame,
            )
        }

        pub(crate) fn selected_boundary_adapter(
            &self,
            receiver: SymbolHandle,
            requirement: SymbolHandle,
            machine_arguments: &[typed_trees::expression::StaticMachineArgument],
        ) -> Option<checked_trees::CheckedBoundaryAdapterDispatch> {
            let requirement = self
                .program
                .closed_requirement_application(requirement)
                .map_or(requirement, |application| application.template);
            self.boundary_adapter_dispatch
                .iter()
                .filter(|dispatch| {
                    dispatch.receiver == receiver && dispatch.requirement == requirement
                })
                .find(|dispatch| {
                    dispatch.family_tuple.is_empty()
                        || machine_arguments
                            .iter()
                            .map(|argument| self.program.static_const_argument_identity(argument))
                            .collect::<Option<Vec<_>>>()
                            .is_some_and(|tuple| tuple.as_slice() == dispatch.family_tuple.as_ref())
                })
                .cloned()
        }

        pub(crate) fn run_boundary_adapter(
            &mut self,
            dispatch: checked_trees::CheckedBoundaryAdapterDispatch,
            arguments: &[ExpressionHandle],
            source: Option<validation::CallSite<'_>>,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            self.run_selected_realization(dispatch.realization_state, arguments, source, frame)
        }

        pub(crate) fn run_selected_realization(
            &mut self,
            realization_state: SymbolHandle,
            arguments: &[ExpressionHandle],
            source: Option<validation::CallSite<'_>>,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let (machine, state) = self
                .resolve_entry_state_symbol(realization_state)
                .ok_or_else(|| {
                    Halt::Trap("selected realization lost its exact entry state".into())
                })?;
            let evaluated = self.eval_state_arguments(state, arguments, source, frame)?;
            let guard_depth = self.guard_depth;
            self.guard_depth = 0;
            let result = self.run_state_collect(machine, state, evaluated);
            self.guard_depth = guard_depth;
            result.map(|value| value.unwrap_or(Value::Unit))
        }

        pub(crate) fn run_dynamic_slot(
            &mut self,
            slot: &typed_trees::dynamic_traits::DynamicSlotSignature,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let Some((selector, arguments)) = arguments.split_first() else {
                return trap("dynamic slot call carries no selector operand");
            };
            let selector = self.eval_expression(*selector, frame)?;
            let Value::Struct { type_symbol, .. } = selector else {
                return trap("dynamic slot selector carries no implementation identity");
            };
            let selected_state =
                self.program
                    .conformance_by_symbol(type_symbol)
                    .and_then(|conformance| {
                        conformance
                            .rows
                            .iter()
                            .find(|row| {
                                row.requirement == slot.requirement
                                    || row.requirement == slot.call_requirement
                            })
                            .map(|row| row.realization_state)
                    });
            if let Some(realization_state) = selected_state {
                return self.run_selected_realization(realization_state, arguments, None, frame);
            }
            let machine = self
                .find_machine_by_symbol(type_symbol)
                .ok_or_else(|| Halt::Trap("dynamic selector names no realized machine".into()))?;
            let conformance = self
                .program
                .machine_trait_conformances(machine)
                .iter()
                .find(|conformance| {
                    conformance.requirement_symbol == slot.requirement
                        || conformance.requirement_symbol == slot.call_requirement
                });
            let state_name = conformance
                .and_then(|conformance| {
                    conformance
                        .alias
                        .as_ref()
                        .or(conformance.requirement.as_ref())
                })
                .or_else(|| {
                    self.program
                        .dynamic_slot_requirement(slot)
                        .map(|signature| &signature.name)
                })
                .map(|name| name.as_str().to_owned())
                .ok_or_else(|| {
                    Halt::Trap("dynamic selector's implementation names no realizing state".into())
                })?;
            let state = self.find_state(machine, &state_name).ok_or_else(|| {
                Halt::Trap(format!(
                    "dynamic realization state `{state_name}` is absent from `{}`",
                    machine.name.as_str()
                ))
            })?;
            let destinations = self
                .program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>();
            if destinations.len() != arguments.len() {
                return trap("dynamic slot argument count differs from its realized signature");
            }
            let mut evaluated = Vec::with_capacity(destinations.len());
            for (argument, destination) in arguments.iter().zip(&destinations) {
                evaluated.push(self.eval_state_argument(*argument, *destination, frame)?);
            }
            let guard_depth = self.guard_depth;
            self.guard_depth = 0;
            let result = self.run_state_collect(machine, state, evaluated);
            self.guard_depth = guard_depth;
            result.map(|value| value.unwrap_or(Value::Unit))
        }
    }
}

pub(crate) mod boundary_console {
    use crate::interpreter::evaluator::{EvalResult, Evaluator, Halt, SymbolHandle, Value};
    use typed_trees::signature::StateSignature;
    use typed_trees::trait_definition::TraitDefinition;
    use validation::{
        exact_compiler_intrinsic_boundary_requirement, is_canonical_process_exit_requirement,
    };
    impl<'program> Evaluator<'program> {
        pub(crate) fn read_stdin_byte_value(&mut self, target: SymbolHandle) -> EvalResult<Value> {
            let requirement = exact_compiler_intrinsic_boundary_requirement(self.program, target)
                .map_or(target, |(requirement, _)| requirement);
            let type_symbol = self
                .program
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.name.as_str() == "Console"
                })
                .flat_map(|definition| self.program.trait_machine_signatures(definition))
                .find(|signature| {
                    signature.symbol == requirement
                        && signature.name.as_str() == "read_byte"
                        && self
                            .program
                            .state_signature_parameters(signature)
                            .is_empty()
                })
                .and_then(|signature| {
                    validation::exact_byte_read_result_type(self.program, signature.return_type)
                })
                .ok_or_else(|| {
                    Halt::Unsupported(
                        "byte input has no exact supported result declaration".to_owned(),
                    )
                })?;
            if self.stdin_cursor < self.stdin.len() {
                let byte = self.stdin[self.stdin_cursor];
                self.stdin_cursor += 1;
                Ok(Value::Enum {
                    type_symbol,
                    variant_name: "Byte".into(),
                    payload: vec![(
                        "value".into(),
                        self.allocate_cell(Value::Int(i64::from(byte)))?,
                    )],
                })
            } else {
                Ok(Value::Enum {
                    type_symbol,
                    variant_name: "Eof".into(),
                    payload: Vec::new(),
                })
            }
        }

        pub(crate) fn read_stdin_line(&mut self) -> String {
            let mut line = String::new();
            while self.stdin_cursor < self.stdin.len() {
                let byte = self.stdin[self.stdin_cursor];
                self.stdin_cursor += 1;
                if byte == b'\n' {
                    break;
                }
                if byte == b'\r' {
                    if self.stdin_cursor < self.stdin.len()
                        && self.stdin[self.stdin_cursor] == b'\n'
                    {
                        self.stdin_cursor += 1;
                    }
                    break;
                }
                line.push(byte as char);
            }
            line
        }

        pub(crate) fn boundary_call_requirement(
            &self,
            target_symbol: SymbolHandle,
        ) -> Option<(&'program TraitDefinition, &'program StateSignature)> {
            self.boundary_trait_requirement(target_symbol).or_else(|| {
                let (requirement, _provider) =
                    exact_compiler_intrinsic_boundary_requirement(self.program, target_symbol)?;
                self.boundary_trait_requirement(requirement)
            })
        }

        pub(crate) fn boundary_trait_requirement(
            &self,
            symbol: SymbolHandle,
        ) -> Option<(&'program TraitDefinition, &'program StateSignature)> {
            if !symbol.is_valid() {
                return None;
            }
            self.program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .find_map(|definition| {
                    self.program
                        .trait_machine_signature_by_symbol(definition, symbol)
                        .map(|signature| (definition, signature))
                })
        }

        pub(crate) fn is_canonical_process_exit_requirement(
            &self,
            trait_symbol: SymbolHandle,
            requirement_symbol: SymbolHandle,
        ) -> bool {
            is_canonical_process_exit_requirement(self.program, trait_symbol, requirement_symbol)
        }
    }
}

pub(crate) mod build_time_arguments {
    use crate::interpreter::evaluator::{
        EvalResult, Evaluator, TypeReferenceHandle, TypeReferenceNode, Value, trap,
        type_reference_table,
    };

    impl Evaluator<'_> {
        pub(crate) fn bind_build_time_argument(
            &self,
            value: &mut Value,
            reference: TypeReferenceHandle,
        ) -> EvalResult<()> {
            let reference =
                type_reference_table(self.program).unconstrained_type_reference(reference);
            match self.program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    return self.bind_build_time_argument(value, *referee);
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    if let Value::Array(elements) = value {
                        for element in elements {
                            self.bind_build_time_argument(
                                &mut element.borrow_mut(),
                                *element_type,
                            )?;
                        }
                    }
                    return Ok(());
                }
                _ => {}
            }
            let Some(data) = self.program.closed_data_field_definition(reference) else {
                return Ok(());
            };
            if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
                return Ok(());
            }
            match value {
                Value::Struct {
                    type_symbol,
                    fields,
                    ..
                } => {
                    for (name, cell) in fields {
                        let Some(field) = self
                            .program
                            .live_data_fields(self.program.data_members(data))
                            .find(|field| field.name.as_str() == name.as_ref())
                        else {
                            return trap("build-time record argument has an undeclared field");
                        };
                        self.bind_build_time_argument(
                            &mut cell.borrow_mut(),
                            field.type_reference,
                        )?;
                    }
                    *type_symbol = data.symbol;
                }
                Value::Enum {
                    type_symbol,
                    variant_name,
                    payload,
                } => {
                    let selected_name = variant_name
                        .strip_prefix(data.name.as_str())
                        .and_then(|tail| tail.strip_prefix("::"))
                        .unwrap_or(variant_name.as_ref());
                    let Some(case) = self.program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(case) = member else {
                            return None;
                        };
                        (case.name.as_str() == selected_name).then_some(case)
                    }) else {
                        return trap(format!(
                            "build-time case argument `{}` has no case in its exact receiving application `{}`",
                            variant_name,
                            self.program.display_type_reference(reference)
                        ));
                    };
                    for (name, cell) in payload {
                        let Some(field) = self
                            .program
                            .data_payload_fields(case)
                            .iter()
                            .chain(
                                self.program
                                    .live_data_fields(self.program.data_members(data)),
                            )
                            .find(|field| field.name.as_str() == name.as_ref())
                        else {
                            return trap(
                                "build-time case argument has an undeclared payload field",
                            );
                        };
                        self.bind_build_time_argument(
                            &mut cell.borrow_mut(),
                            field.type_reference,
                        )?;
                    }
                    *type_symbol = data.symbol;
                    *variant_name = case.name.as_str().into();
                }
                _ => {}
            }
            Ok(())
        }
    }
}

pub(crate) mod host_dispatch {
    use crate::interpreter::evaluator::{
        DataMember, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, TableCall,
        Value, expression_table, statement_table, trap, unsupported,
    };
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
    use validation::checked_shape_data_declaration;
    use validation::exact_byte_read_result_type;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HostOperation {
        ExitProcess,
        ConsoleWrite { error_stream: bool, line: bool },
        ReadByte,
        WriteByte,
        ReadLine,
        VirtualTimeRead(&'static str),
        TickCount,
    }

    impl HostOperation {
        fn from_name(name: &str) -> Option<Self> {
            Some(match name {
                "exit_process" => Self::ExitProcess,
                "write" => Self::ConsoleWrite {
                    error_stream: false,
                    line: false,
                },
                "write_line" => Self::ConsoleWrite {
                    error_stream: false,
                    line: true,
                },
                "write_error" => Self::ConsoleWrite {
                    error_stream: true,
                    line: false,
                },
                "write_error_line" => Self::ConsoleWrite {
                    error_stream: true,
                    line: true,
                },
                "read_byte" => Self::ReadByte,
                "write_byte" => Self::WriteByte,
                "read_line" => Self::ReadLine,
                "monotonic_ticks" => Self::VirtualTimeRead("monotonic_ticks"),
                "monotonic_ticks_per_second" => Self::VirtualTimeRead("monotonic_ticks_per_second"),
                "wall_clock_raw" => Self::VirtualTimeRead("wall_clock_raw"),
                "wall_clock_units_per_second" => {
                    Self::VirtualTimeRead("wall_clock_units_per_second")
                }
                "wall_clock_epoch_offset_seconds" => {
                    Self::VirtualTimeRead("wall_clock_epoch_offset_seconds")
                }
                "tick_count" => Self::TickCount,
                _ => return None,
            })
        }
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn virtual_time_host_value(&self, target: &str) -> Option<Value> {
            match target {
                "monotonic_ticks" => Some(Value::Int(self.virtual_ticks)),
                "monotonic_ticks_per_second" | "wall_clock_units_per_second" => {
                    Some(Value::Int(1000))
                }
                "wall_clock_raw" => 1_767_225_600_000_i64
                    .checked_add(self.virtual_ticks)
                    .map(Value::Int),
                "wall_clock_epoch_offset_seconds" => Some(Value::Int(0)),
                _ => None,
            }
        }

        pub(crate) fn try_host_call(
            &mut self,
            call: &TableCall,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            self.try_host_invocation(
                call.target_symbol,
                call.target.as_str(),
                statement_table(self.program).expression_handles(call.arguments),
                frame,
            )
        }

        pub(crate) fn try_host_invocation(
            &mut self,
            target_symbol: SymbolHandle,
            target_name: &str,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            if self.source_selection_protocol(target_symbol).is_some() {
                return trap(
                    "Build binding protocol requires source-issued designated product operands",
                );
            }
            if let Some(value) =
                self.try_virtual_clock_operation(target_symbol, arguments, frame)?
            {
                return Ok(Some(value));
            }
            let intrinsic_method = self.exact_console_intrinsic_host_method(target_symbol);
            if intrinsic_method.is_none()
                && self.boundary_trait_requirement(target_symbol).is_none()
            {
                return Ok(None);
            }
            self.host_boundary_touched = true;

            let target = intrinsic_method.unwrap_or(target_name);
            let Some(operation) = HostOperation::from_name(target) else {
                return unsupported(format!("host boundary call `{target}` not yet supported"));
            };
            self.serve_host_operation(operation, target_symbol, arguments, frame)
                .map(Some)
        }

        pub(crate) fn try_host_value_call(
            &mut self,
            call: &typed_trees::expression::TableCallExpression,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some((_, requirement)) = self.boundary_trait_requirement(call.target_symbol) else {
                return Ok(None);
            };
            let Some(operation) = HostOperation::from_name(requirement.name.as_str()) else {
                return Ok(None);
            };
            self.host_boundary_touched = true;
            let arguments = expression_table(self.program)
                .expression_handles(call.arguments)
                .to_vec();
            self.serve_host_operation(operation, call.target_symbol, &arguments, frame)
                .map(Some)
        }

        pub(crate) fn try_virtual_clock_operation(
            &mut self,
            requirement: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some(binding) = self
                .virtual_clock_bindings
                .iter()
                .find(|binding| binding.matches_target(requirement))
                .cloned()
            else {
                return Ok(None);
            };
            self.host_boundary_touched = true;
            if arguments.len() != binding.operands.len() {
                return unsupported(
                    "virtual wait argument count differs from its binding".to_owned(),
                );
            }
            let mut values = Vec::with_capacity(arguments.len());
            for (argument, primitive) in arguments.iter().zip(&binding.parameter_types) {
                values.push(self.eval_expression_with_destination(
                    *argument,
                    Some(*primitive),
                    frame,
                )?);
            }
            let mut milliseconds = 0;
            for (value, operand) in values.iter().zip(&binding.operands) {
                let Value::Int(integer) = value else {
                    return unsupported("virtual wait expects integer operands".to_owned());
                };
                match operand {
                    crate::VirtualClockOperand::Milliseconds => {
                        let crate::VirtualClockOperation::Wait {
                            maximum_milliseconds,
                            ..
                        } = binding.operation
                        else {
                            return unsupported(
                                "virtual clock read has a duration operand".to_owned(),
                            );
                        };
                        if *integer < 0 || *integer > maximum_milliseconds {
                            return unsupported(
                                "virtual wait duration is outside its finite range".to_owned(),
                            );
                        }
                        milliseconds = *integer;
                    }
                    crate::VirtualClockOperand::ExactInteger(expected) if *integer != *expected => {
                        return unsupported(
                            "virtual clock requires its exact capability operand".to_owned(),
                        );
                    }
                    crate::VirtualClockOperand::ExactInteger(_)
                    | crate::VirtualClockOperand::Ignored => {}
                }
            }
            let value = match binding.operation {
                crate::VirtualClockOperation::Wait { result, .. } => {
                    self.advance_virtual_clock(milliseconds)?;
                    match result {
                        crate::VirtualClockResult::Unit => Value::Unit,
                        crate::VirtualClockResult::Integer(_) => Value::Int(0),
                    }
                }
                crate::VirtualClockOperation::Read {
                    units_per_millisecond,
                    ..
                } => {
                    let scaled = self
                        .virtual_ticks
                        .checked_mul(units_per_millisecond)
                        .ok_or_else(|| {
                            Halt::Unsupported("virtual clock read overflowed".to_owned())
                        })?;
                    if scaled < 0 {
                        return unsupported(
                            "virtual clock read cannot return negative elapsed time".to_owned(),
                        );
                    }
                    Value::Int(scaled)
                }
            };
            Ok(Some(value))
        }

        pub(crate) fn advance_virtual_clock(&mut self, milliseconds: i64) -> EvalResult<()> {
            self.virtual_ticks = self
                .virtual_ticks
                .checked_add(milliseconds)
                .ok_or_else(|| Halt::Unsupported("virtual clock duration overflowed".to_owned()))?;
            Ok(())
        }

        fn serve_host_operation(
            &mut self,
            operation: HostOperation,
            target_symbol: SymbolHandle,
            arguments: &[ExpressionHandle],
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            match operation {
                HostOperation::ExitProcess => {
                    if let Some((definition, requirement)) =
                        self.boundary_call_requirement(target_symbol)
                        && definition.name.as_str() == "ProcessExit"
                        && !self.is_canonical_process_exit_requirement(
                            definition.symbol,
                            requirement.symbol,
                        )
                    {
                        return unsupported(
                        "boundary call `exit_process` names a non-canonical `ProcessExit` declaration; only the exact toolchain requirement ends the simulated domain"
                            .to_owned(),
                    );
                    }
                    let code = if let Some(first) = arguments.first() {
                        self.eval_expression(*first, frame)?
                            .as_int()
                            .ok_or_else(|| Halt::Trap("exit_process arg not integer".to_owned()))?
                    } else {
                        0
                    };
                    Err(Halt::Exit(code as i32))
                }
                HostOperation::ConsoleWrite { error_stream, line } => {
                    let bytes = if let Some(first) = arguments.first() {
                        let value = self.eval_expression(*first, frame)?;
                        match value {
                            Value::Str(text) => text.borrow().to_vec(),
                            other => {
                                return unsupported(format!(
                                    "host write of non-string value {other:?}"
                                ));
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    let stream = if error_stream {
                        &mut self.stderr
                    } else {
                        &mut self.stdout
                    };
                    stream.extend_from_slice(&bytes);
                    if line {
                        stream.push(b'\n');
                    }
                    Ok(Value::Unit)
                }
                HostOperation::ReadByte => self.read_stdin_byte_value(target_symbol),
                HostOperation::WriteByte => {
                    let byte = arguments
                        .first()
                        .and_then(|argument| self.eval_expression(*argument, frame).ok())
                        .and_then(|value| match value {
                            Value::Int(byte) => Some(byte as u8),
                            _ => None,
                        });
                    match byte {
                        Some(byte) => {
                            self.stdout.push(byte);
                            Ok(Value::Unit)
                        }
                        None => unsupported("write_byte expects one integer argument".to_string()),
                    }
                }
                HostOperation::ReadLine => {
                    if let Some(type_symbol) = self.read_line_result_type(target_symbol) {
                        return self.read_stdin_bounded_line(
                            arguments.first().copied(),
                            frame,
                            type_symbol,
                        );
                    }
                    let line = self.read_stdin_line();
                    if let Some(first) = arguments.first()
                        && let Ok(cell) = self.resolve_place(*first, frame)
                    {
                        let cell = self.deref_cell(cell);
                        if let Value::Str(text) = &*cell.borrow() {
                            text.replace(line.clone().into_bytes())
                                .map_err(Halt::Resource)?;
                        } else {
                            *cell.borrow_mut() = self.allocate_text(line.clone().into_bytes())?;
                        }
                    }
                    Ok(Value::Bool(!line.is_empty()))
                }
                HostOperation::VirtualTimeRead(name) => self
                    .virtual_time_host_value(name)
                    .ok_or_else(|| Halt::Unsupported(format!("virtual time read `{name}`"))),
                HostOperation::TickCount => {
                    self.advance_virtual_clock(1)?;
                    Ok(Value::Int(self.virtual_ticks))
                }
            }
        }

        fn read_line_result_type(&self, target_symbol: SymbolHandle) -> Option<SymbolHandle> {
            let (_, signature) = self.boundary_call_requirement(target_symbol)?;
            self.line_read_result_shape(signature.return_type)
        }

        fn line_read_result_shape(&self, return_type: TypeReferenceHandle) -> Option<SymbolHandle> {
            let (data, symbol) =
                checked_shape_data_declaration(self.program, return_type, "LineReadResult")?;
            let [
                DataMember::Variant(invalid),
                DataMember::Variant(line_complete),
                DataMember::Variant(end_of_input),
                DataMember::Variant(full),
            ] = self.program.data_members(data)
            else {
                return None;
            };
            if invalid.name.as_str() != "Invalid"
                || !invalid.payload.is_empty()
                || line_complete.name.as_str() != "LineComplete"
                || end_of_input.name.as_str() != "EndOfInput"
                || full.name.as_str() != "Full"
            {
                return None;
            }
            for variant in [line_complete, end_of_input, full] {
                let [field] = self.program.data_payload_fields(variant) else {
                    return None;
                };
                if field.name.as_str() != "count"
                    || field.relevance.is_erased()
                    || self.program.primitive_type_reference(field.type_reference)
                        != Some(PrimitiveType::U64)
                {
                    return None;
                }
            }
            Some(symbol)
        }

        fn read_stdin_bounded_line(
            &mut self,
            destination: Option<ExpressionHandle>,
            frame: &mut Frame,
            type_symbol: SymbolHandle,
        ) -> EvalResult<Value> {
            let cell = match destination {
                Some(argument) => {
                    let place = self.resolve_place(argument, frame)?;
                    Some(self.deref_cell(place))
                }
                None => None,
            };
            let capacity = match &cell {
                Some(cell) => match &*cell.borrow() {
                    Value::Str(text) => text.borrow().len(),
                    Value::Array(elements) => elements.len(),
                    _ => return trap("read_line destination is not a bounded byte carrier"),
                },
                None => 0,
            };
            let mut count: u64 = 0;
            let variant = loop {
                if count as usize >= capacity {
                    break "Full";
                }
                let Some(byte) = self.stdin.get(self.stdin_cursor).copied() else {
                    break "EndOfInput";
                };
                self.stdin_cursor += 1;
                if let Some(cell) = &cell {
                    match &*cell.borrow() {
                        Value::Str(text) => {
                            text.write_byte(count as usize, byte).map_err(|_| {
                                Halt::Trap("read_line destination shrank during input".to_owned())
                            })?
                        }
                        Value::Array(elements) => {
                            let Some(element) = elements.get(count as usize) else {
                                return trap("read_line destination shrank during input");
                            };
                            *element.borrow_mut() = Value::Int(i64::from(byte));
                        }
                        _ => return trap("read_line destination is not a bounded byte carrier"),
                    }
                }
                count += 1;
                if byte == b'\n' {
                    break "LineComplete";
                }
            };
            let count = i64::try_from(count)
                .map_err(|_| Halt::Resource("read_line byte count overflowed".to_owned()))?;
            Ok(Value::Enum {
                type_symbol,
                variant_name: variant.into(),
                payload: vec![("count".into(), self.allocate_cell(Value::Int(count))?)],
            })
        }

        pub(crate) fn exact_console_intrinsic_host_method(
            &self,
            target_symbol: SymbolHandle,
        ) -> Option<&'static str> {
            let (requirement_symbol, provider_symbol) =
                validation::exact_compiler_intrinsic_boundary_requirement(
                    self.program,
                    target_symbol,
                )?;
            let (realization, _) = self
                .program
                .machine_states_named(target_symbol)
                .find(|(machine, _)| machine.attached_data_symbol == provider_symbol)?;
            let (provider_name, trait_name) = match realization.name.as_str() {
                "ConsoleNativeProvider::read_byte"
                | "ConsoleNativeProvider::write_byte"
                | "ConsoleNativeProvider::exit_process" => ("ConsoleNativeProvider", "Console"),
                "ProcessExitNativeProvider::exit_process" => {
                    ("ProcessExitNativeProvider", "ProcessExit")
                }
                _ => return None,
            };
            if realization.attached_data.as_ref().map(|name| name.as_str()) != Some(provider_name) {
                return None;
            }
            let requirement = self
                .program
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.name.as_str() == trait_name
                })
                .flat_map(|definition| self.program.trait_machine_signatures(definition))
                .find(|requirement| requirement.symbol == requirement_symbol)?;
            match (realization.name.as_str(), requirement.name.as_str()) {
                ("ConsoleNativeProvider::read_byte", "read_byte")
                    if self
                        .program
                        .state_signature_parameters(requirement)
                        .is_empty()
                        && exact_byte_read_result_type(self.program, requirement.return_type)
                            .is_some() =>
                {
                    Some("read_byte")
                }
                ("ConsoleNativeProvider::write_byte", "write_byte") => Some("write_byte"),
                ("ConsoleNativeProvider::exit_process", "exit_process") => Some("exit_process"),
                ("ProcessExitNativeProvider::exit_process", "exit_process") => Some("exit_process"),
                _ => None,
            }
        }
    }
}

pub(crate) mod normal_cleanup {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, EvaluatedArgument, Evaluator, Frame, Halt, Machine, State, Value,
        statement_table, trap, unsupported,
    };
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan::{
        Parameter, StructuralLocal, StructuralResult,
    };
    use checked_trees::{
        CheckedNormalCleanupAction, CheckedUnitNominalAffineCleanupPlan,
        CheckedUnitStructuralParameterPlan,
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum CompletionBoundary {
        NormalReturn,
        TerminalTransfer,
    }

    impl<'program> Evaluator<'program> {
        fn normal_cleanup_actions(
            &self,
            machine: &Machine,
            state: &State,
        ) -> EvalResult<
            Vec<(
                &'program CheckedUnitNominalAffineCleanupPlan,
                symbols::SymbolHandle,
            )>,
        > {
            let Some(checked) = self.checked else {
                return Ok(Vec::new());
            };
            let flow = &checked.facts.flow;
            let mut actions = Vec::new();
            if let Some(plan) = flow.terminal_unit_effects.for_machine(machine.symbol) {
                if plan.state != state.symbol {
                    return unsupported("normal cleanup plan names another return state");
                }
                let control = &flow.terminal_unit_effects.cleanup_control;
                if control
                    .normal_exits(plan.normal_exit_cleanups)
                    .iter()
                    .any(|row| !row.selected_regions.is_empty())
                {
                    return unsupported(
                        "unimplemented: checked-tree interpreter selected owner cleanup",
                    );
                }
                for action in control.normal_actions(plan.normal_exit_cleanups) {
                    let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                        continue;
                    };
                    let owner = self.normal_cleanup_owner(
                        state,
                        action,
                        &plan.structural_parameters,
                        &plan.operations,
                    )?;
                    actions.push((action, owner));
                }
            }
            Ok(actions)
        }

        fn normal_cleanup_owner(
            &self,
            state: &State,
            action: &CheckedUnitNominalAffineCleanupPlan,
            parameters: &[CheckedUnitStructuralParameterPlan],
            operations: &[checked_trees::CheckedUnitEffectOperationPlan],
        ) -> EvalResult<symbols::SymbolHandle> {
            Ok(match &action.source {
                Parameter { parameter_index } => {
                    let parameter = parameters
                        .get(*parameter_index as usize)
                        .filter(|parameter| {
                            !action.path.is_empty()
                                || parameter.type_identity == action.type_identity
                        })
                        .ok_or_else(|| {
                            Halt::Unsupported("normal cleanup parameter custody drifted".into())
                        })?;
                    self.program
                        .state_parameters(state)
                        .get(parameter.position as usize)
                        .ok_or_else(|| {
                            Halt::Unsupported("normal cleanup parameter is absent".into())
                        })?
                        .symbol
                }
                StructuralLocal { symbol } => *symbol,
                StructuralResult { binding_ordinal } => {
                    let result = operations
                        .iter()
                        .filter_map(|operation| operation.structural_result_binding())
                        .find(|result| {
                            result.binding_ordinal == *binding_ordinal
                                && (!action.path.is_empty()
                                    || result.type_identity == action.type_identity)
                        })
                        .ok_or_else(|| {
                            Halt::Unsupported("normal cleanup result binding is absent".into())
                        })?;
                    let Some(typed_trees::statement::StatementNode::LocalData(local)) =
                        statement_table(self.program)
                            .statements(state.statement_nodes)
                            .get(result.statement_index as usize)
                    else {
                        return unsupported("normal cleanup result has no retained local binding");
                    };
                    local.symbol
                }
                _ => return unsupported("normal cleanup source has no retained runtime owner"),
            })
        }

        pub(crate) fn validate_normal_cleanup_selection(
            &self,
            machine: &Machine,
            state: &State,
        ) -> EvalResult<()> {
            let Some(checked) = self.checked else {
                return Ok(());
            };
            let actions = self.normal_cleanup_actions(machine, state)?;
            let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
            if control
                .value_completions
                .iter()
                .filter(|(_, row)| row.machine == machine.symbol && row.state == state.symbol)
                .any(|(_, row)| {
                    control
                        .actions
                        .span_or_empty(row.actions)
                        .iter()
                        .any(|action| action.nominal().is_some())
                })
            {
                return unsupported(
                    "unimplemented: checked-tree interpreter expression-owned normal cleanup",
                );
            }
            let composed = checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(machine.symbol)
                .and_then(|plan| plan.state(state.symbol));
            if composed.is_some_and(|state| checked.facts.flow.terminal_unit_effects.cleanup_control.normal_exits(state.normal_exit_cleanups).iter().any(|row| !row.selected_regions.is_empty()))
            || self.cleanup_operations(machine.symbol, state.symbol).iter().any(|operation| {
                matches!(operation, checked_trees::CheckedUnitEffectOperationPlan::ContinuationCleanup { selected_regions, .. } if !selected_regions.is_empty())
            })
        {
            return unsupported("unimplemented: checked-tree interpreter selected owner cleanup");
        }
            for (_, dependency) in checked.facts.operational.cleanup_dependencies.iter() {
                if dependency.caller_machine == machine.symbol
                    && dependency.caller_state == state.symbol
                    && !actions.iter().any(|(action, _)| {
                        action.cleanup_machine == dependency.target_machine
                            && action.cleanup_state == dependency.target_state
                    })
                    && !composed.is_some_and(|state| {
                        checked
                            .facts
                            .flow
                            .terminal_unit_effects
                            .cleanup_control
                            .normal_actions(state.normal_exit_cleanups)
                            .any(|action| {
                                matches!(action,
                        CheckedNormalCleanupAction::InvokeNominal(action)
                            if action.cleanup_machine == dependency.target_machine
                                && action.cleanup_state == dependency.target_state)
                            })
                    })
                    && !self
                        .cleanup_operations(machine.symbol, state.symbol)
                        .iter()
                        .any(|operation| {
                            matches!(operation,
                        checked_trees::CheckedUnitEffectOperationPlan::ContinuationCleanup {
                            boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { .. },
                            cleanup_actions,
                            ..
                        } if cleanup_actions.iter().any(|action| matches!(action,
                            CheckedNormalCleanupAction::InvokeNominal(action)
                                if action.cleanup_machine == dependency.target_machine
                                    && action.cleanup_state == dependency.target_state)))
                        })
                {
                    return unsupported(
                        "selected normal cleanup dependency has no executable checked boundary plan",
                    );
                }
            }
            Ok(())
        }

        pub(crate) fn execute_completion_cleanup(
            &mut self,
            machine: &Machine,
            state: &State,
            frame: &Frame,
            exit: checked_trees::FlowClaimJoinExit,
            boundary: CompletionBoundary,
        ) -> EvalResult<()> {
            if let Some((control, rows, parameters, operations)) =
                self.checked.and_then(|checked| {
                    let plans = &checked.facts.flow.terminal_unit_effects;
                    let body = plans
                        .for_machine(machine.symbol)
                        .filter(|plan| plan.state == state.symbol)
                        .map(|plan| {
                            (
                                plan.normal_exit_cleanups,
                                plan.structural_parameters.as_slice(),
                                plan.operations.as_slice(),
                            )
                        })
                        .or_else(|| {
                            plans
                                .composed_for_machine(machine.symbol)
                                .and_then(|machine| machine.state(state.symbol))
                                .map(|state| {
                                    (
                                        state.normal_exit_cleanups,
                                        state.structural_parameters.as_slice(),
                                        state.operations.as_slice(),
                                    )
                                })
                        });
                    body.map(|(rows, parameters, operations)| {
                        (&plans.cleanup_control, rows, parameters, operations)
                    })
                })
            {
                if rows.completion == checked_trees::CheckedNormalCompletion::NoNormal {
                    if !control.normal_exits(rows).is_empty() {
                        return unsupported("no-normal invocation carries normal cleanup receipts");
                    }
                    return if boundary == CompletionBoundary::TerminalTransfer {
                        Ok(())
                    } else {
                        unsupported("normal return from a checked no-normal invocation")
                    };
                }
                let row = control
                    .normal_exits(rows)
                    .iter()
                    .find(|row| row.exit == exit)
                    .ok_or_else(|| {
                        Halt::Unsupported("normal cleanup has no exact outgoing edge plan".into())
                    })?;
                if !row.selected_regions.is_empty() {
                    return unsupported(
                        "unimplemented: checked-tree interpreter selected owner cleanup",
                    );
                }
                for action in control.actions.span_or_empty(row.actions) {
                    let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                        continue;
                    };
                    let symbol =
                        self.normal_cleanup_owner(state, action, parameters, operations)?;
                    let source = frame.local_cell(symbol).ok_or_else(|| {
                        Halt::Unsupported("normal cleanup source binding is absent".into())
                    })?;
                    self.execute_nominal_cleanup(action, source)?;
                }
                return Ok(());
            }
            for (action, owner) in self.normal_cleanup_actions(machine, state)? {
                let source = frame.local_cell(owner).ok_or_else(|| {
                    Halt::Unsupported("normal cleanup source binding is absent".into())
                })?;
                self.execute_nominal_cleanup(action, source)?;
            }
            Ok(())
        }

        fn execute_nominal_cleanup(
            &mut self,
            action: &CheckedUnitNominalAffineCleanupPlan,
            source: Cell,
        ) -> EvalResult<()> {
            let source = self.normal_cleanup_cell(source, &action.path)?;
            self.execute_nominal_cleanup_receiver(action, source)
        }

        pub(crate) fn execute_nominal_cleanup_receiver(
            &mut self,
            action: &CheckedUnitNominalAffineCleanupPlan,
            source: Cell,
        ) -> EvalResult<()> {
            let checked = self
                .checked
                .expect("cleanup actions require checked custody");
            let target_plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .callable_entry_for_machine(action.cleanup_machine)
                .ok_or_else(|| {
                    Halt::Unsupported("nominal cleanup target has no checked body".into())
                })?;
            if target_plan.state != action.cleanup_state
                || target_plan.contract_report_fingerprint
                    != action.cleanup_contract_report_fingerprint
            {
                return unsupported("nominal cleanup target selection drifted");
            }
            let target = self
                .find_machine_by_symbol(action.cleanup_machine)
                .ok_or_else(|| Halt::Unsupported("nominal cleanup target is absent".into()))?;
            let target_state = self
                .find_state_by_symbol(target, action.cleanup_state)
                .ok_or_else(|| {
                    Halt::Unsupported("nominal cleanup target state is absent".into())
                })?;
            if !matches!(*source.borrow(), Value::Struct { .. }) {
                return trap("nominal cleanup source is not its record value");
            }
            let parameters = self.program.state_parameters(target_state);
            let arguments = match parameters {
                [] => Vec::new(),
                [parameter]
                    if validation::reference_shell(self.program, parameter.type_reference)
                        .is_some_and(|shell| shell.access.is_exclusive()) =>
                {
                    vec![EvaluatedArgument::plain(
                        self.allocate_cell(Value::reference(source.clone()))?,
                    )]
                }
                _ => return unsupported("nominal cleanup target argument custody drifted"),
            };
            let _returned = self.run_state_collect(target, target_state, arguments)?;
            Ok(())
        }

        pub(crate) fn normal_cleanup_cell(
            &self,
            mut source: Cell,
            path: &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> EvalResult<Cell> {
            use checked_trees::CheckedUnitStructuralPathSegment;
            use typed_trees::data::DataMember;
            for segment in path {
                source = self.deref_cell(source);
                match segment {
                    CheckedUnitStructuralPathSegment::AllElements => {
                        return unsupported("universal qualification is not a cleanup address");
                    }
                    CheckedUnitStructuralPathSegment::RepresentationView { .. } => {
                        return unsupported(
                            "storage interpretation cannot provide an owned cleanup receiver",
                        );
                    }
                    CheckedUnitStructuralPathSegment::Referent => {}
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        source = self.element_cell(
                            &source,
                            usize::try_from(*index).map_err(|_| {
                                Halt::Unsupported("cleanup index exceeds host storage".into())
                            })?,
                        )?;
                    }
                    CheckedUnitStructuralPathSegment::Field(identity)
                    | CheckedUnitStructuralPathSegment::Case(identity) => {
                        let value = source.borrow();
                        let (type_symbol, case_name) = match &*value {
                            Value::Struct { type_symbol, .. } => (*type_symbol, None),
                            Value::Enum {
                                type_symbol,
                                variant_name,
                                ..
                            } => (*type_symbol, Some(variant_name.as_ref())),
                            _ => return trap("cleanup projection is not a record or case"),
                        };
                        let data = self
                            .program
                            .data_definition_by_symbol(type_symbol)
                            .ok_or_else(|| {
                                Halt::Unsupported("cleanup projection declaration is absent".into())
                            })?;
                        let variant = case_name.and_then(|name| {
                            self.program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Variant(variant)
                                        if variant.name.as_str() == name =>
                                    {
                                        Some(variant)
                                    }
                                    _ => None,
                                })
                        });
                        if matches!(segment, CheckedUnitStructuralPathSegment::Case(_)) {
                            if !variant.is_some_and(|variant| {
                                variant.path_identity() == identity.spelling()
                            }) {
                                return trap(
                                    "cleanup case membership does not match its selected path",
                                );
                            }
                            continue;
                        }
                        let field = if let Some(variant) = variant {
                            self.program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.path_identity() == identity.spelling())
                        } else {
                            self.program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Field(field)
                                        if field.path_identity() == identity.spelling() =>
                                    {
                                        Some(field)
                                    }
                                    _ => None,
                                })
                        }
                        .ok_or_else(|| {
                            Halt::Unsupported("cleanup field identity is absent".into())
                        })?;
                        let selected = self.field_cell(&source, field.name.as_str())?;
                        drop(value);
                        source = selected;
                    }
                    CheckedUnitStructuralPathSegment::FixedRange { .. }
                    | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_) => {
                        return unsupported(
                            "normal cleanup requires a retained static receiver path",
                        );
                    }
                }
            }
            Ok(source)
        }
    }
}

pub(crate) mod replacement_cleanup {
    use crate::interpreter::evaluator::{Cell, EvalResult, Evaluator, Frame, Halt};
    use checked_trees::CheckedUnitStructuralPathSegment;
    use checked_trees::{
        CheckedNormalCleanupAction, CheckedUnitCleanupBoundary, CheckedUnitEffectOperationPlan,
        CheckedUnitNominalAffineCleanupPlan,
    };
    use symbols::SymbolHandle;

    pub(crate) struct ReplacementCleanup<'program> {
        action: &'program CheckedUnitNominalAffineCleanupPlan,
        receiver: Cell,
        path: &'program [CheckedUnitStructuralPathSegment],
    }

    pub(crate) enum ReplacementStorage {
        Cell(Cell),
        ArrayWindow {
            carrier: Cell,
            start: usize,
            end: usize,
        },
    }

    impl<'program> Evaluator<'program> {
        pub(crate) fn cleanup_operations(
            &self,
            machine: SymbolHandle,
            state: SymbolHandle,
        ) -> &'program [CheckedUnitEffectOperationPlan] {
            let Some(checked) = self.checked else {
                return &[];
            };
            let plans = &checked.facts.flow.terminal_unit_effects;
            if let Some(plan) = plans
                .for_machine(machine)
                .filter(|plan| plan.state == state)
            {
                return &plan.operations;
            }
            plans
                .composed_for_machine(machine)
                .and_then(|plan| plan.state(state))
                .map_or(&[], |state| state.operations.as_slice())
        }

        pub(crate) fn capture_replacement_cleanup(
            &self,
            statement_index: usize,
            destination: Option<ReplacementStorage>,
            frame: &Frame,
        ) -> EvalResult<Vec<ReplacementCleanup<'program>>> {
            let mut prepared = Vec::new();
            for operation in self.cleanup_operations(frame.machine_symbol, frame.state_symbol) {
                let CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary:
                        CheckedUnitCleanupBoundary::Replacement {
                            statement_index: selected_statement,
                            old_value,
                            receiver_path,
                            ..
                        },
                    cleanup_actions,
                    selected_regions,
                    // Partition completion guides compiler joins, not runtime hooks.
                    partitions: _,
                } = operation
                else {
                    continue;
                };
                if *selected_statement as usize != statement_index {
                    continue;
                }
                if !selected_regions.is_empty() {
                    return Err(Halt::Unsupported(
                        "unimplemented: checked-tree interpreter selected owner cleanup".into(),
                    ));
                }
                for action in cleanup_actions {
                    let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                        continue;
                    };
                    let path = action
                        .path
                        .strip_prefix(old_value.path.as_slice())
                        .filter(|_| action.source == old_value.source)
                        .ok_or_else(|| {
                            Halt::Unsupported(
                                "replacement cleanup lost its selected old-value home".into(),
                            )
                        })?;
                    let storage = destination.as_ref().ok_or_else(|| {
                        Halt::Unsupported(
                            "replacement cleanup has no retained record storage".into(),
                        )
                    })?;
                    let (receiver, path) = match storage {
                        ReplacementStorage::Cell(receiver) => {
                            let receiver =
                                self.normal_cleanup_cell(receiver.clone(), receiver_path)?;
                            (receiver, path)
                        }
                        ReplacementStorage::ArrayWindow {
                            carrier,
                            start,
                            end,
                        } => {
                            let (first, tail, remaining) =
                                if let Some((first, tail)) = receiver_path.split_first() {
                                    (first, tail, path)
                                } else {
                                    let (first, tail) = path.split_first().ok_or_else(|| {
                                        Halt::Unsupported(
                                            "array window cleanup has no element receiver".into(),
                                        )
                                    })?;
                                    (first, &[][..], tail)
                                };
                            let CheckedUnitStructuralPathSegment::FixedIndex(index) = first else {
                                return Err(Halt::Unsupported(
                                    "array window cleanup lost its relative element".into(),
                                ));
                            };
                            let index = usize::try_from(*index)
                                .ok()
                                .filter(|index| *index < end - start)
                                .ok_or_else(|| {
                                    Halt::Trap(
                                        "array window cleanup element is out of bounds".into(),
                                    )
                                })?;
                            let receiver = self.element_cell(carrier, start + index)?;
                            let receiver = self.normal_cleanup_cell(receiver, tail)?;
                            (receiver, remaining)
                        }
                    };
                    prepared.push(ReplacementCleanup {
                        action,
                        receiver,
                        path,
                    });
                }
            }
            Ok(prepared)
        }

        pub(crate) fn execute_replacement_cleanup(
            &mut self,
            prepared: Vec<ReplacementCleanup<'program>>,
        ) -> EvalResult<()> {
            for cleanup in prepared {
                let receiver = self.normal_cleanup_cell(cleanup.receiver, cleanup.path)?;
                self.execute_nominal_cleanup_receiver(cleanup.action, receiver)?;
            }
            Ok(())
        }
    }
}

pub(crate) mod statements_and_calls {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame, FrameLocal, Halt,
        Machine, Reference, State, StatementNode, SymbolHandle, TableCall, TableTransition,
        TransitionDecision, TransitionGuardNode, TransitionTargetNode, Value, statement_table,
    };
    use checked_trees::FlowClaimJoinExitKind::{TransitionContinuation, TransitionTarget};
    use language_core::inline_assembly::{
        AsmCacheOperationKind, AsmFenceKind, AsmInstructionSerializationKind,
        AsmInterruptControlKind, AsmSchedulingHintKind,
    };
    use typed_trees::statement::TransitionExit;

    impl<'program> Evaluator<'program> {
        pub(crate) fn exec_statement(
            &mut self,
            statement: &StatementNode,
            statement_index: usize,
            frame: &mut Frame,
        ) -> EvalResult<()> {
            self.tick()?;
            match statement {
                StatementNode::Assembly(_) => Ok(()),
                StatementNode::AssemblyFact(_) => Ok(()),
                StatementNode::Assignment(assignment) => {
                    self.assign(statement_index, assignment, frame)
                }
                StatementNode::LocalData(local) => {
                    let value = if local.initial_value.is_valid() {
                        let value = self.eval_expression_at_type(
                            local.initial_value,
                            local.type_reference,
                            frame,
                        )?;
                        let copy_array = matches!(value, Value::Array(_))
                            && self.declared_type_is_fixed_array(local.type_reference);
                        if matches!(value, Value::Struct { .. }) || copy_array {
                            value.deep_clone_with(&|value| self.allocate_cell(value))?
                        } else {
                            value
                        }
                    } else {
                        self.default_value_for_type(local.type_reference)?
                    };
                    let value = self.coerce_scalar_value(value, local.type_reference)?;
                    let scalar = self
                        .program
                        .primitive_type_reference(local.type_reference)
                        .map(|primitive| {
                            (
                                primitive,
                                self.program
                                    .arithmetic_domain_for_type_reference(local.type_reference),
                            )
                        });
                    frame.bind(FrameLocal {
                        symbol: local.symbol,
                        cell: self.allocate_cell(value)?,
                        type_reference: local.type_reference,
                        scalar,
                    });
                    Ok(())
                }
                StatementNode::Call(_) => Err(Halt::Trap(
                    "call statement requires its executed statement identity".to_owned(),
                )),
                StatementNode::Expression(expression) => {
                    let _ = self.eval_expression(*expression, frame)?;
                    Ok(())
                }
                StatementNode::Transition(_) => Ok(()),
            }
        }

        fn crash_site(&self, frame: &Frame, span: source::SourceSpan) -> String {
            format!(
                "machine `{}` state `{}` (source {:?} bytes {}..{})",
                self.program.symbols.name(frame.machine_symbol),
                self.program.symbols.name(frame.state_symbol),
                span.source_id,
                span.span.start,
                span.span.end
            )
        }

        pub(crate) fn eval_transition(
            &mut self,
            transition: &TableTransition,
            frame: &mut Frame,
        ) -> EvalResult<
            Option<(
                TransitionDecision<'program>,
                checked_trees::FlowClaimJoinExitKind,
            )>,
        > {
            let holds = match transition.guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(expression) => {
                    self.guard_depth += 1;
                    let value = self.eval_expression(expression, frame);
                    self.guard_depth -= 1;
                    let value = value?;
                    value
                        .as_bool()
                        .ok_or_else(|| Halt::Trap("transition guard is not boolean".to_owned()))?
                }
            };
            let (target_handle, kind) = if holds {
                (transition.target, TransitionTarget)
            } else if transition.continuation.is_valid() {
                (transition.continuation, TransitionContinuation)
            } else {
                return Ok(None);
            };

            if holds {
                match transition.exit {
                    TransitionExit::Ordinary => {}
                    TransitionExit::Crash(cause) => {
                        return Err(Halt::Crash(
                            cause,
                            self.crash_site(frame, transition.source_span),
                        ));
                    }
                }
            }

            let target = statement_table(self.program)
                .transition_target(target_handle)
                .clone();
            let decision = self.resolve_transition_target(&target, frame)?;
            Ok(Some((decision, kind)))
        }

        fn resolve_transition_target(
            &mut self,
            target: &TransitionTargetNode,
            frame: &mut Frame,
        ) -> EvalResult<TransitionDecision<'program>> {
            match target {
                TransitionTargetNode::Terminal => Ok(TransitionDecision::Terminal),
                TransitionTargetNode::Value(expression) => {
                    let value =
                        self.eval_expression_at_type(*expression, frame.return_type, frame)?;
                    Ok(TransitionDecision::Value(value))
                }
                TransitionTargetNode::Named {
                    path, arguments, ..
                } => {
                    let members = self.program.statement_table.name_path_members(path.members);
                    let (name, receiver_members) = members
                        .split_last()
                        .ok_or_else(|| Halt::Unsupported("empty named transition".to_owned()))?;
                    let state_name = name.as_str();
                    let receiver_symbol = if receiver_members.len() == 1 {
                        path.head_symbol
                    } else {
                        SymbolHandle::invalid()
                    };
                    if let Some(value) = self.try_selected_named_call(
                        receiver_symbol,
                        path.symbol,
                        &[],
                        self.program.statement_table.expression_handles(*arguments),
                        None,
                        frame,
                    )? {
                        return Ok(TransitionDecision::Value(value));
                    }

                    if let Some(value) = self.try_host_invocation(
                        path.symbol,
                        state_name,
                        self.program.statement_table.expression_handles(*arguments),
                        frame,
                    )? {
                        return Ok(TransitionDecision::Value(value));
                    }

                    let (machine, state) = self
                        .resolve_entry_state_symbol(path.symbol)
                        .ok_or_else(|| {
                            Halt::Unsupported(format!(
                                "transition target `{state_name}` has no resolved machine or state"
                            ))
                        })?;

                    let arguments = self.eval_state_arguments(
                        state,
                        self.program.statement_table.expression_handles(*arguments),
                        None,
                        frame,
                    )?;

                    Ok(TransitionDecision::Named {
                        state,
                        machine,
                        arguments,
                    })
                }
            }
        }

        pub(crate) fn eval_call_statement(
            &mut self,
            statement: typed_trees::statement::StatementHandle,
            call: &TableCall,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            if call.static_requirement_dispatch.is_some() {
                return self.run_selected_realization(
                    call.target_symbol,
                    statement_table(self.program).expression_handles(call.arguments),
                    Some(validation::CallSite::Statement(call)),
                    frame,
                );
            }
            if let Some(slot) = self.program.dynamic_slot_by_symbol(call.target_symbol) {
                return self.run_dynamic_slot(
                    slot,
                    statement_table(self.program).expression_handles(call.arguments),
                    frame,
                );
            }
            if let Some(value) = self.try_selected_named_call(
                call.receiver_symbol,
                call.target_symbol,
                &call.machine_arguments,
                statement_table(self.program).expression_handles(call.arguments),
                Some(validation::CallSite::Statement(call)),
                frame,
            )? {
                return Ok(value);
            }

            if call.target.as_str() == "asm#popfq" {
                let arguments = statement_table(self.program).expression_handles(call.arguments);
                let [image] = arguments else {
                    return Err(Halt::Unsupported(
                        "popfq requires one image operand".to_owned(),
                    ));
                };
                let value = self.eval_expression(*image, frame)?;
                let Value::Int(image) = value else {
                    return Err(Halt::Trap(
                        "popfq image operand did not evaluate to an integer".to_owned(),
                    ));
                };
                let Some(environment) = self.x86_flags.as_mut() else {
                    return Err(Halt::Unsupported(
                    "popfq requires an explicitly supplied target flags environment in Terminal execution".to_owned(),
                ));
                };
                environment.restore_arithmetic_bits(image as u64);
                return Ok(Value::Unit);
            }
            if AsmFenceKind::from_intrinsic_name(call.target.as_str()).is_some()
                || AsmInterruptControlKind::from_intrinsic_name(call.target.as_str()).is_some()
                || AsmInstructionSerializationKind::from_intrinsic_name(call.target.as_str())
                    .is_some()
                || AsmSchedulingHintKind::from_intrinsic_name(call.target.as_str()).is_some()
                || AsmCacheOperationKind::from_intrinsic_name(call.target.as_str()).is_some()
            {
                return Ok(Value::Unit);
            }
            if self
                .try_build_facet_routes(
                    call.target_symbol,
                    statement_table(self.program).expression_handles(call.arguments),
                    call.source_span,
                    frame,
                )?
                .is_some()
            {
                return Ok(Value::Unit);
            }
            if let Some(value) = self.try_behavior_exclusion_statement(statement, call, frame)? {
                return Ok(value);
            }
            if let Some(value) = self.try_host_call(call, frame)? {
                return Ok(value);
            }

            let (machine, state) =
                self.resolve_static_call(call.target_symbol, call.target.as_str())?;

            let arguments = self.eval_state_arguments(
                state,
                statement_table(self.program).expression_handles(call.arguments),
                Some(validation::CallSite::Statement(call)),
                frame,
            )?;

            self.run_state_collect(machine, state, arguments)
                .map(|value| value.unwrap_or(Value::Unit))
        }

        pub(crate) fn resolve_entry_state_symbol(
            &self,
            target_symbol: SymbolHandle,
        ) -> Option<(&'program Machine, &'program State)> {
            if !target_symbol.is_valid() {
                return None;
            }
            let (machine, state) = if let Some(machine) = self.find_machine_by_symbol(target_symbol)
            {
                (machine, self.machine_entry_state(machine)?)
            } else {
                let owner = self.program.machine_holding_state(target_symbol)?;
                let machine = self.find_machine_by_symbol(owner.symbol)?;
                if machine.symbol != owner.symbol {
                    let entry = self.program.machine_states(owner).first()?;
                    if entry.symbol != target_symbol {
                        return None;
                    }
                    (machine, self.machine_entry_state(machine)?)
                } else {
                    (machine, self.find_state_by_symbol(machine, target_symbol)?)
                }
            };
            Some((machine, state))
        }

        pub(crate) fn resolve_static_call(
            &self,
            target_symbol: SymbolHandle,
            target: &str,
        ) -> EvalResult<(&'program Machine, &'program State)> {
            self.resolve_entry_state_symbol(target_symbol)
                .ok_or_else(|| {
                    Halt::Unsupported(format!("call target `{target}` has no resolved state"))
                })
        }

        pub(crate) fn is_declaration_namespace(&self, symbol: SymbolHandle) -> bool {
            symbol.is_valid()
                && matches!(
                    self.program.symbols.get(symbol).kind,
                    symbols::SymbolKind::BuiltinType
                        | symbols::SymbolKind::Data
                        | symbols::SymbolKind::Domain
                        | symbols::SymbolKind::Machine
                        | symbols::SymbolKind::Module
                        | symbols::SymbolKind::Trait
                        | symbols::SymbolKind::ConformanceParameter
                )
        }

        pub(crate) fn current_machine(&self, frame: &Frame) -> Option<&'program Machine> {
            if !frame.machine_symbol.is_valid() {
                return None;
            }
            self.program.machine(frame.machine_symbol)
        }

        pub(crate) fn machine_entry_state(&self, machine: &Machine) -> Option<&'program State> {
            let leaf = machine.name.as_str().rsplit("::").next().unwrap_or("");
            if let Some(state) = self.find_state(machine, leaf) {
                return Some(state);
            }
            self.program.machine_states(machine).first()
        }

        pub(crate) fn eval_argument(
            &mut self,
            argument: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Cell> {
            match self.program.expression_table.expression(argument) {
                ExpressionNode::Indexed(indexed)
                    if matches!(
                        self.program.expression_table.expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) =>
                {
                    let value = self.eval_expression(argument, frame)?;
                    self.allocate_cell(value)
                }
                ExpressionNode::Borrow(inner) => {
                    let cell = self.eval_read_cell(inner.target, frame)?;
                    let target = match &*cell.borrow() {
                        Value::Ref(target) => target.clone(),
                        _ => Reference::new(cell.clone()),
                    };
                    self.allocate_cell(Value::Ref(target))
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => {
                    let cell = self.eval_read_cell(argument, frame)?;
                    let value = cell.borrow().clone();
                    self.allocate_cell(value)
                }
                _ => {
                    let value = self.eval_expression(argument, frame)?;
                    self.allocate_cell(value)
                }
            }
        }
    }
}

pub(crate) mod states_and_transitions {
    use super::normal_cleanup::CompletionBoundary;
    use crate::build_time::observe_build_time_value;
    use crate::interpreter::evaluator::{
        BTreeMap, BuildEvaluationSponsor, CALL_DEPTH_BUDGET, Cell, CellMeter, CheckedTrees,
        DataDefinition, DataMember, EvalResult, EvaluatedArgument, EvaluationUsage, Evaluator,
        FilesystemLogicalHandles, FilesystemMetadataLayout, Frame, FrameLocal, Halt, Machine,
        PrimitiveType, State, StatementNode, SymbolHandle, TextByteMeter, TransitionDecision,
        TypedTrees, Value, ambient_step_budget, expression_table, statement_table, trap,
        type_reference_table, unsupported,
    };
    use validation::field_domains::data_definition_by_symbol_or_name;
    impl<'program> Evaluator<'program> {
        pub(crate) fn new_build(
            program: crate::interpreter::BuildMachineProgram<'program>,
        ) -> Self {
            match program {
                crate::interpreter::BuildMachineProgram::Typed(typed) => Self::new(typed, &[]),
                crate::interpreter::BuildMachineProgram::Checked(checked) => {
                    let mut evaluator = Self::new(&checked.typed, &[]);
                    evaluator.checked = Some(checked);
                    evaluator.operator_facts = Some(&checked.facts.operators);
                    evaluator
                }
                crate::interpreter::BuildMachineProgram::CheckedClosure {
                    execution,
                    product_declarations,
                } => {
                    let mut evaluator = Self::new_build(
                        crate::interpreter::BuildMachineProgram::Checked(execution),
                    );
                    evaluator.product_declarations = product_declarations;
                    evaluator
                }
            }
        }

        pub(crate) fn new(program: &'program TypedTrees, stdin: &'program [u8]) -> Self {
            let step_budget = ambient_step_budget();
            Self {
                program,
                product_declarations: program,
                checked: None,
                operator_facts: None,
                boundary_adapter_dispatch: &[],
                selected_product_float_comparisons: None,
                selected_build_time_operators: &[],
                selected_build_time_machine_bodies: &[],
                x86_flags: None,
                semantic_float_evaluation: false,
                stdout: Vec::new(),
                stderr: Vec::new(),
                build_log: Vec::new(),
                root_build: None,
                executed_source_selections: Vec::new(),
                pending_build_selections: Vec::new(),
                executed_behavior_exclusions: Vec::new(),
                executed_build_selections: Vec::new(),
                executed_wire_demands: Vec::new(),
                product_entry_descriptions: Vec::new(),
                product_entry_slots: None,
                product_schema_descriptions: Vec::new(),
                product_provider_descriptions: Vec::new(),
                stdin,
                stdin_cursor: 0,
                virtual_ticks: 0,
                virtual_clock_bindings: Vec::new(),
                virtual_live_windows: std::collections::HashSet::new(),
                virtual_window_next: 0,
                virtual_files: BTreeMap::new(),
                virtual_fds: BTreeMap::new(),
                virtual_next_fd: 3,
                virtual_dirs: std::collections::BTreeSet::new(),
                filesystem_metadata_layout: FilesystemMetadataLayout::default(),
                virtual_finds: BTreeMap::new(),
                virtual_next_find: 1,
                virtual_perms: BTreeMap::new(),
                virtual_symlinks: BTreeMap::new(),
                virtual_times: BTreeMap::new(),
                virtual_flocks: BTreeMap::new(),
                virtual_char_devices: [b"/dev/null".to_vec(), b"/dev/zero".to_vec()]
                    .into_iter()
                    .collect(),
                virtual_errno: 0,
                real_fs: None,
                rooted_build_paths_required: false,
                build_included_sources: Vec::new(),
                output_obligations: Vec::new(),
                output_receipts: Vec::new(),
                output_obligation_paths: BTreeMap::new(),
                output_seal_states: BTreeMap::new(),
                output_open_writers: BTreeMap::new(),
                host_boundary_touched: false,
                filesystem_operation_attempts: Vec::new(),
                filesystem_logical_handles: FilesystemLogicalHandles::default(),
                filesystem_live_handle_leases: BTreeMap::new(),
                filesystem_observation_path_bytes: 0,
                filesystem_observation_evidence_bytes: 0,
                filesystem_observation_resource_halt: None,
                filesystem_operation_attempt_stack: Vec::new(),
                private_layout_placements: Vec::new(),
                usage: EvaluationUsage::empty(step_budget),
                cell_meter: CellMeter::new(None),
                text_byte_meter: TextByteMeter::new(None),
                build_evaluation_sponsor: None,
                step_budget,
                call_depth: 0,
                guard_depth: 0,
            }
        }

        pub(crate) fn new_checked(checked: &'program CheckedTrees, stdin: &'program [u8]) -> Self {
            let mut evaluator = Self::new(&checked.typed, stdin);
            evaluator.checked = Some(checked);
            evaluator.operator_facts = Some(&checked.facts.operators);
            evaluator
        }

        pub(crate) fn tick(&mut self) -> EvalResult<()> {
            self.usage
                .charge_step()
                .ok_or_else(|| Halt::Trap("evaluator usage overflowed".to_owned()))?;
            if self.usage.work_units() > self.step_budget {
                return trap("step budget exceeded");
            }
            if let Some(sponsor) = &self.build_evaluation_sponsor {
                sponsor.charge_work_unit().map_err(Halt::Resource)?;
            }
            Ok(())
        }

        pub(crate) fn configure_build_evaluation(
            &mut self,
            work_budget: u64,
            sponsor: Option<BuildEvaluationSponsor>,
        ) {
            debug_assert_eq!(self.cell_meter.peak(), 0);
            debug_assert_eq!(self.text_byte_meter.peak(), 0);
            self.step_budget = work_budget;
            self.semantic_float_evaluation = true;
            self.usage.set_work_budget(work_budget);
            self.cell_meter = CellMeter::new(sponsor.clone());
            self.text_byte_meter = TextByteMeter::new(sponsor.clone());
            self.build_evaluation_sponsor = sponsor;
        }

        pub(crate) fn allocate_cell(&self, value: Value) -> EvalResult<Cell> {
            self.cell_meter.allocate(value).map_err(Halt::Resource)
        }

        pub(crate) fn allocate_text(&self, bytes: Vec<u8>) -> EvalResult<Value> {
            self.text_byte_meter.allocate(bytes).map_err(Halt::Resource)
        }

        pub(crate) fn finish_cell_usage(&mut self) {
            self.usage.record_peak_live_cells(self.cell_meter.peak());
            self.usage
                .record_peak_live_text_bytes(self.text_byte_meter.peak());
        }

        pub(crate) fn charge_build_log_bytes(&mut self, bytes: usize) -> EvalResult<()> {
            let bytes = u64::try_from(bytes)
                .map_err(|_| Halt::Resource("BuildLog byte count overflowed".to_owned()))?;
            if let Some(sponsor) = &self.build_evaluation_sponsor {
                sponsor
                    .charge_build_log_bytes(bytes)
                    .map_err(Halt::Resource)?;
            }
            self.usage
                .charge_build_log_bytes(bytes)
                .ok_or_else(|| Halt::Resource("BuildLog byte accounting overflowed".to_owned()))
        }

        pub(crate) fn charge_filesystem_operation_attempt(&mut self) -> EvalResult<()> {
            if let Some(sponsor) = &self.build_evaluation_sponsor {
                sponsor
                    .charge_filesystem_operation_attempt()
                    .map_err(Halt::Resource)?;
            }
            self.usage
                .charge_filesystem_operation_attempt()
                .ok_or_else(|| {
                    Halt::Resource("filesystem operation-attempt accounting overflowed".to_owned())
                })
        }

        pub(crate) fn run_entry(&mut self, entry_machine_name: &str) -> EvalResult<()> {
            let entry_machine = self
                .find_machine_by_name(entry_machine_name)
                .ok_or_else(|| {
                    Halt::Unsupported(format!("no entry machine `{entry_machine_name}`"))
                })?;
            self.run_resolved_entry(entry_machine)
        }

        pub(crate) fn run_entry_symbol(
            &mut self,
            entry_machine_symbol: SymbolHandle,
        ) -> EvalResult<()> {
            let entry_machine = self
                .find_machine_by_symbol(entry_machine_symbol)
                .ok_or_else(|| {
                    Halt::Unsupported(
                        "the selected program entry is not a machine in the checked program"
                            .to_owned(),
                    )
                })?;
            self.run_resolved_entry(entry_machine)
        }

        fn run_resolved_entry(&mut self, entry_machine: &'program Machine) -> EvalResult<()> {
            let entry_machine_name = entry_machine.name.as_str();
            let entry_state = self.machine_entry_state(entry_machine).ok_or_else(|| {
                Halt::Unsupported(format!(
                    "entry machine `{entry_machine_name}` has no executable state"
                ))
            })?;

            let arguments = self.entry_arrival_arguments(entry_machine, entry_state)?;
            let returned = self.run_state_collect(entry_machine, entry_state, arguments)?;
            if let Some(value) = returned
                && let Some(code) = value.as_int()
            {
                return Err(Halt::Exit(code as i32));
            }
            Ok(())
        }

        fn entry_arrival_arguments(
            &mut self,
            entry_machine: &Machine,
            entry_state: &'program State,
        ) -> EvalResult<Vec<EvaluatedArgument>> {
            if let Some(parameter) = self.program.state_parameters(entry_state).first() {
                return unsupported(format!(
                    "entry machine `{}` parameter `{}` has no installed arrival value",
                    entry_machine.name.as_str(),
                    parameter.name.as_str()
                ));
            }
            Ok(Vec::new())
        }

        pub(crate) fn run_build_time_machine(
            &mut self,
            machine_name: &str,
            arguments: Vec<build_time_values::BuildTimeValue>,
        ) -> EvalResult<build_time_values::BuildTimeValue> {
            let machine = self
                .find_machine_by_name(machine_name)
                .ok_or_else(|| Halt::Trap(format!("no machine named `{machine_name}` exists")))?;
            self.run_resolved_build_time_machine(machine, arguments)
        }

        pub(crate) fn run_build_time_machine_symbol(
            &mut self,
            machine_symbol: SymbolHandle,
            arguments: Vec<build_time_values::BuildTimeValue>,
        ) -> EvalResult<build_time_values::BuildTimeValue> {
            let machine = self.find_machine_by_symbol(machine_symbol).ok_or_else(|| {
            Halt::Trap(format!(
                "no machine with exact symbol {machine_symbol:?} exists in the evaluated program"
            ))
        })?;
            self.run_resolved_build_time_machine(machine, arguments)
        }

        fn build_time_entry_state(
            &mut self,
            machine: &'program Machine,
            argument_count: usize,
        ) -> EvalResult<&'program State> {
            let machine_name = machine.name.as_str();
            let entry_state = self.machine_entry_state(machine).ok_or_else(|| {
                Halt::Trap(format!(
                    "machine `{machine_name}` has no states to evaluate"
                ))
            })?;
            let parameter_count = self.program.state_parameters(entry_state).len();
            if parameter_count != argument_count {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` takes {parameter_count} argument(s); the build-time                  position supplied {}",
                    argument_count
                )));
            }
            Ok(entry_state)
        }

        fn run_resolved_build_time_machine(
            &mut self,
            machine: &'program Machine,
            arguments: Vec<build_time_values::BuildTimeValue>,
        ) -> EvalResult<build_time_values::BuildTimeValue> {
            let machine_name = machine.name.as_str();
            let entry_state = self.build_time_entry_state(machine, arguments.len())?;

            let argument_cells = arguments
                .into_iter()
                .zip(self.program.state_parameters(entry_state))
                .map(|(argument, parameter)| {
                    let mut value = crate::build_time::materialize_build_time_value(
                        argument,
                        &|value| self.allocate_cell(value),
                        &|bytes| self.allocate_text(bytes),
                    )?;
                    self.bind_build_time_argument(&mut value, parameter.type_reference)?;
                    Ok(EvaluatedArgument::plain(self.allocate_cell(value)?))
                })
                .collect::<EvalResult<Vec<_>>>()?;
            let returned = self.run_state_collect(machine, entry_state, argument_cells)?;
            let value = returned.ok_or_else(|| {
                Halt::Trap(format!(
                    "machine `{machine_name}` terminated without producing a value"
                ))
            })?;
            if self.host_boundary_touched {
                return Err(Halt::Trap(format!(
                    "machine `{machine_name}` is not effect-free: it drove a host-boundary call \
                 during build-time evaluation"
                )));
            }
            observe_build_time_value(&value, &|reference| self.read_reference(reference))
        }

        pub(crate) fn populate_data_fields(
            &mut self,
            data: &DataDefinition,
            fields: &mut BTreeMap<crate::value::ValueName, Cell>,
        ) -> EvalResult<()> {
            self.populate_data_fields_with_bindings(data, fields, &[])
        }

        fn populate_data_fields_with_bindings(
            &mut self,
            data: &DataDefinition,
            fields: &mut BTreeMap<crate::value::ValueName, Cell>,
            bindings: &[(
                SymbolHandle,
                String,
                typed_trees::types::TypeReferenceHandle,
            )],
        ) -> EvalResult<()> {
            let program = self.program;
            for member in program.data_members(data) {
                let Some(field) = member.as_field() else {
                    continue;
                };
                let name = field.name.as_str().to_owned();
                let value =
                    self.default_value_for_type_with_bindings(field.type_reference, bindings)?;
                fields.insert(name.into(), self.allocate_cell(value)?);
            }
            Ok(())
        }

        pub(crate) fn default_value_for_type(
            &mut self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> EvalResult<Value> {
            self.default_value_for_type_with_bindings(type_reference, &[])
        }

        fn default_value_for_type_with_bindings(
            &mut self,
            type_reference: typed_trees::types::TypeReferenceHandle,
            bindings: &[(
                SymbolHandle,
                String,
                typed_trees::types::TypeReferenceHandle,
            )],
        ) -> EvalResult<Value> {
            if type_reference.is_valid() {
                if let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                    type_reference_table(self.program).type_reference(type_reference)
                    && let Some(argument) =
                        self.generic_binding_argument(*symbol, name.as_str(), bindings)
                {
                    return self.default_value_for_type_with_bindings(argument, bindings);
                }

                if let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                    type_reference_table(self.program).type_reference(type_reference)
                {
                    let base_type = *base_type;
                    return self.default_value_for_type_with_bindings(base_type, bindings);
                }

                if let typed_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } = type_reference_table(self.program).type_reference(type_reference)
                {
                    let element_type = *element_type;
                    let count = match length {
                        typed_trees::types::FixedArrayLength::Literal(count) => Some(*count),
                        typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } => {
                            self.generic_binding_argument(*symbol, name.as_str(), bindings)
                                .and_then(|argument| {
                                    self.const_argument_value_with_bindings(argument, bindings, 0)
                                })
                        }
                        typed_trees::types::FixedArrayLength::Expression(_) => {
                            return unsupported(
                                "array extent expression has not completed checked constant evaluation",
                            );
                        }
                        typed_trees::types::FixedArrayLength::ConstCall { name, .. } => {
                            match self.program.closed_fixed_array_length(type_reference) {
                                Some(count) => Some(count),
                                None => {
                                    return unsupported(format!(
                                        "array length `{}` has an unresolved build-time evaluation dependency",
                                        name.as_str()
                                    ));
                                }
                            }
                        }
                    };
                    if let Some(count) = count {
                        let mut elements: Vec<Cell> = Vec::with_capacity(count);
                        let mut copies_by_cells = false;
                        for _ in 0..count {
                            let value = match elements.first() {
                                Some(first) if copies_by_cells => first
                                    .borrow()
                                    .deep_clone_with(&|value| self.allocate_cell(value))?,
                                _ => self
                                    .default_value_for_type_with_bindings(element_type, bindings)?,
                            };
                            if elements.is_empty() {
                                copies_by_cells = default_copies_by_cells(&value);
                            }
                            elements.push(self.allocate_cell(value)?);
                        }
                        return Ok(Value::Array(elements));
                    }
                }

                if let typed_trees::types::TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } = type_reference_table(self.program).type_reference(type_reference)
                {
                    let program = self.program;
                    let definition = program.closed_data_definition(type_reference).or_else(|| {
                        data_definition_by_symbol_or_name(program, *base_symbol, base_name)
                    });
                    if let Some(definition) = definition
                        && matches!(
                            DataDefinition::shape_kind_from_members(
                                self.program.data_members(definition)
                            ),
                            typed_trees::data::DataShapeKind::Record
                        )
                    {
                        let parameters = self.program.data_type_parameters(definition).to_vec();
                        let arguments = type_reference_table(self.program)
                            .type_reference_handles(*arguments)
                            .to_vec();
                        let mut nested_bindings = bindings.to_vec();
                        nested_bindings.extend(parameters.iter().zip(arguments).map(
                            |(parameter, argument)| {
                                (
                                    parameter.symbol,
                                    parameter.name.as_str().to_owned(),
                                    argument,
                                )
                            },
                        ));
                        let mut nested_fields = BTreeMap::new();
                        self.populate_data_fields_with_bindings(
                            definition,
                            &mut nested_fields,
                            &nested_bindings,
                        )?;
                        return Ok(Value::Struct {
                            type_symbol: definition.symbol,
                            type_name: definition.name.as_str().into(),
                            fields: nested_fields,
                        });
                    }
                }

                if let Some(nested) = self.field_nested_data(type_reference) {
                    let mut nested_fields = BTreeMap::new();
                    self.populate_data_fields(nested, &mut nested_fields)?;
                    return Ok(Value::Struct {
                        type_symbol: nested.symbol,
                        type_name: nested.name.as_str().into(),
                        fields: nested_fields,
                    });
                }

                if let Some((type_symbol, variant_name, payload_fields)) =
                    self.enum_zero_case(type_reference)
                {
                    let mut payload = Vec::with_capacity(payload_fields.len());
                    for field in payload_fields {
                        let value = self
                            .default_value_for_type_with_bindings(field.type_reference, bindings)?;
                        payload.push((field.name.as_str().into(), self.allocate_cell(value)?));
                    }
                    return Ok(Value::Enum {
                        type_symbol,
                        variant_name: variant_name.into(),
                        payload,
                    });
                }
            }
            self.default_for_type(type_reference)
        }

        fn generic_binding_argument(
            &self,
            symbol: SymbolHandle,
            name: &str,
            bindings: &[(
                SymbolHandle,
                String,
                typed_trees::types::TypeReferenceHandle,
            )],
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            bindings
                .iter()
                .find(|(parameter, spelling, _)| {
                    if symbol.is_valid() {
                        *parameter == symbol
                    } else {
                        spelling == name
                    }
                })
                .map(|(_, _, argument)| *argument)
        }

        fn const_argument_value_with_bindings(
            &self,
            argument: typed_trees::types::TypeReferenceHandle,
            bindings: &[(
                SymbolHandle,
                String,
                typed_trees::types::TypeReferenceHandle,
            )],
            depth: usize,
        ) -> Option<usize> {
            if depth >= 16 {
                return None;
            }
            if let Some(value) =
                type_reference_table(self.program).anonymous_numeric_literal_value(argument)
            {
                return Some(value);
            }
            let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                self.program.type_reference_table.type_reference(argument)
            else {
                return None;
            };
            let argument = self.generic_binding_argument(*symbol, name.as_str(), bindings)?;
            self.const_argument_value_with_bindings(argument, bindings, depth + 1)
        }

        fn enum_zero_case(
            &self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<(SymbolHandle, String, Vec<typed_trees::data::DataField>)> {
            if self
                .program
                .primitive_type_reference(type_reference)
                .is_some()
            {
                return None;
            }
            let symbol = self.program.type_reference_symbol(type_reference);
            if !symbol.is_valid() {
                return None;
            }
            let data = self.program.data_definition_by_symbol(symbol)?;
            let members = self.program.data_members(data);
            let first_variant = members.iter().find_map(|member| match member {
                DataMember::Variant(variant) => Some(variant),
                _ => None,
            })?;
            let mut fields: Vec<typed_trees::data::DataField> = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.clone()),
                    _ => None,
                })
                .collect();
            fields.extend(self.program.data_payload_fields(first_variant).to_vec());
            Some((data.symbol, first_variant.name.as_str().to_owned(), fields))
        }

        fn field_nested_data(
            &self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<&'program DataDefinition> {
            if !type_reference.is_valid() {
                return None;
            }
            if self
                .program
                .primitive_type_reference(type_reference)
                .is_some()
            {
                return None;
            }
            let symbol = self.program.type_reference_symbol(type_reference);
            if !symbol.is_valid() {
                return None;
            }
            self.program
                .data_definition_by_symbol(symbol)
                .filter(|data| {
                    !matches!(
                        DataDefinition::shape_kind_from_members(self.program.data_members(data)),
                        typed_trees::data::DataShapeKind::Enum
                            | typed_trees::data::DataShapeKind::Mixed
                    )
                })
        }

        pub(crate) fn default_for_type(
            &self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> EvalResult<Value> {
            Ok(
                match self.program.primitive_type_reference(type_reference) {
                    Some(PrimitiveType::Bool) => Value::Bool(false),
                    Some(PrimitiveType::F32) | Some(PrimitiveType::F64) => Value::Float(0.0),
                    Some(_) => Value::Int(0),
                    None if self.program.is_borrowed_byte_slice(type_reference) => {
                        self.allocate_text(Vec::new())?
                    }
                    None => Value::Unit,
                },
            )
        }

        pub(crate) fn run_state_collect(
            &mut self,
            machine: &'program Machine,
            state: &'program State,
            arguments: Vec<EvaluatedArgument>,
        ) -> EvalResult<Option<Value>> {
            self.call_depth += 1;
            if self.call_depth > CALL_DEPTH_BUDGET {
                self.call_depth -= 1;
                return unsupported("recursion depth budget exceeded");
            }
            let result = self.run_state_collect_inner(machine, state, arguments);
            self.call_depth -= 1;
            result
        }

        fn run_state_collect_inner(
            &mut self,
            machine: &'program Machine,
            state: &'program State,
            arguments: Vec<EvaluatedArgument>,
        ) -> EvalResult<Option<Value>> {
            let mut machine = machine;
            let mut state = state;
            let mut current_args = arguments;
            let mut carried: Vec<FrameLocal> = Vec::new();

            loop {
                self.tick()?;

                if machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred {
                    return unsupported(format!(
                        "machine `{}` requires implementation completion before execution",
                        machine.name.as_str(),
                    ));
                }

                if !machine.body_is_present {
                    return unsupported(format!(
                        "call target `{}` is a bodyless requirement or external declaration; the interpreter has no provider for its selected realization",
                        machine.name.as_str(),
                    ));
                }

                let mut frame = self.bind_frame(state, &current_args, machine.symbol, carried)?;
                self.validate_normal_cleanup_selection(machine, state)?;

                let mut next: Option<TransitionDecision<'program>> = None;
                let mut tail_value: Option<Value> = None;
                let mut exit = checked_trees::FlowClaimJoinExit {
                    state_symbol: state.symbol,
                    statement_index: statement_table(self.program)
                        .statements(state.statement_nodes)
                        .len(),
                    kind: checked_trees::FlowClaimJoinExitKind::FinalExpression,
                };
                let order = statement_table(self.program)
                    .finite_execution_order(state.statement_nodes)
                    .map_err(Halt::Trap)?;
                let Some(order) = order else {
                    return unsupported(
                        "cyclic assembly control requires a checked execution plan".to_owned(),
                    );
                };
                for statement_index in order {
                    let statement_handle = state.statement_nodes.handle_at(statement_index as u32);
                    let statement = self.program.statement_table.statement(statement_handle);
                    match statement {
                        StatementNode::Transition(transition) => {
                            if let Some((decision, kind)) =
                                self.eval_transition(transition, &mut frame)?
                            {
                                exit.statement_index = statement_index;
                                exit.kind = kind;
                                next = Some(decision);
                                break;
                            }
                        }
                        StatementNode::Expression(expression) => {
                            if expression_table(self.program)
                                .call_node(*expression)
                                .and_then(|call| call.native_code_layout.as_ref())
                                .is_some_and(|query| query.discards_result)
                            {
                                continue;
                            }

                            if statement_index + 1
                                == statement_table(self.program)
                                    .statements(state.statement_nodes)
                                    .len()
                            {
                                exit.statement_index = statement_index;
                            }
                            tail_value = Some(self.eval_expression_at_type(
                                *expression,
                                frame.return_type,
                                &mut frame,
                            )?);
                        }
                        StatementNode::Call(call) => {
                            self.eval_call_statement(statement_handle, call, &mut frame)?;
                        }
                        other => {
                            self.exec_statement(other, statement_index, &mut frame)?;
                        }
                    }
                }

                match next {
                    None => {
                        self.execute_completion_cleanup(
                            machine,
                            state,
                            &frame,
                            exit,
                            CompletionBoundary::NormalReturn,
                        )?;
                        return Ok(tail_value);
                    }
                    Some(TransitionDecision::Value(value)) => {
                        self.execute_completion_cleanup(
                            machine,
                            state,
                            &frame,
                            exit,
                            CompletionBoundary::NormalReturn,
                        )?;
                        return Ok(Some(value));
                    }
                    Some(TransitionDecision::Terminal) => {
                        self.execute_completion_cleanup(
                            machine,
                            state,
                            &frame,
                            exit,
                            CompletionBoundary::NormalReturn,
                        )?;
                        return Ok(None);
                    }
                    Some(TransitionDecision::Named {
                        state: target_state,
                        machine: target_machine,
                        arguments,
                    }) => {
                        self.execute_completion_cleanup(
                            machine,
                            state,
                            &frame,
                            exit,
                            CompletionBoundary::TerminalTransfer,
                        )?;
                        if target_machine.symbol == machine.symbol {
                            carried = frame.locals;
                            state = target_state;
                            current_args = arguments;
                            continue;
                        }
                        machine = target_machine;
                        state = target_state;
                        current_args = arguments;
                        carried = Vec::new();
                        continue;
                    }
                }
            }
        }

        fn bind_frame(
            &self,
            state: &State,
            arguments: &[EvaluatedArgument],
            machine_symbol: SymbolHandle,
            carried: Vec<FrameLocal>,
        ) -> EvalResult<Frame> {
            self.check_state_argument_count(state, arguments.len())?;
            let mut frame = Frame {
                return_type: state.return_type,
                locals: carried,
                machine_symbol,
                state_symbol: state.symbol,
                guard_call_results: Vec::new(),
                scalar_expression_depth: 0,
                scalar_expression_values: Vec::new(),
                match_subjects: Vec::new(),
            };
            for (parameter, argument) in self.program.state_parameters(state).iter().zip(arguments)
            {
                let argument = argument.clone();
                let cell = argument.cell;
                let mut scalar = None;
                let cell = match self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                {
                    Some(primitive) => {
                        let domain = self
                            .program
                            .arithmetic_domain_for_type_reference(parameter.type_reference);
                        scalar = Some((primitive, domain));
                        let scalar = match &*cell.borrow() {
                            v @ (Value::Int(_) | Value::Float(_) | Value::UndeterminedNaN(_)) => {
                                Some(v.clone())
                            }
                            _ => None,
                        };
                        match scalar {
                            Some(value) => {
                                let value = self.coerce_scalar_with(value, primitive, domain)?;
                                self.allocate_cell(value)?
                            }
                            None => cell,
                        }
                    }
                    None => cell,
                };
                frame.bind(FrameLocal {
                    symbol: parameter.symbol,
                    cell,
                    type_reference: parameter.type_reference,
                    scalar,
                });
            }
            Ok(frame)
        }
    }

    fn default_copies_by_cells(value: &Value) -> bool {
        match value {
            Value::Unit
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Float(_)
            | Value::UndeterminedNaN(_) => true,
            Value::Str(_) | Value::Ref(_) => false,
            Value::Struct { fields, .. } => fields
                .values()
                .all(|cell| default_copies_by_cells(&cell.borrow())),
            Value::Enum { payload, .. } => payload
                .iter()
                .all(|(_, cell)| default_copies_by_cells(&cell.borrow())),
            Value::Array(elements) => elements
                .iter()
                .all(|cell| default_copies_by_cells(&cell.borrow())),
        }
    }
}

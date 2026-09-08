//! Substitute one selected or input contract component at a time.
use super::*;
use semantic_vocabulary::ObligationId;

pub(super) const SELECTED_COUNT: usize = 25;
pub(super) const SOURCE_COUNT: usize = 33;

pub(super) fn selected(selected: &mut SelectedFunction, scalar_count: usize, mutation: usize) {
    let call_id = selected.calls[0].instruction;
    let rows = &mut selected.blocks[0].instructions;
    let position = rows.iter().position(|row| row.id == call_id).unwrap();
    let pointer = rows[position].operands[scalar_count].virtual_register;
    match mutation {
        0 => rows[position].operands.swap(0, scalar_count),
        1 => {
            rows[position].operands[scalar_count].fixed_view = rows[position].operands[0].fixed_view
        }
        2 => {
            rows[position].operands[0].fixed_view = rows[position].operands[scalar_count].fixed_view
        }
        3 => {
            rows[position].operands[scalar_count].virtual_register =
                rows[position].operands[0].virtual_register
        }
        4 => {
            selected.virtual_registers[pointer.0 as usize].origin = selected.virtual_registers
                [rows[position].operands[0].virtual_register.0 as usize]
                .origin
        }
        5 => selected.virtual_registers[pointer.0 as usize].scalar_type = ScalarType::Boolean,
        6 => {
            selected.virtual_registers[pointer.0 as usize].definition_site =
                Some(ValueDefinitionSite::FunctionParameter(0))
        }
        7 => {
            selected.virtual_registers[0].origin = VirtualRegisterOrigin::StructuralParameter {
                place: PlaceId::new(1).unwrap(),
                parameter_index: scalar_count,
            }
        }
        8 => selected.calls[0].call.arguments.swap(0, scalar_count),
        9 => {
            let LegalizedScalarArgument::Structural { target, .. } =
                &mut selected.calls[0].call.arguments[scalar_count]
            else {
                unreachable!();
            };
            target.source = target.destination.clone();
            target.source.shape = ValueShape::integer(8, 8);
        }
        10 => {
            let LegalizedScalarArgument::Structural { target, .. } =
                &mut selected.calls[0].call.arguments[scalar_count]
            else {
                unreachable!();
            };
            target.structural_type = StructuralTypeId::new(99).unwrap();
        }
        11 => selected.calls[0].ownership.clear(),
        12 => selected.calls[0].effect.output += 1,
        13 => selected.calls[0]
            .call
            .requirement_obligations
            .push(ObligationId::new(99).unwrap()),
        14 => rows[position]
            .provenance
            .obligations
            .push(ObligationId::new(99).unwrap()),
        15 => rows[position].provenance.values[0] = ValueId::new(1).unwrap(),
        16 => rows[position].provenance.fuel[0].units += 1,
        17 => {
            let row = rows
                .iter_mut()
                .find(|row| matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. }))
                .unwrap();
            let SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } = &mut row.kind else {
                unreachable!();
            };
            *accepted_fact =
                optimization_core::AcceptedObligationFactIdentity::from_bytes([99; 32]);
        }
        18 => {
            let constant = rows
                .iter()
                .find(|row| matches!(row.kind, SelectedInstructionKind::MaterializeI64 { .. }))
                .unwrap()
                .operands[0]
                .virtual_register;
            rows[position - scalar_count - 1].operands[0].virtual_register = constant;
        }
        19 => {
            let short_result = rows[position].operands[scalar_count + 1].virtual_register;
            let next_id = selected.calls[1].instruction;
            let block = selected
                .blocks
                .iter_mut()
                .find(|block| block.instructions.iter().any(|row| row.id == next_id))
                .unwrap();
            let next_position = block
                .instructions
                .iter()
                .position(|row| row.id == next_id)
                .unwrap();
            block.instructions[next_position - scalar_count - 1].operands[0].virtual_register =
                short_result;
        }
        20 => rows[position - 1].operands[0].virtual_register = VirtualRegisterId(0),
        21 => {
            selected.calls.remove(0);
        }
        22 => rows[position].operands[scalar_count].access = RegisterOperandAccess::Def,
        23 => {
            selected.structural.as_mut().unwrap().parameters[0]
                .target
                .shape = ValueShape::integer(16, 8)
        }
        24 => rows[position]
            .clobbers
            .push(register_model::RegisterUnitId(999)),
        _ => unreachable!(),
    }
}

pub(super) fn source(source: &mut LegalizedScalarFunction, scalar_count: usize, mutation: usize) {
    let row = &mut source.blocks[0].instructions[2];
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        unreachable!();
    };
    let LegalizedScalarArgument::Structural { semantic, target } =
        &mut call.arguments[scalar_count]
    else {
        unreachable!();
    };
    match mutation {
        0 => call.arguments.swap(0, scalar_count),
        1 => semantic.access = StructuralAccess::Owned,
        2 => target.access = StructuralAccess::MutableBorrow,
        3 => target.place = PlaceId::new(99).unwrap(),
        4 => {
            semantic.place = PlaceId::new(99).unwrap();
            target.place = semantic.place;
        }
        5 => target.root_structural_type = StructuralTypeId::new(99).unwrap(),
        6 => target.structural_type = StructuralTypeId::new(99).unwrap(),
        7 => target.shape = ValueShape::integer(16, 8),
        8 => target.source_byte_offset = 8,
        9 => target.fixed_array_length = Some(2),
        10 => target.element_stride = Some(8),
        11 => target.source.shape = ValueShape::integer(16, 8),
        12 => target.destination.shape = ValueShape::integer(16, 8),
        13 => semantic
            .path
            .push(terminal_psi::StructuralPathSegment::FixedIndex(0)),
        14 => target
            .path
            .push(terminal_psi::StructuralPathSegment::FixedIndex(0)),
        15 => {
            let ValueLocation::Register { register, .. } =
                call.call_plan.parameters[0].locations[0]
            else {
                unreachable!();
            };
            let ValueLocation::Indirect { pointer, .. } =
                &mut call.call_plan.parameters[scalar_count].locations[0]
            else {
                unreachable!();
            };
            *pointer = IndirectPointerLocation::Register(register);
            target.destination = call.call_plan.parameters[scalar_count].clone();
        }
        16 => {
            let ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(pointer),
                ..
            } = target.destination.locations[0]
            else {
                unreachable!();
            };
            let ValueLocation::Register { register, .. } =
                &mut call.call_plan.parameters[0].locations[0]
            else {
                unreachable!();
            };
            *register = pointer;
            let LegalizedScalarArgument::Scalar { placement, .. } = &mut call.arguments[0] else {
                unreachable!();
            };
            *placement = call.call_plan.parameters[0].clone();
        }
        17 => {
            let ValueLocation::Indirect {
                copy_stack_byte_offset,
                ..
            } = &mut call.call_plan.parameters[scalar_count].locations[0]
            else {
                unreachable!();
            };
            *copy_stack_byte_offset = Some(0);
            target.destination = call.call_plan.parameters[scalar_count].clone();
        }
        18 => row.ownership.clear(),
        19 => call
            .requirement_obligations
            .push(ObligationId::new(99).unwrap()),
        20 => row
            .ownership
            .push(optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())),
        21 => {
            row.result.as_mut().unwrap().scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
        }
        22 => {
            source.blocks[0].instructions[1]
                .result
                .as_mut()
                .unwrap()
                .scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
        }
        23 => {
            let LegalizedScalarArgument::Scalar { source, .. } = &mut call.arguments[0] else {
                unreachable!();
            };
            *source = ValueId::new(99).unwrap();
        }
        24 => {
            source.structural.as_mut().unwrap().parameters[0]
                .semantic
                .position = scalar_count as u32
        }
        25 => {
            source.structural.as_mut().unwrap().parameters[0]
                .semantic
                .is_self = true;
        }
        26 => source.attachment = Some(StructuralTypeId::new(99).unwrap()),
        27 => {
            row.result = None;
            call.result_placement = None;
            call.call_plan.result = None;
        }
        28 => {
            let mut shapes = vec![ValueShape::integer(8, 8); scalar_count + 1];
            shapes[scalar_count] = ValueShape::integer(16, 8);
            call.call_plan = evaluate_call_plan(
                call.call_plan.policy,
                &CallSignature {
                    parameters: shapes,
                    result: Some(ValueShape::integer(8, 8)),
                },
            )
            .unwrap();
            target.shape = ValueShape::integer(16, 8);
            target.destination = call.call_plan.parameters[scalar_count].clone();
        }
        29 => {
            source.structural.as_mut().unwrap().parameters[0]
                .target
                .placement
                .shape = ValueShape::integer(8, 8);
        }
        30 => {
            // Even synchronized source-home substitutions must rejoin the native ABI.
            let result_register = match source.call_plan.result.as_ref().unwrap().locations[0] {
                ValueLocation::Register { register, .. } => register,
                _ => unreachable!(),
            };
            let pointer_placement = source.call_plan.parameters.last_mut().unwrap();
            let ValueLocation::Indirect { pointer, .. } = &mut pointer_placement.locations[0]
            else {
                unreachable!();
            };
            // ARM's first parameter and result share a register; a shape change
            // makes this control effective for zero-prefix callers there too.
            *pointer = IndirectPointerLocation::Register(result_register);
            pointer_placement.shape.alignment = 16;
            source.structural.as_mut().unwrap().parameters[0]
                .target
                .placement = pointer_placement.clone();
            target.source = pointer_placement.clone();
        }
        31 => {
            // An absent scalar declaration cannot be inferred from ABI shape.
            if source.parameters.is_empty() {
                source.parameters.push(LegalizedScalarParameter {
                    value: ValueId::new(99).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                    definition_site: ValueDefinitionSite::FunctionParameter(0),
                    placement: call.call_plan.parameters[0].clone(),
                });
            } else {
                source.parameters.pop();
            }
        }
        32 => {
            // The scalar lane cannot acquire another view at the same pointer width.
            call.arguments[0] = call.arguments[scalar_count].clone();
            call.call_plan.parameters[0] = call.call_plan.parameters[scalar_count].clone();
        }
        _ => unreachable!(),
    }
}

use super::scalar::build_function;
use crate::selection::constraints::{instruction, require_key_rows, row};
use crate::selection::shared::*;

pub(in crate::selection) fn build_plan(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedInstructionPlan, SelectedInstructionError> {
    let target = legalized.plan();
    require_key_rows(constraints.keys, catalog)?;
    let mut functions = target
        .functions
        .iter()
        .enumerate()
        .map(|(index, source)| build_function(index, source, constraints, physical, catalog))
        .collect::<Result<Vec<_>, _>>()?;
    functions.extend(
        target
            .unit_functions
            .iter()
            .map(|source| build_unit_function(source, constraints.keys, catalog))
            .collect::<Result<Vec<_>, _>>()?,
    );
    functions.sort_by_key(|function| function.machine);
    let mut structural_unit_functions = target
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            build_structural_unit_function(index, source, target, constraints.keys, catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structural_unit_functions.sort_by_key(|function| function.machine);
    Ok(SelectedInstructionPlan {
        psi: target.psi,
        fuel_schedule: target.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions,
        structural_unit_functions,
    })
}

fn build_unit_function(
    source: &SourceUnitFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedFunction, SelectedInstructionError> {
    Ok(SelectedFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![SelectedBlock {
            id: SelectedBlockId(0),
            source_block: source.entry_block,
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(0),
                    SelectedInstructionKind::ReturnUnit,
                    keys.return_unit,
                    &[],
                    SelectedInstructionProvenance {
                        edges: vec![source.return_edge],
                        fuel: source.return_fuel.clone(),
                        ..Default::default()
                    },
                    catalog,
                )?,
                psi_return_edge: source.return_edge,
            },
        }],
    })
}

pub(in crate::selection) fn structural_unit_layout(
    function: usize,
    source: &SourceStructuralUnitFunction,
) -> Result<SelectedMicrosoftX64OwnedIndirectPairLayout, SelectedInstructionError> {
    if source.call_plan.policy != CallingPolicy::MicrosoftX64
        || source.call_plan.result.is_some()
        || !source.call_plan.callback_materializations.is_empty()
        || source.call_plan.stack_alignment != 16
        || source.call_plan.shadow_bytes != 32
        || source.call_plan.entry_control != EntryControl::CallReturn
        || source.parameters.len() != 2
        || source.call_plan.parameters.len() != 2
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    let pointers = [MachineRegister::X86Rcx, MachineRegister::X86Rdx];
    let offsets = [32, 48];
    let mut bindings = [SelectedStructuralUnitIndirectBinding {
        parameter_index: 0,
        pointer: pointers[0],
        copy_stack_byte_offset: offsets[0],
        byte_count: 16,
        alignment: 8,
    }; 2];
    for (index, parameter) in source.parameters.iter().enumerate() {
        if parameter.semantic.position != index as u32
            || parameter.semantic.is_self
            || parameter.semantic.access != StructuralAccess::Owned
            || parameter.target.place != parameter.semantic.place
            || parameter.target.structural_type != parameter.semantic.structural_type
            || parameter.target.multiplicity != parameter.semantic.multiplicity
            || parameter.target.access != StructuralAccess::Owned
            || parameter.target.shape.class != ValueClass::Integer
            || parameter.target.shape.byte_size != 16
            || parameter.target.shape.alignment != 8
            || parameter.target.placement != source.call_plan.parameters[index]
            || parameter.target.placement.locations.len() != 1
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        let ValueLocation::Indirect {
            pointer: IndirectPointerLocation::Register(pointer),
            copy_stack_byte_offset: Some(copy_stack_byte_offset),
            byte_size,
            alignment,
        } = parameter.target.placement.locations[0]
        else {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        };
        if pointer != pointers[index]
            || copy_stack_byte_offset != offsets[index]
            || byte_size != 16
            || alignment != 8
        {
            return Err(SelectedInstructionError::UnsupportedSourceShape { function });
        }
        bindings[index] = SelectedStructuralUnitIndirectBinding {
            parameter_index: index,
            pointer,
            copy_stack_byte_offset,
            byte_count: byte_size,
            alignment,
        };
    }
    if source.parameters[0].semantic.structural_type
        != source.parameters[1].semantic.structural_type
        || source.parameters[0].semantic.multiplicity != source.parameters[1].semantic.multiplicity
        || source.parameters[0].semantic.qualifications
            != source.parameters[1].semantic.qualifications
        || source.parameters[0].semantic.place == source.parameters[1].semantic.place
        || !is_extent_structural_type(source)
    {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    Ok(SelectedMicrosoftX64OwnedIndirectPairLayout {
        shadow_byte_count: 32,
        outgoing_frame_byte_count: 72,
        pre_call_stack_alignment: 16,
        bindings,
    })
}

fn is_extent_structural_type(source: &SourceStructuralUnitFunction) -> bool {
    let structural_type = source.parameters[0].semantic.structural_type;
    let Some(declaration) = source
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return false;
    };
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return false;
    };
    if fields.len() != 2
        || fields
            .iter()
            .any(|field| field.relevance != BindingRelevance::Relevant)
    {
        return false;
    }
    matches!(
        fields[0].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Address
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    ) && matches!(
        fields[1].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    )
}

pub(in crate::selection) fn structural_call_row(
    function: usize,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<&RegisterInstructionConstraint, SelectedInstructionError> {
    let key = keys
        .structural_unit_call
        .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
    let row = row(catalog, key)?;
    if !row.operands.is_empty() {
        return Err(SelectedInstructionError::MissingConstraint(key));
    }
    Ok(row)
}

fn build_structural_unit_function(
    function: usize,
    source: &SourceStructuralUnitFunction,
    plan: &LegalizedOperationPlan,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitFunction, SelectedInstructionError> {
    if plan.target != omega_target::NativeTarget::uefi_x64() {
        return Err(SelectedInstructionError::UnsupportedSourceShape { function });
    }
    let layout = structural_unit_layout(function, source)?;
    let call = source
        .call
        .as_ref()
        .map(|call| {
            let callee = plan
                .structural_unit_functions
                .iter()
                .find(|candidate| candidate.machine == call.callee)
                .ok_or(SelectedInstructionError::UnsupportedSourceShape { function })?;
            let callee_layout = structural_unit_layout(function, callee)?;
            if callee.call_plan != source.call_plan
                || callee_layout != layout
                || call.arguments.len() != 2
                || call.arguments.iter().enumerate().any(|(index, argument)| {
                    argument.semantic.access != StructuralAccess::Owned
                        || !argument.semantic.path.is_empty()
                        || argument.target.place != argument.semantic.place
                        || argument.target.access != argument.semantic.access
                        || argument.target.path != argument.semantic.path
                        || argument.target.root_structural_type
                            != source.parameters[index].semantic.structural_type
                        || argument.target.structural_type
                            != callee.parameters[index].semantic.structural_type
                        || argument.target.source_byte_offset != 0
                        || argument.target.fixed_array_length.is_some()
                        || argument.target.element_stride.is_some()
                        || argument.target.shape != source.parameters[index].target.shape
                        || argument.target.source != source.parameters[index].target.placement
                        || argument.target.destination != callee.parameters[index].target.placement
                })
            {
                return Err(SelectedInstructionError::UnsupportedSourceShape { function });
            }
            let row = structural_call_row(function, keys, catalog)?;
            Ok(SelectedStructuralUnitCallInstruction {
                id: SelectedInstructionId(0),
                source: call.source.clone(),
                operation: call.operation,
                callee: call.callee,
                caller_call_plan: source.call_plan.clone(),
                callee_call_plan: callee.call_plan.clone(),
                arguments: call
                    .arguments
                    .iter()
                    .map(|argument| SelectedStructuralUnitCallArgument {
                        semantic: argument.semantic.clone(),
                        target: argument.target.clone(),
                    })
                    .collect(),
                claim_transfers: call.claim_transfers.clone(),
                layout,
                constraint: row.key,
                implicit_uses: row.implicit_uses.clone(),
                implicit_defs: row.implicit_defs.clone(),
                clobbers: row.clobbers.clone(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![call.operation],
                    fuel: call.fuel.clone(),
                    ..Default::default()
                },
                effect: call.effect,
                ownership: call.ownership.clone(),
            })
        })
        .transpose()?;
    let return_id = SelectedInstructionId(u32::from(call.is_some()));
    let return_instruction = instruction(
        return_id,
        SelectedInstructionKind::ReturnUnit,
        keys.return_unit,
        &[],
        SelectedInstructionProvenance {
            edges: vec![source.return_edge],
            fuel: source.return_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    Ok(SelectedStructuralUnitFunction {
        machine: source.machine,
        attachment: source.attachment,
        provenance: source.provenance.clone(),
        structural_types: source.structural_types.clone(),
        abi: SelectedStructuralUnitAbi {
            recipe: SelectedStructuralUnitAbiRecipe::MicrosoftX64OwnedIndirectPairV1,
            call_plan: source.call_plan.clone(),
            parameters: source
                .parameters
                .iter()
                .map(|parameter| SelectedStructuralUnitParameter {
                    semantic: parameter.semantic.clone(),
                    target: parameter.target.clone(),
                })
                .collect(),
            layout,
        },
        structural_places: source.structural_places.clone(),
        entry_claims: source.entry_claims.clone(),
        published_service_ceiling: source.published_service_ceiling.clone(),
        entry_block: SelectedBlockId(0),
        source_entry_block: source.entry_block,
        boundary_settlements: source.boundary_settlements.clone(),
        call,
        terminator: SelectedStructuralUnitReturn {
            instruction: return_instruction,
            psi_return_edge: source.return_edge,
            effect: source.return_effect,
            ownership: source.return_ownership.clone(),
        },
    })
}

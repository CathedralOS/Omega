//! Independent callable-entry reconstruction from validated object custody.

use super::*;

pub(super) fn reconstruct(
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<OptimizedOrdinaryCallableEntryRecord, OptimizedOrdinaryCallableEntryError> {
    let artifact = source.artifact();
    let object_stage = source.source();
    let object = object_stage.object();
    let fragment = object_stage.source().source();
    let route = fragment.source();
    let module = route.verified_input().context().module();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    if machine.attachment.is_some()
        || !machine.structural_parameters.is_empty()
        || !matches!(machine.result, TerminalMachineResult::Scalar(_))
    {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_declaration = *machine
        .result
        .scalar_ref()
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
    let signature = CallSignature {
        parameters: machine
            .parameters
            .iter()
            .map(|p| scalar_shape(p.scalar_type))
            .collect::<Result<_, _>>()?,
        result: Some(scalar_shape(result_declaration.scalar_type)?),
    };
    let calling_policy = CallingPolicy::native_for_target(artifact.target);
    let call = evaluate_call_plan(calling_policy, &signature)
        .map_err(|_| OptimizedOrdinaryCallableEntryError::AbiPlan)?;
    let selected = route.selected_plan();
    let selected_function = selected
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    let homes = route.register_homes();
    let home_function = homes
        .plan()
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    let environment = route.register_environment();
    let physical = environment.physical();
    let mut parameters = Vec::with_capacity(machine.parameters.len());
    for (index, (declaration, placement)) in
        machine.parameters.iter().zip(&call.parameters).enumerate()
    {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
        };
        if *byte_size != placement.shape.byte_size {
            return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
        }
        let virtual_register = selected_function.virtual_registers.iter().find(|vreg| matches!(vreg.origin, VirtualRegisterOrigin::EntryParameter { source_value, parameter_index } if source_value == declaration.id && parameter_index == index)).ok_or(OptimizedOrdinaryCallableEntryError::MissingParameter(declaration.id))?;
        let fixed_view = virtual_register.entry_fixed_view.ok_or(
            OptimizedOrdinaryCallableEntryError::MissingParameter(declaration.id),
        )?;
        let expected_view = environment
            .fixed_register_view(*register)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(fixed_view))?;
        let home = home_function
            .assignments
            .iter()
            .find(|home| home.virtual_register == virtual_register.id)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingHome(
                virtual_register.id,
            ))?;
        if fixed_view != expected_view
            || home.view != fixed_view
            || home.class != virtual_register.class
        {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed_view)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(fixed_view))?;
        parameters.push(OptimizedOrdinaryCallableParameter {
            ordinal: u64::try_from(index)
                .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
            value: declaration.id,
            scalar_type: declaration.scalar_type,
            shape: placement.shape,
            virtual_register: virtual_register.id,
            class: virtual_register.class,
            abi_register: *register,
            fixed_view,
            assigned_view: home.view,
            storage_units: view.units.clone(),
        });
    }
    let result_placement = call
        .result
        .as_ref()
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
    let [
        ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result_placement.locations.as_slice()
    else {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    };
    if *byte_size != result_placement.shape.byte_size {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_view = environment
        .fixed_register_view(*result_register)
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
    let result_model_view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == result_view)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(
            result_view,
        ))?;
    let exit = route.exit_contract().contract();
    let exit_function = exit
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    if exit.result_view != result_view || exit.physical_register_model != physical.identity() {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
    }
    let mut returns = Vec::new();
    for block in &machine.blocks {
        let Terminator::Return { edge, value, .. } = block.terminator else {
            continue;
        };
        let selected_return = selected_function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                SelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } if *psi_return_edge == edge => Some(instruction),
                _ => None,
            })
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let operand = selected_return
            .operands
            .first()
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let vreg = selected_function
            .virtual_registers
            .iter()
            .find(|vreg| vreg.id == operand.virtual_register)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        if origin_value(vreg.origin) != value {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        let evidence = exit_function
            .returns
            .iter()
            .find(|returned| returned.psi_return_edge == edge)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let omega_machine_code::WholeFunctionReturnValueEvidence::ScalarI64V1 {
            virtual_register,
            view,
            units,
        } = &evidence.value
        else {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        };
        if evidence.instruction != selected_return.id
            || *virtual_register != vreg.id
            || *view != result_view
            || *units != result_model_view.units
        {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        returns.push(OptimizedOrdinaryCallableReturn {
            edge,
            value,
            selected_instruction: selected_return.id,
            virtual_register: vreg.id,
            view: *view,
            storage_units: units.clone(),
        });
    }
    if returns.is_empty() || returns.len() != exit_function.returns.len() {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
    }
    let symbol = object
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == object.semantic_entry_symbol)
        .ok_or(OptimizedOrdinaryCallableEntryError::EntrySymbolMismatch)?;
    if symbol.machine != machine.id
        || symbol.linkage != RelocationFreeObjectSymbolLinkage::ObjectLocalV1
        || symbol.role != RelocationFreeObjectSymbolRole::SemanticEntryV1
    {
        return Err(OptimizedOrdinaryCallableEntryError::EntrySymbolMismatch);
    }
    if artifact.semantic_entry != module.entry
        || selected.entry != module.entry
        || object.semantic_entry != module.entry
        || exit.selected != object.selected
    {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
    }
    let mut record = OptimizedOrdinaryCallableEntryRecord {
        identity: OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"pending"),
        source_artifact: artifact.identity,
        source_manifest: source.manifest().record().identity,
        psi: artifact.psi,
        selections: artifact.selections,
        target: artifact.target,
        semantic_entry: machine.id,
        selected: exit.selected,
        register_homes: register_home_identity(homes.plan()),
        physical_register_model: physical.identity(),
        exit_contract: route.exit_contract().identity(),
        object: object.identity,
        object_container: object_stage.container().identity,
        semantic_entry_symbol: symbol.symbol,
        semantic_entry_symbol_name: symbol.name.clone(),
        semantic_entry_section_offset: symbol.section_offset,
        semantic_entry_byte_count: symbol.byte_count,
        calling_policy,
        parameters,
        result: OptimizedOrdinaryCallableResult {
            declaration: result_declaration,
            shape: result_placement.shape,
            abi_register: *result_register,
            view: result_view,
            storage_units: result_model_view.units.clone(),
        },
        returns,
        exit_policy: exit.policy,
        hardening: exit.hardening,
        entry_assumption: exit.entry_assumption,
        stack_pointer: exit.stack_pointer,
        stack_alignment: exit.stack_alignment,
        red_zone_bytes: exit.red_zone_bytes,
        disposition:
            OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
    };
    record.identity = record.recomputed_identity()?;
    Ok(record)
}

pub(super) fn manifest(
    entry: &OptimizedOrdinaryCallableEntryRecord,
) -> Result<OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryError> {
    let unavailable = OptimizedOrdinaryCallableEntryUnavailableData::Unavailable;
    let mut manifest = OptimizedOrdinaryCallableEntryManifest {
        identity: OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"pending"),
        stage:
            OptimizedOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1,
        entry: entry.identity,
        source_artifact: entry.source_artifact,
        source_manifest: entry.source_manifest,
        psi: entry.psi,
        selections: entry.selections,
        target: entry.target,
        semantic_entry: entry.semantic_entry,
        semantic_entry_symbol: entry.semantic_entry_symbol,
        exit_contract: entry.exit_contract,
        parameter_count: u64::try_from(entry.parameters.len())
            .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
        return_count: u64::try_from(entry.returns.len())
            .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
        disposition: entry.disposition,
        wrapper_bytes: unavailable,
        relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    manifest.identity = manifest.recomputed_identity();
    Ok(manifest)
}
pub(super) fn receipt(
    entry: &OptimizedOrdinaryCallableEntryRecord,
    manifest: &OptimizedOrdinaryCallableEntryManifest,
) -> OptimizedOrdinaryCallableEntryCustodyReceipt {
    OptimizedOrdinaryCallableEntryCustodyReceipt {
        source_artifact: entry.source_artifact,
        entry: entry.identity,
        manifest: manifest.identity,
    }
}
fn scalar_shape(scalar: ScalarType) -> Result<ValueShape, OptimizedOrdinaryCallableEntryError> {
    let bytes = match scalar {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            integer.bits().div_ceil(8)
        }
        _ => return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature),
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}
fn origin_value(origin: VirtualRegisterOrigin) -> ValueId {
    match origin {
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::LegalizationTemporary { source_value, .. } => source_value,
    }
}

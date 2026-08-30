use super::shared::*;

pub(super) fn lower_linux_exit_group_i32(
    function: &AbstractFunction,
    target: NativeTarget,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let Some(AbstractOperation::BoundaryCall { boundary, arguments, .. }) = function
        .operations
        .iter()
        .find(|operation| matches!(operation, AbstractOperation::BoundaryCall { arguments, .. } if !arguments.is_empty()))
    else {
        return Ok(None);
    };
    let Some(binding) = settlements.get(boundary).cloned() else {
        return Err(LoweringError::MissingBoundarySettlement(*boundary));
    };
    let omega_target_operations::BoundarySettlementRealization::Builtin(
        BoundaryRealization::LinuxExitGroupI32(realization),
    ) = binding.realization
    else {
        return Ok(None);
    };
    if target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::X86_64 | Architecture::Aarch64
        )
    {
        return Err(LoweringError::LinuxExitGroupUnsupportedTarget {
            machine: function.machine,
            target,
        });
    }
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    let expected_scalar_type = ScalarType::Integer(i32_type);
    let Some(declaration) = boundary_machines.get(boundary).copied() else {
        return Err(LoweringError::UnknownBoundarySettlement(*boundary));
    };
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: constant_operation,
            result: constant_result,
            scalar_type,
            value,
        },
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: None,
            boundary: called_boundary,
            arguments: call_arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::ReturnUnit {
            psi_edge: nominal_return_edge,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        // A Linux exit may be the nonreturning tail of a larger straight-line
        // Unit effect body (notably write_line -> exit_process). Let the Unit
        // lowering validate that composition; retain the directed error for a
        // malformed isolated exit shape.
        return if function.operations.iter().any(|operation| {
            matches!(
                operation,
                AbstractOperation::EstablishByteSequenceLiteral { .. }
            )
        }) || function
            .operations
            .iter()
            .filter(|operation| matches!(operation, AbstractOperation::BoundaryCall { .. }))
            .count()
            > 1
        {
            Ok(None)
        } else {
            Err(LoweringError::InvalidLinuxExitGroupShape(function.machine))
        };
    };
    if function.result != AbstractFunctionResult::Unit
        || !function.parameters.is_empty()
        || !function.structural_parameters.is_empty()
        || function.block_entries.len() != 1
        || function.block_entries[0].block != function.entry
        || declaration.scalar_parameters.as_slice() != [expected_scalar_type]
        || !declaration.structural_parameters.is_empty()
        || declaration.result.is_some()
        || *called_boundary != *boundary
        || arguments.as_slice() != [*constant_result]
        || call_arguments.as_slice() != [*constant_result]
        || *scalar_type != expected_scalar_type
        || !i32_type.admits(*value)
        || !structural_arguments.is_empty()
        || !cleanup_actions.is_empty()
    {
        return Err(LoweringError::InvalidLinuxExitGroupShape(function.machine));
    }
    let destination = match target.architecture {
        Architecture::X86_64 => MachineRegister::X86Rdi,
        Architecture::Aarch64 => MachineRegister::Aarch64X(0),
    };
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: TerminalPsiProvenance {
            operations: vec![*constant_operation, *psi_operation],
            edges: vec![*nominal_return_edge],
        },
        operation: TargetOperation::ExitProcessI32 {
            constant_operation: *constant_operation,
            psi_operation: *psi_operation,
            nominal_return_edge: *nominal_return_edge,
            boundary: *boundary,
            provider_execution: binding.provider_execution,
            realization,
            argument: BoundaryScalarArgument {
                source_value: *constant_result,
                scalar_type: *scalar_type,
                immediate: *value,
                destination,
            },
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: completion_receipts.clone(),
        },
    }))
}

pub(super) fn claim_completion_only_boundary_is_exact(
    function: &AbstractFunction,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    scalar_arguments: &[ValueId],
    structural_arguments: &[psi_terminal::StructuralArgument],
    completion_claim_sources: &[CompletionClaimSource],
    completion_receipts: &[psi_terminal::CompletionReceipt],
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
) -> bool {
    if !scalar_arguments.is_empty()
        || !declaration.scalar_parameters.is_empty()
        || declaration.result.is_some()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
        || structural_arguments.is_empty()
        || structural_arguments.len() != declaration.structural_parameters.len()
        || declaration.requires.iter().any(|requirement| {
            requirement.argument_index as usize >= declaration.structural_parameters.len()
        })
        || completion_receipts.is_empty()
        || completion_claim_sources
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || completion_receipts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return false;
    }

    for (index, (argument, boundary_parameter)) in structural_arguments
        .iter()
        .zip(&declaration.structural_parameters)
        .enumerate()
    {
        let Some(source) = parameters_by_place.get(&argument.place).copied() else {
            return false;
        };
        let Some(caller_parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
        else {
            return false;
        };
        let mut expected_qualifications = boundary_parameter.qualifications.clone();
        expected_qualifications.extend(
            declaration
                .requires
                .iter()
                .filter(|requirement| requirement.argument_index as usize == index)
                .map(|requirement| requirement.domain),
        );
        expected_qualifications.sort_unstable();
        expected_qualifications.dedup();
        if !argument.path.is_empty()
            || argument.access != psi_terminal::StructuralAccess::Owned
            || source.access != psi_terminal::StructuralAccess::Owned
            || boundary_parameter.access != psi_terminal::StructuralAccess::Owned
            || source.multiplicity != psi_terminal::StructuralMultiplicity::Linear
            || boundary_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Linear
            || boundary_parameter.position != index as u32
            || source.structural_type != boundary_parameter.structural_type
            || caller_parameter.qualifications != expected_qualifications
        {
            return false;
        }
    }

    let canonical_sources = function
        .entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    if completion_claim_sources != canonical_sources {
        return false;
    }

    let expected = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            completion_claim_sources.iter().filter_map(move |source| {
                (source.input() == argument.place).then_some((argument_index as u32, source.claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual = completion_receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    expected == actual && actual.len() == completion_receipts.len()
}

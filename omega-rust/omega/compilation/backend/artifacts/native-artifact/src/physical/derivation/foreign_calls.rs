//! Derive normalized foreign-call evidence from semantic custody, provider
//! bindings, and exact emitted spans and relocation fields. Fragment publication
//! rejoins its retained call custody before those checks continue.

mod covered_intervals;
mod floating_control;
mod fragment_custody;

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NativeProviderExecution;
use crate::NativeSelectedProviderPlan;
use crate::NormalizedForeignCallbackRelocations;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::physical_child_identity;
use crate::physical::derivation::foreign_calls::fragment_custody::fragment_normalized_foreign_rejoin;
use crate::physical::derivation::settlement_identity::admitted_provider_boundary_trait_settlement_identity;
use crate::physical::fragment_publication::FragmentPublicationBinding;
use crate::physical::normalized_foreign_call_import_field;
use crate::physical::normalized_foreign_call_relocation;
use crate::physical::normalized_foreign_callback_relocation;
use installation_evidence::ProviderExecutionEvidence;
use object_file::RelocationKind;
use object_file::SectionKind;
use object_file::boundary_relocation_origin;
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::ScalarType;
use target::Architecture;
use target::NativeTarget;
use target_operations::ProviderExecutionBinding;
use target_operations::ProviderPlanReportIdentity;
use terminal_psi::OperationKind;

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_normalized_foreign_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    final_image_symbol_digest: [u8; 32],
    foreign: &image_emission::ObjectForeignCall,
    publication: Option<&FragmentPublicationBinding>,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let invocation_site = occurrence.invocation_site();
    let source_operation = invocation_site.operation();
    let (boundary, arguments, structural_arguments, source_result, completion) =
        match invocation_site {
            semantic_vocabulary::BoundaryInvocationSite::Operation(operation_id) => {
                let mut operations = module
                    .machines
                    .iter()
                    .filter(|machine| machine.id == occurrence.machine())
                    .flat_map(|machine| machine.operations())
                    .filter(|operation| operation.id == operation_id);
                let (Some(operation), None) = (operations.next(), operations.next()) else {
                    return Err(
                        "normalized foreign occurrence does not rejoin one Terminal operation",
                    );
                };
                let OperationKind::BoundaryCall {
                    boundary,
                    arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } = &operation.kind
                else {
                    return Err("normalized foreign occurrence is not a Terminal boundary call");
                };
                if !completion_receipts.is_empty() {
                    return Ok(None);
                }
                (
                    boundary,
                    arguments,
                    structural_arguments,
                    Some(&operation.result),
                    semantic_vocabulary::BoundaryCompletion::NormalReturn,
                )
            }
            semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge_id) => {
                let mut completions = module
                    .machine(occurrence.machine())
                    .into_iter()
                    .flat_map(|machine| &machine.blocks)
                    .filter_map(|block| match &block.terminator {
                        terminal_psi::Terminator::ExternalCompletion {
                            edge,
                            boundary,
                            arguments,
                            structural_arguments,
                            kind,
                            ..
                        } if *edge == edge_id => {
                            Some((boundary, arguments, structural_arguments, kind))
                        }
                        _ => None,
                    });
                let (Some((boundary, arguments, structural_arguments, kind)), None) =
                    (completions.next(), completions.next())
                else {
                    return Err(
                        "normalized foreign occurrence does not rejoin one Terminal completion",
                    );
                };
                if foreign.scalar_result.is_some()
                    || !foreign.callback_addresses.is_empty()
                    || foreign.x86_floating_control.is_some()
                    || foreign.aarch64_floating_control.is_some()
                    || foreign.binding.boundary_entry_plan.call.result.is_some()
                {
                    return Err("normalized foreign completion retained returning-call custody");
                }
                (
                    boundary,
                    arguments,
                    structural_arguments,
                    None,
                    semantic_vocabulary::BoundaryCompletion::External(*kind),
                )
            }
        };
    let declaration = terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
        .ok_or("normalized foreign D41 occurrence names an absent boundary")?;
    if declaration.completion != completion {
        return Err("normalized foreign invocation changed its completion contract");
    }
    if *boundary != occurrence.boundary()
        || module.identities.text(declaration.identity) != Some(requirement_identity)
    {
        return Ok(None);
    }

    if foreign.operation_ordinal != occurrence.operation_ordinal()
        || !declaration.has_valid_parameter_order()
        || arguments.len() != declaration.scalar_parameters.len()
        || foreign.scalar_arguments.len() != arguments.len()
    {
        return Err("normalized foreign D41 child changed its scalar call occurrence");
    }
    let Some(parameter_shapes) = declaration
        .scalar_parameters
        .iter()
        .copied()
        .map(foreign_scalar_shape)
        .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    for ((argument, scalar_type), physical) in arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&foreign.scalar_arguments)
    {
        if physical.source.source_value() != *argument
            || physical.source.scalar_type() != *scalar_type
        {
            return Err("normalized foreign D41 child changed a scalar argument source");
        }
    }
    let pointer_shape = calling_conventions::ValueShape::integer(
        u32::try_from(target.pointer_size)
            .map_err(|_| "normalized foreign D41 pointer size does not fit its ABI")?,
        u16::try_from(target.pointer_alignment)
            .map_err(|_| "normalized foreign D41 pointer alignment does not fit its ABI")?,
    );
    // The normalized foreign lane admits structural custody only as the exact
    // source-rooted borrowed flat-record lane the lowering owns: each expected
    // Terminal argument row must rejoin its declared formal and the observed
    // evaluated plan must place each formal's referent pointer as one
    // pointer-width word. Any other valid signature shape retains no complete
    // physical evidence rather than claiming a custody join it cannot prove.
    let structural_parameter_shapes =
        if !structural_arguments.is_empty() || !declaration.structural_parameters.is_empty() {
            let caller = module
                .machine(occurrence.machine())
                .ok_or("normalized foreign D41 child names an absent caller machine")?;
            let Some(shapes) = normalized_foreign_structural_parameter_shapes(
                structural_arguments,
                &declaration.structural_parameters,
                &caller.structural_parameters,
                &declaration.parameter_order,
                foreign.callback_addresses.iter().any(|callback| {
                    matches!(
                        callback.target.destination,
                        calling_conventions::NativePlace::Parameter(_)
                    )
                }),
                &foreign.binding.operand_placements(),
                pointer_shape,
            )?
            else {
                return Ok(None);
            };
            Some(shapes)
        } else {
            None
        };
    // Fragment publication retains the call's custody as relocation-free
    // plan rows instead of object relocations; rejoin the unresolved import
    // field, its declared import symbol, and the selected roster row before
    // the custody checks below split on which route produced the call.
    let fragment_rejoin = publication
        .map(|publication| {
            fragment_normalized_foreign_rejoin(
                publication,
                occurrence,
                foreign,
                structural_arguments,
            )
        })
        .transpose()?;
    let result_shape = match (source_result, &declaration.result, &foreign.scalar_result) {
        (
            Some(terminal_psi::OperationResult::Unit),
            terminal_psi::BoundaryMachineResult::Unit,
            None,
        )
        | (None, terminal_psi::BoundaryMachineResult::Unit, None) => None,
        (
            Some(terminal_psi::OperationResult::Scalar(value)),
            terminal_psi::BoundaryMachineResult::Scalar(declared),
            Some(physical),
        ) if value.scalar_type == *declared
            && physical.home.source_value == value.id
            && physical.home.scalar_type == *declared =>
        {
            let Some(shape) = foreign_scalar_shape(*declared) else {
                return Ok(None);
            };
            Some(shape)
        }
        (
            Some(terminal_psi::OperationResult::Scalar(value)),
            terminal_psi::BoundaryMachineResult::Scalar(declared),
            None,
        ) => {
            // Fragment custody rows keep `scalar_result` empty; the retained
            // roster row's result-home requirement carries the same custody
            // the object route proves through its emitted durable-home record.
            let Some(home) =
                fragment_rejoin.and_then(|rejoin| rejoin.record.call.result.scalar_home())
            else {
                return Err("normalized foreign D41 child changed its scalar result custody");
            };
            if value.scalar_type != *declared
                || Some(home.defining_operation) != source_operation
                || home.source_value != value.id
                || home.scalar_type != *declared
            {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            let Some(shape) = foreign_scalar_shape(*declared) else {
                return Ok(None);
            };
            if home.shape != shape {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            Some(shape)
        }
        (
            Some(terminal_psi::OperationResult::Structural(result)),
            terminal_psi::BoundaryMachineResult::Structural(declared),
            None,
        ) => {
            // The admitted fragment retains the complete typed home and independently
            // replayed capture. It is not represented by the legacy scalar record.
            let Some(rejoin) = fragment_rejoin else {
                return Ok(None);
            };
            let target_operations::TargetBoundaryResult::Structural(home) =
                &rejoin.record.call.result
            else {
                return Err("normalized foreign D41 child changed its structural result custody");
            };
            let target_operations::TargetStructuralHomeLayout::Aggregate(shape) = home.layout
            else {
                return Err("normalized foreign D41 child changed its structural result layout");
            };
            // Fragment admission independently classifies the complete public
            // declaration under the retained calling policy. Rejoin those exact
            // declarations below, retaining that admitted ABI class: activation
            // storage carries only ordinary geometry, not SysV/HFA classification.
            let placement = foreign
                .binding
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .ok_or("normalized foreign D41 child lost its structural result placement")?;
            let retained_types = publication
                .and_then(|binding| binding.selected_plan().function(occurrence.machine()))
                .and_then(|function| function.structural.as_ref())
                .ok_or("normalized foreign D41 child lost its structural declarations")?;
            if home.operation_result() != source_operation.map(|operation| (operation, result))
                || result.structural_type != declared.structural_type
                || result.multiplicity != declared.multiplicity
                || result.qualifications != declared.qualifications
                || result.projected_qualifications != declared.projected_qualifications
                || !retained_types
                    .structural_types
                    .iter()
                    .any(|item| item.id == result.structural_type)
                || retained_types
                    .structural_types
                    .iter()
                    .any(|item| !module.structural_types.iter().any(|source| source == item))
                || placement.shape.byte_size != shape.byte_size
                || placement.shape.alignment != shape.alignment
            {
                return Err("normalized foreign D41 child changed its structural result custody");
            }
            Some(placement.shape)
        }
        _ => return Err("normalized foreign D41 child changed its result custody"),
    };
    if let Some(rejoin) = fragment_rejoin
        && matches!(
            rejoin.record.call.result,
            target_operations::TargetBoundaryResult::Unit
        ) != result_shape.is_none()
    {
        return Err("normalized foreign D41 child changed its scalar result custody");
    }
    let callbacks: &[machine_code::CallbackAddressMaterialization] = match (
        foreign.callback_addresses.as_slice(),
        foreign
            .binding
            .boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) {
        (callbacks, materials) if callbacks.len() == materials.len() => callbacks,
        // Fragment custody projects the entry plan wholesale but cannot
        // carry callback materialization custody rows it never derived; the
        // call retains no complete physical evidence rather than claiming a
        // custody join it cannot prove.
        ([], materials) if fragment_rejoin.is_some() && !materials.is_empty() => {
            return Ok(None);
        }
        _ => return Err("normalized foreign D41 child changed its callback-plan custody"),
    };
    let mut callback_ordinals = Vec::with_capacity(callbacks.len());
    for callback in callbacks {
        // Field destinations occupy their root parameter's existing slot;
        // only Parameter destinations add a native parameter position.
        if !matches!(
            callback.target.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) {
            continue;
        }
        let ordinal = usize::try_from(callback.target.application.native_ordinal)
            .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
        if ordinal >= declaration.parameter_order.len() || callback_ordinals.contains(&ordinal) {
            return Err("normalized foreign D41 callback ordinal is outside its native signature");
        }
        callback_ordinals.push(ordinal);
    }
    callback_ordinals.sort_unstable();
    for (physical, semantic_position) in foreign
        .scalar_arguments
        .iter()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
    {
        let native_position = semantic_position
            + callback_ordinals
                .iter()
                .filter(|ordinal| {
                    **ordinal + foreign.binding.dispatch_operand_count() <= semantic_position
                })
                .count();
        if usize::try_from(physical.parameter_index).ok() != Some(native_position)
            || foreign.binding.operand_placements().get(native_position)
                != Some(&physical.placement)
        {
            return Err("normalized foreign D41 child changed a scalar argument position");
        }
    }
    // Terminal retains authored order independently of the physical proposal.
    // Reconstruct that interleave instead of letting either lane claim the
    // other lane's ABI positions, even when their word shapes happen to match.
    let mut scalar_shapes = parameter_shapes.into_iter();
    let mut structural_shapes = structural_parameter_shapes.into_iter().flatten();
    let mut native_parameter_shapes = declaration
        .parameter_order
        .iter()
        .map(|kind| {
            match kind {
                terminal_psi::BoundaryParameterKind::Scalar => scalar_shapes.next(),
                terminal_psi::BoundaryParameterKind::Structural => structural_shapes.next(),
            }
            .ok_or("normalized foreign D41 child changed its parameter order")
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (callback, ordinal) in callbacks.iter().zip(callback_ordinals.iter()) {
        native_parameter_shapes.insert(
            *ordinal + foreign.binding.dispatch_operand_count(),
            callback.target.application.shape,
        );
    }
    let signature = calling_conventions::CallSignature {
        parameters: native_parameter_shapes
            .into_iter()
            .skip(foreign.binding.dispatch_operand_count())
            .collect(),
        result: result_shape,
    };

    let mut ordinal_cursor = callback_ordinals.iter().copied();
    for callback in callbacks {
        let expected_placement = match callback.destination {
            machine_code::CallbackAddressDestination::Register(register) => {
                calling_conventions::ValuePlacement {
                    shape: callback.target.application.shape,
                    locations: vec![calling_conventions::ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: callback.target.application.shape.byte_size,
                    }],
                }
            }
            machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                calling_conventions::ValuePlacement {
                    shape: callback.target.application.shape,
                    locations: vec![calling_conventions::ValueLocation::Stack {
                        stack_byte_offset: byte_offset,
                        value_byte_offset: 0,
                        byte_size: callback.target.application.shape.byte_size,
                        alignment: callback.target.application.shape.alignment,
                    }],
                }
            }
            machine_code::CallbackAddressDestination::RecordField {
                root_register,
                byte_offset,
            } => {
                if callback.target.field_byte_offset
                    != Some(usize::try_from(byte_offset).unwrap_or(usize::MAX))
                {
                    return Err("normalized foreign D41 child changed its callback target custody");
                }
                calling_conventions::ValuePlacement {
                    shape: callback.target.application.shape,
                    locations: vec![calling_conventions::ValueLocation::Register {
                        register: root_register,
                        value_byte_offset: 0,
                        byte_size: callback.target.application.shape.byte_size,
                    }],
                }
            }
        };
        // Parameter-destination callbacks consume the next recorded native
        // ordinal; field destinations bind through their root parameter's
        // slot and take no ordinal of their own.
        let callback_ordinal = usize::try_from(callback.target.application.native_ordinal)
            .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
        if matches!(
            callback.target.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) && ordinal_cursor.next() != Some(callback_ordinal)
        {
            return Err("normalized foreign D41 child changed its callback target custody");
        }
        if Some(callback.target.terminal_operation) != source_operation
            || callback.target.registrar_boundary_entry_plan != foreign.binding.boundary_entry_plan
            || callback
                .target
                .callback_function
                .callback_thunk_placement_index()
                != Some(callback.target.placement_index)
            || callback.target.registrar_application_commitment == [0; 32]
            || callback.target.application.shape != pointer_shape
            || callback.target.application.placement != expected_placement
            || foreign
                .binding
                .boundary_entry_plan
                .call
                .parameters
                .get(callback_ordinal)
                != Some(&callback.target.application.placement)
            || foreign
                .binding
                .boundary_entry_plan
                .call
                .callback_materializations
                .iter()
                .filter(|row| row.destination == callback.target.destination)
                .count()
                != 1
        {
            return Err("normalized foreign D41 child changed its callback target custody");
        }
    }
    if ordinal_cursor.next().is_some() {
        return Err("normalized foreign D41 child changed its callback target custody");
    }
    let validated_plan = if let Some(callback) = callbacks.first() {
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            foreign.binding.boundary_entry_plan.clone(),
            &signature,
            &callback.target.registrar_context,
        )
        .map_err(
            |_| "normalized foreign D41 child contains an invalid callback boundary entry plan",
        )?
    } else {
        calling_conventions::validate_boundary_entry_plan(
            foreign.binding.boundary_entry_plan.clone(),
            &signature,
        )
        .map_err(|_| "normalized foreign D41 child contains an invalid boundary entry plan")?
    };
    let boundary_plan_identity = validated_plan.contract_commitment_digest();
    // Physical provider evidence names the selected schema requirement; the
    // original occurrence above still names and validates the portable head.
    let selected_requirement = selected_plan.selected_requirement_identity(requirement_identity);
    if validated_plan.plan() != &foreign.binding.boundary_entry_plan
        || foreign.binding.native_target() != Some(target)
        || !foreign.binding.has_valid_entry_mechanism()
        || foreign
            .binding
            .requirement_identity(selected_plan.family_realization_identities())
            != Some(selected_requirement)
        || foreign.binding.provider_plan_report_identity() != selected_plan.report_identity()
    {
        return Err("normalized external child changed its binding or boundary plan");
    }
    let commitment = match &foreign.binding.mechanism {
        target_operations::NormalizedForeignCallMechanism::Import {
            same_stack_contribution,
            ..
        }
        | target_operations::NormalizedForeignCallMechanism::Table {
            same_stack_contribution,
            ..
        } => same_stack_contribution
            .provider_plan_commitment()
            .as_bytes(),
        target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) => {
            kernel.provider_plan_commitment
        }
    };
    if commitment != *selected_plan.plan_digest().as_bytes() {
        return Err("normalized external child changed its selected provider commitment");
    }

    let execution_record = foreign.provider_execution;
    if execution_record.provider_plan_report_identity != selected_plan.report_identity() {
        return Err("normalized foreign D41 child names the wrong selected provider plan");
    }
    let matching_executions = provider_executions
        .iter()
        .filter(|execution| {
            execution.requirement_identity() == selected_requirement
                && execution.provider_plan_report_identity()
                    == execution_record.provider_plan_report_identity
                && execution.provider_execution_report_identity()
                    == execution_record.provider_execution_report_identity
                && execution.provider_execution_report_fingerprint()
                    == execution_record.provider_execution_report_fingerprint
                && execution.normalized_root_report_identity()
                    == execution_record.normalized_root_report_identity
                && execution.boundary_contract_report_fingerprint()
                    == execution_record.boundary_contract_report_fingerprint
        })
        .count();
    if matching_executions != 1 {
        return Err("normalized foreign D41 child cannot rejoin one retained provider execution");
    }
    let plan_report_identity =
        ProviderPlanReportIdentity::new(execution_record.provider_plan_report_identity)
            .ok_or("normalized foreign D41 child has a zero provider-plan report identity")?;
    let execution = ProviderExecutionBinding::from_execution_record(
        plan_report_identity,
        execution_record.provider_execution_report_identity,
        execution_record.provider_execution_report_fingerprint,
        execution_record.normalized_root_report_identity,
        execution_record.boundary_contract_report_fingerprint,
    )
    .ok_or("normalized foreign D41 child has an invalid provider execution")?;

    // Fragment publication independently projects the same object roster;
    // the image carries that complete roster without appending extra calls.
    let retained_calls: &[image_emission::ObjectForeignCall] = match publication {
        Some(binding) => binding.foreign_call_custody(),
        None => object.foreign_calls(),
    };
    let matching_image_calls = retained_calls
        .iter()
        .filter(|candidate| {
            candidate.machine == foreign.machine && candidate.owner == foreign.owner
        })
        .collect::<Vec<_>>();
    let [image_call] = matching_image_calls.as_slice() else {
        return Err("normalized foreign D41 child does not rejoin one final-image call");
    };
    if *image_call != foreign {
        return Err("normalized foreign D41 call custody changed before final-image emission");
    }

    let function = object
        .function_on_machine(occurrence.machine())
        .ok_or("normalized foreign D41 child names an absent object function")?;
    let matching_attributions = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == occurrence.machine()
                && attribution.attribution.site == machine_code::semantic_code_site(invocation_site)
                && attribution.attribution.operation_ordinal == occurrence.operation_ordinal()
        })
        .collect::<Vec<_>>();
    let intervals =
        covered_intervals::CoveredInvocationIntervals::derive(function, &matching_attributions)?;
    intervals.require_invocation(
        fragment_rejoin.map(|rejoin| rejoin.call_span),
        target.architecture,
        foreign,
        object.text_bytes(),
    )?;
    let expected_kind = match target.architecture {
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
    };
    // Fragment publication binds the same call custody through the
    // relocation-free object plan — the unresolved import field plus its
    // declared import symbol — where the object route binds it through
    // relocation records and an object-plan import row.
    let (mutable_intervals, relocation) = if foreign.binding.import().is_none() {
        if fragment_rejoin.is_none_or(|rejoin| rejoin.import.is_some()) {
            return Err("kernel entry lacks independently replayed fragment custody");
        }
        if object.relocations().records().any(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && intervals.overlaps_object_span(relocation.offset, relocation.byte_width)
        }) {
            return Err("kernel entry contains an unexpected relocation");
        }
        (
            Vec::new(),
            PhysicalRelocationDisposition::DirectInstructionBytes,
        )
    } else {
        let (locator, _) = foreign
            .binding
            .import()
            .ok_or("normalized external call has no entry mechanism")?;
        if let Some((field_record, import)) = fragment_rejoin.and_then(|rejoin| rejoin.import) {
            let plan = publication
                .ok_or("fragment rejoin requires the publication binding")?
                .relocation_free_object();
            let field = &field_record.resolution;
            let field_offset = usize::try_from(field.field_section_offset).map_err(
                |_| "normalized foreign D41 import field offset does not fit this target",
            )?;
            let field_byte_width = usize::from(field.field_byte_width);
            let field_kind = match field.kind {
            machine_code::NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1 => {
                RelocationKind::X86_64Relative32
            }
            machine_code::NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1 => {
                RelocationKind::Aarch64Branch26
            }
        };
            if field_offset != foreign.text_offset
                || field_byte_width != 4
                || field.addend != 0
                || field_kind != expected_kind
            {
                return Err(
                    "normalized foreign D41 child changed import owner, symbol, addend, kind, or span",
                );
            }
            let overlapping_fields = plan
                .unresolved_normalized_foreign_calls
                .iter()
                .filter(|candidate| {
                    usize::try_from(candidate.resolution.field_section_offset).is_ok_and(|offset| {
                        intervals.overlaps_object_span(
                            offset,
                            usize::from(candidate.resolution.field_byte_width),
                        )
                    })
                })
                .collect::<Vec<_>>();
            let [sole_field] = overlapping_fields.as_slice() else {
                return Err(
                    "normalized foreign invocation interval does not contain exactly one import field",
                );
            };
            if !std::ptr::eq(*sole_field, field_record)
                || !intervals.contains_object_span(field_offset, field_byte_width)
            {
                return Err(
                    "normalized foreign selected import field lies outside its invocation interval",
                );
            }
            let field_end = field_offset
                .checked_add(field_byte_width)
                .ok_or("normalized foreign import field end overflow")?;
            if field.callback_materializations.is_empty() {
                (
                    vec![(field_offset, field_end)],
                    PhysicalRelocationDisposition::UnresolvedNormalizedForeignCallImportField(
                        normalized_foreign_call_import_field(
                            locator.identity_digest().as_bytes(),
                            boundary_plan_identity,
                            occurrence.machine(),
                            invocation_site,
                            field.boundary,
                            field.ordinal,
                            import.symbol,
                            foreign.text_offset,
                            field_byte_width,
                            field.addend,
                            field_kind,
                            final_image_symbol_digest,
                        ),
                    ),
                )
            } else {
                let expected_origin = boundary_relocation_origin(function.symbol, invocation_site);
                let import_records = object
                    .relocations()
                    .records()
                    .map(|(_, relocation)| relocation)
                    .filter(|relocation| {
                        relocation.origin == expected_origin
                            && relocation.offset == field_offset
                            && relocation.byte_width == field_byte_width
                            && relocation.addend == field.addend
                            && relocation.kind == field_kind
                            && object
                                .object()
                                .layout
                                .normalized_imports
                                .iter()
                                .any(|import| {
                                    import.locator == *locator
                                        && import.symbol == relocation.symbol_handle
                                })
                    })
                    .collect::<Vec<_>>();
                let [import_relocation] = import_records.as_slice() else {
                    return Err(
                        "normalized foreign D41 child changed import owner, symbol, addend, kind, or span",
                    );
                };
                if foreign.callback_addresses.len() != field.callback_materializations.len() {
                    return Err(
                        "normalized foreign callback custody differs from its materialization inventory",
                    );
                }
                let mut callback_relocations =
                    Vec::with_capacity(field.callback_materializations.len());
                for (custody, placed) in foreign
                    .callback_addresses
                    .iter()
                    .zip(field.callback_materializations.iter())
                {
                    let (callback_symbol, _) = object_file::object_function_symbol(
                        object.object(),
                        custody.target.callback_function,
                    )
                    .ok_or("normalized foreign D41 callback lost its private object symbol")?;
                    let exact_callback_relocation =
                        |section_offset: u64, kind: RelocationKind| -> Result<_, &'static str> {
                            let offset = usize::try_from(section_offset).map_err(
                        |_| "normalized foreign D41 callback field offset does not fit this target",
                    )?;
                            let matching = object
                                .relocations()
                                .records()
                                .map(|(_, relocation)| relocation.clone())
                                .filter(|candidate| {
                                    candidate.origin == expected_origin
                                        && candidate.offset == offset
                                        && candidate.byte_width == 4
                                        && candidate.symbol_handle == callback_symbol
                                        && candidate.addend == 0
                                        && candidate.kind == kind
                                })
                                .collect::<Vec<_>>();
                            let [matching] = matching.as_slice() else {
                                return Err(
                                    "normalized foreign D41 callback changed a private-function relocation",
                                );
                            };
                            Ok(normalized_foreign_callback_relocation(
                                matching.symbol_handle,
                                matching.origin,
                                matching.offset,
                                matching.byte_width,
                                matching.addend,
                                matching.kind,
                            ))
                        };
                    callback_relocations.push(match placed.encoding {
                        machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                            relocation_section_offset,
                            ..
                        } => NormalizedForeignCallbackRelocations::X86_64Relative32 {
                            callback_function: custody.target.callback_function,
                            relocation: exact_callback_relocation(
                                relocation_section_offset,
                                RelocationKind::X86_64Relative32,
                            )?,
                        },
                        machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_section_offset,
                            page_offset_relocation_section_offset,
                            ..
                        } => NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                            callback_function: custody.target.callback_function,
                            page: exact_callback_relocation(
                                page_relocation_section_offset,
                                RelocationKind::Aarch64Page21,
                            )?,
                            page_offset: exact_callback_relocation(
                                page_offset_relocation_section_offset,
                                RelocationKind::Aarch64PageOffset12,
                            )?,
                        },
                    });
                }
                // Callback materialization patch fields are unresolved
                // private-function fields the final image binds — each 4-byte field
                // inside the operation interval is mutable alongside the import
                // field.
                let mut mutable_intervals = vec![(field_offset, field_end)];
                for callback in &field.callback_materializations {
                    let patch_section_offsets: [u64; 2] = match callback.encoding {
                        machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                            relocation_section_offset,
                            ..
                        } => [relocation_section_offset, u64::MAX],
                        machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_section_offset,
                            page_offset_relocation_section_offset,
                            ..
                        } => [
                            page_relocation_section_offset,
                            page_offset_relocation_section_offset,
                        ],
                    };
                    for patch_section_offset in patch_section_offsets {
                        if patch_section_offset == u64::MAX {
                            continue;
                        }
                        let patch_start = usize::try_from(patch_section_offset).map_err(
                        |_| "normalized foreign D41 callback field offset does not fit this target",
                    )?;
                        if !intervals.contains_object_span(patch_start, 4) {
                            return Err(
                                "normalized foreign callback relocation crosses its invocation interval",
                            );
                        }
                        mutable_intervals.push((
                            patch_start,
                            patch_start
                                .checked_add(4)
                                .ok_or("callback relocation end overflow")?,
                        ));
                    }
                }
                (
                    mutable_intervals,
                    PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
                        normalized_foreign_call_relocation(
                            locator.identity_digest().as_bytes(),
                            boundary_plan_identity,
                            import_relocation.symbol_handle,
                            import_relocation.origin,
                            import_relocation.offset,
                            import_relocation.byte_width,
                            import_relocation.addend,
                            import_relocation.kind,
                            callback_relocations,
                            final_image_symbol_digest,
                        ),
                    ),
                )
            }
        } else {
            let matching_imports = object
                .object()
                .layout
                .normalized_imports
                .iter()
                .filter(|import| import.locator == *locator)
                .collect::<Vec<_>>();
            let [import] = matching_imports.as_slice() else {
                return Err("normalized foreign D41 child does not rejoin one object import");
            };
            let overlapping_relocations = object
                .relocations()
                .records()
                .map(|(_, relocation)| relocation)
                .filter(|relocation| {
                    relocation.section == SectionKind::Text
                        && intervals.overlaps_object_span(relocation.offset, relocation.byte_width)
                })
                .collect::<Vec<_>>();
            let expected_origin = boundary_relocation_origin(function.symbol, invocation_site);
            let matching_import_relocations = overlapping_relocations
                .iter()
                .copied()
                .filter(|relocation| {
                    relocation.origin == expected_origin
                        && relocation.offset == foreign.text_offset
                        && relocation.byte_width == 4
                        && relocation.symbol_handle == import.symbol
                        && relocation.addend == 0
                        && relocation.kind == expected_kind
                })
                .collect::<Vec<_>>();
            let [relocation] = matching_import_relocations.as_slice() else {
                return Err(
                    "normalized foreign D41 child changed import owner, symbol, addend, kind, or span",
                );
            };
            let mut callback_relocations = Vec::with_capacity(callbacks.len());
            for callback in callbacks {
                {
                    let (callback_symbol, _) = object_file::object_function_symbol(
                        object.object(),
                        callback.target.callback_function,
                    )
                    .ok_or("normalized foreign D41 callback lost its private object symbol")?;
                    if !intervals.contains_object_span(callback.code_offset, callback.byte_count) {
                        return Err(
                            "normalized foreign D41 callback materialization left its operation interval",
                        );
                    }
                    let exact_callback_relocation =
                        |offset: usize, kind: RelocationKind| -> Result<_, &'static str> {
                            let matching = overlapping_relocations
                                .iter()
                                .copied()
                                .filter(|candidate| {
                                    candidate.origin == expected_origin
                                        && candidate.offset == offset
                                        && candidate.byte_width == 4
                                        && candidate.symbol_handle == callback_symbol
                                        && candidate.addend == 0
                                        && candidate.kind == kind
                                })
                                .collect::<Vec<_>>();
                            let [matching] = matching.as_slice() else {
                                return Err(
                                    "normalized foreign D41 callback changed a private-function relocation",
                                );
                            };
                            Ok(normalized_foreign_callback_relocation(
                                matching.symbol_handle,
                                matching.origin,
                                matching.offset,
                                matching.byte_width,
                                matching.addend,
                                matching.kind,
                            ))
                        };
                    callback_relocations.push(match callback.encoding {
                        machine_code::CallbackAddressEncoding::X86_64Relative32 {
                            relocation_offset,
                        } => NormalizedForeignCallbackRelocations::X86_64Relative32 {
                            callback_function: callback.target.callback_function,
                            relocation: exact_callback_relocation(
                                relocation_offset,
                                RelocationKind::X86_64Relative32,
                            )?,
                        },
                        machine_code::CallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_offset,
                            page_offset_relocation_offset,
                        } => NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                            callback_function: callback.target.callback_function,
                            page: exact_callback_relocation(
                                page_relocation_offset,
                                RelocationKind::Aarch64Page21,
                            )?,
                            page_offset: exact_callback_relocation(
                                page_offset_relocation_offset,
                                RelocationKind::Aarch64PageOffset12,
                            )?,
                        },
                    });
                }
            }
            let expected_relocation_count = 1 + callback_relocations
                .iter()
                .map(|relocations| match relocations {
                    NormalizedForeignCallbackRelocations::X86_64Relative32 { .. } => 1,
                    NormalizedForeignCallbackRelocations::Aarch64PageAddress { .. } => 2,
                })
                .sum::<usize>();
            if overlapping_relocations.len() != expected_relocation_count
                || overlapping_relocations.iter().any(|relocation| {
                    !intervals.contains_object_span(relocation.offset, relocation.byte_width)
                })
            {
                return Err(
                    "normalized foreign invocation relocation inventory has extra or out-of-span fields",
                );
            }
            let mutable_intervals = overlapping_relocations
                .iter()
                .map(|relocation| {
                    let end = relocation
                        .offset
                        .checked_add(relocation.byte_width)
                        .ok_or("normalized foreign relocation end overflow")?;
                    Ok((relocation.offset, end))
                })
                .collect::<Result<Vec<_>, &'static str>>()?;
            (
                mutable_intervals,
                PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
                    normalized_foreign_call_relocation(
                        locator.identity_digest().as_bytes(),
                        boundary_plan_identity,
                        import.symbol,
                        relocation.origin,
                        relocation.offset,
                        relocation.byte_width,
                        relocation.addend,
                        relocation.kind,
                        callback_relocations,
                        final_image_symbol_digest,
                    ),
                ),
            )
        }
    };

    let byte_digests = intervals.replay_bytes(
        function.bytes(object),
        object.text_bytes(),
        &image.final_text_bytes,
        &mutable_intervals,
    )?;

    let realization = foreign.binding.clone();
    let role = BoundaryTraitSettlementRole::AdmittedProvider {
        execution,
        realization,
    };
    let parent_identity = admitted_provider_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan.plan_digest(),
        target,
        execution,
        boundary_plan_identity,
        &foreign.binding.mechanism,
        structural_arguments,
        &declaration.structural_parameters,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest: selected_plan.plan_digest(),
            target,
            role,
            identity: parent_identity,
        }
        .into(),
    );
    let machine_bytes_digest = byte_digests.machine;
    let object_bytes_digest = byte_digests.object;
    let final_image_bytes_digest = byte_digests.final_image;
    let physical_occurrence = NativePhysicalOccurrence::Boundary(occurrence.identity());
    let identity = physical_child_identity(
        &parent,
        projection,
        physical_occurrence,
        &intervals.machine,
        &intervals.object,
        &intervals.final_image,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        &relocation,
    );
    Ok(Some(
        NativePhysicalChildParts {
            parent,
            projection,
            occurrence: physical_occurrence,
            machine_spans: intervals.machine,
            object_spans: intervals.object,
            final_image_spans: intervals.final_image,
            machine_bytes_digest,
            object_bytes_digest,
            final_image_bytes_digest,
            relocation,
            identity,
        }
        .into(),
    ))
}

/// Classify the retained semantic type through the same closed ABI vocabulary
/// as object custody. The hosted address carrier has an eight-byte integer
/// slot without becoming a fixed integer type; reconstructing only fixed
/// carriers rejects valid address results and their later foreign consumers.
/// Exact source types, placements and producer spans still rejoin separately.
fn foreign_scalar_shape(scalar_type: ScalarType) -> Option<calling_conventions::ValueShape> {
    calling_conventions::ValueShape::for_scalar(scalar_type)
}

/// Rejoin the expected structural-argument custody of one normalized foreign
/// Terminal boundary call against the observed evaluated-plan parameter
/// placements.
///
/// Returns `Ok(Some(_))` — one pointer-width signature shape per declared
/// structural formal — only for the exact source-rooted borrowed flat-record
/// lane the lowering owns: every argument carries a nonempty field-only path
/// rooted at a caller structural parameter, matches its declared formal's
/// position and access, that formal is borrowed, unrestricted, and
/// unqualified, and the observed plan places each formal's referent pointer
/// as exactly one pointer-width word. `Ok(None)` names a valid signature
/// shape this lane cannot prove — a callback-bearing signature, an owned or
/// qualified formal, or an argument
/// that is not such a projection — so the artifact retains no complete
/// physical evidence rather than claiming an unprovable custody join.
/// `Err` names contradictory retained custody: an argument count, declared
/// formal position, or access that changed between declaration and call, or
/// an observed plan whose parameter rows no longer realize each formal's
/// pointer word.
#[allow(clippy::too_many_arguments)]
pub(crate) fn normalized_foreign_structural_parameter_shapes(
    structural_arguments: &[terminal_psi::StructuralArgument],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    caller_structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
    parameter_order: &[terminal_psi::BoundaryParameterKind],
    callback_present: bool,
    plan_parameters: &target_operations::ForeignCallOperands<'_>,
    pointer_shape: calling_conventions::ValueShape,
) -> Result<Option<Vec<calling_conventions::ValueShape>>, &'static str> {
    if structural_arguments.len() != structural_parameters.len() {
        return Err("normalized foreign D41 child changed its structural call occurrence");
    }
    if structural_arguments.is_empty() {
        return Ok(Some(Vec::new()));
    }
    // Callback address custody is a separate native-only slot, not a
    // structural formal. That combination is not yet covered by this lane.
    if callback_present {
        return Ok(None);
    }
    let structural_positions = parameter_order
        .iter()
        .enumerate()
        .filter_map(|(position, kind)| {
            (*kind == terminal_psi::BoundaryParameterKind::Structural).then_some(position)
        });
    if plan_parameters.len() != parameter_order.len()
        || structural_positions.clone().count() != structural_arguments.len()
    {
        return Err("normalized foreign D41 child changed its structural call custody");
    }
    let mut shapes = Vec::with_capacity(structural_arguments.len());
    for (lane_position, ((argument, parameter), native_position)) in structural_arguments
        .iter()
        .zip(structural_parameters)
        .zip(structural_positions)
        .enumerate()
    {
        let placement = plan_parameters
            .get(native_position)
            .ok_or("normalized foreign D41 child lost a source transport operand")?;
        if usize::try_from(parameter.position).ok() != Some(lane_position)
            || argument.access != parameter.access
        {
            return Err("normalized foreign D41 child changed its structural call occurrence");
        }
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || argument.path.is_empty()
            || argument
                .path
                .iter()
                .any(|segment| !matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
            || !caller_structural_parameters
                .iter()
                .any(|source| source.place == argument.place)
        {
            return Ok(None);
        }
        let placed_pointer_word = placement
            .single_word_byte_size()
            .ok_or("normalized foreign D41 child changed its structural call custody")?;
        if placement.shape != pointer_shape || placed_pointer_word != pointer_shape.byte_size {
            return Err("normalized foreign D41 child changed its structural call custody");
        }
        shapes.push(pointer_shape);
    }
    Ok(Some(shapes))
}

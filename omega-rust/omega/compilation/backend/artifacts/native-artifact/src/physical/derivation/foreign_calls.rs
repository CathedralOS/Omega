/*
We derive normalized foreign-call physical evidence (`derive_normalized_foreign_child`)
from semantic boundary custody, selected provider bindings, covered invocation
intervals (`CoveredInvocationIntervals`), floating-point control-state envelopes
(`floating_control`), and emitted relocation or relocation-free import fields.

The normalized foreign lane admits structural argument custody
(`normalized_foreign_structural_parameter_shapes`) only for the exact source-rooted
borrowed flat-record lane that lowering owns, and rejoins fragment-published calls
against their relocation-free plan rows before checking import relocations,
callback address materializations, or direct kernel instruction bytes.
*/

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::NativeByteSpan;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NativeProviderExecution;
use crate::NativeSelectedProviderPlan;
use crate::NormalizedForeignCallbackRelocation;
use crate::NormalizedForeignCallbackRelocations;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::{physical_child_identity, ranges_overlap, span};
use crate::physical::derivation::fragment_custody::fragment_normalized_foreign_rejoin;
use crate::physical::derivation::settlement_identity::admitted_provider_boundary_trait_settlement_identity;
use crate::physical::fragment_publication::FragmentPublicationBinding;
use crate::physical::native_byte_span;
use crate::physical::normalized_foreign_call_import_field;
use crate::physical::normalized_foreign_call_relocation;
use crate::physical::normalized_foreign_callback_relocation;
use calling_conventions::NativePlace;
use calling_conventions::ValuePlacement;
use calling_conventions::validate_boundary_entry_plan;
use image_emission::{ObjectCodeAttribution, ObjectForeignCall, ObjectFunction};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::CallbackAddressDestination;
use object_file::RelocationKind;
use object_file::SectionKind;
use object_file::invocation_relocation_origin;
use optimization_core::NativeOptimizationProjectionIdentity;
use semantic_vocabulary::ScalarType;
use sha2::{Digest, Sha256};
use target::Architecture;
use target::NativeTarget;
use target_operations::NormalizedForeignCallMechanism;
use target_operations::ProviderExecutionBinding;
use target_operations::ProviderPlanReportIdentity;
use target_operations::TargetBoundaryResult;
use terminal_psi::BoundaryMachineResult;
use terminal_psi::OperationKind;
use terminal_psi::OperationResult;

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_normalized_foreign_child(
    pass: crate::physical::PhysicalEvidencePass,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    linked_object: &image_emission::ObjectArtifact,
    image: &[u8],
    final_image_symbol_digest: [u8; 32],
    foreign: &image_emission::ObjectForeignCall,
    publication: Option<&FragmentPublicationBinding>,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let invocation_site = occurrence.invocation_site();
    let source_operation = invocation_site.operation();
    let (boundary, arguments, structural_arguments, source_result, completion) =
        match invocation_site {
            semantic_vocabulary::InvocationSite::InvokeNoNormal(_) => {
                return Err("ordinary no-normal invocation cannot acquire foreign-call authority");
            }
            semantic_vocabulary::InvocationSite::Operation(operation_id) => {
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
            semantic_vocabulary::InvocationSite::ExternalCompletion(edge_id) => {
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
                    matches!(callback.target.destination, NativePlace::Parameter(_))
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
    let fragment_rejoin = publication
        .map(|publication| {
            fragment_normalized_foreign_rejoin(
                pass,
                publication,
                occurrence,
                foreign,
                structural_arguments,
            )
        })
        .transpose()?;
    let result_shape = match (source_result, &declaration.result, &foreign.scalar_result) {
        (Some(OperationResult::Unit), BoundaryMachineResult::Unit, None)
        | (None, BoundaryMachineResult::Unit, None) => None,
        (
            Some(OperationResult::Scalar(value)),
            BoundaryMachineResult::Scalar(declared),
            Some(physical),
        ) if value.value_type() == *declared
            && physical.home.source_value == value.id
            && physical.home.scalar_type == declared.scalar_type =>
        {
            let Some(shape) = foreign_scalar_shape(declared.scalar_type) else {
                return Ok(None);
            };
            Some(shape)
        }
        (Some(OperationResult::Scalar(value)), BoundaryMachineResult::Scalar(declared), None) => {
            let Some(home) =
                fragment_rejoin.and_then(|rejoin| rejoin.record.call.result.scalar_home())
            else {
                return Err("normalized foreign D41 child changed its scalar result custody");
            };
            if value.value_type() != *declared
                || Some(home.defining_operation) != source_operation
                || home.source_value != value.id
                || home.scalar_type != declared.scalar_type
            {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            let Some(shape) = foreign_scalar_shape(declared.scalar_type) else {
                return Ok(None);
            };
            if home.shape != shape {
                return Err("normalized foreign D41 child changed its scalar result custody");
            }
            Some(shape)
        }
        (
            Some(OperationResult::Structural(result)),
            BoundaryMachineResult::Structural(declared),
            None,
        ) => {
            let Some(rejoin) = fragment_rejoin else {
                return Ok(None);
            };
            let TargetBoundaryResult::Structural(home) = &rejoin.record.call.result else {
                return Err("normalized foreign D41 child changed its structural result custody");
            };
            let target_operations::TargetStructuralHomeLayout::Aggregate(shape) = home.layout
            else {
                return Err("normalized foreign D41 child changed its structural result layout");
            };
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
        && matches!(rejoin.record.call.result, TargetBoundaryResult::Unit) != result_shape.is_none()
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
        ([], materials) if fragment_rejoin.is_some() && !materials.is_empty() => {
            return Ok(None);
        }
        _ => return Err("normalized foreign D41 child changed its callback-plan custody"),
    };
    let mut callback_ordinals = Vec::with_capacity(callbacks.len());
    for callback in callbacks {
        if !matches!(callback.target.destination, NativePlace::Parameter(_)) {
            continue;
        }
        let ordinal = usize::try_from(callback.target.application.native_ordinal)
            .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
        if ordinal >= foreign.binding.boundary_entry_plan.call.parameters.len()
            || callback_ordinals.contains(&ordinal)
        {
            return Err("normalized foreign D41 callback ordinal is outside its native signature");
        }
        callback_ordinals.push(ordinal);
    }
    callback_ordinals.sort_unstable();
    // Native-only callbacks occupy ABI slots, not semantic formals. Walk the
    // complete operand roster once so consecutive callbacks and callbacks after
    // the last semantic argument retain the same correspondence as lowering.
    let dispatch_count = foreign.binding.dispatch_operand_count();
    let operand_count = foreign
        .binding
        .boundary_entry_plan
        .call
        .parameters
        .len()
        .checked_add(dispatch_count)
        .ok_or("normalized foreign D41 operand count overflow")?;
    let semantic_positions: Vec<_> = (0..operand_count)
        .filter(|position| {
            position
                .checked_sub(dispatch_count)
                .is_none_or(|ordinal| callback_ordinals.binary_search(&ordinal).is_err())
        })
        .collect();
    if semantic_positions.len() != declaration.parameter_order.len() {
        return Err("normalized foreign D41 child changed its complete native signature");
    }
    for (physical, semantic_position) in foreign
        .scalar_arguments
        .iter()
        .zip(declaration.parameter_positions(terminal_psi::BoundaryParameterKind::Scalar))
    {
        let native_position = semantic_positions[semantic_position];
        if usize::try_from(physical.parameter_index).ok() != Some(native_position)
            || foreign.binding.operand_placements().get(native_position)
                != Some(&physical.placement)
        {
            return Err("normalized foreign D41 child changed a scalar argument position");
        }
    }
    let mut scalar_shapes = parameter_shapes.into_iter();
    let mut structural_shapes = structural_parameter_shapes.into_iter().flatten();
    let semantic_shapes = declaration
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
    let mut native_parameter_shapes = vec![None; operand_count];
    for (position, shape) in semantic_positions.into_iter().zip(semantic_shapes) {
        native_parameter_shapes[position] = Some(shape);
    }
    for callback in callbacks {
        if !matches!(callback.target.destination, NativePlace::Parameter(_)) {
            continue;
        }
        let ordinal = usize::try_from(callback.target.application.native_ordinal)
            .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
        let position = ordinal
            .checked_add(dispatch_count)
            .ok_or("normalized foreign D41 callback position overflow")?;
        let slot = native_parameter_shapes
            .get_mut(position)
            .ok_or("normalized foreign D41 callback ordinal is outside its native signature")?;
        if slot.replace(callback.target.application.shape).is_some() {
            return Err("normalized foreign D41 child repeats a native parameter");
        }
    }
    let signature = calling_conventions::CallSignature {
        parameters: native_parameter_shapes
            .into_iter()
            .skip(dispatch_count)
            .collect::<Option<Vec<_>>>()
            .ok_or("normalized foreign D41 child lost a native parameter")?,
        result: result_shape,
    };

    let mut ordinal_cursor = callback_ordinals.iter().copied();
    for callback in callbacks {
        let expected_placement = match callback.destination.clone() {
            CallbackAddressDestination::Register(register) => ValuePlacement {
                shape: callback.target.application.shape,
                locations: vec![calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size: callback.target.application.shape.byte_size,
                }],
            },
            CallbackAddressDestination::OutgoingStack { byte_offset } => ValuePlacement {
                shape: callback.target.application.shape,
                locations: vec![calling_conventions::ValueLocation::Stack {
                    stack_byte_offset: byte_offset,
                    value_byte_offset: 0,
                    byte_size: callback.target.application.shape.byte_size,
                    alignment: callback.target.application.shape.alignment,
                }],
            },
            CallbackAddressDestination::RecordField {
                root_register,
                placements,
            } => {
                if callback.target.field_placements.as_ref() != Some(&placements) {
                    return Err("normalized foreign D41 child changed its callback target custody");
                }
                ValuePlacement {
                    shape: callback.target.application.shape,
                    locations: vec![calling_conventions::ValueLocation::Register {
                        register: root_register,
                        value_byte_offset: 0,
                        byte_size: callback.target.application.shape.byte_size,
                    }],
                }
            }
        };
        let callback_ordinal = usize::try_from(callback.target.application.native_ordinal)
            .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
        if matches!(callback.target.destination, NativePlace::Parameter(_))
            && ordinal_cursor.next() != Some(callback_ordinal)
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
    pass.verify(|| {
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
            validate_boundary_entry_plan(foreign.binding.boundary_entry_plan.clone(), &signature)
                .map_err(
                    |_| "normalized foreign D41 child contains an invalid boundary entry plan",
                )?
        };
        if validated_plan.plan() != &foreign.binding.boundary_entry_plan {
            return Err("normalized external child changed its boundary plan during replay");
        }
        Ok(())
    })?;
    let boundary_plan_identity = foreign
        .binding
        .boundary_entry_plan
        .contract_commitment_digest();
    let selected_requirement = selected_plan.selected_requirement_identity(requirement_identity);
    if foreign.binding.native_target() != Some(target)
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
        NormalizedForeignCallMechanism::Import {
            same_stack_contribution,
            ..
        }
        | NormalizedForeignCallMechanism::Component {
            same_stack_contribution,
            ..
        }
        | NormalizedForeignCallMechanism::Table {
            same_stack_contribution,
            ..
        } => same_stack_contribution
            .provider_plan_commitment()
            .as_bytes(),
        NormalizedForeignCallMechanism::SystemCall(kernel) => kernel.provider_plan_commitment,
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

    let matching_image_calls = object
        .foreign_calls()
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
    let intervals = CoveredInvocationIntervals::derive(function, &matching_attributions)?;
    intervals.require_invocation(
        pass,
        fragment_rejoin.map(|rejoin| rejoin.call_span),
        target.architecture,
        foreign,
        object.text_bytes(),
    )?;
    let expected_kind = match target.architecture {
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
    };
    let (mutable_intervals, relocation) = if foreign.binding.symbolic_import().is_none() {
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
            .symbolic_import()
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
                let expected_origin =
                    invocation_relocation_origin(function.symbol, invocation_site);
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
                                    import.locator.as_ref() == locator
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
                    let callback_symbol = callback_reference(
                        object,
                        linked_object,
                        custody.target.callback_function,
                    )?;
                    let exact_callback_relocation =
                        |section_offset: u64, kind: RelocationKind| -> Result<_, &'static str> {
                            let offset = usize::try_from(section_offset).map_err(|_| {
                                "normalized foreign D41 callback field offset does not fit this target"
                            })?;
                            admitted_callback_relocation(
                                object
                                    .relocations()
                                    .records()
                                    .map(|(_, relocation)| relocation),
                                expected_origin,
                                offset,
                                callback_symbol,
                                kind,
                            )
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
                .filter(|import| import.locator.as_ref() == locator)
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
            let expected_origin = invocation_relocation_origin(function.symbol, invocation_site);
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
                let callback_symbol =
                    callback_reference(object, linked_object, callback.target.callback_function)?;
                if !intervals.contains_object_span(callback.code_offset, callback.byte_count) {
                    return Err(
                        "normalized foreign D41 callback materialization left its operation interval",
                    );
                }
                let exact_callback_relocation =
                    |offset: usize, kind: RelocationKind| -> Result<_, &'static str> {
                        admitted_callback_relocation(
                            overlapping_relocations.iter().copied(),
                            expected_origin,
                            offset,
                            callback_symbol,
                            kind,
                        )
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

    let byte_digests = intervals.byte_digests(
        pass,
        function.bytes(object),
        object.text_bytes(),
        image,
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

// A private body retains local unresolved symbol handles after composition.
// Its relocation must name that local reference, while the completed object
// must define the same function identity, not merely a matching symbol name.
fn callback_reference(
    object: &image_emission::ObjectArtifact,
    linked_object: &image_emission::ObjectArtifact,
    function: function_identity::MachineFunctionIdentity,
) -> Result<object_file::ObjectSymbolHandle, &'static str> {
    object_file::object_function_symbol(linked_object.object(), function)
        .ok_or("normalized foreign D41 callback lost its linked private definition")?;
    object_file::object_function_reference(object.object(), function)
        .map(|(symbol, _)| symbol)
        .ok_or("normalized foreign D41 callback lost its private object reference")
}

fn admitted_callback_relocation<'a>(
    candidates: impl Iterator<Item = &'a object_file::RelocationRecord>,
    origin: object_file::RelocationOrigin,
    offset: usize,
    callback_symbol: object_file::ObjectSymbolHandle,
    kind: RelocationKind,
) -> Result<NormalizedForeignCallbackRelocation, &'static str> {
    let mut matching = candidates.filter(|candidate| {
        candidate.origin == origin
            && candidate.offset == offset
            && candidate.byte_width == 4
            && candidate.symbol_handle == callback_symbol
            && candidate.addend == 0
            && candidate.kind == kind
    });
    let (Some(matching), None) = (matching.next(), matching.next()) else {
        return Err("normalized foreign D41 callback changed a private-function relocation");
    };
    Ok(normalized_foreign_callback_relocation(
        matching.symbol_handle,
        matching.origin,
        matching.offset,
        matching.byte_width,
        matching.addend,
        matching.kind,
    ))
}

pub(super) fn foreign_scalar_shape(
    scalar_type: ScalarType,
) -> Option<calling_conventions::ValueShape> {
    calling_conventions::ValueShape::for_scalar(scalar_type)
}

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
        let access_pointer_word = placement
            .single_word_byte_size()
            .ok_or("normalized foreign D41 child changed its structural call custody")?;
        if placement.shape != pointer_shape || access_pointer_word != pointer_shape.byte_size {
            return Err("normalized foreign D41 child changed its structural call custody");
        }
        shapes.push(pointer_shape);
    }
    Ok(Some(shapes))
}

struct CoveredInvocationIntervals {
    machine: Vec<NativeByteSpan>,
    object: Vec<NativeByteSpan>,
    final_image: Vec<NativeByteSpan>,
}

struct CoveredByteDigests {
    machine: [u8; 32],
    object: [u8; 32],
    final_image: [u8; 32],
}

impl CoveredInvocationIntervals {
    fn derive(
        function: &ObjectFunction,
        attributions: &[&ObjectCodeAttribution],
    ) -> Result<Self, &'static str> {
        if attributions.is_empty() {
            return Err("normalized foreign call has no emitted attribution intervals");
        }
        let mut machine = Vec::with_capacity(attributions.len());
        let mut object = Vec::with_capacity(attributions.len());
        for attribution in attributions {
            let offset = attribution.attribution.code_offset;
            let count = attribution.attribution.byte_count;
            let object_offset = attribution.text_offset;
            if count == 0
                || function.text_offset.checked_add(offset) != Some(object_offset)
                || offset.checked_add(count).is_none()
                || object_offset.checked_add(count).is_none()
                || machine.last().is_some_and(|previous: &NativeByteSpan| {
                    previous
                        .offset()
                        .checked_add(previous.byte_count())
                        .is_none_or(|end| end >= offset)
                })
            {
                return Err(
                    "normalized foreign call has unordered, overlapping or detached intervals",
                );
            }
            machine.push(native_byte_span(offset, count));
            object.push(native_byte_span(object_offset, count));
        }
        Ok(Self {
            machine,
            final_image: object.clone(),
            object,
        })
    }

    fn contains_object_span(&self, offset: usize, count: usize) -> bool {
        count != 0
            && offset.checked_add(count).is_some_and(|end| {
                self.object.iter().any(|span| {
                    span.offset() <= offset
                        && span
                            .offset()
                            .checked_add(span.byte_count())
                            .is_some_and(|span_end| end <= span_end)
                })
            })
    }

    fn overlaps_object_span(&self, offset: usize, count: usize) -> bool {
        offset.checked_add(count).is_some_and(|end| {
            self.object.iter().any(|span| {
                span.offset()
                    .checked_add(span.byte_count())
                    .is_some_and(|span_end| ranges_overlap(span.offset(), span_end, offset, end))
            })
        })
    }

    fn require_invocation(
        &self,
        pass: crate::physical::PhysicalEvidencePass,
        placed: Option<&machine_code::PlacedInstructionSpan>,
        architecture: target::Architecture,
        foreign: &ObjectForeignCall,
        object_bytes: &[u8],
    ) -> Result<(), &'static str> {
        let (offset, count) = if let Some(placed) = placed {
            (
                usize::try_from(placed.section_offset)
                    .map_err(|_| "foreign call offset overflow")?,
                usize::try_from(placed.byte_count).map_err(|_| "foreign call size overflow")?,
            )
        } else {
            if foreign.binding.symbolic_import().is_none() {
                return Err("kernel entry lacks retained instruction custody");
            }
            match architecture {
                target::Architecture::X86_64 => {
                    let offset = foreign
                        .text_offset
                        .checked_sub(1)
                        .ok_or("foreign x86 call opcode offset underflow")?;
                    let end = offset
                        .checked_add(5)
                        .ok_or("foreign x86 call end overflow")?;
                    if pass.is_receiver()
                        && object_bytes.get(offset..end) != Some(&[0xe8, 0, 0, 0, 0])
                    {
                        return Err(
                            "foreign object call differs from its exact x86 import instruction",
                        );
                    }
                    (offset, 5)
                }
                target::Architecture::Aarch64 => {
                    let end = foreign
                        .text_offset
                        .checked_add(4)
                        .ok_or("foreign aarch64 call end overflow")?;
                    if !foreign.text_offset.is_multiple_of(4)
                        || pass.is_receiver()
                            && object_bytes.get(foreign.text_offset..end)
                                != Some(&0x9400_0000_u32.to_le_bytes())
                    {
                        return Err(
                            "foreign object call differs from its exact aarch64 import instruction",
                        );
                    }
                    (foreign.text_offset, 4)
                }
            }
        };
        if !self.contains_object_span(offset, count) {
            return Err("normalized foreign invocation instruction is outside its owned intervals");
        }
        Ok(())
    }

    fn byte_digests(
        &self,
        pass: crate::physical::PhysicalEvidencePass,
        machine_bytes: &[u8],
        object_bytes: &[u8],
        final_image_bytes: &[u8],
        mutable_offsets: &[(usize, usize)],
    ) -> Result<CoveredByteDigests, &'static str> {
        for &(start, end) in mutable_offsets {
            let count = end
                .checked_sub(start)
                .ok_or("foreign mutable interval is reversed")?;
            if !self.contains_object_span(start, count) {
                return Err("foreign mutable interval crosses uncovered bytes");
            }
        }
        let mut machine_digest = Sha256::new();
        let mut object_digest = Sha256::new();
        let mut final_image_digest = Sha256::new();
        for ((machine_span, object_span), image_span) in
            self.machine.iter().zip(&self.object).zip(&self.final_image)
        {
            let before_machine = span(machine_bytes, *machine_span)?;
            let before_object = span(object_bytes, *object_span)?;
            let after_image = span(final_image_bytes, *image_span)?;
            if pass.is_receiver() && before_machine != before_object {
                return Err("normalized foreign call changed before object custody");
            }
            if pass.is_receiver() {
                for (byte_index, (before, after)) in
                    before_object.iter().zip(after_image).enumerate()
                {
                    let offset = object_span
                        .offset()
                        .checked_add(byte_index)
                        .ok_or("normalized foreign covered byte offset overflow")?;
                    if before != after
                        && !mutable_offsets
                            .iter()
                            .any(|(start, end)| offset >= *start && offset < *end)
                    {
                        return Err(
                            "normalized foreign bytes changed outside their exact relocation set",
                        );
                    }
                }
            }
            machine_digest.update(before_machine);
            object_digest.update(before_object);
            final_image_digest.update(after_image);
        }
        Ok(CoveredByteDigests {
            machine: machine_digest.finalize().into(),
            object: object_digest.finalize().into(),
            final_image: final_image_digest.finalize().into(),
        })
    }
}

pub(super) mod floating_control {
    use crate::physical::fragment_publication::FragmentPublicationBinding;
    use image_emission::ObjectForeignCall;
    use image_emission::fragment_instruction_span;
    use machine_code::PlacedFunctionFragment;
    use selected_instructions::calls::normalized_foreign::return_transport;
    use selected_instructions::calls::normalized_foreign::return_transport::resolve;
    use selected_instructions::{
        LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedNormalizedForeignCall,
    };
    use target::Architecture;

    pub(in crate::physical::derivation) fn rejoin(
        pass: crate::physical::PhysicalEvidencePass,
        publication: &FragmentPublicationBinding,
        function: &SelectedFunction,
        block: &SelectedBlock,
        fragment: &PlacedFunctionFragment,
        record: &SelectedNormalizedForeignCall,
        foreign: &ObjectForeignCall,
    ) -> bool {
        check(
            pass,
            publication,
            function,
            block,
            fragment,
            record,
            foreign,
        ) == Some(())
    }

    fn check(
        pass: crate::physical::PhysicalEvidencePass,
        publication: &FragmentPublicationBinding,
        function: &SelectedFunction,
        block: &SelectedBlock,
        fragment: &PlacedFunctionFragment,
        record: &SelectedNormalizedForeignCall,
        foreign: &ObjectForeignCall,
    ) -> Option<()> {
        if record.invocation_site.operation().is_none() {
            return (foreign.x86_floating_control.is_none()
                && foreign.aarch64_floating_control.is_none())
            .then_some(());
        }
        let operation = record.invocation_site.operation()?;
        let section = publication.text_section();
        let (target, displacement, save_offset, save_count, restore_offset, restore_count) =
            match section.target.architecture {
                Architecture::Aarch64 => {
                    let control = foreign.aarch64_floating_control?;
                    if foreign.x86_floating_control.is_some() {
                        return None;
                    }
                    (
                        control.target,
                        control.saved_slot_byte_offset,
                        control.save_offset,
                        control.save_byte_count,
                        control.restore_offset,
                        control.restore_byte_count,
                    )
                }
                Architecture::X86_64 => {
                    let control = foreign.x86_floating_control?;
                    if foreign.aarch64_floating_control.is_some() {
                        return None;
                    }
                    (
                        control.target,
                        control.saved_slot_byte_offset,
                        control.save_offset,
                        control.save_byte_count,
                        control.restore_offset,
                        control.restore_byte_count,
                    )
                }
            };
        if target != section.target || !displacement.is_multiple_of(8) {
            return None;
        }
        let slot = LocalStorageSlotId::Boundary { operation };
        let mut homes = function
            .local_storage_slots
            .iter()
            .filter(|home| home.id == slot);
        let home = homes.next()?;
        if homes.next().is_some() || home.byte_size != 8 || home.alignment != 8 {
            return None;
        }

        let transport = resolve(function, block, record)?;
        let call_position = transport.call_position;
        let restore_position = transport.restore_position?;
        let restore = block.instructions.get(restore_position)?;
        let save_position =
            return_transport::save_position(function, block, record, call_position)?;
        let save = &block.instructions[save_position];
        let matches_span = |instruction, offset, count| {
            fragment_instruction_span(fragment, instruction).is_some_and(|(start, size)| {
                usize::try_from(start) == Ok(offset) && usize::try_from(size) == Ok(count)
            })
        };
        let (call_start, call_size) = fragment_instruction_span(fragment, record.instruction)?;
        let transported_end = block.instructions[call_position + 1..restore_position]
            .iter()
            .try_fold(call_start.checked_add(call_size)?, |end, copy| {
                let (offset, count) = fragment_instruction_span(fragment, copy.id)?;
                (offset == end).then_some(())?;
                offset.checked_add(count)
            })?;
        if !matches_span(save.id, save_offset, save_count)
            || !matches_span(restore.id, restore_offset, restore_count)
            || u64::try_from(save_offset.checked_add(save_count)?).ok()? > call_start
            || u64::try_from(restore_offset).ok()? != transported_end
        {
            return None;
        }
        let saved_bytes = section
            .bytes
            .get(save_offset..save_offset.checked_add(save_count)?)?;
        let restored_bytes = section
            .bytes
            .get(restore_offset..restore_offset.checked_add(restore_count)?)?;
        if !pass.is_receiver() {
            return Some(());
        }
        let exact = match target.architecture {
            Architecture::Aarch64 => {
                saved_bytes
                    == isa_aarch64::encode_save_fpcr_to_sp_displacement(displacement).ok()?
                    && restored_bytes
                        == isa_aarch64::encode_restore_fpcr_from_sp_displacement(displacement)
                            .ok()?
            }
            Architecture::X86_64 => {
                saved_bytes == isa_x86_64::encode_stmxcsr_rsp_displacement(displacement).ok()?
                    && restored_bytes
                        == isa_x86_64::encode_ldmxcsr_rsp_displacement(displacement).ok()?
            }
        };
        exact.then_some(())
    }
}

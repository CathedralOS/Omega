//! Derive admitted provider settlement evidence by replaying its retained custody.

use crate::BoundaryTraitSettlementParts;
use crate::BoundaryTraitSettlementRole;
use crate::NativePhysicalChild;
use crate::NativePhysicalChildParts;
use crate::NativePhysicalOccurrence;
use crate::NativeProviderExecution;
use crate::NativeSelectedProviderPlan;
use crate::OptimizedBoundaryOccurrence;
use crate::PhysicalChildParent;
use crate::PhysicalRelocationDisposition;
use crate::physical::derivation::evidence::physical_child_identity;
use crate::physical::derivation::evidence::ranges_overlap;
use crate::physical::derivation::evidence::span;
use crate::physical::derivation::hashing::sha256;
use crate::physical::derivation::provider_custody::direct_port_read_custody;
use crate::physical::derivation::provider_custody::linux_write_line_custody_is_exact;
use crate::physical::derivation::provider_custody::metadata_port_effect_custody;
use crate::physical::derivation::provider_custody::rejoin_admitted_provider_execution;
use crate::physical::derivation::provider_custody::settlement_completion_custody_is_exact;
use crate::physical::derivation::settlement_identity::admitted_provider_settlement_identity;
use crate::physical::native_byte_span;
use machine_code::BoundaryExecutionRecord;
use object_file::SectionKind;
use optimization_core::NativeOptimizationProjectionIdentity;
use std::collections::BTreeSet;
use target::NativeTarget;
use target_operations::BoundaryRealization;
use terminal_psi::OperationKind;

/// Derive the D41 physical child for one installed provider settlement
/// realized in place by a supported target mechanism.
///
/// `Ok(None)` leaves realizations this lane does not cover without a claimed
/// child; the artifact remains valid but retains no complete physical
/// evidence. Custody drift that would contradict a validated object or its
/// Terminal operation is an error, matching the hosted-builtin derivations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_admitted_provider_settlement_child(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    installed: &image_emission::ObjectBoundarySettlement,
    consumed_port_effects: &mut BTreeSet<usize>,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let settlement = &installed.settlement;
    let BoundaryExecutionRecord::AdmittedProvider(execution_record) = settlement.execution else {
        return Ok(None);
    };
    let supported = matches!(
        settlement.realization,
        BoundaryRealization::MetadataOnlyPort(_)
            | BoundaryRealization::DirectPortReadU8(_)
            | BoundaryRealization::LinuxWriteLine(_)
            | BoundaryRealization::ClaimCompletionOnly(_)
    );
    if !supported {
        return Ok(None);
    }
    // The retained settlement must still rejoin exactly one Terminal boundary
    // call on the occurrence's machine, and that call must carry the
    // settlement's exact structural-argument and completion-receipt custody.
    // None of the supported mechanisms retain a scalar argument.
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("installed D41 settlement does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        structural_arguments,
        completion_receipts,
        // Proof lane: verified in Psi, no physical correspondence.
        requirement_obligations: _,
    } = &operation.kind
    else {
        return Err("installed D41 settlement owner is not a Terminal boundary call");
    };
    let declaration = module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .ok_or("installed D41 settlement names an absent boundary")?;
    if *boundary != occurrence.boundary()
        || settlement.psi_operation != occurrence.operation()
        || settlement.boundary != *boundary
        || declaration.identity != requirement_identity
        || !arguments.is_empty()
        || *structural_arguments != settlement.arguments
        || *completion_receipts != settlement.completion_receipts
    {
        return Err("installed D41 settlement changed its semantic operation custody");
    }
    if !settlement_completion_custody_is_exact(settlement) {
        return Err("installed D41 settlement changed its completion custody");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("installed D41 settlement names an absent object function")?;
    if !function
        .provenance
        .operations
        .contains(&settlement.psi_operation)
    {
        return Err("installed D41 settlement operation left its function provenance");
    }
    let port_effect = match &settlement.realization {
        BoundaryRealization::MetadataOnlyPort(realization) => Some(metadata_port_effect_custody(
            module,
            object,
            image,
            occurrence,
            settlement,
            realization,
            target,
            function,
            operation,
            declaration,
            consumed_port_effects,
        )?),
        BoundaryRealization::DirectPortReadU8(realization) => {
            direct_port_read_custody(
                object,
                image,
                occurrence,
                settlement,
                realization,
                target,
                function,
                operation,
                declaration,
            )?;
            None
        }
        BoundaryRealization::LinuxWriteLine(_) => {
            if !linux_write_line_custody_is_exact(target, settlement, function.bytes(object))
                || function.unit_stack.is_none()
                || function.scalar_stack.is_some()
                || !matches!(operation.result, terminal_psi::OperationResult::Unit)
                || !declaration.result.is_unit()
            {
                return Err("installed D41 write-line custody is incomplete or substituted");
            }
            None
        }
        BoundaryRealization::ClaimCompletionOnly(_) => {
            if !settlement.scalar_arguments.is_empty()
                || !settlement.runtime_scalar_arguments.is_empty()
                || !settlement.byte_sequence_arguments.is_empty()
                || !settlement.native_result.is_unit()
                || settlement.byte_count != 0
                || !matches!(operation.result, terminal_psi::OperationResult::Unit)
                || !declaration.result.is_unit()
            {
                return Err("installed D41 claim-completion custody is incomplete or substituted");
            }
            None
        }
        _ => unreachable!("supported installed realizations were dispatched above"),
    };
    let execution = rejoin_admitted_provider_execution(
        requirement_identity,
        selected_plan,
        provider_executions,
        execution_record,
    )?;

    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("installed D41 settlement object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("installed D41 settlement object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("installed D41 settlement bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("installed D41 settlement relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("installed D41 settlement unexpectedly contains a relocation");
    }

    let role = BoundaryTraitSettlementRole::AdmittedProviderSettlement {
        execution,
        settlement: settlement.clone(),
        port_effect: port_effect.clone(),
    };
    let parent_identity = admitted_provider_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan.plan_digest(),
        target,
        execution,
        settlement,
        port_effect.as_ref(),
    )?;
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
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let relocation = PhysicalRelocationDisposition::DirectInstructionBytes;
    let identity = physical_child_identity(
        &parent,
        projection,
        NativePhysicalOccurrence::Boundary(occurrence.identity()),
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        &relocation,
    );
    Ok(Some(
        NativePhysicalChildParts {
            parent,
            projection,
            occurrence: NativePhysicalOccurrence::Boundary(occurrence.identity()),
            machine_span,
            object_span,
            final_image_span,
            machine_bytes_digest,
            object_bytes_digest,
            final_image_bytes_digest,
            relocation,
            identity,
        }
        .into(),
    ))
}

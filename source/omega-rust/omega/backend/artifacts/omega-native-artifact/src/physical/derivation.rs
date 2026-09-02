use std::collections::{BTreeMap, BTreeSet};

use omega_boundary_applications::TerminalBoundaryApplicationCoverage;
use omega_installation_evidence::ProviderExecutionEvidence;
use omega_machine_code::BoundaryExecutionRecord;
use omega_object_file::{RelocationKind, RelocationOrigin, SectionKind};
use omega_optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
    OptimizedOperatorOccurrenceIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{
    BoundaryRealization, CallSiteOwner, CompilerBuiltinExecution, NormalizedForeignCallBinding,
    ProviderExecutionBinding, ProviderPlanReportIdentity,
};
use psi_core::{IntegerSign, IntegerType, ScalarType};
use psi_terminal::OperationKind;
use sha2::{Digest, Sha256};

use super::{model::*, operator_applications::derive_operator_physical_span};
use crate::{
    NativePhysicalEvidenceScope, NativeProviderExecution, NativeSelectedProviderPlan,
    NativeSelectedProviderPlanDigest,
    boundary_applications::boundary_application_coverage_identity,
};

pub(crate) fn derive_physical_evidence(
    scope: &NativePhysicalEvidenceScope,
    terminal_artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image::EmittedImageOutput,
    final_image_symbol_digest: [u8; 32],
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Result<Option<NativePhysicalEvidence>, &'static str> {
    if matches!(scope, NativePhysicalEvidenceScope::Unavailable) {
        return Ok(None);
    }
    if let NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) = scope
        && let Some(publication) = optimized.selected_lowering_publication()
    {
        super::selected_lowering::validate_selected_lowering_publication_object(
            publication,
            object,
        )?;
    }
    let module = psi_terminal_codec::decode_module(terminal_artifact.semantic_bytes())
        .map_err(|_| "native physical evidence cannot decode Terminal semantics")?;
    if module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
        || !object.port_effects().is_empty()
    {
        return Ok(None);
    }
    let boundary_application_coverage = boundary_application_coverage
        .ok_or("native physical evidence requires exact boundary-application coverage custody")?;

    let projection = match scope {
        NativePhysicalEvidenceScope::Unavailable => unreachable!("unavailable returned above"),
        NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence => {
            derive_identity_projection(
                terminal_artifact.manifest().semantic(),
                &module,
                boundary_application_coverage,
            )?
        }
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(optimized) => {
            if optimized.projection().terminal() != terminal_artifact.manifest().semantic()
                || optimized.boundary_application_coverage()
                    != &boundary_application_coverage_identity(Some(boundary_application_coverage))
                        .expect("present boundary-application coverage has an identity")
            {
                return Err(
                    "optimized physical scope is detached from its Terminal or D29 coverage",
                );
            }
            optimized.projection().clone()
        }
    };
    let mut settlements = BTreeMap::new();
    for installed in object.boundary_settlements() {
        let key = (
            installed.machine,
            installed.settlement.psi_operation,
            installed.settlement.boundary,
            installed.settlement.operation_ordinal,
        );
        if settlements.insert(key, installed).is_some() {
            return Err("native physical evidence found duplicate boundary settlements");
        }
    }

    let occurrence_keys = projection
        .boundary_occurrences()
        .iter()
        .map(|occurrence| {
            (
                occurrence.machine(),
                occurrence.operation(),
                occurrence.boundary(),
                occurrence.operation_ordinal(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    if settlements.keys().any(|key| !occurrence_keys.contains(key)) {
        return Err("native physical evidence found a stale boundary settlement");
    }

    let mut foreign_calls = BTreeMap::new();
    for foreign in object.foreign_calls() {
        let CallSiteOwner::Operation(operation) = foreign.owner else {
            return Ok(None);
        };
        let key = (foreign.machine, operation);
        if foreign_calls.insert(key, foreign).is_some() {
            return Err("native physical evidence found duplicate normalized foreign calls");
        }
    }
    let foreign_occurrence_keys = projection
        .boundary_occurrences()
        .iter()
        .map(|occurrence| (occurrence.machine(), occurrence.operation()))
        .collect::<BTreeSet<_>>();
    if foreign_calls
        .keys()
        .any(|key| !foreign_occurrence_keys.contains(key))
    {
        return Err("native physical evidence found a stale normalized foreign call");
    }

    let boundary_identities = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary.identity.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut children = Vec::new();
    for occurrence in projection.boundary_occurrences() {
        let key = (
            occurrence.machine(),
            occurrence.operation(),
            occurrence.boundary(),
            occurrence.operation_ordinal(),
        );
        let installed = settlements.get(&key);
        let foreign = foreign_calls.get(&(occurrence.machine(), occurrence.operation()));
        if installed.is_some() && foreign.is_some() {
            return Err("native physical evidence found two realizations for one boundary call");
        }
        let requirement = boundary_identities
            .get(&occurrence.boundary())
            .copied()
            .ok_or("native physical evidence names an absent boundary requirement")?;
        let matching_plans = selected_provider_plans
            .iter()
            .filter(|plan| {
                plan.requirement_identities()
                    .binary_search_by(|identity| identity.as_str().cmp(requirement))
                    .is_ok()
            })
            .collect::<Vec<_>>();
        let [selected_plan] = matching_plans.as_slice() else {
            return Err("native physical evidence cannot rejoin one exact selected provider plan");
        };
        match (installed, foreign) {
            (Some(installed), None)
                if matches!(
                    (
                        installed.settlement.execution,
                        &installed.settlement.realization,
                    ),
                    (
                        BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::LinuxExitGroupI32
                        ),
                        BoundaryRealization::LinuxExitGroupI32(_),
                    )
                ) =>
            {
                if installed.settlement.byte_count == 0 {
                    return Err("Linux exit-group physical child requires a nonempty emitted span");
                }
                children.push(derive_exit_group_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan.plan_digest(),
                    target,
                    object,
                    image,
                    installed,
                )?);
            }
            (Some(installed), None)
                if matches!(
                    (
                        installed.settlement.execution,
                        &installed.settlement.realization,
                    ),
                    (
                        BoundaryExecutionRecord::CompilerBuiltin(
                            CompilerBuiltinExecution::LinuxWriteByteI32
                        ),
                        BoundaryRealization::LinuxWriteByteI32(_),
                    )
                ) =>
            {
                children.push(derive_write_byte_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan.plan_digest(),
                    target,
                    object,
                    image,
                    installed,
                )?);
            }
            (None, Some(foreign)) => {
                let Some(child) = derive_normalized_foreign_child(
                    occurrence,
                    projection.identity(),
                    requirement,
                    selected_plan,
                    provider_executions,
                    target,
                    &module,
                    object,
                    image,
                    final_image_symbol_digest,
                    foreign,
                )?
                else {
                    return Ok(None);
                };
                children.push(child);
            }
            _ => return Ok(None),
        }
    }
    for occurrence in projection.operator_occurrences() {
        let matching_references = boundary_application_coverage
            .references()
            .iter()
            .filter(|reference| reference.terminal_operation() == occurrence.operation())
            .collect::<Vec<_>>();
        let [reference] = matching_references.as_slice() else {
            return Err("D29 physical occurrence does not rejoin one coverage reference");
        };
        let matching_realizations = boundary_application_coverage
            .realizations()
            .rows()
            .iter()
            .filter(|realization| realization.terminal_operation() == occurrence.operation())
            .collect::<Vec<_>>();
        let [realization] = matching_realizations.as_slice() else {
            return Err("D29 physical occurrence does not rejoin one realization companion");
        };
        let Some(span) = derive_operator_physical_span(
            occurrence,
            realization.realization(),
            &module,
            target,
            object,
            image,
        )?
        else {
            // Unsupported compiler-intrinsic or call mechanics leave the
            // artifact valid without claiming complete physical coverage.
            return Ok(None);
        };
        let parent = PhysicalChildParent::OperatorApplicationCoverage(**reference);
        let physical_occurrence = NativePhysicalOccurrence::Operator(occurrence.identity());
        let identity = physical_child_identity(
            &parent,
            projection.identity(),
            physical_occurrence,
            span.machine,
            span.object,
            span.final_image,
            span.machine_bytes_digest,
            span.object_bytes_digest,
            span.final_image_bytes_digest,
            span.relocation,
        );
        children.push(
            NativePhysicalChildParts {
                parent,
                projection: projection.identity(),
                occurrence: physical_occurrence,
                machine_span: span.machine,
                object_span: span.object,
                final_image_span: span.final_image,
                machine_bytes_digest: span.machine_bytes_digest,
                object_bytes_digest: span.object_bytes_digest,
                final_image_bytes_digest: span.final_image_bytes_digest,
                relocation: span.relocation,
                identity,
            }
            .into(),
        );
    }
    children.sort_by_key(|child| child.occurrence());
    validate_exact_physical_children(&projection, &children)?;
    let identity = physical_evidence_identity(projection.identity(), &children);
    Ok(Some(native_physical_evidence(
        projection, children, identity,
    )))
}

#[derive(Clone, Copy)]
struct PhysicalChildCoordinate {
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    parent_role: u8,
}

fn validate_exact_physical_children(
    projection: &NativeOptimizationProjection,
    children: &[NativePhysicalChild],
) -> Result<(), &'static str> {
    validate_exact_physical_child_coordinates(
        projection,
        children.iter().map(|child| PhysicalChildCoordinate {
            projection: child.projection(),
            occurrence: child.occurrence(),
            parent_role: child.parent().role_tag(),
        }),
    )
}

fn validate_exact_physical_child_coordinates(
    projection: &NativeOptimizationProjection,
    children: impl IntoIterator<Item = PhysicalChildCoordinate>,
) -> Result<(), &'static str> {
    let expected = projection
        .operator_occurrences()
        .iter()
        .map(|occurrence| (NativePhysicalOccurrence::Operator(occurrence.identity()), 1))
        .chain(
            projection
                .boundary_occurrences()
                .iter()
                .map(|occurrence| (NativePhysicalOccurrence::Boundary(occurrence.identity()), 2)),
        )
        .collect::<BTreeMap<_, _>>();
    if expected.len()
        != projection
            .operator_occurrences()
            .len()
            .checked_add(projection.boundary_occurrences().len())
            .ok_or("native physical evidence occurrence count overflow")?
    {
        return Err("native physical evidence projection repeats an optimized occurrence");
    }

    let mut observed = BTreeMap::new();
    for child in children {
        if child.projection != projection.identity() {
            return Err("native physical child is detached from its optimized projection");
        }
        if child.parent_role != child.occurrence.role_tag()
            || expected.get(&child.occurrence) != Some(&child.parent_role)
        {
            return Err("native physical child swapped or substituted its semantic parent role");
        }
        if observed
            .insert(child.occurrence, child.parent_role)
            .is_some()
        {
            return Err("native physical evidence contains duplicate optimized occurrences");
        }
    }
    if observed != expected {
        return Err("native physical evidence does not cover the exact surviving occurrence set");
    }
    Ok(())
}

fn derive_identity_projection(
    terminal: psi_terminal::TerminalPsiIdentity,
    module: &psi_terminal::TerminalModule,
    boundary_application_coverage: &TerminalBoundaryApplicationCoverage,
) -> Result<NativeOptimizationProjection, &'static str> {
    let operator_operations = boundary_application_coverage
        .references()
        .iter()
        .map(|reference| reference.terminal_operation())
        .collect::<BTreeSet<_>>();
    if operator_operations.len() != boundary_application_coverage.references().len() {
        return Err("native optimization projection contains duplicate D29 operations");
    }
    let mut operator_occurrences = Vec::with_capacity(operator_operations.len());
    let mut boundary_occurrences = Vec::new();
    for machine in &module.machines {
        let mut operation_ordinal = 0_usize;
        for block in &machine.blocks {
            for operation in &block.operations {
                if operator_operations.contains(&operation.id) {
                    let identity = operator_occurrence_identity(
                        terminal,
                        machine.id,
                        operation.id,
                        operation_ordinal,
                    );
                    operator_occurrences.push(optimized_operator_occurrence(
                        terminal,
                        machine.id,
                        operation.id,
                        operation_ordinal,
                        identity,
                    ));
                }
                if let OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    let identity = boundary_occurrence_identity(
                        terminal,
                        machine.id,
                        operation.id,
                        boundary,
                        operation_ordinal,
                    );
                    boundary_occurrences.push(optimized_boundary_occurrence(
                        terminal,
                        machine.id,
                        operation.id,
                        boundary,
                        operation_ordinal,
                        identity,
                    ));
                }
                operation_ordinal = operation_ordinal
                    .checked_add(1)
                    .ok_or("native optimization projection operation ordinal overflow")?;
            }
            operation_ordinal = operation_ordinal
                .checked_add(1)
                .ok_or("native optimization projection terminator ordinal overflow")?;
        }
    }
    if operator_occurrences.len() != operator_operations.len() {
        return Err("native optimization projection cannot rejoin every D29 operation");
    }
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.push(1); // D29 operator-application occurrences.
    canonical.extend_from_slice(&canonical_usize(operator_occurrences.len()));
    for occurrence in &operator_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    canonical.push(2); // D41 ordinary boundary-trait occurrences.
    canonical.extend_from_slice(&canonical_usize(boundary_occurrences.len()));
    for occurrence in &boundary_occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    let identity = NativeOptimizationProjectionIdentity::from_canonical_bytes(&canonical);
    Ok(native_optimization_projection(
        terminal,
        operator_occurrences,
        boundary_occurrences,
        identity,
    ))
}

fn operator_occurrence_identity(
    terminal: psi_terminal::TerminalPsiIdentity,
    machine: psi_core::MachineId,
    operation: psi_core::OperationId,
    operation_ordinal: usize,
) -> OptimizedOperatorOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(&canonical)
}

fn boundary_occurrence_identity(
    terminal: psi_terminal::TerminalPsiIdentity,
    machine: psi_core::MachineId,
    operation: psi_core::OperationId,
    boundary: psi_core::BoundaryMachineId,
    operation_ordinal: usize,
) -> OptimizedBoundaryOccurrenceIdentity {
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&operation.get().to_le_bytes());
    canonical.extend_from_slice(&boundary.get().to_le_bytes());
    canonical.extend_from_slice(&canonical_usize(operation_ordinal));
    OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(&canonical)
}

#[allow(clippy::too_many_arguments)]
fn derive_exit_group_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image::EmittedImageOutput,
    installed: &omega_image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let [scalar_argument] = settlement.scalar_arguments.as_slice() else {
        return Err("Linux exit-group physical child requires one scalar argument");
    };
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 is valid");
    let expected_destination = match (target.object_format, target.architecture) {
        (ObjectFormat::Elf, Architecture::X86_64) => {
            omega_target_operations::MachineRegister::X86Rdi
        }
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            omega_target_operations::MachineRegister::Aarch64X(0)
        }
        _ => return Err("Linux exit-group physical child requires a Linux ELF target"),
    };
    if scalar_argument.scalar_type != ScalarType::Integer(i32_type)
        || !matches!(scalar_argument.immediate, psi_core::IntegerValue::Signed(value) if i32::try_from(value).is_ok())
        || scalar_argument.destination != expected_destination
        || !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || settlement.native_result.is_some()
    {
        return Err("Linux exit-group D41 settlement custody is incomplete or substituted");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("Linux exit-group physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("Linux exit-group physical child object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("Linux exit-group physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("Linux exit-group physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("Linux exit-group physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("Linux exit-group physical child unexpectedly contains a relocation");
    }

    let role = BoundaryTraitSettlementRole::CompilerBuiltin {
        catalog: NativeCompilerBuiltinCatalogIdentity::LinuxElfV1,
        execution: CompilerBuiltinExecution::LinuxExitGroupI32,
        realization: BoundaryRealization::LinuxExitGroupI32(Default::default()),
        scalar_argument: *scalar_argument,
    };
    let parent_identity = builtin_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        scalar_argument,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
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
        relocation,
    );
    Ok(NativePhysicalChildParts {
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
    .into())
}

#[allow(clippy::too_many_arguments)]
fn derive_write_byte_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image::EmittedImageOutput,
    installed: &omega_image_emission::ObjectBoundarySettlement,
) -> Result<NativePhysicalChild, &'static str> {
    let settlement = &installed.settlement;
    let [scalar_argument] = settlement.runtime_scalar_arguments.as_slice() else {
        return Err("Linux write-byte physical child requires one runtime scalar argument");
    };
    if target.object_format != ObjectFormat::Elf
        || !matches!(
            target.architecture,
            Architecture::X86_64 | Architecture::Aarch64
        )
        || !settlement.scalar_arguments.is_empty()
        || !settlement.arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.completion_claim_sources.is_empty()
        || !settlement.completion_receipts.is_empty()
        || !settlement.completion_provider_custody.is_empty()
        || settlement.native_result.is_some()
    {
        return Err("Linux write-byte D41 settlement custody is incomplete or substituted");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("Linux write-byte physical child names an absent object function")?;
    let expected_object_offset = function
        .text_offset
        .checked_add(settlement.code_offset)
        .ok_or("Linux write-byte physical child object span overflow")?;
    if installed.text_offset != expected_object_offset {
        return Err("Linux write-byte physical child object span is detached");
    }
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    let object_span = native_byte_span(installed.text_offset, settlement.byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes || object_bytes != final_image_bytes {
        return Err("Linux write-byte physical child bytes changed across physical custody");
    }
    let object_end = installed
        .text_offset
        .checked_add(settlement.byte_count)
        .ok_or("Linux write-byte physical child relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                installed.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("Linux write-byte physical child unexpectedly contains a relocation");
    }
    let role = BoundaryTraitSettlementRole::CompilerBuiltinRuntimeScalar {
        catalog: NativeCompilerBuiltinCatalogIdentity::LinuxElfV1,
        execution: CompilerBuiltinExecution::LinuxWriteByteI32,
        realization: BoundaryRealization::LinuxWriteByteI32(Default::default()),
        scalar_argument: scalar_argument.clone(),
    };
    let parent_identity = builtin_runtime_scalar_boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        scalar_argument,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
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
        relocation,
    );
    Ok(NativePhysicalChildParts {
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
    .into())
}

#[allow(clippy::too_many_arguments)]
fn derive_normalized_foreign_child(
    occurrence: &OptimizedBoundaryOccurrence,
    projection: NativeOptimizationProjectionIdentity,
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    target: NativeTarget,
    module: &psi_terminal::TerminalModule,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image::EmittedImageOutput,
    final_image_symbol_digest: [u8; 32],
    foreign: &omega_image_emission::ObjectForeignCall,
) -> Result<Option<NativePhysicalChild>, &'static str> {
    let matching_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.operation())
        .collect::<Vec<_>>();
    let [operation] = matching_operations.as_slice() else {
        return Err("normalized foreign D41 occurrence does not rejoin one Terminal operation");
    };
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        structural_arguments,
        completion_receipts,
    } = &operation.kind
    else {
        return Err("normalized foreign D41 occurrence is not a Terminal boundary call");
    };
    let declaration = module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
        .ok_or("normalized foreign D41 occurrence names an absent boundary")?;
    if *boundary != occurrence.boundary()
        || declaration.identity != requirement_identity
        || !structural_arguments.is_empty()
        || !completion_receipts.is_empty()
        || !declaration.structural_parameters.is_empty()
    {
        return Ok(None);
    }

    if foreign.operation_ordinal != occurrence.operation_ordinal()
        || arguments.len() != declaration.scalar_parameters.len()
        || foreign.scalar_arguments.len() != arguments.len()
    {
        return Err("normalized foreign D41 child changed its scalar call occurrence");
    }
    let Some(parameter_shapes) = declaration
        .scalar_parameters
        .iter()
        .copied()
        .map(fixed_integer_shape)
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
    let result_shape = match (
        &operation.result,
        declaration.result,
        &foreign.scalar_result,
    ) {
        (psi_terminal::OperationResult::Unit, None, None) => None,
        (psi_terminal::OperationResult::Scalar(value), Some(declared), Some(physical))
            if value.scalar_type == declared
                && physical.home.source_value == value.id
                && physical.home.scalar_type == declared =>
        {
            let Some(shape) = fixed_integer_shape(declared) else {
                return Ok(None);
            };
            Some(shape)
        }
        _ => return Err("normalized foreign D41 child changed its scalar result custody"),
    };
    let callback = match (
        foreign.callback_address.as_ref(),
        foreign
            .boundary_entry_plan
            .call
            .callback_materializations
            .as_slice(),
    ) {
        (None, []) => None,
        (Some(callback), [_]) => Some(callback),
        _ => return Err("normalized foreign D41 child changed its callback-plan custody"),
    };
    let callback_ordinal = callback
        .map(|callback| usize::try_from(callback.target.application.native_ordinal))
        .transpose()
        .map_err(|_| "normalized foreign D41 callback ordinal does not fit this target")?;
    if callback_ordinal.is_some_and(|ordinal| ordinal > parameter_shapes.len()) {
        return Err("normalized foreign D41 callback ordinal is outside its native signature");
    }
    let mut native_parameter_shapes = parameter_shapes;
    if let (Some(callback), Some(ordinal)) = (callback, callback_ordinal) {
        native_parameter_shapes.insert(ordinal, callback.target.application.shape);
    }
    let signature = omega_calling_conventions::CallSignature {
        parameters: native_parameter_shapes,
        result: result_shape,
    };

    let validated_plan = match callback {
        Some(callback) => {
            let callback_ordinal = callback_ordinal
                .expect("callback custody establishes one native parameter ordinal");
            let pointer_shape = omega_calling_conventions::ValueShape::integer(
                u16::try_from(target.pointer_size)
                    .map_err(|_| "normalized foreign D41 pointer size does not fit its ABI")?,
                u16::try_from(target.pointer_alignment)
                    .map_err(|_| "normalized foreign D41 pointer alignment does not fit its ABI")?,
            );
            let expected_placement = match callback.destination {
                omega_machine_code::CallbackAddressDestination::Register(register) => {
                    omega_calling_conventions::ValuePlacement {
                        shape: callback.target.application.shape,
                        locations: vec![omega_calling_conventions::ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: callback.target.application.shape.byte_size,
                        }],
                    }
                }
                omega_machine_code::CallbackAddressDestination::OutgoingStack { byte_offset } => {
                    omega_calling_conventions::ValuePlacement {
                        shape: callback.target.application.shape,
                        locations: vec![omega_calling_conventions::ValueLocation::Stack {
                            stack_byte_offset: byte_offset,
                            value_byte_offset: 0,
                            byte_size: callback.target.application.shape.byte_size,
                            alignment: callback.target.application.shape.alignment,
                        }],
                    }
                }
            };
            if callback.target.terminal_operation != occurrence.operation()
                || callback.target.registrar_boundary_entry_plan != foreign.boundary_entry_plan
                || callback
                    .target
                    .callback_function
                    .callback_thunk_placement_index()
                    != Some(callback.target.placement_index)
                || callback.target.registrar_application_commitment == [0; 32]
                || callback.target.application.shape != pointer_shape
                || callback.target.application.placement != expected_placement
                || foreign
                    .boundary_entry_plan
                    .call
                    .parameters
                    .get(callback_ordinal)
                    != Some(&callback.target.application.placement)
                || foreign.boundary_entry_plan.call.callback_materializations[0].destination
                    != omega_calling_conventions::NativePlace::Parameter(
                        callback.target.application.parameter,
                    )
            {
                return Err("normalized foreign D41 child changed its callback target custody");
            }
            omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                foreign.boundary_entry_plan.clone(),
                &signature,
                &callback.target.registrar_context,
            )
            .map_err(
                |_| "normalized foreign D41 child contains an invalid callback boundary entry plan",
            )?
        }
        None => omega_calling_conventions::validate_boundary_entry_plan(
            foreign.boundary_entry_plan.clone(),
            &signature,
        )
        .map_err(|_| "normalized foreign D41 child contains an invalid boundary entry plan")?,
    };
    let boundary_plan_identity = validated_plan.contract_commitment_digest();
    if validated_plan.plan() != &foreign.boundary_entry_plan
        || foreign.locator.target().native_target() != target
        || foreign.same_stack_contribution.requirement_identity() != requirement_identity
        || foreign
            .same_stack_contribution
            .provider_plan_report_identity()
            != selected_plan.report_identity()
        || foreign
            .same_stack_contribution
            .provider_plan_commitment()
            .as_bytes()
            != *selected_plan.plan_digest().as_bytes()
    {
        return Err(
            "normalized foreign D41 child changed its locator, boundary plan, or same-stack admission",
        );
    }

    let execution_record = foreign.provider_execution;
    if execution_record.provider_plan_report_identity != selected_plan.report_identity() {
        return Err("normalized foreign D41 child names the wrong selected provider plan");
    }
    let matching_executions = provider_executions
        .iter()
        .filter(|execution| {
            execution.requirement_identity() == requirement_identity
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
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("normalized foreign D41 child names an absent object function")?;
    let matching_attributions = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == occurrence.machine()
                && attribution.attribution.site
                    == omega_machine_code::SemanticCodeSite::Operation(occurrence.operation())
                && attribution.attribution.operation_ordinal == occurrence.operation_ordinal()
        })
        .collect::<Vec<_>>();
    let [attribution] = matching_attributions.as_slice() else {
        return Err("normalized foreign D41 child does not rejoin one emitted operation interval");
    };
    let code_offset = attribution.attribution.code_offset;
    let byte_count = attribution.attribution.byte_count;
    let object_offset = attribution.text_offset;
    if byte_count == 0
        || function
            .text_offset
            .checked_add(code_offset)
            .filter(|offset| *offset == object_offset)
            .is_none()
    {
        return Err("normalized foreign D41 child has a detached emitted operation interval");
    }
    let object_end = object_offset
        .checked_add(byte_count)
        .ok_or("normalized foreign D41 object end overflow")?;
    let expected_kind = match target.architecture {
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
    };
    let matching_imports = object
        .object()
        .layout
        .normalized_imports
        .iter()
        .filter(|import| import.locator == foreign.locator)
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
                && ranges_overlap(
                    object_offset,
                    object_end,
                    relocation.offset,
                    relocation.offset.saturating_add(relocation.byte_width),
                )
        })
        .collect::<Vec<_>>();
    let expected_origin = RelocationOrigin::SemanticOperation {
        function_symbol_handle: function.symbol,
        operation_identity: occurrence.operation().get(),
    };
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
    let callback_relocations = match callback {
        None => None,
        Some(callback) => {
            let (callback_symbol, _) = omega_object_file::object_function_symbol(
                object.object(),
                callback.target.callback_function,
            )
            .ok_or("normalized foreign D41 callback lost its private object symbol")?;
            let callback_end = callback
                .code_offset
                .checked_add(callback.byte_count)
                .ok_or("normalized foreign D41 callback materialization span overflow")?;
            if callback.code_offset < object_offset || callback_end > object_end {
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
            Some(match callback.encoding {
                omega_machine_code::CallbackAddressEncoding::X86_64Relative32 {
                    relocation_offset,
                } => NormalizedForeignCallbackRelocations::X86_64Relative32 {
                    callback_function: callback.target.callback_function,
                    relocation: exact_callback_relocation(
                        relocation_offset,
                        RelocationKind::X86_64Relative32,
                    )?,
                },
                omega_machine_code::CallbackAddressEncoding::Aarch64PageAddress {
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
            })
        }
    };
    let expected_relocation_count = 1 + match callback_relocations {
        None => 0,
        Some(NormalizedForeignCallbackRelocations::X86_64Relative32 { .. }) => 1,
        Some(NormalizedForeignCallbackRelocations::Aarch64PageAddress { .. }) => 2,
    };
    if overlapping_relocations.len() != expected_relocation_count
        || overlapping_relocations.iter().any(|relocation| {
            relocation.offset < object_offset
                || relocation
                    .offset
                    .checked_add(relocation.byte_width)
                    .is_none_or(|end| end > object_end)
        })
    {
        return Err("normalized foreign D41 child contains an unowned or out-of-span relocation");
    }

    let machine_span = native_byte_span(code_offset, byte_count);
    let object_span = native_byte_span(object_offset, byte_count);
    let final_image_span = object_span;
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, final_image_span)?;
    if machine_bytes != object_bytes {
        return Err("normalized foreign D41 child changed before object custody");
    }
    let mutable_intervals = overlapping_relocations
        .iter()
        .map(|relocation| {
            let start = relocation
                .offset
                .checked_sub(object_offset)
                .ok_or("normalized foreign D41 relocation precedes its operation span")?;
            let end = start
                .checked_add(relocation.byte_width)
                .ok_or("normalized foreign D41 relocation span overflow")?;
            (end <= byte_count)
                .then_some((start, end))
                .ok_or("normalized foreign D41 relocation exceeds its operation span")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if object_bytes
        .iter()
        .zip(final_image_bytes)
        .enumerate()
        .any(|(index, (before, after))| {
            before != after
                && !mutable_intervals
                    .iter()
                    .any(|(start, end)| index >= *start && index < *end)
        })
    {
        return Err("normalized foreign D41 bytes changed outside its exact relocation set");
    }

    let realization = NormalizedForeignCallBinding {
        locator: foreign.locator.clone(),
        boundary_entry_plan: foreign.boundary_entry_plan.clone(),
        same_stack_contribution: foreign.same_stack_contribution.clone(),
    };
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
        &foreign.locator,
        foreign.same_stack_contribution.commitment().as_bytes(),
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
    let relocation = PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(
        normalized_foreign_call_relocation(
            foreign.locator.identity_digest().as_bytes(),
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
    );
    let machine_bytes_digest = sha256(machine_bytes);
    let object_bytes_digest = sha256(object_bytes);
    let final_image_bytes_digest = sha256(final_image_bytes);
    let physical_occurrence = NativePhysicalOccurrence::Boundary(occurrence.identity());
    let identity = physical_child_identity(
        &parent,
        projection,
        physical_occurrence,
        machine_span,
        object_span,
        final_image_span,
        machine_bytes_digest,
        object_bytes_digest,
        final_image_bytes_digest,
        relocation,
    );
    Ok(Some(
        NativePhysicalChildParts {
            parent,
            projection,
            occurrence: physical_occurrence,
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

fn fixed_integer_shape(scalar_type: ScalarType) -> Option<omega_calling_conventions::ValueShape> {
    let ScalarType::Integer(integer) = scalar_type else {
        return None;
    };
    let bits = integer.bits();
    if integer.carrier() != psi_core::IntegerCarrier::Fixed || !matches!(bits, 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = bits / 8;
    Some(omega_calling_conventions::ValueShape::integer(bytes, bytes))
}

fn builtin_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    scalar_argument: &omega_target_operations::BoundaryScalarArgument,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([1]); // LinuxElfV1 compiler-builtin catalog.
    digest.update([1, 1]); // CompilerBuiltin::LinuxExitGroupI32 + realization.
    digest.update(scalar_argument.source_value.get().to_le_bytes());
    digest.update([1]); // exact signed i32 scalar schema
    let psi_core::IntegerValue::Signed(value) = scalar_argument.immediate else {
        unreachable!("D41 settlement shape was checked")
    };
    digest.update(i32::try_from(value).expect("checked i32").to_le_bytes());
    digest.finalize().into()
}

fn builtin_runtime_scalar_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    scalar_argument: &omega_machine_code::ForeignCallScalarArgumentRecord,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([1, 2, 2]);
    digest.update(scalar_argument.parameter_index.to_le_bytes());
    match scalar_argument.source {
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.update([0]);
            digest.update(parameter_index.to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
        }
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.update([1]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            match value {
                psi_core::IntegerValue::Signed(value) => digest.update(value.to_le_bytes()),
                psi_core::IntegerValue::Unsigned(value) => digest.update(value.to_le_bytes()),
            }
        }
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.update([2]);
            digest.update(home.defining_operation.get().to_le_bytes());
            digest.update(home.source_value.get().to_le_bytes());
            digest.update(home.byte_offset.to_le_bytes());
        }
    }
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn admitted_provider_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    execution: ProviderExecutionBinding,
    boundary_plan_identity: [u8; 32],
    locator: &omega_target::NormalizedForeignLocator,
    same_stack_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([2]); // AdmittedProvider::NormalizedForeignCall.
    digest.update(
        execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.update(execution.provider_execution_report_identity().to_le_bytes());
    digest.update(
        execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(execution.normalized_root_report_identity().to_le_bytes());
    digest.update(
        execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(locator.identity_digest().as_bytes());
    digest.update(boundary_plan_identity);
    digest.update(same_stack_identity);
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn physical_child_identity(
    parent: &PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: NativePhysicalOccurrence,
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: PhysicalRelocationDisposition,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-child.sha256.v2\0");
    digest.update([parent.role_tag()]);
    digest.update(parent.identity());
    digest.update(projection.bytes());
    digest.update([occurrence.role_tag()]);
    digest.update(occurrence.identity());
    for span in [machine_span, object_span, final_image_span] {
        digest.update(canonical_usize(span.offset()));
        digest.update(canonical_usize(span.byte_count()));
    }
    digest.update(machine_bytes_digest);
    digest.update(object_bytes_digest);
    digest.update(final_image_bytes_digest);
    digest.update([match relocation {
        PhysicalRelocationDisposition::DirectInstructionBytes => 1,
        PhysicalRelocationDisposition::ResolvedInternalCall => 2,
        PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(_) => 3,
    }]);
    if let PhysicalRelocationDisposition::UnresolvedNormalizedForeignCall(relocation) = relocation {
        digest.update(relocation.locator_identity());
        digest.update(relocation.boundary_plan_identity());
        hash_object_symbol(&mut digest, relocation.object_symbol());
        hash_relocation_origin(&mut digest, relocation.origin());
        digest.update(canonical_usize(relocation.offset()));
        digest.update(canonical_usize(relocation.byte_width()));
        digest.update(relocation.addend().to_le_bytes());
        digest.update([relocation_kind_tag(relocation.kind())]);
        match relocation.callback() {
            None => digest.update([0]),
            Some(NormalizedForeignCallbackRelocations::X86_64Relative32 {
                callback_function,
                relocation,
            }) => {
                digest.update([1]);
                hash_machine_function_identity(&mut digest, callback_function);
                hash_callback_relocation(&mut digest, relocation);
            }
            Some(NormalizedForeignCallbackRelocations::Aarch64PageAddress {
                callback_function,
                page,
                page_offset,
            }) => {
                digest.update([2]);
                hash_machine_function_identity(&mut digest, callback_function);
                hash_callback_relocation(&mut digest, page);
                hash_callback_relocation(&mut digest, page_offset);
            }
        }
        digest.update(relocation.final_image_symbol_identity());
    }
    digest.finalize().into()
}

fn physical_evidence_identity(
    projection: NativeOptimizationProjectionIdentity,
    children: &[NativePhysicalChild],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-evidence.sha256.v2\0");
    digest.update(projection.bytes());
    digest.update(canonical_usize(children.len()));
    for child in children {
        digest.update(child.identity());
    }
    digest.finalize().into()
}

fn terminal_identity_bytes(terminal: psi_terminal::TerminalPsiIdentity) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&terminal.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(terminal.program_fingerprint.as_bytes());
    bytes
}

fn span(bytes: &[u8], span: NativeByteSpan) -> Result<&[u8], &'static str> {
    let end = span
        .offset()
        .checked_add(span.byte_count())
        .ok_or("native physical child byte span overflow")?;
    bytes
        .get(span.offset()..end)
        .ok_or("native physical child byte span is out of bounds")
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hash_target(digest: &mut Sha256, target: NativeTarget) {
    digest.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    digest.update(canonical_usize(target.pointer_size));
    digest.update(canonical_usize(target.pointer_alignment));
}

fn hash_object_symbol(digest: &mut Sha256, symbol: omega_object_file::ObjectSymbolHandle) {
    digest.update([u8::from(symbol.is_valid())]);
    digest.update(u64::from(symbol.arena_index()).to_le_bytes());
    digest.update(u64::from(symbol.generation()).to_le_bytes());
}

fn hash_machine_function_identity(
    digest: &mut Sha256,
    identity: omega_function_identity::MachineFunctionIdentity,
) {
    let (tag, continuation, coordinate) = if let Some(source) = identity.source_key() {
        (1, source, 0)
    } else if let Some(continuation) = identity.program_storage_entry_continuation() {
        (2, continuation, 0)
    } else {
        (
            3,
            identity.associated_source_continuation(),
            identity
                .callback_thunk_placement_index()
                .expect("machine function identity has one closed role"),
        )
    };
    digest.update([tag]);
    for symbol in [continuation.machine, continuation.state] {
        digest.update([u8::from(symbol.is_valid())]);
        digest.update(u64::from(symbol.arena_index()).to_le_bytes());
        digest.update(u64::from(symbol.generation()).to_le_bytes());
    }
    digest.update(canonical_usize(continuation.segment_index));
    digest.update(canonical_usize(coordinate));
}

fn hash_callback_relocation(digest: &mut Sha256, relocation: NormalizedForeignCallbackRelocation) {
    hash_object_symbol(digest, relocation.object_symbol());
    hash_relocation_origin(digest, relocation.origin());
    digest.update(canonical_usize(relocation.offset()));
    digest.update(canonical_usize(relocation.byte_width()));
    digest.update(relocation.addend().to_le_bytes());
    digest.update([relocation_kind_tag(relocation.kind())]);
}

fn hash_relocation_origin(digest: &mut Sha256, origin: RelocationOrigin) {
    hash_object_symbol(digest, origin.symbol_handle());
    let (tag, coordinate) = match origin {
        RelocationOrigin::Instruction {
            selected_instruction_index,
            ..
        } => (1, u64::from(selected_instruction_index)),
        RelocationOrigin::SemanticOperation {
            operation_identity, ..
        } => (2, operation_identity),
        RelocationOrigin::SemanticEdge { edge_identity, .. } => (3, edge_identity),
        RelocationOrigin::Materialization { .. } => (4, 0),
    };
    digest.update([tag]);
    digest.update(coordinate.to_le_bytes());
}

const fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native physical evidence field fits u64")
        .to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    fn physical_projection() -> NativeOptimizationProjection {
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let operator = optimized_operator_occurrence(
            terminal,
            machine,
            psi_core::OperationId::new(2).expect("operator"),
            0,
            OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"operator survivor"),
        );
        let boundary = optimized_boundary_occurrence(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("boundary operation"),
            psi_core::BoundaryMachineId::new(4).expect("boundary"),
            1,
            OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"boundary survivor"),
        );
        native_optimization_projection(
            terminal,
            vec![operator],
            vec![boundary],
            NativeOptimizationProjectionIdentity::from_canonical_bytes(b"physical projection"),
        )
    }

    fn exact_coordinates(
        projection: &NativeOptimizationProjection,
    ) -> [PhysicalChildCoordinate; 2] {
        [
            PhysicalChildCoordinate {
                projection: projection.identity(),
                occurrence: NativePhysicalOccurrence::Operator(
                    projection.operator_occurrences()[0].identity(),
                ),
                parent_role: 1,
            },
            PhysicalChildCoordinate {
                projection: projection.identity(),
                occurrence: NativePhysicalOccurrence::Boundary(
                    projection.boundary_occurrences()[0].identity(),
                ),
                parent_role: 2,
            },
        ]
    }

    #[test]
    fn equal_boundary_requirements_at_distinct_operations_have_distinct_occurrences() {
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let boundary = psi_core::BoundaryMachineId::new(2).expect("boundary");
        let first = boundary_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("first operation"),
            boundary,
            0,
        );
        let second = boundary_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(4).expect("second operation"),
            boundary,
            1,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn operator_occurrence_identity_binds_exact_terminal_operation() {
        let terminal = psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([11; 32]),
        };
        let machine = psi_core::MachineId::new(1).expect("machine");
        let first = operator_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(3).expect("first operation"),
            0,
        );
        let second = operator_occurrence_identity(
            terminal,
            machine,
            psi_core::OperationId::new(4).expect("second operation"),
            1,
        );

        assert_ne!(first, second);
    }

    #[test]
    fn physical_children_require_an_exact_survivor_bijection() {
        let projection = physical_projection();
        let [operator, boundary] = exact_coordinates(&projection);
        assert!(
            validate_exact_physical_child_coordinates(&projection, [operator, boundary]).is_ok()
        );

        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator]),
            Err("native physical evidence does not cover the exact surviving occurrence set")
        );
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator, operator, boundary]),
            Err("native physical evidence contains duplicate optimized occurrences")
        );

        let padded = PhysicalChildCoordinate {
            projection: projection.identity(),
            occurrence: NativePhysicalOccurrence::Operator(
                OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"stale occurrence"),
            ),
            parent_role: 1,
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [operator, boundary, padded]),
            Err("native physical child swapped or substituted its semantic parent role")
        );

        let detached = PhysicalChildCoordinate {
            projection: NativeOptimizationProjectionIdentity::from_canonical_bytes(
                b"detached projection",
            ),
            ..operator
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [detached, boundary]),
            Err("native physical child is detached from its optimized projection")
        );

        let role_swapped = PhysicalChildCoordinate {
            parent_role: 2,
            ..operator
        };
        assert_eq!(
            validate_exact_physical_child_coordinates(&projection, [role_swapped, boundary]),
            Err("native physical child swapped or substituted its semantic parent role")
        );
    }
}

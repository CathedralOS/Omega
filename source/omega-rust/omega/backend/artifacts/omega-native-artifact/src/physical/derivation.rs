use std::collections::BTreeMap;

use omega_machine_code::BoundaryExecutionRecord;
use omega_object_file::SectionKind;
use omega_optimization_core::{
    NativeOptimizationProjectionIdentity, OptimizedBoundaryOccurrenceIdentity,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution,
};
use psi_core::{IntegerSign, IntegerType, ScalarType};
use psi_terminal::OperationKind;
use sha2::{Digest, Sha256};

use super::model::*;
use crate::{
    NativePhysicalEvidenceScope, NativeSelectedProviderPlan, NativeSelectedProviderPlanDigest,
};

pub(crate) fn derive_physical_evidence(
    scope: NativePhysicalEvidenceScope,
    terminal_artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
    selected_provider_plans: &[NativeSelectedProviderPlan],
) -> Result<Option<NativePhysicalEvidence>, &'static str> {
    if scope != NativePhysicalEvidenceScope::UnoptimizedNoBoundaryOperatorApplications {
        return Ok(None);
    }
    let module = psi_terminal_codec::decode_module(terminal_artifact.semantic_bytes())
        .map_err(|_| "native physical evidence cannot decode Terminal semantics")?;
    if module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
        || !object.port_effects().is_empty()
        || object.x86_scalar_fma_provider().is_some()
        || !object.object().layout.normalized_imports.is_empty()
    {
        return Ok(None);
    }

    let projection = derive_identity_projection(terminal_artifact.manifest().semantic(), &module)?;
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
        let Some(installed) = settlements.get(&key) else {
            // A valid artifact may realize this Terminal occurrence through
            // a normalized foreign/callback role outside the first D32 lane.
            return Ok(None);
        };
        if !matches!(
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
        ) {
            // The artifact remains usable, but this first D32 lane must not
            // claim complete physical coverage for roles it cannot replay.
            return Ok(None);
        }
        if installed.settlement.byte_count == 0 {
            return Err("Linux exit-group physical child requires a nonempty emitted span");
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
    children.sort_by_key(|child| child.occurrence());
    if children
        .windows(2)
        .any(|pair| pair[0].occurrence() >= pair[1].occurrence())
    {
        return Err("native physical evidence contains duplicate optimized occurrences");
    }
    let identity = physical_evidence_identity(projection.identity(), &children);
    Ok(Some(native_physical_evidence(
        projection, children, identity,
    )))
}

fn derive_identity_projection(
    terminal: psi_terminal::TerminalPsiIdentity,
    module: &psi_terminal::TerminalModule,
) -> Result<NativeIdentityOptimizationProjection, &'static str> {
    let mut occurrences = Vec::new();
    for machine in &module.machines {
        let mut operation_ordinal = 0_usize;
        for block in &machine.blocks {
            for operation in &block.operations {
                if let OperationKind::BoundaryCall { boundary, .. } = operation.kind {
                    let identity = boundary_occurrence_identity(
                        terminal,
                        machine.id,
                        operation.id,
                        boundary,
                        operation_ordinal,
                    );
                    occurrences.push(optimized_boundary_occurrence(
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
    let mut canonical = terminal_identity_bytes(terminal);
    canonical.extend_from_slice(&canonical_usize(occurrences.len()));
    for occurrence in &occurrences {
        canonical.extend_from_slice(&occurrence.identity().bytes());
    }
    let identity = NativeOptimizationProjectionIdentity::from_canonical_bytes(&canonical);
    Ok(identity_projection(terminal, occurrences, identity))
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
    image: &omega_image_emission::ExecutableImage,
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
    let final_image_bytes = span(&image.output().final_text_bytes, final_image_span)?;
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

    let catalog = NativeCompilerBuiltinCatalogIdentity::LinuxElfV1;
    let execution =
        BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::LinuxExitGroupI32);
    let realization = BoundaryRealization::LinuxExitGroupI32(Default::default());
    let parent_identity = boundary_trait_settlement_identity(
        occurrence,
        requirement_identity,
        selected_plan_digest,
        target,
        catalog,
        scalar_argument,
    );
    let parent = PhysicalChildParent::BoundaryTraitSettlement(
        BoundaryTraitSettlementParts {
            occurrence: *occurrence,
            requirement_identity: requirement_identity.to_owned(),
            selected_plan_digest,
            target,
            catalog,
            execution,
            realization,
            scalar_argument: *scalar_argument,
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
        occurrence.identity(),
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
        occurrence: occurrence.identity(),
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

fn boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    catalog: NativeCompilerBuiltinCatalogIdentity,
    scalar_argument: &omega_target_operations::BoundaryScalarArgument,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v1\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([match catalog {
        NativeCompilerBuiltinCatalogIdentity::LinuxElfV1 => 1,
    }]);
    digest.update([1, 1]); // CompilerBuiltin::LinuxExitGroupI32 + realization.
    digest.update(scalar_argument.source_value.get().to_le_bytes());
    digest.update([1]); // exact signed i32 scalar schema
    let psi_core::IntegerValue::Signed(value) = scalar_argument.immediate else {
        unreachable!("D41 settlement shape was checked")
    };
    digest.update(i32::try_from(value).expect("checked i32").to_le_bytes());
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn physical_child_identity(
    parent: &PhysicalChildParent,
    projection: NativeOptimizationProjectionIdentity,
    occurrence: OptimizedBoundaryOccurrenceIdentity,
    machine_span: NativeByteSpan,
    object_span: NativeByteSpan,
    final_image_span: NativeByteSpan,
    machine_bytes_digest: [u8; 32],
    object_bytes_digest: [u8; 32],
    final_image_bytes_digest: [u8; 32],
    relocation: PhysicalRelocationDisposition,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-child.sha256.v1\0");
    digest.update([1]); // BoundaryTraitSettlement parent role.
    digest.update(parent.identity());
    digest.update(projection.bytes());
    digest.update(occurrence.bytes());
    for span in [machine_span, object_span, final_image_span] {
        digest.update(canonical_usize(span.offset()));
        digest.update(canonical_usize(span.byte_count()));
    }
    digest.update(machine_bytes_digest);
    digest.update(object_bytes_digest);
    digest.update(final_image_bytes_digest);
    digest.update([match relocation {
        PhysicalRelocationDisposition::DirectInstructionBytes => 1,
    }]);
    digest.finalize().into()
}

fn physical_evidence_identity(
    projection: NativeOptimizationProjectionIdentity,
    children: &[NativePhysicalChild],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-evidence.sha256.v1\0");
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
}

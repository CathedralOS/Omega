//! Canonical serialization of the artifact's exact identity inputs.

use super::{
    NativeArtifactIdentity, NativePhysicalEvidenceScope, NativeProviderExecution,
    NativeSelectedProviderPlan,
};
use effects::{TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity};
use sha2::{Digest, Sha256};

const NATIVE_ARTIFACT_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.sha256.v8\0";

pub(super) struct NativeArtifactIdentityFields<'a> {
    pub(super) terminal_artifact_identity: [u8; 32],
    pub(super) target: target::NativeTarget,
    pub(super) object_text_bytes: &'a [u8],
    pub(super) image_bytes: &'a [u8],
    pub(super) final_text_bytes: &'a [u8],
    pub(super) image_subsystem: Option<u16>,
    pub(super) output_file_name: &'a str,
    pub(super) output_format: &'a str,
    pub(super) output_counts: [usize; 8],
    pub(super) callback_placement_identity_report_fingerprint: u64,
    pub(super) final_image_symbol_digest: [u8; 32],
    pub(super) executable_region_inventory_digest: [u8; 32],
    pub(super) executable_region_inventory_report_fingerprint: u64,
    pub(super) compiler_text_validation_digest: Option<[u8; 32]>,
    pub(super) compiler_function_validation: Option<([u8; 32], u64)>,
    pub(super) compiler_entry_region_binding: Option<([u8; 32], u64)>,
    pub(super) compiler_entry_footprint_binding: Option<([u8; 32], u64)>,
    pub(super) selected_provider_closure_digest: [u8; 32],
    pub(super) foreign_call_custody_digest: [u8; 32],
    pub(super) selected_provider_plans: &'a [NativeSelectedProviderPlan],
    pub(super) provider_executions: &'a [NativeProviderExecution],
    pub(super) terminal_authority_policy_identity: TerminalAuthorityPolicyIdentity,
    pub(super) terminal_authority_permission_policy_identity:
        Option<TerminalAuthorityPermissionPolicyIdentity>,
    pub(super) terminal_authority_closure_review_identity: [u8; 32],
    pub(super) boundary_application_coverage_identity: Option<[u8; 32]>,
    pub(super) physical_evidence_scope: &'a NativePhysicalEvidenceScope,
    pub(super) physical_evidence_identity: Option<[u8; 32]>,
    /// Identity of the derivation-owned gap that names where scoped evidence
    /// stopped; `None` when evidence is complete or the scope admits no
    /// derivation. A gap and complete evidence never coexist, but the lanes
    /// stay distinct in the preimage rather than sharing one digest slot.
    pub(super) physical_evidence_gap_identity: Option<[u8; 32]>,
}

pub(super) fn derive_native_artifact_identity(
    fields: NativeArtifactIdentityFields<'_>,
) -> NativeArtifactIdentity {
    let mut digest = Sha256::new();
    digest.update(NATIVE_ARTIFACT_IDENTITY_DOMAIN);
    digest.update(fields.terminal_artifact_identity);
    digest.update([match fields.target.architecture {
        target::Architecture::Aarch64 => 1,
        target::Architecture::X86_64 => 2,
    }]);
    digest.update([match fields.target.object_format {
        target::ObjectFormat::Elf => 1,
        target::ObjectFormat::MachO => 2,
        target::ObjectFormat::Coff => 3,
    }]);
    digest.update(canonical_usize(fields.target.pointer_size));
    digest.update(canonical_usize(fields.target.pointer_alignment));
    hash_bytes(&mut digest, fields.object_text_bytes);
    hash_bytes(&mut digest, fields.image_bytes);
    hash_bytes(&mut digest, fields.final_text_bytes);
    match fields.image_subsystem {
        None => digest.update([0]),
        Some(subsystem) => {
            digest.update([1]);
            digest.update(subsystem.to_le_bytes());
        }
    }
    hash_bytes(&mut digest, fields.output_file_name.as_bytes());
    hash_bytes(&mut digest, fields.output_format.as_bytes());
    // `EmittedImageOutput` currently has one closed output kind. Retaining a
    // tag here makes extending that vocabulary an explicit identity change.
    digest.update([1]);
    for count in fields.output_counts {
        digest.update(canonical_usize(count));
    }
    digest.update(
        fields
            .callback_placement_identity_report_fingerprint
            .to_le_bytes(),
    );
    digest.update(fields.final_image_symbol_digest);
    digest.update(fields.executable_region_inventory_digest);
    digest.update(
        fields
            .executable_region_inventory_report_fingerprint
            .to_le_bytes(),
    );
    hash_optional_digest(&mut digest, fields.compiler_text_validation_digest);
    hash_optional_digest_and_report(&mut digest, fields.compiler_function_validation);
    hash_optional_digest_and_report(&mut digest, fields.compiler_entry_region_binding);
    hash_optional_digest_and_report(&mut digest, fields.compiler_entry_footprint_binding);
    digest.update(fields.selected_provider_closure_digest);
    digest.update(fields.foreign_call_custody_digest);
    digest.update(canonical_usize(fields.selected_provider_plans.len()));
    for plan in fields.selected_provider_plans {
        digest.update(plan.report_identity.to_le_bytes());
        digest.update(plan.plan_digest.as_bytes());
        digest.update(canonical_usize(plan.requirement_identities.len()));
        for requirement in &plan.requirement_identities {
            hash_bytes(&mut digest, requirement.as_bytes());
        }
    }
    digest.update(canonical_usize(fields.provider_executions.len()));
    for execution in fields.provider_executions {
        hash_bytes(&mut digest, execution.requirement_identity.as_bytes());
        for report_coordinate in [
            execution.provider_plan_report_identity,
            execution.provider_execution_report_identity,
            execution.provider_execution_report_fingerprint,
            execution.normalized_root_report_identity,
            execution.boundary_contract_report_fingerprint,
        ] {
            digest.update(report_coordinate.to_le_bytes());
        }
    }
    digest.update(
        fields
            .terminal_authority_policy_identity
            .version()
            .to_le_bytes(),
    );
    digest.update(fields.terminal_authority_policy_identity.commitment());
    // A missing receiver-admission axis is a distinct identity input, never an
    // alias for an empty or populated permission policy.
    match fields.terminal_authority_permission_policy_identity {
        None => digest.update([0]),
        Some(permission_policy_identity) => {
            digest.update([1]);
            digest.update(permission_policy_identity.version().to_le_bytes());
            digest.update(permission_policy_identity.commitment());
        }
    }
    digest.update(fields.terminal_authority_closure_review_identity);
    hash_optional_digest(&mut digest, fields.boundary_application_coverage_identity);
    match fields.physical_evidence_scope {
        NativePhysicalEvidenceScope::Unavailable => digest.update([0]),
        NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence => digest.update([1]),
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(scope) => {
            digest.update([2]);
            digest.update(scope.identity());
        }
    }
    hash_optional_digest(&mut digest, fields.physical_evidence_identity);
    hash_optional_digest(&mut digest, fields.physical_evidence_gap_identity);
    NativeArtifactIdentity(digest.finalize().into())
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

pub(super) fn foreign_call_custody_digest(calls: &[image_emission::ObjectForeignCall]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-artifact.foreign-call-custody.v3\0");
    digest.update(canonical_usize(calls.len()));
    for call in calls {
        digest.update(call.machine.get().to_le_bytes());
        match call.owner {
            target_operations::CallSiteOwner::Operation(operation) => {
                digest.update([1]);
                digest.update(operation.get().to_le_bytes());
                digest.update(0_u32.to_le_bytes());
            }
            target_operations::CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            } => {
                digest.update([2]);
                digest.update(edge.get().to_le_bytes());
                digest.update(action_ordinal.to_le_bytes());
            }
        }
        digest.update(canonical_usize(call.text_offset));
        digest.update(call.caller_live_bytes.to_le_bytes());
        match call.x86_floating_control {
            None => digest.update([0]),
            Some(control) => {
                digest.update([1]);
                digest.update([match control.target.architecture {
                    target::Architecture::Aarch64 => 1,
                    target::Architecture::X86_64 => 2,
                }]);
                digest.update([match control.target.object_format {
                    target::ObjectFormat::Elf => 1,
                    target::ObjectFormat::MachO => 2,
                    target::ObjectFormat::Coff => 3,
                }]);
                digest.update(canonical_usize(control.target.pointer_size));
                digest.update(canonical_usize(control.target.pointer_alignment));
                digest.update(control.saved_slot_byte_offset.to_le_bytes());
                for value in [
                    control.save_offset,
                    control.save_byte_count,
                    control.restore_offset,
                    control.restore_byte_count,
                ] {
                    digest.update(canonical_usize(value));
                }
            }
        }
        match call.aarch64_floating_control {
            None => digest.update([0]),
            Some(control) => {
                digest.update([1]);
                digest.update([match control.target.architecture {
                    target::Architecture::Aarch64 => 1,
                    target::Architecture::X86_64 => 2,
                }]);
                digest.update([match control.target.object_format {
                    target::ObjectFormat::Elf => 1,
                    target::ObjectFormat::MachO => 2,
                    target::ObjectFormat::Coff => 3,
                }]);
                digest.update(canonical_usize(control.target.pointer_size));
                digest.update(canonical_usize(control.target.pointer_alignment));
                digest.update(control.saved_slot_byte_offset.to_le_bytes());
                for value in [
                    control.save_offset,
                    control.save_byte_count,
                    control.restore_offset,
                    control.restore_byte_count,
                ] {
                    digest.update(canonical_usize(value));
                }
            }
        }
        let contribution = &call.same_stack_contribution;
        digest.update(
            contribution
                .report_identity()
                .normalized_identity()
                .to_le_bytes(),
        );
        digest.update(contribution.commitment().as_bytes());
        digest.update(contribution.provider_plan_report_identity().to_le_bytes());
        digest.update(contribution.provider_plan_commitment().as_bytes());
        hash_bytes(&mut digest, contribution.requirement_identity().as_bytes());
        digest.update(contribution.receipt().normalized_identity().to_le_bytes());
        digest.update(contribution.bytes().to_le_bytes());
        digest.update(contribution.alignment().to_le_bytes());
    }
    digest.finalize().into()
}

fn hash_optional_digest(digest: &mut Sha256, value: Option<[u8; 32]>) {
    match value {
        None => digest.update([0]),
        Some(value) => {
            digest.update([1]);
            digest.update(value);
        }
    }
}

fn hash_optional_digest_and_report(digest: &mut Sha256, value: Option<([u8; 32], u64)>) {
    match value {
        None => digest.update([0]),
        Some((strong_digest, report_fingerprint)) => {
            digest.update([1]);
            digest.update(strong_digest);
            digest.update(report_fingerprint.to_le_bytes());
        }
    }
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native artifact identity field fits u64")
        .to_le_bytes()
}

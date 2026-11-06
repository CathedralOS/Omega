//! Canonical serialization of the artifact's exact identity inputs.

use super::{
    NativeArtifactIdentity, NativePhysicalEvidenceScope, NativeProviderExecution,
    NativeSelectedProviderPlan,
};
use crate::digest_encoder::DomainDigestEncoder;
use crate::physical::{canonical_usize, hash_bytes, hash_call_site_owner};
use effects::{TerminalAuthorityPermissionPolicyIdentity, TerminalAuthorityPolicyIdentity};

// v14 retains the selected physical-arrival memory and installed CPU
// requirements independently of the receiver that must satisfy them.
const NATIVE_ARTIFACT_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.sha256.v14\0";

pub(super) struct NativeArtifactIdentityFields<'a> {
    pub(super) physical_entry_requirements: Option<&'a image_emission::PhysicalEntryRequirements>,
    pub(super) terminal_artifact_identity: [u8; 32],
    pub(super) installation_assumptions:
        Vec<&'a installation_evidence::InstallationEnvironmentAssumption>,
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

/// The compiler-attached evidence digests the artifact identity carries as
/// optional custody anchors: present only when the emitted image retained that
/// validation evidence, so both artifact flavors hash the same absence.
pub(super) fn compiler_identity_evidence(
    output: &image::EmittedImageOutput,
) -> (
    Option<[u8; 32]>,
    Option<([u8; 32], u64)>,
    Option<([u8; 32], u64)>,
    Option<([u8; 32], u64)>,
) {
    (
        output
            .compiler_text_validation
            .map(|evidence| *evidence.derivation_digest.as_bytes()),
        output.compiler_function_validation.map(|evidence| {
            (
                *evidence.evidence_digest().as_bytes(),
                evidence.evidence_report_fingerprint(),
            )
        }),
        output
            .compiler_entry_region_binding
            .as_ref()
            .map(|evidence| {
                (
                    *evidence.evidence_digest.as_bytes(),
                    evidence.evidence_report_fingerprint,
                )
            }),
        output.compiler_entry_footprint_binding.map(|evidence| {
            (
                *evidence.evidence_digest.as_bytes(),
                evidence.evidence_report_fingerprint,
            )
        }),
    )
}

pub(super) fn derive_native_artifact_identity(
    fields: NativeArtifactIdentityFields<'_>,
) -> NativeArtifactIdentity {
    let mut digest = DomainDigestEncoder::new(NATIVE_ARTIFACT_IDENTITY_DOMAIN);
    digest.raw(&fields.terminal_artifact_identity);
    hash_physical_entry_requirements(&mut digest, fields.physical_entry_requirements);
    digest.u64(fields.installation_assumptions.len() as u64);
    for row in fields.installation_assumptions {
        digest.byte(row.condition_tag());
        digest.raw(row.psi().program_fingerprint.as_bytes());
        digest.raw(&row.psi().vocabulary_marker.get().to_le_bytes());
        hash_bytes(&mut digest, row.profile().identity().as_str().as_bytes());
        digest.raw(&row.entry().get().to_le_bytes());
        digest.raw(&row.installation().get().to_le_bytes());
        match row {
            installation_evidence::InstallationEnvironmentAssumption::Bootstrap(row) => match row.interval {
                installation_evidence::BootstrapAssumptionInterval::EntryUntilInstallationPublished => digest.byte(0),
            },
            installation_evidence::InstallationEnvironmentAssumption::InterruptDelivery(row) => {
                match row.interval { installation_evidence::InterruptDeliveryInterval::InstalledScopeLifetime => digest.byte(0) }
                match row.delivery { installation_evidence::X86InterruptDeliveryProfile::ArchitecturalExceptionsAndExternalIrqs => digest.byte(0) }
            }
        }
        digest.raw(row.environment_description_sha256());
        hash_bytes(&mut digest, row.source_provenance().as_bytes());
        hash_bytes(&mut digest, row.caller_provenance().as_bytes());
    }
    // The target's canonical wire identity: architecture, object format,
    // pointer size and alignment, stated once by the target owner.
    digest.raw(&fields.target.wire_identity_bytes());
    hash_bytes(&mut digest, fields.object_text_bytes);
    hash_bytes(&mut digest, fields.image_bytes);
    hash_bytes(&mut digest, fields.final_text_bytes);
    match fields.image_subsystem {
        None => digest.raw(&[0]),
        Some(subsystem) => {
            digest.raw(&[1]);
            digest.raw(&subsystem.to_le_bytes());
        }
    }
    hash_bytes(&mut digest, fields.output_file_name.as_bytes());
    hash_bytes(&mut digest, fields.output_format.as_bytes());
    // `EmittedImageOutput` currently has one closed output kind. Retaining a
    // tag here makes extending that vocabulary an explicit identity change.
    digest.raw(&[1]);
    for count in fields.output_counts {
        digest.raw(&canonical_usize(count));
    }
    digest.raw(
        &fields
            .callback_placement_identity_report_fingerprint
            .to_le_bytes(),
    );
    digest.raw(&fields.final_image_symbol_digest);
    digest.raw(&fields.executable_region_inventory_digest);
    digest.raw(
        &fields
            .executable_region_inventory_report_fingerprint
            .to_le_bytes(),
    );
    hash_optional_digest(&mut digest, fields.compiler_text_validation_digest);
    hash_optional_digest_and_report(&mut digest, fields.compiler_function_validation);
    hash_optional_digest_and_report(&mut digest, fields.compiler_entry_region_binding);
    hash_optional_digest_and_report(&mut digest, fields.compiler_entry_footprint_binding);
    digest.raw(&fields.selected_provider_closure_digest);
    digest.raw(&fields.foreign_call_custody_digest);
    digest.raw(&canonical_usize(fields.selected_provider_plans.len()));
    for plan in fields.selected_provider_plans {
        digest.raw(&plan.report_identity.to_le_bytes());
        digest.raw(plan.plan_digest.as_bytes());
        digest.raw(&canonical_usize(plan.requirement_identities.len()));
        for requirement in &plan.requirement_identities {
            hash_bytes(&mut digest, requirement.as_bytes());
        }
        digest.raw(&canonical_usize(
            plan.external_requirement_applications.len(),
        ));
        let identities = &plan.family_realization_identities;
        for row in &plan.external_requirement_applications {
            digest.raw(&row.provider_plan_report_identity.to_le_bytes());
            digest.raw(row.provider_plan_digest.as_bytes());
            hash_identity_text(&mut digest, identities, row.selected_requirement_identity);
            hash_identity_text(&mut digest, identities, row.requirement_identity);
        }
        digest.raw(&canonical_usize(plan.family_realization_rows.len()));
        for row in &plan.family_realization_rows {
            digest.raw(&row.terminal.vocabulary_marker.get().to_le_bytes());
            digest.raw(row.terminal.program_fingerprint.as_bytes());
            hash_bytes(&mut digest, row.target.as_bytes());
            digest.raw(&row.head.get().to_le_bytes());
            digest.raw(&row.member.get().to_le_bytes());
            hash_identity_text(&mut digest, identities, row.head_identity);
            hash_identity_text(&mut digest, identities, row.member_identity);
            hash_identity_text(&mut digest, identities, row.selected_requirement_identity);
            digest.raw(&row.provider_plan_report_identity.to_le_bytes());
            digest.raw(row.provider_plan_digest.as_bytes());
        }
    }
    digest.raw(&canonical_usize(fields.provider_executions.len()));
    for execution in fields.provider_executions {
        hash_bytes(&mut digest, execution.requirement_identity.as_bytes());
        for report_coordinate in [
            execution.provider_plan_report_identity,
            execution.provider_execution_report_identity,
            execution.provider_execution_report_fingerprint,
            execution.normalized_root_report_identity,
            execution.boundary_contract_report_fingerprint,
        ] {
            digest.raw(&report_coordinate.to_le_bytes());
        }
    }
    digest.raw(
        &fields
            .terminal_authority_policy_identity
            .version()
            .to_le_bytes(),
    );
    digest.raw(&fields.terminal_authority_policy_identity.commitment());
    // A missing receiver-admission axis is a distinct identity input, never an
    // alias for an empty or populated permission policy.
    match fields.terminal_authority_permission_policy_identity {
        None => digest.raw(&[0]),
        Some(permission_policy_identity) => {
            digest.raw(&[1]);
            digest.raw(&permission_policy_identity.version().to_le_bytes());
            digest.raw(&permission_policy_identity.commitment());
        }
    }
    digest.raw(&fields.terminal_authority_closure_review_identity);
    hash_optional_digest(&mut digest, fields.boundary_application_coverage_identity);
    match fields.physical_evidence_scope {
        NativePhysicalEvidenceScope::Unavailable => digest.raw(&[0]),
        NativePhysicalEvidenceScope::UnoptimizedCompleteBoundaryEvidence => digest.raw(&[1]),
        NativePhysicalEvidenceScope::ValidatedOptimizedProjection(scope) => {
            digest.raw(&[2]);
            digest.raw(scope.identity());
        }
    }
    hash_optional_digest(&mut digest, fields.physical_evidence_identity);
    hash_optional_digest(&mut digest, fields.physical_evidence_gap_identity);
    NativeArtifactIdentity(digest.finish())
}

pub(super) fn foreign_call_custody_digest(calls: &[image_emission::ObjectForeignCall]) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"omega.native-artifact.foreign-call-custody.v4\0");
    digest.raw(&canonical_usize(calls.len()));
    for call in calls {
        digest.raw(&call.machine.get().to_le_bytes());
        hash_call_site_owner(&mut digest, &call.owner);
        digest.raw(&canonical_usize(call.text_offset));
        digest.raw(&call.caller_live_bytes.to_le_bytes());
        // MXCSR/FPCR preservation around the call: one saved slot plus the
        // exact save and restore intervals per target lane.
        for control in [call.x86_floating_control, call.aarch64_floating_control] {
            hash_floating_control(&mut digest, control);
        }
        if let Some(kernel) = call.binding.system_call() {
            digest.raw(&[2]);
            hash_bytes(&mut digest, kernel.target.target_name().as_bytes());
            digest.raw(&kernel.number.to_le_bytes());
            digest.raw(&kernel.provider_plan_report_identity.to_le_bytes());
            digest.raw(&kernel.provider_plan_commitment);
            hash_bytes(&mut digest, kernel.requirement_identity.as_bytes());
            continue;
        }
        let contribution = match &call.binding.mechanism {
            target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            } => {
                digest.raw(&[1]);
                digest.raw(&locator.identity_digest().as_bytes());
                same_stack_contribution
            }
            target_operations::NormalizedForeignCallMechanism::Component {
                locator,
                provider,
                candidate,
                application_commitment,
                same_stack_contribution,
            } => {
                digest.raw(&[4]);
                digest.raw(locator.identity_digest());
                digest.raw(&provider.vocabulary_marker.get().to_le_bytes());
                digest.raw(provider.program_fingerprint.as_bytes());
                digest.raw(&candidate.get().to_le_bytes());
                digest.raw(application_commitment);
                same_stack_contribution
            }
            target_operations::NormalizedForeignCallMechanism::Table {
                target,
                byte_offset,
                dispatch_only,
                same_stack_contribution,
            } => {
                digest.raw(&[3]);
                hash_bytes(&mut digest, target.target_name().as_bytes());
                digest.raw(&byte_offset.to_le_bytes());
                digest.raw(&[u8::from(*dispatch_only)]);
                same_stack_contribution
            }
            target_operations::NormalizedForeignCallMechanism::SystemCall(_) => continue,
        };
        digest.raw(
            &contribution
                .report_identity()
                .normalized_identity()
                .to_le_bytes(),
        );
        digest.raw(&contribution.commitment().as_bytes());
        digest.raw(&contribution.provider_plan_report_identity().to_le_bytes());
        digest.raw(&contribution.provider_plan_commitment().as_bytes());
        digest.raw(&contribution.requirement_identity().0.to_le_bytes());
        digest.raw(&contribution.receipt().normalized_identity().to_le_bytes());
        digest.raw(&contribution.bytes().to_le_bytes());
        digest.raw(&contribution.alignment().to_le_bytes());
    }
    digest.finish()
}

/// The interned text one identity index names, committed as the exact
/// length-prefixed field the plan encoder wrote; the zero coordinate renders
/// empty, which is the preimage every family-row identity fold requires.
fn hash_identity_text(
    digest: &mut DomainDigestEncoder,
    identities: &terminal_psi::TerminalIdentityTable,
    identity: terminal_psi::TerminalIdentityIndex,
) {
    hash_bytes(
        digest,
        identities.text(identity).unwrap_or_default().as_bytes(),
    );
}

fn hash_floating_control(
    digest: &mut DomainDigestEncoder,
    control: Option<machine_code::ForeignCallFloatingControlRecord>,
) {
    match control {
        None => digest.raw(&[0]),
        Some(control) => {
            digest.raw(&[1]);
            digest.raw(&control.target.wire_identity_bytes());
            digest.raw(&control.saved_slot_byte_offset.to_le_bytes());
            for value in [
                control.save_offset,
                control.save_byte_count,
                control.restore_offset,
                control.restore_byte_count,
            ] {
                digest.raw(&canonical_usize(value));
            }
        }
    }
}

fn hash_optional_digest(digest: &mut DomainDigestEncoder, value: Option<[u8; 32]>) {
    match value {
        None => digest.raw(&[0]),
        Some(value) => {
            digest.raw(&[1]);
            digest.raw(&value);
        }
    }
}

fn hash_optional_digest_and_report(
    digest: &mut DomainDigestEncoder,
    value: Option<([u8; 32], u64)>,
) {
    match value {
        None => digest.raw(&[0]),
        Some((strong_digest, report_fingerprint)) => {
            digest.raw(&[1]);
            digest.raw(&strong_digest);
            digest.raw(&report_fingerprint.to_le_bytes());
        }
    }
}

/// The rule is address-independent. The enclosing native identity commits the
/// emitted image and its layout; a receiver binds its relocated occurrence.
fn hash_physical_entry_requirements(
    digest: &mut DomainDigestEncoder,
    requirement: Option<&image_emission::PhysicalEntryRequirements>,
) {
    let Some(row) = requirement else {
        digest.byte(0);
        return;
    };
    digest.byte(1);
    // Closed semantics: mapped code/data plus retained RW backing throughout
    // the physical invocation and its installed Scopes. This is a requirement,
    // never a caller-supplied assertion that a runtime mapping exists.
    digest.byte(0);
    digest.raw(&row.psi().vocabulary_marker.get().to_le_bytes());
    digest.raw(row.psi().program_fingerprint.as_bytes());
    digest.raw(&row.target().wire_identity_bytes());
    digest.raw(&row.semantic_entry().get().to_le_bytes());
    digest.raw(&row.adapter().get().to_le_bytes());
    let contract = row.contract();
    hash_bytes(
        digest,
        contract.target_slot().owner.identity().as_str().as_bytes(),
    );
    hash_bytes(
        digest,
        contract.target_package().manifest_identity().as_bytes(),
    );
    digest.raw(contract.target_package_source_digest().as_bytes());
    hash_identity_text(
        digest,
        contract.identities(),
        contract.requirement_identity(),
    );
    digest.u64(contract.parameter_type_identities().len() as u64);
    for identity in contract.parameter_type_identities() {
        hash_identity_text(digest, contract.identities(), *identity);
    }
    hash_identity_text(
        digest,
        contract.identities(),
        contract.result_type_identity(),
    );
    let mut plan_bytes = Vec::new();
    legalized_operations::encode_call_plan(&mut plan_bytes, &contract.boundary_entry_plan().call);
    calling_conventions::encode_state_plan_identity(
        &mut plan_bytes,
        &contract.boundary_entry_plan().state,
    );
    hash_bytes(digest, &plan_bytes);
    hash_optional_digest(
        digest,
        contract
            .guaranteed_entry_stack()
            .map(|row| *row.compatibility_commitment()),
    );
    let storage = row.retained_storage();
    for value in [
        storage.retained_start_offset,
        storage.saved_state_offset,
        storage.stack_offset,
        storage.stack_byte_count,
        storage.end_offset,
    ] {
        digest.u64(value);
    }
    digest.u64(row.installed_scopes().len() as u64);
    for operation in row.installed_scopes() {
        digest.raw(&operation.get().to_le_bytes());
    }
    digest.byte(match row.installed_cpu() {
        None => 0,
        Some(image_emission::InstalledPhysicalCpuRequirement::X86LongModeCpl0WithoutCet) => 1,
    });
}

#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.

use std::collections::BTreeSet;

use omega_installation_evidence::ProviderExecutionEvidence;
use sha2::{Digest, Sha256};

const NATIVE_ARTIFACT_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.sha256.v1\0";
mod ranked_native_fuel;

pub use ranked_native_fuel::{RankedNativeFuelArtifact, RankedNativeFuelArtifactParts};

/// Collision-resistant identity of one complete, validated native artifact.
///
/// The identity binds the canonical Terminal artifact, selected native target,
/// pre-relocation object text, exact executable image, strong final-image
/// evidence, and the complete source-selected provider realization retained by
/// [`NativeArtifact`]. It is derived by this owner and cannot be supplied by a
/// caller reconstructing an artifact from parts.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeArtifactIdentity([u8; 32]);

impl NativeArtifactIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for NativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for NativeArtifactIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Collision-resistant commitment to the complete source-selected provider
/// closure carried by this source-free artifact projection.
///
/// The digest is derived by the selected-closure owner and independently
/// replayed when the standalone component candidate rejoins source policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeSelectedProviderClosureDigest([u8; 32]);

impl NativeSelectedProviderClosureDigest {
    pub const fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One selected provider plan projected into source-free native-artifact
/// reporting. Requirements are exact, canonical, strictly ordered, and
/// complete for this selected plan; the compact plan coordinate is not
/// authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelectedProviderPlan {
    report_identity: u64,
    requirement_identities: Vec<String>,
}

impl NativeSelectedProviderPlan {
    pub fn new(report_identity: u64, mut requirement_identities: Vec<String>) -> Self {
        requirement_identities.sort();
        requirement_identities.dedup();
        Self {
            report_identity,
            requirement_identities,
        }
    }

    pub const fn report_identity(&self) -> u64 {
        self.report_identity
    }

    pub fn requirement_identities(&self) -> &[String] {
        &self.requirement_identities
    }
}

/// One source-free report projection of an exact admitted provider execution
/// selected during native realization.
///
/// The exact requirement string and selected-plan requirement catalog are
/// replayed here. Compact execution/root/contract coordinates remain reports;
/// provider authority stays with the non-constructible evidence borrowed by
/// lowering and the selected-closure digest retained by the artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderExecution {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    provider_execution_report_identity: u64,
    provider_execution_report_fingerprint: u64,
    normalized_root_report_identity: u64,
    boundary_contract_report_fingerprint: u64,
}

impl NativeProviderExecution {
    pub fn from_evidence(evidence: &dyn ProviderExecutionEvidence) -> Self {
        Self {
            requirement_identity: evidence.requirement_identity().to_owned(),
            provider_plan_report_identity: evidence.provider_plan(),
            provider_execution_report_identity: evidence.provider_execution_identity(),
            provider_execution_report_fingerprint: evidence.provider_execution_fingerprint(),
            normalized_root_report_identity: evidence.normalized_root_identity(),
            boundary_contract_report_fingerprint: evidence.boundary_contract_fingerprint(),
        }
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn provider_execution_report_identity(&self) -> u64 {
        self.provider_execution_report_identity
    }

    pub const fn provider_execution_report_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    pub const fn normalized_root_report_identity(&self) -> u64 {
        self.normalized_root_report_identity
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

impl ProviderExecutionEvidence for NativeProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan(&self) -> u64 {
        self.provider_plan_report_identity
    }

    fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_report_identity
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_report_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }
}

/// Complete source-independent Terminal-Psi native realization.
#[derive(Debug)]
#[must_use = "a native artifact owns the canonical semantic and target artifact join"]
pub struct NativeArtifact {
    target: omega_target::NativeTarget,
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    object: omega_image_emission::ObjectArtifact,
    image: omega_image_emission::ExecutableImage,
    selected_provider_closure_report_identity: u64,
    selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    provider_executions: Vec<NativeProviderExecution>,
    identity: NativeArtifactIdentity,
}

#[derive(Debug)]
pub struct NativeArtifactParts {
    pub target: omega_target::NativeTarget,
    pub psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    pub object: omega_image_emission::ObjectArtifact,
    pub image: omega_image_emission::ExecutableImage,
    pub selected_provider_closure_report_identity: u64,
    pub selected_provider_closure_digest: NativeSelectedProviderClosureDigest,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
}

type ProviderExecutionReportKey = (String, u64, u64, u64, u64, u64);

fn validate_provider_execution_reports(
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    required_executions: &BTreeSet<ProviderExecutionReportKey>,
) -> Result<(), &'static str> {
    let mut prior_plan = None;
    for plan in selected_provider_plans {
        if plan.report_identity == 0
            || prior_plan.is_some_and(|prior| prior >= plan.report_identity)
        {
            return Err("native artifact selected provider plans are not canonical and unique");
        }
        prior_plan = Some(plan.report_identity);
        if plan.requirement_identities.is_empty()
            || plan
                .requirement_identities
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "native artifact selected provider requirements are not canonical and unique",
            );
        }
    }

    let mut prior_execution = None;
    let mut seen_requirements = BTreeSet::new();
    let mut reported_executions = BTreeSet::new();
    for execution in provider_executions {
        let key = (
            execution.requirement_identity(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
        );
        if prior_execution.is_some_and(|prior| prior >= key) {
            return Err("native artifact provider executions are not in canonical order");
        }
        prior_execution = Some(key);
        if !seen_requirements.insert(execution.requirement_identity()) {
            return Err("native artifact contains duplicate provider requirement executions");
        }
        let Some(plan) = selected_provider_plans
            .iter()
            .find(|plan| plan.report_identity == execution.provider_plan_report_identity())
        else {
            return Err("native artifact provider execution names an unselected plan");
        };
        if plan
            .requirement_identities
            .binary_search_by(|identity| identity.as_str().cmp(execution.requirement_identity()))
            .is_err()
        {
            return Err("native artifact provider execution is absent from its selected plan");
        }
        reported_executions.insert((
            execution.requirement_identity().to_owned(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        ));
    }
    if reported_executions != *required_executions {
        return Err("native artifact provider execution reports disagree with its image");
    }
    Ok(())
}

impl NativeArtifact {
    /// Rejoin already verified proof admission with target artifacts while
    /// replaying every source-free identity and byte relation retained here.
    pub fn from_replayed_parts(parts: NativeArtifactParts) -> Result<Self, &'static str> {
        let mut artifact = Self {
            target: parts.target,
            psi_artifact: parts.psi_artifact,
            object: parts.object,
            image: parts.image,
            selected_provider_closure_report_identity: parts
                .selected_provider_closure_report_identity,
            selected_provider_closure_digest: parts.selected_provider_closure_digest,
            selected_provider_plans: parts.selected_provider_plans,
            provider_executions: parts.provider_executions,
            identity: NativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity();
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.psi_artifact
            .validate()
            .map_err(|_| "native artifact contains an invalid canonical artifact")?;
        let semantic = self.psi_artifact.manifest().semantic();
        if self.object.psi() != semantic || self.image.psi() != semantic {
            return Err("native artifact semantic identity disagrees with its object or image");
        }
        if self.object.target() != self.target || self.image.target() != self.target {
            return Err("native artifact target disagrees with its object or image");
        }
        omega_image_emission::validate_executable_image(&self.object, &self.image)
            .map_err(|_| "native artifact image failed object-to-image replay")?;
        let module = psi_terminal_codec::decode_module(self.psi_artifact.semantic_bytes())
            .map_err(|_| "native artifact canonical semantics failed to decode")?;
        if module.entry != self.object.entry() {
            return Err("native artifact entry disagrees with canonical semantics");
        }
        if self.selected_provider_closure_report_identity == 0 {
            return Err("native artifact selected provider closure has the reserved zero identity");
        }

        let required_executions = self
            .image
            .boundary_settlements()
            .iter()
            .map(|installed| {
                let execution = installed.settlement.provider_execution;
                module
                    .boundary_machines
                    .iter()
                    .find(|boundary| boundary.id == installed.settlement.boundary)
                    .map(|boundary| {
                        (
                            boundary.identity.clone(),
                            execution.provider_plan_report_identity,
                            execution.provider_execution_report_identity,
                            execution.provider_execution_report_fingerprint,
                            execution.normalized_root_report_identity,
                            execution.boundary_contract_report_fingerprint,
                        )
                    })
            })
            .collect::<Option<BTreeSet<_>>>()
            .ok_or("native artifact image settlement names an absent boundary requirement")?;
        validate_provider_execution_reports(
            &self.selected_provider_plans,
            &self.provider_executions,
            &required_executions,
        )?;
        if self.identity != self.recomputed_identity() {
            return Err("native artifact identity disagrees with its retained authority");
        }
        Ok(())
    }

    fn recomputed_identity(&self) -> NativeArtifactIdentity {
        let output = self.image.output();
        let compiler_text_validation_digest = output
            .compiler_text_validation
            .map(|evidence| *evidence.derivation_digest.as_bytes());
        let compiler_function_validation = output.compiler_function_validation.map(|evidence| {
            (
                *evidence.evidence_digest().as_bytes(),
                evidence.evidence_report_fingerprint(),
            )
        });
        let compiler_entry_region_binding =
            output
                .compiler_entry_region_binding
                .as_ref()
                .map(|evidence| {
                    (
                        *evidence.evidence_digest.as_bytes(),
                        evidence.evidence_report_fingerprint,
                    )
                });
        let compiler_entry_footprint_binding =
            output.compiler_entry_footprint_binding.map(|evidence| {
                (
                    *evidence.evidence_digest.as_bytes(),
                    evidence.evidence_report_fingerprint,
                )
            });
        derive_native_artifact_identity(NativeArtifactIdentityFields {
            terminal_artifact_identity: *self.psi_artifact.manifest().identity().as_bytes(),
            target: self.target,
            object_text_bytes: self.object.text_bytes(),
            image_bytes: &output.bytes,
            final_text_bytes: &output.final_text_bytes,
            image_subsystem: self.image.subsystem(),
            output_file_name: &output.file_name,
            output_format: &output.format,
            output_counts: [
                output.text_bytes,
                output.data_bytes,
                output.bss_bytes,
                output.symbols,
                output.relocations,
                output.final_image_symbols,
                output.final_image_imports,
                output.final_image_relocations,
            ],
            callback_placement_identity_report_fingerprint: output
                .callback_placement_identity_report_fingerprint,
            final_image_symbol_digest: *self.image.final_image_symbol_digest().as_bytes(),
            executable_region_inventory_digest: *output
                .executable_regions
                .inventory_digest
                .as_bytes(),
            executable_region_inventory_report_fingerprint: output
                .executable_regions
                .inventory_report_fingerprint,
            compiler_text_validation_digest,
            compiler_function_validation,
            compiler_entry_region_binding,
            compiler_entry_footprint_binding,
            selected_provider_closure_digest: *self.selected_provider_closure_digest.as_bytes(),
            selected_provider_plans: &self.selected_provider_plans,
            provider_executions: &self.provider_executions,
        })
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.psi_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.psi_artifact.proof_bytes()
    }

    pub const fn psi_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.psi_artifact
    }

    pub const fn object(&self) -> &omega_image_emission::ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &omega_image_emission::ExecutableImage {
        &self.image
    }

    /// Non-authoritative compatibility/report identity of the selected
    /// provider closure.
    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.selected_provider_closure_report_identity
    }

    pub const fn selected_provider_closure_digest(&self) -> NativeSelectedProviderClosureDigest {
        self.selected_provider_closure_digest
    }

    pub const fn identity(&self) -> NativeArtifactIdentity {
        self.identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.provider_executions
    }

    pub fn into_parts(self) -> NativeArtifactParts {
        NativeArtifactParts {
            target: self.target,
            psi_artifact: self.psi_artifact,
            object: self.object,
            image: self.image,
            selected_provider_closure_report_identity: self
                .selected_provider_closure_report_identity,
            selected_provider_closure_digest: self.selected_provider_closure_digest,
            selected_provider_plans: self.selected_provider_plans,
            provider_executions: self.provider_executions,
        }
    }
}

struct NativeArtifactIdentityFields<'a> {
    terminal_artifact_identity: [u8; 32],
    target: omega_target::NativeTarget,
    object_text_bytes: &'a [u8],
    image_bytes: &'a [u8],
    final_text_bytes: &'a [u8],
    image_subsystem: Option<u16>,
    output_file_name: &'a str,
    output_format: &'a str,
    output_counts: [usize; 8],
    callback_placement_identity_report_fingerprint: u64,
    final_image_symbol_digest: [u8; 32],
    executable_region_inventory_digest: [u8; 32],
    executable_region_inventory_report_fingerprint: u64,
    compiler_text_validation_digest: Option<[u8; 32]>,
    compiler_function_validation: Option<([u8; 32], u64)>,
    compiler_entry_region_binding: Option<([u8; 32], u64)>,
    compiler_entry_footprint_binding: Option<([u8; 32], u64)>,
    selected_provider_closure_digest: [u8; 32],
    selected_provider_plans: &'a [NativeSelectedProviderPlan],
    provider_executions: &'a [NativeProviderExecution],
}

fn derive_native_artifact_identity(
    fields: NativeArtifactIdentityFields<'_>,
) -> NativeArtifactIdentity {
    let mut digest = Sha256::new();
    digest.update(NATIVE_ARTIFACT_IDENTITY_DOMAIN);
    digest.update(fields.terminal_artifact_identity);
    digest.update([match fields.target.architecture {
        omega_target::Architecture::Aarch64 => 1,
        omega_target::Architecture::X86_64 => 2,
    }]);
    digest.update([match fields.target.object_format {
        omega_target::ObjectFormat::Elf => 1,
        omega_target::ObjectFormat::MachO => 2,
        omega_target::ObjectFormat::Coff => 3,
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
    digest.update(canonical_usize(fields.selected_provider_plans.len()));
    for plan in fields.selected_provider_plans {
        digest.update(plan.report_identity.to_le_bytes());
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
    NativeArtifactIdentity(digest.finalize().into())
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
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

#[cfg(test)]
mod tests {
    use super::*;

    struct IdentityFixture<'a> {
        terminal_marker: u8,
        target: omega_target::NativeTarget,
        object_text: &'a [u8],
        image_bytes: &'a [u8],
        file_name: &'a str,
        callback_fingerprint: u64,
        inventory_marker: u8,
        provider_closure_marker: u8,
        requirement: &'a str,
        execution_fingerprint: u64,
        with_validation_evidence: bool,
    }

    impl Default for IdentityFixture<'static> {
        fn default() -> Self {
            Self {
                terminal_marker: 1,
                target: omega_target::NativeTarget::linux_x64(),
                object_text: b"object text",
                image_bytes: b"executable image",
                file_name: "omega-program",
                callback_fingerprint: 29,
                inventory_marker: 2,
                provider_closure_marker: 3,
                requirement: "core::Console::write",
                execution_fingerprint: 41,
                with_validation_evidence: true,
            }
        }
    }

    fn fixture_identity(fixture: IdentityFixture<'_>) -> NativeArtifactIdentity {
        let plans = vec![NativeSelectedProviderPlan::new(
            7,
            vec![fixture.requirement.to_owned()],
        )];
        let executions = vec![NativeProviderExecution {
            requirement_identity: fixture.requirement.to_owned(),
            provider_plan_report_identity: 7,
            provider_execution_report_identity: 11,
            provider_execution_report_fingerprint: fixture.execution_fingerprint,
            normalized_root_report_identity: 17,
            boundary_contract_report_fingerprint: 19,
        }];
        let evidence = fixture.with_validation_evidence.then_some(([5; 32], 43));
        derive_native_artifact_identity(NativeArtifactIdentityFields {
            terminal_artifact_identity: [fixture.terminal_marker; 32],
            target: fixture.target,
            object_text_bytes: fixture.object_text,
            image_bytes: fixture.image_bytes,
            final_text_bytes: b"final text",
            image_subsystem: None,
            output_file_name: fixture.file_name,
            output_format: "elf",
            output_counts: [13, 17, 19, 23, 29, 31, 37, 41],
            callback_placement_identity_report_fingerprint: fixture.callback_fingerprint,
            final_image_symbol_digest: [47; 32],
            executable_region_inventory_digest: [fixture.inventory_marker; 32],
            executable_region_inventory_report_fingerprint: 53,
            compiler_text_validation_digest: evidence.map(|(digest, _)| digest),
            compiler_function_validation: evidence,
            compiler_entry_region_binding: evidence,
            compiler_entry_footprint_binding: evidence,
            selected_provider_closure_digest: [fixture.provider_closure_marker; 32],
            selected_provider_plans: &plans,
            provider_executions: &executions,
        })
    }

    #[test]
    fn compact_equal_execution_cannot_substitute_an_exact_requirement() {
        let selected = vec![NativeSelectedProviderPlan::new(
            7,
            vec!["core::Expected".to_owned()],
        )];
        let required = BTreeSet::from([("core::Expected".to_owned(), 7, 11, 13, 17, 19)]);
        let substituted = vec![NativeProviderExecution {
            requirement_identity: "core::Substitute".to_owned(),
            provider_plan_report_identity: 7,
            provider_execution_report_identity: 11,
            provider_execution_report_fingerprint: 13,
            normalized_root_report_identity: 17,
            boundary_contract_report_fingerprint: 19,
        }];

        assert_eq!(
            validate_provider_execution_reports(&selected, &substituted, &required),
            Err("native artifact provider execution is absent from its selected plan"),
        );
    }

    #[test]
    fn native_artifact_identity_is_stable_and_canonical() {
        let first = fixture_identity(IdentityFixture::default());
        let replay = fixture_identity(IdentityFixture::default());

        assert_eq!(first, replay);
        assert_eq!(first.to_string().len(), 64);
        assert_eq!(format!("{first:?}"), first.to_string());
    }

    #[test]
    fn native_artifact_identity_binds_terminal_target_and_exact_native_bytes() {
        let baseline = fixture_identity(IdentityFixture::default());
        for mutation in [
            fixture_identity(IdentityFixture {
                terminal_marker: 2,
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                target: omega_target::NativeTarget::macos_arm64(),
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                object_text: b"changed object text",
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                image_bytes: b"changed executable image",
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                file_name: "renamed-program",
                ..IdentityFixture::default()
            }),
        ] {
            assert_ne!(mutation, baseline);
        }
    }

    #[test]
    fn native_artifact_identity_binds_evidence_and_provider_realization() {
        let baseline = fixture_identity(IdentityFixture::default());
        for mutation in [
            fixture_identity(IdentityFixture {
                callback_fingerprint: 31,
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                inventory_marker: 7,
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                provider_closure_marker: 11,
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                requirement: "core::Console::substitute",
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                execution_fingerprint: 43,
                ..IdentityFixture::default()
            }),
            fixture_identity(IdentityFixture {
                with_validation_evidence: false,
                ..IdentityFixture::default()
            }),
        ] {
            assert_ne!(mutation, baseline);
        }
    }
}

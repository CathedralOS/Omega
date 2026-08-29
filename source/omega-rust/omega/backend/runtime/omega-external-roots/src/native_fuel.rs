//! Native realization selection for one sponsor-owned logical-fuel region.
//!
//! Fixed provision is an exact maximum-work/grant proof. Dynamic metering is
//! target-owned admitted instrumentation with a private context transport and
//! an independently fixed-provisioned, suspension-free exhaustion path.
//! Hosted interpretation is an explicit fallback; freestanding installation
//! never treats unavailable native metering as executable.

use std::collections::BTreeSet;

use omega_calling_conventions::MachineRegister;
use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use omega_installation_evidence::{
    FuelAttributionEvidence, FuelAttributionSite, NativeFuelChargeEvidence,
    NativeFuelImageEvidence, NativeFuelRuntimeTextEvidence, NativeFuelTransferRuntimeEvidence,
    NativeFuelTransferRuntimeImageEvidence, NativeFuelTransferRuntimePlanProjection,
    ObjectEvidence,
};
use omega_target::{NativeTarget, TargetProfile};
use psi_layout_plans::EntryStubId;

use super::{
    ComposedFuelDemand, ExternalRootDiagnostic, ExternalRootId, Fnv1a, FuelProvisionId,
    FuelSuspensionFreeEvidence, InstalledExternalRoot, NativeFuelContextLayout,
    NativeFuelMeterPlanId, NativeFuelTargetPlanProjection, ProviderExecutionId,
    SponsorContextTransport,
};

/// Sealed fixed native provision for one exact sponsor region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedNativeFuelProvision {
    demand: ComposedFuelDemand,
    provision: FuelProvisionId,
    granted_units: u64,
}

impl FixedNativeFuelProvision {
    pub const fn provision(&self) -> FuelProvisionId {
        self.provision
    }

    pub const fn maximum_logical_work(&self) -> u64 {
        self.demand.units()
    }

    pub const fn granted_units(&self) -> u64 {
        self.granted_units
    }

    pub const fn meter_elided(&self) -> bool {
        true
    }

    fn matches(
        &self,
        demand: &ComposedFuelDemand,
        provision: FuelProvisionId,
        granted_units: u64,
    ) -> bool {
        self.demand == *demand && self.provision == provision && self.granted_units == granted_units
    }
}

/// Admit meter elision only from the exact installed maximum logical work.
pub fn admit_fixed_native_fuel(
    demand: &ComposedFuelDemand,
    provision: FuelProvisionId,
    granted_units: u64,
) -> Result<FixedNativeFuelProvision, ExternalRootDiagnostic> {
    if granted_units == 0 {
        return Err(ExternalRootDiagnostic(
            "fixed native fuel provision requires a nonzero grant".into(),
        ));
    }
    if demand.units() > granted_units {
        return Err(ExternalRootDiagnostic(format!(
            "fixed native fuel provision grants {granted_units} units, but the exact installed maximum logical work is {}",
            demand.units()
        )));
    }
    Ok(FixedNativeFuelProvision {
        demand: demand.clone(),
        provision,
        granted_units,
    })
}

/// Fixed provision plus the stronger proof required of a dynamic meter's
/// independently sponsored exhaustion path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuspensionFreeFixedFuelProvision {
    fixed: FixedNativeFuelProvision,
    suspension_free: FuelSuspensionFreeEvidence,
}

impl SuspensionFreeFixedFuelProvision {
    pub const fn fixed(&self) -> &FixedNativeFuelProvision {
        &self.fixed
    }
}

pub fn bind_suspension_free_fixed_fuel(
    fixed: FixedNativeFuelProvision,
    suspension_free: FuelSuspensionFreeEvidence,
) -> Result<SuspensionFreeFixedFuelProvision, ExternalRootDiagnostic> {
    if fixed.demand != *suspension_free.exact_demand() {
        return Err(ExternalRootDiagnostic(
            "fuel-suspension-free evidence does not name the fixed sponsor-path provision".into(),
        ));
    }
    Ok(SuspensionFreeFixedFuelProvision {
        fixed,
        suspension_free,
    })
}

/// Sealed structural target policy for charge and cold-transfer lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeFuelTargetPolicy(NativeFuelTargetPlanProjection);

impl AdmittedNativeFuelTargetPolicy {
    pub const fn projection(&self) -> &NativeFuelTargetPlanProjection {
        &self.0
    }

    pub const fn profile(&self) -> TargetProfile {
        self.0.profile
    }

    pub const fn target(&self) -> NativeTarget {
        self.0.target
    }
}

/// Validate the first concrete native-fuel transport slice. RBX and X28 are
/// nonvolatile in the supported x86-64 and AArch64 ABIs and are not ordinary
/// terminal-emitter scratches; later allocation and artifact replay must still
/// prove the selected register remains reserved throughout the instrumented
/// closure.
pub fn admit_native_fuel_target_policy(
    projection: NativeFuelTargetPlanProjection,
) -> Result<AdmittedNativeFuelTargetPolicy, ExternalRootDiagnostic> {
    if projection.profile.native_target() != projection.target {
        return Err(ExternalRootDiagnostic(
            "native fuel target policy profile does not match its native target".into(),
        ));
    }
    if projection.target.pointer_size != 8 || projection.target.pointer_alignment != 8 {
        return Err(ExternalRootDiagnostic(
            "native fuel target policy is not in the admitted 64-bit transport slice".into(),
        ));
    }
    match (projection.target.architecture, projection.transport) {
        (
            omega_target::Architecture::X86_64,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
        )
        | (
            omega_target::Architecture::Aarch64,
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::Aarch64X(28),
            },
        ) => {}
        (_, SponsorContextTransport::ReservedNonvolatileRegister { .. }) => {
            return Err(ExternalRootDiagnostic(
                "native fuel context transport requires reserved nonvolatile RBX or X28 for its target architecture"
                    .into(),
            ));
        }
    }
    validate_native_fuel_context_layout(&projection.context)?;
    if projection.transfer_plan_identity == 0 {
        return Err(ExternalRootDiagnostic(
            "native fuel target policy requires a nonzero transfer-plan identity".into(),
        ));
    }
    Ok(AdmittedNativeFuelTargetPolicy(projection))
}

fn validate_native_fuel_context_layout(
    layout: &NativeFuelContextLayout,
) -> Result<(), ExternalRootDiagnostic> {
    if layout.byte_size == 0
        || layout.alignment < 8
        || !layout.alignment.is_power_of_two()
        || !layout.byte_size.is_multiple_of(layout.alignment)
    {
        return Err(ExternalRootDiagnostic(
            "native fuel context size and alignment are not canonical".into(),
        ));
    }
    let scalar_offsets = [
        layout.remaining_units_offset,
        layout.unpaid_site_kind_offset,
        layout.unpaid_site_identity_offset,
        layout.required_units_offset,
        layout.transfer_entry_offset,
        layout.retry_code_offset_offset,
        layout.sponsor_stack_top_offset,
    ];
    let mut ranges = Vec::with_capacity(scalar_offsets.len() + 1);
    for offset in scalar_offsets {
        if !offset.is_multiple_of(8)
            || offset
                .checked_add(8)
                .is_none_or(|end| end > layout.byte_size)
        {
            return Err(ExternalRootDiagnostic(
                "native fuel context contains an unaligned or out-of-range scalar slot".into(),
            ));
        }
        ranges.push((offset, offset + 8));
    }
    if layout.activation_state_byte_count == 0
        || !layout.activation_state_offset.is_multiple_of(8)
        || layout
            .activation_state_offset
            .checked_add(layout.activation_state_byte_count)
            .is_none_or(|end| end > layout.byte_size)
    {
        return Err(ExternalRootDiagnostic(
            "native fuel activation-state interval is empty, unaligned, or out of range".into(),
        ));
    }
    ranges.push((
        layout.activation_state_offset,
        layout.activation_state_offset + layout.activation_state_byte_count,
    ));
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(ExternalRootDiagnostic(
            "native fuel context slots overlap".into(),
        ));
    }
    Ok(())
}

/// Sealed structural transfer plan for one exact admitted native-fuel target
/// policy. This validates plan identity and shape only; executable transfer
/// runtime custody remains unavailable until target emission and image replay
/// can construct [`InstalledNativeFuelTransferRuntime`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeFuelTransferPlan {
    target_policy: AdmittedNativeFuelTargetPolicy,
    projection: NativeFuelTransferRuntimePlanProjection,
}

impl ValidatedNativeFuelTransferPlan {
    pub const fn target_policy(&self) -> &AdmittedNativeFuelTargetPolicy {
        &self.target_policy
    }

    pub const fn projection(&self) -> &NativeFuelTransferRuntimePlanProjection {
        &self.projection
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.projection.normalized_identity()
    }
}

/// Seal a dependency-light runtime plan only when every target-policy field
/// and its derived normalized identity agree with the admitted charge recipe.
pub fn admit_native_fuel_transfer_plan(
    target_policy: AdmittedNativeFuelTargetPolicy,
    projection: NativeFuelTransferRuntimePlanProjection,
) -> Result<ValidatedNativeFuelTransferPlan, ExternalRootDiagnostic> {
    let policy = *target_policy.projection();
    if projection.profile() != policy.profile
        || projection.target() != policy.target
        || projection.transport() != policy.transport
        || projection.context() != policy.context
        || projection.normalized_identity() != policy.transfer_plan_identity
        || projection.validate_target_policy(policy).is_err()
    {
        return Err(ExternalRootDiagnostic(
            "native fuel transfer runtime plan does not match the exact admitted target policy"
                .into(),
        ));
    }
    Ok(ValidatedNativeFuelTransferPlan {
        target_policy,
        projection,
    })
}

/// Exact installed-code custody for the compiler-owned transfer and resume
/// entries. This binds the complete object/final text pair and the replayed
/// runtime intervals to one installed realization. It intentionally does not
/// include the separately required installed sponsor route and therefore is
/// not executable transfer authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelTransferCode {
    transfer_plan: ValidatedNativeFuelTransferPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    runtime_evidence: NativeFuelTransferRuntimeEvidence,
    sponsor_text_offset: usize,
    unrelocated_text_report_fingerprint: u64,
    final_text_report_fingerprint: u64,
    non_authoritative_report_fingerprint: u64,
}

impl InstalledNativeFuelTransferCode {
    pub const fn transfer_plan(&self) -> &ValidatedNativeFuelTransferPlan {
        &self.transfer_plan
    }

    pub const fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.psi
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn runtime_evidence(&self) -> &NativeFuelTransferRuntimeEvidence {
        &self.runtime_evidence
    }

    pub const fn sponsor_text_offset(&self) -> usize {
        self.sponsor_text_offset
    }

    pub const fn unrelocated_text_report_fingerprint(&self) -> u64 {
        self.unrelocated_text_report_fingerprint
    }

    /// Compatibility accessor for [`Self::unrelocated_text_report_fingerprint`].
    pub const fn unrelocated_text_fingerprint(&self) -> u64 {
        self.unrelocated_text_report_fingerprint()
    }

    pub const fn final_text_report_fingerprint(&self) -> u64 {
        self.final_text_report_fingerprint
    }

    /// Compatibility accessor for [`Self::final_text_report_fingerprint`].
    pub const fn final_text_fingerprint(&self) -> u64 {
        self.final_text_report_fingerprint()
    }

    /// Compact report coordinate retained for compatibility. Sponsor-route
    /// and executable-runtime authority use exact transfer-code custody.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    /// Compatibility accessor for [`Self::report_fingerprint`].
    pub const fn fingerprint(&self) -> u64 {
        self.report_fingerprint()
    }

    pub fn binds_installed_code(&self, installed_code: &InstalledCode) -> bool {
        self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Bind the complete replayed transfer-runtime image to one exact installed
/// code occurrence. The runtime spans are checked against both full text
/// coordinate spaces so a valid interval from another image cannot be joined
/// by fingerprint or compact identity alone.
pub fn bind_installed_native_fuel_transfer_code<Image: NativeFuelTransferRuntimeImageEvidence>(
    transfer_plan: ValidatedNativeFuelTransferPlan,
    image: &Image,
    installed_code: &InstalledCode,
) -> Result<InstalledNativeFuelTransferCode, ExternalRootDiagnostic> {
    let runtime_evidence = image.transfer_runtime_evidence();
    if image.target() != transfer_plan.target_policy().target()
        || runtime_evidence.plan() != transfer_plan.projection()
        || installed_code.architecture() != image.target().architecture
    {
        return Err(ExternalRootDiagnostic(
            "installed native fuel transfer image does not match its exact admitted plan and target"
                .into(),
        ));
    }
    if !installed_code.binds_exact_materialized_artifact_bytes(
        image.unrelocated_text_bytes(),
        image.final_text_bytes(),
    ) {
        return Err(ExternalRootDiagnostic(
            "installed native fuel transfer image bytes do not match the exact installed artifact"
                .into(),
        ));
    }
    if !runtime_text_matches_image(
        runtime_evidence.transfer_text(),
        image.unrelocated_text_bytes(),
        image.final_text_bytes(),
    ) || !runtime_text_matches_image(
        runtime_evidence.resume_text(),
        image.unrelocated_text_bytes(),
        image.final_text_bytes(),
    ) {
        return Err(ExternalRootDiagnostic(
            "native fuel transfer runtime intervals do not match the complete replayed image"
                .into(),
        ));
    }
    if image.sponsor_text_offset() >= image.final_text_bytes().len() {
        return Err(ExternalRootDiagnostic(
            "native fuel transfer sponsor coordinate is outside the complete replayed image".into(),
        ));
    }

    let mut unrelocated_hash = Fnv1a::new();
    unrelocated_hash.bytes(b"omega.native-fuel-transfer-unrelocated-text.v1");
    unrelocated_hash.bytes(image.unrelocated_text_bytes());
    let unrelocated_text_report_fingerprint = unrelocated_hash.finish();
    let mut final_hash = Fnv1a::new();
    final_hash.bytes(b"omega.native-fuel-transfer-final-text.v1");
    final_hash.bytes(image.final_text_bytes());
    let final_text_report_fingerprint = final_hash.finish();

    let psi = image.psi();
    let mut fingerprint = Fnv1a::new();
    fingerprint.bytes(b"omega.installed-native-fuel-transfer-code.v1");
    fingerprint.u64(transfer_plan.normalized_identity());
    fingerprint.u64(u64::from(psi.vocabulary_marker.get()));
    fingerprint.bytes(psi.program_fingerprint.as_bytes());
    fingerprint.u64(installed_code.identity().normalized_identity());
    fingerprint.u64(installed_code.artifact().normalized_identity());
    fingerprint.u64(runtime_evidence.report_fingerprint());
    fingerprint.u64(image.sponsor_text_offset() as u64);
    fingerprint.u64(unrelocated_text_report_fingerprint);
    fingerprint.u64(final_text_report_fingerprint);

    Ok(InstalledNativeFuelTransferCode {
        transfer_plan,
        psi,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        runtime_evidence: runtime_evidence.clone(),
        sponsor_text_offset: image.sponsor_text_offset(),
        unrelocated_text_report_fingerprint,
        final_text_report_fingerprint,
        non_authoritative_report_fingerprint: fingerprint.finish(),
    })
}

fn runtime_text_matches_image(
    evidence: &NativeFuelRuntimeTextEvidence,
    unrelocated_text: &[u8],
    final_text: &[u8],
) -> bool {
    let span = evidence.span();
    let Some(end) = span.text_offset.checked_add(span.byte_count) else {
        return false;
    };
    unrelocated_text
        .get(span.text_offset..end)
        .is_some_and(|bytes| bytes == evidence.unrelocated_bytes())
        && final_text
            .get(span.text_offset..end)
            .is_some_and(|bytes| bytes == evidence.final_bytes())
}

/// Exact transfer-code evidence retained by an installed sponsor route. The
/// installed occurrence context carries the complete admitted artifact bytes;
/// the remaining fields retain the exact transfer plan, Psi, runtime rows, and
/// sponsor coordinate. Compact report fingerprints are deliberately excluded.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledNativeFuelTransferCodeCustody {
    transfer_plan: ValidatedNativeFuelTransferPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    runtime_evidence: NativeFuelTransferRuntimeEvidence,
    sponsor_text_offset: usize,
}

impl InstalledNativeFuelTransferCodeCustody {
    fn from_transfer_code(transfer_code: &InstalledNativeFuelTransferCode) -> Self {
        Self {
            transfer_plan: transfer_code.transfer_plan.clone(),
            psi: transfer_code.psi,
            installed_code: transfer_code.installed_code,
            installed_code_context: transfer_code.installed_code_context.clone(),
            artifact: transfer_code.artifact,
            runtime_evidence: transfer_code.runtime_evidence.clone(),
            sponsor_text_offset: transfer_code.sponsor_text_offset,
        }
    }

    fn binds(&self, transfer_code: &InstalledNativeFuelTransferCode) -> bool {
        self.transfer_plan == transfer_code.transfer_plan
            && self.psi == transfer_code.psi
            && self.installed_code == transfer_code.installed_code
            && self.installed_code_context == transfer_code.installed_code_context
            && self.artifact == transfer_code.artifact
            && self.runtime_evidence == transfer_code.runtime_evidence
            && self.sponsor_text_offset == transfer_code.sponsor_text_offset
    }
}

/// Installed provider route reached by the compiler-owned transfer stub.
/// This retains the exact fixed, suspension-free sponsor provision separately
/// from transfer-code custody and binds the relocation's sponsor coordinate to
/// the selected entry of one installed external root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelSponsorRoute {
    sponsor_path: SuspensionFreeFixedFuelProvision,
    transfer_code_custody: InstalledNativeFuelTransferCodeCustody,
    transfer_code_report_fingerprint: u64,
    root: ExternalRootId,
    provider_execution: ProviderExecutionId,
    provider_execution_report_fingerprint: u64,
    entry: EntryStubId,
    sponsor_text_offset: usize,
    non_authoritative_report_fingerprint: u64,
}

impl InstalledNativeFuelSponsorRoute {
    pub const fn sponsor_path(&self) -> &SuspensionFreeFixedFuelProvision {
        &self.sponsor_path
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn provider_execution(&self) -> ProviderExecutionId {
        self.provider_execution
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn sponsor_text_offset(&self) -> usize {
        self.sponsor_text_offset
    }

    pub const fn transfer_code_report_fingerprint(&self) -> u64 {
        self.transfer_code_report_fingerprint
    }

    pub const fn provider_execution_report_fingerprint(&self) -> u64 {
        self.provider_execution_report_fingerprint
    }

    /// Compact report coordinate retained for compatibility. Runtime authority
    /// uses the exact transfer-code custody held by this route.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    /// Compatibility accessor for [`Self::report_fingerprint`].
    pub const fn fingerprint(&self) -> u64 {
        self.report_fingerprint()
    }

    fn binds_transfer_code(&self, transfer_code: &InstalledNativeFuelTransferCode) -> bool {
        self.transfer_code_custody.binds(transfer_code)
    }
}

/// Join the transfer image's resolved sponsor coordinate to one exact
/// installed provider entry whose native-fuel column is the same fixed
/// provision retained by the independently derived suspension-free proof.
pub fn bind_installed_native_fuel_sponsor_route(
    sponsor_path: SuspensionFreeFixedFuelProvision,
    transfer_code: &InstalledNativeFuelTransferCode,
    sponsor_root: &InstalledExternalRoot<'_>,
) -> Result<InstalledNativeFuelSponsorRoute, ExternalRootDiagnostic> {
    let installed_code = sponsor_root.installed_code;
    let provider_execution = &sponsor_root.evidence.provider_execution;
    let entry = provider_execution.selected_entry();
    if !transfer_code.binds_installed_code(installed_code)
        || sponsor_root.evidence.installed_code != installed_code.receipt_context()
        || provider_execution
            .validate_installed_entry_binding(installed_code)
            .is_err()
    {
        return Err(ExternalRootDiagnostic(
            "native fuel sponsor route does not bind the transfer image's exact installed code and provider execution"
                .into(),
        ));
    }
    if !installed_code.binds_entry_offset(entry, transfer_code.sponsor_text_offset as u64) {
        return Err(ExternalRootDiagnostic(
            "native fuel transfer sponsor coordinate does not name the selected installed root entry"
                .into(),
        ));
    }
    if sponsor_root.evidence.native_fuel.kind() != NativeFuelRealizationKind::FixedProvision
        || !sponsor_root.evidence.native_fuel.matches(
            &sponsor_path.fixed.demand,
            sponsor_path.fixed.provision,
            sponsor_path.fixed.granted_units,
            installed_code,
        )
    {
        return Err(ExternalRootDiagnostic(
            "native fuel sponsor route is not the exact installed fixed provision retained by its suspension-free proof"
                .into(),
        ));
    }

    let mut fingerprint = Fnv1a::new();
    fingerprint.bytes(b"omega.installed-native-fuel-sponsor-route.v1");
    fingerprint.u64(transfer_code.report_fingerprint());
    fingerprint.u64(sponsor_root.root.normalized_identity());
    fingerprint.u64(provider_execution.identity().normalized_identity());
    fingerprint.u64(provider_execution.normalized_report_identity());
    fingerprint.u64(entry.normalized_identity());
    fingerprint.u64(transfer_code.sponsor_text_offset as u64);
    fingerprint.u64(sponsor_path.fixed.demand.composition_fingerprint());
    fingerprint.u64(sponsor_path.fixed.provision.normalized_identity());
    fingerprint.u64(sponsor_path.fixed.granted_units);
    fingerprint.u64(sponsor_path.suspension_free.composition_fingerprint());
    fingerprint.u64(sponsor_root.evidence.root.normalized_report_identity());
    fingerprint.u64(
        sponsor_root
            .evidence
            .root
            .boundary_contract_report_fingerprint(),
    );

    Ok(InstalledNativeFuelSponsorRoute {
        sponsor_path,
        transfer_code_custody: InstalledNativeFuelTransferCodeCustody::from_transfer_code(
            transfer_code,
        ),
        transfer_code_report_fingerprint: transfer_code.report_fingerprint(),
        root: sponsor_root.root,
        provider_execution: provider_execution.identity(),
        provider_execution_report_fingerprint: provider_execution.normalized_report_identity(),
        entry,
        sponsor_text_offset: transfer_code.sponsor_text_offset,
        non_authoritative_report_fingerprint: fingerprint.finish(),
    })
}

/// Pre-install dynamic meter selection. Structural transfer-plan admission is
/// necessary but not installed execution authority; the final executable
/// runtime value deliberately still has no public constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicNativeFuelMeterPlan {
    transfer_plan: ValidatedNativeFuelTransferPlan,
    schedule: psi_core::FuelScheduleIdentity,
    meter: NativeFuelMeterPlanId,
    sponsor_path: SuspensionFreeFixedFuelProvision,
}

impl DynamicNativeFuelMeterPlan {
    pub fn from_admitted_transfer_plan(
        transfer_plan: ValidatedNativeFuelTransferPlan,
        schedule: psi_core::FuelScheduleIdentity,
        meter: NativeFuelMeterPlanId,
        sponsor_path: SuspensionFreeFixedFuelProvision,
    ) -> Self {
        Self {
            transfer_plan,
            schedule,
            meter,
            sponsor_path,
        }
    }

    pub const fn target(&self) -> NativeTarget {
        self.transfer_plan.target_policy.target()
    }

    pub const fn target_policy(&self) -> &AdmittedNativeFuelTargetPolicy {
        &self.transfer_plan.target_policy
    }

    pub const fn transfer_plan(&self) -> &ValidatedNativeFuelTransferPlan {
        &self.transfer_plan
    }

    pub const fn sponsor_path(&self) -> &SuspensionFreeFixedFuelProvision {
        &self.sponsor_path
    }
}

/// Installed custody for the executable exhaustion-transfer runtime. This
/// is the join of independently sealed transfer-code bytes and an installed
/// fixed, suspension-free sponsor route. Its presence in the final realization
/// makes opaque validation receipt identifiers insufficient to admit dynamic
/// execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelTransferRuntime {
    plan: DynamicNativeFuelMeterPlan,
    transfer_code: InstalledNativeFuelTransferCode,
    sponsor_route: InstalledNativeFuelSponsorRoute,
    non_authoritative_report_fingerprint: u64,
}

impl InstalledNativeFuelTransferRuntime {
    pub const fn transfer_code(&self) -> &InstalledNativeFuelTransferCode {
        &self.transfer_code
    }

    pub const fn sponsor_route(&self) -> &InstalledNativeFuelSponsorRoute {
        &self.sponsor_route
    }

    /// Compact report coordinate retained for compatibility. Exact runtime
    /// authority is the plan/code/route evidence held by this carrier.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    /// Compatibility accessor for [`Self::report_fingerprint`].
    pub const fn fingerprint(&self) -> u64 {
        self.report_fingerprint()
    }

    fn matches(&self, plan: &DynamicNativeFuelMeterPlan, installed_code: &InstalledCode) -> bool {
        self.plan == *plan
            && self.transfer_code.binds_installed_code(installed_code)
            && self.sponsor_route.binds_transfer_code(&self.transfer_code)
            && self.sponsor_route.sponsor_path == plan.sponsor_path
    }
}

/// Construct executable transfer custody only after both independent joins
/// agree with the exact dynamic plan. Neither installed bytes nor an installed
/// fixed root can manufacture this value alone.
pub fn bind_installed_native_fuel_transfer_runtime(
    plan: DynamicNativeFuelMeterPlan,
    transfer_code: InstalledNativeFuelTransferCode,
    sponsor_route: InstalledNativeFuelSponsorRoute,
) -> Result<InstalledNativeFuelTransferRuntime, ExternalRootDiagnostic> {
    if transfer_code.transfer_plan != plan.transfer_plan {
        return Err(ExternalRootDiagnostic(
            "installed native fuel transfer code does not match the dynamic plan's exact transfer projection"
                .into(),
        ));
    }
    if sponsor_route.sponsor_path != plan.sponsor_path
        || !sponsor_route.binds_transfer_code(&transfer_code)
    {
        return Err(ExternalRootDiagnostic(
            "installed native fuel sponsor route does not match the dynamic plan and exact transfer code"
                .into(),
        ));
    }

    let mut fingerprint = Fnv1a::new();
    fingerprint.bytes(b"omega.installed-native-fuel-transfer-runtime.v1");
    fingerprint.u64(transfer_code.report_fingerprint());
    fingerprint.u64(sponsor_route.report_fingerprint());
    fingerprint.u64(plan.transfer_plan.normalized_identity());
    fingerprint.u64(plan.sponsor_path.fixed.demand.composition_fingerprint());
    fingerprint.u64(plan.sponsor_path.fixed.provision.normalized_identity());
    fingerprint.u64(plan.sponsor_path.suspension_free.composition_fingerprint());

    Ok(InstalledNativeFuelTransferRuntime {
        plan,
        transfer_code,
        sponsor_route,
        non_authoritative_report_fingerprint: fingerprint.finish(),
    })
}

/// Validated pre-install input to target instrumentation. This owns the exact
/// semantic rows and source bytes; it is deliberately not installed execution
/// evidence because charge insertion changes both bytes and offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDynamicFuelAttributionBasis {
    plan: DynamicNativeFuelMeterPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    attributions: Vec<FuelAttributionEvidence>,
    fingerprint: u64,
}

impl ValidatedDynamicFuelAttributionBasis {
    pub const fn plan(&self) -> &DynamicNativeFuelMeterPlan {
        &self.plan
    }

    pub fn attributions(&self) -> &[FuelAttributionEvidence] {
        &self.attributions
    }

    pub const fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.psi
    }

    pub const fn source_text_fingerprint(&self) -> u64 {
        self.source_text_fingerprint
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

/// Final dynamic-meter evidence constructed only by joining the validated
/// source basis, independently replayed metered/final image, and exact
/// installed-code realization. An unmetered artifact cannot manufacture this
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDynamicFuelAttributionPlan {
    plan: DynamicNativeFuelMeterPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    basis_fingerprint: u64,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    attributions: Vec<FuelAttributionEvidence>,
    charges: Vec<NativeFuelChargeEvidence>,
    final_text_fingerprint: u64,
    fingerprint: u64,
}

impl InstalledDynamicFuelAttributionPlan {
    pub const fn plan(&self) -> &DynamicNativeFuelMeterPlan {
        &self.plan
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn source_text_fingerprint(&self) -> u64 {
        self.source_text_fingerprint
    }

    pub const fn basis_fingerprint(&self) -> u64 {
        self.basis_fingerprint
    }

    pub fn attributions(&self) -> &[FuelAttributionEvidence] {
        &self.attributions
    }

    pub fn charges(&self) -> &[NativeFuelChargeEvidence] {
        &self.charges
    }

    pub const fn final_text_fingerprint(&self) -> u64 {
        self.final_text_fingerprint
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn matches_installed_code(&self, installed_code: &InstalledCode) -> bool {
        self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Bind independently replayed metered and final text to one exact installed
/// realization. Source attribution, target recipe, every charge/cold interval,
/// and both sides of relocation must agree before installed dynamic evidence
/// can exist.
pub fn bind_installed_dynamic_fuel_attribution<Image: NativeFuelImageEvidence>(
    basis: ValidatedDynamicFuelAttributionBasis,
    image: &Image,
    installed_code: &InstalledCode,
) -> Result<InstalledDynamicFuelAttributionPlan, ExternalRootDiagnostic> {
    let (charges, final_text_fingerprint) = validate_dynamic_fuel_image(
        &basis.plan,
        basis.psi,
        basis.source_text_fingerprint,
        &basis.attributions,
        image,
        installed_code,
    )?;
    let fingerprint = fingerprint_installed_dynamic_fuel(
        basis.fingerprint,
        installed_code,
        final_text_fingerprint,
        &charges,
    );
    Ok(InstalledDynamicFuelAttributionPlan {
        plan: basis.plan,
        psi: basis.psi,
        source_text_fingerprint: basis.source_text_fingerprint,
        basis_fingerprint: basis.fingerprint,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        attributions: basis.attributions,
        charges,
        final_text_fingerprint,
        fingerprint,
    })
}

/// Independently replay a sealed installed dynamic-meter plan against the
/// exact source/metered/final image and installed-code occurrence.
///
/// This check grants no meter insertion, transfer, root, or publication
/// authority. It only proves that the retained attribution catalog and charge
/// intervals are still the values admitted when the carrier was constructed.
pub fn validate_installed_dynamic_fuel_attribution<Image: NativeFuelImageEvidence>(
    binding: &InstalledDynamicFuelAttributionPlan,
    image: &Image,
    installed_code: &InstalledCode,
) -> Result<(), ExternalRootDiagnostic> {
    if !binding.matches_installed_code(installed_code) {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel attribution does not bind the exact installed-code occurrence"
                .into(),
        ));
    }
    let expected_basis_fingerprint = fingerprint_dynamic_fuel_attribution_basis(
        &binding.plan,
        binding.psi,
        binding.source_text_fingerprint,
        &binding.attributions,
    );
    if binding.basis_fingerprint != expected_basis_fingerprint {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel attribution basis fingerprint drifted".into(),
        ));
    }
    let (charges, final_text_fingerprint) = validate_dynamic_fuel_image(
        &binding.plan,
        binding.psi,
        binding.source_text_fingerprint,
        &binding.attributions,
        image,
        installed_code,
    )?;
    if binding.charges != charges || binding.final_text_fingerprint != final_text_fingerprint {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel charge or final-text evidence drifted".into(),
        ));
    }
    let expected_fingerprint = fingerprint_installed_dynamic_fuel(
        binding.basis_fingerprint,
        installed_code,
        final_text_fingerprint,
        &charges,
    );
    if binding.fingerprint != expected_fingerprint {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel attribution fingerprint drifted".into(),
        ));
    }
    Ok(())
}

fn validate_dynamic_fuel_image<Image: NativeFuelImageEvidence>(
    plan: &DynamicNativeFuelMeterPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    attributions: &[FuelAttributionEvidence],
    image: &Image,
    installed_code: &InstalledCode,
) -> Result<(Vec<NativeFuelChargeEvidence>, u64), ExternalRootDiagnostic> {
    if image.psi() != psi
        || image.target() != plan.target()
        || image.target_policy() != *plan.target_policy().projection()
        || installed_code.architecture() != image.target().architecture
    {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel image does not match its semantic identity or admitted target recipe"
                .into(),
        ));
    }
    let mut source_hash = Fnv1a::new();
    source_hash.bytes(b"omega.dynamic-fuel-source-text.v1");
    source_hash.bytes(image.source_text_bytes());
    if source_hash.finish() != source_text_fingerprint {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel image does not retain the validated source text".into(),
        ));
    }
    if !installed_code.binds_exact_materialized_artifact_bytes(
        image.metered_text_bytes(),
        image.final_text_bytes(),
    ) {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel evidence does not bind the exact unrelocated and materialized metered text"
                .into(),
        ));
    }

    let charges = image.charges();
    if charges.len() != attributions.len()
        || charges
            .iter()
            .zip(attributions)
            .any(|(charge, attribution)| charge.attribution != *attribution)
    {
        return Err(ExternalRootDiagnostic(
            "installed dynamic fuel charges do not correspond one-for-one with the validated source attribution"
                .into(),
        ));
    }
    let mut spans = Vec::with_capacity(charges.len() * 2);
    for charge in &charges {
        let charge_end = charge
            .charge_text_offset
            .checked_add(charge.charge_byte_count)
            .ok_or_else(|| {
                ExternalRootDiagnostic("installed native fuel charge interval overflowed".into())
            })?;
        let cold_end = charge
            .cold_dispatch_text_offset
            .checked_add(charge.cold_dispatch_byte_count)
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "installed native fuel cold-dispatch interval overflowed".into(),
                )
            })?;
        let function_offset = image
            .function_text_offset(charge.attribution.machine)
            .ok_or_else(|| {
                ExternalRootDiagnostic(
                    "installed native fuel charge names an unknown metered function".into(),
                )
            })?;
        if charge.charge_byte_count == 0
            || charge.cold_dispatch_byte_count == 0
            || charge.charge_text_offset < function_offset
            || charge.semantic_text_offset < charge_end
            || charge.semantic_text_offset > image.metered_text_bytes().len()
            || charge_end > image.metered_text_bytes().len()
            || cold_end > image.metered_text_bytes().len()
        {
            return Err(ExternalRootDiagnostic(
                "installed native fuel charge has an invalid hot, semantic, or cold interval"
                    .into(),
            ));
        }
        spans.push((charge.charge_text_offset, charge_end));
        spans.push((charge.cold_dispatch_text_offset, cold_end));
    }
    spans.sort_unstable();
    if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(ExternalRootDiagnostic(
            "installed native fuel hot/cold intervals overlap".into(),
        ));
    }

    let mut final_text_hash = Fnv1a::new();
    final_text_hash.bytes(b"omega.dynamic-fuel-final-text.v1");
    final_text_hash.bytes(image.final_text_bytes());
    let final_text_fingerprint = final_text_hash.finish();
    Ok((charges, final_text_fingerprint))
}

fn fingerprint_installed_dynamic_fuel(
    basis_fingerprint: u64,
    installed_code: &InstalledCode,
    final_text_fingerprint: u64,
    charges: &[NativeFuelChargeEvidence],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.installed-dynamic-fuel.v1");
    hash.u64(basis_fingerprint);
    hash.u64(installed_code.identity().normalized_identity());
    hash.u64(installed_code.artifact().normalized_identity());
    hash.u64(final_text_fingerprint);
    hash.u64(charges.len() as u64);
    for charge in charges {
        hash.u64(charge.attribution.machine.get());
        hash.u64(charge.attribution.operation_ordinal as u64);
        hash.u64(charge.charge_text_offset as u64);
        hash.u64(charge.charge_byte_count as u64);
        hash.u64(charge.semantic_text_offset as u64);
        hash.u64(charge.cold_dispatch_text_offset as u64);
        hash.u64(charge.cold_dispatch_byte_count as u64);
    }
    hash.finish()
}

/// Validate and retain the exact source attribution catalog before any bytes
/// are installed. Later target lowering consumes this basis, emits charge
/// records, and must independently validate the final metered artifact before
/// constructing installed dynamic evidence.
pub fn validate_dynamic_fuel_attribution_basis<Artifact: ObjectEvidence>(
    plan: DynamicNativeFuelMeterPlan,
    artifact: &Artifact,
) -> Result<ValidatedDynamicFuelAttributionBasis, ExternalRootDiagnostic> {
    if plan.target() != artifact.target() {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel meter target does not match the source terminal artifact".into(),
        ));
    }

    let attributions = artifact.fuel_attribution();
    validate_dynamic_fuel_attributions(&plan, artifact, &attributions)?;

    let psi = artifact.psi();
    let mut source_text_hash = Fnv1a::new();
    source_text_hash.bytes(b"omega.dynamic-fuel-source-text.v1");
    source_text_hash.bytes(artifact.text_bytes());
    let source_text_fingerprint = source_text_hash.finish();
    let fingerprint = fingerprint_dynamic_fuel_attribution_basis(
        &plan,
        psi,
        source_text_fingerprint,
        &attributions,
    );
    Ok(ValidatedDynamicFuelAttributionBasis {
        plan,
        psi,
        source_text_fingerprint,
        attributions,
        fingerprint,
    })
}

fn validate_dynamic_fuel_attributions<Artifact: ObjectEvidence>(
    plan: &DynamicNativeFuelMeterPlan,
    artifact: &Artifact,
    attributions: &[FuelAttributionEvidence],
) -> Result<(), ExternalRootDiagnostic> {
    if attributions.is_empty() {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel meter requires at least one installed attribution row".into(),
        ));
    }
    if attributions
        .windows(2)
        .any(|rows| attribution_order_key(&rows[0]) >= attribution_order_key(&rows[1]))
    {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel attribution rows are not in canonical machine/site order".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    for row in attributions {
        let end = row.text_offset.checked_add(row.byte_count).ok_or_else(|| {
            ExternalRootDiagnostic("dynamic fuel attribution byte range overflowed".into())
        })?;
        if row.schedule != plan.schedule
            || row.units == 0
            || end > artifact.text_bytes().len()
            || artifact.function_text_offset(row.machine).is_none()
        {
            return Err(ExternalRootDiagnostic(
                "dynamic fuel attribution row is invalid for the exact terminal artifact".into(),
            ));
        }
        if !sites.insert((row.machine, row.site)) {
            return Err(ExternalRootDiagnostic(
                "dynamic fuel attribution repeats one semantic charge site".into(),
            ));
        }
    }
    Ok(())
}

fn attribution_order_key(
    row: &FuelAttributionEvidence,
) -> (u64, usize, usize, FuelAttributionSite) {
    (
        row.machine.get(),
        row.operation_ordinal,
        row.text_offset,
        row.site,
    )
}

fn fingerprint_dynamic_fuel_attribution_basis(
    plan: &DynamicNativeFuelMeterPlan,
    psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    rows: &[FuelAttributionEvidence],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.dynamic-fuel-attribution-basis.v3");
    hash.u64(u64::from(psi.vocabulary_marker.get()));
    hash.bytes(psi.program_fingerprint.as_bytes());
    hash.u64(source_text_fingerprint);
    hash.u64(u64::from(plan.schedule.marker()));
    hash.u64(plan.meter.normalized_identity());
    hash_native_fuel_target_policy(&mut hash, plan.target_policy().projection());
    hash.u64(plan.transfer_plan.normalized_identity());
    hash.u64(plan.sponsor_path.fixed.demand.composition_fingerprint());
    hash.u64(plan.sponsor_path.fixed.provision.normalized_identity());
    hash.u64(plan.sponsor_path.fixed.granted_units);
    hash.u64(plan.sponsor_path.suspension_free.composition_fingerprint());
    hash.u64(rows.len() as u64);
    for row in rows {
        hash.u64(row.machine.get());
        match row.site {
            FuelAttributionSite::Operation(operation) => {
                hash.u64(0);
                hash.u64(operation.get());
            }
            FuelAttributionSite::Edge(edge) => {
                hash.u64(1);
                hash.u64(edge.get());
            }
        }
        hash.u64(row.units);
        hash.u64(row.operation_ordinal as u64);
        hash.u64(row.text_offset as u64);
        hash.u64(row.byte_count as u64);
    }
    hash.finish()
}

fn hash_native_fuel_target_policy(hash: &mut Fnv1a, plan: &NativeFuelTargetPlanProjection) {
    hash.u64(match plan.profile {
        TargetProfile::LinuxArm64 => 0,
        TargetProfile::LinuxX64 => 1,
        TargetProfile::MacosArm64 => 2,
        TargetProfile::WindowsX64 => 3,
        TargetProfile::UefiX64 => 4,
        TargetProfile::CrossPlatformCli => 5,
        TargetProfile::LocalUnchecked => 6,
    });
    match plan.transport {
        SponsorContextTransport::ReservedNonvolatileRegister { register } => {
            hash.u64(0);
            hash.u64(match register {
                MachineRegister::X86Rbx => 3,
                MachineRegister::Aarch64X(28) => 0x21c,
                _ => u64::MAX,
            });
        }
    }
    hash.u64(u64::from(plan.context.byte_size));
    hash.u64(u64::from(plan.context.alignment));
    hash.u64(u64::from(plan.context.remaining_units_offset));
    hash.u64(u64::from(plan.context.unpaid_site_kind_offset));
    hash.u64(u64::from(plan.context.unpaid_site_identity_offset));
    hash.u64(u64::from(plan.context.required_units_offset));
    hash.u64(u64::from(plan.context.transfer_entry_offset));
    hash.u64(u64::from(plan.context.retry_code_offset_offset));
    hash.u64(u64::from(plan.context.sponsor_stack_top_offset));
    hash.u64(u64::from(plan.context.activation_state_offset));
    hash.u64(u64::from(plan.context.activation_state_byte_count));
    hash.u64(plan.transfer_plan_identity);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFuelExecutionEnvironment {
    Hosted {
        profile: TargetProfile,
        interpreter_available: bool,
    },
    Freestanding {
        profile: TargetProfile,
    },
}

impl NativeFuelExecutionEnvironment {
    fn profile(self) -> TargetProfile {
        match self {
            Self::Hosted { profile, .. } | Self::Freestanding { profile } => profile,
        }
    }

    fn target(self) -> NativeTarget {
        self.profile().native_target()
    }
}

pub enum NativeFuelRealizationRequest<'a> {
    Fixed(&'a FixedNativeFuelProvision),
    Dynamic(&'a DynamicNativeFuelMeterPlan),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFuelRealizationKind {
    FixedProvision,
    DynamicMetering,
    Interpreted,
}

/// Validated execution choice for one exact sponsor region and grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNativeFuelRealization {
    demand: ComposedFuelDemand,
    provision: FuelProvisionId,
    granted_units: u64,
    target: Option<NativeTarget>,
    kind: NativeFuelRealizationKind,
    dynamic_plan: Option<DynamicNativeFuelMeterPlan>,
}

impl ValidatedNativeFuelRealization {
    pub const fn kind(&self) -> NativeFuelRealizationKind {
        self.kind
    }

    pub const fn maximum_logical_work(&self) -> u64 {
        self.demand.units()
    }

    pub const fn granted_units(&self) -> u64 {
        self.granted_units
    }

    pub const fn provision(&self) -> FuelProvisionId {
        self.provision
    }

    pub const fn target(&self) -> Option<NativeTarget> {
        self.target
    }

    pub const fn dynamic_plan(&self) -> Option<&DynamicNativeFuelMeterPlan> {
        self.dynamic_plan.as_ref()
    }

    fn matches_resource(
        &self,
        demand: &ComposedFuelDemand,
        provision: FuelProvisionId,
        granted_units: u64,
    ) -> bool {
        self.demand == *demand && self.provision == provision && self.granted_units == granted_units
    }
}

pub fn select_fixed_native_fuel(fixed: FixedNativeFuelProvision) -> ValidatedNativeFuelRealization {
    ValidatedNativeFuelRealization {
        demand: fixed.demand,
        provision: fixed.provision,
        granted_units: fixed.granted_units,
        target: None,
        kind: NativeFuelRealizationKind::FixedProvision,
        dynamic_plan: None,
    }
}

pub fn admit_native_fuel_realization(
    demand: &ComposedFuelDemand,
    provision: FuelProvisionId,
    granted_units: u64,
    environment: NativeFuelExecutionEnvironment,
    request: NativeFuelRealizationRequest<'_>,
) -> Result<ValidatedNativeFuelRealization, ExternalRootDiagnostic> {
    if granted_units == 0 {
        return Err(ExternalRootDiagnostic(
            "native logical-fuel realization requires a nonzero grant".into(),
        ));
    }
    let (kind, dynamic_plan, target) = match request {
        NativeFuelRealizationRequest::Fixed(fixed) => {
            if !fixed.matches(demand, provision, granted_units) {
                return Err(ExternalRootDiagnostic(
                    "fixed native fuel evidence does not match the exact sponsor region, provision, and grant"
                        .into(),
                ));
            }
            (NativeFuelRealizationKind::FixedProvision, None, None)
        }
        NativeFuelRealizationRequest::Dynamic(plan) => {
            if plan.target_policy().profile() != environment.profile() {
                return Err(ExternalRootDiagnostic(
                    "dynamic native fuel plan does not match the selected target profile".into(),
                ));
            }
            if plan.schedule != demand.schedule() {
                return Err(ExternalRootDiagnostic(
                    "dynamic native fuel plan and sponsor region use different schedule versions"
                        .into(),
                ));
            }
            if plan.sponsor_path.fixed.demand.schedule() != demand.schedule() {
                return Err(ExternalRootDiagnostic(
                    "dynamic fuel region and its exhaustion sponsor path use different schedule versions"
                        .into(),
                ));
            }
            if plan.sponsor_path.fixed.demand.root() == demand.root()
                || plan.sponsor_path.fixed.provision == provision
                || !plan
                    .sponsor_path
                    .fixed
                    .demand
                    .summaries()
                    .is_disjoint(demand.summaries())
            {
                return Err(ExternalRootDiagnostic(
                    "dynamic fuel exhaustion path must use an independently provisioned sponsor region"
                        .into(),
                ));
            }
            (
                NativeFuelRealizationKind::DynamicMetering,
                Some(plan.clone()),
                Some(plan.target()),
            )
        }
        NativeFuelRealizationRequest::Unavailable => match environment {
            NativeFuelExecutionEnvironment::Hosted {
                interpreter_available: true,
                ..
            } => (
                NativeFuelRealizationKind::Interpreted,
                None,
                Some(environment.target()),
            ),
            NativeFuelExecutionEnvironment::Hosted {
                interpreter_available: false,
                ..
            } => {
                return Err(ExternalRootDiagnostic(
                    "native fuel realization is unavailable and the hosted target has no interpreter"
                        .into(),
                ));
            }
            NativeFuelExecutionEnvironment::Freestanding { .. } => {
                return Err(ExternalRootDiagnostic(
                    "freestanding installation requires an executable native fuel realization"
                        .into(),
                ));
            }
        },
    };

    Ok(ValidatedNativeFuelRealization {
        demand: demand.clone(),
        provision,
        granted_units,
        target,
        kind,
        dynamic_plan,
    })
}

/// Installed custody for the selected native realization. Dynamic selection
/// is incomplete without both exact installed attribution and executable
/// transfer-runtime bindings; fixed and interpreted selections reject either
/// stray binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelRealization {
    selected: ValidatedNativeFuelRealization,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    dynamic_attribution: Option<InstalledDynamicFuelAttributionPlan>,
    dynamic_transfer_runtime: Option<InstalledNativeFuelTransferRuntime>,
    fingerprint: u64,
}

impl InstalledNativeFuelRealization {
    pub const fn kind(&self) -> NativeFuelRealizationKind {
        self.selected.kind()
    }

    pub const fn selected(&self) -> &ValidatedNativeFuelRealization {
        &self.selected
    }

    pub const fn dynamic_attribution(&self) -> Option<&InstalledDynamicFuelAttributionPlan> {
        self.dynamic_attribution.as_ref()
    }

    pub const fn dynamic_transfer_runtime(&self) -> Option<&InstalledNativeFuelTransferRuntime> {
        self.dynamic_transfer_runtime.as_ref()
    }

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub(super) fn matches(
        &self,
        demand: &ComposedFuelDemand,
        provision: FuelProvisionId,
        granted_units: u64,
        installed_code: &InstalledCode,
    ) -> bool {
        self.selected
            .matches_resource(demand, provision, granted_units)
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

pub fn bind_installed_native_fuel_realization(
    selected: ValidatedNativeFuelRealization,
    demand: &ComposedFuelDemand,
    provision: FuelProvisionId,
    granted_units: u64,
    installed_code: &InstalledCode,
    dynamic_attribution: Option<InstalledDynamicFuelAttributionPlan>,
    dynamic_transfer_runtime: Option<InstalledNativeFuelTransferRuntime>,
) -> Result<InstalledNativeFuelRealization, ExternalRootDiagnostic> {
    if !selected.matches_resource(demand, provision, granted_units) {
        return Err(ExternalRootDiagnostic(
            "native fuel selection does not match the exact installed resource demand, provision, and grant"
                .into(),
        ));
    }
    match selected.kind {
        NativeFuelRealizationKind::FixedProvision => {
            if dynamic_attribution.is_some()
                || dynamic_transfer_runtime.is_some()
                || selected.dynamic_plan.is_some()
            {
                return Err(ExternalRootDiagnostic(
                    "fixed native fuel provision cannot retain dynamic execution evidence".into(),
                ));
            }
        }
        NativeFuelRealizationKind::DynamicMetering => {
            let attribution = dynamic_attribution.as_ref().ok_or_else(|| {
                ExternalRootDiagnostic(
                    "dynamic native fuel realization lacks installed attribution evidence".into(),
                )
            })?;
            if selected.dynamic_plan.as_ref() != Some(attribution.plan())
                || !attribution.matches_installed_code(installed_code)
            {
                return Err(ExternalRootDiagnostic(
                    "dynamic fuel selection and attribution do not bind the exact installed realization"
                    .into(),
                ));
            }
            let transfer_runtime = dynamic_transfer_runtime.as_ref().ok_or_else(|| {
                ExternalRootDiagnostic(
                    "dynamic native fuel realization lacks installed executable exhaustion-transfer runtime evidence"
                        .into(),
                )
            })?;
            if !transfer_runtime.matches(attribution.plan(), installed_code) {
                return Err(ExternalRootDiagnostic(
                    "dynamic fuel transfer runtime does not bind the exact plan and installed realization"
                        .into(),
                ));
            }
        }
        NativeFuelRealizationKind::Interpreted => {
            if dynamic_attribution.is_some()
                || dynamic_transfer_runtime.is_some()
                || selected.dynamic_plan.is_some()
            {
                return Err(ExternalRootDiagnostic(
                    "interpreted fuel realization cannot retain native dynamic execution evidence"
                        .into(),
                ));
            }
            if selected
                .target
                .is_some_and(|target| target.architecture != installed_code.architecture())
            {
                return Err(ExternalRootDiagnostic(
                    "interpreted fuel realization target does not match the installed artifact"
                        .into(),
                ));
            }
        }
    }

    let fingerprint = fingerprint_installed_native_fuel_realization(
        &selected,
        installed_code,
        dynamic_attribution.as_ref(),
        dynamic_transfer_runtime.as_ref(),
    );
    Ok(InstalledNativeFuelRealization {
        selected,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        dynamic_attribution,
        dynamic_transfer_runtime,
        fingerprint,
    })
}

fn fingerprint_installed_native_fuel_realization(
    selected: &ValidatedNativeFuelRealization,
    installed_code: &InstalledCode,
    dynamic_attribution: Option<&InstalledDynamicFuelAttributionPlan>,
    dynamic_transfer_runtime: Option<&InstalledNativeFuelTransferRuntime>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.installed-native-fuel-realization.v1");
    hash.u64(installed_code.identity().normalized_identity());
    hash.u64(installed_code.artifact().normalized_identity());
    hash.u64(selected.demand.composition_fingerprint());
    hash.u64(selected.provision.normalized_identity());
    hash.u64(selected.granted_units);
    hash.u64(match selected.kind {
        NativeFuelRealizationKind::FixedProvision => 0,
        NativeFuelRealizationKind::DynamicMetering => 1,
        NativeFuelRealizationKind::Interpreted => 2,
    });
    if let Some(target) = selected.target {
        hash.u64(1);
        fingerprint_native_target(&mut hash, target);
    } else {
        hash.u64(0);
    }
    if let Some(attribution) = dynamic_attribution {
        hash.u64(attribution.fingerprint());
    }
    if let Some(transfer_runtime) = dynamic_transfer_runtime {
        hash.u64(transfer_runtime.fingerprint());
    }
    hash.finish()
}

fn fingerprint_native_target(hash: &mut Fnv1a, target: NativeTarget) {
    hash.u64(match target.architecture {
        omega_target::Architecture::Aarch64 => 0,
        omega_target::Architecture::X86_64 => 1,
    });
    hash.u64(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    hash.u64(target.pointer_size as u64);
    hash.u64(target.pointer_alignment as u64);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use omega_calling_conventions::{
        MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };

    use super::*;
    use crate::{
        AdmittedOpaqueFuelSuspensionFree, FixedFuelProviderSummary,
        FuelSuspensionValidationReceiptId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
        RootProviderId, compose_fixed_fuel, derive_fuel_suspension_free,
    };

    struct TestArtifact {
        target: NativeTarget,
        rows: Vec<FuelAttributionEvidence>,
        bytes: Vec<u8>,
    }

    struct TestNativeFuelImage {
        target: NativeTarget,
        policy: NativeFuelTargetPlanProjection,
        source: Vec<u8>,
        metered: Vec<u8>,
        final_text: Vec<u8>,
        charges: Vec<NativeFuelChargeEvidence>,
    }

    struct TestTransferRuntimeImage {
        target: NativeTarget,
        unrelocated: Vec<u8>,
        final_text: Vec<u8>,
        sponsor_text_offset: usize,
        evidence: NativeFuelTransferRuntimeEvidence,
    }

    impl NativeFuelTransferRuntimeImageEvidence for TestTransferRuntimeImage {
        fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
            psi_terminal::TerminalPsiIdentity {
                vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([7; 32]),
            }
        }

        fn target(&self) -> NativeTarget {
            self.target
        }

        fn unrelocated_text_bytes(&self) -> &[u8] {
            &self.unrelocated
        }

        fn final_text_bytes(&self) -> &[u8] {
            &self.final_text
        }

        fn sponsor_text_offset(&self) -> usize {
            self.sponsor_text_offset
        }

        fn transfer_runtime_evidence(&self) -> &NativeFuelTransferRuntimeEvidence {
            &self.evidence
        }
    }

    impl NativeFuelImageEvidence for TestNativeFuelImage {
        fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
            psi_terminal::TerminalPsiIdentity {
                vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([7; 32]),
            }
        }

        fn target(&self) -> NativeTarget {
            self.target
        }

        fn target_policy(&self) -> NativeFuelTargetPlanProjection {
            self.policy
        }

        fn source_text_bytes(&self) -> &[u8] {
            &self.source
        }

        fn metered_text_bytes(&self) -> &[u8] {
            &self.metered
        }

        fn final_text_bytes(&self) -> &[u8] {
            &self.final_text
        }

        fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize> {
            (machine == psi_core::MachineId::new(1).unwrap()).then_some(0)
        }

        fn charges(&self) -> Vec<NativeFuelChargeEvidence> {
            self.charges.clone()
        }
    }

    impl ObjectEvidence for TestArtifact {
        fn psi(&self) -> psi_terminal::TerminalPsiIdentity {
            psi_terminal::TerminalPsiIdentity {
                vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
                program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([7; 32]),
            }
        }

        fn target(&self) -> NativeTarget {
            self.target
        }

        fn text_bytes(&self) -> &[u8] {
            &self.bytes
        }

        fn function_text_offset(&self, machine: psi_core::MachineId) -> Option<usize> {
            (machine == psi_core::MachineId::new(1).unwrap()).then_some(0)
        }

        fn fuel_attribution(&self) -> Vec<FuelAttributionEvidence> {
            self.rows.clone()
        }
    }

    fn id<T>(value: u64, constructor: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(value).expect("nonzero test identity")
    }

    fn schedule() -> psi_core::FuelScheduleIdentity {
        psi_core::FuelScheduleIdentity::new(1).expect("current test schedule")
    }

    fn opaque_demand(
        identity: u64,
        provider: u64,
        work_receipt: u64,
        units: u64,
    ) -> (ComposedFuelDemand, AdmittedOpaqueFuelSuspensionFree) {
        let identity = id(identity, ProviderFuelSummaryId::from_normalized_identity);
        let provider = id(provider, RootProviderId::from_normalized_identity);
        let work_receipt = id(
            work_receipt,
            ProviderFuelValidationReceiptId::from_normalized_identity,
        );
        let summary = FixedFuelProviderSummary::from_admitted_provider(
            identity,
            provider,
            schedule(),
            units,
            BTreeSet::new(),
            work_receipt,
        );
        let demand = compose_fixed_fuel(identity, [&summary]).expect("one-node sponsor region");
        let suspension = AdmittedOpaqueFuelSuspensionFree::from_admitted_provider(
            identity,
            provider,
            schedule(),
            work_receipt,
            id(
                work_receipt.normalized_identity() + 100,
                FuelSuspensionValidationReceiptId::from_normalized_identity,
            ),
        );
        (demand, suspension)
    }

    fn x86_context() -> NativeFuelContextLayout {
        NativeFuelContextLayout {
            byte_size: 112,
            alignment: 16,
            remaining_units_offset: 0,
            unpaid_site_kind_offset: 8,
            unpaid_site_identity_offset: 16,
            required_units_offset: 24,
            transfer_entry_offset: 32,
            retry_code_offset_offset: 40,
            sponsor_stack_top_offset: 48,
            activation_state_offset: 64,
            activation_state_byte_count: 40,
        }
    }

    fn transfer_state() -> MachineStateSet {
        MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ])
    }

    fn x86_transfer_projection(profile: TargetProfile) -> NativeFuelTransferRuntimePlanProjection {
        NativeFuelTransferRuntimePlanProjection::new(
            profile,
            profile.native_target(),
            SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            x86_context(),
            vec![
                omega_installation_evidence::NativeFuelActivationStateSlot {
                    value: omega_installation_evidence::NativeFuelSavedValue::Register(
                        MachineRegister::X86Rax,
                    ),
                    context_offset: 64,
                    byte_count: 8,
                },
                omega_installation_evidence::NativeFuelActivationStateSlot {
                    value: omega_installation_evidence::NativeFuelSavedValue::Flags,
                    context_offset: 72,
                    byte_count: 8,
                },
                omega_installation_evidence::NativeFuelActivationStateSlot {
                    value: omega_installation_evidence::NativeFuelSavedValue::Register(
                        MachineRegister::X86Xmm(0),
                    ),
                    context_offset: 80,
                    byte_count: 16,
                },
                omega_installation_evidence::NativeFuelActivationStateSlot {
                    value: omega_installation_evidence::NativeFuelSavedValue::StackPointer,
                    context_offset: 96,
                    byte_count: 8,
                },
            ],
            omega_installation_evidence::NativeFuelSponsorStackPlan {
                alignment: 16,
                byte_ceiling: 256,
            },
            transfer_state(),
            transfer_state(),
            transfer_state(),
            omega_installation_evidence::NativeFuelRuntimeEntryIdentity {
                section_identity: 1,
                symbol_identity: 2,
            },
            omega_installation_evidence::NativeFuelRuntimeEntryIdentity {
                section_identity: 1,
                symbol_identity: 3,
            },
        )
        .expect("canonical x86-64 transfer-runtime projection")
    }

    fn x86_target_projection(profile: TargetProfile) -> NativeFuelTargetPlanProjection {
        let transfer = x86_transfer_projection(profile);
        NativeFuelTargetPlanProjection {
            profile,
            target: profile.native_target(),
            transport: transfer.transport(),
            context: transfer.context(),
            transfer_plan_identity: transfer.normalized_identity(),
        }
    }

    fn x86_target_policy(profile: TargetProfile) -> AdmittedNativeFuelTargetPolicy {
        admit_native_fuel_target_policy(x86_target_projection(profile))
            .expect("canonical x86-64 native fuel target policy")
    }

    fn x86_transfer_plan(profile: TargetProfile) -> ValidatedNativeFuelTransferPlan {
        admit_native_fuel_transfer_plan(
            x86_target_policy(profile),
            x86_transfer_projection(profile),
        )
        .expect("canonical x86-64 native fuel transfer plan")
    }

    fn x86_transfer_runtime_evidence(
        profile: TargetProfile,
        transfer_unrelocated: Vec<u8>,
        transfer_final: Vec<u8>,
        resume_unrelocated: Vec<u8>,
        resume_final: Vec<u8>,
    ) -> NativeFuelTransferRuntimeEvidence {
        NativeFuelTransferRuntimeEvidence::new(
            x86_transfer_projection(profile),
            NativeFuelRuntimeTextEvidence::new(
                omega_installation_evidence::NativeFuelRuntimeEntryIdentity {
                    section_identity: 1,
                    symbol_identity: 2,
                },
                omega_installation_evidence::NativeFuelRuntimeTextSpan {
                    text_offset: 0,
                    byte_count: 4,
                },
                transfer_unrelocated,
                transfer_final,
            )
            .expect("transfer interval"),
            NativeFuelRuntimeTextEvidence::new(
                omega_installation_evidence::NativeFuelRuntimeEntryIdentity {
                    section_identity: 1,
                    symbol_identity: 3,
                },
                omega_installation_evidence::NativeFuelRuntimeTextSpan {
                    text_offset: 8,
                    byte_count: 4,
                },
                resume_unrelocated,
                resume_final,
            )
            .expect("resume interval"),
            StateFootprintEvidence::new(
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rsp,
                    MachineRegister::X86Xmm(0),
                ]),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                ]),
            ),
            24,
        )
        .expect("complete runtime evidence")
    }

    fn transfer_runtime_image(
        profile: TargetProfile,
        evidence: NativeFuelTransferRuntimeEvidence,
        unrelocated_fill: u8,
        final_fill: u8,
    ) -> TestTransferRuntimeImage {
        TestTransferRuntimeImage {
            target: profile.native_target(),
            unrelocated: vec![unrelocated_fill; 64],
            final_text: vec![final_fill; 64],
            sponsor_text_offset: 16,
            evidence,
        }
    }

    fn installed_root_sponsor_path(provision_identity: u64) -> SuspensionFreeFixedFuelProvision {
        let demand = crate::tests::fixed_fuel();
        let root_suspension = AdmittedOpaqueFuelSuspensionFree::from_admitted_provider(
            id(30, ProviderFuelSummaryId::from_normalized_identity),
            id(2, RootProviderId::from_normalized_identity),
            schedule(),
            id(
                40,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
            id(
                140,
                FuelSuspensionValidationReceiptId::from_normalized_identity,
            ),
        );
        let leaf_suspension = AdmittedOpaqueFuelSuspensionFree::from_admitted_provider(
            id(31, ProviderFuelSummaryId::from_normalized_identity),
            id(12, RootProviderId::from_normalized_identity),
            schedule(),
            id(
                41,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
            id(
                141,
                FuelSuspensionValidationReceiptId::from_normalized_identity,
            ),
        );
        let suspension_free =
            derive_fuel_suspension_free(&demand, [root_suspension, leaf_suspension])
                .expect("complete installed sponsor closure");
        let fixed = admit_fixed_native_fuel(
            &demand,
            id(
                provision_identity,
                FuelProvisionId::from_normalized_identity,
            ),
            64,
        )
        .expect("fixed installed sponsor provision");
        bind_suspension_free_fixed_fuel(fixed, suspension_free)
            .expect("fixed suspension-free installed sponsor path")
    }

    #[test]
    fn installed_transfer_code_binds_full_image_and_exact_runtime_intervals() {
        let profile = TargetProfile::LinuxX64;
        let image = transfer_runtime_image(
            profile,
            x86_transfer_runtime_evidence(profile, vec![9; 4], vec![9; 4], vec![9; 4], vec![9; 4]),
            9,
            9,
        );
        let installed = crate::tests::installed_code_with_fill(
            390,
            psi_layout_plans::EntryStubId::from_normalized_identity(1390).expect("entry"),
            9,
        );
        let custody = bind_installed_native_fuel_transfer_code(
            x86_transfer_plan(profile),
            &image,
            &installed,
        )
        .expect("exact replayed runtime image binds to installed code");
        assert_eq!(custody.installed_code(), installed.identity());
        assert_eq!(custody.runtime_evidence(), &image.evidence);
        assert!(custody.binds_installed_code(&installed));
        assert_ne!(custody.unrelocated_text_report_fingerprint(), 0);
        assert_ne!(custody.final_text_report_fingerprint(), 0);
        assert_ne!(custody.report_fingerprint(), 0);
    }

    #[test]
    fn installed_transfer_code_rejects_plan_interval_and_full_image_substitution() {
        let profile = TargetProfile::LinuxX64;
        let installed = crate::tests::installed_code_with_fill(
            391,
            psi_layout_plans::EntryStubId::from_normalized_identity(1391).expect("entry"),
            9,
        );
        let wrong_interval = transfer_runtime_image(
            profile,
            x86_transfer_runtime_evidence(profile, vec![8; 4], vec![9; 4], vec![9; 4], vec![9; 4]),
            9,
            9,
        );
        assert!(
            bind_installed_native_fuel_transfer_code(
                x86_transfer_plan(profile),
                &wrong_interval,
                &installed,
            )
            .expect_err("an interval from another object cannot enter full-image custody")
            .0
            .contains("intervals")
        );

        let wrong_full_image = transfer_runtime_image(
            profile,
            x86_transfer_runtime_evidence(profile, vec![8; 4], vec![8; 4], vec![8; 4], vec![8; 4]),
            8,
            8,
        );
        assert!(
            bind_installed_native_fuel_transfer_code(
                x86_transfer_plan(profile),
                &wrong_full_image,
                &installed,
            )
            .expect_err("different complete bytes cannot bind to installed code")
            .0
            .contains("exact installed artifact")
        );

        let uefi_evidence = transfer_runtime_image(
            TargetProfile::UefiX64,
            x86_transfer_runtime_evidence(
                TargetProfile::UefiX64,
                vec![9; 4],
                vec![9; 4],
                vec![9; 4],
                vec![9; 4],
            ),
            9,
            9,
        );
        assert!(
            bind_installed_native_fuel_transfer_code(
                x86_transfer_plan(profile),
                &uefi_evidence,
                &installed,
            )
            .expect_err("a profile sharing one native tuple cannot substitute")
            .0
            .contains("exact admitted plan")
        );
    }

    #[test]
    fn installed_transfer_runtime_requires_the_exact_fixed_sponsor_entry_route() {
        let profile = TargetProfile::LinuxX64;
        let transfer_plan = x86_transfer_plan(profile);
        let image = transfer_runtime_image(
            profile,
            x86_transfer_runtime_evidence(profile, vec![9; 4], vec![9; 4], vec![9; 4], vec![9; 4]),
            9,
            9,
        );
        let entry = psi_layout_plans::EntryStubId::from_normalized_identity(1490).expect("entry");
        let mut installed = crate::tests::installed_code_with_fill(490, entry, 9);
        let transfer_code =
            bind_installed_native_fuel_transfer_code(transfer_plan.clone(), &image, &installed)
                .expect("exact transfer bytes");
        let sponsor_path = installed_root_sponsor_path(53);
        let plan = DynamicNativeFuelMeterPlan::from_admitted_transfer_plan(
            transfer_plan,
            schedule(),
            id(490, NativeFuelMeterPlanId::from_normalized_identity),
            sponsor_path.clone(),
        );
        let (_ledger, sponsor_root) = crate::tests::install_test_root(&mut installed, entry);

        let route = bind_installed_native_fuel_sponsor_route(
            sponsor_path.clone(),
            &transfer_code,
            &sponsor_root,
        )
        .expect("resolved sponsor call names exact installed fixed root entry");
        assert_eq!(route.entry(), entry);
        assert_eq!(route.sponsor_text_offset(), 16);

        let mut compact_equal_transfer_substitute = transfer_code.clone();
        compact_equal_transfer_substitute.psi.program_fingerprint =
            psi_terminal::SemanticFingerprint::from_bytes([8; 32]);
        assert_eq!(
            compact_equal_transfer_substitute.report_fingerprint(),
            transfer_code.report_fingerprint(),
            "the adversarial substitute deliberately preserves the compact report coordinate"
        );
        assert!(
            bind_installed_native_fuel_transfer_runtime(
                plan.clone(),
                compact_equal_transfer_substitute,
                route.clone(),
            )
            .expect_err("compact-equal Psi substitution must not reuse sponsor-route authority")
            .0
            .contains("exact transfer code")
        );

        let runtime = bind_installed_native_fuel_transfer_runtime(
            plan.clone(),
            transfer_code.clone(),
            route.clone(),
        )
        .expect("both installed joins unlock transfer runtime custody");
        assert!(runtime.matches(&plan, sponsor_root.installed_code));
        assert_eq!(runtime.transfer_code(), &transfer_code);
        assert_eq!(runtime.sponsor_route(), &route);

        let (runtime_demand, _) = opaque_demand(510, 2, 512, 20);
        let runtime_provision = id(513, FuelProvisionId::from_normalized_identity);
        let machine = psi_core::MachineId::new(1).expect("machine");
        let attribution_row = FuelAttributionEvidence {
            machine,
            schedule: schedule(),
            site: FuelAttributionSite::Operation(psi_core::OperationId::new(1).expect("operation")),
            units: 1,
            operation_ordinal: 0,
            text_offset: 0,
            byte_count: 4,
        };
        let source = TestArtifact {
            target: profile.native_target(),
            rows: vec![attribution_row],
            bytes: vec![0; 4],
        };
        let basis = validate_dynamic_fuel_attribution_basis(plan.clone(), &source)
            .expect("dynamic attribution basis");
        let metered_image = TestNativeFuelImage {
            target: profile.native_target(),
            policy: x86_target_projection(profile),
            source: vec![0; 4],
            metered: vec![9; 64],
            final_text: vec![9; 64],
            charges: vec![NativeFuelChargeEvidence {
                attribution: attribution_row,
                charge_text_offset: 0,
                charge_byte_count: 4,
                semantic_text_offset: 4,
                cold_dispatch_text_offset: 40,
                cold_dispatch_byte_count: 4,
            }],
        };
        let installed_attribution = bind_installed_dynamic_fuel_attribution(
            basis,
            &metered_image,
            sponsor_root.installed_code,
        )
        .expect("final metered bytes bind to the installed occurrence");

        let mut dynamic_candidate =
            crate::tests::candidate_for_code_with_root(entry, sponsor_root.installed_code, 590);
        dynamic_candidate.logical_fuel.provision = runtime_provision;
        dynamic_candidate.logical_fuel.ceiling_units = 20;
        dynamic_candidate.logical_fuel.realization = runtime_demand;
        let dynamic_root =
            crate::validate_external_root(dynamic_candidate, &crate::tests::boundary())
                .expect("dynamic root plan");
        let dynamic_execution = crate::tests::provider_execution(&dynamic_root);
        let dynamic_slot = crate::RootSlotAuthority::from_admitted_owner(
            id(591, crate::RootSlotId::from_normalized_identity),
            id(592, crate::RootSlotOwnerId::from_normalized_identity),
        );
        let dynamic_admission =
            crate::RootAdmission::from_admitted_provider_with_dynamic_native_fuel(
                id(593, crate::RootAdmissionId::from_normalized_identity),
                &dynamic_root,
                &dynamic_execution,
                sponsor_root.installed_code,
                &dynamic_slot,
                NativeFuelExecutionEnvironment::Hosted {
                    profile,
                    interpreter_available: false,
                },
                installed_attribution.clone(),
                runtime.clone(),
                dynamic_root.candidate().trust_receipts.iter().copied(),
            )
            .expect("both sealed installed halves admit the deployed dynamic root");
        assert_eq!(
            dynamic_admission.native_fuel.kind(),
            NativeFuelRealizationKind::DynamicMetering
        );
        assert_eq!(
            dynamic_admission
                .native_fuel
                .dynamic_attribution()
                .expect("retained installed attribution"),
            &installed_attribution
        );
        assert_eq!(
            dynamic_admission
                .native_fuel
                .dynamic_transfer_runtime()
                .expect("retained executable transfer runtime"),
            &runtime
        );

        let default_admission = crate::RootAdmission::from_admitted_provider(
            id(594, crate::RootAdmissionId::from_normalized_identity),
            &dynamic_root,
            &dynamic_execution,
            sponsor_root.installed_code,
            &dynamic_slot,
            dynamic_root.candidate().trust_receipts.iter().copied(),
        )
        .expect("the ordinary deployed-root path remains available");
        assert_eq!(
            default_admission.native_fuel.kind(),
            NativeFuelRealizationKind::FixedProvision,
            "dynamic custody must never change the default root constructor"
        );

        let mut wrong_attribution = installed_attribution;
        wrong_attribution.installed_code =
            InstalledCodeId::from_normalized_identity(595).expect("installed code identity");
        assert!(
            crate::RootAdmission::from_admitted_provider_with_dynamic_native_fuel(
                id(596, crate::RootAdmissionId::from_normalized_identity),
                &dynamic_root,
                &dynamic_execution,
                sponsor_root.installed_code,
                &dynamic_slot,
                NativeFuelExecutionEnvironment::Hosted {
                    profile,
                    interpreter_available: false,
                },
                wrong_attribution,
                runtime.clone(),
                dynamic_root.candidate().trust_receipts.iter().copied(),
            )
            .expect_err("installed attribution from another code occurrence must reject")
            .0
            .contains("exact installed realization")
        );

        let mut wrong_runtime = runtime;
        wrong_runtime.plan = DynamicNativeFuelMeterPlan::from_admitted_transfer_plan(
            x86_transfer_plan(profile),
            schedule(),
            id(597, NativeFuelMeterPlanId::from_normalized_identity),
            installed_root_sponsor_path(54),
        );
        assert!(
            crate::RootAdmission::from_admitted_provider_with_dynamic_native_fuel(
                id(598, crate::RootAdmissionId::from_normalized_identity),
                &dynamic_root,
                &dynamic_execution,
                sponsor_root.installed_code,
                &dynamic_slot,
                NativeFuelExecutionEnvironment::Hosted {
                    profile,
                    interpreter_available: false,
                },
                dynamic_admission
                    .native_fuel
                    .dynamic_attribution()
                    .expect("retained installed attribution")
                    .clone(),
                wrong_runtime,
                dynamic_root.candidate().trust_receipts.iter().copied(),
            )
            .expect_err("a runtime sealed for another dynamic plan must reject")
            .0
            .contains("exact plan")
        );

        let mut wrong_coordinate_image = image;
        wrong_coordinate_image.sponsor_text_offset = 17;
        let wrong_coordinate_code = bind_installed_native_fuel_transfer_code(
            x86_transfer_plan(profile),
            &wrong_coordinate_image,
            sponsor_root.installed_code,
        )
        .expect("in-range coordinate remains byte custody only");
        assert!(
            bind_installed_native_fuel_sponsor_route(
                sponsor_path,
                &wrong_coordinate_code,
                &sponsor_root,
            )
            .expect_err("adjacent text cannot substitute for selected entry")
            .0
            .contains("selected installed root entry")
        );

        let wrong_provision = installed_root_sponsor_path(54);
        assert!(
            bind_installed_native_fuel_sponsor_route(
                wrong_provision,
                &transfer_code,
                &sponsor_root,
            )
            .expect_err("a different fixed provision cannot borrow this root")
            .0
            .contains("exact installed fixed provision")
        );
    }

    #[test]
    fn native_fuel_target_policy_rejects_unsafe_transport_and_layout() {
        let mut volatile = x86_target_projection(TargetProfile::LinuxX64);
        volatile.transport = SponsorContextTransport::ReservedNonvolatileRegister {
            register: MachineRegister::X86Rax,
        };
        assert!(
            admit_native_fuel_target_policy(volatile)
                .expect_err("a volatile context register cannot be admitted")
                .0
                .contains("RBX")
        );

        let mut overlapping = x86_target_projection(TargetProfile::LinuxX64);
        overlapping.context.required_units_offset = overlapping.context.remaining_units_offset;
        assert!(
            admit_native_fuel_target_policy(overlapping)
                .expect_err("context fields must be disjoint")
                .0
                .contains("overlap")
        );
    }

    #[test]
    fn native_fuel_transfer_plan_requires_the_exact_admitted_target_policy() {
        let profile = TargetProfile::WindowsX64;
        let projection = x86_transfer_projection(profile);
        let admitted =
            admit_native_fuel_transfer_plan(x86_target_policy(profile), projection.clone())
                .expect("exact structural transfer plan");
        assert_eq!(admitted.projection(), &projection);
        assert_eq!(
            admitted.normalized_identity(),
            projection.normalized_identity()
        );

        let mut wrong_identity = x86_target_projection(profile);
        wrong_identity.transfer_plan_identity =
            wrong_identity.transfer_plan_identity.wrapping_add(1);
        let wrong_identity = admit_native_fuel_target_policy(wrong_identity)
            .expect("nonzero alternate identity remains a structurally valid target recipe");
        assert!(
            admit_native_fuel_transfer_plan(wrong_identity, projection.clone())
                .expect_err("transfer identity drift must reject")
                .0
                .contains("exact admitted target policy")
        );

        assert!(
            admit_native_fuel_transfer_plan(
                x86_target_policy(TargetProfile::UefiX64),
                projection.clone(),
            )
            .expect_err("profiles sharing one native tuple remain distinct")
            .0
            .contains("exact admitted target policy")
        );

        let mut wrong_context = x86_target_projection(profile);
        wrong_context.context.byte_size = 128;
        let wrong_context = admit_native_fuel_target_policy(wrong_context)
            .expect("larger nonoverlapping context remains structurally valid");
        assert!(
            admit_native_fuel_transfer_plan(wrong_context, projection)
                .expect_err("context drift must reject")
                .0
                .contains("exact admitted target policy")
        );
    }

    #[test]
    fn fixed_native_provision_uses_exact_maximum_logical_work() {
        let (demand, _) = opaque_demand(1, 2, 3, 8);
        let provision = id(4, FuelProvisionId::from_normalized_identity);
        let fixed = admit_fixed_native_fuel(&demand, provision, 8).expect("exact fixed grant");
        assert_eq!(fixed.maximum_logical_work(), 8);
        assert_eq!(fixed.granted_units(), 8);
        assert!(fixed.meter_elided());

        let error = admit_fixed_native_fuel(&demand, provision, 7)
            .expect_err("a conservative undersized grant cannot elide metering");
        assert!(
            error
                .0
                .contains("exact installed maximum logical work is 8")
        );
    }

    #[test]
    fn dynamic_native_realization_requires_an_independent_fixed_sponsor_path() {
        let (runtime_demand, _) = opaque_demand(10, 11, 12, 20);
        let runtime_provision = id(13, FuelProvisionId::from_normalized_identity);
        let (sponsor_demand, sponsor_suspension) = opaque_demand(20, 21, 22, 4);
        let sponsor_fixed = admit_fixed_native_fuel(
            &sponsor_demand,
            id(23, FuelProvisionId::from_normalized_identity),
            4,
        )
        .expect("fixed sponsor path");
        let sponsor_free = derive_fuel_suspension_free(&sponsor_demand, [sponsor_suspension])
            .expect("sponsor path cannot suspend for fuel");
        let sponsor_path = bind_suspension_free_fixed_fuel(sponsor_fixed, sponsor_free.clone())
            .expect("exact fixed/suspension join");
        let alternate_sponsor_fixed = admit_fixed_native_fuel(
            &sponsor_demand,
            id(230, FuelProvisionId::from_normalized_identity),
            4,
        )
        .expect("alternate exact fixed sponsor path");
        let alternate_sponsor_path =
            bind_suspension_free_fixed_fuel(alternate_sponsor_fixed, sponsor_free)
                .expect("alternate fixed/suspension join");
        let profile = TargetProfile::WindowsX64;
        let target = profile.native_target();
        let plan = DynamicNativeFuelMeterPlan::from_admitted_transfer_plan(
            x86_transfer_plan(profile),
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            sponsor_path,
        );
        let alternate_sponsor_plan = DynamicNativeFuelMeterPlan::from_admitted_transfer_plan(
            x86_transfer_plan(profile),
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            alternate_sponsor_path,
        );
        let machine = psi_core::MachineId::new(1).unwrap();
        let rows = vec![
            FuelAttributionEvidence {
                machine,
                schedule: schedule(),
                site: FuelAttributionSite::Operation(psi_core::OperationId::new(1).unwrap()),
                units: 1,
                operation_ordinal: 0,
                text_offset: 0,
                byte_count: 0,
            },
            FuelAttributionEvidence {
                machine,
                schedule: schedule(),
                site: FuelAttributionSite::Edge(psi_core::EdgeId::new(1).unwrap()),
                units: 1,
                operation_ordinal: 1,
                text_offset: 0,
                byte_count: 4,
            },
        ];
        let artifact = TestArtifact {
            target,
            rows: rows.clone(),
            bytes: vec![0; 4],
        };
        let basis = validate_dynamic_fuel_attribution_basis(plan.clone(), &artifact)
            .expect("zero-code and byte-bearing attribution sites are valid meter inputs");
        assert_eq!(basis.attributions(), rows);
        let alternate_sponsor_basis =
            validate_dynamic_fuel_attribution_basis(alternate_sponsor_plan, &artifact)
                .expect("alternate sponsor path remains structurally valid");
        assert_ne!(
            basis.fingerprint(),
            alternate_sponsor_basis.fingerprint(),
            "the published basis identity must bind the exact sponsor provision"
        );
        let changed_source = TestArtifact {
            target,
            rows: rows.clone(),
            bytes: vec![1; 4],
        };
        let changed_basis = validate_dynamic_fuel_attribution_basis(plan.clone(), &changed_source)
            .expect("different valid source bytes remain a distinct instrumentation input");
        assert_ne!(basis.fingerprint(), changed_basis.fingerprint());

        let mut image = TestNativeFuelImage {
            target,
            policy: x86_target_projection(profile),
            source: vec![0; 4],
            metered: vec![9; 64],
            final_text: vec![9; 64],
            charges: vec![
                NativeFuelChargeEvidence {
                    attribution: rows[0],
                    charge_text_offset: 0,
                    charge_byte_count: 4,
                    semantic_text_offset: 4,
                    cold_dispatch_text_offset: 40,
                    cold_dispatch_byte_count: 4,
                },
                NativeFuelChargeEvidence {
                    attribution: rows[1],
                    charge_text_offset: 4,
                    charge_byte_count: 4,
                    semantic_text_offset: 8,
                    cold_dispatch_text_offset: 44,
                    cold_dispatch_byte_count: 4,
                },
            ],
        };
        let installed = crate::tests::installed_code_with_fill(
            91,
            psi_layout_plans::EntryStubId::from_normalized_identity(1091).expect("entry"),
            9,
        );
        let installed_attribution =
            bind_installed_dynamic_fuel_attribution(basis, &image, &installed)
                .expect("replayed final bytes bind dynamic attribution to installation");
        assert_eq!(installed_attribution.charges(), image.charges());
        assert_eq!(installed_attribution.installed_code(), installed.identity());
        validate_installed_dynamic_fuel_attribution(&installed_attribution, &image, &installed)
            .expect("installed attribution independently replays");

        let mismatched_installation = crate::tests::installed_code_with_fill(
            92,
            psi_layout_plans::EntryStubId::from_normalized_identity(1092).expect("entry"),
            8,
        );
        assert!(
            !mismatched_installation.binds_exact_materialized_artifact_bytes(
                image.metered_text_bytes(),
                image.final_text_bytes()
            )
        );
        assert!(
            validate_installed_dynamic_fuel_attribution(
                &installed_attribution,
                &image,
                &mismatched_installation,
            )
            .expect_err("compact installed identities cannot substitute exact occurrence custody")
            .0
            .contains("exact installed-code occurrence")
        );

        image.source[0] ^= 1;
        assert!(validate_installed_dynamic_fuel_attribution(
            &installed_attribution,
            &image,
            &installed,
        )
        .expect_err("source text drift must reject replay")
        .0
        .contains("validated source text"));
        image.source[0] ^= 1;
        image.policy = x86_target_projection(TargetProfile::UefiX64);
        assert!(validate_installed_dynamic_fuel_attribution(
            &installed_attribution,
            &image,
            &installed,
        )
        .expect_err("target-profile policy cannot be substituted by equal native tuple")
        .0
        .contains("admitted target recipe"));
        image.policy = x86_target_projection(profile);
        image.charges[0].attribution.operation_ordinal += 1;
        assert!(validate_installed_dynamic_fuel_attribution(
            &installed_attribution,
            &image,
            &installed,
        )
        .expect_err("charge attribution drift must reject replay")
        .0
        .contains("one-for-one"));
        image.charges[0].attribution.operation_ordinal -= 1;
        image.final_text[0] ^= 1;
        assert!(validate_installed_dynamic_fuel_attribution(
            &installed_attribution,
            &image,
            &installed,
        )
        .expect_err("materialized final-text drift must reject replay")
        .0
        .contains("exact unrelocated and materialized"));
        image.final_text[0] ^= 1;

        let mut drifted = installed_attribution.clone();
        drifted.basis_fingerprint ^= 1;
        assert!(
            validate_installed_dynamic_fuel_attribution(&drifted, &image, &installed)
                .expect_err("retained basis fingerprint drift must reject replay")
                .0
                .contains("basis fingerprint")
        );
        let mut drifted = installed_attribution.clone();
        drifted.charges.swap(0, 1);
        assert!(
            validate_installed_dynamic_fuel_attribution(&drifted, &image, &installed)
                .expect_err("retained charge order drift must reject replay")
                .0
                .contains("charge or final-text")
        );
        let mut drifted = installed_attribution.clone();
        drifted.fingerprint ^= 1;
        assert!(
            validate_installed_dynamic_fuel_attribution(&drifted, &image, &installed)
                .expect_err("aggregate installed attribution identity drift must reject replay")
                .0
                .contains("attribution fingerprint")
        );
        let mut duplicate = rows.clone();
        duplicate[1].site = duplicate[0].site;
        let error = validate_dynamic_fuel_attributions(&plan, &artifact, &duplicate)
            .expect_err("one semantic charge site cannot be inserted twice");
        assert!(error.0.contains("repeats one semantic charge site"));

        let realized = admit_native_fuel_realization(
            &runtime_demand,
            runtime_provision,
            3,
            NativeFuelExecutionEnvironment::Hosted {
                profile,
                interpreter_available: true,
            },
            NativeFuelRealizationRequest::Dynamic(&plan),
        )
        .expect("admitted dynamic meter");
        assert_eq!(realized.kind(), NativeFuelRealizationKind::DynamicMetering);
        assert_eq!(realized.maximum_logical_work(), 20);
        assert_eq!(realized.granted_units(), 3);
        assert_eq!(realized.dynamic_plan(), Some(&plan));

        assert_eq!(
            TargetProfile::WindowsX64.native_target(),
            TargetProfile::UefiX64.native_target(),
            "profile custody matters even when the native tuples are identical"
        );
        let wrong_target = admit_native_fuel_realization(
            &runtime_demand,
            runtime_provision,
            3,
            NativeFuelExecutionEnvironment::Freestanding {
                profile: TargetProfile::UefiX64,
            },
            NativeFuelRealizationRequest::Dynamic(&plan),
        )
        .expect_err("a target meter plan cannot cross an identical native tuple's profile");
        assert!(wrong_target.0.contains("selected target"));

        let missing_transfer_runtime = bind_installed_native_fuel_realization(
            realized,
            &runtime_demand,
            runtime_provision,
            3,
            &installed,
            Some(installed_attribution),
            None,
        )
        .expect_err("installed charge attribution alone is not an executable transfer path");
        assert!(
            missing_transfer_runtime
                .0
                .contains("executable exhaustion-transfer runtime")
        );
    }

    #[test]
    fn unavailable_native_fuel_interprets_only_on_an_enabled_host() {
        let (demand, _) = opaque_demand(30, 31, 32, 5);
        let provision = id(33, FuelProvisionId::from_normalized_identity);
        let profile = TargetProfile::LinuxX64;
        let interpreted = admit_native_fuel_realization(
            &demand,
            provision,
            2,
            NativeFuelExecutionEnvironment::Hosted {
                profile,
                interpreter_available: true,
            },
            NativeFuelRealizationRequest::Unavailable,
        )
        .expect("hosted interpreter fallback");
        assert_eq!(interpreted.kind(), NativeFuelRealizationKind::Interpreted);

        let native_only = admit_native_fuel_realization(
            &demand,
            provision,
            2,
            NativeFuelExecutionEnvironment::Hosted {
                profile,
                interpreter_available: false,
            },
            NativeFuelRealizationRequest::Unavailable,
        )
        .expect_err("native-only host has no fallback");
        assert!(native_only.0.contains("no interpreter"));

        let freestanding = admit_native_fuel_realization(
            &demand,
            provision,
            2,
            NativeFuelExecutionEnvironment::Freestanding { profile },
            NativeFuelRealizationRequest::Unavailable,
        )
        .expect_err("freestanding targets reject unavailable metering");
        assert!(freestanding.0.contains("freestanding installation"));
    }
}

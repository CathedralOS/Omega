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
use omega_target::{NativeTarget, TargetProfile};
use omega_terminal_installation_evidence::{
    TerminalFuelAttributionEvidence, TerminalFuelAttributionSite, TerminalNativeFuelChargeEvidence,
    TerminalNativeFuelImageEvidence, TerminalObjectEvidence,
};

use super::{
    ComposedFuelDemand, DynamicFuelMeterValidationReceiptId, ExternalRootDiagnostic, Fnv1a,
    FuelExhaustionTransferPlanId, FuelProvisionId, FuelSuspensionFreeEvidence,
    NativeFuelContextLayout, NativeFuelMeterPlanId, NativeFuelTargetPlanProjection,
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

/// Pre-install dynamic meter selection. The transfer identity must agree with
/// the admitted target recipe. The validation receipt remains plan metadata;
/// it cannot authorize installed execution without separately constructed
/// executable transfer-runtime custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicNativeFuelMeterPlan {
    target_policy: AdmittedNativeFuelTargetPolicy,
    schedule: psi_core::FuelScheduleIdentity,
    meter: NativeFuelMeterPlanId,
    exhaustion_transfer: FuelExhaustionTransferPlanId,
    sponsor_path: SuspensionFreeFixedFuelProvision,
    validation_receipt: DynamicFuelMeterValidationReceiptId,
}

impl DynamicNativeFuelMeterPlan {
    pub fn from_admitted_target_policy(
        target_policy: AdmittedNativeFuelTargetPolicy,
        schedule: psi_core::FuelScheduleIdentity,
        meter: NativeFuelMeterPlanId,
        exhaustion_transfer: FuelExhaustionTransferPlanId,
        sponsor_path: SuspensionFreeFixedFuelProvision,
        validation_receipt: DynamicFuelMeterValidationReceiptId,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if exhaustion_transfer.normalized_identity()
            != target_policy.projection().transfer_plan_identity
        {
            return Err(ExternalRootDiagnostic(
                "dynamic native fuel transfer does not match the admitted target policy".into(),
            ));
        }
        Ok(Self {
            target_policy,
            schedule,
            meter,
            exhaustion_transfer,
            sponsor_path,
            validation_receipt,
        })
    }

    pub const fn target(&self) -> NativeTarget {
        self.target_policy.target()
    }

    pub const fn target_policy(&self) -> &AdmittedNativeFuelTargetPolicy {
        &self.target_policy
    }

    pub const fn sponsor_path(&self) -> &SuspensionFreeFixedFuelProvision {
        &self.sponsor_path
    }
}

/// Installed custody for the executable exhaustion-transfer runtime. This
/// value deliberately has no public constructor until target-runtime emission,
/// final-byte replay, and sponsor-path installation can establish every field.
/// Its presence in the final join makes opaque validation receipt identifiers
/// insufficient to admit dynamic execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelTransferRuntime {
    plan: DynamicNativeFuelMeterPlan,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    fingerprint: u64,
}

impl InstalledNativeFuelTransferRuntime {
    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn matches(&self, plan: &DynamicNativeFuelMeterPlan, installed_code: &InstalledCode) -> bool {
        self.plan == *plan
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Validated pre-install input to target instrumentation. This owns the exact
/// semantic rows and source bytes; it is deliberately not installed execution
/// evidence because charge insertion changes both bytes and offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDynamicFuelAttributionBasis {
    plan: DynamicNativeFuelMeterPlan,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    attributions: Vec<TerminalFuelAttributionEvidence>,
    fingerprint: u64,
}

impl ValidatedDynamicFuelAttributionBasis {
    pub const fn plan(&self) -> &DynamicNativeFuelMeterPlan {
        &self.plan
    }

    pub fn attributions(&self) -> &[TerminalFuelAttributionEvidence] {
        &self.attributions
    }

    pub const fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
        self.terminal_psi
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
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    attributions: Vec<TerminalFuelAttributionEvidence>,
    charges: Vec<TerminalNativeFuelChargeEvidence>,
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

    pub fn attributions(&self) -> &[TerminalFuelAttributionEvidence] {
        &self.attributions
    }

    pub fn charges(&self) -> &[TerminalNativeFuelChargeEvidence] {
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
pub fn bind_installed_dynamic_fuel_attribution<Image: TerminalNativeFuelImageEvidence>(
    basis: ValidatedDynamicFuelAttributionBasis,
    image: &Image,
    installed_code: &InstalledCode,
) -> Result<InstalledDynamicFuelAttributionPlan, ExternalRootDiagnostic> {
    if image.terminal_psi() != basis.terminal_psi
        || image.target() != basis.plan.target()
        || image.target_policy() != *basis.plan.target_policy.projection()
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
    if source_hash.finish() != basis.source_text_fingerprint {
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
    if charges.len() != basis.attributions.len()
        || charges
            .iter()
            .zip(&basis.attributions)
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
    let fingerprint = fingerprint_installed_dynamic_fuel(
        &basis,
        installed_code,
        final_text_fingerprint,
        &charges,
    );
    Ok(InstalledDynamicFuelAttributionPlan {
        plan: basis.plan,
        terminal_psi: basis.terminal_psi,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        attributions: basis.attributions,
        charges,
        final_text_fingerprint,
        fingerprint,
    })
}

fn fingerprint_installed_dynamic_fuel(
    basis: &ValidatedDynamicFuelAttributionBasis,
    installed_code: &InstalledCode,
    final_text_fingerprint: u64,
    charges: &[TerminalNativeFuelChargeEvidence],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.installed-dynamic-fuel.v1");
    hash.u64(basis.fingerprint);
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
pub fn validate_dynamic_fuel_attribution_basis<Artifact: TerminalObjectEvidence>(
    plan: DynamicNativeFuelMeterPlan,
    terminal_artifact: &Artifact,
) -> Result<ValidatedDynamicFuelAttributionBasis, ExternalRootDiagnostic> {
    if plan.target() != terminal_artifact.target() {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel meter target does not match the source terminal artifact".into(),
        ));
    }

    let attributions = terminal_artifact.fuel_attribution();
    validate_dynamic_fuel_attributions(&plan, terminal_artifact, &attributions)?;

    let terminal_psi = terminal_artifact.terminal_psi();
    let mut source_text_hash = Fnv1a::new();
    source_text_hash.bytes(b"omega.dynamic-fuel-source-text.v1");
    source_text_hash.bytes(terminal_artifact.text_bytes());
    let source_text_fingerprint = source_text_hash.finish();
    let fingerprint = fingerprint_dynamic_fuel_attribution_basis(
        &plan,
        terminal_psi,
        source_text_fingerprint,
        &attributions,
    );
    Ok(ValidatedDynamicFuelAttributionBasis {
        plan,
        terminal_psi,
        source_text_fingerprint,
        attributions,
        fingerprint,
    })
}

fn validate_dynamic_fuel_attributions<Artifact: TerminalObjectEvidence>(
    plan: &DynamicNativeFuelMeterPlan,
    terminal_artifact: &Artifact,
    attributions: &[TerminalFuelAttributionEvidence],
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
            || end > terminal_artifact.text_bytes().len()
            || terminal_artifact
                .function_text_offset(row.machine)
                .is_none()
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
    row: &TerminalFuelAttributionEvidence,
) -> (u64, usize, usize, TerminalFuelAttributionSite) {
    (
        row.machine.get(),
        row.operation_ordinal,
        row.text_offset,
        row.site,
    )
}

fn fingerprint_dynamic_fuel_attribution_basis(
    plan: &DynamicNativeFuelMeterPlan,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    source_text_fingerprint: u64,
    rows: &[TerminalFuelAttributionEvidence],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.dynamic-fuel-attribution-basis.v2");
    hash.u64(u64::from(terminal_psi.vocabulary_marker.get()));
    hash.bytes(terminal_psi.program_fingerprint.as_bytes());
    hash.u64(source_text_fingerprint);
    hash.u64(u64::from(plan.schedule.marker()));
    hash.u64(plan.meter.normalized_identity());
    hash_native_fuel_target_policy(&mut hash, plan.target_policy.projection());
    hash.u64(plan.exhaustion_transfer.normalized_identity());
    hash.u64(plan.validation_receipt.normalized_identity());
    hash.u64(plan.sponsor_path.fixed.demand.composition_fingerprint());
    hash.u64(plan.sponsor_path.fixed.provision.normalized_identity());
    hash.u64(plan.sponsor_path.fixed.granted_units);
    hash.u64(plan.sponsor_path.suspension_free.composition_fingerprint());
    hash.u64(rows.len() as u64);
    for row in rows {
        hash.u64(row.machine.get());
        match row.site {
            TerminalFuelAttributionSite::Operation(operation) => {
                hash.u64(0);
                hash.u64(operation.get());
            }
            TerminalFuelAttributionSite::Edge(edge) => {
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
            if plan.target_policy.profile() != environment.profile() {
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

    use super::*;
    use crate::{
        AdmittedOpaqueFuelSuspensionFree, FixedFuelProviderSummary,
        FuelSuspensionValidationReceiptId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
        RootProviderId, compose_fixed_fuel, derive_fuel_suspension_free,
    };

    struct TestTerminalArtifact {
        target: NativeTarget,
        rows: Vec<TerminalFuelAttributionEvidence>,
        bytes: Vec<u8>,
    }

    struct TestNativeFuelImage {
        target: NativeTarget,
        policy: NativeFuelTargetPlanProjection,
        source: Vec<u8>,
        metered: Vec<u8>,
        final_text: Vec<u8>,
        charges: Vec<TerminalNativeFuelChargeEvidence>,
    }

    impl TerminalNativeFuelImageEvidence for TestNativeFuelImage {
        fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
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

        fn charges(&self) -> Vec<TerminalNativeFuelChargeEvidence> {
            self.charges.clone()
        }
    }

    impl TerminalObjectEvidence for TestTerminalArtifact {
        fn terminal_psi(&self) -> psi_terminal::TerminalPsiIdentity {
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

        fn fuel_attribution(&self) -> Vec<TerminalFuelAttributionEvidence> {
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

    fn x86_target_projection(profile: TargetProfile) -> NativeFuelTargetPlanProjection {
        NativeFuelTargetPlanProjection {
            profile,
            target: profile.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister {
                register: MachineRegister::X86Rbx,
            },
            context: NativeFuelContextLayout {
                byte_size: 256,
                alignment: 16,
                remaining_units_offset: 0,
                unpaid_site_kind_offset: 8,
                unpaid_site_identity_offset: 16,
                required_units_offset: 24,
                transfer_entry_offset: 32,
                retry_code_offset_offset: 40,
                sponsor_stack_top_offset: 48,
                activation_state_offset: 64,
                activation_state_byte_count: 192,
            },
            transfer_plan_identity: 26,
        }
    }

    fn x86_target_policy(profile: TargetProfile) -> AdmittedNativeFuelTargetPolicy {
        admit_native_fuel_target_policy(x86_target_projection(profile))
            .expect("canonical x86-64 native fuel target policy")
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
        let mismatched_transfer = DynamicNativeFuelMeterPlan::from_admitted_target_policy(
            x86_target_policy(profile),
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            id(25, FuelExhaustionTransferPlanId::from_normalized_identity),
            sponsor_path.clone(),
            id(
                27,
                DynamicFuelMeterValidationReceiptId::from_normalized_identity,
            ),
        )
        .expect_err("an opaque receipt cannot hide transfer-plan identity drift");
        assert!(mismatched_transfer.0.contains("target policy"));
        let plan = DynamicNativeFuelMeterPlan::from_admitted_target_policy(
            x86_target_policy(profile),
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            id(26, FuelExhaustionTransferPlanId::from_normalized_identity),
            sponsor_path,
            id(
                27,
                DynamicFuelMeterValidationReceiptId::from_normalized_identity,
            ),
        )
        .expect("target policy and exhaustion transfer identity agree");
        let alternate_sponsor_plan = DynamicNativeFuelMeterPlan::from_admitted_target_policy(
            x86_target_policy(profile),
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            id(26, FuelExhaustionTransferPlanId::from_normalized_identity),
            alternate_sponsor_path,
            id(
                27,
                DynamicFuelMeterValidationReceiptId::from_normalized_identity,
            ),
        )
        .expect("alternate sponsor still matches the target transfer identity");
        let machine = psi_core::MachineId::new(1).unwrap();
        let rows = vec![
            TerminalFuelAttributionEvidence {
                machine,
                schedule: schedule(),
                site: TerminalFuelAttributionSite::Operation(
                    psi_core::OperationId::new(1).unwrap(),
                ),
                units: 1,
                operation_ordinal: 0,
                text_offset: 0,
                byte_count: 0,
            },
            TerminalFuelAttributionEvidence {
                machine,
                schedule: schedule(),
                site: TerminalFuelAttributionSite::Edge(psi_core::EdgeId::new(1).unwrap()),
                units: 1,
                operation_ordinal: 1,
                text_offset: 0,
                byte_count: 4,
            },
        ];
        let artifact = TestTerminalArtifact {
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
        let changed_source = TestTerminalArtifact {
            target,
            rows: rows.clone(),
            bytes: vec![1; 4],
        };
        let changed_basis = validate_dynamic_fuel_attribution_basis(plan.clone(), &changed_source)
            .expect("different valid source bytes remain a distinct instrumentation input");
        assert_ne!(basis.fingerprint(), changed_basis.fingerprint());

        let image = TestNativeFuelImage {
            target,
            policy: x86_target_projection(profile),
            source: vec![0; 4],
            metered: vec![9; 64],
            final_text: vec![9; 64],
            charges: vec![
                TerminalNativeFuelChargeEvidence {
                    attribution: rows[0],
                    charge_text_offset: 0,
                    charge_byte_count: 4,
                    semantic_text_offset: 4,
                    cold_dispatch_text_offset: 40,
                    cold_dispatch_byte_count: 4,
                },
                TerminalNativeFuelChargeEvidence {
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

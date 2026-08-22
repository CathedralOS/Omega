//! Native realization selection for one sponsor-owned logical-fuel region.
//!
//! Fixed provision is an exact maximum-work/grant proof. Dynamic metering is
//! target-owned admitted instrumentation with a private context transport and
//! an independently fixed-provisioned, suspension-free exhaustion path.
//! Hosted interpretation is an explicit fallback; freestanding installation
//! never treats unavailable native metering as executable.

use std::collections::BTreeSet;

use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use omega_target::NativeTarget;
use omega_terminal_installation_evidence::{
    TerminalFuelAttributionEvidence, TerminalFuelAttributionSite, TerminalObjectEvidence,
};

use super::{
    ComposedFuelDemand, DynamicFuelMeterValidationReceiptId, ExternalRootDiagnostic, Fnv1a,
    FuelExhaustionTransferPlanId, FuelProvisionId, FuelSuspensionFreeEvidence,
    NativeFuelMeterPlanId, SponsorContextTransportId,
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

/// Target-admitted dynamic meter contract. The validation receipt covers the
/// compare-before-subtract implementation, exact unpaid-site transfer, opaque
/// activation-state preservation, and resume at the failed pre-charge check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicNativeFuelMeterPlan {
    target: NativeTarget,
    schedule: psi_core::FuelScheduleIdentity,
    meter: NativeFuelMeterPlanId,
    context_transport: SponsorContextTransportId,
    exhaustion_transfer: FuelExhaustionTransferPlanId,
    sponsor_path: SuspensionFreeFixedFuelProvision,
    validation_receipt: DynamicFuelMeterValidationReceiptId,
}

impl DynamicNativeFuelMeterPlan {
    pub const fn from_admitted_target(
        target: NativeTarget,
        schedule: psi_core::FuelScheduleIdentity,
        meter: NativeFuelMeterPlanId,
        context_transport: SponsorContextTransportId,
        exhaustion_transfer: FuelExhaustionTransferPlanId,
        sponsor_path: SuspensionFreeFixedFuelProvision,
        validation_receipt: DynamicFuelMeterValidationReceiptId,
    ) -> Self {
        Self {
            target,
            schedule,
            meter,
            context_transport,
            exhaustion_transfer,
            sponsor_path,
            validation_receipt,
        }
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn sponsor_path(&self) -> &SuspensionFreeFixedFuelProvision {
        &self.sponsor_path
    }
}

/// A dynamic meter plan bound to the exact relocation-free terminal artifact
/// and installed code whose charge sites it must instrument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDynamicFuelAttributionPlan {
    plan: DynamicNativeFuelMeterPlan,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    attributions: Vec<TerminalFuelAttributionEvidence>,
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

    pub const fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn matches_installed_code(&self, installed_code: &InstalledCode) -> bool {
        self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
    }
}

/// Bind dynamic metering to the exact already-validated attribution catalog.
/// This emits no instructions; it closes the evidence input that later target
/// lowering must consume without rediscovering semantic sites from bytes.
pub fn bind_installed_dynamic_fuel_attributions<Artifact: TerminalObjectEvidence>(
    plan: DynamicNativeFuelMeterPlan,
    terminal_artifact: &Artifact,
    installed_code: &InstalledCode,
) -> Result<InstalledDynamicFuelAttributionPlan, ExternalRootDiagnostic> {
    if plan.target != terminal_artifact.target()
        || plan.target.architecture != installed_code.architecture()
    {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel meter target does not match the terminal artifact and installed code"
                .into(),
        ));
    }
    if !installed_code.binds_exact_unrelocated_artifact_bytes(terminal_artifact.text_bytes()) {
        return Err(ExternalRootDiagnostic(
            "dynamic fuel attribution requires the exact relocation-free installed terminal bytes"
                .into(),
        ));
    }

    let attributions = terminal_artifact.fuel_attribution();
    validate_dynamic_fuel_attributions(&plan, terminal_artifact, &attributions)?;

    let terminal_psi = terminal_artifact.terminal_psi();
    let installed_code_context = installed_code.receipt_context();
    let artifact = installed_code.artifact();
    let fingerprint = fingerprint_installed_dynamic_fuel_attributions(
        &plan,
        terminal_psi,
        installed_code.identity(),
        artifact,
        &attributions,
    );
    Ok(InstalledDynamicFuelAttributionPlan {
        plan,
        terminal_psi,
        installed_code: installed_code.identity(),
        installed_code_context,
        artifact,
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

fn fingerprint_installed_dynamic_fuel_attributions(
    plan: &DynamicNativeFuelMeterPlan,
    terminal_psi: psi_terminal::TerminalPsiIdentity,
    installed_code: InstalledCodeId,
    artifact: ArtifactId,
    rows: &[TerminalFuelAttributionEvidence],
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.installed-dynamic-fuel-attribution.v1");
    hash.u64(u64::from(terminal_psi.vocabulary_marker.get()));
    hash.bytes(terminal_psi.program_fingerprint.as_bytes());
    hash.u64(installed_code.normalized_identity());
    hash.u64(artifact.normalized_identity());
    hash.u64(u64::from(plan.schedule.marker()));
    hash.u64(plan.meter.normalized_identity());
    hash.u64(plan.context_transport.normalized_identity());
    hash.u64(plan.exhaustion_transfer.normalized_identity());
    hash.u64(plan.validation_receipt.normalized_identity());
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFuelExecutionEnvironment {
    Hosted {
        target: NativeTarget,
        interpreter_available: bool,
    },
    Freestanding {
        target: NativeTarget,
    },
}

impl NativeFuelExecutionEnvironment {
    const fn target(self) -> NativeTarget {
        match self {
            Self::Hosted { target, .. } | Self::Freestanding { target } => target,
        }
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
            if plan.target != environment.target() {
                return Err(ExternalRootDiagnostic(
                    "dynamic native fuel plan does not match the selected target".into(),
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
                Some(plan.target),
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
/// is incomplete without the exact installed attribution binding; fixed and
/// interpreted selections reject a stray binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledNativeFuelRealization {
    selected: ValidatedNativeFuelRealization,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    dynamic_attribution: Option<InstalledDynamicFuelAttributionPlan>,
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
) -> Result<InstalledNativeFuelRealization, ExternalRootDiagnostic> {
    if !selected.matches_resource(demand, provision, granted_units) {
        return Err(ExternalRootDiagnostic(
            "native fuel selection does not match the exact installed resource demand, provision, and grant"
                .into(),
        ));
    }
    match selected.kind {
        NativeFuelRealizationKind::FixedProvision => {
            if dynamic_attribution.is_some() || selected.dynamic_plan.is_some() {
                return Err(ExternalRootDiagnostic(
                    "fixed native fuel provision cannot retain dynamic attribution".into(),
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
        }
        NativeFuelRealizationKind::Interpreted => {
            if dynamic_attribution.is_some() || selected.dynamic_plan.is_some() {
                return Err(ExternalRootDiagnostic(
                    "interpreted fuel realization cannot retain native dynamic attribution".into(),
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
    );
    Ok(InstalledNativeFuelRealization {
        selected,
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        dynamic_attribution,
        fingerprint,
    })
}

fn fingerprint_installed_native_fuel_realization(
    selected: &ValidatedNativeFuelRealization,
    installed_code: &InstalledCode,
    dynamic_attribution: Option<&InstalledDynamicFuelAttributionPlan>,
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
        let sponsor_path = bind_suspension_free_fixed_fuel(sponsor_fixed, sponsor_free)
            .expect("exact fixed/suspension join");
        let target = NativeTarget::linux_x64();
        let plan = DynamicNativeFuelMeterPlan::from_admitted_target(
            target,
            schedule(),
            id(24, NativeFuelMeterPlanId::from_normalized_identity),
            id(25, SponsorContextTransportId::from_normalized_identity),
            id(26, FuelExhaustionTransferPlanId::from_normalized_identity),
            sponsor_path,
            id(
                27,
                DynamicFuelMeterValidationReceiptId::from_normalized_identity,
            ),
        );
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
        validate_dynamic_fuel_attributions(&plan, &artifact, &rows)
            .expect("zero-code and byte-bearing attribution sites are valid meter inputs");
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
                target,
                interpreter_available: true,
            },
            NativeFuelRealizationRequest::Dynamic(&plan),
        )
        .expect("admitted dynamic meter");
        assert_eq!(realized.kind(), NativeFuelRealizationKind::DynamicMetering);
        assert_eq!(realized.maximum_logical_work(), 20);
        assert_eq!(realized.granted_units(), 3);
        assert_eq!(realized.dynamic_plan(), Some(&plan));

        let wrong_target = admit_native_fuel_realization(
            &runtime_demand,
            runtime_provision,
            3,
            NativeFuelExecutionEnvironment::Freestanding {
                target: NativeTarget::linux_arm64(),
            },
            NativeFuelRealizationRequest::Dynamic(&plan),
        )
        .expect_err("a target meter plan cannot be transplanted");
        assert!(wrong_target.0.contains("selected target"));
    }

    #[test]
    fn unavailable_native_fuel_interprets_only_on_an_enabled_host() {
        let (demand, _) = opaque_demand(30, 31, 32, 5);
        let provision = id(33, FuelProvisionId::from_normalized_identity);
        let target = NativeTarget::linux_x64();
        let interpreted = admit_native_fuel_realization(
            &demand,
            provision,
            2,
            NativeFuelExecutionEnvironment::Hosted {
                target,
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
                target,
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
            NativeFuelExecutionEnvironment::Freestanding { target },
            NativeFuelRealizationRequest::Unavailable,
        )
        .expect_err("freestanding targets reject unavailable metering");
        assert!(freestanding.0.contains("freestanding installation"));
    }
}

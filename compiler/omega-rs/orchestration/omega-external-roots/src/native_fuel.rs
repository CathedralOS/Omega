//! Native realization selection for one sponsor-owned logical-fuel region.
//!
//! Fixed provision is an exact maximum-work/grant proof. Dynamic metering is
//! target-owned admitted instrumentation with a private context transport and
//! an independently fixed-provisioned, suspension-free exhaustion path.
//! Hosted interpretation is an explicit fallback; freestanding installation
//! never treats unavailable native metering as executable.

use omega_target::NativeTarget;

use super::{
    ComposedFuelDemand, DynamicFuelMeterValidationReceiptId, ExternalRootDiagnostic,
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
    environment: NativeFuelExecutionEnvironment,
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

    pub const fn environment(&self) -> NativeFuelExecutionEnvironment {
        self.environment
    }

    pub const fn dynamic_plan(&self) -> Option<&DynamicNativeFuelMeterPlan> {
        self.dynamic_plan.as_ref()
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
    let (kind, dynamic_plan) = match request {
        NativeFuelRealizationRequest::Fixed(fixed) => {
            if !fixed.matches(demand, provision, granted_units) {
                return Err(ExternalRootDiagnostic(
                    "fixed native fuel evidence does not match the exact sponsor region, provision, and grant"
                        .into(),
                ));
            }
            (NativeFuelRealizationKind::FixedProvision, None)
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
            )
        }
        NativeFuelRealizationRequest::Unavailable => match environment {
            NativeFuelExecutionEnvironment::Hosted {
                interpreter_available: true,
                ..
            } => (NativeFuelRealizationKind::Interpreted, None),
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
        environment,
        kind,
        dynamic_plan,
    })
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

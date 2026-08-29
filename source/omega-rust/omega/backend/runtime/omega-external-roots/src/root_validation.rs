use std::collections::BTreeSet;

use omega_calling_conventions::{
    BoundaryEntryPlan, MachineRegister, StateFootprintEvidence, ValidatedBoundaryEntryPlan,
    validate_state_footprint,
};
use omega_effects::{
    InstallationReachResolution, SelectedProviderClosureDigest, SelectedProviderPlanFacts,
    provider_plan::ProviderPlanDigest,
};
use psi_layout_plans::EntryStubId;
use psi_terminal::{ServiceDeclaration, TerminalModule, TerminalRootServiceReach};

use super::{
    AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
    ComponentVersionPinId, ExternalRootDiagnostic, ExternalRootId, Fnv1a,
    LogicalFuelResourceColumn, NestingRelationId, ProviderPlanId, RootEffectId, RootProviderId,
    StackResourceColumn, StateValidationReceiptId, TrustReceiptId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentVersionPin {
    pub contract: ComponentContractId,
    pub artifact: ComponentArtifactId,
    pub provider: ComponentProviderId,
    pub version: ComponentVersionPinId,
}

/// The `StatePlan` itself is the public ceiling. This column retains only the
/// final transitive footprint that refined it and the public validation
/// receipt; instruction-selection/allocation derivations stay private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineStateResourceColumn {
    pub realization: StateFootprintEvidence,
    pub validation_receipt: StateValidationReceiptId,
}

/// One source qualification accepted by the exact external-root requirement.
///
/// The compiler constructs these rows from the selected provider schema. The
/// runtime ledger retains them structurally so an invocation receipt can bind
/// a concrete parameter subject without parsing a type-display string or
/// trusting the provider to restate the admitted contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootEntryClaim {
    pub parameter_index: usize,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
}

/// One routed result qualification supplied by an independently selected
/// boundary provider used during an external-root invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootResultClaim {
    pub provider_plan: ProviderPlanId,
    /// Strong identity derived from the exact compiler-selected provider plan.
    /// The compact ID remains a report coordinate only.
    pub provider_plan_digest: ProviderPlanDigest,
    pub requirement_identity: String,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
}

/// Final, source-handle-free service reach of one installed-root closure.
///
/// The concrete row comes from checked code. Installation-bound requirements
/// remain nominal dependencies until provider selection supplies each exact
/// realization row. Construction fails closed on an absent selection; the
/// published root therefore never substitutes an authored upper bound for a
/// provider's actual reach.
///
/// The compact selected-closure value is report compatibility only. The
/// collision-resistant closure digest and exact resolution rows remain beside
/// it for retained-root replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRootServiceReach {
    concrete: Vec<String>,
    installation_requirements: Vec<String>,
    resolutions: Vec<InstallationReachResolution>,
    effective: Vec<String>,
    selected_provider_closure_report_fingerprint: u64,
    selected_provider_closure_digest: SelectedProviderClosureDigest,
}

impl ResolvedRootServiceReach {
    /// Close the exact source-free Terminal Psi root reach against the selected
    /// provider plans. The dependency bound carried by the semantic artifact
    /// must agree with the selected-resolution bound; installation never
    /// reconstructs concrete reach by subtracting conservative bounds.
    pub fn from_module(
        module: &TerminalModule,
        selected: &SelectedProviderPlanFacts,
    ) -> Result<Self, ExternalRootDiagnostic> {
        Self::from_root_service_reach(&module.root_service_reach, &module.services, selected)
    }

    pub fn from_root_service_reach(
        root_reach: &TerminalRootServiceReach,
        services: &[ServiceDeclaration],
        selected: &SelectedProviderPlanFacts,
    ) -> Result<Self, ExternalRootDiagnostic> {
        let service_identity = |service| {
            services
                .iter()
                .find(|declaration| declaration.id == service)
                .map(|declaration| declaration.identity.clone())
                .ok_or_else(|| {
                    ExternalRootDiagnostic(
                        "terminal root service reach references an unknown service identity".into(),
                    )
                })
        };
        let concrete = root_reach
            .concrete
            .iter()
            .map(|service| service_identity(*service))
            .collect::<Result<Vec<_>, _>>()?;
        let mut requirements = Vec::with_capacity(root_reach.installation_dependencies.len());
        for dependency in &root_reach.installation_dependencies {
            let mut terminal_bound = dependency
                .upper_bound
                .iter()
                .map(|service| service_identity(*service))
                .collect::<Result<Vec<_>, _>>()?;
            terminal_bound.sort();
            terminal_bound.dedup();
            let resolution = selected
                .installation_reach_resolution(&dependency.requirement_identity)
                .ok_or_else(|| {
                    ExternalRootDiagnostic(format!(
                        "installation reach requirement `{}` remains unresolved at final admission",
                        dependency.requirement_identity
                    ))
                })?;
            if resolution.upper_bound != terminal_bound {
                return Err(ExternalRootDiagnostic(format!(
                    "installation reach requirement `{}` changed its published upper bound before final admission",
                    dependency.requirement_identity
                )));
            }
            requirements.push(dependency.requirement_identity.clone());
        }
        Self::from_selected_provider_closure(concrete, requirements, selected)
    }

    pub fn from_selected_provider_closure(
        mut concrete: Vec<String>,
        mut installation_requirements: Vec<String>,
        selected: &SelectedProviderPlanFacts,
    ) -> Result<Self, ExternalRootDiagnostic> {
        if concrete.iter().any(String::is_empty) {
            return Err(ExternalRootDiagnostic(
                "external-root concrete service reach contains an empty service identity".into(),
            ));
        }
        concrete.sort();
        concrete.dedup();

        if installation_requirements.iter().any(String::is_empty) {
            return Err(ExternalRootDiagnostic(
                "external-root installation reach contains an empty requirement identity".into(),
            ));
        }
        installation_requirements.sort();
        if installation_requirements
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(ExternalRootDiagnostic(
                "external-root installation reach requirements must be unique".into(),
            ));
        }

        let effective = selected
            .resolve_installation_reach(&concrete, &installation_requirements)
            .map_err(ExternalRootDiagnostic)?;
        let resolutions = installation_requirements
            .iter()
            .map(|requirement| {
                selected
                    .installation_reach_resolution(requirement)
                    .cloned()
                    .ok_or_else(|| {
                        ExternalRootDiagnostic(format!(
                            "installation reach requirement `{requirement}` remains unresolved at final admission"
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            concrete,
            installation_requirements,
            resolutions,
            effective,
            selected_provider_closure_report_fingerprint: selected.compatibility_report_identity(),
            selected_provider_closure_digest: selected.identity_digest(),
        })
    }

    pub fn concrete(&self) -> &[String] {
        &self.concrete
    }

    pub fn installation_requirements(&self) -> &[String] {
        &self.installation_requirements
    }

    pub fn resolutions(&self) -> &[InstallationReachResolution] {
        &self.resolutions
    }

    pub fn effective(&self) -> &[String] {
        &self.effective
    }

    pub const fn selected_provider_closure_report_fingerprint(&self) -> u64 {
        self.selected_provider_closure_report_fingerprint
    }

    pub const fn selected_provider_closure_digest(&self) -> SelectedProviderClosureDigest {
        self.selected_provider_closure_digest
    }
}

/// Provider-independent facts required for one externally invoked entry.
///
/// Effects and receipts are normalized open sets. The concrete interrupt,
/// firmware, syscall, or callback package owns their vocabulary; the ledger
/// only requires that admission bind the exact sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRootCandidate {
    pub identity: ExternalRootId,
    pub entry: EntryStubId,
    pub provider: RootProviderId,
    /// Exact normalized compiler-selected provider plan that supplies this
    /// root. Validation binds it into the root identity before execution or
    /// slot admission can be constructed.
    pub provider_plan: ProviderPlanId,
    /// Strong identity derived from the exact compiler-selected provider plan.
    /// The compact ID remains a report coordinate only.
    pub provider_plan_digest: ProviderPlanDigest,
    /// Stable identity of the exact boundary requirement implemented by this
    /// entry stub, not merely the containing provider schema.
    pub requirement_identity: String,
    /// Compiler-owned accepted qualification rows for that one requirement.
    /// Validation requires canonical ordering and rejects duplicate claims.
    pub entry_claims: Vec<ExternalRootEntryClaim>,
    /// Parameter whose concrete subject is the provider-minted interrupt
    /// acknowledgement. `None` is valid for roots without that obligation.
    pub acknowledgement_parameter_index: Option<usize>,
    /// Exact routed result contract used when this root's mask control saves
    /// and masks the current invocation. It belongs to the independently
    /// selected mask-control provider, not implicitly to the root provider.
    pub interrupt_mask_guard_claim: Option<ExternalRootResultClaim>,
    /// Fully selected service reach for this exact root closure. This carrier
    /// is constructed only after every installation-bound requirement has an
    /// exact selected-provider resolution.
    pub service_reach: ResolvedRootServiceReach,
    pub effects: BTreeSet<RootEffectId>,
    pub trust_receipts: BTreeSet<TrustReceiptId>,
    /// Identity of the artifact-wide relation that names which other roots may
    /// preempt this one. Stack class and maximum depth remain the one copy in
    /// `BoundaryEntryPlan::state`; they are not re-authored here.
    pub nesting_relation: NestingRelationId,
    pub acknowledgement_policy: Option<AcknowledgementPolicyId>,
    pub stack: StackResourceColumn,
    pub logical_fuel: LogicalFuelResourceColumn,
    pub machine_state: MachineStateResourceColumn,
    pub component_pins: BTreeSet<ComponentVersionPin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExternalRoot {
    pub(crate) candidate: ExternalRootCandidate,
    pub(crate) boundary: ValidatedBoundaryEntryPlan,
    pub(crate) boundary_contract_report_fingerprint: u64,
    pub(crate) normalized_report_identity: u64,
}

impl ValidatedExternalRoot {
    pub const fn candidate(&self) -> &ExternalRootCandidate {
        &self.candidate
    }

    pub const fn boundary(&self) -> &BoundaryEntryPlan {
        self.boundary.plan()
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }

    pub const fn normalized_report_identity(&self) -> u64 {
        self.normalized_report_identity
    }
}

pub fn validate_external_root(
    candidate: ExternalRootCandidate,
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<ValidatedExternalRoot, ExternalRootDiagnostic> {
    if candidate.requirement_identity.is_empty() {
        return Err(ExternalRootDiagnostic(
            "external-root requirement identity cannot be empty".into(),
        ));
    }
    let mut prior_claim: Option<(usize, &str)> = None;
    for claim in &candidate.entry_claims {
        if claim.domain.is_empty() {
            return Err(ExternalRootDiagnostic(
                "external-root entry claim domain identity cannot be empty".into(),
            ));
        }
        let key = (claim.parameter_index, claim.domain.as_str());
        if prior_claim.is_some_and(|prior| prior >= key) {
            return Err(ExternalRootDiagnostic(
                "external-root entry claims must be uniquely sorted by parameter and domain".into(),
            ));
        }
        if boundary
            .plan()
            .call
            .parameters
            .get(claim.parameter_index)
            .is_none()
        {
            return Err(ExternalRootDiagnostic(format!(
                "external-root entry claim parameter {} has no exact ABI placement in the validated boundary plan",
                claim.parameter_index
            )));
        }
        prior_claim = Some(key);
    }
    if let Some(parameter_index) = candidate.acknowledgement_parameter_index
        && !candidate
            .entry_claims
            .iter()
            .any(|claim| claim.parameter_index == parameter_index)
    {
        return Err(ExternalRootDiagnostic(
            "external-root acknowledgement parameter has no accepted qualification claim".into(),
        ));
    }
    if candidate
        .interrupt_mask_guard_claim
        .as_ref()
        .is_some_and(|claim| claim.requirement_identity.is_empty() || claim.domain.is_empty())
    {
        return Err(ExternalRootDiagnostic(
            "interrupt-mask guard claim requires exact requirement and domain identities".into(),
        ));
    }
    if candidate.stack.ceiling_bytes == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root stack ceiling must be nonzero".into(),
        ));
    }
    let Some(stack_input) = candidate.stack.realization.input(candidate.identity) else {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not name the candidate root".into(),
        ));
    };
    if stack_input.provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization provider does not match the selected provider".into(),
        ));
    }
    if candidate.stack.realization.relation().identity != candidate.nesting_relation {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not use the selected nesting relation".into(),
        ));
    }
    if stack_input
        .realization_evidence()
        .boundary_contract_report_fingerprint()
        != boundary.contract_fingerprint()
        || stack_input
            .realization_evidence()
            .boundary_contract_commitment()
            == [0; 32]
        || stack_input
            .realization_evidence()
            .boundary_contract_commitment()
            != boundary.contract_commitment_digest()
    {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not match the validated boundary contract".into(),
        ));
    }
    let stack_demand = candidate
        .stack
        .realization
        .demand(candidate.identity)
        .expect("bound stack input has a composed demand");
    if stack_demand
        .domains()
        .any(|(_, demand)| demand.bytes > candidate.stack.ceiling_bytes)
    {
        return Err(ExternalRootDiagnostic(
            "external-root composed WCSU exceeds the admitted stack ceiling".into(),
        ));
    }
    if candidate.logical_fuel.ceiling_units == 0 {
        return Err(ExternalRootDiagnostic(
            "external-root logical-fuel ceiling must be nonzero".into(),
        ));
    }
    if candidate.logical_fuel.schedule != candidate.logical_fuel.realization.schedule() {
        return Err(ExternalRootDiagnostic(
            "external-root fuel provision and realization use different schedule versions".into(),
        ));
    }
    if candidate.logical_fuel.realization.units() > candidate.logical_fuel.ceiling_units {
        return Err(ExternalRootDiagnostic(
            "external-root composed logical fuel exceeds the admitted ceiling".into(),
        ));
    }
    if candidate.logical_fuel.realization.root_provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root logical-fuel root provider does not match the selected provider".into(),
        ));
    }
    validate_state_footprint(boundary, &candidate.machine_state.realization).map_err(|error| {
        ExternalRootDiagnostic(format!(
            "external-root machine-state realization is invalid: {error}"
        ))
    })?;
    let mut component_contracts = BTreeSet::new();
    for pin in &candidate.component_pins {
        if !component_contracts.insert(pin.contract) {
            return Err(ExternalRootDiagnostic(
                "external root cannot pin more than one realization of one component contract"
                    .into(),
            ));
        }
    }

    let boundary_contract_report_fingerprint = boundary.contract_fingerprint();
    let normalized_report_identity =
        root_report_fingerprint(&candidate, boundary_contract_report_fingerprint);
    Ok(ValidatedExternalRoot {
        candidate,
        boundary: boundary.clone(),
        boundary_contract_report_fingerprint,
        normalized_report_identity,
    })
}

fn root_report_fingerprint(candidate: &ExternalRootCandidate, boundary: u64) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(candidate.provider_plan.normalized_identity());
    hash.bytes(candidate.provider_plan_digest.as_bytes());
    hash.string(&candidate.requirement_identity);
    hash.u64(candidate.entry_claims.len() as u64);
    for claim in &candidate.entry_claims {
        hash.u64(claim.parameter_index as u64);
        hash.string(&claim.domain);
        fingerprint_carry_policy(&mut hash, claim.effective_carry);
    }
    hash.u64(
        candidate
            .acknowledgement_parameter_index
            .map(|index| index as u64 + 1)
            .unwrap_or_default(),
    );
    match &candidate.interrupt_mask_guard_claim {
        Some(claim) => {
            hash.u64(1);
            hash.u64(claim.provider_plan.normalized_identity());
            hash.bytes(claim.provider_plan_digest.as_bytes());
            hash.string(&claim.requirement_identity);
            hash.string(&claim.domain);
            fingerprint_carry_policy(&mut hash, claim.effective_carry);
        }
        None => hash.u64(0),
    }
    hash.u64(
        candidate
            .service_reach
            .selected_provider_closure_report_fingerprint(),
    );
    hash.bytes(
        candidate
            .service_reach
            .selected_provider_closure_digest()
            .as_bytes(),
    );
    hash.u64(candidate.service_reach.concrete().len() as u64);
    for service in candidate.service_reach.concrete() {
        hash.string(service);
    }
    hash.u64(candidate.service_reach.installation_requirements().len() as u64);
    for resolution in candidate.service_reach.resolutions() {
        hash.string(&resolution.requirement_identity);
        hash.u64(resolution.provider_plan_identity);
        hash.u64(resolution.upper_bound.len() as u64);
        for service in &resolution.upper_bound {
            hash.string(service);
        }
        hash.u64(resolution.resolved_row.len() as u64);
        for service in &resolution.resolved_row {
            hash.string(service);
        }
    }
    hash.u64(candidate.service_reach.effective().len() as u64);
    for service in candidate.service_reach.effective() {
        hash.string(service);
    }
    hash.u64(boundary);
    hash.u64(candidate.nesting_relation.normalized_identity());
    hash.u64(
        candidate
            .acknowledgement_policy
            .map(AcknowledgementPolicyId::normalized_identity)
            .unwrap_or_default(),
    );
    hash.u64(candidate.stack.ceiling_bytes);
    hash.u64(candidate.stack.realization.report_fingerprint());
    hash.u64(candidate.stack.validation_receipt.normalized_identity());
    hash.u64(u64::from(candidate.logical_fuel.schedule.marker()));
    hash.u64(candidate.logical_fuel.provision.normalized_identity());
    hash.u64(candidate.logical_fuel.ceiling_units);
    hash.u64(
        candidate
            .logical_fuel
            .realization
            .root()
            .normalized_identity(),
    );
    hash.u64(
        candidate
            .logical_fuel
            .realization
            .root_provider()
            .normalized_identity(),
    );
    hash.u64(candidate.logical_fuel.realization.units());
    hash.u64(
        candidate
            .logical_fuel
            .realization
            .non_authoritative_composition_report_fingerprint(),
    );
    hash.u64(
        candidate
            .logical_fuel
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(
        candidate
            .machine_state
            .realization
            .machine_state()
            .bits()
            .into(),
    );
    hash.u64(
        candidate
            .machine_state
            .realization
            .registers()
            .as_slice()
            .len() as u64,
    );
    for register in candidate.machine_state.realization.registers().as_slice() {
        hash.u64(machine_register_identity(*register));
    }
    hash.u64(
        candidate
            .machine_state
            .validation_receipt
            .normalized_identity(),
    );
    hash.u64(candidate.effects.len() as u64);
    for effect in &candidate.effects {
        hash.u64(effect.normalized_identity());
    }
    hash.u64(0xff01);
    hash.u64(candidate.trust_receipts.len() as u64);
    for receipt in &candidate.trust_receipts {
        hash.u64(receipt.normalized_identity());
    }
    hash.u64(0xff02);
    hash.u64(candidate.component_pins.len() as u64);
    for pin in &candidate.component_pins {
        hash.u64(pin.contract.normalized_identity());
        hash.u64(pin.artifact.normalized_identity());
        hash.u64(pin.provider.normalized_identity());
        hash.u64(pin.version.normalized_identity());
    }
    hash.finish()
}

fn machine_register_identity(register: MachineRegister) -> u64 {
    match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(index) => 0x100 + u64::from(index),
        MachineRegister::Aarch64X(index) => 0x200 + u64::from(index),
        MachineRegister::Aarch64V(index) => 0x300 + u64::from(index),
    }
}

fn fingerprint_carry_policy(hash: &mut Fnv1a, policy: psi_language_semantics::CarryPolicy) {
    use psi_language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    hash.u64(match policy.suspension {
        CarrySuspension::Forbidden => 0,
        CarrySuspension::Allowed => 1,
    });
    hash.u64(match policy.cpu {
        CarryCpu::Origin => 0,
        CarryCpu::Any => 1,
    });
    hash.u64(match policy.host_thread {
        CarryHostThread::Origin => 0,
        CarryHostThread::Any => 1,
    });
    hash.u64(match policy.address {
        CarryAddress::Stable => 0,
        CarryAddress::Movable => 1,
    });
}

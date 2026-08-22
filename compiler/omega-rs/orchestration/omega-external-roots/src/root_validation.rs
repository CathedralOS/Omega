use std::collections::BTreeSet;

use omega_calling_conventions::{
    BoundaryEntryPlan, MachineRegister, StateFootprintEvidence, ValidatedBoundaryEntryPlan,
    validate_state_footprint,
};
use psi_layout_plans::EntryStubId;

use super::stack_demand::fingerprint_entry_stack;
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
    pub requirement_identity: String,
    pub domain: String,
    pub effective_carry: psi_language_semantics::CarryPolicy,
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
    pub(crate) boundary_contract_fingerprint: u64,
    pub(crate) normalized_identity: u64,
}

impl ValidatedExternalRoot {
    pub const fn candidate(&self) -> &ExternalRootCandidate {
        &self.candidate
    }

    pub const fn boundary(&self) -> &BoundaryEntryPlan {
        self.boundary.plan()
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
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
    if candidate.stack.realization.root() != candidate.identity {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not name the candidate root".into(),
        ));
    }
    if candidate.stack.realization.root_provider() != candidate.provider {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization provider does not match the selected provider".into(),
        ));
    }
    if candidate.stack.realization.relation() != candidate.nesting_relation {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not use the selected nesting relation".into(),
        ));
    }
    if candidate.stack.realization.stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "external-root stack realization does not match the boundary StatePlan stack".into(),
        ));
    }
    if candidate.stack.realization.composed_wcsu_bytes() > candidate.stack.ceiling_bytes {
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

    let boundary_contract_fingerprint = boundary.contract_fingerprint();
    let normalized_identity = fingerprint_root(&candidate, boundary_contract_fingerprint);
    Ok(ValidatedExternalRoot {
        candidate,
        boundary: boundary.clone(),
        boundary_contract_fingerprint,
        normalized_identity,
    })
}

fn fingerprint_root(candidate: &ExternalRootCandidate, boundary: u64) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(candidate.identity.normalized_identity());
    hash.u64(candidate.entry.normalized_identity());
    hash.u64(candidate.provider.normalized_identity());
    hash.u64(candidate.provider_plan.normalized_identity());
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
            hash.string(&claim.requirement_identity);
            hash.string(&claim.domain);
            fingerprint_carry_policy(&mut hash, claim.effective_carry);
        }
        None => hash.u64(0),
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
    hash.u64(candidate.stack.realization.root().normalized_identity());
    hash.u64(
        candidate
            .stack
            .realization
            .root_provider()
            .normalized_identity(),
    );
    hash.u64(candidate.stack.realization.relation().normalized_identity());
    fingerprint_entry_stack(&mut hash, candidate.stack.realization.stack());
    hash.u64(candidate.stack.realization.local_wcsu_bytes());
    hash.u64(candidate.stack.realization.composed_wcsu_bytes());
    hash.u64(candidate.stack.realization.wcsu_alignment());
    hash.u64(
        candidate
            .stack
            .realization
            .artifact_composition_fingerprint(),
    );
    hash.u64(candidate.stack.realization.composition_fingerprint());
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
    hash.u64(candidate.logical_fuel.realization.composition_fingerprint());
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

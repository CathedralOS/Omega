//! Canonical root, component and host provisions for a Terminal interpreter.
//! Native code and mutable compilation products stay with the publisher.
#![forbid(unsafe_code)]

use calling_conventions::BoundaryCallingApplication;
use native_stack_demand::StackDemand;
use semantic_vocabulary::MachineId;
use std::sync::Arc;
use terminal_psi::TerminalPsiIdentity;
pub use verified_optimization_input::runtime_provisions::TerminalRuntimeHostProvision;
use verified_optimization_input::runtime_provisions::{
    NativeRuntimePlan, TerminalCallableRuntimeProvisions, TerminalRuntimeHostProvisions,
};

pub type RuntimeAdapterIdentity = Arc<calling_conventions::PublishedForeignAdapterIdentity>;
pub type RuntimeHostProvisions = TerminalRuntimeHostProvisions<RuntimeAdapterIdentity>;

/// The interpreter receives the admitted component's canonical artifact, its
/// checked entry and the corresponding host records. The image stays with its
/// publisher and the native sponsor.
#[derive(Debug, Clone)]
pub struct RuntimeComponent {
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    selection: psi_provider_selections::AdmittedComponentProvider,
    application: BoundaryCallingApplication,
    runtime: TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity>,
    authority: provider_admission::AdmittedNativeCallableAuthority,
    locator: target::NormalizedComponentLocator,
    plan: provider_stack_contribution::SameStackProviderPlanCommitment,
    stack_bytes: u64,
    stack_alignment: u32,
}

impl RuntimeComponent {
    pub fn admit(
        call: &verified_optimization_input::AdmittedNativeComponentCall<'_>,
        runtime: TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity>,
        authority: provider_admission::AdmittedNativeCallableAuthority,
        demand: &StackDemand,
        image_contents: &[u8],
        export_slot: [u8; 32],
    ) -> Result<Self, &'static str> {
        let selection = call.selection();
        let application = call.application();
        let artifact = call.component().shared_artifact();
        let locator = target::NormalizedComponentLocator::for_image(
            runtime.target(),
            image_contents,
            export_slot,
        )?;
        let stack_bytes = demand
            .ceiling_bytes()
            .ok_or("component calls require a finite stack demand without hosted imports")?;
        let stack_alignment = demand.stack_alignment();
        if selection.provider() != runtime.identity()
            || selection.provider() != authority.subject()
            || !runtime.matches_artifact(artifact)
            || selection.candidate() != runtime.entry()
            || selection.candidate() != authority.entry()
            || runtime.target().native_target() != application.target
            || authority.review().target() != application.target
            || authority.review().terminal_artifact_identity()
                != *artifact.manifest().identity().as_bytes()
            || application.recomputed_commitment().ok() != Some(application.commitment)
            || demand.psi() != selection.provider()
            || demand.entry() != selection.candidate()
            || demand.target() != application.target
            || *locator.slot_commitment()
                != target::component_export_slot(
                    selection.provider().vocabulary_marker.get(),
                    selection.provider().program_fingerprint.as_bytes(),
                    &application.commitment,
                )
            || !stack_alignment.is_power_of_two()
        {
            return Err(
                "component publication product lost its admitted artifact, entry, target or complete ABI application",
            );
        }
        Ok(Self {
            artifact: Arc::clone(artifact),
            selection: selection.clone(),
            application: application.clone(),
            runtime,
            authority,
            locator,
            plan: provider_stack_contribution::SameStackProviderPlanCommitment::from_digest(
                *call.plan_digest().as_bytes(),
            ),
            stack_bytes,
            stack_alignment,
        })
    }
    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub fn selection(&self) -> &psi_provider_selections::AdmittedComponentProvider {
        &self.selection
    }
    pub fn locator(&self) -> &target::NormalizedComponentLocator {
        &self.locator
    }
    pub fn application(&self) -> &BoundaryCallingApplication {
        &self.application
    }

    pub fn runtime(&self) -> &TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity> {
        &self.runtime
    }
    pub fn authority(
        &self,
    ) -> &provider_admission::AdmittedNativeCallableAuthority {
        &self.authority
    }
    pub const fn stack_bytes(&self) -> u64 {
        self.stack_bytes
    }

    pub const fn stack_alignment(&self) -> u32 {
        self.stack_alignment
    }

    pub fn validate_same_stack(
        &self,
        contribution: &provider_stack_contribution::AdmittedSameStackContribution,
    ) -> Result<(), &'static str> {
        if contribution.provider_plan_commitment() != self.plan
            || contribution.bytes() != self.stack_bytes
            || contribution.alignment() != u64::from(self.stack_alignment)
        {
            return Err(
                "component same-stack contribution differs from its actual image demand or exact provider plan",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionProvisions {
    plan: NativeRuntimePlan<RuntimeAdapterIdentity>,
    components: Result<Vec<Arc<RuntimeComponent>>, &'static str>,
}
impl RuntimeExecutionProvisions {
    pub fn from_plan(
        plan: NativeRuntimePlan<RuntimeAdapterIdentity>,
        components: Vec<Arc<RuntimeComponent>>,
    ) -> Self {
        let components = validate_component_roster(&plan, components.iter().map(AsRef::as_ref))
            .map(|()| components);
        Self { plan, components }
    }

    pub fn component_products(&self) -> Result<&[Arc<RuntimeComponent>], &'static str> {
        self.components.as_deref().map_err(|reason| *reason)
    }
    pub fn entry(&self) -> MachineId {
        self.plan.entry()
    }
    pub fn identity(&self) -> TerminalPsiIdentity {
        self.plan.identity()
    }
    pub fn matches_artifact(&self, artifact: &terminal_codec::CanonicalTerminalArtifact) -> bool {
        self.plan.matches_artifact(artifact)
    }
    pub fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.plan.target_slot()
    }
    pub fn installation(&self) -> Option<&psi_provider_selections::AdmittedProviderSelections> {
        self.plan.installation()
    }
    pub fn scalar_applications(
        &self,
    ) -> Option<&psi_scalar_applications::AdmittedScalarApplications> {
        self.plan.scalar_applications()
    }
    pub fn host_provisions(&self) -> &RuntimeHostProvisions {
        self.plan.host_provisions()
    }
    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        self.plan.host()
    }
    pub fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe] {
        self.plan.private_entry_ram()
    }
    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        self.plan.borrowed_storage_layouts()
    }
}

/// Check that each selected component names its actual receiving boundary,
/// application, published slot, provider and same-stack commitment.
pub fn validate_component_roster<'a, A>(
    plan: &NativeRuntimePlan<A>,
    components: impl Iterator<Item = &'a RuntimeComponent> + Clone,
) -> Result<(), &'static str> {
    let mut expected = 0;
    for provision in plan.host() {
        let target_vocabulary::BoundarySettlementRealization::NormalizedForeignCall(binding) =
            &provision.realization
        else {
            continue;
        };
        let target_vocabulary::NormalizedForeignCallMechanism::Component {
            locator,
            provider,
            candidate,
            application_commitment,
            same_stack_contribution,
        } = &binding.mechanism
        else {
            continue;
        };
        expected += 1;
        let mut matches = components.clone().filter(|component| {
            component.selection().receiver()
                == &terminal_codec::TerminalExecutionIdentity::portable(plan.identity())
                && component.selection().receiving_boundary() == provision.boundary
                && component.selection().provider() == *provider
                && component.selection().candidate() == *candidate
                && &component.locator == locator
                && component.application.commitment == *application_commitment
                && component.application.plan.plan() == &binding.boundary_entry_plan
        });
        let component = matches.next().ok_or(
            "component runtime roster does not exactly cover an admitted independent native call",
        )?;
        if matches.next().is_some() {
            return Err(
                "component runtime roster ambiguously duplicates an admitted independent native call",
            );
        }
        component.validate_same_stack(same_stack_contribution)?;
    }
    if expected != components.count() {
        return Err("component runtime roster contains an unconsumed or duplicated image");
    }
    Ok(())
}

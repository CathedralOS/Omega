//! Immutable inputs to the source-free Terminal interpreter.

use super::*;

pub type RuntimeAdapterIdentity = Arc<calling_conventions::PublishedForeignAdapterIdentity>;
pub type RuntimeHostProvisions = TerminalRuntimeHostProvisions<RuntimeAdapterIdentity>;

/// The interpreter receives the admitted component's canonical artifact, its
/// checked entry and the corresponding host records. The image stays with its
/// publisher and the native sponsor.
#[derive(Debug, Clone)]
pub struct RuntimeComponent {
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    selection: terminal_installation::AdmittedComponentProvider,
    application: BoundaryCallingApplication,
    runtime: TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity>,
    authority: provider_admission::provider_admission::AdmittedNativeCallableAuthority,
    locator: target::NormalizedComponentLocator,
    plan: effects::provider_plan::ProviderPlanDigest,
    stack_bytes: u64,
    stack_alignment: u32,
}

impl RuntimeComponent {
    pub fn admit(
        call: &crate::AdmittedNativeComponentCall<'_>,
        runtime: TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity>,
        authority: provider_admission::provider_admission::AdmittedNativeCallableAuthority,
        locator: target::NormalizedComponentLocator,
        stack_bytes: u64,
        stack_alignment: u32,
    ) -> Result<Self, &'static str> {
        let selection = call.selection();
        let application = call.application();
        let artifact = call.component().shared_artifact();
        if selection.provider() != runtime.identity() || selection.provider() != authority.subject()
            || selection.provider() != artifact.view().identity()
            || selection.candidate() != runtime.entry() || selection.candidate() != authority.entry()
            || runtime.target().native_target() != application.target
            || authority.review().target() != application.target
            || authority.review().terminal_artifact_identity() != *artifact.manifest().identity().as_bytes()
            || locator.target() != runtime.target()
            || *locator.slot_commitment() != target::component_export_slot(selection.provider().vocabulary_marker.get(), selection.provider().program_fingerprint.as_bytes(), &application.commitment)
            || !stack_alignment.is_power_of_two()
        {
            return Err("component publication product lost its admitted artifact, entry, target or complete ABI application");
        }
        Ok(Self { artifact: Arc::clone(artifact), selection: selection.clone(), application: application.clone(), runtime, authority, locator, plan: *call.plan_digest(), stack_bytes, stack_alignment })
    }
    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> { &self.artifact }
    pub fn selection(&self) -> &terminal_installation::AdmittedComponentProvider { &self.selection }
    pub fn runtime(&self) -> &TerminalCallableRuntimeProvisions<RuntimeAdapterIdentity> { &self.runtime }
    pub fn authority(&self) -> &provider_admission::provider_admission::AdmittedNativeCallableAuthority { &self.authority }
    pub fn validate_same_stack(&self, contribution: &task_plans::AdmittedSameStackContribution) -> Result<(), &'static str> {
        if contribution.provider_plan_commitment() != task_plans::SameStackProviderPlanCommitment::from_digest(*self.plan.as_bytes())
            || contribution.bytes() != self.stack_bytes || contribution.alignment() != u64::from(self.stack_alignment)
        { return Err("component same-stack contribution differs from its actual image demand or exact provider plan"); }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionProvisions {
    plan: NativeRuntimePlan<RuntimeAdapterIdentity>,
    components: Vec<Arc<RuntimeComponent>>,
}
impl std::ops::Deref for RuntimeExecutionProvisions {
    type Target = NativeRuntimePlan<RuntimeAdapterIdentity>;
    fn deref(&self) -> &Self::Target { &self.plan }
}
impl RuntimeExecutionProvisions {
    pub fn from_plan(plan: NativeRuntimePlan<RuntimeAdapterIdentity>, components: Vec<Arc<RuntimeComponent>>) -> Self {
        Self { plan, components }
    }
    pub fn component_products(&self) -> &[Arc<RuntimeComponent>] { &self.components }
    pub fn validate_component_product_roster(&self) -> Result<(), &'static str> {
        validate_component_roster(&self.plan, self.components.iter().map(AsRef::as_ref))
    }
}

/// Check that each selected component names its actual receiving boundary,
/// application, published slot, provider and same-stack commitment.
pub fn validate_component_roster<'a, A>(plan: &NativeRuntimePlan<A>, components: impl Iterator<Item = &'a RuntimeComponent> + Clone) -> Result<(), &'static str> {
        let mut expected = 0;
        for provision in plan.host() {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let target_operations::NormalizedForeignCallMechanism::Component {
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

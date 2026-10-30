//! One whole-provider choice policy for invocation and canonical declarations.

use crate::{ProviderDeclaration, ProviderSubject, SelectionProvenance};
use effects::provider_plan::ProviderPlan;

type ProviderSelectionKey =
    language_semantics::declaration_selection::provider_choices::ApplicationKey;

fn provider_slot_key(plan: &ProviderPlan) -> ProviderSelectionKey {
    (
        plan.schema.trait_package_identity,
        plan.schema.identity_text(plan.schema.trait_name).to_owned(),
        plan.schema.root_application_identity().map(str::to_owned),
    )
}

fn selected_subject_keys<Declaration: PartialEq, Origin>(
    selection: &ProviderDeclaration<Declaration, Origin>,
    slots: &[ProviderSelectionKey],
) -> Vec<ProviderSelectionKey> {
    match &selection.subject {
        ProviderSubject::BoundaryTrait(identity)
        | ProviderSubject::BoundaryRequirement(identity) => {
            language_semantics::declaration_selection::provider_choices::selected_application_slots(
                &(
                    identity.package,
                    identity.canonical_path.clone(),
                    identity.application_identity.clone(),
                ),
                slots,
            )
        }
        ProviderSubject::BoundaryOperatorFamily(family) => family
            .coordinates
            .iter()
            .map(|coordinate| {
                (
                    family.package,
                    coordinate.requirement_identity.clone(),
                    None,
                )
            })
            .collect(),
    }
}

fn selected_provider_key<Declaration, Origin>(
    selection: &ProviderDeclaration<Declaration, Origin>,
) -> ProviderSelectionKey {
    (
        selection.provider_type.package,
        selection.provider_type.canonical_path.clone(),
        selection.provider_type.application_identity.clone(),
    )
}

fn provider_plan_key(plan: &ProviderPlan) -> ProviderSelectionKey {
    (
        plan.provider_type_package_identity,
        plan.provider_type.clone(),
        plan.application_identity().map(str::to_owned),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectedProviderPlanIndex<Declaration, Origin> {
    pub(crate) candidate: usize,
    pub(crate) selected_by: SelectionProvenance<Declaration, Origin>,
}

/// The policy consumes one borrowed whole-plan description. ABI realization
/// remains with this adapter and admission, outside the shared chooser.
struct NativeCandidate<'plan> {
    plan: &'plan ProviderPlan,
    slot: ProviderSelectionKey,
    provider: ProviderSelectionKey,
}

impl<'plan> NativeCandidate<'plan> {
    fn new(plan: &'plan ProviderPlan) -> Self {
        Self {
            plan,
            slot: provider_slot_key(plan),
            provider: provider_plan_key(plan),
        }
    }
}

impl language_semantics::declaration_selection::provider_choices::Candidate
    for NativeCandidate<'_>
{
    type Slot = ProviderSelectionKey;
    type Provider = ProviderSelectionKey;
    fn slot(&self) -> &Self::Slot {
        &self.slot
    }
    fn provider(&self) -> &Self::Provider {
        &self.provider
    }
    fn target(&self) -> &str {
        &self.plan.target
    }
    fn has_requirements(&self) -> bool {
        !self.plan.schema.methods.is_empty()
    }
    fn covers_requirements(&self) -> bool {
        self.plan.covers_schema()
    }
    fn receiver_count(&self) -> usize {
        self.plan.schema.receiver_applications.len()
    }
    fn supplies_receiver(&self, demanded: &Self) -> Option<bool> {
        Some(
            self.plan
                .schema
                .supplies_receiver_schema(&demanded.plan.schema),
        )
    }
    fn shares_receiver(&self, other: &Self) -> Option<bool> {
        Some(
            self.plan
                .schema
                .shares_receiver_application(&other.plan.schema),
        )
    }
}

struct NativeDeclaration<'choice, Declaration, Origin> {
    choice: &'choice ProviderDeclaration<Declaration, Origin>,
    provider: ProviderSelectionKey,
}
impl<'choice, Declaration, Origin> NativeDeclaration<'choice, Declaration, Origin> {
    fn new(choice: &'choice ProviderDeclaration<Declaration, Origin>) -> Self {
        Self {
            choice,
            provider: selected_provider_key(choice),
        }
    }
}
impl<Declaration: PartialEq, Origin>
    language_semantics::declaration_selection::provider_choices::Declaration<
        ProviderSelectionKey,
        ProviderSelectionKey,
    > for NativeDeclaration<'_, Declaration, Origin>
{
    fn target(&self) -> Option<&str> {
        self.choice.target.map(|target| target.canonical_name())
    }
    fn slots(&self, available: &[ProviderSelectionKey]) -> Vec<ProviderSelectionKey> {
        selected_subject_keys(self.choice, available)
    }
    fn provider(&self) -> &ProviderSelectionKey {
        &self.provider
    }
}

pub(crate) fn select_provider_plan_indices<Declaration: Clone + Eq, Origin: Clone + Eq>(
    plans: &[&effects::provider_plan::ProviderPlan],
    selected_target: target::TargetProfile,
    defaults: &[ProviderDeclaration<Declaration, Origin>],
    requested: &[ProviderDeclaration<Declaration, Origin>],
) -> Result<Vec<SelectedProviderPlanIndex<Declaration, Origin>>, Vec<diagnostics::Diagnostic>> {
    use language_semantics::declaration_selection::provider_choices::{self, Error, Reason, Tier};
    let candidates = plans
        .iter()
        .map(|plan| NativeCandidate::new(plan))
        .collect::<Vec<_>>();
    let prepared_defaults = defaults
        .iter()
        .map(NativeDeclaration::new)
        .collect::<Vec<_>>();
    let prepared_requests = requested
        .iter()
        .map(NativeDeclaration::new)
        .collect::<Vec<_>>();
    let outcome = provider_choices::choose(
        &candidates,
        selected_target.target_name(),
        &prepared_defaults,
        &prepared_requests,
    );
    let declaration = |tier: Tier, index: usize| match tier {
        Tier::Override => &requested[index],
        Tier::Default => &defaults[index],
    };
    let owner = |tier: Tier| match tier {
        Tier::Override => "build",
        Tier::Default => "target package",
    };
    let diagnostics = outcome.errors.into_iter().map(|error| {
        let message = match error {
            Error::UnknownSlot { tier, declaration: index, slot } => {
                let declaration = declaration(tier, index);
                let owner = owner(tier);
                match &declaration.subject {
                    ProviderSubject::BoundaryTrait(identity) => format!(
                        "{owner} selects provider `{}` for unknown boundary slot `{}`; the slot must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    ProviderSubject::BoundaryRequirement(identity) => format!(
                        "{owner} selects provider `{}` for unknown top-level boundary requirement `{}`; the requirement must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, identity.authored_path,
                    ),
                    ProviderSubject::BoundaryOperatorFamily(_) => format!(
                        "{owner} selects provider `{}` for unknown boundary coordinate `{}` in subject `{}`; every selected coordinate must exist in the loaded dependency closure",
                        declaration.provider_type.authored_path, slot.1, declaration.subject.authored_path(),
                    ),
                }
            }
            Error::RepeatedOverride { slot, declarations } => format!(
                "build declares provider selection for slot `{}` more than once: {}", slot.1,
                declarations.iter().map(|index| {
                    let declaration = &requested[*index];
                    format!("`{} -> {}`", declaration.subject.authored_path(), declaration.provider_type.authored_path)
                }).collect::<Vec<_>>().join(", "),
            ),
            Error::ConflictingDefaults { slot, providers } => format!(
                "slot `{}` has conflicting target-package defaults: {} -- a target supplies at most one default provider type per slot", slot.1,
                providers.iter().map(|(_, provider, application)| format!("`{provider}`{}", application.as_deref().map(|application| format!("<{application}>")).unwrap_or_default())).collect::<Vec<_>>().join(", "),
            ),
            Error::PartialChoice { tier, declaration: index, candidate } => {
                let declaration = declaration(tier, index);
                let plan = &plans[candidate];
                format!("{} selects provider `{}` for slot `{}`, but candidate `{}` is partial ({}/{}) and cannot be selected", owner(tier), declaration.provider_type.authored_path, provider_slot_key(plan).1, plan.name, plan.rows.len(), plan.schema.methods.len())
            }
            Error::MissingChoice { tier, declaration: index, slot, wrong_target } => format!(
                "{} selects provider `{}` for slot `{}`, but no {}candidate exists in the loaded dependency closure",
                owner(tier), declaration(tier, index).provider_type.authored_path, slot.1,
                if wrong_target { "selected-target " } else { "" },
            ),
            Error::RepeatedCandidate { tier, declaration: index, slot } => format!(
                "{} selection `{}` for slot `{}` resolves to multiple provider candidates with the same exact identity",
                owner(tier), declaration(tier, index).provider_type.authored_path, slot.1,
            ),
            Error::UnresolvedInheritedReceiver { slot } => format!("native provider receiver correspondence for `{}` is unresolved", slot.1),
            Error::UnresolvedOverlap { first, second } => format!("native provider receiver overlap is unresolved between `{}` and `{}`", plans[first].name, plans[second].name),
            Error::RepeatedInheritedReceiver { slot } => format!(
                "slot `{}` is supplied by multiple selected whole provider maps; an inherited receiver must retain one choosing owner", slot.1,
            ),
            Error::AmbiguousCandidates { slot, candidates } => format!(
                "slot `{}` has {} covering provider plans for the selected target: {} -- choose one in build.omg with `Build::select_provider<{}, ProviderType>(b);`",
                slot.1, if candidates.len() == 2 { "two".to_owned() } else { candidates.len().to_string() },
                candidates.iter().map(|index| { let plan = &plans[*index]; format!("`{}` [{:016x}]", plan.name, plan.report_fingerprint()) }).collect::<Vec<_>>().join(", "), slot.1,
            ),
            Error::OverlappingChoices { first, second } => format!(
                "selected whole provider maps `{}` and `{}` supply the same exact receiver application; inherited rows cannot be mixed across choosing owners", plans[first].name, plans[second].name,
            ),
        };
        diagnostics::Diagnostic::error(message)
    }).collect::<Vec<_>>();
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(outcome
        .choices
        .into_iter()
        .map(|choice| SelectedProviderPlanIndex {
            candidate: choice.candidate,
            selected_by: match choice.reason {
                Reason::Override(indices) => SelectionProvenance::BuildOverride(
                    indices
                        .into_iter()
                        .map(|index| requested[index].clone())
                        .collect(),
                ),
                Reason::Default(indices) => SelectionProvenance::TargetDefault(
                    indices
                        .into_iter()
                        .map(|index| defaults[index].clone())
                        .collect(),
                ),
                Reason::UniqueCoveringCandidate => SelectionProvenance::UniqueCoveringCandidate,
            },
        })
        .collect())
}

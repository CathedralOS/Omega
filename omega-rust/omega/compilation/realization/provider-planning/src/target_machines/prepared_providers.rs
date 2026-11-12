// We collect candidate provider implementations and their declarations before
// publishing the shared intermediate program so later stages can attach target
// Calling plans without reopening source. Native-build rows precede authored
// derivation so a `boundary machine` satisfies declaration on a build-bound
// requirement joins its package-owned evaluated binding instead of inferring a
// compiler leaf. When Calling plans are attached, we clear any cached digest
// memo (`digest_memo = Default::default()`) and validate native Build
// candidates against their completed schemas.

use super::{AuthoredTargetProviders, TargetProviderPreparationRequest};
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use crate::provider_planning::provider_rows::CapturedProviderRows;
use crate::provider_settlement::assumed_bindings::PreparedAssumedBindings;
use crate::provider_settlement::{
    completion,
    semantic_roles::{PreparedCompletionRole, PreparedCompletionRoles},
};
use crate::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration, ProviderSelection,
};
use checked_trees::typed_trees::service_declarations::{RetainedServiceSurface, ServiceCatalog};
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use provider_selection::{
    CanonicalProviderDeclaration, PreparedTargetChoices, TargetProviderChoice,
};
use std::sync::{Arc, Weak};
use symbols::SymbolHandle;

type SourceChoices = PreparedTargetChoices<ProviderPlanProvenance, ProviderSelection>;
type ProductChoices = PreparedTargetChoices<PreparedProviderMetadata, ProviderSelection>;

#[derive(Clone)]
pub(crate) struct PreparedProviderMetadata {
    pub provenance: ProviderPlanProvenance,
    pub completion: Result<Vec<Vec<SymbolHandle>>, Vec<Diagnostic>>,
    pub role: Option<PreparedCompletionRole>,
    pub families:
        Result<Vec<crate::native_provider_sources::PreparedFamilyRealization>, Vec<Diagnostic>>,
}

type ApplicationRoots = Result<Option<Vec<CanonicalProviderDeclaration>>, Vec<Diagnostic>>;

pub struct ProductProviderPreparation<'program> {
    pub checked: &'program Arc<checked_trees::CheckedTrees>,
    pub services: ServiceCatalog<'program>,
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub origins: &'program [crate::SelectedTargetMachineOrigin],
    pub defaults: &'program [crate::TargetProviderDefaults],
    pub target_machines: crate::SettledTargetMachineDeclarations,
    pub selected_target_profile: Option<target::TargetProfile>,
    pub evaluated_bindings: EvaluatedViaBindingTable,
    pub application_roots: Option<&'program [(String, SymbolHandle)]>,
    pub overrides: &'program [ProviderSelection],
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
    pub native_bindings: &'program crate::native_bindings::PreparedNativeBuildBindings,
    pub native_supplies: &'program [crate::native_bindings::NativeBuildSupply],
    pub assumed_bindings:
        &'program [language_semantics::declaration_selection::AssumedBindingContract],
}

pub struct PreparedProductProviders {
    owner: Weak<checked_trees::CheckedTrees>,
    planning_target: Option<target::TargetProfile>,
    assumptions: PreparedAssumedBindings,
    bindings: EvaluatedViaBindingTable,
    candidates: Vec<ProductCandidate>,
    native_start: usize,
    choices: Result<ProductChoices, Vec<Diagnostic>>,
    completion_roles: PreparedCompletionRoles,
    publication: SourcePublication,
}

struct ProductCandidate {
    calling: CallingCandidate,
    rows: CapturedProviderRows,
}

pub(crate) struct SettledProductCandidate {
    pub source: DerivedProviderPlan,
    pub rows: CapturedProviderRows,
}

struct CallingCandidate {
    derived: DerivedProviderPlan,
    surface: Option<RetainedServiceSurface>,
}

impl CallingCandidate {
    fn capture(
        services: ServiceCatalog<'_>,
        derived: DerivedProviderPlan,
    ) -> Result<Self, Vec<Diagnostic>> {
        let surface =
            if let ProviderSchemaDeclaration::BoundaryTrait(receiver) = derived.provenance.schema {
                let surface = match &derived.provenance.named_application {
                    Some(application) => services.retain_named_application(
                        application.declaration,
                        application.capture,
                        receiver,
                    ),
                    None => services.retain_declaration(derived.provenance.schema),
                }
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "provider candidate lost its exact prepared Calling surface",
                    )]
                })?;
                Some(surface)
            } else {
                None
            };
        Ok(Self { derived, surface })
    }

    fn attach(
        self,
        calling: &[BoundaryCallingPlanRealization],
    ) -> Result<DerivedProviderPlan, Vec<Diagnostic>> {
        let Self {
            mut derived,
            surface,
        } = self;
        if let Some(surface) = surface {
            let surface = surface.surface().map_err(|error| vec![error])?;
            derived.plan.schema = crate::service_schema::attach_calling(&surface, calling);
            derived.plan.digest_memo = Default::default();
        }
        Ok(derived)
    }
}

struct SourceProposal {
    candidates: Vec<CallingCandidate>,
    choices: Result<SourceChoices, Vec<Diagnostic>>,
}

impl SourceProposal {
    fn capture(
        services: ServiceCatalog<'_>,
        request: &TargetProviderPreparationRequest<'_>,
        target: &str,
        inventory: &AuthoredTargetProviders,
    ) -> Result<Self, Vec<Diagnostic>> {
        let choices = request.prepare_target(target, inventory, &[]);
        let candidates = inventory
            .candidates()
            .iter()
            .cloned()
            .map(|derived| CallingCandidate::capture(services, derived))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            candidates,
            choices,
        })
    }

    fn attach(
        self,
        calling: &[BoundaryCallingPlanRealization],
    ) -> Result<PreparedTargetChoices, Vec<Diagnostic>> {
        let candidates = attach_candidates(self.candidates, calling)?;
        attach_choices(self.choices, candidates.iter()).map(SourceChoices::without_correspondence)
    }
}

struct SourcePublicationRow {
    target: String,
    roots: ApplicationRoots,
    proposal: Result<SourceProposal, Vec<Diagnostic>>,
}

struct SourcePublication {
    base_targets: Vec<String>,
    deployed: bool,
    rows: Vec<SourcePublicationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPublicationCatalog {
    base_targets: Vec<String>,
    deployed: bool,
    rows: Vec<PublicationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicationRow {
    target: String,
    roots: ApplicationRoots,
    choices: Result<PreparedTargetChoices, Vec<Diagnostic>>,
}

impl ProviderPublicationCatalog {
    pub fn prepare(
        &self,
        requested_scopes: &[String],
        selected: Option<&Arc<provider_selection::SelectedTargetProviderPublication>>,
        assumptions: &[effects::AssumedBindingContractSelection],
        components: &[Arc<component_description::VerifiedComponent>],
    ) -> provider_selection::PreparedTargetProviderPublication {
        let mut targets = self.base_targets.clone();
        targets.extend(requested_scopes.iter().cloned());
        targets.sort();
        targets.dedup();
        let targets = targets
            .into_iter()
            .map(|target| {
                let choice = match self.rows.iter().find(|row| row.target == target) {
                    Some(row) => {
                        if let Some(selected) = selected
                            && selected.profile.target_name() == target
                        {
                            row.roots.clone().map(|application_roots| {
                                TargetProviderChoice::Selected {
                                    selected: Arc::clone(selected),
                                    application_roots,
                                }
                            })
                        } else {
                            row.choices.clone().map(|mut choices| {
                                choices.independent_components = components.to_vec();
                                TargetProviderChoice::Prepared(choices)
                            })
                        }
                    }
                    None => Err(vec![Diagnostic::error(format!(
                        "provider proposal has no receiving target catalog for `{target}`"
                    ))]),
                };
                (target, choice)
            })
            .collect();
        provider_selection::PreparedTargetProviderPublication::new(
            targets,
            assumptions.to_vec(),
            self.deployed,
        )
    }
}

pub(crate) struct SettledProductProviders {
    pub assumptions: PreparedAssumedBindings,
    pub bindings: EvaluatedViaBindingTable,
    pub candidates: Vec<SettledProductCandidate>,
    pub native_start: usize,
    pub choices: Result<ProductChoices, Vec<Diagnostic>>,
    pub completion_roles: PreparedCompletionRoles,
    pub publication: ProviderPublicationCatalog,
}

impl PreparedProductProviders {
    pub(crate) fn attach_calling(
        self,
        program: &crate::calling_policy_plans::CallingProgram,
        planning_target: Option<target::TargetProfile>,
        calling: &[BoundaryCallingPlanRealization],
    ) -> Result<SettledProductProviders, Vec<Diagnostic>> {
        if !Weak::ptr_eq(&self.owner, &program.source_owner)
            || self.planning_target != planning_target
        {
            return Err(vec![Diagnostic::error(
                "provider preparation belongs to another Product program or planning target",
            )]);
        }
        let candidates = self
            .candidates
            .into_iter()
            .map(|candidate| {
                Ok(SettledProductCandidate {
                    source: candidate.calling.attach(calling)?,
                    rows: candidate.rows,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        for candidate in &candidates[self.native_start..] {
            let errors = candidate.source.plan.validate_candidate_against_schema();
            if !errors.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "incomplete native Build provider `{}`: {}",
                    candidate.source.plan.name,
                    errors.join("; "),
                ))]);
            }
        }
        let choices = attach_choices(
            self.choices,
            candidates.iter().map(|candidate| &candidate.source),
        );
        let publication = ProviderPublicationCatalog {
            base_targets: self.publication.base_targets,
            deployed: self.publication.deployed,
            rows: self
                .publication
                .rows
                .into_iter()
                .map(|row| PublicationRow {
                    target: row.target,
                    roots: row.roots,
                    choices: row.proposal.and_then(|proposal| proposal.attach(calling)),
                })
                .collect(),
        };
        Ok(SettledProductProviders {
            assumptions: self.assumptions,
            completion_roles: self.completion_roles,
            bindings: self.bindings,
            candidates,
            native_start: self.native_start,
            choices,
            publication,
        })
    }
}

fn attach_candidates(
    candidates: Vec<CallingCandidate>,
    calling: &[BoundaryCallingPlanRealization],
) -> Result<Vec<DerivedProviderPlan>, Vec<Diagnostic>> {
    candidates
        .into_iter()
        .map(|candidate| candidate.attach(calling))
        .collect()
}

fn attach_choices<'candidate, Owner>(
    choices: Result<PreparedTargetChoices<Owner, ProviderSelection>, Vec<Diagnostic>>,
    candidates: impl ExactSizeIterator<Item = &'candidate DerivedProviderPlan>,
) -> Result<PreparedTargetChoices<Owner, ProviderSelection>, Vec<Diagnostic>> {
    let mut choices = choices?;
    if choices.candidates.len() != candidates.len() {
        return Err(vec![Diagnostic::error(
            "provider Calling attachment lost its prepared candidate correspondence",
        )]);
    }
    for (choice, candidate) in choices.candidates.iter_mut().zip(candidates) {
        choice.choice.plan = candidate.plan.clone();
    }
    Ok(choices)
}

impl ProductProviderPreparation<'_> {
    pub fn capture(self) -> Result<PreparedProductProviders, Vec<Diagnostic>> {
        if !std::ptr::eq(&*self.checked.typed, self.services.context().typed)
            || self.services.context().scope != source::DependencyScope::Product
        {
            return Err(vec![Diagnostic::error(
                "provider preparation requires the exact Product owner's service catalog",
            )]);
        }
        let planning_target = self
            .selected_target_profile
            .or_else(target::TargetProfile::host_if_supported);
        let profile = planning_target.unwrap_or_else(target::TargetProfile::host);
        let mut bindings = self.evaluated_bindings;
        let mut identities = bindings.identities().clone().into_builder();
        let native =
            self.native_bindings
                .form(self.native_supplies, planning_target, &mut identities)?;
        let authored = AuthoredTargetProviders::derive(
            self.services,
            self.body_declarations,
            &bindings,
            planning_target,
            self.target_machines,
            &native,
        )?;
        bindings.retain_extended_identities(identities.build())?;
        let request = TargetProviderPreparationRequest {
            context: self.services.context(),
            checked: self.checked,
            exact_toolchain_sources: self.exact_toolchain_sources,
            application_roots: self.application_roots,
            overrides: self.overrides,
        };
        let choices = request.prepare_target(profile.target_name(), &authored, &native);

        let mut base_targets = self
            .defaults
            .iter()
            .filter_map(|row| row.target.clone())
            .collect::<Vec<_>>();
        base_targets.extend(self.overrides.iter().filter_map(|selection| {
            selection
                .target
                .map(|target| target.canonical_name().to_owned())
        }));
        base_targets.extend(
            self.application_roots
                .into_iter()
                .flatten()
                .map(|(target, _)| target.clone()),
        );
        base_targets.push(profile.target_name().to_owned());
        base_targets.extend(
            checked_trees::ProofApplications::new(
                &self.checked.facts.proof_applications,
                &self.checked.facts.operational,
            )
            .scopes(profile.target_name())
            .into_iter()
            .map(str::to_owned),
        );
        base_targets.sort();
        base_targets.dedup();
        let mut catalog_targets = base_targets.clone();
        catalog_targets.extend(
            target::TargetProfile::ALL
                .into_iter()
                .map(|target| target.target_name().to_owned()),
        );
        catalog_targets.sort();
        catalog_targets.dedup();
        let mut rows = Vec::with_capacity(catalog_targets.len());
        for target in catalog_targets {
            let roots = request.canonical_roots(&target);
            let proposal = (|| {
                let target_profile = target::TargetProfile::from_canonical_target_name(&target)
                    .map_err(|_| {
                        vec![Diagnostic::error(format!(
                            "provider proposal has no receiving target catalog for `{target}`"
                        ))]
                    })?;
                target_profile.native_realization().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "target `{target}` has no provider realization in this compiler"
                    ))]
                })?;
                if authored.target() == Some(target_profile) {
                    return SourceProposal::capture(self.services, &request, &target, &authored);
                }
                let declarations =
                    super::settle_provider_defaults(self.origins, self.defaults, target_profile);
                let inventory = AuthoredTargetProviders::derive(
                    self.services,
                    self.body_declarations,
                    &bindings,
                    Some(target_profile),
                    declarations,
                    &native,
                )?;
                SourceProposal::capture(self.services, &request, &target, &inventory)
            })();
            rows.push(SourcePublicationRow {
                target,
                roots,
                proposal,
            });
        }
        let native_start = authored.candidates().len();
        let inventory = authored
            .into_candidates()
            .into_iter()
            .chain(native)
            .collect::<Vec<_>>();
        let original_candidates = inventory.iter().collect::<Vec<_>>();
        let choices = choices.map(|choices| {
            choices.map_owner(|plan, provenance| {
                let completion = completion::capture(
                    self.checked,
                    &provenance,
                    planning_target.map(target::TargetProfile::target_name),
                    &original_candidates,
                );
                let role = PreparedCompletionRole::capture(&self.checked.typed, plan, &provenance);
                PreparedProviderMetadata {
                    families: crate::native_provider_sources::capture_family_realizations(
                        self.services.context(),
                        plan,
                        &provenance,
                    ),
                    provenance,
                    completion,
                    role,
                }
            })
        });
        let candidates = inventory
            .into_iter()
            .map(|derived| {
                let rows = CapturedProviderRows::capture(
                    &derived,
                    self.services,
                    self.exact_toolchain_sources,
                );
                Ok(ProductCandidate {
                    calling: CallingCandidate::capture(self.services, derived)?,
                    rows,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        Ok(PreparedProductProviders {
            owner: Arc::downgrade(self.checked),
            completion_roles: PreparedCompletionRoles::capture(&self.checked.typed),
            planning_target,
            assumptions: PreparedAssumedBindings::capture(
                &self.checked.typed,
                self.assumed_bindings,
            ),
            bindings,
            candidates,
            native_start,
            choices,
            publication: SourcePublication {
                base_targets,
                deployed: self.application_roots.is_some(),
                rows,
            },
        })
    }
}

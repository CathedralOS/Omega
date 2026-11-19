use super::{
    CompletedScalarConditions, ScalarApplicationObservations, ScalarApplicationValues,
    ScalarConditionSelection,
};
use bignum::BigInt;
use psi_provider_selections::AdmittedProviderSelections;
use psi_target_layouts::{AdmittedTargetLayouts, TerminalTargetContext};
use terminal_codec::TerminalArtifactView;
use terminal_psi::{
    ScalarCompletionCondition, ScalarConditionSubject, ScalarRangeCondition, TerminalPsiIdentity,
};

use semantic_vocabulary::ScalarAddressModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarScopeInputs {
    targets: Option<TerminalTargetContext>,
    providers: AdmittedProviderSelections,
    layouts: Option<AdmittedTargetLayouts>,
    layout_target: Option<psi_target_layouts::LayoutTargetFacts>,
    address_model: Option<ScalarAddressModel>,
    carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    entry_membership: ScalarEntryMembership,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScalarEntryMembership {
    Source,
    Selected(Vec<terminal_psi::ProofDeclarationId>),
    None,
}

impl ScalarScopeInputs {
    pub fn targets(&self) -> Option<&TerminalTargetContext> {
        self.targets.as_ref()
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        &self.providers
    }
    pub fn layouts(&self) -> Option<&AdmittedTargetLayouts> {
        self.layouts.as_ref()
    }
    pub fn address_model(&self) -> Option<&ScalarAddressModel> {
        self.address_model.as_ref()
    }
    pub fn carrier_layout(&self) -> Option<semantic_vocabulary::TargetCarrierLayout> {
        self.carrier_layout
    }

    pub fn admits_machine(
        &self,
        module: &terminal_psi::TerminalModule,
        machine: semantic_vocabulary::MachineId,
    ) -> bool {
        let members = match &self.entry_membership {
            ScalarEntryMembership::Source => return true,
            ScalarEntryMembership::None => return false,
            ScalarEntryMembership::Selected(members) => members,
        };
        if module.scalar_applications.conditions.is_empty() {
            return true;
        }
        module
            .scalar_applications
            .proof_scope
            .callable_declaration(terminal_psi::TargetMachineCallable::Machine(machine))
            .is_some_and(|owner| members.contains(&owner))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedScalarApplications {
    // The exact code these values were observed on. Invocation code keeps
    // its process-local scope here, so an admission formed for one
    // build-time invocation never matches a portable artifact or another
    // invocation that happens to share the semantic identity.
    code: terminal_codec::TerminalExecutionIdentity,
    product_formation: bool,
    primary: CompletedScalarScope,
    supplemental: Vec<CompletedScalarScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletedScalarScope {
    pub(super) inputs: ScalarScopeInputs,
    pub(super) values: ScalarApplicationValues,
    pub(super) geometry: std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]>,
    pub(super) conditions: CompletedScalarConditions,
}

impl AdmittedScalarApplications {
    pub fn require_product_formation(&self) -> Result<(), String> {
        self.product_formation.then_some(()).ok_or_else(|| {
            "entry scalar admission does not establish Product declaration formation".into()
        })
    }

    pub fn terminal(&self) -> TerminalPsiIdentity {
        self.code.semantic()
    }
    pub fn execution_identity(&self) -> &terminal_codec::TerminalExecutionIdentity {
        &self.code
    }
    pub fn primary(&self) -> &ScalarScopeInputs {
        &self.primary.inputs
    }
    pub fn supplemental(&self) -> impl Iterator<Item = &ScalarScopeInputs> {
        self.supplemental.iter().map(|scope| &scope.inputs)
    }

    pub(super) fn restore_scope(
        &self,
        observations: &mut ScalarApplicationObservations<'_>,
        address_model: &Result<Option<ScalarAddressModel>, String>,
    ) -> Result<(), String> {
        self.require_product_formation()?;
        if observations.inherited.is_some()
            || observations
                .applications
                .storage_slice()
                .iter()
                .any(|row| row.value.is_some() || !row.results.is_empty())
            || matches!(
                observations.condition_scope,
                super::ScalarConditionScope::Invocation(_)
                    | super::ScalarConditionScope::AbiStorage(_)
            )
        {
            return Err("completed scalar scope requires fresh Product observations".into());
        }
        if self.code != observations.artifact().custody_identity() {
            return Err("scalar completion belongs to another execution identity".into());
        }
        let previous = std::iter::once(&self.primary)
            .chain(&self.supplemental)
            .find(|scope| {
                scope
                    .inputs
                    .targets
                    .as_ref()
                    .map(TerminalTargetContext::target)
                    == observations.target()
            })
            .ok_or("scalar completion has no matching authored target")?;
        let inputs = &previous.inputs;
        if inputs.targets.as_ref() != observations.targets()
            || &inputs.providers != observations.providers()
            || inputs.layouts.as_ref() != observations.layouts()
            || inputs.layout_target != observations.layout_target
            || inputs.carrier_layout != observations.carrier_layout()
            || address_model.as_ref().map_err(Clone::clone)? != &inputs.address_model
        {
            return Err("scalar completion changed its provider or target inputs".into());
        }
        observations.applications = std::sync::Arc::clone(&previous.values.0);
        observations.reexecuted = vec![false; previous.values.0.storage_slice().len()];
        observations.inherited = Some(previous.clone());
        Ok(())
    }
    pub fn values(&self) -> &ScalarApplicationValues {
        &self.primary.values
    }
    pub fn geometry(&self) -> &std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]> {
        &self.primary.geometry
    }
}

pub struct ScalarApplicationClosure<'artifact> {
    artifact: terminal_codec::TerminalExecutionView<'artifact>,
    primary: Option<String>,
    scopes: Vec<(Option<String>, bool)>,
    ranges: Vec<bool>,
    predicates: Vec<bool>,
    completions: Vec<bool>,
    primary_result: Option<CompletedScalarScope>,
    supplemental: Vec<CompletedScalarScope>,
    kind: ScalarClosureKind,
}

pub(crate) enum ScalarClosureKind {
    Source,
    Deployment,
    Invocation(Vec<terminal_psi::ProofDeclarationId>),
    AbiStorage(super::AbiStorageScope),
}

impl<'artifact> ScalarApplicationClosure<'artifact> {
    pub fn new<'scope>(
        artifact: TerminalArtifactView<'artifact>,
        primary: impl Into<Option<&'scope str>>,
    ) -> Self {
        Self::source(artifact.into(), primary.into())
    }

    fn source(
        artifact: terminal_codec::TerminalExecutionView<'artifact>,
        primary: Option<&str>,
    ) -> Self {
        let catalog = &artifact.module().scalar_applications;
        let applications = catalog.proof_scope.applications();
        let mut owners = Vec::new();
        let mut unscoped = false;
        let subjects = catalog
            .conditions
            .ranges()
            .iter()
            .filter_map(|condition| match condition {
                ScalarRangeCondition::Range { subject, .. } => Some(*subject),
                _ => None,
            })
            .chain(
                catalog
                    .conditions
                    .predicates()
                    .iter()
                    .map(|condition| condition.subject),
            )
            .chain(catalog.conditions.completions().iter().filter_map(
                |condition| match condition {
                    ScalarCompletionCondition::Complete { subject, .. } => Some(*subject),
                    ScalarCompletionCondition::Missing => None,
                },
            ));
        for subject in subjects {
            match subject {
                ScalarConditionSubject::Module => unscoped = true,
                ScalarConditionSubject::Declaration(owner) if !owners.contains(&owner) => {
                    owners.push(owner)
                }
                _ => {}
            }
        }
        let scopes = applications
            .required_scopes(primary, &owners, unscoped)
            .into_iter()
            .map(|scope| (scope.map(str::to_owned), false))
            .collect();
        Self::with_scopes(artifact, primary, scopes, ScalarClosureKind::Source)
    }

    fn with_scopes(
        artifact: terminal_codec::TerminalExecutionView<'artifact>,
        primary: Option<&str>,
        scopes: Vec<(Option<String>, bool)>,
        kind: ScalarClosureKind,
    ) -> Self {
        let conditions = &artifact.module().scalar_applications.conditions;
        Self {
            artifact,
            primary: primary.map(str::to_owned),
            scopes,
            ranges: vec![false; conditions.ranges().len()],
            predicates: vec![false; conditions.predicates().len()],
            completions: vec![false; conditions.completions().len()],
            primary_result: None,
            supplemental: Vec::new(),
            kind,
        }
    }

    pub fn for_invocation<'scope>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'artifact>>,
        primary: impl Into<Option<&'scope str>>,
        roots: &[terminal_psi::ProofDeclarationId],
    ) -> Self {
        let artifact = artifact.into();
        let primary = primary.into();
        let mut declarations = Vec::with_capacity(roots.len());
        for &root in roots {
            if !declarations.contains(&root) {
                declarations.push(root);
            }
        }
        Self::with_scopes(
            artifact,
            primary,
            vec![(primary.map(str::to_owned), false)],
            ScalarClosureKind::Invocation(declarations),
        )
    }

    pub fn for_abi_storage(
        artifact: TerminalArtifactView<'artifact>,
        demand: &super::CallingAbiStorageDemand,
    ) -> Result<Self, String> {
        demand.require_artifact(artifact)?;
        Ok(Self::with_scopes(
            artifact.into(),
            Some(demand.target()),
            vec![(Some(demand.target().to_owned()), false)],
            ScalarClosureKind::AbiStorage(demand.into()),
        ))
    }

    pub fn with_deployment_scopes(mut self, scopes: &[String]) -> Self {
        if matches!(
            self.kind,
            ScalarClosureKind::Invocation(_) | ScalarClosureKind::AbiStorage(_)
        ) {
            self = Self::source(self.artifact, self.primary.as_deref());
        }
        self.kind = ScalarClosureKind::Deployment;
        for scope in scopes {
            if !self
                .scopes
                .iter()
                .any(|(target, _)| target.as_ref() == Some(scope))
            {
                self.scopes.push((Some(scope.clone()), false));
            }
        }
        self
    }

    pub fn scopes(&self) -> impl Iterator<Item = Option<&str>> {
        self.scopes.iter().map(|(scope, _)| scope.as_deref())
    }

    pub fn close_scope(
        &mut self,
        observations: ScalarApplicationObservations<'_>,
        conditions: ScalarConditionSelection<'_>,
        address_model: Result<Option<ScalarAddressModel>, String>,
    ) -> Result<(), String> {
        if observations.inherited.is_some() {
            observations.require_completed_address_model(
                address_model.as_ref().map_err(Clone::clone)?.as_ref(),
            )?;
        }
        if observations.artifact().identity() != self.artifact.identity()
            || !conditions.matches(&observations, self.primary.as_deref())
        {
            return Err(
                "scalar closure selection differs from its artifact or scope inputs".into(),
            );
        }
        let matching_scope = match (&self.kind, conditions.scope()) {
            (ScalarClosureKind::Source, super::ScalarConditionScope::Source) => true,
            (ScalarClosureKind::Deployment, super::ScalarConditionScope::SourceRoots(_)) => true,
            (
                ScalarClosureKind::Invocation(expected),
                super::ScalarConditionScope::Invocation(actual),
            ) => expected == actual,
            (
                ScalarClosureKind::AbiStorage(expected),
                super::ScalarConditionScope::AbiStorage(actual),
            ) => expected == actual,
            _ => false,
        };
        if !matching_scope {
            return Err(
                "scalar closure deployment roots differ from its selected conditions".into(),
            );
        }
        let target = observations.target();
        let scope = self
            .scopes
            .iter()
            .position(|(scope, _)| scope.as_deref() == target)
            .ok_or("scalar closure received an unrequested authored scope")?;
        if self.scopes[scope].1 {
            return Err("scalar closure scope completed more than once".into());
        }
        let catalog = &self.artifact.module().scalar_applications.conditions;
        for (position, condition) in catalog.completions().iter().enumerate() {
            if let ScalarCompletionCondition::Complete {
                subject: ScalarConditionSubject::Application(application),
                ..
            } = condition
                && observations.value(*application)?.is_some()
                && !conditions.completions().contains(&position)
            {
                return Err("completed scalar application has unselected completion debt".into());
            }
        }
        for &position in conditions.completions() {
            let ScalarCompletionCondition::Complete { application, .. } =
                catalog.completions()[position]
            else {
                return Err("scalar closure has an absent completion condition".into());
            };
            if observations.value(application)?.is_none() {
                return Err(format!(
                    "scalar application {application} has not completed normally"
                ));
            }
        }
        for (position, condition) in catalog.ranges().iter().enumerate() {
            if let ScalarRangeCondition::Range {
                subject: ScalarConditionSubject::Application(application),
                ..
            } = condition
                && observations.value(*application)?.is_some()
                && !conditions.ranges().contains(&position)
            {
                return Err("completed scalar application has unselected range debt".into());
            }
        }
        for (position, condition) in catalog.predicates().iter().enumerate() {
            if let ScalarConditionSubject::Application(application) = condition.subject
                && observations.value(application)?.is_some()
                && !conditions.predicates().contains(&position)
            {
                return Err("completed scalar application has unselected predicate debt".into());
            }
        }
        for &position in conditions.predicates() {
            observations.discharge_predicate(position)?;
        }
        let address_model = address_model?;
        let bounds = if let Some(model) = &address_model {
            if Some(model.target.as_str()) != target
                || !model.carrier.is_address()
                || model.semantics_version == 0
            {
                return Err(
                    "scalar closure address model differs from its selected target or carrier"
                        .into(),
                );
            }
            Some((
                BigInt::zero(),
                BigInt::from_u64(1)
                    .shl_bits(usize::from(model.carrier.bits()))
                    .sub(&BigInt::from_u64(1)),
            ))
        } else {
            None
        };
        for &position in conditions.ranges() {
            observations.discharge_range(
                position,
                bounds.as_ref().map(|(minimum, maximum)| (minimum, maximum)),
            )?;
        }
        let mut geometry = observations
            .layouts()
            .map_or_else(Vec::new, |layouts| layouts.resolved_records().to_vec());
        for declaration in &observations.artifact().module().structural_types {
            if let terminal_psi::StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Application(application),
            } = declaration.shape
                && let Some(length) = observations.extent(application)?
            {
                let mut resolved = declaration.clone();
                resolved.shape = terminal_psi::StructuralTypeShape::FixedArray {
                    element,
                    length: length.into(),
                };
                geometry.push(resolved);
            }
        }
        geometry.sort_unstable_by_key(|declaration| declaration.id);
        for &position in conditions.predicates() {
            self.predicates[position] = true;
        }
        for &position in conditions.completions() {
            self.completions[position] = true;
        }
        for &position in conditions.ranges() {
            self.ranges[position] = true;
        }
        let inputs = ScalarScopeInputs {
            targets: observations.targets().cloned(),
            providers: observations.providers().clone(),
            layouts: observations.layouts().cloned(),
            layout_target: observations.layout_target.clone(),
            address_model,
            carrier_layout: observations.carrier_layout(),
            entry_membership: match conditions.scope() {
                super::ScalarConditionScope::AbiStorage(_) => ScalarEntryMembership::None,
                _ => conditions
                    .application_machines()
                    .map(|machines| ScalarEntryMembership::Selected(machines.to_vec()))
                    .unwrap_or(ScalarEntryMembership::Source),
            },
        };
        let completed = CompletedScalarScope {
            inputs,
            values: observations.values(),
            geometry: geometry.into(),
            conditions: CompletedScalarConditions::new(&conditions),
        };
        if target == self.primary.as_deref() {
            self.primary_result = Some(completed);
        } else {
            self.supplemental.push(completed);
        }
        self.scopes[scope].1 = true;
        Ok(())
    }

    pub fn finish(self) -> Result<AdmittedScalarApplications, String> {
        if self.scopes.iter().any(|(_, complete)| !complete) {
            return Err("scalar closure has an unfinished authored scope".into());
        }
        let conditions = &self.artifact.module().scalar_applications.conditions;
        for (position, condition) in conditions.completions().iter().enumerate() {
            let ScalarCompletionCondition::Complete { subject, .. } = condition else {
                return Err("scalar closure has an absent completion condition".into());
            };
            if matches!(self.kind, ScalarClosureKind::Source)
                && !matches!(subject, ScalarConditionSubject::Application(_))
                && !self.completions[position]
            {
                return Err("retained scalar completion has no target application".into());
            }
        }
        for (position, condition) in conditions.ranges().iter().enumerate() {
            let ScalarRangeCondition::Range { subject, .. } = condition else {
                return Err("scalar closure has an absent range condition".into());
            };
            if matches!(self.kind, ScalarClosureKind::Source)
                && !matches!(subject, ScalarConditionSubject::Application(_))
                && !self.ranges[position]
            {
                return Err("retained scalar range has no target application".into());
            }
        }
        for (position, condition) in conditions.predicates().iter().enumerate() {
            if matches!(self.kind, ScalarClosureKind::Source)
                && !matches!(condition.subject, ScalarConditionSubject::Application(_))
                && !self.predicates[position]
            {
                return Err("retained scalar predicate has no target application".into());
            }
        }
        let primary = self
            .primary_result
            .ok_or("scalar closure has no primary value scope")?;
        Ok(AdmittedScalarApplications {
            code: self.artifact.custody_identity(),
            product_formation: !matches!(
                self.kind,
                ScalarClosureKind::Invocation(_) | ScalarClosureKind::AbiStorage(_)
            ),
            primary,
            supplemental: self.supplemental,
        })
    }
}

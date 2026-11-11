/*
Scalar application closure, entry condition selection, ABI storage demand, and
layout observation checks (`scalar_applications`).
*/

use arena::Arena;
use numerics::bignum::BigInt;
use numerics::scalar_predicates::ScalarValue;
use semantic_vocabulary::{IntegerValue, MachineId, ScalarApplicationId, ScalarType, ValueId};
use terminal_codec::TerminalArtifactView;
use terminal_psi::{
    LayoutMeasurement, ScalarApplication, ScalarRangeObservation, ScalarRangeRequirement,
};
use terminal_semantics::{ReferenceStorageGeometry, StorageGeometryError};

use crate::{
    AdmittedProviderSelections, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    TerminalScalarValue,
};

pub(crate) mod conditions {
    use numerics::integer_bounds::IntegerBoundStep;
    use numerics::scalar_predicates::PredicateNode;
    use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
    use terminal_psi::{
        ProofApplicationScope, ScalarCompletionCondition, ScalarConditionSubject, ScalarConditions,
        ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
        TargetMachineCallable,
    };

    pub struct ScalarConditionSelection<'artifact> {
        proof_scope: &'artifact ProofApplicationScope,
        conditions: &'artifact ScalarConditions,
        target: String,
        primary: bool,
        providers: Vec<(BoundaryMachineId, MachineId)>,
        ranges: Vec<usize>,
        predicates: Vec<usize>,
        completions: Vec<usize>,
        application_machines: Option<Vec<terminal_psi::ProofDeclarationId>>,
        condition_scope: super::ScalarConditionScope,
    }

    impl<'artifact> ScalarConditionSelection<'artifact> {
        pub(crate) fn select(
            scope: &'artifact ProofApplicationScope,
            conditions: &'artifact ScalarConditions,
            target: &str,
            primary: bool,
            providers: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
            demanded: &[ScalarApplicationId],
            condition_scope: &super::ScalarConditionScope,
        ) -> Result<Self, String> {
            let providers: Vec<_> = providers.into_iter().collect();
            let mut selected = Vec::new();
            for &(boundary, body) in &providers {
                let requirement = scope
                    .callable_declaration(TargetMachineCallable::Boundary(boundary))
                    .ok_or("selected provider requirement lost its proof declaration")?;
                let realization = scope
                    .callable_declaration(TargetMachineCallable::Machine(body))
                    .ok_or("selected provider body lost its proof declaration")?;
                selected.push((requirement, realization));
            }
            let applications = scope.applications();
            let (machines, declarations) = match condition_scope {
                super::ScalarConditionScope::Source => {
                    applications.membership(target, primary, &selected, None)
                }
                super::ScalarConditionScope::SourceRoots(roots) => {
                    applications.membership(target, primary, &selected, Some(roots))
                }
                super::ScalarConditionScope::Invocation(roots) => {
                    let machines = applications
                        .invocation_reachability(target, &selected, roots)
                        .machines;
                    let mut dependencies = roots.clone();
                    dependencies.extend(machines.iter().copied());
                    let declarations = scope.plan.declaration_closure(&dependencies);
                    (machines, declarations)
                }
                super::ScalarConditionScope::AbiStorage(_) => (Vec::new(), Vec::new()),
            };
            let source_formation = !matches!(
                condition_scope,
                super::ScalarConditionScope::Invocation(_)
                    | super::ScalarConditionScope::AbiStorage(_)
            );
            let all_declarations = applications.declarations();
            let applies = |subject, address_model| match subject {
                ScalarConditionSubject::Module => true,
                ScalarConditionSubject::Declaration(owner) => {
                    machines.contains(&owner)
                        || (address_model
                            && (declarations.contains(&owner)
                                || (source_formation
                                    && primary
                                    && all_declarations.contains(&owner))))
                }
                ScalarConditionSubject::Application(application) => demanded.contains(&application),
            };
            let mut ranges = Vec::new();
            for (position, condition) in conditions.ranges().iter().enumerate() {
                let ScalarRangeCondition::Range {
                    subject,
                    requirement,
                    ..
                } = condition
                else {
                    return Err("scalar scope contains an absent range condition".into());
                };
                if applies(
                    *subject,
                    matches!(requirement, ScalarRangeRequirement::AddressModel),
                ) {
                    ranges.push(position);
                }
            }
            let predicates = conditions
                .predicates()
                .iter()
                .enumerate()
                .filter_map(|(position, condition)| {
                    applies(condition.subject, false).then_some(position)
                })
                .collect();
            let mut completions = Vec::new();
            for (position, condition) in conditions.completions().iter().enumerate() {
                let ScalarCompletionCondition::Complete { subject, .. } = condition else {
                    return Err("scalar scope contains an absent completion condition".into());
                };
                if applies(*subject, true) {
                    completions.push(position);
                }
            }
            Ok(Self {
                proof_scope: scope,
                conditions,
                target: target.to_owned(),
                primary,
                providers,
                ranges,
                predicates,
                completions,
                application_machines: (!matches!(
                    condition_scope,
                    super::ScalarConditionScope::Source
                ))
                .then_some(machines),
                condition_scope: condition_scope.clone(),
            })
        }

        pub(crate) fn application_machines(&self) -> Option<&[terminal_psi::ProofDeclarationId]> {
            self.application_machines.as_deref()
        }

        pub(crate) fn scope(&self) -> &super::ScalarConditionScope {
            &self.condition_scope
        }

        pub fn include_evaluation_entry(&mut self, entry: MachineId) -> Result<(), String> {
            if !matches!(
                self.condition_scope,
                super::ScalarConditionScope::Invocation(_)
                    | super::ScalarConditionScope::AbiStorage(_)
            ) {
                return Ok(());
            }
            let declaration = self
                .proof_scope
                .callable_declaration(TargetMachineCallable::Machine(entry))
                .ok_or("scalar evaluation entry has no checked proof declaration")?;
            if self
                .application_machines
                .as_ref()
                .is_some_and(|machines| machines.contains(&declaration))
            {
                return Ok(());
            }
            let extra = Self::select(
                self.proof_scope,
                self.conditions,
                &self.target,
                self.primary,
                self.providers.iter().copied(),
                &[],
                &super::ScalarConditionScope::Invocation(vec![declaration]),
            )?;
            extend_unique(&mut self.ranges, extra.ranges);
            extend_unique(&mut self.predicates, extra.predicates);
            extend_unique(&mut self.completions, extra.completions);
            if let (Some(machines), Some(extra)) =
                (&mut self.application_machines, extra.application_machines)
            {
                extend_unique(machines, extra);
            }
            Ok(())
        }

        pub fn ranges(&self) -> &[usize] {
            &self.ranges
        }
        pub fn predicates(&self) -> &[usize] {
            &self.predicates
        }

        pub fn completions(&self) -> &[usize] {
            &self.completions
        }

        pub(crate) fn matches(
            &self,
            observations: &super::ScalarApplicationObservations<'_>,
            primary: &str,
        ) -> bool {
            std::ptr::eq(
                self.conditions,
                &observations
                    .artifact()
                    .module()
                    .scalar_applications
                    .conditions,
            ) && self.target == observations.targets().target()
                && self.primary == (self.target == primary)
                && self.condition_scope == observations.condition_scope
                && self
                    .providers
                    .iter()
                    .copied()
                    .eq(observations.providers().providers().chain(
                        observations
                            .providers()
                            .assumed_bindings()
                            .iter()
                            .map(|binding| (binding.required, binding.implementation)),
                    ))
        }

        pub fn include_applications(&mut self, demanded: &[ScalarApplicationId]) {
            for (position, condition) in self.conditions.ranges().iter().enumerate() {
                if matches!(condition, ScalarRangeCondition::Range { subject: ScalarConditionSubject::Application(application), .. } if demanded.contains(application))
                    && !self.ranges.contains(&position)
                {
                    self.ranges.push(position);
                }
            }
            for (position, condition) in self.conditions.predicates().iter().enumerate() {
                if matches!(condition.subject, ScalarConditionSubject::Application(application) if demanded.contains(&application))
                    && !self.predicates.contains(&position)
                {
                    self.predicates.push(position);
                }
            }
            self.ranges.sort_unstable();
            self.predicates.sort_unstable();
            for (position, condition) in self.conditions.completions().iter().enumerate() {
                if matches!(condition, ScalarCompletionCondition::Complete { subject: ScalarConditionSubject::Application(application), .. } if demanded.contains(application))
                    && !self.completions.contains(&position)
                {
                    self.completions.push(position);
                }
            }
            self.completions.sort_unstable();
        }

        pub fn discharge_ready(
            &self,
            observations: &super::ScalarApplicationObservations<'_>,
        ) -> Result<(), String> {
            if !std::ptr::eq(
                self.conditions,
                &observations
                    .artifact()
                    .module()
                    .scalar_applications
                    .conditions,
            ) {
                return Err("scalar condition selection names another artifact".into());
            }
            for &position in &self.predicates {
                let mut ready = true;
                for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                    if let PredicateNode::Leaf(observation) = &node.0 {
                        ready &= observations.observation_ready(observation)?;
                    }
                }
                if ready {
                    observations.discharge_predicate(position)?;
                }
            }
            for &position in &self.ranges {
                let condition = &self.conditions.ranges()[position];
                let ScalarRangeCondition::Range { requirement, .. } = condition else {
                    return Err("scalar range condition is absent".into());
                };
                if matches!(requirement, ScalarRangeRequirement::AddressModel) {
                    continue;
                }
                let mut ready = true;
                for step in self.conditions.steps(condition)? {
                    if let IntegerBoundStep::Observation(observation) = step {
                        ready &= observations.observation_ready(observation)?;
                    }
                }
                if ready {
                    observations.discharge_range(position, None)?;
                }
            }
            Ok(())
        }

        pub fn observations(&self) -> Result<Vec<&'artifact ScalarRangeObservation>, String> {
            let mut observations = Vec::new();
            for &position in &self.ranges {
                for step in self.conditions.steps(&self.conditions.ranges()[position])? {
                    if let IntegerBoundStep::Observation(observation) = step {
                        observations.push(observation);
                    }
                }
            }
            for &position in &self.predicates {
                for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                    if let PredicateNode::Leaf(observation) = &node.0 {
                        observations.push(observation);
                    }
                }
            }
            Ok(observations)
        }

        pub fn applications(&self) -> Result<Vec<ScalarApplicationId>, String> {
            let mut applications = Vec::new();
            for &position in &self.completions {
                let ScalarCompletionCondition::Complete { application, .. } =
                    self.conditions.completions()[position]
                else {
                    return Err("scalar completion condition is absent".into());
                };
                if !applications.contains(&application) {
                    applications.push(application);
                }
            }
            for observation in self.observations()? {
                let application = match observation {
                    ScalarRangeObservation::Application(application)
                    | ScalarRangeObservation::EntryResult { application, .. } => *application,
                    ScalarRangeObservation::Literal(_) => continue,
                    ScalarRangeObservation::Missing => {
                        return Err("scalar condition observation is absent".into());
                    }
                };
                if !applications.contains(&application) {
                    applications.push(application);
                }
            }
            Ok(applications)
        }
    }

    fn extend_unique<T: PartialEq>(selected: &mut Vec<T>, extra: impl IntoIterator<Item = T>) {
        for value in extra {
            if !selected.contains(&value) {
                selected.push(value);
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::super::ScalarConditionScope;
        use super::*;
        use arena::Arena;
        use numerics::{bignum::BigInt, scalar_predicates::ScalarValue};
        use semantic_vocabulary::proof_applications::{
            ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
            ProofDeclarationDependencies,
        };
        use terminal_psi::{
            ProofCallableAssociation, ProofDeclarationId, ProofScopeDeclaration,
            ScalarPredicateNode, TerminalIdentityIndex,
        };

        fn fixture() -> (ProofApplicationScope, ScalarConditions) {
            let mut scope = ProofApplicationScope::default();
            for _ in 1..=7 {
                scope
                    .declarations
                    .insert(ProofScopeDeclaration::Nominal(TerminalIdentityIndex(1)));
            }
            for ordinal in 1..=3 {
                scope.plan.machines.insert(ProofApplicationMachine {
                    machine: ProofDeclarationId(ordinal),
                    source_target: (ordinal == 1).then(|| "macos_arm64".into()),
                    product_selected: true,
                    ..Default::default()
                });
            }
            let call = scope.calls.calls.insert_many([ProofApplicationCall {
                callee: ProofDeclarationId(6),
                ..Default::default()
            }]);
            scope.calls.insert_machine(ProofCallMachine {
                machine: ProofDeclarationId(1),
                calls: call,
                ..Default::default()
            });
            scope.plan.declarations = scope
                .plan
                .symbols
                .insert_many([ProofDeclarationId(4), ProofDeclarationId(5)]);
            let dependency = scope.plan.symbols.insert_many([ProofDeclarationId(4)]);
            scope
                .plan
                .declaration_dependencies
                .insert(ProofDeclarationDependencies {
                    declaration: ProofDeclarationId(2),
                    dependencies: dependency,
                    ..Default::default()
                });
            scope.callables.insert(ProofCallableAssociation::Callable {
                declaration: ProofDeclarationId(6),
                callable: TargetMachineCallable::Boundary(BoundaryMachineId::new(1).unwrap()),
            });
            for ordinal in 2..=3 {
                scope.callables.insert(ProofCallableAssociation::Callable {
                    declaration: ProofDeclarationId(ordinal),
                    callable: TargetMachineCallable::Machine(
                        MachineId::new(u64::from(ordinal)).unwrap(),
                    ),
                });
                scope
                    .plan
                    .provider_candidates
                    .insert((ProofDeclarationId(6), ProofDeclarationId(ordinal)));
            }
            let mut conditions = ScalarConditions::default();
            let pending = ScalarApplicationId::new(7).unwrap();
            for subject in (1..=5)
                .map(|id| ScalarConditionSubject::Declaration(ProofDeclarationId(id)))
                .chain([
                    ScalarConditionSubject::Module,
                    ScalarConditionSubject::Application(pending),
                ])
            {
                conditions
                    .retain_range(
                        subject,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        ScalarRangeRequirement::AddressModel,
                        vec![IntegerBoundStep::Observation(
                            ScalarRangeObservation::Application(pending),
                        )],
                    )
                    .unwrap();
                let mut nodes = Arena::new();
                let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                    ScalarValue::Boolean(true),
                )));
                conditions
                    .retain_predicate(
                        subject,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        "scope test".into(),
                        nodes,
                        root,
                    )
                    .unwrap();
            }
            conditions
                .retain_range(
                    ScalarConditionSubject::Declaration(ProofDeclarationId(4)),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    ScalarRangeRequirement::Fixed {
                        minimum: BigInt::from_u64(0),
                        maximum: BigInt::from_u64(255),
                    },
                    vec![IntegerBoundStep::Bounds {
                        low: BigInt::from_u64(0),
                        high: BigInt::from_u64(0),
                    }],
                )
                .unwrap();
            (scope, conditions)
        }

        #[test]
        fn scalar_condition_scope_uses_selected_edges_not_available_candidates() {
            let (scope, conditions) = fixture();
            let boundary = BoundaryMachineId::new(1).unwrap();
            for (body, ranges, predicates) in [
                (2, vec![0, 1, 3, 5], vec![0, 1, 5]),
                (3, vec![0, 2, 5], vec![0, 2, 5]),
            ] {
                let selected = ScalarConditionSelection::select(
                    &scope,
                    &conditions,
                    "macos_arm64",
                    false,
                    [(boundary, MachineId::new(body).unwrap())],
                    &[],
                    &ScalarConditionScope::Source,
                )
                .unwrap();
                assert_eq!(selected.ranges(), ranges);
                assert_eq!(selected.predicates(), predicates);
                assert_eq!(
                    selected.applications().unwrap(),
                    [ScalarApplicationId::new(7).unwrap()]
                );
            }
            let none = ScalarConditionSelection::select(
                &scope,
                &conditions,
                "macos_arm64",
                false,
                [],
                &[],
                &ScalarConditionScope::Source,
            )
            .unwrap();
            assert_eq!(none.ranges(), [0, 5]);
            assert!(
                ScalarConditionSelection::select(
                    &scope,
                    &conditions,
                    "macos_arm64",
                    false,
                    [(boundary, MachineId::new(99).unwrap())],
                    &[],
                    &ScalarConditionScope::Source
                )
                .is_err()
            );
        }

        #[test]
        fn scalar_condition_scope_keeps_primary_formation_and_explicit_entry_demands_separate() {
            let (scope, conditions) = fixture();
            let pending = ScalarApplicationId::new(7).unwrap();
            let secondary = ScalarConditionSelection::select(
                &scope,
                &conditions,
                "windows_x86_64",
                false,
                [],
                &[],
                &ScalarConditionScope::Source,
            )
            .unwrap();
            assert_eq!(secondary.ranges(), [5]);
            assert_eq!(secondary.predicates(), [5]);
            let primary = ScalarConditionSelection::select(
                &scope,
                &conditions,
                "windows_x86_64",
                true,
                [],
                &[pending],
                &ScalarConditionScope::Source,
            )
            .unwrap();
            assert_eq!(primary.ranges(), [1, 2, 3, 4, 5, 6]);
            assert_eq!(primary.predicates(), [1, 2, 5, 6]);
        }

        #[test]
        fn scalar_condition_scope_selects_family_members_without_rooting_other_targets() {
            use semantic_vocabulary::proof_applications::ProofApplicationTargetMember;
            let (mut scope, conditions) = fixture();
            let members = scope.plan.target_members.insert_many([
                ProofApplicationTargetMember {
                    target: "macos_arm64".into(),
                    machine: ProofDeclarationId(2),
                },
                ProofApplicationTargetMember {
                    target: "windows_x86_64".into(),
                    machine: ProofDeclarationId(3),
                },
            ]);
            scope.plan.machines.for_each_mut(|_, row| {
                row.source_target = None;
                if row.machine == ProofDeclarationId(1) {
                    row.family_interface = true;
                    row.family_members = members;
                } else {
                    let target = if row.machine == ProofDeclarationId(2) {
                        "macos_arm64"
                    } else {
                        "windows_x86_64"
                    };
                    row.source_target = Some(target.into());
                    row.authored_target = Some(target.into());
                }
            });
            for (target, predicate) in [("macos_arm64", 1), ("windows_x86_64", 2)] {
                let selected = ScalarConditionSelection::select(
                    &scope,
                    &conditions,
                    target,
                    true,
                    [],
                    &[],
                    &ScalarConditionScope::Source,
                )
                .unwrap();
                assert_eq!(selected.predicates(), [predicate, 5]);
                assert!(
                    !selected.predicates().contains(&0),
                    "family head is not a selected body"
                );
            }
            let unavailable = ScalarConditionSelection::select(
                &scope,
                &conditions,
                "linux_arm64",
                false,
                [],
                &[],
                &ScalarConditionScope::Source,
            )
            .unwrap();
            assert_eq!(unavailable.predicates(), [5]);
            assert_eq!(unavailable.ranges(), [5]);
        }

        #[test]
        fn entry_admission_retains_module_and_reached_debts_without_certifying_product_formation() {
            let (scope, mut conditions) = fixture();
            for owner in [2, 4, 5] {
                conditions.retain_completion(
                    ScalarConditionSubject::Declaration(ProofDeclarationId(owner)),
                    ScalarApplicationId::new(u64::from(owner)).unwrap(),
                );
            }
            let roots = [ProofDeclarationId(2)];
            let select = |scope_kind| {
                ScalarConditionSelection::select(
                    &scope,
                    &conditions,
                    "windows_x86_64",
                    true,
                    [],
                    &[],
                    &scope_kind,
                )
                .unwrap()
            };
            let mut entry = select(ScalarConditionScope::Invocation(roots.to_vec()));
            assert_eq!(entry.ranges(), [1, 3, 5]);
            assert_eq!(entry.predicates(), [1, 5]);
            assert_eq!(entry.completions(), [0, 1]);
            let product = select(ScalarConditionScope::SourceRoots(roots.to_vec()));
            assert!(product.ranges().contains(&4));
            assert_eq!(product.completions(), [0, 1, 2]);

            entry
                .include_evaluation_entry(MachineId::new(3).unwrap())
                .unwrap();
            let mut predicates = entry.predicates().to_vec();
            predicates.sort_unstable();
            assert_eq!(predicates, [1, 2, 5]);
            assert!(!entry.ranges().contains(&4));
            assert_eq!(
                entry.scope(),
                &ScalarConditionScope::Invocation(roots.to_vec())
            );
        }

        #[test]
        fn abi_storage_demands_keep_actual_helper_debt_without_product_declarations() {
            let (scope, mut conditions) = fixture();
            let pending = ScalarApplicationId::new(7).unwrap();
            conditions.retain_completion(
                ScalarConditionSubject::Declaration(ProofDeclarationId(5)),
                ScalarApplicationId::new(5).unwrap(),
            );
            let mut nodes = Arena::new();
            let root = nodes.insert(ScalarPredicateNode(PredicateNode::Value(
                ScalarValue::Boolean(false),
            )));
            let helper_refusal = conditions.predicates().len();
            conditions
                .retain_predicate(
                    ScalarConditionSubject::Declaration(ProofDeclarationId(3)),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    "actual scalar helper must satisfy this condition".into(),
                    nodes,
                    root,
                )
                .unwrap();
            let abi_scope = ScalarConditionScope::AbiStorage(super::super::AbiStorageScope {
                declaration: semantic_vocabulary::CallingAbiIdentity {
                    requirement_identity: "exact boundary".into(),
                    declaring_application: "exact declaring application".into(),
                    operands: Vec::new(),
                },
                applications: vec![pending],
            });
            let mut selected = ScalarConditionSelection::select(
                &scope,
                &conditions,
                "macos_arm64",
                true,
                [],
                &[pending],
                &abi_scope,
            )
            .unwrap();
            assert_eq!(selected.ranges(), [5, 6]);
            assert_eq!(selected.predicates(), [5, 6]);
            assert!(selected.completions().is_empty());
            assert_eq!(selected.application_machines(), Some([].as_slice()));

            selected
                .include_evaluation_entry(MachineId::new(3).unwrap())
                .unwrap();
            assert!(selected.predicates().contains(&helper_refusal));
            assert!(selected.predicates().contains(&2));
            assert!(!selected.predicates().contains(&4));
            assert!(selected.completions().is_empty());
            assert_eq!(selected.scope(), &abi_scope);
        }
    }
}
pub(crate) mod prerequisites {
    use super::{ScalarAddressModel, ScalarApplicationObservations};
    use crate::TerminalScalarValue;
    use numerics::bignum::BigInt;
    use numerics::integer_bounds::IntegerBoundStep;
    use numerics::scalar_predicates::{PredicateNode, ScalarValue};
    use semantic_vocabulary::{
        IntegerValue, MachineId, ScalarApplicationId, ScalarConditionRole, ScalarType, ValueId,
    };
    use terminal_psi::{
        ScalarConditionSubject, ScalarConditions, ScalarPredicateCondition, ScalarRangeCondition,
        ScalarRangeObservation, ScalarRangeRequirement, TargetMachineCallable,
    };

    #[derive(Clone, Copy)]
    pub(crate) enum ConditionId {
        Range(usize),
        Predicate(usize),
    }

    pub struct ScalarEntryPrerequisites<'artifact> {
        conditions: &'artifact ScalarConditions,
        application: ScalarApplicationId,
        entry: MachineId,
        target: String,
        providers: crate::AdmittedProviderSelections,
        rows: Vec<ConditionId>,
    }

    pub struct AdmittedScalarEntry<'input, 'artifact> {
        observations: &'input ScalarApplicationObservations<'artifact>,
        application: ScalarApplicationId,
        checks: ScalarEntryChecks,
    }

    impl<'input, 'artifact> AdmittedScalarEntry<'input, 'artifact> {
        pub fn observations(&self) -> &'input ScalarApplicationObservations<'artifact> {
            self.observations
        }
        pub fn application(&self) -> ScalarApplicationId {
            self.application
        }
        pub fn checks(&self) -> ScalarEntryChecks {
            self.checks.clone()
        }
    }

    impl<'artifact> ScalarEntryPrerequisites<'artifact> {
        pub fn prepare(
            observations: &ScalarApplicationObservations<'artifact>,
            application: ScalarApplicationId,
        ) -> Result<Self, String> {
            let catalog = &observations.artifact().module().scalar_applications;
            let conditions = &catalog.conditions;
            let scope = &catalog.proof_scope;
            let entry = observations.entry(application)?;
            let mut rows = Vec::new();
            for (position, condition) in conditions.ranges().iter().enumerate() {
                if matches!(
                    condition,
                    ScalarRangeCondition::Range {
                        role: ScalarConditionRole::Prerequisite,
                        subject: ScalarConditionSubject::Declaration(_)
                            | ScalarConditionSubject::Application(_),
                        ..
                    }
                ) {
                    rows.push(ConditionId::Range(position));
                }
            }
            for (position, condition) in conditions.predicates().iter().enumerate() {
                if condition.role == ScalarConditionRole::Prerequisite
                    && !matches!(condition.subject, ScalarConditionSubject::Module)
                {
                    rows.push(ConditionId::Predicate(position));
                }
            }
            if !rows.is_empty() {
                let root = scope
                    .callable_declaration(TargetMachineCallable::Machine(entry))
                    .ok_or("scalar prerequisite entry has no checked declaration")?;
                let invocation = crate::compiler_evaluation::compiler_invocation_scope(
                    scope,
                    observations.targets().target(),
                    observations.providers(),
                    root,
                )?;
                let applies = |subject| -> Result<bool, String> {
                    Ok(match subject {
                        ScalarConditionSubject::Declaration(owner) => {
                            invocation.dependencies.contains(&owner)
                        }
                        ScalarConditionSubject::Application(owner) if owner == application => true,
                        ScalarConditionSubject::Application(owner) => {
                            match catalog.declaration(owner) {
                                Some(terminal_psi::ScalarApplication::Evaluate { .. }) => {
                                    let owner = scope.callable_declaration(TargetMachineCallable::Machine(observations.entry(owner)?))
                            .ok_or("scalar prerequisite owner has no checked entry declaration")?;
                                    invocation.dependencies.contains(&owner)
                                }
                                Some(terminal_psi::ScalarApplication::LayoutObservation {
                                    ..
                                }) => false,
                                _ => {
                                    return Err(
                                        "scalar prerequisite owner application is absent".into()
                                    );
                                }
                            }
                        }
                        ScalarConditionSubject::Module => false,
                    })
                };
                let mut retained = Vec::new();
                for row in rows {
                    let subject = match row {
                        ConditionId::Range(position) => match conditions.ranges()[position] {
                            ScalarRangeCondition::Range { subject, .. } => subject,
                            _ => return Err("scalar prerequisite range is absent".into()),
                        },
                        ConditionId::Predicate(position) => {
                            conditions.predicates()[position].subject
                        }
                    };
                    if applies(subject)? {
                        retained.push(row);
                    }
                }
                rows = retained;
            }
            Ok(Self {
                conditions,
                application,
                entry,
                target: observations.targets().target().to_owned(),
                providers: observations.providers().clone(),
                rows,
            })
        }

        fn local(&self, observation: &ScalarRangeObservation) -> bool {
            matches!(observation, ScalarRangeObservation::EntryResult { application, machine, .. } if *application == self.application && *machine == self.entry)
        }

        fn visit(
            &self,
            row: ConditionId,
            mut visit: impl FnMut(&ScalarRangeObservation) -> Result<(), String>,
        ) -> Result<(), String> {
            match row {
                ConditionId::Range(position) => {
                    for step in self.conditions.steps(&self.conditions.ranges()[position])? {
                        if let IntegerBoundStep::Observation(observation) = step {
                            visit(observation)?;
                        }
                    }
                }
                ConditionId::Predicate(position) => {
                    for (_, node) in self.conditions.predicates()[position].nodes().iter() {
                        if let PredicateNode::Leaf(observation) = &node.0 {
                            visit(observation)?;
                        }
                    }
                }
            }
            Ok(())
        }

        pub fn applications(&self) -> Result<Vec<ScalarApplicationId>, String> {
            let mut applications = Vec::new();
            for &row in &self.rows {
                self.visit(row, |observation| {
                    if !self.local(observation)
                        && let ScalarRangeObservation::Application(application)
                        | ScalarRangeObservation::EntryResult { application, .. } = observation
                        && !applications.contains(application)
                    {
                        applications.push(*application);
                    }
                    Ok(())
                })?;
            }
            Ok(applications)
        }

        pub fn dependency_reason(
            &self,
            dependency: ScalarApplicationId,
        ) -> Result<Option<String>, String> {
            for &row in &self.rows {
                let mut needs_value = false;
                self.visit(row, |observation| {
                needs_value |= !self.local(observation)
                    && matches!(observation, ScalarRangeObservation::Application(application)
                        | ScalarRangeObservation::EntryResult { application, .. } if *application == dependency);
                Ok(())
            })?;
                if needs_value {
                    return Ok(Some(match row {
                        ConditionId::Range(position) => format!("range condition {position}"),
                        ConditionId::Predicate(position) => {
                            self.conditions.predicates()[position].description.clone()
                        }
                    }));
                }
            }
            Ok(None)
        }

        pub fn admit<'input>(
            &self,
            observations: &'input ScalarApplicationObservations<'artifact>,
            address_model: Option<&ScalarAddressModel>,
        ) -> Result<AdmittedScalarEntry<'input, 'artifact>, String> {
            if !std::ptr::eq(
                self.conditions,
                &observations
                    .artifact()
                    .module()
                    .scalar_applications
                    .conditions,
            ) || self.target != observations.targets().target()
                || self.providers != *observations.providers()
                || self.entry != observations.entry(self.application)?
            {
                return Err("scalar prerequisites name a different artifact, invocation, target or provider selection".into());
            }
            let scope = &observations
                .artifact()
                .module()
                .scalar_applications
                .proof_scope;
            let root = scope
                .callable_declaration(TargetMachineCallable::Machine(self.entry))
                .ok_or("scalar prerequisite entry has no checked declaration")?;
            crate::compiler_evaluation::require_root_failure_admission(
                observations.artifact().module(),
                root,
            )?;
            let mut checks = ScalarEntryChecks::default();
            for &row in &self.rows {
                let mut inputs = Vec::new();
                self.visit(row, |observation| {
                    if inputs.iter().any(|(existing, _)| existing == observation) {
                        return Ok(());
                    }
                    let value = if self.local(observation) {
                        if let ScalarRangeObservation::EntryResult {
                            result,
                            scalar_type,
                            ..
                        } = observation
                        {
                            let capture = (*result, *scalar_type);
                            if !checks.results.contains(&capture) {
                                checks.results.push(capture);
                            }
                        }
                        None
                    } else {
                        Some(observations.integer(observation)?)
                    };
                    inputs.push((observation.clone(), value));
                    Ok(())
                })?;
                let condition = match row {
                    ConditionId::Range(position) => {
                        let range = &self.conditions.ranges()[position];
                        let ScalarRangeCondition::Range { requirement, .. } = range else {
                            return Err("scalar prerequisite range is absent".into());
                        };
                        let (minimum, maximum) = match requirement {
                            ScalarRangeRequirement::Fixed { minimum, maximum } => {
                                (minimum.clone(), maximum.clone())
                            }
                            ScalarRangeRequirement::AddressModel => {
                                let model = address_model
                                    .ok_or("scalar prerequisite needs an admitted address model")?;
                                if model.target != self.target
                                    || !model.carrier.is_address()
                                    || model.semantics_version == 0
                                {
                                    return Err("scalar prerequisite address model differs from the selected target".into());
                                }
                                (
                                    BigInt::zero(),
                                    BigInt::from_u64(1)
                                        .shl_bits(usize::from(model.carrier.bits()))
                                        .sub(&BigInt::from_u64(1)),
                                )
                            }
                        };
                        EntryCondition::Range {
                            position,
                            steps: self.conditions.steps(range)?.to_vec(),
                            minimum,
                            maximum,
                        }
                    }
                    ConditionId::Predicate(position) => {
                        EntryCondition::Predicate(self.conditions.predicates()[position].clone())
                    }
                };
                let mut check = EntryCheck {
                    condition,
                    inputs,
                    complete: false,
                };
                check.discharge_ready(&mut |_, _| Ok(None))?;
                if !check.complete {
                    checks.pending.push(check);
                }
            }
            Ok(AdmittedScalarEntry {
                observations,
                application: self.application,
                checks,
            })
        }
    }

    #[derive(Clone, Default)]
    pub struct ScalarEntryChecks {
        results: Vec<(ValueId, ScalarType)>,
        pending: Vec<EntryCheck>,
    }

    #[derive(Clone)]
    pub(crate) struct EntryCheck {
        condition: EntryCondition,
        inputs: Vec<(ScalarRangeObservation, Option<BigInt>)>,
        complete: bool,
    }

    #[derive(Clone)]
    pub(crate) enum EntryCondition {
        Range {
            position: usize,
            steps: Vec<IntegerBoundStep<ScalarRangeObservation>>,
            minimum: BigInt,
            maximum: BigInt,
        },
        Predicate(ScalarPredicateCondition),
    }

    impl ScalarEntryChecks {
        pub fn results(&self) -> &[(ValueId, ScalarType)] {
            &self.results
        }

        pub fn discharge_ready(
            &mut self,
            mut observe: impl FnMut(ValueId, ScalarType) -> Result<Option<TerminalScalarValue>, String>,
        ) -> Result<(), String> {
            for check in &mut self.pending {
                if !check.complete {
                    check.discharge_ready(&mut observe)?;
                }
            }
            Ok(())
        }

        pub fn require_complete(&self) -> Result<(), String> {
            if self.pending.iter().any(|check| !check.complete) {
                return Err("constant entry returned without establishing every prerequisite from its exact operand occurrences".into());
            }
            Ok(())
        }
    }

    impl EntryCheck {
        fn discharge_ready(
            &mut self,
            observe: &mut impl FnMut(ValueId, ScalarType) -> Result<Option<TerminalScalarValue>, String>,
        ) -> Result<(), String> {
            for (observation, value) in &mut self.inputs {
                if value.is_some() {
                    continue;
                }
                let ScalarRangeObservation::EntryResult {
                    result,
                    scalar_type,
                    ..
                } = observation
                else {
                    return Err("scalar prerequisite is missing an independent observation".into());
                };
                if let Some(observed) = observe(*result, *scalar_type)? {
                    if observed.scalar_type() != *scalar_type {
                        return Err("scalar prerequisite operand changed its carrier".into());
                    }
                    *value = Some(match observed {
                        TerminalScalarValue::Integer {
                            value: IntegerValue::Signed(value),
                            ..
                        } => BigInt::from_i128(value),
                        TerminalScalarValue::Integer {
                            value: IntegerValue::Unsigned(value),
                            ..
                        } => BigInt::from_u128(value),
                        _ => return Err("scalar prerequisite operand is not an integer".into()),
                    });
                }
            }
            if self.inputs.iter().any(|(_, value)| value.is_none()) {
                return Ok(());
            }
            let integer = |observation: &ScalarRangeObservation| {
                self.inputs
                    .iter()
                    .find(|(input, _)| input == observation)
                    .and_then(|(_, value)| value.clone())
                    .ok_or_else(|| "scalar prerequisite lost its captured operand".to_owned())
            };
            match &self.condition {
                EntryCondition::Range {
                    position,
                    steps,
                    minimum,
                    maximum,
                } => {
                    let (low, high) =
                        numerics::integer_bounds::evaluate(steps, &mut |observation| {
                            integer(observation)
                        })?;
                    if low < *minimum || high > *maximum {
                        return Err(format!(
                            "constant invocation prerequisite: scalar range condition {position}: bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
                        ));
                    }
                }
                EntryCondition::Predicate(condition) => {
                    match condition.evaluate(&mut |observation| {
                        integer(&observation).ok().map(ScalarValue::Integer)
                    }) {
                        Some(ScalarValue::Boolean(true)) => (),
                        Some(ScalarValue::Boolean(false)) => {
                            return Err(format!(
                                "constant invocation prerequisite: scalar predicate is false: {}",
                                condition.description
                            ));
                        }
                        _ => {
                            return Err(format!(
                                "constant invocation prerequisite could not be established: {}",
                                condition.description
                            ));
                        }
                    }
                }
            }
            self.complete = true;
            Ok(())
        }
    }
}
pub use conditions::ScalarConditionSelection;
pub use prerequisites::{AdmittedScalarEntry, ScalarEntryChecks, ScalarEntryPrerequisites};
#[cfg(test)]
pub(crate) mod abi_storage_tests {
    use super::*;
    use semantic_vocabulary::{CallingAbiIdentity, StructuralTypeId};
    use terminal_codec::CanonicalTerminalArtifact;
    use terminal_psi::{
        CallingAbiDeclaration, CallingAbiStorage, CallingSignatureTelescope, TerminalModule,
    };

    fn fixture(
        with_extents: bool,
        false_helper: bool,
    ) -> (CanonicalTerminalArtifact, Vec<CallingAbiIdentity>) {
        let mut module = TerminalModule::declarations();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let integer = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .unwrap(),
        );
        let mut keys = Vec::new();
        for receiver in ["Receiver<u8>", "Receiver<u16>"] {
            let key = CallingAbiIdentity {
                requirement_identity: "Receiver::inspect".into(),
                declaring_application: receiver.into(),
                operands: Vec::new(),
            };
            module.calling_abi_signatures.push(CallingAbiDeclaration {
                id: key.id(),
                requirement_identity: identities.intern(&key.requirement_identity),
                declaring_application: identities.intern(&key.declaring_application),
                operands: Vec::new(),
                formals: Vec::new(),
                default_eligible: true,
                physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::BoundaryTrait),
                storage: Ok(CallingAbiStorage {
                    parameters: Vec::new(),
                    result: with_extents.then(|| StructuralTypeId::new(3).unwrap()),
                    telescope: CallingSignatureTelescope {
                        parameters: Vec::new(),
                        binders: Vec::new(),
                        direct_callbacks: Vec::new(),
                    },
                }),
            });
            keys.push(key);
        }
        if with_extents {
            for (ordinal, shape) in [
                terminal_psi::StructuralTypeShape::PrimitiveScalar(integer),
                terminal_psi::StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(1).unwrap(),
                    length: terminal_psi::ArrayExtent::Application(
                        ScalarApplicationId::new(1).unwrap(),
                    ),
                },
                terminal_psi::StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length: terminal_psi::ArrayExtent::Application(
                        ScalarApplicationId::new(2).unwrap(),
                    ),
                },
            ]
            .into_iter()
            .enumerate()
            {
                module
                    .structural_types
                    .push(terminal_psi::StructuralTypeDeclaration {
                        requires: Vec::new(),
                        id: StructuralTypeId::new(ordinal as u64 + 1).unwrap(),
                        identity: identities.intern(&format!("storage-{ordinal}")),
                        quotient_carrier: None,
                        owns_linear_claim: false,
                        copyable: true,
                        no_code_affine_cleanup: true,
                        descriptor_cleanup: true,
                        scalar_restriction: None,
                        shape,
                    });
            }
            for measurement in [LayoutMeasurement::Size, LayoutMeasurement::Alignment] {
                module
                    .scalar_applications
                    .insert(ScalarApplication::LayoutObservation {
                        subject: StructuralTypeId::new(1).unwrap(),
                        measurement,
                        semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
                        scalar_type: integer,
                    })
                    .unwrap();
            }
        }
        if false_helper {
            let helper = MachineId::new(1).unwrap();
            let declaration = terminal_psi::ProofDeclarationId(1);
            let identity = identities.intern("extent-helper");
            module.scalar_applications.proof_scope.declarations.insert(
                terminal_psi::ProofScopeDeclaration::Callable(
                    terminal_psi::CheckedMachineDeclaration {
                        machine: terminal_psi::NominalDeclarationIdentity::new(identity),
                        state: terminal_psi::NominalDeclarationIdentity::new(identity),
                        semantic_declaration: identity,
                        parameter_types: Vec::new(),
                        callable: identity,
                        result_type: terminal_psi::TerminalIdentityIndex::ABSENT,
                        contract: [1; 32],
                        specialization: None,
                    },
                ),
            );
            module.scalar_applications.proof_scope.callables.insert(
                terminal_psi::ProofCallableAssociation::Callable {
                    declaration,
                    callable: terminal_psi::TargetMachineCallable::Machine(helper),
                },
            );
            module.scalar_applications.proof_scope.plan.machines.insert(
                semantic_vocabulary::proof_applications::ProofApplicationMachine {
                    machine: declaration,
                    product_selected: true,
                    ..Default::default()
                },
            );
            let mut nodes = Arena::new();
            let root = nodes.insert(terminal_psi::ScalarPredicateNode(
                numerics::scalar_predicates::PredicateNode::Value(ScalarValue::Boolean(false)),
            ));
            module
                .scalar_applications
                .conditions
                .retain_predicate(
                    terminal_psi::ScalarConditionSubject::Declaration(declaration),
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    "actual ABI extent helper debt".into(),
                    nodes,
                    root,
                )
                .unwrap();
        }
        module.identities = identities.build();
        module.calling_abi_signatures.sort_by_key(|row| row.id);
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
        (
            CanonicalTerminalArtifact::from_parts(module, optimization, None).unwrap(),
            keys,
        )
    }

    fn facts(target: &str) -> crate::LayoutTargetFacts {
        crate::LayoutTargetFacts {
            profile: target.into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        }
    }

    #[test]
    fn abi_storage_request_requires_exact_canonical_roster() {
        let (artifact, keys) = fixture(true, false);
        let demand =
            CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
        let expected = [
            ScalarApplicationId::new(1).unwrap(),
            ScalarApplicationId::new(2).unwrap(),
        ];
        assert_eq!(demand.applications(), expected);
        let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
        let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &facts("macos_arm64"),
        )
        .unwrap()
        .with_abi_storage(&demand)
        .unwrap();
        assert!(observations.conditions("macos_arm64", &expected).is_ok());
        for changed in [
            vec![],
            vec![expected[0]],
            vec![expected[1], expected[0]],
            vec![expected[0], expected[1], expected[0]],
        ] {
            let error = observations
                .conditions("macos_arm64", &changed)
                .err()
                .unwrap();
            assert!(error.contains("canonical extent roster"), "{error}");
        }
    }

    #[test]
    fn abi_storage_receipt_binds_full_key_and_grants_no_entry_or_product_authority() {
        let (artifact, keys) = fixture(false, false);
        let demands = keys
            .iter()
            .map(|key| CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", key).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(demands[0].applications(), demands[1].applications());
        let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
        let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let observations = || {
            ScalarApplicationObservations::new(
                artifact.view(),
                &targets,
                &providers,
                &facts("macos_arm64"),
            )
            .unwrap()
            .with_abi_storage(&demands[0])
            .unwrap()
        };
        let observed = observations();
        let conditions = observed.conditions("macos_arm64", &[]).unwrap();
        let mut wrong =
            ScalarApplicationClosure::for_abi_storage(artifact.view(), &demands[1]).unwrap();
        let error = wrong
            .close_scope(observed, conditions, Ok(None))
            .unwrap_err();
        assert!(error.contains("roots differ"), "{error}");
        let observed = observations();
        let conditions = observed.conditions("macos_arm64", &[]).unwrap();
        let mut closure =
            ScalarApplicationClosure::for_abi_storage(artifact.view(), &demands[0]).unwrap();
        closure.close_scope(observed, conditions, Ok(None)).unwrap();
        let completed = closure.finish().unwrap();
        assert!(completed.require_product_formation().is_err());
        assert!(
            !completed
                .primary()
                .admits_machine(artifact.semantic_module(), MachineId::new(1).unwrap())
        );
    }

    #[test]
    fn abi_storage_request_refuses_other_artifact_and_target() {
        let (artifact, keys) = fixture(false, false);
        let (other, _) = fixture(true, false);
        let demand =
            CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
        assert!(
            ScalarApplicationClosure::for_abi_storage(other.view(), &demand)
                .err()
                .unwrap()
                .contains("another Terminal artifact")
        );
        let targets = crate::admit_target_machine_selections(artifact.view(), "linux_arm64");
        let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let error = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &facts("linux_arm64"),
        )
        .unwrap()
        .with_abi_storage(&demand)
        .err()
        .unwrap();
        assert!(error.contains("another selected target"), "{error}");
    }

    #[test]
    fn abi_storage_completion_refuses_actual_false_helper_debt() {
        let (artifact, keys) = fixture(false, true);
        let demand =
            CallingAbiStorageDemand::new(artifact.view(), "macos_arm64", &keys[0]).unwrap();
        let targets = crate::admit_target_machine_selections(artifact.view(), "macos_arm64");
        let providers = crate::admit_provider_installation(artifact.view(), &[], &[]).unwrap();
        let observations = ScalarApplicationObservations::new(
            artifact.view(),
            &targets,
            &providers,
            &facts("macos_arm64"),
        )
        .unwrap()
        .with_abi_storage(&demand)
        .unwrap();
        let mut conditions = observations.conditions("macos_arm64", &[]).unwrap();
        conditions.discharge_ready(&observations).unwrap();
        conditions
            .include_evaluation_entry(MachineId::new(1).unwrap())
            .unwrap();
        let error = conditions.discharge_ready(&observations).unwrap_err();
        assert!(error.contains("actual ABI extent helper debt"), "{error}");
        let mut closure =
            ScalarApplicationClosure::for_abi_storage(artifact.view(), &demand).unwrap();
        let error = closure
            .close_scope(observations, conditions, Ok(None))
            .unwrap_err();
        assert!(error.contains("actual ABI extent helper debt"), "{error}");
    }
}
pub(crate) mod closure {
    use super::{ScalarApplicationObservations, ScalarApplicationValues, ScalarConditionSelection};
    use crate::{
        AdmittedProviderSelections, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    };
    use numerics::bignum::BigInt;
    use semantic_vocabulary::IntegerType;
    use terminal_codec::TerminalArtifactView;
    use terminal_psi::{
        ScalarCompletionCondition, ScalarConditionSubject, ScalarRangeCondition,
        TerminalPsiIdentity,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarAddressModel {
        pub target: String,
        pub carrier: IntegerType,
        pub semantics_version: u32,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ScalarScopeInputs {
        targets: AdmittedTargetMachineSelections,
        providers: AdmittedProviderSelections,
        layouts: AdmittedTargetLayouts,
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
        pub fn targets(&self) -> &AdmittedTargetMachineSelections {
            &self.targets
        }
        pub fn providers(&self) -> &AdmittedProviderSelections {
            &self.providers
        }
        pub fn layouts(&self) -> &AdmittedTargetLayouts {
            &self.layouts
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
        terminal: TerminalPsiIdentity,
        product_formation: bool,
        primary: CompletedScalarScope,
        supplemental: Vec<ScalarScopeInputs>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CompletedScalarScope {
        inputs: ScalarScopeInputs,
        values: ScalarApplicationValues,
        geometry: std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]>,
    }

    impl AdmittedScalarApplications {
        pub fn require_product_formation(&self) -> Result<(), String> {
            self.product_formation.then_some(()).ok_or_else(|| {
                "entry scalar admission does not establish Product declaration formation".into()
            })
        }

        pub fn terminal(&self) -> TerminalPsiIdentity {
            self.terminal
        }
        pub fn primary(&self) -> &ScalarScopeInputs {
            &self.primary.inputs
        }
        pub fn supplemental(&self) -> &[ScalarScopeInputs] {
            &self.supplemental
        }
        pub fn values(&self) -> &ScalarApplicationValues {
            &self.primary.values
        }
        pub fn geometry(&self) -> &std::sync::Arc<[terminal_psi::StructuralTypeDeclaration]> {
            &self.primary.geometry
        }
    }

    pub struct ScalarApplicationClosure<'artifact> {
        artifact: TerminalArtifactView<'artifact>,
        primary: String,
        scopes: Vec<(String, bool)>,
        ranges: Vec<bool>,
        predicates: Vec<bool>,
        completions: Vec<bool>,
        primary_result: Option<CompletedScalarScope>,
        supplemental: Vec<ScalarScopeInputs>,
        kind: ScalarClosureKind,
    }

    pub(crate) enum ScalarClosureKind {
        Source,
        Deployment,
        Invocation(Vec<terminal_psi::ProofDeclarationId>),
        AbiStorage(super::AbiStorageScope),
    }

    impl<'artifact> ScalarApplicationClosure<'artifact> {
        pub fn new(artifact: TerminalArtifactView<'artifact>, primary: &str) -> Self {
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
                .chain(
                    catalog.conditions.completions().iter().filter_map(
                        |condition| match condition {
                            ScalarCompletionCondition::Complete { subject, .. } => Some(*subject),
                            ScalarCompletionCondition::Missing => None,
                        },
                    ),
                );
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
                .map(|scope| (scope.to_owned(), false))
                .collect();
            Self::with_scopes(artifact, primary, scopes, ScalarClosureKind::Source)
        }

        fn with_scopes(
            artifact: TerminalArtifactView<'artifact>,
            primary: &str,
            scopes: Vec<(String, bool)>,
            kind: ScalarClosureKind,
        ) -> Self {
            let conditions = &artifact.module().scalar_applications.conditions;
            Self {
                artifact,
                primary: primary.to_owned(),
                scopes,
                ranges: vec![false; conditions.ranges().len()],
                predicates: vec![false; conditions.predicates().len()],
                completions: vec![false; conditions.completions().len()],
                primary_result: None,
                supplemental: Vec::new(),
                kind,
            }
        }

        pub fn for_invocation(
            artifact: TerminalArtifactView<'artifact>,
            primary: &str,
            roots: &[terminal_psi::ProofDeclarationId],
        ) -> Self {
            let mut declarations = Vec::with_capacity(roots.len());
            for &root in roots {
                if !declarations.contains(&root) {
                    declarations.push(root);
                }
            }
            Self::with_scopes(
                artifact,
                primary,
                vec![(primary.to_owned(), false)],
                ScalarClosureKind::Invocation(declarations),
            )
        }

        pub fn for_abi_storage(
            artifact: TerminalArtifactView<'artifact>,
            demand: &super::CallingAbiStorageDemand,
        ) -> Result<Self, String> {
            demand.require_artifact(artifact)?;
            Ok(Self::with_scopes(
                artifact,
                &demand.target,
                vec![(demand.target.clone(), false)],
                ScalarClosureKind::AbiStorage(demand.scope.clone()),
            ))
        }

        pub fn with_deployment_scopes(mut self, scopes: &[String]) -> Self {
            if matches!(
                self.kind,
                ScalarClosureKind::Invocation(_) | ScalarClosureKind::AbiStorage(_)
            ) {
                self = Self::new(self.artifact, &self.primary);
            }
            self.kind = ScalarClosureKind::Deployment;
            for scope in scopes {
                if !self.scopes.iter().any(|(target, _)| target == scope) {
                    self.scopes.push((scope.clone(), false));
                }
            }
            self
        }

        pub fn scopes(&self) -> impl Iterator<Item = &str> {
            self.scopes.iter().map(|(scope, _)| scope.as_str())
        }

        pub fn close_scope(
            &mut self,
            observations: ScalarApplicationObservations<'_>,
            conditions: ScalarConditionSelection<'_>,
            address_model: Result<Option<ScalarAddressModel>, String>,
        ) -> Result<(), String> {
            if observations.artifact().identity() != self.artifact.identity()
                || !conditions.matches(&observations, &self.primary)
            {
                return Err(
                    "scalar closure selection differs from its artifact or scope inputs".into(),
                );
            }
            let matching_scope = match (&self.kind, conditions.scope()) {
                (ScalarClosureKind::Source, super::ScalarConditionScope::Source) => true,
                (ScalarClosureKind::Deployment, super::ScalarConditionScope::SourceRoots(_)) => {
                    true
                }
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
            let target = observations.targets().target();
            let scope = self
                .scopes
                .iter()
                .position(|(scope, _)| scope == target)
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
                    return Err(
                        "completed scalar application has unselected completion debt".into(),
                    );
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
                if model.target != target
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
            let mut geometry = observations.layouts().resolved_records().to_vec();
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
                targets: observations.targets().clone(),
                providers: observations.providers().clone(),
                layouts: observations.layouts().clone(),
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
            if target == self.primary {
                self.primary_result = Some(CompletedScalarScope {
                    inputs,
                    values: observations.values(),
                    geometry: geometry.into(),
                });
            } else {
                self.supplemental.push(inputs);
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
                terminal: self.artifact.identity(),
                product_formation: !matches!(
                    self.kind,
                    ScalarClosureKind::Invocation(_) | ScalarClosureKind::AbiStorage(_)
                ),
                primary,
                supplemental: self.supplemental,
            })
        }
    }
}
pub use closure::{
    AdmittedScalarApplications, ScalarAddressModel, ScalarApplicationClosure, ScalarScopeInputs,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum ScalarConditionScope {
    #[default]
    Source,
    SourceRoots(Vec<terminal_psi::ProofDeclarationId>),
    Invocation(Vec<terminal_psi::ProofDeclarationId>),
    AbiStorage(AbiStorageScope),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbiStorageScope {
    declaration: semantic_vocabulary::CallingAbiIdentity,
    applications: Vec<ScalarApplicationId>,
}

fn abi_storage_scope(
    artifact: TerminalArtifactView<'_>,
    target: &str,
    declaration: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<AbiStorageScope, String> {
    let module = artifact.module();
    let retained = module
        .calling_abi_signature(declaration.id())
        .ok_or("ABI scalar demand has no canonical declaration")?;
    if retained.identity(&module.identities)? != *declaration {
        return Err("ABI scalar demand differs from its complete declaration key".into());
    }
    let applications =
        terminal_semantics::calling_abi_extent_dependencies(module, retained.id, target)?;
    Ok(AbiStorageScope {
        declaration: declaration.clone(),
        applications,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiStorageDemand {
    terminal: terminal_psi::TerminalPsiIdentity,
    target: String,
    scope: AbiStorageScope,
}

impl CallingAbiStorageDemand {
    pub fn new(
        artifact: TerminalArtifactView<'_>,
        target: &str,
        declaration: &semantic_vocabulary::CallingAbiIdentity,
    ) -> Result<Self, String> {
        Ok(Self {
            terminal: artifact.identity(),
            target: target.to_owned(),
            scope: abi_storage_scope(artifact, target, declaration)?,
        })
    }

    pub fn declaration(&self) -> &semantic_vocabulary::CallingAbiIdentity {
        &self.scope.declaration
    }

    pub fn applications(&self) -> &[ScalarApplicationId] {
        &self.scope.applications
    }

    pub fn with_borrowed_parameter(
        mut self,
        artifact: TerminalArtifactView<'_>,
        ordinal: usize,
    ) -> Result<Self, String> {
        self.require_artifact(artifact)?;
        let module = artifact.module();
        let storage = module
            .calling_abi_signature(self.declaration().id())
            .ok_or("borrowed storage demand lost its exact ABI")?
            .storage
            .as_ref()
            .map_err(Clone::clone)?;
        let parameter = storage
            .parameters
            .get(ordinal)
            .ok_or("borrowed storage demand has no such semantic formal")?;
        if !matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) {
            return Err("borrowed storage demand requires a readable formal".into());
        }
        let dependencies = terminal_semantics::foreign_storage_extent_dependencies(
            module,
            [parameter.structural_type],
            &self.target,
        )?;
        self.scope.applications.extend(dependencies);
        self.scope.applications.sort_unstable();
        self.scope.applications.dedup();
        Ok(self)
    }

    fn require_artifact(&self, artifact: TerminalArtifactView<'_>) -> Result<(), String> {
        if self.terminal != artifact.identity() {
            return Err("ABI storage demand belongs to another Terminal artifact".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ApplicationObservations {
    value: Option<TerminalScalarValue>,
    results: Arena<EntryResult>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum EntryResult {
    #[default]
    Missing,
    Observed {
        result: ValueId,
        value: TerminalScalarValue,
    },
}

pub struct ScalarApplicationObservations<'artifact> {
    artifact: TerminalArtifactView<'artifact>,
    targets: &'artifact AdmittedTargetMachineSelections,
    providers: &'artifact AdmittedProviderSelections,
    layouts: AdmittedTargetLayouts,
    carrier_layout: Option<semantic_vocabulary::TargetCarrierLayout>,
    applications: std::sync::Arc<Arena<ApplicationObservations>>,
    condition_scope: ScalarConditionScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarApplicationValues(std::sync::Arc<Arena<ApplicationObservations>>);

impl ScalarApplicationValues {
    pub fn value(&self, application: ScalarApplicationId) -> Option<TerminalScalarValue> {
        let position = usize::try_from(application.get() - 1).ok()?;
        self.0.storage_slice().get(position)?.value
    }

    pub fn observed(
        &self,
    ) -> impl Iterator<Item = (ScalarApplicationId, TerminalScalarValue)> + '_ {
        self.0
            .storage_slice()
            .iter()
            .enumerate()
            .filter_map(|(position, row)| {
                let value = row.value?;
                let application =
                    ScalarApplicationId::new(u64::try_from(position).ok()?.checked_add(1)?)?;
                Some((application, value))
            })
    }
}

impl<'artifact> ScalarApplicationObservations<'artifact> {
    pub fn new(
        artifact: TerminalArtifactView<'artifact>,
        targets: &'artifact AdmittedTargetMachineSelections,
        providers: &'artifact AdmittedProviderSelections,
        target_layout: &crate::LayoutTargetFacts,
    ) -> Result<Self, String> {
        if artifact.identity() != targets.terminal_psi()
            || artifact.identity() != providers.terminal_psi()
        {
            return Err("scalar observations name a different Terminal artifact".into());
        }
        if let Some(target) = artifact.module().folded_constants.target.as_deref()
            && (targets.target() != target || providers.folded_constant_target() != Some(target))
        {
            return Err(
                "scalar observations differ from the artifact's folded target closure".into(),
            );
        }
        let layouts = crate::admit_target_layouts(artifact, targets, target_layout)
            .map_err(|error| error.to_string())?;
        let mut applications = Arena::new();
        applications.insert_many(
            artifact
                .module()
                .scalar_applications
                .declarations()
                .iter()
                .map(|_| ApplicationObservations::default()),
        );
        Ok(Self {
            artifact,
            targets,
            providers,
            layouts,
            carrier_layout: None,
            applications: std::sync::Arc::new(applications),
            condition_scope: ScalarConditionScope::Source,
        })
    }

    pub fn artifact(&self) -> TerminalArtifactView<'artifact> {
        self.artifact
    }

    pub fn with_target_carriers(
        mut self,
        carriers: Option<semantic_vocabulary::TargetCarrierLayout>,
    ) -> Self {
        self.carrier_layout = carriers;
        self
    }

    pub fn carrier_layout(&self) -> Option<semantic_vocabulary::TargetCarrierLayout> {
        self.carrier_layout
    }
    pub fn targets(&self) -> &AdmittedTargetMachineSelections {
        self.targets
    }
    pub fn providers(&self) -> &AdmittedProviderSelections {
        self.providers
    }
    pub fn layouts(&self) -> &AdmittedTargetLayouts {
        &self.layouts
    }

    pub fn with_application_roots(
        mut self,
        roots: Option<&[terminal_psi::ProofDeclarationId]>,
    ) -> Result<Self, String> {
        let Some(roots) = roots else {
            return Ok(self);
        };
        let scope = &self.artifact.module().scalar_applications.proof_scope;
        let mut declarations = Vec::with_capacity(roots.len());
        for &root in roots {
            if !matches!(
                scope.declaration(root),
                Some(terminal_psi::ProofScopeDeclaration::Callable(_))
            ) {
                return Err(
                    "Build scalar application root has no exact checked proof declaration".into(),
                );
            }
            if !declarations.contains(&root) {
                declarations.push(root);
            }
        }
        for callable in self
            .providers
            .providers()
            .map(|(_, body)| terminal_psi::TargetMachineCallable::Machine(body))
            .chain(self.providers.assumed_bindings().iter().map(|binding| {
                terminal_psi::TargetMachineCallable::Machine(binding.implementation)
            }))
        {
            let owner = scope
                .callable_declaration(callable)
                .ok_or("selected scalar provider has no retained proof declaration")?;
            if !declarations.contains(&owner) {
                declarations.push(owner);
            }
        }
        self.condition_scope = ScalarConditionScope::SourceRoots(declarations);
        Ok(self)
    }

    pub fn with_invocation_roots(
        mut self,
        roots: &[terminal_psi::ProofDeclarationId],
    ) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("scalar invocation has no checked entry root".into());
        }
        let scope = &self.artifact.module().scalar_applications.proof_scope;
        let mut declarations = Vec::with_capacity(roots.len());
        for &root in roots {
            if !matches!(
                scope.declaration(root),
                Some(terminal_psi::ProofScopeDeclaration::Callable(_))
            ) {
                return Err("scalar invocation root has no exact checked proof declaration".into());
            }
            if !declarations.contains(&root) {
                declarations.push(root);
            }
        }
        self.condition_scope = ScalarConditionScope::Invocation(declarations);
        Ok(self)
    }

    pub fn with_abi_storage(mut self, demand: &CallingAbiStorageDemand) -> Result<Self, String> {
        demand.require_artifact(self.artifact)?;
        if demand.target != self.targets.target() {
            return Err("ABI storage demand names another selected target".into());
        }
        self.condition_scope = ScalarConditionScope::AbiStorage(demand.scope.clone());
        Ok(self)
    }

    pub fn values(&self) -> ScalarApplicationValues {
        ScalarApplicationValues(std::sync::Arc::clone(&self.applications))
    }

    pub fn conditions(
        &self,
        value_scope: &str,
        demanded: &[ScalarApplicationId],
    ) -> Result<ScalarConditionSelection<'artifact>, String> {
        if let ScalarConditionScope::AbiStorage(scope) = &self.condition_scope
            && scope.applications != demanded
        {
            return Err("ABI scalar request differs from its canonical extent roster".into());
        }
        let catalog = &self.artifact.module().scalar_applications;
        ScalarConditionSelection::select(
            &catalog.proof_scope,
            &catalog.conditions,
            self.targets.target(),
            self.targets.target() == value_scope,
            self.providers.providers().chain(
                self.providers
                    .assumed_bindings()
                    .iter()
                    .map(|binding| (binding.required, binding.implementation)),
            ),
            demanded,
            &self.condition_scope,
        )
    }

    fn position(&self, application: ScalarApplicationId) -> Result<usize, String> {
        let position = usize::try_from(application.get() - 1)
            .map_err(|_| "scalar application coordinate exceeds the host range")?;
        if position >= self.applications.len() {
            return Err(format!("unknown scalar application {}", application.get()));
        }
        Ok(position)
    }

    pub fn value(
        &self,
        application: ScalarApplicationId,
    ) -> Result<Option<TerminalScalarValue>, String> {
        Ok(self.applications.storage_slice()[self.position(application)?].value)
    }

    pub fn entry(&self, application: ScalarApplicationId) -> Result<MachineId, String> {
        let Some(ScalarApplication::Evaluate { entry, scalar_type }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
        else {
            return Err("scalar application is not a complete evaluation entry".into());
        };
        let machine = self
            .targets
            .entry(self.artifact.identity(), self.artifact.module(), *entry)
            .map_err(|error| error.to_string())?;
        let declaration = self
            .artifact
            .module()
            .machine(machine)
            .ok_or("scalar evaluation entry is absent")?;
        if !declaration.parameters.is_empty()
            || !declaration.structural_parameters.is_empty()
            || !matches!(&declaration.result, terminal_psi::TerminalMachineResult::Scalar(result) if result.scalar_type == *scalar_type)
        {
            return Err(
                "scalar evaluation entry has runtime inputs or a different result carrier".into(),
            );
        }
        Ok(machine)
    }

    pub fn record_value(
        &mut self,
        application: ScalarApplicationId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        self.entry(application)?;
        self.store_value(application, value)
    }

    fn store_value(
        &mut self,
        application: ScalarApplicationId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        let position = self.position(application)?;
        let expected = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
            .and_then(|row| row.scalar_type())
            .ok_or("scalar application has no carrier")?;
        validate_value(value, expected)?;
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if row.value.is_some() {
            return Err("scalar application completed more than once".into());
        }
        row.value = Some(value);
        Ok(())
    }

    pub fn record_entry_result(
        &mut self,
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        value: TerminalScalarValue,
    ) -> Result<(), String> {
        if machine != self.entry(application)? {
            return Err("scalar observation names another entry activation".into());
        }
        let expected = self
            .artifact
            .module()
            .scalar_applications
            .conditions
            .observations()
            .find_map(|row| match row {
                ScalarRangeObservation::EntryResult {
                    application: owner,
                    machine: entry,
                    result: observed,
                    scalar_type,
                } if *owner == application && *entry == machine && *observed == result => {
                    Some(*scalar_type)
                }
                _ => None,
            })
            .ok_or("scalar observation answers no retained entry result")?;
        validate_value(value, expected)?;
        let position = self.position(application)?;
        let applications = std::sync::Arc::make_mut(&mut self.applications);
        let handle = applications
            .handle_of(&applications.storage_slice()[position])
            .ok_or("scalar application storage lost its row")?;
        let row = applications.get_mut(handle);
        if row.results.storage_slice().iter().any(|row| matches!(row, EntryResult::Observed { result: observed, .. } if *observed == result)) {
            return Err("scalar entry result was recorded more than once".into());
        }
        row.results.insert(EntryResult::Observed { result, value });
        Ok(())
    }

    pub fn entry_result(
        &self,
        application: ScalarApplicationId,
        machine: MachineId,
        result: ValueId,
        scalar_type: ScalarType,
    ) -> Result<TerminalScalarValue, String> {
        if self.entry(application)? != machine {
            return Err("scalar condition names another entry activation".into());
        }
        let row = &self.applications.storage_slice()[self.position(application)?];
        if row.value.is_none() {
            return Err("scalar entry has not completed normally".into());
        }
        let value = row
            .results
            .storage_slice()
            .iter()
            .find_map(|row| match row {
                EntryResult::Observed {
                    result: observed,
                    value,
                } if *observed == result => Some(*value),
                _ => None,
            })
            .ok_or("scalar condition has an unobserved entry result")?;
        validate_value(value, scalar_type)?;
        Ok(value)
    }

    pub fn layout_dependencies(
        &self,
        application: ScalarApplicationId,
    ) -> Result<Vec<ScalarApplicationId>, String> {
        let Some(ScalarApplication::LayoutObservation { subject, .. }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
        else {
            return Err("application is not a layout observation".into());
        };
        let geometry = self
            .layouts
            .geometry(self.artifact)
            .map_err(|error| error.to_string())?;
        ReferenceStorageGeometry::new(geometry.declarations(), |application| {
            Err(StorageGeometryError::UnresolvedExtent(application))
        })
        .with_target_carriers(self.carrier_layout)
        .dependencies(*subject)
        .map_err(|error| error.to_string())
    }

    pub fn observe_layout(
        &mut self,
        application: ScalarApplicationId,
    ) -> Result<TerminalScalarValue, ScalarLayoutObservationError> {
        let Some(ScalarApplication::LayoutObservation {
            subject,
            measurement,
            semantics_version,
            scalar_type,
        }) = self
            .artifact
            .module()
            .scalar_applications
            .declaration(application)
            .copied()
        else {
            return Err(ScalarLayoutObservationError::Invalid(
                "application is not a layout observation".into(),
            ));
        };
        if semantics_version != LayoutMeasurement::SEMANTICS_VERSION {
            return Err(ScalarLayoutObservationError::Invalid(
                "unsupported scalar layout semantics".into(),
            ));
        }
        let geometry = self
            .layouts
            .geometry(self.artifact)
            .map_err(|error| ScalarLayoutObservationError::Invalid(error.to_string()))?;
        let mut input_error = None;
        let mut geometry = ReferenceStorageGeometry::new(geometry.declarations(), |dependency| {
            match self.extent(dependency) {
                Ok(Some(length)) => Ok(length),
                Ok(None) => Err(StorageGeometryError::UnresolvedExtent(dependency)),
                Err(reason) => {
                    input_error = Some(reason);
                    Err(StorageGeometryError::UnresolvedExtent(dependency))
                }
            }
        })
        .with_target_carriers(self.carrier_layout);
        let layout = geometry.layout(subject);
        drop(geometry);
        if let Some(reason) = input_error {
            return Err(ScalarLayoutObservationError::Invalid(reason));
        }
        let (size, alignment) = layout.map_err(ScalarLayoutObservationError::Geometry)?;
        let ScalarType::Integer(integer_type) = scalar_type else {
            return Err(ScalarLayoutObservationError::Invalid(
                "layout observation has a non-integer carrier".into(),
            ));
        };
        let value = TerminalScalarValue::Integer {
            scalar_type: integer_type,
            value: IntegerValue::Unsigned(u128::from(match measurement {
                LayoutMeasurement::Size => size,
                LayoutMeasurement::Alignment => alignment,
            })),
        };
        self.store_value(application, value)
            .map_err(ScalarLayoutObservationError::Invalid)?;
        Ok(value)
    }

    pub fn extent(&self, application: ScalarApplicationId) -> Result<Option<u64>, String> {
        match self.value(application)? {
            None => Ok(None),
            Some(TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            }) => u64::try_from(value)
                .map(Some)
                .map_err(|_| "array extent exceeds u64".into()),
            Some(TerminalScalarValue::Integer {
                value: IntegerValue::Signed(value),
                ..
            }) => u64::try_from(value)
                .map(Some)
                .map_err(|_| "array extent is negative or exceeds u64".into()),
            Some(_) => Err("array extent requires an integer result".into()),
        }
    }

    fn observation_ready(&self, observation: &ScalarRangeObservation) -> Result<bool, String> {
        let ready = match observation {
            ScalarRangeObservation::Literal(_) => return Ok(true),
            ScalarRangeObservation::Application(application) => {
                let scalar_type = self
                    .artifact
                    .module()
                    .scalar_applications
                    .declaration(*application)
                    .and_then(|row| row.scalar_type())
                    .ok_or("scalar condition application has no carrier")?;
                if !matches!(scalar_type, ScalarType::Integer(_)) {
                    return Err("scalar condition requires an integer observation".into());
                }
                self.value(*application)?.is_some()
            }
            ScalarRangeObservation::EntryResult {
                application,
                machine,
                scalar_type,
                ..
            } => {
                if self.entry(*application)? != *machine {
                    return Err("scalar condition names another entry activation".into());
                }
                if !matches!(scalar_type, ScalarType::Integer(_)) {
                    return Err("scalar condition requires an integer observation".into());
                }
                self.value(*application)?.is_some()
            }
            ScalarRangeObservation::Missing => {
                return Err("scalar condition has an absent observation".into());
            }
        };
        if ready {
            self.integer(observation)?;
        }
        Ok(ready)
    }

    fn integer(&self, observation: &ScalarRangeObservation) -> Result<BigInt, String> {
        let value = match observation {
            ScalarRangeObservation::Literal(value) => return Ok(value.clone()),
            ScalarRangeObservation::Application(application) => self
                .value(*application)?
                .ok_or("scalar condition has an unevaluated application")?,
            ScalarRangeObservation::EntryResult {
                application,
                machine,
                result,
                scalar_type,
            } => self.entry_result(*application, *machine, *result, *scalar_type)?,
            ScalarRangeObservation::Missing => {
                return Err("scalar condition has an absent observation".into());
            }
        };
        match value {
            TerminalScalarValue::Integer {
                value: IntegerValue::Signed(value),
                ..
            } => Ok(BigInt::from_i128(value)),
            TerminalScalarValue::Integer {
                value: IntegerValue::Unsigned(value),
                ..
            } => Ok(BigInt::from_u128(value)),
            _ => Err("scalar condition requires an integer observation".into()),
        }
    }

    pub fn discharge_range(
        &self,
        position: usize,
        address_model: Option<(&BigInt, &BigInt)>,
    ) -> Result<(), String> {
        let conditions = &self.artifact.module().scalar_applications.conditions;
        let condition = conditions
            .ranges()
            .get(position)
            .ok_or("scalar range condition is absent")?;
        let terminal_psi::ScalarRangeCondition::Range { requirement, .. } = condition else {
            return Err("scalar range condition is absent".into());
        };
        let (minimum, maximum) = match requirement {
            ScalarRangeRequirement::Fixed { minimum, maximum } => (minimum, maximum),
            ScalarRangeRequirement::AddressModel => {
                address_model.ok_or("scalar range condition needs an admitted address model")?
            }
        };
        if minimum > maximum {
            return Err("scalar range condition has inverted required bounds".into());
        }
        let (low, high) =
            numerics::integer_bounds::evaluate(conditions.steps(condition)?, &mut |observation| {
                self.integer(observation)
            })?;
        if low < *minimum || high > *maximum {
            return Err(format!(
                "scalar range condition {position}: bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
            ));
        }
        Ok(())
    }

    pub fn discharge_predicate(&self, position: usize) -> Result<(), String> {
        let condition = self
            .artifact
            .module()
            .scalar_applications
            .conditions
            .predicates()
            .get(position)
            .ok_or("scalar predicate condition is absent")?;
        let mut missing = None;
        let result = condition.evaluate(&mut |observation| match self.integer(&observation) {
            Ok(value) => Some(ScalarValue::Integer(value)),
            Err(reason) => {
                missing = Some(reason);
                None
            }
        });
        if let Some(reason) = missing {
            return Err(reason);
        }
        match result {
            Some(ScalarValue::Boolean(true)) => Ok(()),
            Some(ScalarValue::Boolean(false)) => Err(format!(
                "scalar predicate is false: {}",
                condition.description
            )),
            _ => Err(format!(
                "scalar predicate could not be established: {}",
                condition.description
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarLayoutObservationError {
    Geometry(StorageGeometryError),
    Invalid(String),
}

impl std::fmt::Display for ScalarLayoutObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Geometry(error) => error.fmt(formatter),
            Self::Invalid(reason) => formatter.write_str(reason),
        }
    }
}
impl std::error::Error for ScalarLayoutObservationError {}

fn validate_value(value: TerminalScalarValue, expected: ScalarType) -> Result<(), String> {
    if value.scalar_type() != expected {
        return Err("scalar observation has a different carrier".into());
    }
    if let TerminalScalarValue::Integer { scalar_type, value } = value
        && !scalar_type.admits(value)
    {
        return Err("scalar observation does not fit its integer carrier".into());
    }
    Ok(())
}

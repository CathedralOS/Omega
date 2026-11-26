use numerics::integer_bounds::IntegerBoundStep;
use numerics::scalar_predicates::PredicateNode;
use semantic_vocabulary::{BoundaryMachineId, MachineId, ScalarApplicationId};
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ScalarCompletionCondition, ScalarConditionSubject,
    ScalarConditions, ScalarRangeCondition, ScalarRangeObservation, ScalarRangeRequirement,
    TargetMachineCallable,
};

use terminal_semantics::scalar_conditions::{
    ConditionPosition, invocation_declarations, proof_provider_edges, visit_condition_applications,
    visit_condition_rows,
};
pub(super) fn invocation_membership(
    scope: &ProofApplicationScope,
    target: Option<&str>,
    providers: &[(ProofDeclarationId, ProofDeclarationId)],
    roots: &[ProofDeclarationId],
) -> (Vec<ProofDeclarationId>, Vec<ProofDeclarationId>) {
    let machines = scope
        .applications()
        .invocation_reachability(target, providers, roots)
        .machines;
    let declarations = invocation_declarations(scope, roots, &machines);
    (machines, declarations)
}

pub struct ScalarConditionSelection<'artifact> {
    proof_scope: &'artifact ProofApplicationScope,
    conditions: &'artifact ScalarConditions,
    target: Option<String>,
    primary: bool,
    providers: Vec<(BoundaryMachineId, MachineId)>,
    ranges: Vec<usize>,
    predicates: Vec<usize>,
    completions: Vec<usize>,
    application_machines: Option<Vec<terminal_psi::ProofDeclarationId>>,
    condition_scope: super::ScalarConditionScope,
}

impl<'artifact> ScalarConditionSelection<'artifact> {
    pub(crate) fn select<'scope>(
        scope: &'artifact ProofApplicationScope,
        conditions: &'artifact ScalarConditions,
        target: impl Into<Option<&'scope str>>,
        primary: bool,
        providers: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
        demanded: &[ScalarApplicationId],
        condition_scope: &super::ScalarConditionScope,
    ) -> Result<Self, String> {
        let target = target.into();
        let providers: Vec<_> = providers.into_iter().collect();
        let selected = proof_provider_edges(scope, providers.iter().copied())?;
        let applications = scope.applications();
        let (machines, declarations) = match condition_scope {
            super::ScalarConditionScope::Source => {
                applications.membership(target, primary, &selected, None)
            }
            super::ScalarConditionScope::SourceRoots(roots) => {
                applications.membership(target, primary, &selected, Some(roots))
            }
            super::ScalarConditionScope::Invocation(roots) => {
                invocation_membership(scope, target, &selected, roots)
            }
            super::ScalarConditionScope::AbiStorage(_) => (Vec::new(), Vec::new()),
        };
        let source_formation = !matches!(
            condition_scope,
            super::ScalarConditionScope::Invocation(_) | super::ScalarConditionScope::AbiStorage(_)
        );
        let all_declarations = applications.declarations();
        let applies = |subject, address_model| match subject {
            ScalarConditionSubject::Module => true,
            ScalarConditionSubject::Declaration(owner) => {
                machines.contains(&owner)
                    || (address_model
                        && (declarations.contains(&owner)
                            || (source_formation && primary && all_declarations.contains(&owner))))
            }
            ScalarConditionSubject::Application(application) => demanded.contains(&application),
        };
        let mut ranges = Vec::new();
        let mut predicates = Vec::new();
        let mut completions = Vec::new();
        visit_condition_rows(conditions, |subject, address_model, position| {
            if applies(subject, address_model) {
                match position {
                    ConditionPosition::Range(position) => ranges.push(position),
                    ConditionPosition::Predicate(position) => predicates.push(position),
                    ConditionPosition::Completion(position) => completions.push(position),
                }
            }
        })?;
        Ok(Self {
            proof_scope: scope,
            conditions,
            target: target.map(str::to_owned),
            primary,
            providers,
            ranges,
            predicates,
            completions,
            application_machines: (!matches!(condition_scope, super::ScalarConditionScope::Source))
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

    pub(super) fn include_completed(&mut self, previous: &super::CompletedScalarConditions) {
        self.ranges.extend(&previous.ranges);
        self.predicates.extend(&previous.predicates);
        self.completions.extend(&previous.completions);
        self.ranges.sort_unstable();
        self.predicates.sort_unstable();
        self.completions.sort_unstable();
        self.ranges.dedup();
        self.predicates.dedup();
        self.completions.dedup();
    }

    pub fn include_evaluation_entry(&mut self, entry: MachineId) -> Result<(), String> {
        if !matches!(
            self.condition_scope,
            super::ScalarConditionScope::Invocation(_) | super::ScalarConditionScope::AbiStorage(_)
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
            self.target.as_deref(),
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
        primary: Option<&str>,
    ) -> bool {
        std::ptr::eq(
            self.conditions,
            &observations
                .artifact()
                .module()
                .scalars.scalar_applications
                .conditions,
        ) && self.target.as_deref() == observations.target()
            && self.primary == (self.target.as_deref() == primary)
            && self.condition_scope == observations.condition_scope
            && self
                .providers
                .iter()
                .copied()
                .eq(observations.providers().providers())
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
                .scalars.scalar_applications
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

    fn rows(&self) -> impl Iterator<Item = ConditionPosition> + '_ {
        self.completions
            .iter()
            .copied()
            .map(ConditionPosition::Completion)
            .chain(self.ranges.iter().copied().map(ConditionPosition::Range))
            .chain(
                self.predicates
                    .iter()
                    .copied()
                    .map(ConditionPosition::Predicate),
            )
    }

    pub fn applications(&self) -> Result<Vec<ScalarApplicationId>, String> {
        let mut applications = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for row in self.rows() {
            visit_condition_applications(self.conditions, row, |application| {
                if seen.insert(application) {
                    applications.push(application);
                }
            })?;
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
    use bignum::BigInt;
    use numerics::scalar_predicates::ScalarValue;
    use proof_applications::{
        ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
        ProofDeclarationDependencies,
    };
    use terminal_psi::{
        ProofCallableAssociation, ProofDeclarationId, ProofScopeDeclaration, ScalarPredicateNode,
        TerminalIdentityIndex,
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
        use proof_applications::ProofApplicationTargetMember;
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

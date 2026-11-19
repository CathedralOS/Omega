use crate::scalar_conditions::{
    ConditionPosition, visit_condition_applications, visit_condition_rows,
};
use semantic_vocabulary::ScalarApplicationId;
use terminal_psi::{ProofDeclarationId, ScalarConditionSubject, ScalarConditions};

// The selected source scopes can overlap. Index application subjects once
// and follow each selected condition's observations once across all roots.
pub(super) struct ScalarConditionDemand<'artifact> {
    conditions: &'artifact ScalarConditions,
    rows: Vec<(ScalarConditionSubject, bool, ConditionPosition)>,
    has_invocation_conditions: bool,
    visited_ranges: Vec<bool>,
    visited_predicates: Vec<bool>,
    visited_completions: Vec<bool>,
}

impl<'artifact> ScalarConditionDemand<'artifact> {
    pub(super) fn new(conditions: &'artifact ScalarConditions) -> Result<Self, String> {
        let mut demand = Self {
            conditions,
            rows: Vec::new(),
            has_invocation_conditions: false,
            visited_ranges: vec![false; conditions.ranges().len()],
            visited_predicates: vec![false; conditions.predicates().len()],
            visited_completions: vec![false; conditions.completions().len()],
        };
        visit_condition_rows(conditions, |subject, address_model, row| {
            demand.rows.push((subject, address_model, row));
            demand.has_invocation_conditions |=
                matches!(subject, ScalarConditionSubject::Declaration(_));
        })?;
        demand.rows.sort_unstable_by_key(|(subject, _, _)| *subject);
        Ok(demand)
    }

    pub(super) fn has_invocation_conditions(&self) -> bool {
        self.has_invocation_conditions
    }

    pub(super) fn include_module(
        &mut self,
        visit: impl FnMut(ScalarApplicationId),
    ) -> Result<(), String> {
        self.include_subject(ScalarConditionSubject::Module, false, visit)
    }

    pub(super) fn include_invocation(
        &mut self,
        machines: &[ProofDeclarationId],
        declarations: &[ProofDeclarationId],
        mut visit: impl FnMut(ScalarApplicationId),
    ) -> Result<(), String> {
        for &owner in machines {
            self.include_subject(
                ScalarConditionSubject::Declaration(owner),
                false,
                &mut visit,
            )?;
        }
        for &owner in declarations {
            self.include_subject(ScalarConditionSubject::Declaration(owner), true, &mut visit)?;
        }
        Ok(())
    }

    pub(super) fn include_application(
        &mut self,
        application: ScalarApplicationId,
        visit: impl FnMut(ScalarApplicationId),
    ) -> Result<(), String> {
        self.include_subject(
            ScalarConditionSubject::Application(application),
            false,
            visit,
        )
    }

    fn include_subject(
        &mut self,
        subject: ScalarConditionSubject,
        address_model_only: bool,
        mut visit: impl FnMut(ScalarApplicationId),
    ) -> Result<(), String> {
        let first = self.rows.partition_point(|(key, _, _)| *key < subject);
        for index in first..self.rows.len() {
            let (key, address_model, row) = self.rows[index];
            if key != subject {
                break;
            }
            if !address_model_only || address_model {
                self.include_row(row, &mut visit)?;
            }
        }
        Ok(())
    }

    fn include_row(
        &mut self,
        row: ConditionPosition,
        visit: impl FnMut(ScalarApplicationId),
    ) -> Result<(), String> {
        let visited = match row {
            ConditionPosition::Range(position) => &mut self.visited_ranges[position],
            ConditionPosition::Predicate(position) => &mut self.visited_predicates[position],
            ConditionPosition::Completion(position) => &mut self.visited_completions[position],
        };
        if !std::mem::replace(visited, true) {
            visit_condition_applications(self.conditions, row, visit)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar_conditions::invocation_declarations;
    use arena::Arena;
    use bignum::BigInt;
    use numerics::scalar_predicates::ScalarValue;
    use numerics::{integer_bounds::IntegerBoundStep, scalar_predicates::PredicateNode};
    use proof_applications::{
        ProofApplicationCall, ProofApplicationMachine, ProofCallMachine,
        ProofDeclarationDependencies,
    };
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use terminal_psi::{
        ProofApplicationScope, ScalarRangeObservation, ScalarRangeRequirement,
        TargetMachineCallable,
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
    fn scalar_demand_visits_only_reached_conditions_across_shared_roots() {
        let (scope, mut conditions) = fixture();
        let application = |id| ScalarApplicationId::new(id).unwrap();
        let address = ScalarRangeRequirement::AddressModel;
        let fixed = ScalarRangeRequirement::Fixed {
            minimum: BigInt::from_u64(0),
            maximum: BigInt::from_u64(255),
        };
        for (subject, requirement, dependency) in [
            (
                ScalarConditionSubject::Declaration(ProofDeclarationId(2)),
                fixed.clone(),
                2,
            ),
            (
                ScalarConditionSubject::Declaration(ProofDeclarationId(3)),
                fixed.clone(),
                3,
            ),
            (
                ScalarConditionSubject::Declaration(ProofDeclarationId(4)),
                address,
                4,
            ),
            (
                ScalarConditionSubject::Declaration(ProofDeclarationId(4)),
                fixed.clone(),
                6,
            ),
            (ScalarConditionSubject::Module, fixed.clone(), 8),
            (
                ScalarConditionSubject::Application(application(7)),
                fixed,
                9,
            ),
        ] {
            conditions
                .retain_range(
                    subject,
                    semantic_vocabulary::ScalarConditionRole::Prerequisite,
                    requirement,
                    vec![IntegerBoundStep::Observation(
                        ScalarRangeObservation::Application(application(dependency)),
                    )],
                )
                .unwrap();
        }

        let proof = scope.applications();
        let providers = [(ProofDeclarationId(6), ProofDeclarationId(2))];
        let mut walk = proof.invocation_machines_walk(Some("macos_arm64"), &providers);
        let mut demand = ScalarConditionDemand::new(&conditions).unwrap();
        let mut roots = vec![ProofDeclarationId(1)];
        let machines = walk(&roots);
        let declarations = invocation_declarations(&scope, &roots, &machines);
        let mut reached = std::collections::BTreeSet::new();
        demand
            .include_module(|id| {
                reached.insert(id);
            })
            .unwrap();
        demand
            .include_invocation(&machines, &declarations, |id| {
                reached.insert(id);
            })
            .unwrap();
        assert_eq!(
            reached.into_iter().collect::<Vec<_>>(),
            [2, 4, 7, 8].map(application)
        );

        roots.extend([ProofDeclarationId(2), ProofDeclarationId(3)]);
        let machines = walk(&roots);
        assert_eq!(machines, [ProofDeclarationId(3)]);
        let declarations = invocation_declarations(&scope, &roots, &machines);
        let mut reached = std::collections::BTreeSet::new();
        demand
            .include_invocation(&machines, &declarations, |id| {
                reached.insert(id);
            })
            .unwrap();
        demand
            .include_application(application(7), |id| {
                reached.insert(id);
            })
            .unwrap();
        assert_eq!(
            reached.into_iter().collect::<Vec<_>>(),
            [3, 7, 9].map(application)
        );
        demand
            .include_application(application(7), |_| panic!("condition revisited"))
            .unwrap();
    }
}

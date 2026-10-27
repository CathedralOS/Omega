//! Exact selected execution for deferred source constant evaluation.
//! The ordinary selected ProviderPlan set is the authority. Each fold lands
//! its value in the typed program once; final checking consumes that program
//! and the compile does not replay the folds against its own checked result
//! (pipeline.md, "Verification placement").
use build_time_evaluation::{
    PreCheckEvaluation, SelectedBuildTimeBinaryOperator, SelectedBuildTimeOperators,
    SelectedBuildTimeProviderBody,
};
use checked_trees::CheckedProviderPlanCommitment;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedConstantEvaluation {
    operators: Vec<SelectedBuildTimeBinaryOperator>,
    /// Selected uses whose meaning is the selected provider's ordinary checked
    /// machine body. Each row carries the exact provider entry the semantic
    /// evaluation owner rebinds to on its private evaluation copy.
    provider_bodies: Vec<SelectedBuildTimeProviderBody>,
    target_bodies: Vec<build_time_evaluation::SelectedBuildTimeTargetBody>,
    /// The exact selected target the deferred folds are answered under — the
    /// explicit target custody a retained `size_of`/`align_of` query names its
    /// subject from. Never the host's facts: a `--target` build folds every
    /// query against this target's layout closure.
    target_profile: target::TargetProfile,
    /// Only these declaration roots own folds; canonical arenas also retain
    /// Product descriptions during Build admission.
    selection: typed_trees::references::DeclarationSelection,
    /// The opaque representation selections the layout authority consults for
    /// the same fold; Build admission has none and supplies an empty set.
    opaque_representation_selections: Vec<representation_planning::OpaqueRepresentationSelection>,
}

impl SelectedConstantEvaluation {
    /// Resume pending Psi evaluation under the exact selected providers.
    /// Consume continuations in their original order against canonical storage.
    /// Preparation may select an executable declaration projection of that same
    /// storage; only its occurrences acquire selected execution. Each fold
    /// lands its value in `typed`.
    ///
    /// Retained `size_of`/`align_of` queries fold first, under the selected
    /// target's custody: every typed call carrying a `layout_query` becomes
    /// the `u64` literal the layout authority reports for this target, in
    /// constant and runtime positions alike, before the deferred
    /// continuations' own evaluation observes the program.
    pub fn evaluate_pending(
        self,
        typed: &mut TypedTrees,
        pending: Vec<PreCheckEvaluation>,
    ) -> Result<(), Vec<Diagnostic>> {
        build_time_evaluation::SelectedConstantEvaluationRequest {
            program: typed,
            pending,
            operators: SelectedBuildTimeOperators {
                operators: &self.operators,
                provider_bodies: &self.provider_bodies,
            },
            target_bodies: &self.target_bodies,
            selection: &self.selection,
            layouts: &provider_planning::data_layout_queries::TargetLayoutQueries {
                target: self.target_profile,
                opaque_representations: &self.opaque_representation_selections,
            },
        }
        .evaluate()
    }

    /// The provider rows this target's pending constant folds are evaluated
    /// under — the part of the selection every realized target must agree on,
    /// since each fold lands its value in the one checked program.
    /// `target_profile` and the opaque representation selections are legitimate
    /// per-target layout custody and are not part of the agreement.
    pub fn pending_fold_providers(
        &self,
    ) -> (
        &[SelectedBuildTimeBinaryOperator],
        &[SelectedBuildTimeProviderBody],
    ) {
        (&self.operators, &self.provider_bodies)
    }

    /// Close a retained Product application without mutating its shared source.
    /// Selection owns dependency receipts; Psi owns evaluation and value rejoining.
    pub fn close_constants(
        self,
        retained: &build_time_evaluation::RetainedConstantEvaluation,
        checked: &TypedTrees,
    ) -> Result<TypedTrees, Vec<Diagnostic>> {
        use typed_trees::constant_recipes::ConstantRecipeDependencyReceipt;

        let mut receipts: Vec<_> = self
            .operators
            .iter()
            .map(|row| ConstantRecipeDependencyReceipt {
                requirement: row.requirement,
                plan_identity: *row.provider.as_bytes(),
                target_scope: self.target_profile.target_name().to_owned(),
                provider_declaration: Default::default(),
                provider_machine: Default::default(),
            })
            .chain(
                self.provider_bodies
                    .iter()
                    .map(|row| ConstantRecipeDependencyReceipt {
                        requirement: row.requirement,
                        plan_identity: *row.provider.as_bytes(),
                        target_scope: row.target_scope.clone(),
                        provider_declaration: row.provider_declaration,
                        provider_machine: row.provider_machine,
                    }),
            )
            .collect();
        receipts.sort_by(|left, right| {
            left.target_scope
                .cmp(&right.target_scope)
                .then(
                    left.requirement
                        .arena_index()
                        .cmp(&right.requirement.arena_index()),
                )
                .then(left.plan_identity.cmp(&right.plan_identity))
        });
        receipts.dedup();
        build_time_evaluation::ConstantValueClosureRequest {
            retained,
            checked,
            operators: SelectedBuildTimeOperators {
                operators: &self.operators,
                provider_bodies: &self.provider_bodies,
            },
            target_bodies: &self.target_bodies,
            selection: &self.selection,
            layouts: provider_planning::data_layout_queries::TargetLayoutQueries {
                target: self.target_profile,
                opaque_representations: &self.opaque_representation_selections,
            },
            receipts,
            target_scope: self.target_profile.target_name(),
        }
        .close()
    }

    /// Join provider authority once for every retained source use. Psi prepares
    /// semantic operands and independently admits both intrinsic and checked-body
    /// execution. Its coordinator supplies the operator selections; realization
    /// does not enter a source stage to reconstruct them. `target_profile` and
    /// `opaque_representation_selections` are the exact selection the deferred
    /// data-layout queries are answered under.
    pub fn prepare(
        context: provider_planning::ProviderPlanningContext<'_>,
        target_profile: target::TargetProfile,
        opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
        facts: &checked_trees::CheckedOperatorFacts,
        plans: &SelectedProviderPlanFacts,
    ) -> Result<Self, Vec<Diagnostic>> {
        let typed = context.typed;
        let uses = facts.resolved_expression_selections();
        let mut selected = Self {
            operators: Vec::new(),
            provider_bodies: Vec::new(),
            target_bodies: provider_planning::target_machines::selected_constant_target_bodies(
                context,
                target_profile,
            )?,
            target_profile,
            selection: typed_trees::references::DeclarationSelection::from_program(typed),
            opaque_representation_selections: opaque_representation_selections.to_vec(),
        };
        for (expression, origin, requirement, policy) in uses {
            let Some((_, plan)) =
                provider_planning::selected_requirement_plan(context, plans.plans(), requirement)
            else {
                continue;
            };
            // Select this exact method, not the binding kind of a sibling
            // row in the same boundary-trait plan.
            if let Some((provider, declaration)) =
                provider_planning::selected_requirement_checked_adapter(context, plan, requirement)
                    .map_err(|error| vec![error])?
            {
                selected.provider_bodies.push(
                    SelectedBuildTimeProviderBody::prepare(
                        typed,
                        expression,
                        origin,
                        requirement,
                        provider,
                        plan.provider_type.clone(),
                        declaration,
                        plan.provider_type_package_identity,
                        target_profile.target_name(),
                        CheckedProviderPlanCommitment::from_digest(
                            *plan.identity_digest().as_bytes(),
                        ),
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                );
            } else if crate::derive_selected_primitive_float_binary_execution(
                typed,
                plan,
                requirement,
            )
            .map_err(|error| vec![error])?
            .is_some()
            {
                selected.operators.push(
                    SelectedBuildTimeBinaryOperator::prepare(
                        typed,
                        expression,
                        origin,
                        requirement,
                        policy,
                        CheckedProviderPlanCommitment::from_digest(
                            *plan.identity_digest().as_bytes(),
                        ),
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                );
            }
        }
        build_time_evaluation::validate_selected_operators(typed, &selected.operators)
            .and_then(|()| {
                build_time_evaluation::validate_selected_provider_bodies(
                    typed,
                    &selected.provider_bodies,
                )
            })
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok(selected)
    }
}

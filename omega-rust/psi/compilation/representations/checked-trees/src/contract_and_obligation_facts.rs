/*
We represent constant-predicate, constant-recipe, and integer-range obligations
alongside machine contract, crash-route, and resource-envelope plans
(`contract_plans`) in `CheckedTrees`.
*/

pub(crate) mod constant_predicate_obligations {
    use crate::facts::scalar_predicates::{self, PredicateNode, ScalarValue};
    use arena::{Arena, Handle};
    use diagnostics::Diagnostic;
    use semantic_vocabulary::ScalarConditionRole;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, UnaryOperator};

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantPredicateObligation {
        nodes: Arena<CapturedPredicateNode>,
        root: Handle<CapturedPredicateNode>,
        source_span: source::SourceSpan,
        description: String,
        caller: symbols::SymbolHandle,
        role: ScalarConditionRole,
        discharged_target: Option<typed_trees::name::Identifier>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    struct CapturedPredicateNode(PredicateNode<Handle<CapturedPredicateNode>, ScalarObservation>);

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    struct ScalarObservation {
        expression: ExpressionHandle,
        owner: symbols::SymbolHandle,
    }

    impl ScalarObservation {
        fn complete_integer_entry(self, program: &TypedTrees) -> Option<bool> {
            let entry = program
                .constant_evaluation_for_expression(self.expression, self.owner)
                .ok()?;
            Some(entry.is_some_and(|entry| {
                program
                    .primitive_type_reference(entry.result_type)
                    .is_some_and(|primitive| primitive.accepts_integer_literal())
            }))
        }
    }

    impl ConstantPredicateObligation {
        pub fn retain(
            program: &TypedTrees,
            expression: ExpressionHandle,
            owner: symbols::SymbolHandle,
            caller: symbols::SymbolHandle,
            role: ScalarConditionRole,
            description: String,
            builtin: &impl Fn(ExpressionHandle) -> bool,
        ) -> Option<Self> {
            let mut pending = vec![ScalarObservation { expression, owner }];
            let mut visited = Vec::new();
            let mut needs_closure = false;
            while let Some(observation) = pending.pop() {
                if visited.contains(&observation) {
                    continue;
                }
                let current = observation.expression;
                if !program.expression_table.expression_is_valid(current) {
                    return None;
                }
                visited.push(observation);
                if observation.complete_integer_entry(program)? {
                    needs_closure = true;
                    continue;
                }
                match program.expression_table.expression(current) {
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                    ExpressionNode::Call(call) if call.layout_query.is_some() => {
                        needs_closure = true;
                    }
                    ExpressionNode::Name(path) => {
                        let declaration = program.const_declaration_by_symbol(path.symbol)?;
                        pending.push(ScalarObservation {
                            expression: declaration.materialized_initializer,
                            owner: declaration.symbol,
                        });
                    }
                    ExpressionNode::Binary(binary) => {
                        if !builtin(current) {
                            return None;
                        }
                        pending.extend([binary.left, binary.right].map(|expression| {
                            ScalarObservation {
                                expression,
                                owner: observation.owner,
                            }
                        }));
                    }
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        pending.push(ScalarObservation {
                            expression: unary.operand,
                            ..observation
                        });
                    }
                    ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
                        pending.push(ScalarObservation {
                            expression: cast.value,
                            ..observation
                        });
                    }
                    _ => return None,
                }
            }
            if !needs_closure {
                return None;
            }
            let (nodes, root) =
                capture_predicate(program, ScalarObservation { expression, owner })?;
            Some(Self {
                nodes,
                root,
                source_span: program.expression_table.source_span(expression),
                description,
                caller,
                role,
                discharged_target: None,
            })
        }

        pub fn is_pending(&self) -> bool {
            self.discharged_target.is_none()
        }

        pub fn caller(&self) -> symbols::SymbolHandle {
            self.caller
        }

        pub fn role(&self) -> ScalarConditionRole {
            self.role
        }

        pub fn description(&self) -> &str {
            &self.description
        }

        pub fn map_nodes<Reference: Copy, Leaf>(
            &self,
            observe: &mut impl FnMut(ExpressionHandle, symbols::SymbolHandle) -> Result<Leaf, String>,
            publish: &mut impl FnMut(PredicateNode<Reference, Leaf>) -> Result<Reference, String>,
        ) -> Result<Reference, String> {
            let mut coordinates = Vec::new();
            for (source, captured) in self.nodes.iter() {
                if matches!(captured.0, PredicateNode::Missing) {
                    return Err("retained predicate contains an absent node".into());
                }
                let node = captured.0.try_map(
                    &mut |child| {
                        coordinates
                            .iter()
                            .find(|(original, _)| *original == child)
                            .map(|(_, mapped)| *mapped)
                            .ok_or_else(|| "retained predicate lost a preceding child".to_owned())
                    },
                    &mut |leaf| observe(leaf.expression, leaf.owner),
                )?;
                coordinates.push((source, publish(node)?));
            }
            coordinates
                .iter()
                .find(|(source, _)| *source == self.root)
                .map(|(_, mapped)| *mapped)
                .ok_or_else(|| "retained predicate lost its root".to_owned())
        }

        pub fn discharge_all<'obligation>(
            obligations: impl IntoIterator<Item = &'obligation mut Self>,
            observe: &mut impl FnMut(ExpressionHandle, symbols::SymbolHandle) -> Option<ScalarValue>,
            target: &str,
        ) -> Result<(), Vec<Diagnostic>> {
            let obligations: Vec<_> = obligations.into_iter().collect();
            let diagnostics: Vec<_> = obligations
            .iter()
            .filter_map(|obligation| {
                let result = scalar_predicates::evaluate_nodes(
                    obligation.root,
                    &mut |reference| {
                        obligation.nodes.is_valid(reference)
                            .then(|| obligation.nodes.get(reference).0.clone())
                    },
                    &mut |leaf: ScalarObservation| observe(leaf.expression, leaf.owner),
                    &|_, _| true,
                );
                if matches!(result, Some(ScalarValue::Boolean(true))) {
                    return None;
                }
                Some(
                    Diagnostic::error(format!(
                        "{}: constant predicate {} under the selected target realization `{target}`",
                        obligation.description,
                        if matches!(result, Some(ScalarValue::Boolean(false))) {
                            "is false"
                        } else {
                            "has no closed Boolean value"
                        },
                    ))
                    .with_source_span(obligation.source_span),
                )
            })
            .collect();
            if diagnostics.is_empty() {
                for obligation in obligations {
                    obligation.discharged_target =
                        Some(typed_trees::name::Identifier::generated(target));
                }
                Ok(())
            } else {
                Err(diagnostics)
            }
        }
    }

    fn capture_predicate(
        program: &TypedTrees,
        root: ScalarObservation,
    ) -> Option<(Arena<CapturedPredicateNode>, Handle<CapturedPredicateNode>)> {
        enum Step {
            Enter(ScalarObservation),
            Capture(
                ScalarObservation,
                PredicateNode<ScalarObservation, ScalarObservation>,
            ),
        }
        let mut pending = vec![Step::Enter(root)];
        let mut active = Vec::new();
        let mut captured: Vec<(ScalarObservation, Handle<CapturedPredicateNode>)> = Vec::new();
        let mut nodes = Arena::new();
        while let Some(step) = pending.pop() {
            match step {
                Step::Enter(expression) => {
                    if captured.iter().any(|(source, _)| *source == expression) {
                        continue;
                    }
                    if active.contains(&expression) {
                        return None;
                    }
                    active.push(expression);
                    let node = if expression.complete_integer_entry(program)? {
                        PredicateNode::Leaf(expression)
                    } else if let ExpressionNode::Name(path) =
                        program.expression_table.expression(expression.expression)
                    {
                        let declaration = program.const_declaration_by_symbol(path.symbol)?;
                        PredicateNode::Forward(ScalarObservation {
                            expression: declaration.materialized_initializer,
                            owner: declaration.symbol,
                        })
                    } else {
                        scalar_predicates::source_node(program, expression.expression, &mut |_| {
                            None
                        })?
                        .try_map(
                            &mut |child| {
                                Ok::<_, ()>(ScalarObservation {
                                    expression: child,
                                    owner: expression.owner,
                                })
                            },
                            &mut |leaf| {
                                Ok::<_, ()>(ScalarObservation {
                                    expression: *leaf,
                                    owner: expression.owner,
                                })
                            },
                        )
                        .ok()?
                    };
                    let (children, count) = match &node {
                        PredicateNode::Not(child) | PredicateNode::Forward(child) => {
                            ([*child, ScalarObservation::default()], 1)
                        }
                        PredicateNode::Binary { left, right, .. } => ([*left, *right], 2),
                        _ => ([ScalarObservation::default(); 2], 0),
                    };
                    pending.push(Step::Capture(expression, node));
                    pending.extend(children.into_iter().take(count).rev().map(Step::Enter));
                }
                Step::Capture(expression, node) => {
                    let child = |expression| {
                        captured
                            .iter()
                            .find(|(source, _)| *source == expression)
                            .map(|(_, handle)| *handle)
                    };
                    if matches!(node, PredicateNode::Missing) {
                        return None;
                    }
                    let node = node
                        .try_map(&mut |reference| child(reference).ok_or(()), &mut |leaf| {
                            Ok(*leaf)
                        })
                        .ok()?;
                    captured.push((expression, nodes.insert(CapturedPredicateNode(node))));
                    if active.pop() != Some(expression) {
                        return None;
                    }
                }
            }
        }
        let root = captured.iter().find(|(source, _)| *source == root)?.1;
        Some((nodes, root))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use numerics::bignum::BigInt;
        use numerics::scalar_predicates::PredicateOperator;

        #[test]
        fn retained_predicate_publication_preserves_leaf_owner_and_caller_scope() {
            let mut nodes = Arena::new();
            let first = ExpressionHandle::from_parts(17, 1);
            let second = ExpressionHandle::from_parts(23, 1);
            let owner = symbols::SymbolHandle::from_parts(31, 1);
            let other_owner = symbols::SymbolHandle::from_parts(33, 1);
            let left = nodes.insert(CapturedPredicateNode(PredicateNode::Leaf(
                ScalarObservation {
                    expression: first,
                    owner,
                },
            )));
            let right = nodes.insert(CapturedPredicateNode(PredicateNode::Leaf(
                ScalarObservation {
                    expression: second,
                    owner: other_owner,
                },
            )));
            let root = nodes.insert(CapturedPredicateNode(PredicateNode::Binary {
                operator: PredicateOperator::Equal,
                left,
                right,
            }));
            let caller = symbols::SymbolHandle::from_parts(37, 1);
            let obligation = ConstantPredicateObligation {
                nodes,
                root,
                caller,
                role: ScalarConditionRole::Prerequisite,
                source_span: source::SourceSpan::default(),
                description: "captured equality".into(),
                discharged_target: None,
            };
            let mut mapped = Vec::new();
            let mut observations = Vec::new();
            let root = obligation
                .map_nodes(
                    &mut |expression, actual_owner| {
                        observations.push((expression, actual_owner));
                        Ok(if expression == first { 41u32 } else { 43 })
                    },
                    &mut |node| {
                        let ordinal = mapped.len();
                        mapped.push(node);
                        Ok(ordinal)
                    },
                )
                .unwrap();
            assert_eq!(obligation.caller(), caller);
            assert_eq!(obligation.role(), ScalarConditionRole::Prerequisite);
            assert_eq!(observations, vec![(first, owner), (second, other_owner)]);
            drop(obligation);
            assert_eq!(
                scalar_predicates::evaluate_nodes(
                    root,
                    &mut |reference| mapped.get(reference).cloned(),
                    &mut |leaf| Some(ScalarValue::Integer(BigInt::from_u64(leaf as u64))),
                    &|_, _| true
                ),
                Some(ScalarValue::Boolean(false))
            );
        }
    }
}

pub(crate) mod constant_recipe_obligations {
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use semantic_vocabulary::ScalarConditionRole;
    use typed_trees::TypedTrees;
    use typed_trees::constant_recipes::ConstantRecipeHandle;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum ConstantRecipeRequirement {
        ExceedsBound(BigInt),
        AtLeast(BigInt),
        AtMost(BigInt),
        EqualRecipe(ConstantRecipeHandle),
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct ConstantRecipeObligation {
        owner: symbols::SymbolHandle,
        role: ScalarConditionRole,
        recipe_owner: symbols::SymbolHandle,
        recipe: ConstantRecipeHandle,
        requirement: ConstantRecipeRequirement,
        source_span: source::SourceSpan,
        description: String,
        discharged: bool,
    }

    impl ConstantRecipeObligation {
        pub fn retain(
            owner: symbols::SymbolHandle,
            role: ScalarConditionRole,
            recipe: ConstantRecipeHandle,
            requirement: ConstantRecipeRequirement,
            source_span: source::SourceSpan,
            description: String,
        ) -> Self {
            Self {
                owner,
                role,
                recipe_owner: symbols::SymbolHandle::invalid(),
                recipe,
                requirement,
                source_span,
                description,
                discharged: false,
            }
        }

        pub fn owner(&self) -> symbols::SymbolHandle {
            self.owner
        }

        pub fn role(&self) -> ScalarConditionRole {
            self.role
        }

        pub fn with_recipe_owner(mut self, owner: symbols::SymbolHandle) -> Self {
            self.recipe_owner = owner;
            self
        }

        pub fn recipe_owner(&self) -> symbols::SymbolHandle {
            self.recipe_owner
        }
        pub fn description(&self) -> &str {
            &self.description
        }

        pub fn recipe(&self) -> ConstantRecipeHandle {
            self.recipe
        }
        pub fn requirement(&self) -> &ConstantRecipeRequirement {
            &self.requirement
        }

        pub fn is_pending(&self) -> bool {
            !self.discharged
        }

        pub fn discharge_all(
            obligations: &mut [Self],
            program: &TypedTrees,
        ) -> Result<(), Vec<Diagnostic>> {
            for obligation in obligations.iter_mut() {
                obligation.discharged = false;
            }
            let mut diagnostics = Vec::new();
            for obligation in obligations.iter() {
                let recipe = program.constant_recipes.get(obligation.recipe);
                let Some(value) = program.closed_constant_recipe_value(obligation.recipe) else {
                    diagnostics.push(
                    Diagnostic::error(format!(
                        "{}: constant recipe `{}` has no closed value under the selected target realization",
                        obligation.description,
                        recipe.name.as_str(),
                    ))
                    .with_source_span(obligation.source_span),
                );
                    continue;
                };
                if let ConstantRecipeRequirement::EqualRecipe(other) = &obligation.requirement {
                    let other_value = program.closed_constant_recipe_value(*other);
                    if other_value != Some(value) {
                        diagnostics.push(Diagnostic::error(format!(
                        "{}: contract endpoint equality requires both admitted recipe values to agree; selected values are {} and {:?}",
                        obligation.description, value, other_value,
                    )).with_source_span(obligation.source_span));
                    }
                    continue;
                }
                let (holds, relation) = match &obligation.requirement {
                    ConstantRecipeRequirement::ExceedsBound(bound) => (bound < value, "<"),
                    ConstantRecipeRequirement::AtLeast(bound) => (bound <= value, "<="),
                    ConstantRecipeRequirement::AtMost(bound) => (value <= bound, ">="),
                    ConstantRecipeRequirement::EqualRecipe(_) => {
                        unreachable!("handled both-recipe relation above")
                    }
                };
                if !holds {
                    let bound = match &obligation.requirement {
                        ConstantRecipeRequirement::ExceedsBound(bound)
                        | ConstantRecipeRequirement::AtLeast(bound)
                        | ConstantRecipeRequirement::AtMost(bound) => bound,
                        ConstantRecipeRequirement::EqualRecipe(_) => {
                            unreachable!("handled both-recipe relation above")
                        }
                    };
                    diagnostics.push(
                    Diagnostic::error(format!(
                        "{}: requires {} {} `{}`, but the selected target realization closes `{}` to {}",
                        obligation.description,
                        bound,
                        relation,
                        recipe.name.as_str(),
                        recipe.name.as_str(),
                        value,
                    ))
                    .with_source_span(obligation.source_span),
                );
                }
            }
            if diagnostics.is_empty() {
                for obligation in obligations {
                    obligation.discharged = true;
                }
                Ok(())
            } else {
                Err(diagnostics)
            }
        }
    }
}

pub(crate) mod integer_range_obligations {
    use arena::{Arena, HandleSpan};
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use numerics::integer_bounds::{self, IntegerBoundStep};
    use semantic_vocabulary::ScalarConditionRole;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    struct SourceObservation {
        expression: ExpressionHandle,
        owner: SymbolHandle,
    }

    type RangeStep = IntegerBoundStep<SourceObservation>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerRangeRecipe {
        steps: Vec<RangeStep>,
        target_dependent: bool,
    }

    impl IntegerRangeRecipe {
        pub fn bounds(low: BigInt, high: BigInt) -> Self {
            Self {
                steps: vec![RangeStep::Bounds { low, high }],
                target_dependent: false,
            }
        }

        pub fn observation(expression: ExpressionHandle, owner: SymbolHandle) -> Self {
            Self {
                steps: vec![RangeStep::Observation(SourceObservation {
                    expression,
                    owner,
                })],
                target_dependent: true,
            }
        }

        pub fn is_target_dependent(&self) -> bool {
            self.target_dependent
        }

        pub fn ground_bounds(&self) -> Option<(&BigInt, &BigInt)> {
            match self.steps.as_slice() {
                [RangeStep::Bounds { low, high }] if !self.target_dependent => Some((low, high)),
                _ => None,
            }
        }

        pub fn binary(&self, operator: BinaryOperator, right: &Self) -> Option<Self> {
            let step = match operator {
                BinaryOperator::Add => RangeStep::Add,
                BinaryOperator::Subtract => RangeStep::Subtract,
                BinaryOperator::Multiply => RangeStep::Multiply,
                _ => return None,
            };
            Some(self.compose(right, step))
        }

        pub fn interval(lower: &Self, upper: &Self) -> Self {
            lower.compose(upper, RangeStep::Interval)
        }

        pub fn intersect(&self, right: &Self) -> Self {
            self.compose(right, RangeStep::Intersect)
        }

        pub fn union(&self, right: &Self) -> Self {
            if self == right {
                return self.clone();
            }
            self.compose(right, RangeStep::Union)
        }

        fn compose(&self, right: &Self, step: RangeStep) -> Self {
            let mut steps = Vec::with_capacity(self.steps.len() + right.steps.len() + 1);
            steps.extend_from_slice(&self.steps);
            steps.extend_from_slice(&right.steps);
            steps.push(step);
            let target_dependent = self.target_dependent || right.target_dependent;
            if !target_dependent
                && let Ok((low, high)) = integer_bounds::evaluate(&steps, &mut |_| {
                    Err("ground integer bounds contain an observation".to_owned())
                })
            {
                return Self::bounds(low, high);
            }
            Self {
                steps,
                target_dependent,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    enum RangeRequirement {
        Fixed { minimum: BigInt, maximum: BigInt },
        AddressModel,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct IntegerRangeModel {
        pub carrier: semantic_vocabulary::IntegerType,
        pub semantics_version: u32,
        pub commitment: [u8; 32],
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct IntegerRangeObligation {
        expression: ExpressionHandle,
        owner: SymbolHandle,
        role: ScalarConditionRole,
        steps: Arena<RangeStep>,
        recipe: HandleSpan<RangeStep>,
        requirement: RangeRequirement,
        description: String,
        discharged_address_model: Option<IntegerRangeModel>,
        discharged_target: Option<typed_trees::name::Identifier>,
    }

    impl IntegerRangeObligation {
        pub fn retain(
            expression: ExpressionHandle,
            owner: SymbolHandle,
            role: ScalarConditionRole,
            recipe: IntegerRangeRecipe,
            minimum: BigInt,
            maximum: BigInt,
            description: String,
        ) -> Self {
            Self::with_requirement(
                expression,
                owner,
                role,
                recipe,
                RangeRequirement::Fixed { minimum, maximum },
                description,
            )
        }

        fn with_requirement(
            expression: ExpressionHandle,
            owner: SymbolHandle,
            role: ScalarConditionRole,
            recipe: IntegerRangeRecipe,
            requirement: RangeRequirement,
            description: String,
        ) -> Self {
            let mut steps = Arena::new();
            let recipe = steps.insert_many(recipe.steps);
            Self {
                expression,
                owner,
                role,
                steps,
                recipe,
                requirement,
                description,
                discharged_address_model: None,
                discharged_target: None,
            }
        }

        pub fn retain_address_fit(
            expression: ExpressionHandle,
            owner: SymbolHandle,
            role: ScalarConditionRole,
            low: BigInt,
            high: BigInt,
            description: String,
        ) -> Self {
            Self::with_requirement(
                expression,
                owner,
                role,
                IntegerRangeRecipe::bounds(low, high),
                RangeRequirement::AddressModel,
                description,
            )
        }

        pub fn needs_address_model(&self) -> bool {
            matches!(self.requirement, RangeRequirement::AddressModel)
        }

        pub fn discharged_address_model(&self) -> Option<&IntegerRangeModel> {
            self.discharged_address_model.as_ref()
        }

        pub fn owner(&self) -> SymbolHandle {
            self.owner
        }
        pub fn role(&self) -> ScalarConditionRole {
            self.role
        }
        pub fn expression(&self) -> ExpressionHandle {
            self.expression
        }
        pub fn is_pending(&self) -> bool {
            self.discharged_target.is_none()
        }

        pub fn map_steps<Observation>(
            &self,
            observe: &mut impl FnMut(ExpressionHandle, SymbolHandle) -> Result<Observation, String>,
        ) -> Result<Vec<IntegerBoundStep<Observation>>, String> {
            self.steps
                .span(self.recipe)
                .ok_or_else(|| "retained integer range has no recipe storage".to_owned())?
                .iter()
                .map(|step| {
                    step.map_observation(&mut |observation| {
                        observe(observation.expression, observation.owner)
                    })
                })
                .collect()
        }

        pub fn fixed_limits(&self) -> Option<(&BigInt, &BigInt)> {
            match &self.requirement {
                RangeRequirement::Fixed { minimum, maximum } => Some((minimum, maximum)),
                RangeRequirement::AddressModel => None,
            }
        }

        pub fn discharge(
            &mut self,
            program: &TypedTrees,
            target: &str,
            address_model: Option<&IntegerRangeModel>,
        ) -> Result<(), Diagnostic> {
            let limits = match &self.requirement {
                RangeRequirement::Fixed { minimum, maximum } => {
                    Some((minimum.clone(), maximum.clone()))
                }
                RangeRequirement::AddressModel => address_model
                    .filter(|model| model.carrier.is_address())
                    .map(|model| {
                        (
                            BigInt::zero(),
                            BigInt::from_u64(1)
                                .shl_bits(usize::from(model.carrier.bits()))
                                .sub(&BigInt::from_u64(1)),
                        )
                    }),
            };
            let result = self
                .steps
                .span(self.recipe)
                .ok_or_else(|| "retained integer range has no recipe storage".to_owned())
                .and_then(|steps| {
                    integer_bounds::evaluate(steps, &mut |observation| {
                        program
                            .closed_integer_expression_value_in(
                                observation.expression,
                                observation.owner,
                            )
                            .ok_or_else(|| {
                                "target observation has no closed integer value".to_owned()
                            })
                    })
                })
                .and_then(|(low, high)| {
                    let (minimum, maximum) = limits
                        .as_ref()
                        .ok_or_else(|| "selected address model is not closed".to_owned())?;
                    if minimum <= &low && &high <= maximum {
                        Ok(())
                    } else {
                        Err(format!(
                            "bounds [{low}, {high}] lie outside [{minimum}, {maximum}]"
                        ))
                    }
                });
            match result {
                Ok(()) => {
                    self.discharged_target = Some(typed_trees::name::Identifier::generated(target));
                    if self.needs_address_model() {
                        self.discharged_address_model = address_model.cloned();
                    }
                    Ok(())
                }
                Err(reason) => Err(Diagnostic::error(format!(
                    "{}: exact arithmetic may overflow under target `{target}`: {reason}",
                    self.description,
                ))
                .with_source_span(program.expression_table.source_span(self.expression))),
            }
        }
    }

    #[cfg(test)]
    mod recipe_tests {
        use super::*;

        fn bounds(low: i64, high: i64) -> IntegerRangeRecipe {
            IntegerRangeRecipe::bounds(BigInt::from_i64(low), BigInt::from_i64(high))
        }

        #[test]
        fn ground_composition_retains_evaluated_interval_normal_form() {
            let product = bounds(-3, 2)
                .binary(BinaryOperator::Multiply, &bounds(-4, 5))
                .expect("builtin product");
            assert_eq!(product, bounds(-15, 12));
            assert_eq!(product.intersect(&bounds(-10, 11)), bounds(-10, 11));
            assert_eq!(product.union(&bounds(20, 21)), bounds(-15, 21));
            assert_eq!(
                IntegerRangeRecipe::interval(&bounds(-3, 2), &bounds(4, 5)),
                bounds(-3, 5)
            );
            assert_eq!(
                bounds(2, 4).binary(BinaryOperator::Subtract, &bounds(3, 5)),
                Some(bounds(-3, 1))
            );
            let mut joined = bounds(0, 7);
            for _ in 0..100 {
                joined = joined.intersect(&bounds(-10, 20)).union(&bounds(1, 6));
            }
            assert_eq!(joined, bounds(0, 7));
        }

        #[test]
        fn refused_ground_children_survive_later_composition() {
            for refused in [
                bounds(2, 1).union(&bounds(0, 3)),
                bounds(0, 1).intersect(&bounds(2, 3)),
                IntegerRangeRecipe::interval(&bounds(5, 6), &bounds(1, 2)),
            ] {
                let original = integer_bounds::evaluate(&refused.steps, &mut |_| {
                    Err("unexpected observation".to_owned())
                });
                assert!(original.is_err());
                let cancelled = refused
                    .binary(BinaryOperator::Multiply, &bounds(0, 0))
                    .expect("builtin product")
                    .union(&bounds(0, 0));
                assert_eq!(
                    integer_bounds::evaluate(&cancelled.steps, &mut |_| {
                        Err("unexpected observation".to_owned())
                    }),
                    original
                );
            }
        }

        #[test]
        fn observations_keep_their_exact_deferred_recipe() {
            let observed = IntegerRangeRecipe::observation(
                ExpressionHandle::from_arena_index(42),
                SymbolHandle::from_arena_index(9),
            );
            let combined = observed.union(&bounds(7, 7)).intersect(&bounds(0, 10));
            assert!(combined.is_target_dependent());
            assert_eq!(combined.steps.len(), 5);
            let mut seen = Vec::new();
            assert_eq!(
                integer_bounds::evaluate(&combined.steps, &mut |observation| {
                    seen.push((observation.expression, observation.owner));
                    Ok(BigInt::from_i64(8))
                }),
                Ok((BigInt::from_i64(7), BigInt::from_i64(8)))
            );
            assert_eq!(
                seen,
                vec![(
                    ExpressionHandle::from_arena_index(42),
                    SymbolHandle::from_arena_index(9)
                )]
            );
            assert!(
                integer_bounds::evaluate(&combined.steps, &mut |_| {
                    Err("missing captured observation".to_owned())
                })
                .is_err()
            );
        }
    }
}

pub(crate) mod contract_plans {
    macro_rules! contract_identity_accessors {
        ($machine:ident, $entry:ident, $fingerprint:ident, $commitment:ident) => {
            pub const fn $machine(&self) -> SymbolHandle {
                self.$machine
            }

            pub const fn $entry(&self) -> SymbolHandle {
                self.$entry
            }

            pub const fn $fingerprint(&self) -> u64 {
                self.$fingerprint
            }

            pub const fn $commitment(&self) -> MachineContractCommitment {
                self.$commitment
            }
        };
    }
    pub(crate) use contract_identity_accessors;

    pub(crate) mod contract_identity {
        use super::{CrashInterface, MachineContractCommitment, MachineContractIdentity};
        use language_semantics::{
            BlockingInterface, MachineSupplyMode, SuspensionInterface,
            SynchronousInvocationInterface,
        };
        use std::hash::{Hash, Hasher};
        use typed_trees::type_identity::{NormalizedTypeIdentity, TypeIdentityTable};

        pub use typed_trees::contract_structure::{
            DeclaredContractFact, DeclaredContractParameter,
        };
        pub type DeclaredContractKind =
            typed_trees::contract_structure::DeclaredContractKind<super::CrashCause>;
        pub type DeclaredContractClause =
            typed_trees::contract_structure::DeclaredContractClause<super::CrashCause>;
        pub type DeclaredCallableContract =
            typed_trees::contract_structure::DeclaredCallableContract<super::CrashCause>;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct DeclaredContractStructure {
            pub callables: Vec<DeclaredCallableContract>,
            pub clauses: Vec<DeclaredContractClause>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct DeclaredProgressPremise {
            pub profile: String,
            pub subject: String,
            pub projections: Vec<String>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum DeclaredTermination {
            InternalDerived,
            NoGuarantee,
            Terminates(Vec<DeclaredProgressPremise>),
        }

        pub fn contract_declarations_commitment(
            clauses: &[DeclaredContractClause],
            type_identities: &TypeIdentityTable,
        ) -> MachineContractCommitment {
            let mut identity = ContractHasher::new(b"omega.declared-contract-structure.v2\0");
            canonical_clauses(clauses, type_identities).hash(&mut identity);
            identity.complete().commitment
        }

        pub fn declared_termination_commitment(
            termination: &DeclaredTermination,
        ) -> MachineContractCommitment {
            let mut identity = ContractHasher::new(b"omega.declared-termination.v1\0");
            termination.hash(&mut identity);
            identity.complete().commitment
        }

        pub fn contract_identity(
            supply_mode: MachineSupplyMode,
            external_binding: Option<&language_semantics::ExternalBindingIdentity>,
            published_service_names: &[String],
            invocation_interface: SynchronousInvocationInterface,
            published_invocations: &[String],
            suspension_interface: SuspensionInterface,
            blocking_interface: BlockingInterface,
            crash_interface: CrashInterface,
            termination: &DeclaredTermination,
            declaration: &DeclaredContractStructure,
            type_identities: &TypeIdentityTable,
        ) -> MachineContractIdentity {
            let mut identity = ContractHasher::new(b"omega.checked-machine-public-contract.v3\0");
            let supply = match supply_mode {
                MachineSupplyMode::CheckedBody => 1_u8,
                MachineSupplyMode::Requirement => 2,
                MachineSupplyMode::Boundary => 3,
                MachineSupplyMode::AdmissionClaim => 4,
                MachineSupplyMode::ExternalRealization { .. } => 5,
                MachineSupplyMode::TopLevelRequirement => 6,
            };
            supply.hash(&mut identity);
            if let MachineSupplyMode::ExternalRealization { mechanism, .. } = supply_mode {
                (
                    external_binding,
                    mechanism.map(|mechanism| mechanism.identity_tag()),
                )
                    .hash(&mut identity);
            }
            let mut services = published_service_names.iter().collect::<Vec<_>>();
            services.sort_unstable();
            services.dedup();
            services.hash(&mut identity);
            matches!(
                invocation_interface,
                SynchronousInvocationInterface::PublishedCeiling
            )
            .hash(&mut identity);
            let mut invocations = published_invocations.iter().collect::<Vec<_>>();
            invocations.sort_unstable();
            invocations.dedup();
            invocations.hash(&mut identity);
            match suspension_interface {
                SuspensionInterface::InternalInferred => None,
                SuspensionInterface::PublishedMaySuspend(value) => Some(value),
            }
            .hash(&mut identity);
            match blocking_interface {
                BlockingInterface::InternalInferred => None,
                BlockingInterface::PublishedMayBlock(value) => Some(value),
            }
            .hash(&mut identity);
            matches!(crash_interface, CrashInterface::PublishedCeiling).hash(&mut identity);
            termination.hash(&mut identity);
            CanonicalDeclaration {
                callables: declaration
                    .callables
                    .iter()
                    .map(|callable| CanonicalCallable {
                        parameters: callable
                            .parameters
                            .iter()
                            .map(|parameter| CanonicalParameter {
                                mutable: parameter.mutable,
                                constant: parameter.constant,
                                relevance: parameter.relevance,
                                type_key: type_key(type_identities, parameter.type_identity),
                            })
                            .collect(),
                        result: type_key(type_identities, callable.result),
                        clauses: canonical_clauses(&callable.clauses, type_identities),
                    })
                    .collect(),
                clauses: canonical_clauses(&declaration.clauses, type_identities),
            }
            .hash(&mut identity);
            identity.complete()
        }

        #[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
        enum CanonicalFact<'a> {
            Expression(&'a str),
            Membership {
                value: &'a str,
                domain: &'a str,
                arguments: Vec<&'a str>,
            },
            Proposition(&'a str),
            ContentConservation(&'a [u8]),
        }

        #[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
        struct CanonicalClause<'a> {
            kind: &'a DeclaredContractKind,
            fact: Option<CanonicalFact<'a>>,
        }

        #[derive(Hash)]
        struct CanonicalParameter<'a> {
            mutable: bool,
            constant: bool,
            relevance: language_core::BindingRelevance,
            type_key: &'a str,
        }

        #[derive(Hash)]
        struct CanonicalCallable<'a> {
            parameters: Vec<CanonicalParameter<'a>>,
            result: &'a str,
            clauses: Vec<CanonicalClause<'a>>,
        }

        #[derive(Hash)]
        struct CanonicalDeclaration<'a> {
            callables: Vec<CanonicalCallable<'a>>,
            clauses: Vec<CanonicalClause<'a>>,
        }

        fn type_key(table: &TypeIdentityTable, identity: NormalizedTypeIdentity) -> &str {
            table
                .text(identity)
                .expect("checked contract type must retain its owning identity")
        }

        fn canonical_clauses<'a>(
            clauses: &'a [DeclaredContractClause],
            table: &'a TypeIdentityTable,
        ) -> Vec<CanonicalClause<'a>> {
            let mut projected = clauses
                .iter()
                .map(|clause| CanonicalClause {
                    kind: &clause.kind,
                    fact: clause.fact.as_ref().map(|fact| canonical_fact(fact, table)),
                })
                .collect::<Vec<_>>();
            projected.sort();
            projected.dedup();
            projected
        }

        fn canonical_fact<'a>(
            fact: &'a DeclaredContractFact,
            table: &'a TypeIdentityTable,
        ) -> CanonicalFact<'a> {
            match fact {
                DeclaredContractFact::Expression(value) => CanonicalFact::Expression(value),
                DeclaredContractFact::Membership {
                    value,
                    domain,
                    arguments,
                } => CanonicalFact::Membership {
                    value,
                    domain,
                    arguments: arguments
                        .iter()
                        .map(|argument| type_key(table, *argument))
                        .collect(),
                },
                DeclaredContractFact::Proposition(value) => CanonicalFact::Proposition(value),
                DeclaredContractFact::ContentConservation(value) => {
                    CanonicalFact::ContentConservation(value)
                }
            }
        }

        pub(crate) fn hash_declared_fact(
            fact: &DeclaredContractFact,
            table: &TypeIdentityTable,
            identity: &mut ContractHasher,
        ) {
            canonical_fact(fact, table).hash(identity);
        }

        pub(crate) struct ContractHasher {
            report: semantic_vocabulary::Fnv1a,
            commitment: crate::digest_encoder::DomainDigestEncoder,
        }

        impl ContractHasher {
            pub(crate) fn new(domain: &[u8]) -> Self {
                let mut identity = Self {
                    report: semantic_vocabulary::Fnv1a::new(),
                    commitment: crate::digest_encoder::DomainDigestEncoder::new(b""),
                };
                identity.write(domain);
                identity
            }

            pub(crate) fn complete(self) -> MachineContractIdentity {
                MachineContractIdentity {
                    report_fingerprint: self.report.finish(),
                    commitment: MachineContractCommitment::from_digest(self.commitment.finish()),
                }
            }
        }

        macro_rules! integer_writes {
    ($($method:ident: $integer:ty),* $(,)?) => {
        $(fn $method(&mut self, value: $integer) { self.write(&value.to_le_bytes()); })*
    };
}

        impl Hasher for ContractHasher {
            fn finish(&self) -> u64 {
                self.report.finish()
            }

            fn write(&mut self, bytes: &[u8]) {
                self.commitment.raw(bytes);
                self.report.bytes(bytes);
            }

            integer_writes!(write_u8: u8, write_u16: u16, write_u32: u32, write_u64: u64,
        write_u128: u128, write_i8: i8, write_i16: i16, write_i32: i32,
        write_i64: i64, write_i128: i128);

            fn write_usize(&mut self, value: usize) {
                self.write_u64(value as u64);
            }
            fn write_isize(&mut self, value: isize) {
                self.write_i64(value as i64);
            }
        }

        #[cfg(test)]
        mod binding_projection_tests {
            use super::*;
            use language_semantics::{
                ExternalBindingIdentity, ExternalBindingMechanism, ExternalBindingTable,
            };

            #[test]
            fn membership_clause_hash_uses_content_order_after_minting() {
                let clauses = |table: &TypeIdentityTable, keys: &[&str]| {
                    keys.iter()
                        .map(|key| DeclaredContractClause {
                            kind: DeclaredContractKind::Requires,
                            fact: Some(DeclaredContractFact::Membership {
                                value: "$V0".into(),
                                domain: "AcrossSuspend".into(),
                                arguments: vec![table.intern(key)],
                            }),
                        })
                        .collect::<Vec<_>>()
                };
                let first = TypeIdentityTable::default();
                let mut left = clauses(&first, &["Alpha", "Beta"]);
                left.sort();
                let reordered = TypeIdentityTable::default();
                let mut right = clauses(&reordered, &["Beta", "Alpha"]);
                right.sort();
                assert_ne!(
                    first.text(
                        left[0]
                            .fact
                            .as_ref()
                            .and_then(|fact| match fact {
                                DeclaredContractFact::Membership { arguments, .. } =>
                                    Some(arguments[0]),
                                _ => None,
                            })
                            .unwrap()
                    ),
                    reordered.text(
                        right[0]
                            .fact
                            .as_ref()
                            .and_then(|fact| match fact {
                                DeclaredContractFact::Membership { arguments, .. } =>
                                    Some(arguments[0]),
                                _ => None,
                            })
                            .unwrap()
                    )
                );
                assert_eq!(
                    contract_declarations_commitment(&left, &first),
                    contract_declarations_commitment(&right, &reordered)
                );
                right.pop();
                assert_ne!(
                    contract_declarations_commitment(&left, &first),
                    contract_declarations_commitment(&right, &reordered)
                );
            }

            #[test]
            fn declared_crash_fact_hash_uses_owned_type_content() {
                let first = TypeIdentityTable::default();
                let alpha = first.intern("Alpha");
                let reordered = TypeIdentityTable::default();
                reordered.intern("Beta");
                let same = reordered.intern("Alpha");
                let different = TypeIdentityTable::default();
                let beta = different.intern("Beta");
                assert_ne!(alpha, same);
                assert_eq!(alpha, beta);
                let fact = |argument| DeclaredContractFact::Membership {
                    value: "$V0".into(),
                    domain: "AcrossSuspend".into(),
                    arguments: vec![argument],
                };
                assert_eq!(
                    crate::CrashPredicateIdentity::from_declared_fact(&fact(alpha), &first),
                    crate::CrashPredicateIdentity::from_declared_fact(&fact(same), &reordered)
                );
                assert_ne!(
                    crate::CrashPredicateIdentity::from_declared_fact(&fact(alpha), &first),
                    crate::CrashPredicateIdentity::from_declared_fact(&fact(beta), &different)
                );
            }

            #[test]
            fn external_contract_hash_uses_owned_binding_content_across_mint_orders() {
                let mut first = ExternalBindingTable::default();
                let binding = first.intern(ExternalBindingIdentity::Syscall { number: 39 });
                let mut reordered = ExternalBindingTable::default();
                reordered.intern(ExternalBindingIdentity::Syscall { number: 60 });
                let same = reordered.intern(ExternalBindingIdentity::Syscall { number: 39 });
                let mut different = ExternalBindingTable::default();
                let other = different.intern(ExternalBindingIdentity::Syscall { number: 60 });
                assert_ne!(binding, same);
                assert_eq!(binding, other);
                let commitment = |table: &ExternalBindingTable, binding| {
                    contract_identity(
                        MachineSupplyMode::ExternalRealization {
                            binding: Some(binding),
                            mechanism: Some(ExternalBindingMechanism::Syscall),
                        },
                        table.identity(binding),
                        &[],
                        SynchronousInvocationInterface::PublishedCeiling,
                        &[],
                        SuspensionInterface::PublishedMaySuspend(false),
                        BlockingInterface::PublishedMayBlock(false),
                        CrashInterface::PublishedCeiling,
                        &DeclaredTermination::NoGuarantee,
                        &DeclaredContractStructure {
                            callables: Vec::new(),
                            clauses: Vec::new(),
                        },
                        &TypeIdentityTable::default(),
                    )
                    .commitment
                };
                assert_eq!(commitment(&first, binding), commitment(&reordered, same));
                assert_ne!(commitment(&first, binding), commitment(&different, other));
            }
        }
    }
    pub(crate) mod crash_plans {
        use crate::CheckedBooleanExpression::Not;
        use crate::CheckedScalarExpression::Boolean;
        use crate::CheckedScalarExpression::IntegerExactCast;
        use crate::CheckedScalarExpression::StructuralSequenceLength;
        use crate::facts::checked_crash_operator_site::CheckedCrashOperatorSite;
        use crate::facts::contract_plans::{
            CheckedCrashCallSite, CheckedCrashSite, CrashCause, CrashInterface, CrashRouteBucketId,
            CrashRouteGuard, CrashSiteLocation,
        };
        use language_semantics::ScopedPermissionClaimIdentity::Unknown;
        use std::hash::Hash;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
        pub struct CrashRouteBucket {
            pub(crate) cause: CrashCause,
            pub(crate) alternative_guards: Vec<CrashRouteGuard>,
        }

        impl CrashRouteBucket {
            pub fn new(
                cause: CrashCause,
                mut alternative_guards: Vec<CrashRouteGuard>,
            ) -> Option<Self> {
                if alternative_guards.contains(&CrashRouteGuard::Truth) {
                    alternative_guards = vec![CrashRouteGuard::Truth];
                } else {
                    alternative_guards.sort();
                    alternative_guards.dedup();
                }
                (!alternative_guards.is_empty()).then_some(Self {
                    cause,
                    alternative_guards,
                })
            }

            pub fn unconditional(cause: CrashCause) -> Self {
                Self::new(cause, vec![CrashRouteGuard::Truth])
                    .expect("the unconditional crash bucket has one canonical guard")
            }

            pub fn cause(&self) -> CrashCause {
                self.cause
            }

            pub fn alternative_guards(&self) -> &[CrashRouteGuard] {
                &self.alternative_guards
            }

            pub fn is_unconditional(&self) -> bool {
                self.alternative_guards == [CrashRouteGuard::Truth]
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedBodyCrashSummary {
            machine: SymbolHandle,
            body: arena::HandleSpan<typed_trees::state::State>,
            buckets: Vec<CrashRouteBucket>,
        }

        impl CheckedBodyCrashSummary {
            pub fn new(
                machine: SymbolHandle,
                body: arena::HandleSpan<typed_trees::state::State>,
                mut buckets: Vec<CrashRouteBucket>,
            ) -> Self {
                buckets.sort();
                buckets.dedup();
                Self {
                    machine,
                    body,
                    buckets,
                }
            }

            pub const fn machine(&self) -> SymbolHandle {
                self.machine
            }

            pub const fn body(&self) -> arena::HandleSpan<typed_trees::state::State> {
                self.body
            }

            pub fn buckets(&self) -> &[CrashRouteBucket] {
                &self.buckets
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedRuntimeMembershipSubject {
            pub fact_ordinal: u32,
            pub membership: crate::domain::ProofMembershipFact,
            pub subject: crate::CheckedScalarExpression,
            pub predicate_count: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedTrappingSite {
            pub location: CrashSiteLocation,
            pub may_trap: bool,
        }

        impl CheckedTrappingSite {
            pub fn new(location: CrashSiteLocation, may_trap: bool) -> Self {
                Self { location, may_trap }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct CrashPlan {
            pub(crate) interface: CrashInterface,
            pub(crate) published: Vec<CrashRouteBucket>,
            checked_body_summary: Option<CheckedBodyCrashSummary>,
            structural_runtime_requirements: Option<Vec<crate::CheckedBooleanExpression>>,
            runtime_membership_subjects: Vec<CheckedRuntimeMembershipSubject>,
            structural_runtime_guarantees: Option<Vec<crate::CheckedBooleanExpression>>,
            checked_sites: Vec<CheckedCrashSite>,
            checked_calls: Vec<CheckedCrashCallSite>,
            checked_operators: Vec<CheckedCrashOperatorSite>,
            trapping_sites: Vec<CheckedTrappingSite>,
        }

        impl CrashPlan {
            pub fn published_ceiling(mut published: Vec<CrashRouteBucket>) -> Self {
                published.sort();
                published.dedup();
                Self {
                    interface: CrashInterface::PublishedCeiling,
                    published,
                    checked_body_summary: None,
                    structural_runtime_requirements: None,
                    runtime_membership_subjects: Vec::new(),
                    structural_runtime_guarantees: None,
                    checked_sites: Vec::new(),
                    checked_calls: Vec::new(),
                    checked_operators: Vec::new(),
                    trapping_sites: Vec::new(),
                }
            }

            pub fn with_published_ceiling(mut self, mut published: Vec<CrashRouteBucket>) -> Self {
                published.sort();
                published.dedup();
                self.interface = CrashInterface::PublishedCeiling;
                self.published = published;
                self
            }

            pub fn with_checked_body_summary(
                mut self,
                summary: Option<CheckedBodyCrashSummary>,
            ) -> Self {
                self.checked_body_summary = summary;
                self
            }

            pub fn checked_body_summary(
                &self,
                machine: SymbolHandle,
                body: arena::HandleSpan<typed_trees::state::State>,
            ) -> Option<&CheckedBodyCrashSummary> {
                self.checked_body_summary
                    .as_ref()
                    .filter(|summary| summary.machine == machine && summary.body == body)
            }

            pub fn with_trapping_sites(mut self, mut sites: Vec<CheckedTrappingSite>) -> Self {
                sites.sort_by_key(|site| {
                    (
                        site.location.state.arena_index(),
                        site.location.state.generation(),
                        site.location.statement_ordinal,
                    )
                });
                sites.dedup_by(|next, previous| {
                    if next.location == previous.location {
                        previous.may_trap |= next.may_trap;
                        true
                    } else {
                        false
                    }
                });
                self.trapping_sites = sites;
                self
            }

            pub fn trapping_sites(&self) -> &[CheckedTrappingSite] {
                &self.trapping_sites
            }

            pub fn has_possible_trapping_site(&self) -> bool {
                self.trapping_sites.iter().any(|site| site.may_trap)
            }

            pub fn with_structural_runtime_requirements(
                mut self,
                requirements: Option<Vec<crate::CheckedBooleanExpression>>,
            ) -> Self {
                self.structural_runtime_requirements = requirements;
                self
            }

            pub fn with_runtime_membership_subjects(
                mut self,
                subjects: Vec<CheckedRuntimeMembershipSubject>,
            ) -> Self {
                self.runtime_membership_subjects = subjects;
                self
            }

            pub fn runtime_membership_subjects(&self) -> &[CheckedRuntimeMembershipSubject] {
                &self.runtime_membership_subjects
            }

            pub fn structural_runtime_requirements(
                &self,
            ) -> Option<&[crate::CheckedBooleanExpression]> {
                self.structural_runtime_requirements.as_deref()
            }

            pub fn with_structural_runtime_guarantees(
                mut self,
                guarantees: Option<Vec<crate::CheckedBooleanExpression>>,
            ) -> Self {
                self.structural_runtime_guarantees = guarantees;
                self
            }

            pub fn structural_runtime_guarantees(
                &self,
            ) -> Option<&[crate::CheckedBooleanExpression]> {
                self.structural_runtime_guarantees.as_deref()
            }

            pub fn uses_structural_proof_gated_arithmetic(&self) -> bool {
                fn scalar_uses_proof_gated_arithmetic(
                    expression: &crate::CheckedScalarExpression,
                ) -> bool {
                    match expression {
                        StructuralSequenceLength { path, .. } => {
                            path.iter().any(|step| match step {
                                crate::CheckedScalarReadProjection::Static(_) => false,
                                crate::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                                    scalar_uses_proof_gated_arithmetic(&selector.expression)
                                }
                            })
                        }
                        crate::CheckedScalarExpression::StructuralParameterIndexedRead {
                            index,
                            element_path,
                            ..
                        } => {
                            scalar_uses_proof_gated_arithmetic(&index.expression)
                                || element_path.iter().any(|projection| match projection {
                                    crate::CheckedScalarReadProjection::Static(_) => false,
                                    crate::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                                        scalar_uses_proof_gated_arithmetic(&selector.expression)
                                    }
                                })
                        }
                        crate::CheckedScalarExpression::IntegerBinary {
                            kind, left, right, ..
                        } => {
                            let exact = matches!(
                                kind,
                                crate::CheckedIntegerBinaryKind::ExactDivide
                                    | crate::CheckedIntegerBinaryKind::ExactRemainder
                            );
                            let policy = matches!(
                                kind,
                                crate::CheckedIntegerBinaryKind::WrappingDivide
                                    | crate::CheckedIntegerBinaryKind::WrappingRemainder
                                    | crate::CheckedIntegerBinaryKind::SaturatingDivide
                                    | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                            );
                            let runtime_divisor = matches!(
                                kind,
                                crate::CheckedIntegerBinaryKind::ExactDivide
                                    | crate::CheckedIntegerBinaryKind::ExactRemainder
                                    | crate::CheckedIntegerBinaryKind::WrappingDivide
                                    | crate::CheckedIntegerBinaryKind::WrappingRemainder
                                    | crate::CheckedIntegerBinaryKind::SaturatingDivide
                                    | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                            ) && !matches!(
                                right.as_ref(),
                                crate::CheckedScalarExpression::IntegerLiteral { literal }
                                    if literal.landing().is_some_and(|landing| {
                                        if landing.landed_type.is_signed() {
                                            literal.value_i64().is_some_and(|value| {
                                                value != 0 && (policy || (exact && value != -1))
                                            })
                                        } else {
                                            literal.value_u64().is_some_and(|value| value != 0)
                                        }
                                    })
                            );
                            let exact_shift = matches!(
                                kind,
                                crate::CheckedIntegerBinaryKind::ExactShiftLeft
                                    | crate::CheckedIntegerBinaryKind::ExactShiftRight
                            );
                            runtime_divisor
                                || exact_shift
                                || scalar_uses_proof_gated_arithmetic(left)
                                || scalar_uses_proof_gated_arithmetic(right)
                        }
                        crate::CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
                        | crate::CheckedScalarExpression::IntegerWiden { operand, .. }
                        | IntegerExactCast { operand, .. } => {
                            scalar_uses_proof_gated_arithmetic(operand)
                        }
                        Boolean(expression) => boolean_uses_proof_gated_arithmetic(expression),
                        _ => false,
                    }
                }

                fn boolean_uses_proof_gated_arithmetic(
                    expression: &crate::CheckedBooleanExpression,
                ) -> bool {
                    match expression {
                        Not(operand) => boolean_uses_proof_gated_arithmetic(operand),
                        crate::CheckedBooleanExpression::Equal { left, right }
                        | crate::CheckedBooleanExpression::And { left, right }
                        | crate::CheckedBooleanExpression::Or { left, right } => {
                            boolean_uses_proof_gated_arithmetic(left)
                                || boolean_uses_proof_gated_arithmetic(right)
                        }
                        crate::CheckedBooleanExpression::IntegerComparison {
                            left, right, ..
                        } => {
                            scalar_uses_proof_gated_arithmetic(left)
                                || scalar_uses_proof_gated_arithmetic(right)
                        }
                        _ => false,
                    }
                }

                self.published.iter().any(|bucket| {
            bucket.alternative_guards().iter().any(|guard| {
                matches!(guard, CrashRouteGuard::Predicate(predicate)
                    if predicate.scalar_expression().is_some_and(boolean_uses_proof_gated_arithmetic))
            })
        })
            }

            pub fn with_checked_sites(
                mut self,
                mut checked_sites: Vec<CheckedCrashSite>,
            ) -> Option<Self> {
                checked_sites.sort_by_key(|site| {
                    (
                        site.location.state.arena_index(),
                        site.location.state.generation(),
                        site.location.statement_ordinal,
                        site.cause,
                    )
                });
                checked_sites.dedup();
                if checked_sites.windows(2).any(|sites| {
                    sites[0].location.state == sites[1].location.state
                        && sites[0].location.statement_ordinal
                            == sites[1].location.statement_ordinal
                }) {
                    return None;
                }
                if checked_sites.iter().any(|site| {
                    site.guard_covering_buckets.iter().any(|bucket| {
                        self.published_bucket(*bucket)
                            .is_none_or(|published| published.cause != site.cause)
                    }) || site.frontier_lower_bound.contains(&Unknown)
                }) {
                    return None;
                }
                self.checked_sites = checked_sites;
                Some(self)
            }

            pub fn interface(&self) -> CrashInterface {
                self.interface
            }

            pub fn published(&self) -> &[CrashRouteBucket] {
                &self.published
            }

            pub fn published_with_ids(
                &self,
            ) -> impl Iterator<Item = (CrashRouteBucketId, &CrashRouteBucket)> {
                self.published
                    .iter()
                    .enumerate()
                    .map(|(index, bucket)| (CrashRouteBucketId::from_index(index), bucket))
            }

            pub fn published_bucket(&self, id: CrashRouteBucketId) -> Option<&CrashRouteBucket> {
                self.published.get(id.index()?)
            }

            pub fn checked_sites(&self) -> &[CheckedCrashSite] {
                &self.checked_sites
            }

            pub fn with_checked_calls(
                mut self,
                mut checked_calls: Vec<CheckedCrashCallSite>,
            ) -> Option<Self> {
                checked_calls.sort_by_key(|call| {
                    (
                        call.location.state.arena_index(),
                        call.location.state.generation(),
                        call.location.statement_ordinal,
                        call.location.call_ordinal,
                    )
                });
                checked_calls.dedup();
                if checked_calls
                    .windows(2)
                    .any(|calls| calls[0].location == calls[1].location)
                {
                    return None;
                }
                self.checked_calls = checked_calls;
                Some(self)
            }

            pub fn checked_calls(&self) -> &[CheckedCrashCallSite] {
                &self.checked_calls
            }

            pub fn with_checked_operators(
                mut self,
                mut operators: Vec<CheckedCrashOperatorSite>,
            ) -> Option<Self> {
                let identity = |site: &CheckedCrashOperatorSite| {
                    if site.named_use.is_valid() {
                        (1, site.named_use.arena_index(), site.named_use.generation())
                    } else {
                        (
                            0,
                            site.operator_use.arena_index(),
                            site.operator_use.generation(),
                        )
                    }
                };
                operators.sort_by_key(identity);
                if operators.iter().any(|site| {
                    !site.selected_operator.is_valid()
                        || site.operator_use.is_valid() == site.named_use.is_valid()
                        || (site.named_use.is_valid() && site.invocation.is_valid())
                        || (!site.named_use.is_valid() && !site.invocation.is_valid())
                }) || operators
                    .windows(2)
                    .any(|sites| identity(&sites[0]) == identity(&sites[1]))
                {
                    return None;
                }
                self.checked_operators = operators;
                Some(self)
            }

            pub fn checked_operators(&self) -> &[CheckedCrashOperatorSite] {
                &self.checked_operators
            }

            pub fn checked_call_at(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
                call_ordinal: u32,
            ) -> Option<&CheckedCrashCallSite> {
                self.checked_calls.iter().find(|call| {
                    call.location.state == state
                        && call.location.statement_ordinal == statement_ordinal
                        && call.location.call_ordinal == call_ordinal
                })
            }

            pub fn covering_buckets_for_site<'plan>(
                &'plan self,
                site: &'plan CheckedCrashSite,
            ) -> impl Iterator<Item = (CrashRouteBucketId, &'plan CrashRouteBucket)> + 'plan
            {
                site.guard_covering_buckets.iter().filter_map(move |id| {
                    self.published_bucket(*id)
                        .and_then(|bucket| (bucket.cause == site.cause).then_some((*id, bucket)))
                })
            }

            pub fn checked_site_at(
                &self,
                state: SymbolHandle,
                statement_ordinal: u32,
            ) -> Option<&CheckedCrashSite> {
                self.checked_sites.iter().find(|site| {
                    site.location.state == state
                        && site.location.statement_ordinal == statement_ordinal
                })
            }
        }
    }
    pub(crate) mod crash_predicates {
        use super::contract_identity::ContractHasher;
        use std::cmp::Ordering;
        use std::hash::Hash;
        use std::sync::Arc;

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub enum CrashCause {
            Trap,
            Abort,
        }

        impl From<typed_trees::signature::CrashCause> for CrashCause {
            fn from(cause: typed_trees::signature::CrashCause) -> Self {
                match cause {
                    typed_trees::signature::CrashCause::Trap => Self::Trap,
                    typed_trees::signature::CrashCause::Abort => Self::Abort,
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum CrashInterface {
            #[default]
            InternalInferred,
            PublishedCeiling,
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
        pub enum CrashPredicateExpression {
            Invalid,
            StructuralIdentity {
                type_commitment: super::MachineContractCommitment,
                left: Box<Self>,
                right: Box<Self>,
            },
            Binary {
                operator: u8,
                left: Box<Self>,
                right: Box<Self>,
            },
            Unary {
                operator: u8,
                operand: Box<Self>,
            },
            IntegerWiden {
                source_type: u8,
                target_type: u8,
                domain: u8,
                operand: Box<Self>,
            },
            RepresentationView {
                source_type: semantic_vocabulary::IntegerType,
                target_type: semantic_vocabulary::IntegerType,
                operand: Box<Self>,
            },
            Integer(String),
            Float(String),
            Boolean(bool),
            Name(Vec<String>),
            Member {
                receiver: Box<Self>,
                member: String,
            },
            Indexed {
                collection: Box<Self>,
                index: Box<Self>,
            },
            Range {
                start: Box<Self>,
                end: Box<Self>,
                end_inclusive: bool,
            },
            Call {
                target: String,
                receiver: Box<Self>,
                arguments: Vec<Self>,
            },
            Opaque(String),
            Parameter(u32),
            ContentConservation(Vec<u8>),
        }

        impl CrashPredicateExpression {
            pub fn contains_structural_identity(&self) -> bool {
                let mut pending = vec![self];
                while let Some(expression) = pending.pop() {
                    match expression {
                        Self::StructuralIdentity { .. } => return true,
                        Self::Binary { left, right, .. } => {
                            pending.extend([left.as_ref(), right.as_ref()])
                        }
                        Self::Unary { operand, .. }
                        | Self::IntegerWiden { operand, .. }
                        | Self::RepresentationView { operand, .. } => pending.push(operand),
                        Self::Member { receiver, .. } => pending.push(receiver),
                        Self::Indexed { collection, index } => {
                            pending.extend([collection.as_ref(), index.as_ref()])
                        }
                        Self::Range { start, end, .. } => {
                            pending.extend([start.as_ref(), end.as_ref()])
                        }
                        Self::Call {
                            receiver,
                            arguments,
                            ..
                        } => {
                            pending.push(receiver);
                            pending.extend(arguments);
                        }
                        Self::Invalid
                        | Self::Integer(_)
                        | Self::Float(_)
                        | Self::Boolean(_)
                        | Self::Name(_)
                        | Self::Opaque(_)
                        | Self::Parameter(_)
                        | Self::ContentConservation(_) => {}
                    }
                }
                false
            }

            pub fn structural_identity(
                type_identity: typed_trees::type_identity::NormalizedTypeIdentity,
                types: &typed_trees::type_identity::TypeIdentityTable,
                mut left: Self,
                mut right: Self,
            ) -> Option<Self> {
                let mut identity = ContractHasher::new(b"omega.crash-data-identity-type.v1\0");
                types.text(type_identity)?.hash(&mut identity);
                if left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                Some(Self::StructuralIdentity {
                    type_commitment: identity.complete().commitment,
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }

            pub fn substitute(&self, arguments: &[Option<Self>]) -> Self {
                enum Step<'a> {
                    Visit(&'a CrashPredicateExpression),
                    Finalize(&'a CrashPredicateExpression),
                }
                let mut pending = vec![Step::Visit(self)];
                let mut built: Vec<Self> = Vec::new();
                while let Some(step) = pending.pop() {
                    match step {
                        Step::Visit(term) => match term {
                            Self::Parameter(index) => built.push(
                                arguments
                                    .get(*index as usize)
                                    .and_then(Clone::clone)
                                    .unwrap_or_else(|| term.clone()),
                            ),
                            Self::Binary { left, right, .. }
                            | Self::StructuralIdentity { left, right, .. } => {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(right));
                                pending.push(Step::Visit(left));
                            }
                            Self::Unary { operand, .. }
                            | Self::IntegerWiden { operand, .. }
                            | Self::RepresentationView { operand, .. } => {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(operand));
                            }
                            Self::Member { receiver, .. } => {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(receiver));
                            }
                            Self::Indexed { collection, index } => {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(index));
                                pending.push(Step::Visit(collection));
                            }
                            Self::Range { start, end, .. } => {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(end));
                                pending.push(Step::Visit(start));
                            }
                            Self::Call {
                                receiver,
                                arguments: nested,
                                ..
                            } => {
                                pending.push(Step::Finalize(term));
                                for argument in nested.iter().rev() {
                                    pending.push(Step::Visit(argument));
                                }
                                pending.push(Step::Visit(receiver));
                            }
                            Self::Invalid
                            | Self::Integer(_)
                            | Self::Float(_)
                            | Self::Boolean(_)
                            | Self::Name(_)
                            | Self::Opaque(_)
                            | Self::ContentConservation(_) => built.push(term.clone()),
                        },
                        Step::Finalize(term) => {
                            let rebuilt = match term {
                                Self::StructuralIdentity {
                                    type_commitment, ..
                                } => {
                                    let mut right =
                                        built.pop().expect("right child rebuilt before its parent");
                                    let mut left =
                                        built.pop().expect("left child rebuilt before its parent");
                                    if left > right {
                                        std::mem::swap(&mut left, &mut right);
                                    }
                                    Self::StructuralIdentity {
                                        type_commitment: *type_commitment,
                                        left: Box::new(left),
                                        right: Box::new(right),
                                    }
                                }
                                Self::Binary { operator, .. } => {
                                    let right =
                                        built.pop().expect("right child rebuilt before its parent");
                                    let left =
                                        built.pop().expect("left child rebuilt before its parent");
                                    Self::Binary {
                                        operator: *operator,
                                        left: Box::new(left),
                                        right: Box::new(right),
                                    }
                                }
                                Self::Unary { operator, .. } => {
                                    let operand =
                                        built.pop().expect("operand rebuilt before its parent");
                                    Self::Unary {
                                        operator: *operator,
                                        operand: Box::new(operand),
                                    }
                                }
                                Self::IntegerWiden {
                                    source_type,
                                    target_type,
                                    domain,
                                    ..
                                } => {
                                    let operand =
                                        built.pop().expect("operand rebuilt before its parent");
                                    Self::IntegerWiden {
                                        source_type: *source_type,
                                        target_type: *target_type,
                                        domain: *domain,
                                        operand: Box::new(operand),
                                    }
                                }
                                Self::RepresentationView {
                                    source_type,
                                    target_type,
                                    ..
                                } => Self::RepresentationView {
                                    source_type: *source_type,
                                    target_type: *target_type,
                                    operand: Box::new(
                                        built.pop().expect("view operand rebuilt before parent"),
                                    ),
                                },
                                Self::Member { member, .. } => {
                                    let receiver =
                                        built.pop().expect("receiver rebuilt before its parent");
                                    Self::Member {
                                        receiver: Box::new(receiver),
                                        member: member.clone(),
                                    }
                                }
                                Self::Indexed { .. } => {
                                    let index =
                                        built.pop().expect("index rebuilt before its parent");
                                    let collection =
                                        built.pop().expect("collection rebuilt before its parent");
                                    Self::Indexed {
                                        collection: Box::new(collection),
                                        index: Box::new(index),
                                    }
                                }
                                Self::Range { end_inclusive, .. } => {
                                    let end =
                                        built.pop().expect("range end rebuilt before its parent");
                                    let start =
                                        built.pop().expect("range start rebuilt before its parent");
                                    Self::Range {
                                        start: Box::new(start),
                                        end: Box::new(end),
                                        end_inclusive: *end_inclusive,
                                    }
                                }
                                Self::Call {
                                    target,
                                    arguments: nested,
                                    ..
                                } => {
                                    let mut arguments = Vec::with_capacity(nested.len());
                                    for _ in nested {
                                        arguments.push(
                                            built
                                                .pop()
                                                .expect("call argument rebuilt before its parent"),
                                        );
                                    }
                                    arguments.reverse();
                                    let receiver = built
                                        .pop()
                                        .expect("call receiver rebuilt before its parent");
                                    Self::Call {
                                        target: target.clone(),
                                        receiver: Box::new(receiver),
                                        arguments,
                                    }
                                }
                                _ => unreachable!("childless nodes finish inside Visit"),
                            };
                            built.push(rebuilt);
                        }
                    }
                }
                debug_assert_eq!(built.len(), 1);
                built.pop().expect("substitution finishes its root")
            }

            pub fn boolean_value(&self) -> Option<bool> {
                use typed_trees::expression::{BinaryOperator, UnaryOperator};

                enum Step<'a> {
                    Visit(&'a CrashPredicateExpression),
                    Finalize(&'a CrashPredicateExpression),
                }
                let mut pending = vec![Step::Visit(self)];
                let mut values: Vec<bool> = Vec::new();
                while let Some(step) = pending.pop() {
                    match step {
                        Step::Visit(term) => match term {
                            Self::Boolean(value) => values.push(*value),
                            Self::Unary { operator, operand }
                                if *operator == UnaryOperator::LogicalNot as u8 =>
                            {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(operand));
                            }
                            Self::Binary {
                                operator,
                                left,
                                right,
                                ..
                            } if *operator == BinaryOperator::And as u8
                                || *operator == BinaryOperator::Or as u8 =>
                            {
                                pending.push(Step::Finalize(term));
                                pending.push(Step::Visit(right));
                                pending.push(Step::Visit(left));
                            }
                            _ => return None,
                        },
                        Step::Finalize(term) => match term {
                            Self::Unary { .. } => {
                                let operand =
                                    values.pop().expect("operand evaluated before its parent");
                                values.push(!operand);
                            }
                            Self::Binary { operator, .. }
                                if *operator == BinaryOperator::And as u8 =>
                            {
                                let right = values
                                    .pop()
                                    .expect("right operand evaluated before its parent");
                                let left = values
                                    .pop()
                                    .expect("left operand evaluated before its parent");
                                values.push(left && right);
                            }
                            Self::Binary { operator, .. }
                                if *operator == BinaryOperator::Or as u8 =>
                            {
                                let right = values
                                    .pop()
                                    .expect("right operand evaluated before its parent");
                                let left = values
                                    .pop()
                                    .expect("left operand evaluated before its parent");
                                values.push(left || right);
                            }
                            _ => unreachable!("only boolean-subset nodes reach Finalize"),
                        },
                    }
                }
                debug_assert_eq!(values.len(), 1);
                values.pop()
            }

            fn write_canonical(&self, out: &mut Vec<u8>) {
                enum Step<'a> {
                    Node(&'a CrashPredicateExpression),
                    Emit(u8),
                    EmitBytes(&'a [u8]),
                }
                let mut pending = vec![Step::Node(self)];
                while let Some(step) = pending.pop() {
                    match step {
                        Step::Node(term) => match term {
                            Self::Invalid => out.push(0),
                            Self::StructuralIdentity {
                                type_commitment,
                                left,
                                right,
                            } => {
                                out.push(0x0f);
                                out.extend(type_commitment.as_bytes());
                                pending.push(Step::Node(right));
                                pending.push(Step::Node(left));
                            }
                            Self::Binary {
                                operator,
                                left,
                                right,
                            } => {
                                out.push(1);
                                out.push(*operator);
                                pending.push(Step::Node(right));
                                pending.push(Step::Node(left));
                            }
                            Self::Unary { operator, operand } => {
                                out.push(2);
                                out.push(*operator);
                                pending.push(Step::Node(operand));
                            }
                            Self::IntegerWiden {
                                source_type,
                                target_type,
                                domain,
                                operand,
                            } => {
                                out.extend([0x0e, *source_type, *target_type, *domain]);
                                pending.push(Step::Node(operand));
                            }
                            Self::RepresentationView {
                                source_type,
                                target_type,
                                operand,
                            } => {
                                out.extend([
                                    0x10,
                                    source_type.sign() as u8,
                                    source_type.bits() as u8,
                                    target_type.sign() as u8,
                                    target_type.bits() as u8,
                                ]);
                                pending.push(Step::Node(operand));
                            }
                            Self::Integer(value) => {
                                out.push(3);
                                out.extend(value.as_bytes());
                                out.push(0);
                            }
                            Self::Boolean(value) => {
                                out.push(4);
                                out.push(u8::from(*value));
                            }
                            Self::Float(value) => {
                                out.push(0x0a);
                                out.extend(value.as_bytes());
                                out.push(0);
                            }
                            Self::Name(members) => {
                                out.push(5);
                                for member in members {
                                    out.extend(member.as_bytes());
                                    out.push(b'.');
                                }
                                out.push(0);
                            }
                            Self::Member { receiver, member } => {
                                out.push(6);
                                pending.push(Step::Emit(0));
                                pending.push(Step::EmitBytes(member.as_bytes()));
                                pending.push(Step::Node(receiver));
                            }
                            Self::Indexed { collection, index } => {
                                out.push(0x0b);
                                pending.push(Step::Node(index));
                                pending.push(Step::Node(collection));
                            }
                            Self::Range {
                                start,
                                end,
                                end_inclusive,
                            } => {
                                out.push(0x0d);
                                out.push(u8::from(*end_inclusive));
                                pending.push(Step::Node(end));
                                pending.push(Step::Node(start));
                            }
                            Self::Call {
                                target,
                                receiver,
                                arguments,
                            } => {
                                out.push(7);
                                out.extend(target.as_bytes());
                                out.push(0);
                                pending.push(Step::Emit(0xfe));
                                for argument in arguments.iter().rev() {
                                    pending.push(Step::Node(argument));
                                }
                                pending.push(Step::Node(receiver));
                            }
                            Self::Opaque(display) => {
                                out.push(8);
                                out.extend(display.as_bytes());
                                out.push(0);
                            }
                            Self::Parameter(index) => {
                                out.push(9);
                                out.extend(index.to_le_bytes());
                            }
                            Self::ContentConservation(bytes) => {
                                out.push(0xcc);
                                out.extend(bytes);
                            }
                        },
                        Step::Emit(byte) => out.push(byte),
                        Step::EmitBytes(bytes) => out.extend(bytes),
                    }
                }
            }
        }

        #[derive(Clone, compiler_storage::Record)]
        pub struct CrashPredicateIdentity(Arc<CrashPredicateIdentityParts>);

        #[derive(compiler_storage::Record)]
        struct CrashPredicateIdentityParts {
            canonical_bytes: Vec<u8>,
            expression: Option<CrashPredicateExpression>,
            scalar_expression: Option<crate::CheckedBooleanExpression>,
        }

        impl CrashPredicateIdentity {
            pub fn from_declared_fact(
                fact: &super::DeclaredContractFact,
                type_identities: &typed_trees::type_identity::TypeIdentityTable,
            ) -> Self {
                let mut identity = ContractHasher::new(b"omega.declared-crash-fact.v2\0");
                super::contract_identity::hash_declared_fact(fact, type_identities, &mut identity);
                let mut bytes = vec![2];
                bytes.extend(identity.complete().commitment.as_bytes());
                Self(Arc::new(CrashPredicateIdentityParts {
                    canonical_bytes: bytes,
                    expression: None,
                    scalar_expression: None,
                }))
            }

            pub fn from_operand(expression: CrashPredicateExpression) -> Self {
                let mut canonical_bytes = vec![0x6f];
                expression.write_canonical(&mut canonical_bytes);
                Self(Arc::new(CrashPredicateIdentityParts {
                    canonical_bytes,
                    expression: None,
                    scalar_expression: None,
                }))
            }

            pub fn from_expression(expression: CrashPredicateExpression) -> Self {
                Self::from_parts(expression, None)
            }

            pub fn from_expression_and_scalar(
                expression: CrashPredicateExpression,
                scalar_expression: crate::CheckedBooleanExpression,
            ) -> Self {
                Self::from_parts(expression, Some(scalar_expression))
            }

            fn from_parts(
                expression: CrashPredicateExpression,
                scalar_expression: Option<crate::CheckedBooleanExpression>,
            ) -> Self {
                let mut canonical_bytes = vec![1];
                expression.write_canonical(&mut canonical_bytes);
                Self(Arc::new(CrashPredicateIdentityParts {
                    canonical_bytes,
                    expression: Some(expression),
                    scalar_expression,
                }))
            }

            pub fn canonical_bytes(&self) -> &[u8] {
                &self.0.canonical_bytes
            }

            pub fn expression(&self) -> Option<&CrashPredicateExpression> {
                self.0.expression.as_ref()
            }

            pub fn scalar_expression(&self) -> Option<&crate::CheckedBooleanExpression> {
                self.0.scalar_expression.as_ref()
            }
        }

        impl std::fmt::Debug for CrashPredicateIdentity {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("CrashPredicateIdentity")
                    .field("canonical_bytes", &self.0.canonical_bytes)
                    .field("expression", &self.0.expression)
                    .field("scalar_expression", &self.0.scalar_expression)
                    .finish()
            }
        }

        impl PartialEq for CrashPredicateIdentity {
            fn eq(&self, other: &Self) -> bool {
                Arc::ptr_eq(&self.0, &other.0) || self.0.canonical_bytes == other.0.canonical_bytes
            }
        }

        impl Eq for CrashPredicateIdentity {}

        impl PartialOrd for CrashPredicateIdentity {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for CrashPredicateIdentity {
            fn cmp(&self, other: &Self) -> Ordering {
                if Arc::ptr_eq(&self.0, &other.0) {
                    return Ordering::Equal;
                }
                self.0.canonical_bytes.cmp(&other.0.canonical_bytes)
            }
        }

        impl Hash for CrashPredicateIdentity {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.canonical_bytes.hash(state);
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
        pub enum CrashRouteGuard {
            Truth,
            Predicate(CrashPredicateIdentity),
        }

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub struct CrashRouteBucketId(u32);

        impl CrashRouteBucketId {
            pub(crate) fn from_index(index: usize) -> Self {
                Self(
                    u32::try_from(index)
                        .expect("published crash bucket count exceeds checked identity range")
                        .checked_add(1)
                        .expect("published crash bucket identity is one-based"),
                )
            }

            pub const fn get(self) -> u32 {
                self.0
            }

            pub(crate) fn index(self) -> Option<usize> {
                usize::try_from(self.0.checked_sub(1)?).ok()
            }
        }
    }
    pub(crate) mod crash_sites {
        use crate::facts::contract_plans::{
            CrashCause, CrashPredicateIdentity, CrashRouteBucket, CrashRouteBucketId,
            MachineContractCommitment,
        };
        use language_semantics::TerminationGuarantee;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CrashSiteLocation {
            pub(crate) state: SymbolHandle,
            pub(crate) statement_ordinal: u32,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct CrashCallSiteLocation {
            pub(crate) state: SymbolHandle,
            pub(crate) statement_ordinal: u32,
            pub(crate) call_ordinal: u32,
        }

        macro_rules! sorted_roster_setter {
            ($name:ident, $field:ident) => {
                pub fn $name(mut self, mut $field: Vec<CrashPredicateIdentity>) -> Self {
                    $field = sorted_dedup($field);
                    self.$field = $field;
                    self
                }
            };
        }

        impl CrashCallSiteLocation {
            pub const fn new(
                state: SymbolHandle,
                statement_ordinal: u32,
                call_ordinal: u32,
            ) -> Self {
                Self {
                    state,
                    statement_ordinal,
                    call_ordinal,
                }
            }

            pub const fn state(self) -> SymbolHandle {
                self.state
            }

            pub const fn statement_ordinal(self) -> u32 {
                self.statement_ordinal
            }

            pub const fn call_ordinal(self) -> u32 {
                self.call_ordinal
            }
        }

        impl CrashSiteLocation {
            pub const fn new(state: SymbolHandle, statement_ordinal: u32) -> Self {
                Self {
                    state,
                    statement_ordinal,
                }
            }

            pub const fn state(self) -> SymbolHandle {
                self.state
            }

            pub const fn statement_ordinal(self) -> u32 {
                self.statement_ordinal
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCrashSite {
            pub(crate) location: CrashSiteLocation,
            pub(crate) cause: CrashCause,
            path_guard_conjuncts: Vec<CrashPredicateIdentity>,
            path_guard_consequences: Vec<CrashPredicateIdentity>,
            pub(crate) guard_covering_buckets: Vec<CrashRouteBucketId>,
            pub(crate) frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
        }

        impl CheckedCrashSite {
            pub fn new(
                location: CrashSiteLocation,
                cause: CrashCause,
                mut guard_covering_buckets: Vec<CrashRouteBucketId>,
                mut frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
            ) -> Self {
                guard_covering_buckets.sort_unstable();
                guard_covering_buckets.dedup();
                frontier_lower_bound
                    .sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
                frontier_lower_bound.dedup();
                Self {
                    location,
                    cause,
                    path_guard_conjuncts: Vec::new(),
                    path_guard_consequences: Vec::new(),
                    guard_covering_buckets,
                    frontier_lower_bound,
                }
            }

            pub const fn location(&self) -> CrashSiteLocation {
                self.location
            }

            pub const fn cause(&self) -> CrashCause {
                self.cause
            }

            pub fn with_guard_covering_buckets(
                mut self,
                mut guard_covering_buckets: Vec<CrashRouteBucketId>,
            ) -> Self {
                guard_covering_buckets.sort_unstable();
                guard_covering_buckets.dedup();
                self.guard_covering_buckets = guard_covering_buckets;
                self
            }

            sorted_roster_setter!(with_path_guard_conjuncts, path_guard_conjuncts);

            sorted_roster_setter!(with_path_guard_consequences, path_guard_consequences);

            pub fn with_frontier_lower_bound(
                mut self,
                mut frontier_lower_bound: Vec<language_semantics::PermissionClaimIdentity>,
            ) -> Self {
                frontier_lower_bound
                    .sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
                frontier_lower_bound.dedup();
                self.frontier_lower_bound = frontier_lower_bound;
                self
            }

            pub fn guard_covering_buckets(&self) -> &[CrashRouteBucketId] {
                &self.guard_covering_buckets
            }

            pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
                &self.path_guard_conjuncts
            }

            pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
                &self.path_guard_consequences
            }

            pub fn frontier_lower_bound(&self) -> &[language_semantics::PermissionClaimIdentity] {
                &self.frontier_lower_bound
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedCrashCallSite {
            pub(crate) location: CrashCallSiteLocation,
            target_machine: SymbolHandle,
            target_state: SymbolHandle,
            target_contract_report_fingerprint: u64,
            target_contract_commitment: MachineContractCommitment,
            path_guard_conjuncts: Vec<CrashPredicateIdentity>,
            path_guard_consequences: Vec<CrashPredicateIdentity>,
            surviving_buckets: Vec<CrashRouteBucket>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CrashContractCapsule {
            pub(crate) target_machine: SymbolHandle,
            pub(crate) target_state: SymbolHandle,
            target_contract_report_fingerprint: u64,
            target_contract_commitment: MachineContractCommitment,
            published_service_reach: Vec<String>,
            published_synchronous_invocations: Vec<String>,
            published_may_suspend: bool,
            published_may_block: bool,
            published_termination: TerminationGuarantee,
            published_buckets: Vec<CrashRouteBucket>,
        }

        impl CrashContractCapsule {
            pub fn new(
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
                target_contract_report_fingerprint: u64,
                published_buckets: Vec<CrashRouteBucket>,
            ) -> Self {
                Self::new_with_commitment(
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    MachineContractCommitment::from_digest([0; 32]),
                    published_buckets,
                )
            }

            pub fn new_with_commitment(
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
                target_contract_report_fingerprint: u64,
                target_contract_commitment: MachineContractCommitment,
                mut published_buckets: Vec<CrashRouteBucket>,
            ) -> Self {
                published_buckets = sorted_dedup(published_buckets);
                Self {
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    target_contract_commitment,
                    published_service_reach: Vec::new(),
                    published_synchronous_invocations: Vec::new(),
                    published_may_suspend: false,
                    published_may_block: false,
                    published_termination: TerminationGuarantee::NoGuarantee,
                    published_buckets,
                }
            }

            pub fn with_operational_envelope(
                mut self,
                mut published_service_reach: Vec<String>,
                mut published_synchronous_invocations: Vec<String>,
                published_may_suspend: bool,
                published_may_block: bool,
                published_termination: TerminationGuarantee,
            ) -> Self {
                published_service_reach = sorted_dedup(published_service_reach);
                published_synchronous_invocations = sorted_dedup(published_synchronous_invocations);
                self.published_service_reach = published_service_reach;
                self.published_synchronous_invocations = published_synchronous_invocations;
                self.published_may_suspend = published_may_suspend;
                self.published_may_block = published_may_block;
                self.published_termination = published_termination;
                self
            }

            super::contract_identity_accessors!(
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                target_contract_commitment
            );

            pub fn published_buckets(&self) -> &[CrashRouteBucket] {
                &self.published_buckets
            }

            pub fn published_service_reach(&self) -> &[String] {
                &self.published_service_reach
            }

            pub fn published_synchronous_invocations(&self) -> &[String] {
                &self.published_synchronous_invocations
            }

            pub const fn published_may_suspend(&self) -> bool {
                self.published_may_suspend
            }

            pub const fn published_may_block(&self) -> bool {
                self.published_may_block
            }

            pub const fn published_termination(&self) -> &TerminationGuarantee {
                &self.published_termination
            }
        }

        impl CheckedCrashCallSite {
            pub fn executable_contract_identity(
                &self,
                plans: &crate::MachineContractPlans,
                proof: &crate::ProofFacts,
                caller_machine: SymbolHandle,
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
            ) -> Result<crate::MachineContractIdentity, &'static str> {
                let target = plans
                    .for_machine(target_machine)
                    .ok_or("executable call target has no checked contract plan")?;
                let identity = crate::MachineContractIdentity {
                    report_fingerprint: target.report_fingerprint,
                    commitment: target.commitment,
                };
                if self.target_machine == target_machine && self.target_state == target_state {
                    if self.target_contract_report_fingerprint != identity.report_fingerprint
                        || self.target_contract_commitment != identity.commitment
                    {
                        return Err("checked call target contract identity disagrees");
                    }
                    return Ok(identity);
                }

                let binding = proof
                    .unique_contract_call_at(&(
                        caller_machine,
                        self.location.state,
                        self.location.statement_ordinal as usize,
                        self.location.call_ordinal as usize,
                    ))
                    .ok_or("requirement call has no unique checked executable binding")?;
                if binding.target_machine_symbol != target_machine
                    || binding.target_state_symbol != target_state
                    || !proof.call_contract_signatures.is_valid(binding.signature)
                {
                    return Err("requirement call has a mismatched checked executable binding");
                }
                let public = proof.call_contract_signatures.get(binding.signature);
                if public.caller_machine_symbol != caller_machine
                    || public.caller_state_symbol != self.location.state
                    || public.statement_index != self.location.statement_ordinal as usize
                    || public.call_ordinal != self.location.call_ordinal as usize
                    || self.target_machine != public.declaring_trait
                    || self.target_state != public.signature.symbol
                {
                    return Err("requirement crash check belongs to another public call");
                }
                Ok(identity)
            }

            pub fn new(
                location: CrashCallSiteLocation,
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
                target_contract_report_fingerprint: u64,
                surviving_buckets: Vec<CrashRouteBucket>,
            ) -> Self {
                Self::new_with_commitment(
                    location,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    MachineContractCommitment::from_digest([0; 32]),
                    surviving_buckets,
                )
            }

            pub fn new_with_commitment(
                location: CrashCallSiteLocation,
                target_machine: SymbolHandle,
                target_state: SymbolHandle,
                target_contract_report_fingerprint: u64,
                target_contract_commitment: MachineContractCommitment,
                mut surviving_buckets: Vec<CrashRouteBucket>,
            ) -> Self {
                surviving_buckets = sorted_dedup(surviving_buckets);
                Self {
                    location,
                    target_machine,
                    target_state,
                    target_contract_report_fingerprint,
                    target_contract_commitment,
                    path_guard_conjuncts: Vec::new(),
                    path_guard_consequences: Vec::new(),
                    surviving_buckets,
                }
            }

            pub const fn location(&self) -> CrashCallSiteLocation {
                self.location
            }

            super::contract_identity_accessors!(
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                target_contract_commitment
            );

            pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
                &self.path_guard_conjuncts
            }

            pub fn path_guard_consequences(&self) -> &[CrashPredicateIdentity] {
                &self.path_guard_consequences
            }

            pub fn surviving_buckets(&self) -> &[CrashRouteBucket] {
                &self.surviving_buckets
            }

            sorted_roster_setter!(with_path_guard_conjuncts, path_guard_conjuncts);

            sorted_roster_setter!(with_path_guard_consequences, path_guard_consequences);
        }

        fn sorted_dedup<Row: Ord>(mut rows: Vec<Row>) -> Vec<Row> {
            rows.sort();
            rows.dedup();
            rows
        }

        fn crash_frontier_claim_sort_key(
            identity: language_semantics::PermissionClaimIdentity,
        ) -> [u64; 11] {
            use language_semantics::{PermissionClaimIdentity, PermissionEventSource};

            let PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } = identity
            else {
                return [0; 11];
            };
            let mut key = [0; 11];
            key[0] = 1;
            key[1] = u64::from(machine_symbol.arena_index());
            key[2] = u64::from(machine_symbol.generation());
            key[3] = u64::from(state_symbol.arena_index());
            key[4] = u64::from(state_symbol.generation());
            match source {
                PermissionEventSource::StateEntry => key[5] = 0,
                PermissionEventSource::Statement { statement_index } => {
                    key[5] = 1;
                    key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
                }
                PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => {
                    key[5] = 2;
                    key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
                    key[7] = u64::try_from(call_ordinal).unwrap_or(u64::MAX);
                    key[8] = u64::from(target_symbol.arena_index());
                    key[9] = u64::from(target_symbol.generation());
                }
                PermissionEventSource::StateExit => key[5] = 3,
                PermissionEventSource::InstructionExitDisposition { statement_index } => {
                    key[5] = 5;
                    key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
                }
                PermissionEventSource::InstructionDisposition { statement_index } => {
                    key[5] = 4;
                    key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
                }
            }
            key[10] = u64::from(ordinal);
            key
        }
    }
    pub(crate) mod resource_envelopes {
        use crate::facts::contract_plans::{CrashPlan, MachineContractCommitment};
        use language_semantics::TerminationGuarantee;
        use std::hash::Hash;
        use symbols::SymbolHandle;

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
        )]
        pub enum CheckedResourceDerivationObligation {
            TerminalAndTargetStackClosure,
            TerminalControlAndWorkSchedule,
            SelectedInstructionMachineStateFootprint,
        }

        impl CheckedResourceDerivationObligation {
            const fn identity_tag(self) -> u8 {
                match self {
                    Self::TerminalAndTargetStackClosure => 1,
                    Self::TerminalControlAndWorkSchedule => 2,
                    Self::SelectedInstructionMachineStateFootprint => 3,
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedResourceAxisAnchor {
            machine: SymbolHandle,
            entry: SymbolHandle,
            contract_report_fingerprint: u64,
            contract_commitment: MachineContractCommitment,
            derivation_obligation: CheckedResourceDerivationObligation,
            pub(crate) report_fingerprint: u64,
        }

        impl CheckedResourceAxisAnchor {
            fn from_checked_contract(
                machine: SymbolHandle,
                entry: SymbolHandle,
                contract_report_fingerprint: u64,
                contract_commitment: MachineContractCommitment,
                derivation_obligation: CheckedResourceDerivationObligation,
            ) -> Self {
                let report_fingerprint = checked_resource_axis_report_fingerprint(
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                    derivation_obligation,
                );
                Self {
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                    derivation_obligation,
                    report_fingerprint,
                }
            }

            super::contract_identity_accessors!(
                machine,
                entry,
                contract_report_fingerprint,
                contract_commitment
            );

            pub const fn derivation_obligation(&self) -> CheckedResourceDerivationObligation {
                self.derivation_obligation
            }

            pub const fn report_fingerprint(&self) -> u64 {
                self.report_fingerprint
            }

            fn validate(
                &self,
                machine: SymbolHandle,
                entry: SymbolHandle,
                contract_report_fingerprint: u64,
                contract_commitment: MachineContractCommitment,
                expected_obligation: CheckedResourceDerivationObligation,
            ) -> Result<(), &'static str> {
                if self.machine != machine
                    || self.entry != entry
                    || self.contract_report_fingerprint != contract_report_fingerprint
                    || self.contract_commitment != contract_commitment
                {
                    return Err(
                        "checked resource axis does not bind the enclosing exact machine entry contract",
                    );
                }
                if self.derivation_obligation != expected_obligation {
                    return Err(
                        "checked resource envelope substituted or fused an independent resource axis",
                    );
                }
                if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
                    return Err("checked resource axis requires a nonzero exact contract identity");
                }
                if self.report_fingerprint
                    != checked_resource_axis_report_fingerprint(
                        self.machine,
                        self.entry,
                        self.contract_report_fingerprint,
                        self.contract_commitment,
                        self.derivation_obligation,
                    )
                {
                    return Err("checked resource axis report fingerprint does not replay");
                }
                Ok(())
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedEntryResourceEnvelope {
            machine: SymbolHandle,
            entry: SymbolHandle,
            contract_report_fingerprint: u64,
            contract_commitment: MachineContractCommitment,
            pub(crate) stack: CheckedResourceAxisAnchor,
            pub(crate) logical_structural_work: CheckedResourceAxisAnchor,
            pub(crate) machine_state: CheckedResourceAxisAnchor,
            report_fingerprint: u64,
        }

        impl CheckedEntryResourceEnvelope {
            pub fn from_checked_contract(
                machine: SymbolHandle,
                entry: SymbolHandle,
                contract_report_fingerprint: u64,
                contract_commitment: MachineContractCommitment,
            ) -> Self {
                let [stack, logical_structural_work, machine_state] = [
                    CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
                    CheckedResourceDerivationObligation::TerminalControlAndWorkSchedule,
                    CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
                ]
                .map(|obligation| {
                    CheckedResourceAxisAnchor::from_checked_contract(
                        machine,
                        entry,
                        contract_report_fingerprint,
                        contract_commitment,
                        obligation,
                    )
                });
                let report_fingerprint = checked_resource_envelope_report_fingerprint(
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                    stack.report_fingerprint,
                    logical_structural_work.report_fingerprint,
                    machine_state.report_fingerprint,
                );
                Self {
                    machine,
                    entry,
                    contract_report_fingerprint,
                    contract_commitment,
                    stack,
                    logical_structural_work,
                    machine_state,
                    report_fingerprint,
                }
            }

            super::contract_identity_accessors!(
                machine,
                entry,
                contract_report_fingerprint,
                contract_commitment
            );

            pub const fn stack(&self) -> &CheckedResourceAxisAnchor {
                &self.stack
            }

            pub const fn logical_structural_work(&self) -> &CheckedResourceAxisAnchor {
                &self.logical_structural_work
            }

            pub const fn machine_state(&self) -> &CheckedResourceAxisAnchor {
                &self.machine_state
            }

            pub const fn report_fingerprint(&self) -> u64 {
                self.report_fingerprint
            }

            pub fn validate(&self) -> Result<(), &'static str> {
                if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
                    return Err(
                        "checked resource envelope requires a nonzero exact contract identity",
                    );
                }
                for (anchor, obligation) in [
            (
                &self.stack,
                CheckedResourceDerivationObligation::TerminalAndTargetStackClosure,
            ),
            (
                &self.logical_structural_work,
                CheckedResourceDerivationObligation::TerminalControlAndWorkSchedule,
            ),
            (
                &self.machine_state,
                CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint,
            ),
        ] {
            anchor.validate(
                self.machine,
                self.entry,
                self.contract_report_fingerprint,
                self.contract_commitment,
                obligation,
            )?;
        }
                if self.report_fingerprint
                    != checked_resource_envelope_report_fingerprint(
                        self.machine,
                        self.entry,
                        self.contract_report_fingerprint,
                        self.contract_commitment,
                        self.stack.report_fingerprint,
                        self.logical_structural_work.report_fingerprint,
                        self.machine_state.report_fingerprint,
                    )
                {
                    return Err("checked resource envelope report fingerprint does not replay");
                }
                Ok(())
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct CheckedMachineResourceEnvelopes {
            pub(crate) machine: SymbolHandle,
            pub(crate) contract_report_fingerprint: u64,
            pub(crate) contract_commitment: MachineContractCommitment,
            pub(crate) entries: Vec<CheckedEntryResourceEnvelope>,
            roster_report_fingerprint: u64,
        }

        impl CheckedMachineResourceEnvelopes {
            pub fn from_checked_contract_entries(
                machine: SymbolHandle,
                contract_report_fingerprint: u64,
                contract_commitment: MachineContractCommitment,
                entries: impl IntoIterator<Item = SymbolHandle>,
            ) -> Self {
                let entries = entries
                    .into_iter()
                    .map(|entry| {
                        CheckedEntryResourceEnvelope::from_checked_contract(
                            machine,
                            entry,
                            contract_report_fingerprint,
                            contract_commitment,
                        )
                    })
                    .collect::<Vec<_>>();
                let roster_report_fingerprint = checked_resource_roster_report_fingerprint(
                    machine,
                    contract_report_fingerprint,
                    contract_commitment,
                    &entries,
                );
                Self {
                    machine,
                    contract_report_fingerprint,
                    contract_commitment,
                    entries,
                    roster_report_fingerprint,
                }
            }

            pub fn iter(&self) -> std::slice::Iter<'_, CheckedEntryResourceEnvelope> {
                self.entries.iter()
            }

            pub fn get(&self, entry: SymbolHandle) -> Option<&CheckedEntryResourceEnvelope> {
                self.entries.iter().find(|resource| resource.entry == entry)
            }

            pub const fn len(&self) -> usize {
                self.entries.len()
            }

            pub const fn is_empty(&self) -> bool {
                self.entries.is_empty()
            }

            pub fn validate(&self) -> Result<(), &'static str> {
                if self.contract_report_fingerprint == 0 || self.contract_commitment.is_zero() {
                    return Err(
                        "checked resource roster requires a nonzero exact contract identity",
                    );
                }
                for (index, resource) in self.entries.iter().enumerate() {
                    resource.validate()?;
                    if resource.machine != self.machine
                        || resource.contract_report_fingerprint != self.contract_report_fingerprint
                        || resource.contract_commitment != self.contract_commitment
                    {
                        return Err(
                            "checked entry resource envelope does not bind its exact machine contract",
                        );
                    }
                    if self.entries[..index]
                        .iter()
                        .any(|prior| prior.entry == resource.entry)
                    {
                        return Err("checked resource roster retained a duplicate machine entry");
                    }
                    if resource
                        != &CheckedEntryResourceEnvelope::from_checked_contract(
                            self.machine,
                            resource.entry,
                            self.contract_report_fingerprint,
                            self.contract_commitment,
                        )
                    {
                        return Err(
                            "checked entry resource envelope does not independently replay",
                        );
                    }
                }
                if self.roster_report_fingerprint
                    != checked_resource_roster_report_fingerprint(
                        self.machine,
                        self.contract_report_fingerprint,
                        self.contract_commitment,
                        &self.entries,
                    )
                {
                    return Err("checked resource roster report fingerprint does not replay");
                }
                Ok(())
            }
        }

        fn checked_resource_axis_report_fingerprint(
            machine: SymbolHandle,
            entry: SymbolHandle,
            contract_report_fingerprint: u64,
            contract_commitment: MachineContractCommitment,
            derivation_obligation: CheckedResourceDerivationObligation,
        ) -> u64 {
            let mut bytes = b"omega.checked.resource-axis.v1".to_vec();
            bytes.extend(machine.arena_index().to_le_bytes());
            bytes.extend(machine.generation().to_le_bytes());
            bytes.extend(entry.arena_index().to_le_bytes());
            bytes.extend(entry.generation().to_le_bytes());
            bytes.extend(contract_report_fingerprint.to_le_bytes());
            bytes.extend(contract_commitment.as_bytes());
            bytes.push(derivation_obligation.identity_tag());
            checked_resource_report_fingerprint(bytes)
        }

        fn checked_resource_envelope_report_fingerprint(
            machine: SymbolHandle,
            entry: SymbolHandle,
            contract_report_fingerprint: u64,
            contract_commitment: MachineContractCommitment,
            stack: u64,
            logical_structural_work: u64,
            machine_state: u64,
        ) -> u64 {
            let mut bytes = b"omega.checked.resource-envelope.v1".to_vec();
            bytes.extend(machine.arena_index().to_le_bytes());
            bytes.extend(machine.generation().to_le_bytes());
            bytes.extend(entry.arena_index().to_le_bytes());
            bytes.extend(entry.generation().to_le_bytes());
            bytes.extend(contract_report_fingerprint.to_le_bytes());
            bytes.extend(contract_commitment.as_bytes());
            bytes.extend(stack.to_le_bytes());
            bytes.extend(logical_structural_work.to_le_bytes());
            bytes.extend(machine_state.to_le_bytes());
            checked_resource_report_fingerprint(bytes)
        }

        fn checked_resource_roster_report_fingerprint(
            machine: SymbolHandle,
            contract_report_fingerprint: u64,
            contract_commitment: MachineContractCommitment,
            entries: &[CheckedEntryResourceEnvelope],
        ) -> u64 {
            let mut bytes = b"omega.checked.resource-roster.v1".to_vec();
            bytes.extend(machine.arena_index().to_le_bytes());
            bytes.extend(machine.generation().to_le_bytes());
            bytes.extend(contract_report_fingerprint.to_le_bytes());
            bytes.extend(contract_commitment.as_bytes());
            bytes.extend(
                u64::try_from(entries.len())
                    .unwrap_or(u64::MAX)
                    .to_le_bytes(),
            );
            for entry in entries {
                bytes.extend(entry.entry.arena_index().to_le_bytes());
                bytes.extend(entry.entry.generation().to_le_bytes());
                bytes.extend(entry.report_fingerprint.to_le_bytes());
            }
            checked_resource_report_fingerprint(bytes)
        }

        fn checked_resource_report_fingerprint(bytes: impl IntoIterator<Item = u8>) -> u64 {
            let mut hash = semantic_vocabulary::Fnv1a::new();
            for byte in bytes {
                hash.byte(byte);
            }
            hash.finish()
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct RealizedMachineContractEnvelope {
            pub machine: SymbolHandle,
            pub contract_report_fingerprint: u64,
            pub contract_commitment: MachineContractCommitment,
            pub effective_service_reach: Vec<String>,
            pub concrete_service_reach: Vec<String>,
            pub unresolved_installation_reaches: Vec<flow_effects::InstallationReachRequirement>,
            pub effective_synchronous_invocations: Vec<String>,
            pub checked_may_suspend: bool,
            pub checked_may_block: bool,
            pub checked_termination: TerminationGuarantee,
            pub checked_crash: CrashPlan,
            pub mutation: Vec<crate::StateWriteFramePlan>,
            pub capabilities: Vec<flow_effects::CapabilityFlowFact>,
            pub resources: CheckedMachineResourceEnvelopes,
        }
    }
    pub(crate) mod scalar_contracts {
        use numerics::literals::IntegerLiteral;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct ClosedFloatRangeRequirement {
            pub position: usize,
            pub primitive_type: typed_trees::types::PrimitiveType,
            pub minimum: semantic_vocabulary::IeeeFloatValue,
            pub maximum: semantic_vocabulary::IeeeFloatValue,
            pub maximum_inclusive: bool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ClosedIntegerRangeRequirement {
            pub position: usize,
            pub primitive_type: typed_trees::types::PrimitiveType,
            pub minimum: IntegerLiteral,
            pub maximum: IntegerLiteral,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum ClosedScalarContractValue {
            Boolean(bool),
            Integer(IntegerLiteral),
            Predicate(crate::CheckedBooleanExpression),
            FloatRange(ClosedFloatRangeRequirement),
            SourceCheckedGuarantee {
                facts: arena::HandleSpan<typed_trees::domain::ProofFact>,
                result_qualifications: Box<[crate::CheckedStructuralPathQualification]>,
            },
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct ClosedScalarValueContractPlan {
            requires: Vec<Option<ClosedScalarContractValue>>,
            authored_requires_len: usize,
            ensures: Vec<Option<ClosedScalarContractValue>>,
            has_crash_clauses: bool,
            has_outcome_specific_clauses: bool,
            float_entry_ranges: Option<Vec<ClosedFloatRangeRequirement>>,
            integer_entry_ranges: Option<Vec<ClosedIntegerRangeRequirement>>,
        }

        impl ClosedScalarValueContractPlan {
            pub fn new(
                requires: Vec<Option<ClosedScalarContractValue>>,
                ensures: Vec<Option<ClosedScalarContractValue>>,
                has_crash_clauses: bool,
                has_outcome_specific_clauses: bool,
            ) -> Self {
                let authored_requires_len = requires.len();
                Self {
                    requires,
                    authored_requires_len,
                    ensures,
                    has_crash_clauses,
                    has_outcome_specific_clauses,
                    float_entry_ranges: None,
                    integer_entry_ranges: None,
                }
            }

            pub fn with_authored_requires_len(mut self, authored_requires_len: usize) -> Self {
                self.authored_requires_len = authored_requires_len;
                self
            }

            pub fn with_float_entry_ranges(
                mut self,
                float_entry_ranges: Option<Vec<ClosedFloatRangeRequirement>>,
            ) -> Self {
                self.float_entry_ranges = float_entry_ranges;
                self
            }

            pub fn float_entry_ranges(&self) -> Option<&[ClosedFloatRangeRequirement]> {
                self.float_entry_ranges.as_deref()
            }

            pub fn with_integer_entry_ranges(
                mut self,
                integer_entry_ranges: Option<Vec<ClosedIntegerRangeRequirement>>,
            ) -> Self {
                self.integer_entry_ranges = integer_entry_ranges;
                self
            }

            pub fn integer_entry_ranges(&self) -> Option<&[ClosedIntegerRangeRequirement]> {
                self.integer_entry_ranges.as_deref()
            }

            pub fn requires(&self) -> &[Option<ClosedScalarContractValue>] {
                &self.requires
            }

            pub fn authored_requires(&self) -> &[Option<ClosedScalarContractValue>] {
                &self.requires[..self.authored_requires_len]
            }

            pub fn ensures(&self) -> &[Option<ClosedScalarContractValue>] {
                &self.ensures
            }

            pub const fn has_crash_clauses(&self) -> bool {
                self.has_crash_clauses
            }

            pub const fn has_outcome_specific_clauses(&self) -> bool {
                self.has_outcome_specific_clauses
            }
        }
    }

    pub use contract_identity::{
        DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact,
        DeclaredContractKind, DeclaredContractParameter, DeclaredContractStructure,
        DeclaredProgressPremise, DeclaredTermination, contract_declarations_commitment,
        contract_identity, declared_termination_commitment,
    };
    pub use crash_plans::{
        CheckedBodyCrashSummary, CheckedRuntimeMembershipSubject, CheckedTrappingSite, CrashPlan,
        CrashRouteBucket,
    };
    pub use crash_predicates::{
        CrashCause, CrashInterface, CrashPredicateExpression, CrashPredicateIdentity,
        CrashRouteBucketId, CrashRouteGuard,
    };
    pub use crash_sites::{
        CheckedCrashCallSite, CheckedCrashSite, CrashCallSiteLocation, CrashContractCapsule,
        CrashSiteLocation,
    };
    pub use resource_envelopes::{
        CheckedEntryResourceEnvelope, CheckedMachineResourceEnvelopes, CheckedResourceAxisAnchor,
        CheckedResourceDerivationObligation, RealizedMachineContractEnvelope,
    };
    pub use scalar_contracts::{
        ClosedFloatRangeRequirement, ClosedIntegerRangeRequirement, ClosedScalarContractValue,
        ClosedScalarValueContractPlan,
    };

    use std::hash::Hash;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, compiler_storage::Record)]
    pub struct MachineContractPlans {
        machines: Vec<MachineContractPlan>,
        pub crash_capsules: Vec<CrashContractCapsule>,
        pub realized_envelopes: Vec<RealizedMachineContractEnvelope>,
        #[record(skip)]
        machine_index: Vec<((u32, u32), usize)>,
        #[record(skip)]
        indexed: bool,
    }

    impl PartialEq for MachineContractPlans {
        fn eq(&self, other: &Self) -> bool {
            self.machines == other.machines
                && self.crash_capsules == other.crash_capsules
                && self.realized_envelopes == other.realized_envelopes
        }
    }

    impl Eq for MachineContractPlans {}

    impl MachineContractPlans {
        pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
            if !self.indexed {
                return contract_plan_by_machine(&self.machines, machine);
            }
            let key = (machine.arena_index(), machine.generation());
            let start = self.machine_index.partition_point(|(row, _)| *row < key);
            self.machine_index[start..]
                .iter()
                .take_while(|(row, _)| *row == key)
                .filter_map(|(_, position)| self.machines.get(*position))
                .find(|plan| plan.machine == machine)
        }

        pub fn machines(&self) -> &[MachineContractPlan] {
            &self.machines
        }

        pub fn machines_mut(&mut self) -> &mut Vec<MachineContractPlan> {
            self.indexed = false;
            &mut self.machines
        }

        pub fn machines_and_envelopes_mut(
            &mut self,
        ) -> (
            &[MachineContractPlan],
            &mut Vec<RealizedMachineContractEnvelope>,
        ) {
            (&self.machines, &mut self.realized_envelopes)
        }

        pub fn update_plans(&mut self, mut edit: impl FnMut(&mut MachineContractPlan)) {
            for plan in &mut self.machines {
                let machine = plan.machine;
                edit(plan);
                if plan.machine != machine {
                    self.indexed = false;
                }
            }
        }

        pub fn update_plan(
            &mut self,
            position: usize,
            edit: impl FnOnce(&mut MachineContractPlan),
        ) {
            let plan = &mut self.machines[position];
            let machine = plan.machine;
            edit(plan);
            if plan.machine != machine {
                self.indexed = false;
            }
        }

        pub fn index_machines(&mut self) {
            let mut index = self
                .machines
                .iter()
                .enumerate()
                .map(|(position, plan)| {
                    (
                        (plan.machine.arena_index(), plan.machine.generation()),
                        position,
                    )
                })
                .collect::<Vec<_>>();
            index.sort_unstable();
            self.machine_index = index;
            self.indexed = true;
        }

        pub fn new(
            machines: Vec<MachineContractPlan>,
            crash_capsules: Vec<CrashContractCapsule>,
            realized_envelopes: Vec<RealizedMachineContractEnvelope>,
        ) -> Self {
            let mut plans = Self {
                machines,
                crash_capsules,
                realized_envelopes,
                machine_index: Vec::new(),
                indexed: false,
            };
            plans.index_machines();
            plans
        }

        pub fn crash_capsule(
            &self,
            target_machine: SymbolHandle,
            target_state: SymbolHandle,
        ) -> Option<&CrashContractCapsule> {
            self.crash_capsules.iter().find(|capsule| {
                capsule.target_machine == target_machine && capsule.target_state == target_state
            })
        }

        pub fn realized_envelope(
            &self,
            machine: SymbolHandle,
        ) -> Option<&RealizedMachineContractEnvelope> {
            self.realized_envelopes
                .iter()
                .find(|envelope| envelope.machine == machine)
        }

        pub fn resource_envelope(
            &self,
            machine: SymbolHandle,
            entry: SymbolHandle,
        ) -> Option<&CheckedEntryResourceEnvelope> {
            self.realized_envelope(machine)
                .and_then(|envelope| envelope.resources.get(entry))
        }

        pub fn validate_resource_envelopes(&self) -> Result<(), &'static str> {
            let mut realized_by_machine = symbols::SymbolKeyMap::default();
            for (index, realized) in self.realized_envelopes.iter().enumerate() {
                if realized_by_machine
                    .insert(realized.machine, index)
                    .is_some()
                {
                    return Err("checked resource envelope has duplicate realized machine rows");
                }
            }
            for contract in &self.machines {
                if contract.report_fingerprint == 0 {
                    return Err(
                        "checked resource envelope requires a nonzero contract report coordinate",
                    );
                }
                if contract.commitment.is_zero() {
                    return Err(
                        "checked resource envelope requires a nonzero machine contract commitment",
                    );
                }
                let Some(index) = realized_by_machine.get(&contract.machine).copied() else {
                    return Err(
                        "checked resource envelope is missing its exact realized machine row",
                    );
                };
                let realized = &self.realized_envelopes[index];
                if realized.contract_report_fingerprint != contract.report_fingerprint
                    || realized.contract_commitment != contract.commitment
                {
                    return Err(
                        "checked resource envelope contract identity disagrees with its realized machine row",
                    );
                }
                realized.resources.validate()?;
                if realized.resources.machine != contract.machine
                    || realized.resources.contract_report_fingerprint != contract.report_fingerprint
                    || realized.resources.contract_commitment != contract.commitment
                {
                    return Err(
                        "checked resource roster does not bind its exact realized machine contract",
                    );
                }
            }
            if self.realized_envelopes.iter().any(|realized| {
                !self
                    .machines
                    .iter()
                    .any(|contract| contract.machine == realized.machine)
            }) {
                return Err("checked resource envelope retained an unknown realized machine row");
            }
            Ok(())
        }
    }

    pub fn contract_plan_by_machine(
        plans: &[MachineContractPlan],
        machine: SymbolHandle,
    ) -> Option<&MachineContractPlan> {
        plans.iter().find(|plan| plan.machine == machine)
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineContractPlan {
        pub machine: SymbolHandle,
        pub closed_scalar_values: ClosedScalarValueContractPlan,
        pub crash: CrashPlan,
        pub restored_declaration_predicates: Vec<crate::CheckedBooleanExpression>,
        pub report_fingerprint: u64,
        pub commitment: MachineContractCommitment,
    }

    typed_trees::digest_commitment! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        MachineContractCommitment
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct MachineContractIdentity {
        pub report_fingerprint: u64,
        pub commitment: MachineContractCommitment,
    }
}

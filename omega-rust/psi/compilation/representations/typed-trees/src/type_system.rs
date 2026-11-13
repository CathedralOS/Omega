/*
We define the typed type-reference vocabulary (`type_system::types`), closed
numeric types (`closed_numeric`), and proven case-membership type predicates
(`type_predicates`) in `TypedTrees`.
*/

pub mod closed_numeric {
    use crate::{
        TypedTrees,
        expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
        types::{PrimitiveType, TypeReferenceHandle},
    };
    use numerics::{
        arithmetic::ArithmeticDomain,
        bignum::{BigInt, BigRational},
        literals::LandedIntegerType,
    };
    use semantic_vocabulary::{IntegerType, IntegerValue};

    pub struct AnonymousNumericValue {
        pub value: BigRational,
        pub fractional_origin: ExpressionHandle,
    }

    struct NumericValue {
        value: BigRational,
        primitive: Option<PrimitiveType>,
        type_reference: Option<TypeReferenceHandle>,
        fractional_origin: ExpressionHandle,
    }

    pub struct ClosedIntegerValue {
        pub value: BigInt,
        pub primitive: Option<PrimitiveType>,
        pub type_reference: Option<TypeReferenceHandle>,
    }

    impl NumericValue {
        fn anonymous(value: BigRational) -> Self {
            Self {
                value,
                primitive: None,
                type_reference: None,
                fractional_origin: ExpressionHandle::invalid(),
            }
        }
    }

    pub fn evaluate_anonymous_value<const ALLOW_DECIMAL_LITERALS: bool>(
        program: &TypedTrees,
        expression: ExpressionHandle,
        selected_arms: &[(ExpressionHandle, ExpressionHandle)],
        builtin: &mut impl FnMut(ExpressionHandle) -> bool,
    ) -> Option<AnonymousNumericValue> {
        let value = evaluate_value::<ALLOW_DECIMAL_LITERALS>(
            program,
            expression,
            selected_arms,
            None,
            builtin,
            &mut |_| None,
        )?;
        Some(AnonymousNumericValue {
            value: value.value,
            fractional_origin: value.fractional_origin,
        })
    }

    pub fn has_anonymous_numeric_results(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        fn visit(
            program: &TypedTrees,
            expression: ExpressionHandle,
            active: &mut Vec<ExpressionHandle>,
        ) -> bool {
            if !program.expression_table.expression_is_valid(expression)
                || active.contains(&expression)
            {
                return false;
            }
            if let Some(dispatch) = program.expression_table.match_node(expression) {
                active.push(expression);
                let anonymous = !program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .is_empty()
                    && program
                        .expression_table
                        .match_arms_all(dispatch.arms, |_, value| visit(program, value, active));
                active.pop();
                anonymous
            } else {
                evaluate_anonymous_value::<true>(program, expression, &[], &mut |expression| {
                    has_anonymous_operator_meaning(program, expression)
                })
                .is_some()
            }
        }
        visit(program, expression, &mut Vec::new())
    }

    impl TypedTrees {
        pub fn closed_integer_expression_value(
            &self,
            expression: ExpressionHandle,
        ) -> Option<BigInt> {
            self.closed_integer_expression_value_in(expression, symbols::SymbolHandle::invalid())
        }

        pub fn closed_integer_range_endpoint(
            &self,
            expression: ExpressionHandle,
            end_inclusive: bool,
        ) -> Option<BigInt> {
            let value = self.closed_integer_expression_value(expression)?;
            Some(canonical_integer_range_endpoint(value, end_inclusive))
        }

        pub fn closed_integer_expression_value_in(
            &self,
            expression: ExpressionHandle,
            machine: symbols::SymbolHandle,
        ) -> Option<BigInt> {
            let recipe = self.constant_recipe_for_contract_expression(expression);
            if let Some(value) = self.closed_constant_recipe_value(recipe) {
                return Some(value.clone());
            }
            Some(self.closed_integer_value_in(expression, machine)?.value)
        }

        pub fn closed_integer_value_in(
            &self,
            expression: ExpressionHandle,
            machine: symbols::SymbolHandle,
        ) -> Option<ClosedIntegerValue> {
            let value = evaluate_value::<true>(
                self,
                expression,
                &[],
                Some(machine),
                &mut |expression| has_anonymous_operator_meaning(self, expression),
                &mut |_| None,
            )?;
            Some(ClosedIntegerValue {
                value: value.value.to_integer_exact()?,
                primitive: value.primitive,
                type_reference: value.type_reference,
            })
        }

        pub fn integer_expression_value_with_bindings(
            &self,
            expression: ExpressionHandle,
            machine: symbols::SymbolHandle,
            binding: &mut impl FnMut(ExpressionHandle) -> Option<(BigInt, TypeReferenceHandle)>,
        ) -> Option<BigInt> {
            evaluate_value::<true>(
                self,
                expression,
                &[],
                Some(machine),
                &mut |expression| has_anonymous_operator_meaning(self, expression),
                binding,
            )?
            .value
            .to_integer_exact()
        }
    }

    pub fn canonical_integer_range_endpoint(value: BigInt, end_inclusive: bool) -> BigInt {
        if end_inclusive {
            value
        } else {
            value.sub(&BigInt::from_u64(1))
        }
    }

    fn evaluate_value<const ALLOW_DECIMAL_LITERALS: bool>(
        program: &TypedTrees,
        expression: ExpressionHandle,
        selected_arms: &[(ExpressionHandle, ExpressionHandle)],
        typed_owner: Option<symbols::SymbolHandle>,
        builtin: &mut impl FnMut(ExpressionHandle) -> bool,
        binding: &mut impl FnMut(ExpressionHandle) -> Option<(BigInt, TypeReferenceHandle)>,
    ) -> Option<NumericValue> {
        enum Step {
            Enter(ExpressionHandle),
            Leave(ExpressionHandle),
            Binary(ExpressionHandle, BinaryOperator),
            Cast(TypeReferenceHandle),
        }
        let mut pending = vec![Step::Enter(expression)];
        let mut active = Vec::new();
        let mut values: Vec<NumericValue> = Vec::new();
        let mut fractional_origin = ExpressionHandle::invalid();
        while let Some(step) = pending.pop() {
            match step {
                Step::Enter(expression) => {
                    if !program.expression_table.expression_is_valid(expression)
                        || active.contains(&expression)
                    {
                        return None;
                    }
                    if let Some((value, type_reference)) = binding(expression) {
                        typed_owner?;
                        let primitive = program
                            .type_reference_table
                            .primitive_type(type_reference)?;
                        land_integer(&value, primitive)?;
                        values.push(NumericValue {
                            value: BigRational::from_integer(value),
                            primitive: Some(primitive),
                            type_reference: Some(type_reference),
                            fractional_origin: ExpressionHandle::invalid(),
                        });
                        continue;
                    }
                    match program.expression_table.expression(expression) {
                        ExpressionNode::Match(dispatch) => {
                            let mut selections = selected_arms
                                .iter()
                                .filter(|(owner, _)| *owner == expression);
                            let (_, selected) = selections.next()?;
                            if selections.any(|(_, other)| other != selected) {
                                return None;
                            }
                            let arms = program.expression_table.match_arms(dispatch.arms);
                            if arms.len() != dispatch.arms.len()
                                || !arms.iter().any(|arm| arm.value == *selected)
                            {
                                return None;
                            }
                            active.push(expression);
                            pending.push(Step::Leave(expression));
                            pending.push(Step::Enter(*selected));
                        }
                        ExpressionNode::Integer(literal) if literal.landing().is_none() => values
                            .push(NumericValue::anonymous(BigRational::from_integer(
                                literal.value_bignum()?,
                            ))),
                        ExpressionNode::Float(literal)
                            if ALLOW_DECIMAL_LITERALS && literal.landing().is_none() =>
                        {
                            let value = BigRational::from_decimal_str(literal.text())?;
                            if !fractional_origin.is_valid() && value.to_integer_exact().is_none() {
                                fractional_origin = expression;
                            }
                            values.push(NumericValue::anonymous(value));
                        }
                        ExpressionNode::Integer(literal) if typed_owner.is_some() => {
                            let landing = literal.landing()?;
                            let (primitive, atom) = literal_carrier(landing.landed_type)?;
                            let symbol = program
                                .symbols
                                .child_handles(program.symbols.root())?
                                .find(|symbol| {
                                    program.symbols.builtin_type_atom(*symbol) == Some(atom)
                                })?;
                            let type_reference = program
                                .type_reference_table
                                .find_arithmetic_result_type_reference(symbol, landing.domain)?;
                            let value = literal.value_bignum()?;
                            land_integer(&value, primitive)?;
                            values.push(NumericValue {
                                value: BigRational::from_integer(value),
                                primitive: Some(primitive),
                                type_reference: Some(type_reference),
                                fractional_origin: ExpressionHandle::invalid(),
                            });
                        }
                        ExpressionNode::Cast(cast)
                            if typed_owner.is_some() && !cast.form.is_recast() =>
                        {
                            if !program.contains_type_reference(cast.result_type)
                                || !program
                                    .type_reference_table
                                    .primitive_type(cast.result_type)
                                    .is_some_and(PrimitiveType::is_fixed_integer)
                            {
                                return None;
                            }
                            active.push(expression);
                            pending.push(Step::Leave(expression));
                            pending.push(Step::Cast(cast.result_type));
                            pending.push(Step::Enter(cast.value));
                        }
                        ExpressionNode::Name(path) if typed_owner.is_some() => {
                            let declaration = program.const_declaration_by_symbol(path.symbol)?;
                            active.push(expression);
                            pending.push(Step::Leave(expression));
                            pending.push(Step::Cast(declaration.declared_type));
                            pending.push(Step::Enter(declaration.materialized_initializer));
                        }
                        ExpressionNode::Binary(binary)
                            if typed_owner.is_some() || builtin(expression) =>
                        {
                            active.push(expression);
                            pending.push(Step::Leave(expression));
                            pending.push(Step::Binary(expression, binary.operator));
                            pending.push(Step::Enter(binary.right));
                            pending.push(Step::Enter(binary.left));
                        }
                        _ => return None,
                    }
                }
                Step::Leave(expression) => {
                    if active.pop() != Some(expression) {
                        return None;
                    }
                }
                Step::Cast(reference) => {
                    let source = values.pop()?;
                    let value = source.value.to_integer_exact()?;
                    let primitive = program.type_reference_table.primitive_type(reference)?;
                    land_integer(&value, primitive)?;
                    values.push(NumericValue {
                        value: BigRational::from_integer(value),
                        primitive: Some(primitive),
                        type_reference: Some(reference),
                        fractional_origin: source.fractional_origin,
                    });
                }
                Step::Binary(expression, operator) => {
                    let right = values.pop()?;
                    let left = values.pop()?;
                    let mut value = apply_numeric(
                        program,
                        typed_owner,
                        expression,
                        operator,
                        left,
                        right,
                        builtin,
                    )?;
                    if !fractional_origin.is_valid() && value.value.to_integer_exact().is_none() {
                        fractional_origin = expression;
                    }
                    value.fractional_origin = fractional_origin;
                    values.push(value);
                }
            }
        }
        if values.len() != 1 {
            return None;
        }
        let mut value = values.pop()?;
        value.fractional_origin = fractional_origin;
        Some(value)
    }

    fn apply_numeric(
        program: &TypedTrees,
        typed_owner: Option<symbols::SymbolHandle>,
        expression: ExpressionHandle,
        operator: BinaryOperator,
        left: NumericValue,
        right: NumericValue,
        builtin: &mut impl FnMut(ExpressionHandle) -> bool,
    ) -> Option<NumericValue> {
        if left.primitive.is_none() && right.primitive.is_none() {
            if typed_owner.is_some() && !builtin(expression) {
                return None;
            }
            return Some(NumericValue::anonymous(match operator {
                BinaryOperator::Add => left.value.add(&right.value),
                BinaryOperator::Subtract => left.value.sub(&right.value),
                BinaryOperator::Multiply => left.value.mul(&right.value),
                BinaryOperator::Divide => left.value.div(&right.value)?,
                _ => return None,
            }));
        }
        let machine = typed_owner?;
        let spelling = match operator {
            BinaryOperator::Add => language_core::OperatorSpelling::Add,
            BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
            BinaryOperator::Multiply => language_core::OperatorSpelling::Multiply,
            BinaryOperator::Divide => language_core::OperatorSpelling::Divide,
            BinaryOperator::Modulo => language_core::OperatorSpelling::Modulo,
            _ => return None,
        };
        let references = [left.type_reference, right.type_reference];
        if !machine.is_valid()
            && program
                .machine_specializations
                .iter()
                .any(|specialization| {
                    !crate::operator::selected_trait_operator_meanings(
                        program,
                        specialization.instance,
                        spelling,
                        &references,
                    )
                    .is_empty()
                })
        {
            return None;
        }
        if !crate::operator::has_builtin_spelled_expression_meaning(
            program,
            machine,
            expression,
            spelling,
            &references,
        ) || left
            .primitive
            .zip(right.primitive)
            .is_some_and(|(left, right)| left != right)
        {
            return None;
        }
        let primitive = left.primitive.or(right.primitive)?;
        let left_policy = left
            .type_reference
            .map(|reference| program.type_reference_table.arithmetic_domain(reference));
        let right_policy = right
            .type_reference
            .map(|reference| program.type_reference_table.arithmetic_domain(reference));
        if left_policy
            .zip(right_policy)
            .is_some_and(|(left, right)| left != right)
        {
            return None;
        }
        let policy = left_policy.or(right_policy)?;
        let integer = integer_carrier(primitive)?;
        let left_value = land_integer(&left.value.to_integer_exact()?, primitive)?;
        let right_value = land_integer(&right.value.to_integer_exact()?, primitive)?;
        let result = match (policy, operator) {
            (ArithmeticDomain::Exact | ArithmeticDomain::Trapping, BinaryOperator::Add) => {
                integer.exact_add(left_value, right_value)
            }
            (ArithmeticDomain::Exact | ArithmeticDomain::Trapping, BinaryOperator::Subtract) => {
                integer.exact_sub(left_value, right_value)
            }
            (ArithmeticDomain::Exact | ArithmeticDomain::Trapping, BinaryOperator::Multiply) => {
                integer.exact_mul(left_value, right_value)
            }
            (ArithmeticDomain::Exact | ArithmeticDomain::Trapping, BinaryOperator::Divide) => {
                integer.exact_div(left_value, right_value)
            }
            (ArithmeticDomain::Exact | ArithmeticDomain::Trapping, BinaryOperator::Modulo) => {
                integer.exact_rem(left_value, right_value)
            }
            (ArithmeticDomain::Wrapping, BinaryOperator::Add) => {
                integer.wrapping_add(left_value, right_value)
            }
            (ArithmeticDomain::Wrapping, BinaryOperator::Subtract) => {
                integer.wrapping_sub(left_value, right_value)
            }
            (ArithmeticDomain::Wrapping, BinaryOperator::Multiply) => {
                integer.wrapping_mul(left_value, right_value)
            }
            (ArithmeticDomain::Wrapping, BinaryOperator::Divide) => {
                integer.wrapping_div(left_value, right_value)
            }
            (ArithmeticDomain::Wrapping, BinaryOperator::Modulo) => {
                integer.wrapping_rem(left_value, right_value)
            }
            (ArithmeticDomain::Saturating, BinaryOperator::Add) => {
                integer.saturating_add(left_value, right_value)
            }
            (ArithmeticDomain::Saturating, BinaryOperator::Subtract) => {
                integer.saturating_sub(left_value, right_value)
            }
            (ArithmeticDomain::Saturating, BinaryOperator::Multiply) => {
                integer.saturating_mul(left_value, right_value)
            }
            (ArithmeticDomain::Saturating, BinaryOperator::Divide) => {
                integer.saturating_div(left_value, right_value)
            }
            (ArithmeticDomain::Saturating, BinaryOperator::Modulo) => {
                integer.saturating_rem(left_value, right_value)
            }
            _ => return None,
        }?;
        let result = match result {
            IntegerValue::Signed(value) => BigInt::from_i128(value),
            IntegerValue::Unsigned(value) => BigInt::from_u128(value),
        };
        Some(NumericValue {
            value: BigRational::from_integer(result),
            primitive: Some(primitive),
            type_reference: left.type_reference.or(right.type_reference),
            fractional_origin: ExpressionHandle::invalid(),
        })
    }

    pub fn land_integer(value: &BigInt, primitive: PrimitiveType) -> Option<IntegerValue> {
        let value = if primitive.is_signed_integer() {
            IntegerValue::Signed(i128::from(value.to_i64()?))
        } else {
            IntegerValue::Unsigned(u128::from(value.to_u64()?))
        };
        integer_carrier(primitive)?.admits(value).then_some(value)
    }

    fn integer_carrier(primitive: PrimitiveType) -> Option<IntegerType> {
        IntegerType::from_fixed(primitive.fixed_integer()?)
    }

    fn literal_carrier(
        landing: LandedIntegerType,
    ) -> Option<(PrimitiveType, symbols::BuiltinTypeAtom)> {
        match landing {
            LandedIntegerType::Fixed(carrier) => Some((
                PrimitiveType::Integer(carrier),
                symbols::BuiltinTypeAtom::Integer(carrier),
            )),
            LandedIntegerType::Addr => None,
        }
    }
    pub fn has_anonymous_operator_meaning(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        use language_core::OperatorSpelling;
        let spelling = match binary.operator {
            BinaryOperator::Add => OperatorSpelling::Add,
            BinaryOperator::Subtract => OperatorSpelling::Subtract,
            BinaryOperator::Multiply => OperatorSpelling::Multiply,
            BinaryOperator::Divide => OperatorSpelling::Divide,
            _ => return false,
        };
        has_builtin_anonymous_operands(program, expression, spelling)
    }

    pub fn has_builtin_anonymous_operands(
        program: &TypedTrees,
        expression: ExpressionHandle,
        _spelling: language_core::OperatorSpelling,
    ) -> bool {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionIntrinsic as Intrinsic,
            AuthoredDeclarationSelectionKind as Kind,
            AuthoredDeclarationSelectionLateBinding as LateBinding,
            AuthoredDeclarationSelectionTarget as Target,
        };
        program
            .expression_table
            .authored_selection_occurrences(expression)
            .all(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| match selection.target() {
                        Target::Resolved(resolved) if selection.kind() == Kind::Call => matches!(
                            program.symbols.get(resolved.selected_symbol()).kind,
                            symbols::SymbolKind::Machine
                                | symbols::SymbolKind::State
                                | symbols::SymbolKind::Operator
                        ),
                        Target::Resolved(resolved)
                            if selection.kind() == Kind::DomainMembership =>
                        {
                            program.symbols.get(resolved.selected_symbol()).kind
                                == symbols::SymbolKind::Domain
                        }
                        Target::Intrinsic(Intrinsic::BuiltinOperator)
                        | Target::LateBound(LateBinding::CheckedOperator) => true,
                        Target::Resolved(resolved) => {
                            program.symbols.get(resolved.selected_symbol()).kind
                                == symbols::SymbolKind::Const
                        }
                        _ => false,
                    })
            })
    }
}
pub use crate::type_identity;
pub(crate) mod type_predicates {
    use crate::domain::index_argument_is_closed;
    use crate::domain::index_argument_is_closed_with_substitutions;
    use crate::expression::UnaryOperator;
    use crate::{
        TypedTrees,
        expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
        types::TypeReferenceHandle,
    };

    impl TypedTrees {
        pub fn static_type_predicate_operands(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
            let ExpressionNode::Binary(binary) = self.expression_table.expression(expression)
            else {
                return None;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                return None;
            }
            let ExpressionNode::TypeExpression(left) =
                self.expression_table.expression(binary.left)
            else {
                return None;
            };
            let ExpressionNode::TypeExpression(right) =
                self.expression_table.expression(binary.right)
            else {
                return None;
            };
            Some((*left, *right, binary.operator == BinaryOperator::NotEqual))
        }

        pub fn applied_static_type_predicate_value(
            &self,
            expression: ExpressionHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> Option<bool> {
            if let Some((left, right, negate)) = self.static_type_predicate_operands(expression) {
                if !index_argument_is_closed_with_substitutions(self, left, substitutions)
                    || !index_argument_is_closed_with_substitutions(self, right, substitutions)
                {
                    return None;
                }
                let identity = |reference| {
                    self.type_identity(crate::type_identity::TypeIdentityRequest {
                        substitutions,
                        ..crate::type_identity::TypeIdentityRequest::ordinary(reference)
                    })
                };
                return Some((identity(left) == identity(right)) != negate);
            }
            match self.expression_table.expression(expression) {
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
                {
                    let left =
                        self.applied_static_type_predicate_value(binary.left, substitutions)?;
                    let right =
                        self.applied_static_type_predicate_value(binary.right, substitutions)?;
                    Some(if binary.operator == BinaryOperator::And {
                        left && right
                    } else {
                        left || right
                    })
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => self
                    .applied_static_type_predicate_value(unary.operand, substitutions)
                    .map(|value| !value),
                _ => None,
            }
        }

        pub fn static_type_predicate_value(&self, expression: ExpressionHandle) -> Option<bool> {
            self.static_type_predicate_value_in(expression, expression)
        }

        pub fn static_type_predicate_value_in(
            &self,
            expression: ExpressionHandle,
            context: ExpressionHandle,
        ) -> Option<bool> {
            let (left, right, negate) = self.static_type_predicate_operands(expression)?;
            let left = self.statically_refined_type_reference(context, left);
            let right = self.statically_refined_type_reference(context, right);
            if !self.type_reference_table.contains_type_reference(left)
                || !self.type_reference_table.contains_type_reference(right)
                || !index_argument_is_closed(self, left)
                || !index_argument_is_closed(self, right)
            {
                return None;
            }
            Some(
                (self.normalized_type_identity(left) == self.normalized_type_identity(right))
                    != negate,
            )
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct CaseMembershipTypeFacts {
        pub membership: ExpressionHandle,
        pub impossible: bool,
        pub equations: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct StaticTypeContexts {
        boolean_equalities: Vec<ExpressionHandle>,
        completed_case_values: Vec<(ExpressionHandle, bool)>,
        admitted_match_cases: Vec<(ExpressionHandle, Vec<symbols::SymbolHandle>)>,
        case_memberships: Vec<CaseMembershipTypeFacts>,
        rows: Vec<(ExpressionHandle, Vec<TypeWorld>)>,
        statement_rows: Vec<(crate::statement::StatementHandle, Vec<TypeWorld>)>,
        transition_rows: Vec<(crate::statement::TransitionTargetHandle, Vec<TypeWorld>)>,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    struct TypeWorld {
        bindings: Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
        exclusions: Vec<(symbols::SymbolHandle, TypeReferenceHandle)>,
    }

    impl StaticTypeContexts {
        pub fn refined_reference(
            &self,
            program: &TypedTrees,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let Ok(index) = self.rows.binary_search_by_key(
                &(expression.arena_index(), expression.generation()),
                |(owned, _)| (owned.arena_index(), owned.generation()),
            ) else {
                return reference;
            };
            Self::refined_in_worlds(program, reference, &self.rows[index].1)
        }

        fn refined_in_worlds(
            program: &TypedTrees,
            reference: TypeReferenceHandle,
            worlds: &[TypeWorld],
        ) -> TypeReferenceHandle {
            let mut result = None;
            for world in worlds {
                let current = world.resolve(program, reference);
                if let Some(previous) = result {
                    if program.normalized_type_identity(previous)
                        != program.normalized_type_identity(current)
                    {
                        return reference;
                    }
                } else {
                    result = Some(current);
                }
            }
            result.unwrap_or(reference)
        }

        pub fn is_unreachable(&self, expression: ExpressionHandle) -> bool {
            self.rows
                .binary_search_by_key(
                    &(expression.arena_index(), expression.generation()),
                    |(owned, _)| (owned.arena_index(), owned.generation()),
                )
                .is_ok_and(|index| self.rows[index].1.is_empty())
        }

        pub fn agree(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
            let default = [TypeWorld::default()];
            let worlds = |expression: ExpressionHandle| {
                self.rows
                    .binary_search_by_key(
                        &(expression.arena_index(), expression.generation()),
                        |(owned, _)| (owned.arena_index(), owned.generation()),
                    )
                    .map_or(default.as_slice(), |index| self.rows[index].1.as_slice())
            };
            worlds(left) == worlds(right)
        }

        fn case_membership(
            &self,
            expression: ExpressionHandle,
        ) -> Option<&CaseMembershipTypeFacts> {
            self.case_memberships
                .binary_search_by_key(
                    &(expression.arena_index(), expression.generation()),
                    |facts| {
                        (
                            facts.membership.arena_index(),
                            facts.membership.generation(),
                        )
                    },
                )
                .ok()
                .map(|index| &self.case_memberships[index])
        }
    }

    #[derive(Default)]
    struct StaticContextBuilder {
        rows:
            std::collections::HashMap<ExpressionHandle, Vec<TypeWorld>, symbols::BuildSymbolHasher>,
        statement_rows: std::collections::HashMap<
            crate::statement::StatementHandle,
            Vec<TypeWorld>,
            symbols::BuildSymbolHasher,
        >,
        transition_rows: std::collections::HashMap<
            crate::statement::TransitionTargetHandle,
            Vec<TypeWorld>,
            symbols::BuildSymbolHasher,
        >,
    }
    impl StaticContextBuilder {
        fn record(&mut self, expression: ExpressionHandle, worlds: &[TypeWorld]) {
            let previous = self.rows.entry(expression).or_default();
            for world in worlds {
                if !previous.contains(world) {
                    previous.push(world.clone());
                }
            }
        }
    }

    impl TypeWorld {
        fn resolve(
            &self,
            program: &TypedTrees,
            mut reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let mut seen = Vec::new();
            while !seen.contains(&reference) && program.contains_type_reference(reference) {
                seen.push(reference);
                let crate::types::TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    break;
                };
                let Some((_, replacement)) = self
                    .bindings
                    .iter()
                    .rev()
                    .find(|(binder, _)| binder == symbol)
                else {
                    break;
                };
                reference = *replacement;
            }
            reference
        }

        fn constrain(
            mut self,
            program: &TypedTrees,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
            equal: bool,
        ) -> Option<Self> {
            let left = self.resolve(program, left);
            let right = self.resolve(program, right);
            if program.normalized_type_identity(left) == program.normalized_type_identity(right) {
                return equal.then_some(self);
            }
            let binder = |reference| match program.type_reference_table.type_reference(reference) {
                crate::types::TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter =>
                {
                    Some(*symbol)
                }
                _ => None,
            };
            let (symbol, value) = if let Some(symbol) = binder(left) {
                (symbol, right)
            } else if let Some(symbol) = binder(right) {
                (symbol, left)
            } else {
                if index_argument_is_closed(program, left)
                    && index_argument_is_closed(program, right)
                {
                    return (!equal).then_some(self);
                }
                return Some(self);
            };
            if equal {
                if self.exclusions.iter().any(|(excluded, reference)| {
                    *excluded == symbol
                        && program.normalized_type_identity(*reference)
                            == program.normalized_type_identity(value)
                }) {
                    return None;
                }
                self.bindings.push((symbol, value));
            } else {
                self.exclusions.push((symbol, value));
            }
            Some(self)
        }
    }

    fn condition(
        program: &TypedTrees,
        worlds: &[TypeWorld],
        expression: ExpressionHandle,
        holds: bool,
    ) -> Vec<TypeWorld> {
        if !contains_type_predicate(program, expression, &mut Vec::new()) {
            return worlds.to_vec();
        }
        condition_inner(program, worlds, expression, holds)
    }

    fn contains_type_predicate(
        program: &TypedTrees,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> bool {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return false;
        }
        if program.static_type_predicate_operands(expression).is_some()
            || program
                .static_type_contexts
                .case_membership(expression)
                .is_some()
        {
            return true;
        }
        active.push(expression);
        let result = match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    || program
                        .static_type_contexts
                        .boolean_equalities
                        .contains(&expression) =>
            {
                contains_type_predicate(program, binary.left, active)
                    || contains_type_predicate(program, binary.right, active)
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                contains_type_predicate(program, unary.operand, active)
            }
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
            {
                let state_symbol = program.symbols.get(path.symbol).parent;
                let machine_symbol = program.symbols.get(state_symbol).parent;
                program
                    .machine_by_symbol(machine_symbol)
                    .and_then(|machine| program.state_in_machine(machine, state_symbol))
                    .and_then(|state| program.state_local_declaration(state, path.symbol))
                    .filter(|(_, local)| !local.is_mutable)
                    .and_then(|(_, local)| local.ordinary_value_initializer())
                    .is_some_and(|initializer| {
                        contains_type_predicate(program, initializer, active)
                    })
            }
            _ => false,
        };
        active.pop();
        result
    }

    fn condition_inner(
        program: &TypedTrees,
        worlds: &[TypeWorld],
        expression: ExpressionHandle,
        holds: bool,
    ) -> Vec<TypeWorld> {
        if !program.expression_table.expression_is_valid(expression) {
            return worlds.to_vec();
        }
        if let Some((left, right, negate)) = program.static_type_predicate_operands(expression) {
            return worlds
                .iter()
                .cloned()
                .filter_map(|world| world.constrain(program, left, right, holds != negate))
                .collect();
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && program.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
            {
                let state_symbol = program.symbols.get(path.symbol).parent;
                let machine_symbol = program.symbols.get(state_symbol).parent;
                if let Some(machine) = program.machine_by_symbol(machine_symbol)
                    && let Some(state) = program.state_in_machine(machine, state_symbol)
                    && let Some((_, local)) = program.state_local_declaration(state, path.symbol)
                    && !local.is_mutable
                    && let Some(initializer) = local.ordinary_value_initializer()
                {
                    condition_inner(program, worlds, initializer, holds)
                } else {
                    worlds.to_vec()
                }
            }
            ExpressionNode::Binary(_)
                if program
                    .static_type_contexts
                    .case_membership(expression)
                    .is_some() =>
            {
                let facts = program.static_type_contexts.case_membership(expression);
                match facts {
                    Some(facts) if holds && facts.impossible => Vec::new(),
                    Some(facts) if holds => worlds
                        .iter()
                        .cloned()
                        .filter_map(|world| {
                            facts
                                .equations
                                .iter()
                                .try_fold(world, |world, (left, right)| {
                                    world.constrain(program, *left, *right, true)
                                })
                        })
                        .collect(),
                    _ => worlds.to_vec(),
                }
            }
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::CaseMembership
                    || program
                        .static_type_contexts
                        .boolean_equalities
                        .binary_search_by_key(
                            &(expression.arena_index(), expression.generation()),
                            |handle| (handle.arena_index(), handle.generation()),
                        )
                        .is_ok() =>
            {
                let wrapped = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                    (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                    _ => None,
                };
                if let Some((subject, value)) = wrapped {
                    condition_inner(
                        program,
                        worlds,
                        subject,
                        holds == (value != (binary.operator == BinaryOperator::NotEqual)),
                    )
                } else {
                    worlds.to_vec()
                }
            }
            ExpressionNode::Boolean(value) => {
                if *value == holds {
                    worlds.to_vec()
                } else {
                    Vec::new()
                }
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                condition_inner(program, worlds, unary.operand, !holds)
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                let conjunction = binary.operator == BinaryOperator::And;
                if holds == conjunction {
                    condition_inner(
                        program,
                        &condition_inner(program, worlds, binary.left, holds),
                        binary.right,
                        holds,
                    )
                } else {
                    let mut alternatives = condition_inner(program, worlds, binary.left, holds);
                    alternatives.extend(condition_inner(
                        program,
                        &condition_inner(program, worlds, binary.left, !holds),
                        binary.right,
                        holds,
                    ));
                    alternatives
                }
            }
            _ => worlds.to_vec(),
        }
    }

    fn visit_context(
        program: &TypedTrees,
        output: &mut StaticContextBuilder,
        expression: ExpressionHandle,
        worlds: &[TypeWorld],
        active: &mut Vec<ExpressionHandle>,
    ) {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return;
        }
        output.record(expression, worlds);
        active.push(expression);
        let mut visit = |child, environment: &[TypeWorld]| {
            visit_context(program, output, child, environment, active)
        };
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                visit(binary.left, worlds);
                let right = if binary.operator == BinaryOperator::And {
                    condition(program, worlds, binary.left, true)
                } else if binary.operator == BinaryOperator::Or {
                    condition(program, worlds, binary.left, false)
                } else {
                    worlds.to_vec()
                };
                visit(binary.right, &right);
            }
            ExpressionNode::Match(dispatch) => {
                visit(dispatch.subject, worlds);
                let mut remaining = worlds.to_vec();
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    let environment = match arm.pattern {
                        crate::expression::MatchPattern::Value(pattern) => {
                            visit(pattern, &remaining);
                            if let ExpressionNode::Boolean(value) =
                                program.expression_table.expression(pattern)
                            {
                                let environment =
                                    condition(program, &remaining, dispatch.subject, *value);
                                remaining =
                                    condition(program, &remaining, dispatch.subject, !*value);
                                environment
                            } else {
                                remaining.clone()
                            }
                        }
                        crate::expression::MatchPattern::Destructure(pattern) => {
                            condition(program, &remaining, pattern.condition, true)
                        }
                        crate::expression::MatchPattern::Wildcard => remaining.clone(),
                    };
                    visit(arm.value, &environment);
                }
            }
            ExpressionNode::Call(call) => {
                visit(call.receiver, worlds);
                for &argument in program.expression_table.expression_handles(call.arguments) {
                    visit(argument, worlds);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                visit(atomic.value, worlds);
                visit(atomic.result, worlds);
            }
            ExpressionNode::Unary(unary) => visit(unary.operand, worlds),
            ExpressionNode::Borrow(borrow) => visit(borrow.target, worlds),
            ExpressionNode::Cast(cast) => visit(cast.value, worlds),
            ExpressionNode::Member(member) => visit(member.receiver, worlds),
            ExpressionNode::Indexed(indexed) => {
                visit(indexed.collection, worlds);
                visit(indexed.index, worlds);
            }
            ExpressionNode::Range(range) => {
                visit(range.start, worlds);
                visit(range.end, worlds);
            }
            ExpressionNode::ArrayLiteral(values) => {
                for &value in program.expression_table.expression_handles(*values) {
                    visit(value, worlds);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    visit(field.value, worlds);
                }
            }
            ExpressionNode::SavedSubject(_)
            | ExpressionNode::TypeExpression(_)
            | crate::literal_expression_pattern!() => {}
        }
        active.pop();
    }

    impl TypedTrees {
        pub fn prepare_static_type_contexts(
            &mut self,
            boolean_equalities: &[ExpressionHandle],
            mut case_memberships: Vec<CaseMembershipTypeFacts>,
        ) {
            case_memberships.sort_unstable_by_key(|facts| {
                (
                    facts.membership.arena_index(),
                    facts.membership.generation(),
                )
            });
            case_memberships.dedup_by_key(|facts| facts.membership);
            self.static_type_contexts = StaticTypeContexts {
                boolean_equalities: boolean_equalities.to_vec(),
                case_memberships,
                ..Default::default()
            };
            use crate::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
            let mut contexts = StaticContextBuilder::default();
            for machine in self.machines() {
                let mut worlds = vec![TypeWorld::default()];
                for equation in self
                    .declared_type_equations
                    .span_or_empty(machine.declared_type_equations)
                {
                    worlds = worlds
                        .into_iter()
                        .filter_map(|world| {
                            world.constrain(self, equation.parameter, equation.definition, true)
                        })
                        .collect();
                }
                let mut active = Vec::new();
                for contract in self.machine_contracts(machine) {
                    for fact in self.proof_facts.span_or_empty(contract.facts) {
                        if let crate::domain::ProofFact::Expression(expression) = fact {
                            visit_context(self, &mut contexts, *expression, &worlds, &mut active);
                            if contract.kind == crate::signature::SignatureContractKind::Requires {
                                worlds = condition(self, &worlds, *expression, true);
                            }
                        }
                    }
                }
                for state in self.machine_states(machine) {
                    let mut remaining = worlds.clone();
                    for contract in self.state_contracts(state) {
                        for fact in self.proof_facts.span_or_empty(contract.facts) {
                            if let crate::domain::ProofFact::Expression(expression) = fact {
                                visit_context(
                                    self,
                                    &mut contexts,
                                    *expression,
                                    &remaining,
                                    &mut active,
                                );
                                if contract.kind
                                    == crate::signature::SignatureContractKind::Requires
                                {
                                    remaining = condition(self, &remaining, *expression, true);
                                }
                            }
                        }
                    }
                    for (handle, statement) in state
                        .statement_nodes
                        .handles()
                        .zip(self.statement_table.statements(state.statement_nodes))
                    {
                        let retained = contexts.statement_rows.entry(handle).or_default();
                        for world in &remaining {
                            if !retained.contains(world) {
                                retained.push(world.clone());
                            }
                        }
                        let mut visit = |expression, environment: &[TypeWorld]| {
                            visit_context(self, &mut contexts, expression, environment, &mut active)
                        };
                        match statement {
                            StatementNode::Transition(transition) => {
                                let selected = match transition.guard {
                                    TransitionGuardNode::Always => remaining.clone(),
                                    TransitionGuardNode::When(guard) => {
                                        visit(guard, &remaining);
                                        condition(self, &remaining, guard, true)
                                    }
                                };
                                let complement = match transition.guard {
                                    TransitionGuardNode::Always => Vec::new(),
                                    TransitionGuardNode::When(guard) => {
                                        condition(self, &remaining, guard, false)
                                    }
                                };
                                for target in transition.targets() {
                                    let target_worlds = if target == transition.target {
                                        &selected
                                    } else {
                                        &complement
                                    };
                                    let retained =
                                        contexts.transition_rows.entry(target).or_default();
                                    for world in target_worlds {
                                        if !retained.contains(world) {
                                            retained.push(world.clone());
                                        }
                                    }
                                    match self.statement_table.transition_target(target) {
                                        TransitionTargetNode::Value(value) => visit_context(
                                            self,
                                            &mut contexts,
                                            *value,
                                            target_worlds,
                                            &mut active,
                                        ),
                                        TransitionTargetNode::Named { arguments, .. } => {
                                            for &argument in
                                                self.statement_table.expression_handles(*arguments)
                                            {
                                                visit_context(
                                                    self,
                                                    &mut contexts,
                                                    argument,
                                                    target_worlds,
                                                    &mut active,
                                                );
                                            }
                                        }
                                        TransitionTargetNode::Terminal => {}
                                    }
                                }
                                remaining = match transition.guard {
                                    TransitionGuardNode::Always => remaining.clone(),
                                    TransitionGuardNode::When(guard) => {
                                        condition(self, &remaining, guard, false)
                                    }
                                };
                            }
                            StatementNode::Expression(expression) => visit(*expression, &remaining),
                            StatementNode::LocalData(local) => {
                                if let Some(value) = local.ordinary_value_initializer() {
                                    visit(value, &remaining);
                                }
                            }
                            StatementNode::Assignment(assignment) => {
                                visit(assignment.target, &remaining);
                                visit(assignment.value, &remaining);
                            }
                            StatementNode::Call(call) => {
                                for &argument in
                                    self.statement_table.expression_handles(call.arguments)
                                {
                                    visit(argument, &remaining);
                                }
                            }
                            StatementNode::AssemblyFact(fact) => {
                                if let crate::domain::ProofFact::Expression(expression) =
                                    self.proof_facts.get(fact.fact)
                                {
                                    visit(*expression, &remaining);
                                }
                            }
                            StatementNode::Assembly(_) => {}
                        }
                    }
                }
            }
            let mut rows = contexts.rows.into_iter().collect::<Vec<_>>();
            rows.sort_unstable_by_key(|(expression, _)| {
                (expression.arena_index(), expression.generation())
            });
            self.static_type_contexts.rows = rows;
            let mut statement_rows = contexts.statement_rows.into_iter().collect::<Vec<_>>();
            statement_rows.sort_unstable_by_key(|(statement, _)| {
                (statement.arena_index(), statement.generation())
            });
            self.static_type_contexts.statement_rows = statement_rows;
            let mut transition_rows = contexts.transition_rows.into_iter().collect::<Vec<_>>();
            transition_rows
                .sort_unstable_by_key(|(target, _)| (target.arena_index(), target.generation()));
            self.static_type_contexts.transition_rows = transition_rows;
        }

        pub fn elide_unreachable_case_arms(&mut self) {
            self.elide_unreachable_transition_rows();
            let rewrites = self
                .expression_table
                .expression_entries()
                .filter_map(|(handle, node)| {
                    let ExpressionNode::Match(dispatch) = node else {
                        return None;
                    };
                    let arms = self.expression_table.match_arms(dispatch.arms);
                    let excluded = |arm: &crate::expression::TableMatchArm| {
                        matches!(arm.pattern, crate::expression::MatchPattern::Destructure(_))
                            && self.static_type_contexts.is_unreachable(arm.value)
                    };
                    let live = arms
                        .iter()
                        .filter(|arm| !excluded(arm))
                        .cloned()
                        .collect::<Vec<_>>();
                    if live.len() == arms.len() || live.is_empty() {
                        return None;
                    }
                    let alias = |arm: &crate::expression::TableMatchArm| match arm.pattern {
                        crate::expression::MatchPattern::Destructure(pattern) => {
                            self.case_tag_alias(pattern)
                        }
                        _ => None,
                    };
                    let dropped_aliases = arms
                        .iter()
                        .filter(|arm| excluded(arm))
                        .filter_map(alias)
                        .collect::<Vec<_>>();
                    let survivor = live.iter().find_map(alias);
                    Some((handle, dispatch.capture, live, dropped_aliases, survivor))
                })
                .collect::<Vec<_>>();
            for (handle, capture, live, dropped_aliases, survivor) in rewrites {
                let arms = self.expression_table.insert_match_arms(live);
                if let ExpressionNode::Match(dispatch) =
                    self.expression_table.expression_mut(handle)
                {
                    dispatch.arms = arms;
                }
                if self.expression_table.contains_value_capture(capture)
                    && let Some(survivor) = survivor
                    && dropped_aliases
                        .contains(&self.expression_table.value_capture(capture).value_anchor)
                {
                    self.expression_table.set_value_anchor(capture, survivor);
                }
            }
        }

        fn elide_unreachable_transition_rows(&mut self) {
            use crate::statement::{StatementNode, TransitionGuardNode};
            let mut rewrites = Vec::new();
            for machine in self.machines() {
                for (state_index, state) in self.machine_states(machine).iter().enumerate() {
                    let statements = self.statement_table.statements(state.statement_nodes);
                    let excluded = |statement: &StatementNode| {
                        matches!(statement, StatementNode::Transition(transition)
                        if matches!(transition.guard, TransitionGuardNode::When(guard)
                            if self.guard_opens_with_refuted_case(guard))
                            && self.static_transition_type_context_is_unreachable(transition.target))
                    };
                    if !statements.iter().any(excluded) {
                        continue;
                    }
                    let live = statements
                        .iter()
                        .filter(|statement| !excluded(statement))
                        .cloned()
                        .collect::<Vec<_>>();
                    if live
                        .iter()
                        .any(|statement| matches!(statement, StatementNode::Transition(_)))
                    {
                        rewrites.push((machine.clone(), state_index, live));
                    }
                }
            }
            for (machine, state_index, live) in rewrites {
                let body = self.statement_table.insert_statements(live);
                self.machine_states_mut(&machine)[state_index].statement_nodes = body;
            }
        }

        fn guard_opens_with_refuted_case(&self, mut guard: ExpressionHandle) -> bool {
            loop {
                if !self.expression_table.expression_is_valid(guard) {
                    return false;
                }
                if let Some(facts) = self.static_type_contexts.case_membership(guard) {
                    return facts.impossible;
                }
                match self.expression_table.expression(guard) {
                    ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                        guard = binary.left;
                    }
                    ExpressionNode::Binary(binary)
                        if binary.operator == BinaryOperator::Equal
                            && self
                                .static_type_contexts
                                .boolean_equalities
                                .binary_search_by_key(
                                    &(guard.arena_index(), guard.generation()),
                                    |handle| (handle.arena_index(), handle.generation()),
                                )
                                .is_ok() =>
                    {
                        guard = match (
                            self.expression_table.expression(binary.left),
                            self.expression_table.expression(binary.right),
                        ) {
                            (_, ExpressionNode::Boolean(true)) => binary.left,
                            (ExpressionNode::Boolean(true), _) => binary.right,
                            _ => return false,
                        };
                    }
                    _ => return false,
                }
            }
        }

        fn case_tag_alias(
            &self,
            pattern: crate::expression::TableDestructurePattern,
        ) -> Option<ExpressionHandle> {
            let table = &self.expression_table;
            if !table.expression_is_valid(pattern.condition) {
                return None;
            }
            let ExpressionNode::Binary(conjunction) = table.expression(pattern.condition) else {
                return None;
            };
            let ExpressionNode::Binary(tag) = table.expression(conjunction.left) else {
                return None;
            };
            (conjunction.operator == BinaryOperator::And
                && tag.operator == BinaryOperator::CaseMembership
                && matches!(table.expression(tag.left), ExpressionNode::SavedSubject(_)))
            .then_some(tag.left)
        }

        pub fn static_statement_type_context_is_unreachable(
            &self,
            statement: crate::statement::StatementHandle,
        ) -> bool {
            self.static_type_contexts
                .statement_rows
                .binary_search_by_key(
                    &(statement.arena_index(), statement.generation()),
                    |(owned, _)| (owned.arena_index(), owned.generation()),
                )
                .is_ok_and(|position| {
                    self.static_type_contexts.statement_rows[position]
                        .1
                        .is_empty()
                })
        }

        pub fn static_transition_type_context_is_unreachable(
            &self,
            target: crate::statement::TransitionTargetHandle,
        ) -> bool {
            self.static_type_contexts
                .transition_rows
                .binary_search_by_key(
                    &(target.arena_index(), target.generation()),
                    |(owned, _)| (owned.arena_index(), owned.generation()),
                )
                .is_ok_and(|index| {
                    self.static_type_contexts.transition_rows[index]
                        .1
                        .is_empty()
                })
        }

        pub fn statically_refined_transition_type_reference(
            &self,
            target: crate::statement::TransitionTargetHandle,
            reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let Ok(index) = self
                .static_type_contexts
                .transition_rows
                .binary_search_by_key(
                    &(target.arena_index(), target.generation()),
                    |(owned, _)| (owned.arena_index(), owned.generation()),
                )
            else {
                return reference;
            };
            StaticTypeContexts::refined_in_worlds(
                self,
                reference,
                &self.static_type_contexts.transition_rows[index].1,
            )
        }

        pub fn statically_refined_type_reference(
            &self,
            expression: ExpressionHandle,
            reference: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            self.static_type_contexts
                .refined_reference(self, expression, reference)
        }
        pub fn static_type_requirement_entailed(
            &self,
            predicate: ExpressionHandle,
            context: ExpressionHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let Ok(position) = self.static_type_contexts.rows.binary_search_by_key(
                &(context.arena_index(), context.generation()),
                |(expression, _)| (expression.arena_index(), expression.generation()),
            ) else {
                return false;
            };
            self.static_type_requirement_entailed_in_worlds(
                predicate,
                &self.static_type_contexts.rows[position].1,
                substitutions,
            )
        }

        pub fn static_type_requirement_entailed_at_statement(
            &self,
            predicate: ExpressionHandle,
            context: crate::statement::StatementHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            let Ok(position) = self
                .static_type_contexts
                .statement_rows
                .binary_search_by_key(
                    &(context.arena_index(), context.generation()),
                    |(statement, _)| (statement.arena_index(), statement.generation()),
                )
            else {
                return false;
            };
            self.static_type_requirement_entailed_in_worlds(
                predicate,
                &self.static_type_contexts.statement_rows[position].1,
                substitutions,
            )
        }

        fn static_type_requirement_entailed_in_worlds(
            &self,
            predicate: ExpressionHandle,
            worlds: &[TypeWorld],
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            if !self.expression_table.expression_is_valid(predicate)
                || !contains_type_predicate(self, predicate, &mut Vec::new())
            {
                return false;
            }
            let substitutions = substitutions
                .iter()
                .copied()
                .filter(|(formal, actual)| {
                    !matches!(self.type_reference_table.type_reference(*actual),
                crate::types::TypeReferenceNode::Named { symbol, .. } if symbol == formal)
                })
                .collect::<Vec<_>>();
            let substitutions = substitutions.as_slice();
            let formals = substitutions
                .iter()
                .map(|(formal, _)| *formal)
                .collect::<Vec<_>>();
            if formals
                .iter()
                .enumerate()
                .any(|(position, formal)| formals[..position].contains(formal))
            {
                return false;
            }
            if substitutions.iter().any(|(formal, actual)| {
                !formal.is_valid()
                    || self.symbols.get(*formal).kind != symbols::SymbolKind::TypeParameter
                    || crate::references::collect_type_symbol_dependencies(self, *actual)
                        .iter()
                        .any(|symbol| formals.contains(symbol))
            }) {
                return false;
            }
            for world in worlds {
                if world
                    .bindings
                    .iter()
                    .chain(&world.exclusions)
                    .any(|(binder, actual)| {
                        formals.contains(binder)
                            || crate::references::collect_type_symbol_dependencies(self, *actual)
                                .iter()
                                .any(|symbol| formals.contains(symbol))
                    })
                {
                    return false;
                }
                let mut applied = world.clone();
                applied.bindings.extend_from_slice(substitutions);
                if !condition_inner(self, &[applied], predicate, false).is_empty() {
                    return false;
                }
            }
            true
        }

        pub fn static_type_context_is_unreachable(&self, expression: ExpressionHandle) -> bool {
            self.static_type_contexts.is_unreachable(expression)
        }
        pub fn static_type_contexts_agree(
            &self,
            left: ExpressionHandle,
            right: ExpressionHandle,
        ) -> bool {
            self.static_type_contexts.agree(left, right)
        }
    }

    /// A store for `TypedTrees::static_boolean_value_memoized` answers.
    pub trait StaticBooleanMemo {
        /// `Some(answer)` if `expression`'s answer is stored.
        fn static_boolean(&self, expression: ExpressionHandle) -> Option<Option<bool>>;
        fn remember_static_boolean(&self, expression: ExpressionHandle, value: Option<bool>);
    }

    struct StaticBooleanWalk<'memo> {
        memo: Option<&'memo dyn StaticBooleanMemo>,
        // Per arena slot: whether the expression encloses the one being valued.
        active: Vec<bool>,
        // Whether a cycle was cut since the current expression was entered.
        cut: bool,
    }

    impl<'memo> StaticBooleanWalk<'memo> {
        fn new(memo: Option<&'memo dyn StaticBooleanMemo>) -> Self {
            Self {
                memo,
                active: Vec::new(),
                cut: false,
            }
        }

        fn is_active(&self, expression: ExpressionHandle) -> bool {
            self.active
                .get(expression.arena_index() as usize)
                .copied()
                .unwrap_or(false)
        }

        fn enter(&mut self, expression: ExpressionHandle) {
            let slot = expression.arena_index() as usize;
            if slot >= self.active.len() {
                self.active.resize(slot + 1, false);
            }
            self.active[slot] = true;
        }

        fn leave(&mut self, expression: ExpressionHandle) {
            self.active[expression.arena_index() as usize] = false;
        }
    }

    impl TypedTrees {
        pub fn static_boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
            self.static_boolean_value_inner(expression, &mut StaticBooleanWalk::new(None))
        }

        /*
            `static_boolean_value` with every answer that does not depend on
            the walk's path kept in `memo` for later queries. Callers ask it
            of each logical operator's left operand, so without a memo a long
            `&&` chain re-walked its left spine at every operator.
        */
        pub fn static_boolean_value_memoized(
            &self,
            expression: ExpressionHandle,
            memo: &dyn StaticBooleanMemo,
        ) -> Option<bool> {
            self.static_boolean_value_inner(expression, &mut StaticBooleanWalk::new(Some(memo)))
        }

        fn static_boolean_value_inner(
            &self,
            expression: ExpressionHandle,
            walk: &mut StaticBooleanWalk<'_>,
        ) -> Option<bool> {
            if !self.expression_table.expression_is_valid(expression) {
                return None;
            }
            if let Some(memo) = walk.memo
                && let Some(value) = memo.static_boolean(expression)
            {
                return value;
            }
            if walk.is_active(expression) {
                // A cycle back to an enclosing expression: true only of this
                // walk, so nothing that met it is memoized.
                walk.cut = true;
                return None;
            }
            if let Ok(position) = self
                .static_type_contexts
                .completed_case_values
                .binary_search_by_key(
                    &(expression.arena_index(), expression.generation()),
                    |(expression, _)| (expression.arena_index(), expression.generation()),
                )
            {
                return Some(self.static_type_contexts.completed_case_values[position].1);
            }
            if let Some(value) = self.static_type_predicate_value(expression) {
                return Some(value);
            }
            walk.enter(expression);
            let enclosing_cut = std::mem::take(&mut walk.cut);
            let value = (|| match self.expression_table.expression(expression) {
                ExpressionNode::Boolean(value) => Some(*value),
                ExpressionNode::Name(path)
                    if path.symbol.is_valid()
                        && self.symbols.get(path.symbol).kind == symbols::SymbolKind::Local =>
                {
                    let state_symbol = self.symbols.get(path.symbol).parent;
                    let machine_symbol = self.symbols.get(state_symbol).parent;
                    let machine = self.machine_by_symbol(machine_symbol)?;
                    let state = self.state_in_machine(machine, state_symbol)?;
                    let (_, local) = self.state_local_declaration(state, path.symbol)?;
                    if local.is_mutable {
                        return None;
                    }
                    self.static_boolean_value_inner(local.ordinary_value_initializer()?, walk)
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => self
                    .static_boolean_value_inner(unary.operand, walk)
                    .map(|value| !value),
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                    if self.static_boolean_value_inner(binary.left, walk)? {
                        self.static_boolean_value_inner(binary.right, walk)
                    } else {
                        Some(false)
                    }
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Or => {
                    if self.static_boolean_value_inner(binary.left, walk)? {
                        Some(true)
                    } else {
                        self.static_boolean_value_inner(binary.right, walk)
                    }
                }
                ExpressionNode::Binary(binary)
                    if self
                        .static_type_contexts
                        .boolean_equalities
                        .contains(&expression) =>
                {
                    let left = self.static_boolean_value_inner(binary.left, walk)?;
                    let right = self.static_boolean_value_inner(binary.right, walk)?;
                    Some((left == right) != (binary.operator == BinaryOperator::NotEqual))
                }
                _ => None,
            })();
            walk.leave(expression);
            if !walk.cut
                && let Some(memo) = walk.memo
            {
                memo.remember_static_boolean(expression, value);
            }
            walk.cut |= enclosing_cut;
            value
        }

        pub fn static_boolean_normal_form(
            &self,
            mut expression: ExpressionHandle,
        ) -> ExpressionHandle {
            loop {
                let ExpressionNode::Binary(binary) = self.expression_table.expression(expression)
                else {
                    return expression;
                };
                let Some(left) = self.static_boolean_value(binary.left) else {
                    return expression;
                };
                expression = match binary.operator {
                    BinaryOperator::And => {
                        if left {
                            binary.right
                        } else {
                            binary.left
                        }
                    }
                    BinaryOperator::Or => {
                        if left {
                            binary.left
                        } else {
                            binary.right
                        }
                    }
                    _ => return expression,
                };
            }
        }
    }

    pub struct ProvenTypeContext {
        pub expression: ExpressionHandle,
        pub equations: Vec<(TypeReferenceHandle, TypeReferenceHandle)>,
        pub reachable: bool,
        pub case_value: Option<bool>,
        pub admitted_cases: Option<Vec<symbols::SymbolHandle>>,
    }

    impl TypedTrees {
        pub fn retain_proven_type_contexts(&mut self, mut contexts: Vec<ProvenTypeContext>) {
            contexts.sort_unstable_by_key(|context| {
                (
                    context.expression.arena_index(),
                    context.expression.generation(),
                )
            });
            let mut joined: Vec<ProvenTypeContext> = Vec::new();
            for context in contexts {
                if let Some(previous) = joined
                    .last_mut()
                    .filter(|previous| previous.expression == context.expression)
                {
                    if context.reachable {
                        if previous.reachable {
                            match (&mut previous.admitted_cases, context.admitted_cases) {
                                (Some(previous), Some(cases)) => {
                                    for case in cases {
                                        if !previous.contains(&case) {
                                            previous.push(case);
                                        }
                                    }
                                }
                                _ => previous.admitted_cases = None,
                            }
                            if previous.case_value != context.case_value {
                                previous.case_value = None;
                            }
                            previous
                                .equations
                                .retain(|equation| context.equations.contains(equation));
                        } else {
                            previous.equations = context.equations;
                            previous.case_value = context.case_value;
                            previous.admitted_cases = context.admitted_cases;
                        }
                        previous.reachable = true;
                    }
                } else {
                    joined.push(context);
                }
            }
            self.static_type_contexts.completed_case_values = joined
                .iter()
                .filter_map(|context| context.case_value.map(|value| (context.expression, value)))
                .collect();
            self.static_type_contexts.admitted_match_cases = joined
                .iter()
                .filter_map(|context| {
                    context
                        .admitted_cases
                        .as_ref()
                        .map(|cases| (context.expression, cases.clone()))
                })
                .collect();
            for context in joined {
                let key = (
                    context.expression.arena_index(),
                    context.expression.generation(),
                );
                let position = match self
                    .static_type_contexts
                    .rows
                    .binary_search_by_key(&key, |(expression, _)| {
                        (expression.arena_index(), expression.generation())
                    }) {
                    Ok(position) => position,
                    Err(position) => {
                        self.static_type_contexts
                            .rows
                            .insert(position, (context.expression, vec![TypeWorld::default()]));
                        position
                    }
                };
                let mut worlds = self.static_type_contexts.rows[position].1.clone();
                if !context.reachable {
                    worlds.clear();
                } else {
                    for (parameter, definition) in context.equations {
                        worlds = worlds
                            .into_iter()
                            .filter_map(|world| world.constrain(self, parameter, definition, true))
                            .collect();
                    }
                }
                self.static_type_contexts.rows[position].1 = worlds;
            }
        }

        pub fn admitted_match_cases(
            &self,
            expression: ExpressionHandle,
        ) -> Option<&[symbols::SymbolHandle]> {
            self.static_type_contexts
                .admitted_match_cases
                .binary_search_by_key(
                    &(expression.arena_index(), expression.generation()),
                    |(expression, _)| (expression.arena_index(), expression.generation()),
                )
                .ok()
                .map(|position| {
                    self.static_type_contexts.admitted_match_cases[position]
                        .1
                        .as_slice()
                })
        }

        pub fn common_static_type_reference(
            &self,
            values: &[(ExpressionHandle, TypeReferenceHandle)],
        ) -> Option<TypeReferenceHandle> {
            let &(first_expression, _) = values.first()?;
            let mut candidates = Vec::new();
            if let Ok(position) = self.static_type_contexts.rows.binary_search_by_key(
                &(
                    first_expression.arena_index(),
                    first_expression.generation(),
                ),
                |(expression, _)| (expression.arena_index(), expression.generation()),
            ) {
                for world in &self.static_type_contexts.rows[position].1 {
                    for &(symbol, _) in &world.bindings {
                        for (reference, node) in self.type_reference_table.type_reference_rows() {
                            if matches!(node, crate::types::TypeReferenceNode::Named { symbol: formal, .. } if *formal == symbol)
                                && !candidates.contains(&reference)
                            {
                                candidates.push(reference);
                            }
                        }
                    }
                }
            }
            candidates.into_iter().find(|&candidate| {
                values.iter().all(|&(expression, reference)| {
                    self.static_type_context_is_unreachable(expression)
                        || self.normalized_type_identity(
                            self.statically_refined_type_reference(expression, candidate),
                        ) == self.normalized_type_identity(
                            self.statically_refined_type_reference(expression, reference),
                        )
                })
            })
        }
    }
}
pub mod types {
    use arena::{Arena, Handle, HandleSpan};
    use language_core::ReferenceAccess;
    use numerics::arithmetic::ArithmeticDomain;
    use numerics::integer_carrier::FixedInteger;
    use std::fmt;
    use symbols::SymbolHandle;

    use crate::name::Identifier;
    use crate::remapped;

    pub(crate) mod copies {
        use super::*;

        pub struct TypeExpressionCopy {
            pub source: TypeReferenceHandle,
            pub destination: TypeReferenceHandle,
            pub expression: crate::expression::ExpressionCopy,
        }

        impl TypeReferenceTable {
            pub fn copy_from(
                &mut self,
                source: &TypeReferenceTable,
                source_expressions: &crate::expression::ExpressionTable,
                target_expressions: &mut crate::expression::ExpressionTable,
                type_reference: TypeReferenceHandle,
            ) -> TypeReferenceHandle {
                self.copy_with_expressions(
                    source,
                    source_expressions,
                    target_expressions,
                    type_reference,
                    &mut Vec::new(),
                )
            }

            pub fn copy_with_expressions(
                &mut self,
                source: &TypeReferenceTable,
                source_expressions: &crate::expression::ExpressionTable,
                target_expressions: &mut crate::expression::ExpressionTable,
                type_reference: TypeReferenceHandle,
                expressions: &mut Vec<TypeExpressionCopy>,
            ) -> TypeReferenceHandle {
                let copied = match source.type_reference(type_reference) {
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    } => {
                        let referee = self.copy_with_expressions(
                            source,
                            source_expressions,
                            target_expressions,
                            *referee,
                            expressions,
                        );
                        self.insert(TypeReferenceNode::Reference {
                            referee,
                            access: *access,
                            lifetime: lifetime.clone(),
                        })
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        let base_type = self.copy_with_expressions(
                            source,
                            source_expressions,
                            target_expressions,
                            *base_type,
                            expressions,
                        );
                        let constraints = self.copy_constraints_from(
                            source,
                            source_expressions,
                            target_expressions,
                            *constraints,
                            expressions,
                        );
                        self.insert(TypeReferenceNode::Constrained {
                            base_type,
                            constraints,
                        })
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        let element_type = self.copy_with_expressions(
                            source,
                            source_expressions,
                            target_expressions,
                            *element_type,
                            expressions,
                        );
                        let length = match length {
                            FixedArrayLength::Expression(argument) => {
                                FixedArrayLength::Expression(self.copy_with_expressions(
                                    source,
                                    source_expressions,
                                    target_expressions,
                                    *argument,
                                    expressions,
                                ))
                            }
                            other => other.clone(),
                        };
                        self.insert(TypeReferenceNode::FixedArray {
                            element_type,
                            length,
                        })
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        let element_type = self.copy_with_expressions(
                            source,
                            source_expressions,
                            target_expressions,
                            *element_type,
                            expressions,
                        );
                        self.insert(TypeReferenceNode::Slice { element_type })
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        lifetime_arguments,
                        arguments,
                    } => {
                        let arguments = self.copy_type_reference_handles_from(
                            source,
                            source_expressions,
                            target_expressions,
                            *arguments,
                            expressions,
                        );
                        self.insert(TypeReferenceNode::Generic {
                            base_symbol: *base_symbol,
                            base_name: base_name.clone(),
                            lifetime_arguments: lifetime_arguments.clone(),
                            arguments,
                        })
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        let expression =
                            target_expressions.copy_graph(Some(source_expressions), *expression);
                        let destination =
                            self.insert(TypeReferenceNode::ConstExpression(expression.root));
                        expressions.push(TypeExpressionCopy {
                            source: type_reference,
                            destination,
                            expression,
                        });
                        destination
                    }
                    TypeReferenceNode::DynamicTrait {
                        symbol,
                        name,
                        arguments,
                    } => {
                        let arguments = self.copy_type_reference_handles_from(
                            source,
                            source_expressions,
                            target_expressions,
                            *arguments,
                            expressions,
                        );
                        self.insert(TypeReferenceNode::DynamicTrait {
                            symbol: *symbol,
                            name: name.clone(),
                            arguments,
                        })
                    }
                    TypeReferenceNode::Named { symbol, name } => {
                        self.insert(TypeReferenceNode::Named {
                            symbol: *symbol,
                            name: name.clone(),
                        })
                    }
                    TypeReferenceNode::Unit => self.insert(TypeReferenceNode::Unit),
                };
                let argument = source.array_extent_argument(type_reference);
                if argument.is_valid() {
                    let argument = match self.type_reference(copied) {
                        TypeReferenceNode::FixedArray {
                            length: FixedArrayLength::Expression(argument),
                            ..
                        } => *argument,
                        _ => self.copy_with_expressions(
                            source,
                            source_expressions,
                            target_expressions,
                            argument,
                            expressions,
                        ),
                    };
                    self.retain_array_extent_argument(copied, argument)
                        .expect("copied array extent child");
                }
                let source_owner =
                    Handle::from_parts(type_reference.arena_index(), type_reference.generation());
                let target_owner = Handle::from_parts(copied.arena_index(), copied.generation());
                let span = *source.constant_origin_spans.get(source_owner);
                *self.constant_origin_spans.get_mut(target_owner) = self
                    .constant_origins
                    .insert_many(source.constant_origins.span_or_empty(span).iter().copied());
                *self.operand_lifetime_owners.get_mut(Handle::from_parts(
                    copied.arena_index(),
                    copied.generation(),
                )) = source.operand_lifetime_owner(type_reference);
                let lifetimes = source.operand_lifetimes(type_reference);
                let span = self
                    .operand_lifetimes
                    .insert_many(lifetimes.iter().copied());
                *self.operand_lifetime_spans.get_mut(Handle::from_parts(
                    copied.arena_index(),
                    copied.generation(),
                )) = span;
                self.set_operand_origin(copied, source.operand_origin(type_reference));
                for application in source.type_alias_applications(type_reference) {
                    let copied_application = self.copy_with_expressions(
                        source,
                        source_expressions,
                        target_expressions,
                        application.application,
                        expressions,
                    );
                    let (arguments, lifetime_arguments) = match self
                        .type_reference(copied_application)
                    {
                        TypeReferenceNode::Generic {
                            arguments,
                            lifetime_arguments,
                            ..
                        } => (*arguments, lifetime_arguments.clone()),
                        TypeReferenceNode::Named { .. } => (HandleSpan::empty(), Vec::new()),
                        _ => unreachable!("retained alias application keeps its declaration head"),
                    };
                    self.retain_type_alias_application(crate::type_aliases::TypeAliasApplication {
                        instance: copied,
                        application: copied_application,
                        arguments,
                        lifetime_arguments,
                        ..application.clone()
                    });
                }
                copied
            }

            pub fn remap_symbols_in<'scope>(
                &mut self,
                type_reference: TypeReferenceHandle,
                expressions: &mut crate::expression::ExpressionTable,
                symbols: impl Into<crate::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                if !type_reference.is_valid() {
                    return;
                }
                let argument = self.array_extent_argument(type_reference);
                if argument.is_valid()
                    && !matches!(self.type_reference(type_reference), TypeReferenceNode::FixedArray { length: FixedArrayLength::Expression(current), .. } if *current == argument)
                {
                    self.remap_symbols_in(argument, expressions, symbols);
                }
                let applications = self
                    .type_alias_applications(type_reference)
                    .map(|application| application.application)
                    .collect::<Vec<_>>();
                for application in applications {
                    self.remap_symbols_in(application, expressions, symbols);
                }
                let node = self.type_reference(type_reference).clone();
                match node {
                    TypeReferenceNode::Reference { referee, .. } => {
                        self.remap_symbols_in(referee, expressions, symbols);
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        self.remap_symbols_in(base_type, expressions, symbols);
                        for constraint in self.constraints(constraints).to_vec() {
                            match constraint {
                                TypeConstraintNode::Domain(domain) => {
                                    for argument in domain.arguments {
                                        self.remap_symbols_in(argument, expressions, symbols);
                                    }
                                }

                                TypeConstraintNode::Named(_)
                                | TypeConstraintNode::ArithmeticDomain(_) => {}
                            }
                        }
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        self.remap_symbols_in(element_type, expressions, symbols);
                        if let FixedArrayLength::Expression(argument) = length {
                            self.remap_symbols_in(argument, expressions, symbols);
                        }
                        if let FixedArrayLength::ConstParameter { symbol, .. } = length
                            && let TypeReferenceNode::FixedArray { length, .. } =
                                self.type_references.get_mut(type_reference)
                            && let FixedArrayLength::ConstParameter {
                                symbol: current, ..
                            } = length
                        {
                            *current = remapped(symbol, symbols);
                        }
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        self.remap_symbols_in(element_type, expressions, symbols);
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => {
                        for argument in self.type_reference_handles(arguments).to_vec() {
                            self.remap_symbols_in(argument, expressions, symbols);
                        }
                        let TypeReferenceNode::Generic {
                            base_symbol: current,
                            ..
                        } = self.type_references.get_mut(type_reference)
                        else {
                            unreachable!();
                        };
                        *current = remapped(base_symbol, symbols);
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        expressions.remap_symbols_in(expression, symbols);
                    }
                    TypeReferenceNode::DynamicTrait {
                        symbol, arguments, ..
                    } => {
                        for argument in self.type_reference_handles(arguments).to_vec() {
                            self.remap_symbols_in(argument, expressions, symbols);
                        }
                        let TypeReferenceNode::DynamicTrait {
                            symbol: current, ..
                        } = self.type_references.get_mut(type_reference)
                        else {
                            unreachable!();
                        };
                        *current = remapped(symbol, symbols);
                    }
                    TypeReferenceNode::Named { symbol, .. } => {
                        let replaced_named = matches!(
                            self.type_references.get(type_reference),
                            TypeReferenceNode::Named { .. }
                        )
                        .then_some(symbol);
                        let TypeReferenceNode::Named {
                            symbol: current, ..
                        } = self.type_references.get_mut(type_reference)
                        else {
                            unreachable!();
                        };
                        *current = remapped(symbol, symbols);
                        self.renamed_row(type_reference, replaced_named);
                    }
                    TypeReferenceNode::Unit => {}
                }
            }

            fn copy_type_reference_handles_from(
                &mut self,
                source: &TypeReferenceTable,
                source_expressions: &crate::expression::ExpressionTable,
                target_expressions: &mut crate::expression::ExpressionTable,
                type_references: HandleSpan<TypeReferenceHandle>,
                expressions: &mut Vec<TypeExpressionCopy>,
            ) -> HandleSpan<TypeReferenceHandle> {
                let source_type_references = source.type_reference_handles(type_references);
                let copied = self.reserve_type_reference_handles(
                    source_type_references
                        .len()
                        .try_into()
                        .expect("type reference handle span count overflow"),
                );

                for (offset, type_reference) in source_type_references.iter().enumerate() {
                    let type_reference = self.copy_with_expressions(
                        source,
                        source_expressions,
                        target_expressions,
                        *type_reference,
                        expressions,
                    );
                    self.set_type_reference_handle_at_offset(
                        copied,
                        offset
                            .try_into()
                            .expect("type reference handle span count overflow"),
                        type_reference,
                    );
                }

                copied
            }

            fn copy_constraints_from(
                &mut self,
                source: &TypeReferenceTable,
                source_expressions: &crate::expression::ExpressionTable,
                target_expressions: &mut crate::expression::ExpressionTable,
                constraints: HandleSpan<TypeConstraintNode>,
                expressions: &mut Vec<TypeExpressionCopy>,
            ) -> HandleSpan<TypeConstraintNode> {
                let source_constraints = source.constraints(constraints);
                let copied = self.reserve_constraints(
                    source_constraints
                        .len()
                        .try_into()
                        .expect("type constraint span count overflow"),
                );

                for (offset, constraint) in source_constraints.iter().enumerate() {
                    let copied_constraint = match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            let mut domain = domain.clone();
                            domain.arguments = domain
                                .arguments
                                .iter()
                                .map(|argument| {
                                    self.copy_with_expressions(
                                        source,
                                        source_expressions,
                                        target_expressions,
                                        *argument,
                                        expressions,
                                    )
                                })
                                .collect();
                            TypeConstraintNode::Domain(domain)
                        }
                        _ => constraint.clone(),
                    };
                    self.set_constraint_at_offset(
                        copied,
                        offset
                            .try_into()
                            .expect("type constraint span count overflow"),
                        copied_constraint,
                    );
                }

                copied
            }
        }
    }
    use crate::expression::ExpressionHandle;
    pub use copies::TypeExpressionCopy;
    #[cfg(test)]
    pub(crate) mod alias_tests {
        use super::*;
        use crate::type_aliases::TypeAliasApplication;

        fn application(
            table: &mut TypeReferenceTable,
            declaration: SymbolHandle,
            argument: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            let arguments = table.insert_type_reference_handles([argument]);
            table.insert(TypeReferenceNode::Generic {
                base_symbol: declaration,
                base_name: Identifier::generated("Alias"),
                lifetime_arguments: Vec::new(),
                arguments,
            })
        }

        fn retain(
            table: &mut TypeReferenceTable,
            instance: TypeReferenceHandle,
            application: TypeReferenceHandle,
            declaration: SymbolHandle,
        ) {
            let TypeReferenceNode::Generic {
                arguments,
                lifetime_arguments,
                ..
            } = table.type_reference(application)
            else {
                panic!("application");
            };
            let (arguments, lifetime_arguments) = (*arguments, lifetime_arguments.clone());
            table.retain_type_alias_application(TypeAliasApplication {
                instance,
                application,
                declaration,
                arguments,
                lifetime_arguments,
                ..Default::default()
            });
        }

        #[test]
        fn alias_obligations_survive_copy_actual_substitution_and_complete_applied_views() {
            let parameter = SymbolHandle::from_parts(1, 3);
            let receiving_alias = SymbolHandle::from_parts(11, 2);
            let supplied_alias = SymbolHandle::from_parts(12, 4);
            let mut source = TypeReferenceTable::new();
            let input = source.insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: Identifier::generated("T"),
            });
            let written = source.fresh_occurrence(input);
            let original_application = application(&mut source, receiving_alias, input);
            retain(&mut source, written, original_application, receiving_alias);
            let mut program = crate::TypedTrees::default();
            let tables = &mut *program;
            tables.type_reference_table = source.clone();
            let operand = program.static_operand_from_type_reference(written).unwrap();
            assert_eq!(operand.type_reference, written);
            assert!(operand.path.is_empty());
            let mut target = TypeReferenceTable::new();
            let mut expressions = crate::expression::ExpressionTable::default();
            let copied = target.copy_from(
                &source,
                &crate::expression::ExpressionTable::default(),
                &mut expressions,
                written,
            );
            let copied_obligation = target
                .type_alias_applications(copied)
                .next()
                .unwrap()
                .clone();
            assert_eq!(copied_obligation.declaration, receiving_alias);
            let TypeReferenceNode::Generic { arguments, .. } =
                target.type_reference(copied_obligation.application)
            else {
                panic!("copied application");
            };
            assert_eq!(*arguments, copied_obligation.arguments);
            assert!(
                matches!(target.type_reference(target.type_reference_handles(*arguments)[0]), TypeReferenceNode::Named { symbol, .. } if *symbol == parameter)
            );

            let supplied_value = target.insert(TypeReferenceNode::Unit);
            let actual = target.fresh_occurrence(supplied_value);
            let supplied_application = application(&mut target, supplied_alias, supplied_value);
            retain(&mut target, actual, supplied_application, supplied_alias);
            target.substitute_from(copied, actual).unwrap();
            assert_eq!(
                target
                    .type_alias_applications(copied)
                    .map(|row| row.declaration)
                    .collect::<Vec<_>>(),
                [receiving_alias, supplied_alias]
            );
            target.substitute_shared_children(0, parameter, actual);
            let receiving = target
                .type_alias_applications(copied)
                .find(|row| row.declaration == receiving_alias)
                .unwrap();
            assert_eq!(target.type_reference_handles(receiving.arguments), [actual]);

            let applied = target.fresh_occurrence(actual);
            let applied_application = application(&mut target, receiving_alias, actual);
            retain(&mut target, applied, applied_application, receiving_alias);
            let expected = target
                .type_alias_applications(applied)
                .cloned()
                .collect::<Vec<_>>();
            target.substitute_applied_from(copied, applied).unwrap();
            let actual_rows = target
                .type_alias_applications(copied)
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(actual_rows.len(), expected.len());
            for (actual, mut expected) in actual_rows.into_iter().zip(expected) {
                expected.instance = copied;
                assert_eq!(actual, expected);
            }
        }
    }
    pub(crate) mod display {
        use diagnostics::comma_join_display;

        use crate::expression::ExpressionTable;
        use crate::types::{TypeConstraintNode, TypeReferenceNode, TypeReferenceTable};

        impl TypeReferenceNode {
            pub fn display_name(&self, table: &TypeReferenceTable) -> String {
                match self {
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: _,
                    } => {
                        format!("{}{}", access.borrow_prefix(), table.display_name(*referee))
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        format!(
                            "{}[{}]",
                            table.display_name(*base_type),
                            match constraints.count() {
                                1 => "1 constraint".to_owned(),
                                count => format!("{count} constraints"),
                            }
                        )
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        format!("[{}; {}]", table.display_name(*element_type), length)
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        format!("[{}]", table.display_name(*element_type))
                    }
                    TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } => {
                        format!(
                            "{base_name}<{}>",
                            comma_join_display(
                                table.type_reference_handles(*arguments),
                                |argument| { table.display_name(*argument) }
                            )
                        )
                    }
                    TypeReferenceNode::ConstExpression(_) => "const <expression>".to_owned(),
                    TypeReferenceNode::DynamicTrait {
                        name, arguments, ..
                    } => match arguments.count() {
                        0 => format!("dyn {name}"),
                        1 => format!("dyn {name}<1 argument>"),
                        count => format!("dyn {name}<{count} arguments>"),
                    },
                    TypeReferenceNode::Named { name, .. } => name.to_string(),
                    TypeReferenceNode::Unit => "()".to_owned(),
                }
            }

            pub fn display_name_with_constraints(
                &self,
                table: &TypeReferenceTable,
                expressions: &ExpressionTable,
            ) -> String {
                match self {
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime: _,
                    } => {
                        format!(
                            "{}{}",
                            access.borrow_prefix(),
                            table.display_name_with_constraints(*referee, expressions)
                        )
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        format!(
                            "{}[{}]",
                            table.display_name_with_constraints(*base_type, expressions),
                            comma_join_display(table.constraints(*constraints), |constraint| {
                                constraint.display_name()
                            })
                        )
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        format!(
                            "[{}; {}]",
                            table.display_name_with_constraints(*element_type, expressions),
                            length
                        )
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        format!(
                            "[{}]",
                            table.display_name_with_constraints(*element_type, expressions)
                        )
                    }
                    TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } => {
                        format!(
                            "{base_name}<{}>",
                            comma_join_display(
                                table.type_reference_handles(*arguments),
                                |argument| {
                                    table.display_name_with_constraints(*argument, expressions)
                                }
                            )
                        )
                    }
                    TypeReferenceNode::ConstExpression(expression) => {
                        format!("const {}", expressions.display_name(*expression))
                    }
                    TypeReferenceNode::DynamicTrait {
                        name, arguments, ..
                    } => format!(
                        "dyn {name}<{}>",
                        comma_join_display(table.type_reference_handles(*arguments), |argument| {
                            table.display_name_with_constraints(*argument, expressions)
                        })
                    ),
                    TypeReferenceNode::Named { name, .. } => name.to_string(),
                    TypeReferenceNode::Unit => "()".to_owned(),
                }
            }
        }

        impl TypeConstraintNode {
            pub fn display_name(&self) -> String {
                match self {
                    TypeConstraintNode::Named(name) => name.to_string(),
                    TypeConstraintNode::Domain(name) => format!("in {name}"),

                    TypeConstraintNode::ArithmeticDomain(domain) => format!("in {}", domain.name()),
                }
            }
        }
    }
    #[cfg(test)]
    pub(crate) mod extent_custody_tests {
        use super::*;
        use crate::expression::{ExpressionNode, ExpressionTable};
        use numerics::literals::{IntegerLiteral, IntegerRadix};

        fn integer(expressions: &mut ExpressionTable, value: &str) -> ExpressionHandle {
            expressions.insert(ExpressionNode::Integer(
                IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value).unwrap(),
            ))
        }

        #[test]
        fn completed_extent_custody_survives_distinct_arenas_and_replacement() {
            let mut expressions = ExpressionTable::new();
            let authored = integer(&mut expressions, "1");
            let materialized = integer(&mut expressions, "1");
            expressions.retain_materialization_source(materialized, authored);
            let mut source = TypeReferenceTable::new();
            let argument = source.insert(TypeReferenceNode::ConstExpression(materialized));
            let element_type = source.insert(TypeReferenceNode::Unit);
            let array = source.insert(TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Expression(argument),
            });
            source.set_fixed_array_length(array, 1);
            let fresh = source.fresh_occurrence(array);
            assert_eq!(source.array_extent_argument(fresh), argument);

            let mut receiving_expressions = ExpressionTable::new();
            for _ in 0..4 {
                integer(&mut receiving_expressions, "99");
            }
            let mut receiving = TypeReferenceTable::new();
            for _ in 0..4 {
                receiving.insert(TypeReferenceNode::Unit);
            }
            let copied =
                receiving.copy_from(&source, &expressions, &mut receiving_expressions, fresh);
            let copied_argument = receiving.array_extent_argument(copied);
            assert_ne!(copied_argument, argument);
            let TypeReferenceNode::ConstExpression(value) =
                receiving.type_reference(copied_argument)
            else {
                panic!("completed extent lost its value child");
            };
            assert_ne!(*value, materialized);
            assert_eq!(
                receiving_expressions.constant_integer_value(*value),
                Some(1)
            );
            let original = receiving_expressions.materialization_source(*value);
            assert!(original.is_valid());
            assert_ne!(original, authored);
            assert_eq!(
                receiving_expressions.constant_integer_value(original),
                Some(1)
            );
            assert!(matches!(
                receiving.type_reference(copied),
                TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Literal(1),
                    ..
                }
            ));

            let replacement = receiving.insert(TypeReferenceNode::Unit);
            receiving.substitute_from(copied, replacement).unwrap();
            assert!(!receiving.array_extent_argument(copied).is_valid());
            assert_eq!(source.array_extent_argument(fresh), argument);

            let mut program = crate::TypedTrees::default();
            let tables = &mut *program;
            tables.type_reference_table = source;
            tables.expression_table = expressions;
            let applied = program
                .apply_callable_type(fresh, &[], &[], SymbolHandle::invalid())
                .unwrap();
            let applied_argument = program.type_reference_table.array_extent_argument(applied);
            let TypeReferenceNode::ConstExpression(applied_value) = program
                .type_reference_table
                .type_reference(applied_argument)
            else {
                panic!("callable application lost the completed extent child");
            };
            assert!(
                program
                    .expression_table
                    .materialization_source(*applied_value)
                    .is_valid()
            );

            let binder = SymbolHandle::from_parts(19, 0);
            let template = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstParameter {
                        symbol: binder,
                        name: Identifier::generated("N"),
                    },
                });
            let supplied = program
                .apply_callable_type(
                    template,
                    &[(binder, argument)],
                    &[],
                    SymbolHandle::invalid(),
                )
                .unwrap();
            assert_eq!(
                program.type_reference_table.array_extent_argument(supplied),
                argument,
                "an already supplied actual must retain its own telescope"
            );
        }
    }
    pub(crate) mod substitution {
        use super::*;

        impl TypeReferenceTable {
            pub fn substitute_node(
                &mut self,
                handle: TypeReferenceHandle,
                node: TypeReferenceNode,
            ) {
                let same_region_slots = match (self.type_references.get(handle), &node) {
                    (
                        TypeReferenceNode::Generic {
                            lifetime_arguments: before,
                            ..
                        },
                        TypeReferenceNode::Generic {
                            lifetime_arguments: after,
                            ..
                        },
                    ) => before == after,
                    (
                        TypeReferenceNode::Reference {
                            lifetime: before, ..
                        },
                        TypeReferenceNode::Reference {
                            lifetime: after, ..
                        },
                    ) => before == after,
                    _ => false,
                };
                let replaced = std::mem::replace(self.type_references.get_mut(handle), node);
                if !same_region_slots {
                    *self.operand_lifetime_spans.get_mut(Handle::from_parts(
                        handle.arena_index(),
                        handle.generation(),
                    )) = HandleSpan::empty();
                }
                let replaced_named = match replaced {
                    TypeReferenceNode::Named { symbol, .. } => Some(symbol),
                    _ => None,
                };
                if !matches!(
                    self.type_reference(handle),
                    TypeReferenceNode::FixedArray { .. }
                ) {
                    *self.array_extent_arguments.get_mut(Handle::from_parts(
                        handle.arena_index(),
                        handle.generation(),
                    )) = TypeReferenceHandle::invalid();
                }
                self.renamed_row(handle, replaced_named);
            }

            pub fn substitute_from(
                &mut self,
                occurrence: TypeReferenceHandle,
                replacement: TypeReferenceHandle,
            ) -> Result<(), &'static str> {
                self.substitute_occurrence(occurrence, replacement, false)
            }

            pub fn substitute_applied_from(
                &mut self,
                occurrence: TypeReferenceHandle,
                replacement: TypeReferenceHandle,
            ) -> Result<(), &'static str> {
                self.substitute_occurrence(occurrence, replacement, true)
            }

            fn substitute_occurrence(
                &mut self,
                occurrence: TypeReferenceHandle,
                replacement: TypeReferenceHandle,
                complete_applied_view: bool,
            ) -> Result<(), &'static str> {
                self.constant_origins(occurrence)?;
                let origins = self.constant_origins(replacement)?.to_vec();
                if occurrence == replacement {
                    return Ok(());
                }
                let argument = self.array_extent_argument(replacement);
                let node = self.type_reference(replacement).clone();
                self.attach_constant_origins(occurrence, origins)?;
                let owner = self.operand_lifetime_owner(replacement);
                *self.operand_lifetime_owners.get_mut(Handle::from_parts(
                    occurrence.arena_index(),
                    occurrence.generation(),
                )) = owner;
                self.set_operand_origin(occurrence, self.operand_origin(replacement));
                let lifetimes = self.operand_lifetimes(replacement).to_vec();
                self.substitute_node(occurrence, node);
                *self.array_extent_arguments.get_mut(Handle::from_parts(
                    occurrence.arena_index(),
                    occurrence.generation(),
                )) = argument;
                self.retain_operand_lifetimes(occurrence, lifetimes)?;
                let applications = self
                    .type_alias_applications(replacement)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut previous = if complete_applied_view {
                    self.type_alias_applications
                        .iter()
                        .filter_map(|(handle, application)| {
                            (application.instance == occurrence).then_some(handle)
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
                .into_iter();
                for application in applications {
                    let application = crate::type_aliases::TypeAliasApplication {
                        instance: occurrence,
                        ..application
                    };
                    match previous.next() {
                        Some(handle) => *self.type_alias_applications.get_mut(handle) = application,
                        None => self.retain_type_alias_application(application),
                    }
                }
                for handle in previous {
                    self.type_alias_applications.free(handle);
                }
                Ok(())
            }

            pub(crate) fn renamed_row(
                &mut self,
                handle: TypeReferenceHandle,
                replaced_named: Option<SymbolHandle>,
            ) {
                self.arithmetic_rows.0.take();
                if let Some(symbol) = replaced_named
                    && self
                        .named_rows
                        .0
                        .get()
                        .is_some_and(|rows| rows.get(&symbol) == Some(&handle))
                {
                    self.rescan_named_row(symbol);
                }
                if let Some(symbol) = self.type_references.get(handle).as_named_symbol() {
                    self.note_named_row(symbol, handle);
                }
            }

            pub fn substitute_shared_children(
                &mut self,
                type_start: usize,
                parameter_symbol: SymbolHandle,
                replacement: TypeReferenceHandle,
            ) -> usize {
                let parents: Vec<TypeReferenceHandle> = self
                    .references_from(type_start)
                    .map(|(handle, _)| handle)
                    .collect();
                let mut rewritten = 0;
                for parent in parents {
                    match self.type_references.get(parent).clone() {
                        TypeReferenceNode::Reference {
                            referee,
                            access,
                            lifetime,
                        } if self.named_child_matches(referee, parameter_symbol) => {
                            self.substitute_node(
                                parent,
                                TypeReferenceNode::Reference {
                                    referee: replacement,
                                    access,
                                    lifetime,
                                },
                            );
                            rewritten += 1;
                        }
                        TypeReferenceNode::Constrained {
                            base_type,
                            constraints,
                        } if self.named_child_matches(base_type, parameter_symbol) => {
                            self.substitute_node(
                                parent,
                                TypeReferenceNode::Constrained {
                                    base_type: replacement,
                                    constraints,
                                },
                            );
                            rewritten += 1;
                        }
                        TypeReferenceNode::FixedArray {
                            element_type,
                            length,
                        } if self.named_child_matches(element_type, parameter_symbol) => {
                            self.substitute_node(
                                parent,
                                TypeReferenceNode::FixedArray {
                                    element_type: replacement,
                                    length,
                                },
                            );
                            rewritten += 1;
                        }
                        TypeReferenceNode::Slice { element_type }
                            if self.named_child_matches(element_type, parameter_symbol) =>
                        {
                            self.substitute_node(
                                parent,
                                TypeReferenceNode::Slice {
                                    element_type: replacement,
                                },
                            );
                            rewritten += 1;
                        }
                        TypeReferenceNode::Generic { arguments, .. } => {
                            for (offset, child) in self
                                .type_reference_handles(arguments)
                                .to_vec()
                                .into_iter()
                                .enumerate()
                            {
                                if self.named_child_matches(child, parameter_symbol) {
                                    self.set_type_reference_handle_at_offset(
                                        arguments,
                                        u32::try_from(offset).expect("generic argument offset"),
                                        replacement,
                                    );
                                    rewritten += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                rewritten
            }

            fn named_child_matches(
                &self,
                child: TypeReferenceHandle,
                symbol: SymbolHandle,
            ) -> bool {
                matches!(
                    self.type_references.get(child),
                    TypeReferenceNode::Named {
                        symbol: named,
                        ..
                    } if *named == symbol
                )
            }

            pub fn instantiate(
                &mut self,
                handle: TypeReferenceHandle,
                substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            ) -> Result<TypeReferenceHandle, &'static str> {
                if !handle.is_valid() {
                    return Ok(handle);
                }
                let rebuilt = match self.type_reference(handle).clone() {
                    TypeReferenceNode::Named { symbol, .. } => {
                        return Ok(substitutions
                            .iter()
                            .find(|(binder, _)| *binder == symbol)
                            .map_or(handle, |(_, argument)| *argument));
                    }
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    } => {
                        let instantiated = self.instantiate(referee, substitutions)?;
                        (instantiated != referee).then_some(TypeReferenceNode::Reference {
                            referee: instantiated,
                            access,
                            lifetime,
                        })
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        // `run: Run<P, T> in Live<P, T>` applied at `<u8, Item>`
                        // is `Run<u8, Item> in Live<u8, Item>`: a domain's
                        // arguments are types of the same telescope as the
                        // carrier, so they take the same substitution. The
                        // authored span stays as written for the template;
                        // whether the applied domain holds is not asked here.
                        let instantiated = self.instantiate(base_type, substitutions)?;
                        let mut applied = self.constraints(constraints).to_vec();
                        let mut changed = false;
                        for constraint in &mut applied {
                            let TypeConstraintNode::Domain(domain) = constraint else {
                                continue;
                            };
                            for argument in &mut domain.arguments {
                                let actual = self.instantiate(*argument, substitutions)?;
                                changed |= actual != *argument;
                                *argument = actual;
                            }
                        }
                        let constraints = if changed {
                            self.insert_constraints(applied)
                        } else {
                            constraints
                        };
                        (changed || instantiated != base_type).then_some(
                            TypeReferenceNode::Constrained {
                                base_type: instantiated,
                                constraints,
                            },
                        )
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        if let FixedArrayLength::ConstParameter { symbol, .. } = &length
                            && substitutions.iter().any(|(binder, _)| binder == symbol)
                        {
                            return Err(
                                "a trait parameter as an array length has no instantiation",
                            );
                        }
                        let instantiated = self.instantiate(element_type, substitutions)?;
                        (instantiated != element_type).then_some(TypeReferenceNode::FixedArray {
                            element_type: instantiated,
                            length,
                        })
                    }
                    TypeReferenceNode::Slice { element_type } => {
                        let instantiated = self.instantiate(element_type, substitutions)?;
                        (instantiated != element_type).then_some(TypeReferenceNode::Slice {
                            element_type: instantiated,
                        })
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        lifetime_arguments,
                        arguments,
                    } => self
                        .instantiate_arguments(arguments, substitutions)?
                        .map(|arguments| TypeReferenceNode::Generic {
                            base_symbol,
                            base_name,
                            lifetime_arguments,
                            arguments,
                        }),
                    TypeReferenceNode::DynamicTrait {
                        symbol,
                        name,
                        arguments,
                    } => self
                        .instantiate_arguments(arguments, substitutions)?
                        .map(|arguments| TypeReferenceNode::DynamicTrait {
                            symbol,
                            name,
                            arguments,
                        }),
                    TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => None,
                };
                let Some(node) = rebuilt else {
                    return Ok(handle);
                };
                let owner = self.operand_lifetime_owner(handle);
                let lifetimes = self.operand_lifetimes(handle).to_vec();
                let origin = self.operand_origin(handle);
                let origins = self.constant_origins(handle)?.to_vec();
                let extent_argument = self.array_extent_argument(handle);
                let rebuilt = self.insert(node);
                if extent_argument.is_valid() {
                    self.retain_array_extent_argument(rebuilt, extent_argument)?;
                }
                self.retain_operand_lifetime_owner(rebuilt, owner)?;
                self.retain_operand_lifetimes(rebuilt, lifetimes)?;
                self.set_operand_origin(rebuilt, origin);
                self.attach_constant_origins(rebuilt, origins)?;
                Ok(rebuilt)
            }

            fn instantiate_arguments(
                &mut self,
                arguments: HandleSpan<TypeReferenceHandle>,
                substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            ) -> Result<Option<HandleSpan<TypeReferenceHandle>>, &'static str> {
                let original = self.type_reference_handles(arguments).to_vec();
                let mut instantiated = Vec::with_capacity(original.len());
                for argument in &original {
                    instantiated.push(self.instantiate(*argument, substitutions)?);
                }
                Ok((instantiated != original)
                    .then(|| self.insert_type_reference_handles(instantiated)))
            }
        }
    }

    pub type TypeReferenceHandle = Handle<TypeReferenceNode>;

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeConstantOrigin {
        pub declaration: SymbolHandle,
        pub reference: source::SourceSpan,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
    pub enum TypeOperandOrigin {
        #[default]
        Lexical,
        MachineParameter {
            instance: SymbolHandle,
            parameter: SymbolHandle,
        },
    }

    #[derive(Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct TypeReferenceTable {
        type_references: Arena<TypeReferenceNode>,
        type_reference_handles: Arena<TypeReferenceHandle>,
        constraints: Arena<TypeConstraintNode>,
        constant_origin_spans: Arena<HandleSpan<TypeConstantOrigin>>,
        constant_origins: Arena<TypeConstantOrigin>,
        array_extent_arguments: Arena<TypeReferenceHandle>,
        operand_lifetime_owners: Arena<SymbolHandle>,
        operand_lifetime_spans: Arena<HandleSpan<crate::type_identity::LifetimeParameterIdentity>>,
        operand_lifetimes: Arena<crate::type_identity::LifetimeParameterIdentity>,
        operand_origins: Arena<TypeOperandOrigin>,
        named_rows: NamedRowIndex,
        arithmetic_rows: ArithmeticRowIndex,
        slice_views: SliceViewIndex,
        policy_shells: PolicyShellIndex,
        type_alias_applications: Arena<crate::type_aliases::TypeAliasApplication>,
    }

    impl std::fmt::Debug for TypeReferenceTable {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("TypeReferenceTable")
                .field("type_references", &self.type_references)
                .field("type_reference_handles", &self.type_reference_handles)
                .field("constraints", &self.constraints)
                .field("constant_origin_spans", &self.constant_origin_spans)
                .field("constant_origins", &self.constant_origins)
                .field("array_extent_arguments", &self.array_extent_arguments)
                .field("operand_lifetime_owners", &self.operand_lifetime_owners)
                .field("operand_lifetime_spans", &self.operand_lifetime_spans)
                .field("operand_lifetimes", &self.operand_lifetimes)
                .field("operand_origins", &self.operand_origins)
                .field("type_alias_applications", &self.type_alias_applications)
                .finish()
        }
    }

    #[derive(Clone, Default)]
    struct NamedRowIndex(
        std::sync::OnceLock<symbols::SymbolKeyMap<SymbolHandle, TypeReferenceHandle>>,
    );

    impl PartialEq for NamedRowIndex {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for NamedRowIndex {}

    #[derive(Clone, Default)]
    struct ArithmeticRowIndex(
        std::sync::OnceLock<
            symbols::SymbolKeyMap<
                SymbolHandle,
                Vec<(numerics::arithmetic::ArithmeticDomain, TypeReferenceHandle)>,
            >,
        >,
    );

    impl PartialEq for ArithmeticRowIndex {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for ArithmeticRowIndex {}

    #[derive(Clone, Default)]
    struct SliceViewIndex {
        covered: Option<(u64, usize)>,
        accesses: std::collections::HashMap<TypeReferenceHandle, [bool; 2]>,
    }

    impl PartialEq for SliceViewIndex {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for SliceViewIndex {}

    #[derive(Default)]
    struct PolicyShellIndex(std::sync::Mutex<PolicyShells>);

    #[derive(Default)]
    struct PolicyShells {
        covered: Option<((u64, usize), (u64, usize))>,
        unresolved: bool,
        shells: Vec<Vec<(TypeReferenceHandle, ArithmeticDomain, TypeReferenceHandle)>>,
    }

    impl Clone for PolicyShellIndex {
        fn clone(&self) -> Self {
            Self::default()
        }
    }

    impl PartialEq for PolicyShellIndex {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for PolicyShellIndex {}

    impl TypeReferenceTable {
        pub fn new() -> Self {
            Self {
                type_references: Arena::new(),
                type_reference_handles: Arena::new(),
                constraints: Arena::new(),
                constant_origin_spans: Arena::new(),
                constant_origins: Arena::new(),
                array_extent_arguments: Arena::new(),
                operand_lifetime_owners: Arena::new(),
                operand_lifetime_spans: Arena::new(),
                operand_lifetimes: Arena::new(),
                operand_origins: Arena::new(),
                named_rows: NamedRowIndex::default(),
                arithmetic_rows: ArithmeticRowIndex::default(),
                slice_views: SliceViewIndex::default(),
                policy_shells: PolicyShellIndex::default(),
                type_alias_applications: Arena::new(),
            }
        }

        pub fn revision(&self) -> u64 {
            self.type_references
                .revision()
                .wrapping_add(self.type_reference_handles.revision())
                .wrapping_add(self.constraints.revision())
                .wrapping_add(self.constant_origins.revision())
                .wrapping_add(self.constant_origin_spans.revision())
                .wrapping_add(self.array_extent_arguments.revision())
                .wrapping_add(self.operand_lifetime_owners.revision())
                .wrapping_add(self.operand_lifetime_spans.revision())
                .wrapping_add(self.operand_lifetimes.revision())
                .wrapping_add(self.operand_origins.revision())
                .wrapping_add(self.type_alias_applications.revision())
        }

        pub fn insert(&mut self, type_reference: TypeReferenceNode) -> TypeReferenceHandle {
            self.arithmetic_rows.0.take();
            let named = match &type_reference {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                _ => None,
            };
            let handle = self.type_references.insert(type_reference);
            let origins = self.constant_origin_spans.insert(HandleSpan::empty());
            let extent_argument = self
                .array_extent_arguments
                .insert(TypeReferenceHandle::invalid());
            debug_assert_eq!(handle.arena_index(), extent_argument.arena_index());
            debug_assert_eq!(handle.generation(), extent_argument.generation());
            let lifetime_owner = self.operand_lifetime_owners.insert(SymbolHandle::invalid());
            let lifetime_span = self.operand_lifetime_spans.insert(HandleSpan::empty());
            debug_assert_eq!(handle.arena_index(), lifetime_span.arena_index());
            let operand_origin = self.operand_origins.insert(TypeOperandOrigin::Lexical);
            debug_assert_eq!(handle.arena_index(), operand_origin.arena_index());
            debug_assert_eq!(handle.arena_index(), lifetime_owner.arena_index());
            debug_assert_eq!(handle.arena_index(), origins.arena_index());
            debug_assert_eq!(handle.generation(), origins.generation());
            if let Some(symbol) = named {
                self.note_named_row(symbol, handle);
            }
            handle
        }

        pub fn type_alias_applications(
            &self,
            reference: TypeReferenceHandle,
        ) -> impl Iterator<Item = &crate::type_aliases::TypeAliasApplication> {
            self.type_alias_applications
                .iter()
                .filter_map(move |(_, application)| {
                    (application.instance == reference).then_some(application)
                })
        }

        pub fn retain_type_alias_application(
            &mut self,
            application: crate::type_aliases::TypeAliasApplication,
        ) {
            if !self
                .type_alias_applications
                .iter()
                .any(|(_, row)| row == &application)
            {
                self.type_alias_applications.insert(application);
            }
        }

        pub fn fresh_occurrence(&mut self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
            let fresh = self.insert(self.type_reference(reference).clone());
            let source = Handle::from_parts(reference.arena_index(), reference.generation());
            let target = Handle::from_parts(fresh.arena_index(), fresh.generation());
            *self.constant_origin_spans.get_mut(target) = *self.constant_origin_spans.get(source);
            *self
                .array_extent_arguments
                .get_mut(Handle::from_parts(fresh.arena_index(), fresh.generation())) =
                self.array_extent_argument(reference);
            *self
                .operand_lifetime_owners
                .get_mut(Handle::from_parts(fresh.arena_index(), fresh.generation())) =
                self.operand_lifetime_owner(reference);
            *self
                .operand_lifetime_spans
                .get_mut(Handle::from_parts(fresh.arena_index(), fresh.generation())) =
                *self.operand_lifetime_spans.get(Handle::from_parts(
                    reference.arena_index(),
                    reference.generation(),
                ));
            self.set_operand_origin(fresh, self.operand_origin(reference));
            let applications = self
                .type_alias_applications(reference)
                .cloned()
                .collect::<Vec<_>>();
            for application in applications {
                self.retain_type_alias_application(crate::type_aliases::TypeAliasApplication {
                    instance: fresh,
                    ..application
                });
            }
            fresh
        }

        pub fn operand_origin(&self, reference: TypeReferenceHandle) -> TypeOperandOrigin {
            *self.operand_origins.get(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            ))
        }

        pub fn set_operand_origin(
            &mut self,
            reference: TypeReferenceHandle,
            origin: TypeOperandOrigin,
        ) {
            *self.operand_origins.get_mut(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            )) = origin;
        }

        pub fn retain_operand_lifetime_owner(
            &mut self,
            reference: TypeReferenceHandle,
            owner: SymbolHandle,
        ) -> Result<(), &'static str> {
            if !self.contains_type_reference(reference) {
                return Err("substituted operand has no registered type row");
            }
            let row = Handle::from_parts(reference.arena_index(), reference.generation());
            let retained = self.operand_lifetime_owners.get_mut(row);
            if !retained.is_valid() {
                *retained = owner;
            }
            Ok(())
        }

        pub fn operand_lifetime_owner(&self, reference: TypeReferenceHandle) -> SymbolHandle {
            if !self.contains_type_reference(reference) {
                return SymbolHandle::invalid();
            }
            *self.operand_lifetime_owners.get(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            ))
        }

        pub fn operand_lifetimes(
            &self,
            reference: TypeReferenceHandle,
        ) -> &[crate::type_identity::LifetimeParameterIdentity] {
            if !self.contains_type_reference(reference) {
                return &[];
            }
            let row = Handle::from_parts(reference.arena_index(), reference.generation());
            self.operand_lifetimes
                .span_or_empty(*self.operand_lifetime_spans.get(row))
        }

        pub fn has_resolved_operand_lifetimes(&self, reference: TypeReferenceHandle) -> bool {
            let lifetimes = self.operand_lifetimes(reference);
            let expected = match self.type_reference(reference) {
                TypeReferenceNode::Generic {
                    lifetime_arguments, ..
                } => lifetime_arguments.len(),
                TypeReferenceNode::Reference {
                    lifetime: Some(_), ..
                } => 1,
                _ => 0,
            };
            !lifetimes.is_empty()
                && lifetimes.len() == expected
                && lifetimes.iter().all(|region| region.declaration.is_valid())
        }

        pub fn retain_operand_lifetimes(
            &mut self,
            reference: TypeReferenceHandle,
            lifetimes: impl IntoIterator<Item = crate::type_identity::LifetimeParameterIdentity>,
        ) -> Result<(), &'static str> {
            if !self.contains_type_reference(reference) {
                return Err("resolved regions have no registered type row");
            }
            let lifetimes = lifetimes.into_iter().collect::<Vec<_>>();
            let expected = match self.type_reference(reference) {
                TypeReferenceNode::Generic {
                    lifetime_arguments, ..
                } => lifetime_arguments.len(),
                TypeReferenceNode::Reference {
                    lifetime: Some(_), ..
                } => 1,
                _ => 0,
            };
            if !lifetimes.is_empty() && lifetimes.len() != expected {
                return Err("resolved regions do not match the complete type lifetime roster");
            }
            let span = self.operand_lifetimes.insert_many(lifetimes);
            let row = Handle::from_parts(reference.arena_index(), reference.generation());
            *self.operand_lifetime_spans.get_mut(row) = span;
            Ok(())
        }

        pub fn attach_constant_origins(
            &mut self,
            reference: TypeReferenceHandle,
            origins: impl IntoIterator<Item = TypeConstantOrigin>,
        ) -> Result<(), &'static str> {
            let mut origins = origins.into_iter().peekable();
            if origins.peek().is_none() {
                return Ok(());
            }
            let mut combined = self.constant_origins(reference)?.to_vec();
            for origin in origins {
                if !combined.contains(&origin) {
                    combined.push(origin);
                }
            }
            *self.constant_origin_spans.get_mut(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            )) = self.constant_origins.insert_many(combined);
            Ok(())
        }

        pub fn constant_origins(
            &self,
            reference: TypeReferenceHandle,
        ) -> Result<&[TypeConstantOrigin], &'static str> {
            let owner = Handle::from_parts(reference.arena_index(), reference.generation());
            if !self.type_references.is_valid(reference)
                || !self.constant_origin_spans.is_valid(owner)
            {
                return Err("constant type origin has a stale receiving type");
            }
            let span = *self.constant_origin_spans.get(owner);
            let origins = self.constant_origins.span_or_empty(span);
            if origins.len() != span.len() {
                return Err("constant type origin has a stale occurrence span");
            }
            Ok(origins)
        }

        fn note_named_row(&mut self, symbol: SymbolHandle, handle: TypeReferenceHandle) {
            if let Some(rows) = self.named_rows.0.get_mut() {
                rows.entry(symbol)
                    .and_modify(|first| {
                        if handle.arena_index() < first.arena_index() {
                            *first = handle;
                        }
                    })
                    .or_insert(handle);
            }
        }

        fn rescan_named_row(&mut self, symbol: SymbolHandle) {
            let Some(rows) = self.named_rows.0.get_mut() else {
                return;
            };
            let first = self
                .type_references
                .iter()
                .find_map(|(handle, node)| match node {
                    TypeReferenceNode::Named {
                        symbol: candidate, ..
                    } if *candidate == symbol => Some(handle),
                    _ => None,
                });
            match first {
                Some(handle) => {
                    rows.insert(symbol, handle);
                }
                None => {
                    rows.remove(&symbol);
                }
            }
        }

        pub fn insert_type_reference_handles(
            &mut self,
            type_references: impl IntoIterator<Item = TypeReferenceHandle>,
        ) -> HandleSpan<TypeReferenceHandle> {
            self.type_reference_handles.insert_many(type_references)
        }

        crate::span_columns! {
            type_reference_handles(TypeReferenceHandle)
                reserve reserve_type_reference_handles("type reference handle span count overflow")
                set set_type_reference_handle_at_offset
                push push_type_reference_handle;
        }

        pub fn insert_constraints(
            &mut self,
            constraints: impl IntoIterator<Item = TypeConstraintNode>,
        ) -> HandleSpan<TypeConstraintNode> {
            self.arithmetic_rows.0.take();
            self.constraints.insert_many(constraints)
        }

        pub fn reserve_constraints(&mut self, count: u32) -> HandleSpan<TypeConstraintNode> {
            self.arithmetic_rows.0.take();
            self.constraints.insert_many(
                std::iter::repeat_with(TypeConstraintNode::default)
                    .take(usize::try_from(count).expect("type constraint span count overflow")),
            )
        }

        pub fn set_constraint_at_offset(
            &mut self,
            constraints: HandleSpan<TypeConstraintNode>,
            offset: u32,
            constraint: TypeConstraintNode,
        ) {
            self.arithmetic_rows.0.take();
            *self.constraints.get_mut(constraints.handle_at(offset)) = constraint;
        }

        pub fn push_constraint(
            &mut self,
            span: &mut HandleSpan<TypeConstraintNode>,
            constraint: TypeConstraintNode,
        ) {
            self.arithmetic_rows.0.take();
            self.constraints.append_to_span(span, constraint);
        }

        #[inline]
        pub fn type_reference(&self, handle: TypeReferenceHandle) -> &TypeReferenceNode {
            self.type_references.get(handle)
        }

        pub fn dereferenced_type_reference(
            &self,
            mut handle: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::Reference { referee, .. } = self.type_reference(handle) {
                handle = *referee;
            }
            handle
        }

        pub fn peeled_type_reference(
            &self,
            mut handle: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } = self.type_reference(handle)
            {
                handle = *referee;
            }
            handle
        }

        pub fn anonymous_numeric_literal_value(
            &self,
            handle: TypeReferenceHandle,
        ) -> Option<usize> {
            let TypeReferenceNode::Named { symbol, name } = self.type_reference(handle) else {
                return None;
            };
            if symbol.is_valid() {
                return None;
            }
            name.as_str().parse::<usize>().ok()
        }

        pub fn closed_nominal_symbol(&self, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
            match self.type_reference(handle) {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if self.type_reference_handles(*arguments).is_empty() => Some(*base_symbol),
                _ => None,
            }
        }

        pub fn unconstrained_type_reference(
            &self,
            mut handle: TypeReferenceHandle,
        ) -> TypeReferenceHandle {
            while let TypeReferenceNode::Constrained { base_type, .. } = self.type_reference(handle)
            {
                handle = *base_type;
            }
            handle
        }

        pub fn unconstrained_named_symbol(
            &self,
            handle: TypeReferenceHandle,
        ) -> Option<SymbolHandle> {
            match self.type_reference(self.unconstrained_type_reference(handle)) {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                _ => None,
            }
        }

        pub fn type_head_symbol(&self, handle: TypeReferenceHandle) -> Option<SymbolHandle> {
            match self.type_reference(handle) {
                TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
                TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
                _ => None,
            }
        }

        pub fn contains_type_reference(&self, handle: TypeReferenceHandle) -> bool {
            self.type_references.is_valid(handle)
        }

        pub fn fixed_array_lengths(
            &self,
        ) -> impl Iterator<Item = (TypeReferenceHandle, &FixedArrayLength)> {
            self.type_references
                .iter()
                .filter_map(|(handle, node)| match node {
                    TypeReferenceNode::FixedArray { length, .. } => Some((handle, length)),
                    _ => None,
                })
        }

        pub fn references(
            &self,
        ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
            self.type_references.iter()
        }

        pub fn references_from(
            &self,
            start_arena_index: usize,
        ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
            self.type_references
                .iter_from(start_arena_index.saturating_sub(1))
                .filter(move |(handle, _)| handle.arena_index() as usize >= start_arena_index)
        }

        pub fn named_references(
            &self,
        ) -> impl Iterator<Item = (TypeReferenceHandle, SymbolHandle, &str)> + '_ {
            self.type_references
                .iter()
                .filter_map(|(handle, node)| match node {
                    TypeReferenceNode::Named { symbol, name } => {
                        Some((handle, *symbol, name.as_str()))
                    }
                    _ => None,
                })
        }

        pub fn named_references_from(
            &self,
            start_arena_index: usize,
        ) -> impl Iterator<Item = (TypeReferenceHandle, SymbolHandle, &str)> + '_ {
            self.references_from(start_arena_index)
                .filter_map(|(handle, node)| match node {
                    TypeReferenceNode::Named { symbol, name } => {
                        Some((handle, *symbol, name.as_str()))
                    }
                    _ => None,
                })
        }

        pub fn array_extent_argument(&self, reference: TypeReferenceHandle) -> TypeReferenceHandle {
            if !self.contains_type_reference(reference) {
                return TypeReferenceHandle::invalid();
            }
            *self.array_extent_arguments.get(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            ))
        }

        pub fn retain_array_extent_argument(
            &mut self,
            reference: TypeReferenceHandle,
            argument: TypeReferenceHandle,
        ) -> Result<(), &'static str> {
            if !self.contains_type_reference(reference)
                || !self.contains_type_reference(argument)
                || reference == argument
                || !matches!(
                    self.type_reference(reference),
                    TypeReferenceNode::FixedArray { .. }
                )
            {
                return Err("array extent custody has no exact array and argument rows");
            }
            *self.array_extent_arguments.get_mut(Handle::from_parts(
                reference.arena_index(),
                reference.generation(),
            )) = argument;
            Ok(())
        }

        pub fn set_fixed_array_length(&mut self, handle: TypeReferenceHandle, value: usize) {
            if let TypeReferenceNode::FixedArray {
                length: FixedArrayLength::Expression(argument),
                ..
            } = self.type_reference(handle)
            {
                let argument = *argument;
                self.retain_array_extent_argument(handle, argument)
                    .expect("existing array extent child");
            }
            let TypeReferenceNode::FixedArray { length, .. } = self.type_references.get_mut(handle)
            else {
                panic!("set_fixed_array_length called on a non-fixed-array type reference");
            };
            *length = FixedArrayLength::Literal(value);
        }

        pub fn type_reference_handles(
            &self,
            span: HandleSpan<TypeReferenceHandle>,
        ) -> &[TypeReferenceHandle] {
            self.type_reference_handles.span_or_empty(span)
        }

        pub fn constraints(&self, span: HandleSpan<TypeConstraintNode>) -> &[TypeConstraintNode] {
            self.constraints.span_or_empty(span)
        }

        pub fn constrained_type_references(
            &self,
        ) -> Vec<(TypeReferenceHandle, HandleSpan<TypeConstraintNode>)> {
            self.type_references
                .iter()
                .filter_map(|(_, node)| match node {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => Some((*base_type, *constraints)),
                    _ => None,
                })
                .collect()
        }

        pub fn constrained_type_reference_sites(
            &self,
        ) -> Vec<(
            TypeReferenceHandle,
            TypeReferenceHandle,
            HandleSpan<TypeConstraintNode>,
        )> {
            self.type_references
                .iter()
                .filter_map(|(handle, node)| match node {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => Some((handle, *base_type, *constraints)),
                    _ => None,
                })
                .collect()
        }

        pub fn const_expression_sites(&self) -> Vec<(TypeReferenceHandle, ExpressionHandle)> {
            self.type_references
                .iter()
                .filter_map(|(handle, node)| match node {
                    TypeReferenceNode::ConstExpression(expression) => Some((handle, *expression)),
                    _ => None,
                })
                .collect()
        }

        pub fn set_constraint_span(
            &mut self,
            handle: TypeReferenceHandle,
            constraints: HandleSpan<TypeConstraintNode>,
        ) {
            self.arithmetic_rows.0.take();
            let TypeReferenceNode::Constrained {
                constraints: current,
                ..
            } = self.type_references.get_mut(handle)
            else {
                panic!("set_constraint_span called on a non-constrained type reference");
            };
            *current = constraints;
        }

        pub fn constraints_mut(
            &mut self,
            span: HandleSpan<TypeConstraintNode>,
        ) -> &mut [TypeConstraintNode] {
            self.arithmetic_rows.0.take();
            self.constraints.span_mut_or_empty(span)
        }

        pub fn constraint_span(
            &self,
            span: HandleSpan<TypeConstraintNode>,
        ) -> Option<&[TypeConstraintNode]> {
            self.constraints.span(span)
        }

        pub fn slice_view_references(
            &self,
        ) -> impl Iterator<
            Item = (
                TypeReferenceHandle,
                language_core::ReferenceAccess,
                TypeReferenceHandle,
            ),
        > + '_ {
            self.type_references.iter().filter_map(|(handle, node)| {
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = node
                else {
                    return None;
                };
                let TypeReferenceNode::Slice { element_type } = self.type_reference(*referee)
                else {
                    return None;
                };
                Some((handle, *access, *element_type))
            })
        }

        fn slice_view_accesses(&mut self, element: TypeReferenceHandle) -> [bool; 2] {
            let epoch = self.type_references.append_epoch();
            let start = match (epoch, self.slice_views.covered) {
                (Some((revision, length)), Some((covered_revision, covered)))
                    if revision == covered_revision && covered <= length =>
                {
                    covered
                }
                _ => {
                    self.slice_views.accesses.clear();
                    0
                }
            };
            for (_, node) in self.type_references.iter_from(start) {
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = node
                else {
                    continue;
                };
                let slot = match access {
                    ReferenceAccess::Shared => 0,
                    ReferenceAccess::Mutable => 1,
                    ReferenceAccess::WriteOnly => continue,
                };
                if let TypeReferenceNode::Slice { element_type } =
                    self.type_references.get(*referee)
                {
                    self.slice_views.accesses.entry(*element_type).or_default()[slot] = true;
                }
            }
            self.slice_views.covered = epoch;
            self.slice_views
                .accesses
                .get(&element)
                .copied()
                .unwrap_or_default()
        }

        pub fn type_reference_count(&self) -> usize {
            self.type_references.len()
        }

        pub fn type_reference_rows(
            &self,
        ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
            self.type_references.iter()
        }

        pub fn type_reference_rows_from(
            &self,
            start: usize,
        ) -> impl Iterator<Item = (TypeReferenceHandle, &TypeReferenceNode)> + '_ {
            self.type_references.iter_from(start)
        }

        pub fn retain_collection_result_types(
            &mut self,
            symbols: &symbols::SymbolTable,
            reference: TypeReferenceHandle,
        ) {
            let mut current = reference;
            let mut element = loop {
                match self.type_reference(current) {
                    TypeReferenceNode::Reference { referee, .. }
                    | TypeReferenceNode::Constrained {
                        base_type: referee, ..
                    } => current = *referee,
                    TypeReferenceNode::Slice { element_type }
                    | TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
                    _ => return,
                }
            };
            let policy = self.arithmetic_domain(reference);
            if self.element_inherits_collection_arithmetic_policy(element, symbols)
                && policy != ArithmeticDomain::Exact
                && self.arithmetic_domain(element) == ArithmeticDomain::Exact
            {
                element = match self.find_policy_qualified_type_reference(element, policy) {
                    Some(retained) => retained,
                    None => {
                        let constraints =
                            self.insert_constraints([TypeConstraintNode::ArithmeticDomain(policy)]);
                        self.insert(TypeReferenceNode::Constrained {
                            base_type: element,
                            constraints,
                        })
                    }
                };
                self.retain_collection_result_types(symbols, element);
            }
            let present = self.slice_view_accesses(element);
            for (access, exists) in [
                (ReferenceAccess::Shared, present[0]),
                (ReferenceAccess::Mutable, present[1]),
            ] {
                if exists {
                    continue;
                }
                let slice = self.insert(TypeReferenceNode::Slice {
                    element_type: element,
                });
                self.insert(TypeReferenceNode::Reference {
                    referee: slice,
                    access,
                    lifetime: None,
                });
            }
        }

        pub fn find_named_type_reference(
            &self,
            symbol: SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            self.named_rows
                .0
                .get_or_init(|| {
                    let mut rows = symbols::SymbolKeyMap::default();
                    for (handle, node) in self.type_references.iter() {
                        if let Some(symbol) = node.as_named_symbol() {
                            rows.entry(symbol).or_insert(handle);
                        }
                    }
                    rows
                })
                .get(&symbol)
                .copied()
        }

        pub fn find_arithmetic_result_type_reference(
            &self,
            carrier: SymbolHandle,
            domain: numerics::arithmetic::ArithmeticDomain,
        ) -> Option<TypeReferenceHandle> {
            use numerics::arithmetic::ArithmeticDomain;
            if !carrier.is_valid() {
                return None;
            }
            if domain == ArithmeticDomain::Exact {
                return self.find_named_type_reference(carrier);
            }
            self.arithmetic_rows
                .0
                .get_or_init(|| {
                    let mut rows = symbols::SymbolKeyMap::<
                        SymbolHandle,
                        Vec<(ArithmeticDomain, TypeReferenceHandle)>,
                    >::default();
                    for (handle, node) in self.type_references.iter() {
                        let TypeReferenceNode::Constrained {
                            base_type,
                            constraints,
                        } = node
                        else {
                            continue;
                        };
                        if !self.contains_type_reference(*base_type) {
                            continue;
                        }
                        let Some(symbol) = self.type_reference(*base_type).as_named_symbol() else {
                            continue;
                        };
                        let Some([TypeConstraintNode::ArithmeticDomain(retained)]) =
                            self.constraint_span(*constraints)
                        else {
                            continue;
                        };
                        let policies = rows.entry(symbol).or_default();
                        if !policies.iter().any(|(policy, _)| policy == retained) {
                            policies.push((*retained, handle));
                        }
                    }
                    rows
                })
                .get(&carrier)?
                .iter()
                .find_map(|(policy, handle)| (*policy == domain).then_some(*handle))
        }

        pub fn constraint_count(&self) -> usize {
            self.constraints.len()
        }

        pub fn find_policy_qualified_type_reference(
            &self,
            target: TypeReferenceHandle,
            domain: numerics::arithmetic::ArithmeticDomain,
        ) -> Option<TypeReferenceHandle> {
            if !self.contains_type_reference(target) {
                return None;
            }
            let mut index = self
                .policy_shells
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let rows = self.type_references.append_epoch();
            let constraints = self.constraints.append_epoch();
            let start = match (rows, constraints, index.covered) {
                (
                    Some((revision, length)),
                    Some((constraint_revision, constraint_length)),
                    Some((
                        (covered_revision, covered),
                        (covered_constraint_revision, covered_constraints),
                    )),
                ) if revision == covered_revision
                    && covered <= length
                    && constraint_revision == covered_constraint_revision
                    && (!index.unresolved || constraint_length == covered_constraints) =>
                {
                    covered
                }
                _ => {
                    index.shells.clear();
                    index.unresolved = false;
                    0
                }
            };
            for (handle, node) in self.type_references.iter_from(start) {
                let TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } = node
                else {
                    continue;
                };
                let Some(span) = self.constraint_span(*constraints) else {
                    index.unresolved = true;
                    continue;
                };
                let [TypeConstraintNode::ArithmeticDomain(retained)] = span else {
                    continue;
                };
                let slot = base_type.arena_index() as usize;
                if index.shells.len() <= slot {
                    index.shells.resize_with(slot + 1, Vec::new);
                }
                let shells = &mut index.shells[slot];
                if !shells
                    .iter()
                    .any(|(base, policy, _)| base == base_type && policy == retained)
                {
                    shells.push((*base_type, *retained, handle));
                }
            }
            index.covered = rows.zip(constraints);
            index
                .shells
                .get(target.arena_index() as usize)?
                .iter()
                .find_map(|(base, policy, handle)| {
                    (*base == target && *policy == domain).then_some(*handle)
                })
        }

        pub fn element_inherits_collection_arithmetic_policy(
            &self,
            mut element: TypeReferenceHandle,
            symbols: &symbols::SymbolTable,
        ) -> bool {
            let mut visited = Vec::new();
            while self.contains_type_reference(element) && !visited.contains(&element) {
                visited.push(element);
                match self.type_reference(element) {
                    TypeReferenceNode::Constrained { base_type, .. } => element = *base_type,
                    TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
                        return true;
                    }
                    TypeReferenceNode::Named { symbol, .. } => {
                        return matches!(
                            symbols.builtin_type_atom(*symbol),
                            Some(
                                symbols::BuiltinTypeAtom::Integer(_)
                                    | symbols::BuiltinTypeAtom::Address
                                    | symbols::BuiltinTypeAtom::F32
                                    | symbols::BuiltinTypeAtom::F64
                            )
                        );
                    }
                    _ => return false,
                }
            }
            false
        }

        pub fn display_name(&self, handle: TypeReferenceHandle) -> String {
            self.type_reference(handle).display_name(self)
        }

        pub fn display_name_with_constraints(
            &self,
            handle: TypeReferenceHandle,
            expressions: &crate::expression::ExpressionTable,
        ) -> String {
            self.type_reference(handle)
                .display_name_with_constraints(self, expressions)
        }

        #[inline]
        pub fn primitive_type(&self, handle: TypeReferenceHandle) -> Option<PrimitiveType> {
            self.type_reference(handle).primitive_type(self)
        }

        pub fn named_type(&self, handle: TypeReferenceHandle) -> Option<&Identifier> {
            if !handle.is_valid() {
                return None;
            }
            match self.type_reference(handle) {
                TypeReferenceNode::Reference { referee, .. } => self.named_type(*referee),
                TypeReferenceNode::Constrained { base_type, .. } => self.named_type(*base_type),
                TypeReferenceNode::Named { name, .. } => Some(name),
                _ => None,
            }
        }

        pub fn is_borrowed_byte_slice(&self, handle: TypeReferenceHandle) -> bool {
            let mut handle = handle;
            loop {
                if !handle.is_valid() {
                    return false;
                }
                match self.type_reference(handle) {
                    TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                    TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    TypeReferenceNode::Slice { element_type } => {
                        return self.primitive_type(*element_type) == Some(PrimitiveType::U8);
                    }
                    _ => return false,
                }
            }
        }

        pub fn arithmetic_domain(
            &self,
            handle: TypeReferenceHandle,
        ) -> numerics::arithmetic::ArithmeticDomain {
            self.type_reference(handle).arithmetic_domain(self)
        }

        pub fn type_symbol(&self, handle: TypeReferenceHandle) -> SymbolHandle {
            self.type_reference(handle).type_symbol(self)
        }
    }

    impl Default for TypeReferenceTable {
        fn default() -> Self {
            Self::new()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum TypeReferenceNode {
        Reference {
            referee: TypeReferenceHandle,
            access: language_core::ReferenceAccess,
            lifetime: Option<Identifier>,
        },
        Constrained {
            base_type: TypeReferenceHandle,
            constraints: HandleSpan<TypeConstraintNode>,
        },
        FixedArray {
            element_type: TypeReferenceHandle,
            length: FixedArrayLength,
        },
        Slice {
            element_type: TypeReferenceHandle,
        },
        Generic {
            base_symbol: SymbolHandle,
            base_name: Identifier,
            lifetime_arguments: Vec<Identifier>,
            arguments: HandleSpan<TypeReferenceHandle>,
        },
        ConstExpression(ExpressionHandle),
        DynamicTrait {
            symbol: SymbolHandle,
            name: Identifier,
            arguments: HandleSpan<TypeReferenceHandle>,
        },
        Named {
            symbol: SymbolHandle,
            name: Identifier,
        },
        #[default]
        Unit,
    }

    #[macro_export]
    macro_rules! constrained_type_reference_pair {
        ($actual:ident, $required:ident) => {
            (
                $crate::types::TypeReferenceNode::Constrained {
                    base_type: $actual,
                    ..
                },
                $crate::types::TypeReferenceNode::Constrained {
                    base_type: $required,
                    ..
                },
            )
        };
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum FixedArrayLength {
        Literal(usize),
        Expression(TypeReferenceHandle),
        ConstParameter {
            symbol: SymbolHandle,
            name: Identifier,
        },
        ConstCall {
            machine: SymbolHandle,
            name: Identifier,
            source_span: source::SourceSpan,
        },
    }

    impl Default for FixedArrayLength {
        fn default() -> Self {
            Self::Literal(0)
        }
    }

    impl From<usize> for FixedArrayLength {
        fn from(value: usize) -> Self {
            Self::Literal(value)
        }
    }

    impl fmt::Display for FixedArrayLength {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Literal(value) => write!(formatter, "{value}"),
                Self::Expression(_) => formatter.write_str("<constant expression>"),
                Self::ConstParameter { name, .. } => write!(formatter, "{name}"),
                Self::ConstCall { name, .. } => write!(formatter, "{name}()"),
            }
        }
    }

    impl TypeReferenceNode {
        pub fn primitive_type(&self, table: &TypeReferenceTable) -> Option<PrimitiveType> {
            match self {
                TypeReferenceNode::Reference { .. } => None,
                TypeReferenceNode::Constrained { base_type, .. } => {
                    table.primitive_type(*base_type)
                }
                TypeReferenceNode::Named { name, .. } => PrimitiveType::from_name(name),
                TypeReferenceNode::DynamicTrait { .. } => Some(PrimitiveType::Selector),
                TypeReferenceNode::FixedArray { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::Generic { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::Unit => None,
            }
        }

        pub fn arithmetic_domain(
            &self,
            table: &TypeReferenceTable,
        ) -> numerics::arithmetic::ArithmeticDomain {
            match self {
                TypeReferenceNode::Reference { referee, .. } => table.arithmetic_domain(*referee),
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => table
                    .constraints(*constraints)
                    .iter()
                    .find_map(|constraint| match constraint {
                        TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                        _ => None,
                    })
                    .unwrap_or_else(|| table.arithmetic_domain(*base_type)),
                _ => numerics::arithmetic::ArithmeticDomain::Exact,
            }
        }

        pub fn type_symbol(&self, table: &TypeReferenceTable) -> SymbolHandle {
            match self {
                TypeReferenceNode::Reference { referee, .. } => table.type_symbol(*referee),
                TypeReferenceNode::Constrained { base_type, .. } => table.type_symbol(*base_type),
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    table.type_symbol(*element_type)
                }
                TypeReferenceNode::Slice { element_type } => table.type_symbol(*element_type),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } => {
                    if PrimitiveType::from_name(base_name.as_str()).is_some() {
                        SymbolHandle::invalid()
                    } else {
                        *base_symbol
                    }
                }
                TypeReferenceNode::DynamicTrait { symbol, .. } => *symbol,
                TypeReferenceNode::ConstExpression(_) => SymbolHandle::invalid(),
                TypeReferenceNode::Named { symbol, name } => {
                    if PrimitiveType::from_name(name.as_str()).is_some() {
                        SymbolHandle::invalid()
                    } else {
                        *symbol
                    }
                }
                TypeReferenceNode::Unit => SymbolHandle::invalid(),
            }
        }

        pub fn as_named_symbol(&self) -> Option<SymbolHandle> {
            let TypeReferenceNode::Named { symbol, .. } = self else {
                return None;
            };
            Some(*symbol)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum TypeConstraintNode {
        Named(Identifier),

        ArithmeticDomain(numerics::arithmetic::ArithmeticDomain),
        Domain(DomainConstraint),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, compiler_storage::Record)]
    pub enum DomainConstraintSubject {
        #[default]
        Declared,
        Carry(language_semantics::CarryPermission),
        Value(language_semantics::value_domain::ValueDomain),
        OmegaLayout {
            grammar: OmegaLayoutGrammar,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum OmegaLayoutGrammar {
        Derived,
    }

    impl OmegaLayoutGrammar {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Derived => "derived",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
    pub struct DomainConstraint {
        pub name: Identifier,
        pub arguments: Vec<TypeReferenceHandle>,
        pub subject: DomainConstraintSubject,
        pub symbol: SymbolHandle,
        pub semantic_id: language_semantics::SemanticDomainId,
        pub classification: Option<language_semantics::DomainClassification>,
        pub predicate_presence: language_semantics::DomainPredicatePresence,
        pub semantic_roles: language_semantics::DomainSemanticRoles,
        pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
        pub source_selection: Option<DomainConstraintSource>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct DomainConstraintSource {
        pub source_span: source::SourceSpan,
        pub authored_exposure:
            Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure>,
    }

    impl DomainConstraint {
        pub fn as_str(&self) -> &str {
            self.name.as_str()
        }
    }

    impl fmt::Display for DomainConstraint {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.name.fmt(formatter)
        }
    }

    impl Default for TypeConstraintNode {
        fn default() -> Self {
            Self::Named(Identifier::default())
        }
    }

    /*
        The scalar a type reference denotes. A fixed-width integer is its
        carrier (sign and width); the built-in names are spellings of it,
        so nothing here lists widths.
    */
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum PrimitiveType {
        Bool,
        F32,
        F64,
        Integer(FixedInteger),
        Addr,
        Selector,
    }

    impl PrimitiveType {
        pub const U8: Self = Self::Integer(FixedInteger::U8);
        pub const U64: Self = Self::Integer(FixedInteger::U64);

        pub fn from_builtin_atom(atom: symbols::BuiltinTypeAtom) -> Option<Self> {
            atom.fixed_integer().map(Self::Integer)
        }

        /* The carrier of a fixed-width integer; the address is not one. */
        #[inline]
        pub fn fixed_integer(self) -> Option<FixedInteger> {
            match self {
                Self::Integer(carrier) => Some(carrier),
                Self::Bool | Self::F32 | Self::F64 | Self::Addr | Self::Selector => None,
            }
        }

        #[inline]
        pub fn is_integer(&self) -> bool {
            matches!(self, Self::Integer(_))
        }

        #[inline]
        pub fn is_unsigned_integer(self) -> bool {
            matches!(self, Self::Integer(carrier) if !carrier.is_signed())
        }

        #[inline]
        pub fn from_name(name: &str) -> Option<Self> {
            if !matches!(
                name.as_bytes().first(),
                Some(b'b' | b'f' | b'i' | b'u' | b'a' | b'A')
            ) {
                return None;
            }
            match name {
                "bool" => Some(Self::Bool),
                "f32" => Some(Self::F32),
                "f64" => Some(Self::F64),
                "addr" => Some(Self::Addr),
                "AtomicBool" => Some(Self::Bool),
                "AtomicU32" => Some(Self::Integer(FixedInteger::unsigned(32))),
                "AtomicU64" => Some(Self::U64),
                name if name.starts_with("AtomicBool#AccessField<") => Some(Self::Bool),
                name if name.starts_with("AtomicU32#AccessField<") => {
                    Some(Self::Integer(FixedInteger::unsigned(32)))
                }
                name if name.starts_with("AtomicU64#AccessField<") => Some(Self::U64),
                name => FixedInteger::from_builtin_name(name).map(Self::Integer),
            }
        }

        pub fn name(self) -> String {
            self.to_string()
        }

        /*
            Host-integer endpoints, or `None` for a carrier whose endpoints
            exceed them; such a caller refuses the width.
        */
        pub fn integer_bounds(self) -> Option<(i128, i128)> {
            match self {
                Self::Integer(carrier) => carrier.bounds_i128(),
                Self::Addr => FixedInteger::U64.bounds_i128(),
                Self::Bool | Self::F32 | Self::F64 | Self::Selector => None,
            }
        }

        pub fn accepts_integer_literal(self) -> bool {
            self.accepts_range_constraint() && !self.accepts_finite_constraint()
        }

        pub fn accepts_float_literal(self) -> bool {
            matches!(self, Self::F32 | Self::F64)
        }

        pub fn is_fixed_integer(self) -> bool {
            self.is_integer()
        }

        pub fn is_signed_integer(self) -> bool {
            match self {
                Self::Integer(carrier) => carrier.is_signed(),
                Self::Bool | Self::F32 | Self::F64 => true,
                Self::Addr | Self::Selector => false,
            }
        }

        /*
            Bytes of the native scalar. An integer whose width has no native
            scalar yet answers `None`, and its user refuses that width.
        */
        pub fn scalar_byte_size(self) -> Option<usize> {
            match self {
                Self::Bool => Some(1),
                Self::Integer(carrier) => carrier.native_scalar_bytes(),
                Self::F32 => Some(4),
                Self::F64 | Self::Addr | Self::Selector => Some(8),
            }
        }

        pub fn accepts_range_constraint(self) -> bool {
            matches!(self, Self::F32 | Self::F64 | Self::Integer(_) | Self::Addr)
        }

        pub fn accepts_finite_constraint(self) -> bool {
            matches!(self, Self::F32 | Self::F64)
        }

        pub fn from_landed_integer(kind: numerics::literals::LandedIntegerType) -> Self {
            match kind {
                numerics::literals::LandedIntegerType::Fixed(carrier) => Self::Integer(carrier),
                numerics::literals::LandedIntegerType::Addr => Self::Addr,
            }
        }

        pub fn landed_landing(self) -> Option<numerics::literals::LandedIntegerType> {
            use numerics::literals::LandedIntegerType;
            match self {
                Self::Integer(carrier) => Some(LandedIntegerType::Fixed(carrier)),
                Self::Addr => Some(LandedIntegerType::Addr),
                Self::Selector | Self::Bool | Self::F32 | Self::F64 => None,
            }
        }

        pub fn landed_landing_fixed(self) -> Option<numerics::literals::LandedIntegerType> {
            self.fixed_integer()
                .map(numerics::literals::LandedIntegerType::Fixed)
        }

        pub fn non_address_landed_integer(
            kind: numerics::literals::LandedIntegerType,
        ) -> Option<Self> {
            match kind {
                numerics::literals::LandedIntegerType::Addr => None,
                kind => Some(Self::from_landed_integer(kind)),
            }
        }
    }

    impl std::fmt::Display for PrimitiveType {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Bool => formatter.write_str("bool"),
                Self::F32 => formatter.write_str("f32"),
                Self::F64 => formatter.write_str("f64"),
                Self::Integer(carrier) => carrier.fmt(formatter),
                Self::Addr => formatter.write_str("addr"),
                Self::Selector => formatter.write_str("selector"),
            }
        }
    }

    pub fn builtin_type_atom(
        kind: numerics::literals::LandedIntegerType,
    ) -> symbols::BuiltinTypeAtom {
        match kind {
            numerics::literals::LandedIntegerType::Fixed(carrier) => {
                symbols::BuiltinTypeAtom::Integer(carrier)
            }
            numerics::literals::LandedIntegerType::Addr => symbols::BuiltinTypeAtom::Address,
        }
    }

    impl compiler_storage::Record for NamedRowIndex {
        fn write(
            &self,
            _: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            Ok(())
        }
        fn read<Context>(
            _: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Ok(Self::default())
        }
    }

    impl compiler_storage::Record for ArithmeticRowIndex {
        fn write(
            &self,
            _: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            Ok(())
        }
        fn read<Context>(
            _: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Ok(Self::default())
        }
    }

    impl compiler_storage::Record for SliceViewIndex {
        fn write(
            &self,
            _: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            Ok(())
        }
        fn read<Context>(
            _: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Ok(Self::default())
        }
    }

    impl compiler_storage::Record for PolicyShellIndex {
        fn write(
            &self,
            _: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            Ok(())
        }
        fn read<Context>(
            _: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            Ok(Self::default())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            FixedArrayLength, PolicyShellIndex, SliceViewIndex, TypeConstraintNode,
            TypeOperandOrigin, TypeReferenceNode, TypeReferenceTable,
        };
        use crate::name::Identifier;
        use language_core::ReferenceAccess;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        /*
            A built-in integer name denotes its carrier and nothing else, so
            every question about it is answered from the sign and width. The
            twins are the names that are not carriers and the widths the
            front end does not admit yet.
        */
        #[test]
        fn a_built_in_integer_name_is_its_sign_and_width() {
            use super::PrimitiveType;
            use numerics::integer_carrier::FixedInteger;
            use numerics::literals::LandedIntegerType;

            for (name, carrier, bytes) in [
                ("i8", FixedInteger::signed(8), 1),
                ("i16", FixedInteger::signed(16), 2),
                ("i32", FixedInteger::signed(32), 4),
                ("i64", FixedInteger::signed(64), 8),
                ("u8", FixedInteger::unsigned(8), 1),
                ("u16", FixedInteger::unsigned(16), 2),
                ("u32", FixedInteger::unsigned(32), 4),
                ("u64", FixedInteger::unsigned(64), 8),
            ] {
                let primitive = PrimitiveType::from_name(name).unwrap();
                assert_eq!(primitive, PrimitiveType::Integer(carrier));
                assert_eq!(primitive.name(), name);
                assert_eq!(primitive.fixed_integer(), Some(carrier));
                assert_eq!(primitive.is_signed_integer(), carrier.is_signed());
                assert_eq!(primitive.is_unsigned_integer(), !carrier.is_signed());
                assert_eq!(primitive.scalar_byte_size(), Some(bytes));
                assert_eq!(primitive.integer_bounds(), carrier.bounds_i128());
                assert_eq!(
                    primitive.landed_landing(),
                    Some(LandedIntegerType::Fixed(carrier))
                );
                assert_eq!(
                    PrimitiveType::from_builtin_atom(symbols::BuiltinTypeAtom::Integer(carrier)),
                    Some(primitive)
                );
                assert_eq!(
                    super::builtin_type_atom(LandedIntegerType::Fixed(carrier)),
                    symbols::BuiltinTypeAtom::Integer(carrier)
                );
            }
            assert_eq!(PrimitiveType::U8, PrimitiveType::from_name("u8").unwrap());
            assert_eq!(PrimitiveType::U64, PrimitiveType::from_name("u64").unwrap());

            /* The address has the unsigned 64-bit range and is not an integer. */
            let address = PrimitiveType::from_name("addr").unwrap();
            assert_eq!(address.fixed_integer(), None);
            assert!(!address.is_integer() && !address.is_unsigned_integer());
            assert_eq!(
                address.integer_bounds(),
                PrimitiveType::U64.integer_bounds()
            );
            assert_eq!(address.landed_landing(), Some(LandedIntegerType::Addr));
            assert_eq!(address.landed_landing_fixed(), None);

            for name in ["u", "i", "u0", "u13", "i1", "u128", "U32", "usize", "u032"] {
                assert_eq!(PrimitiveType::from_name(name), None, "{name}");
            }
            /* A width with no native scalar has no byte size to hand out. */
            let odd = PrimitiveType::Integer(FixedInteger::unsigned(13));
            assert_eq!(odd.scalar_byte_size(), None);
            assert_eq!(odd.name(), "u13");
            assert_eq!(odd.integer_bounds(), Some((0, 8191)));
        }

        #[test]
        fn type_operand_origin_survives_copy_and_whole_substitution() {
            let mut source = TypeReferenceTable::new();
            let reference = source.insert(TypeReferenceNode::Unit);
            let origin = TypeOperandOrigin::MachineParameter {
                instance: symbols::SymbolHandle::from_parts(1, 1),
                parameter: symbols::SymbolHandle::from_parts(2, 1),
            };
            source.set_operand_origin(reference, origin);
            let source_expressions = crate::expression::ExpressionTable::default();
            let mut target_expressions = crate::expression::ExpressionTable::default();
            let mut target = TypeReferenceTable::new();
            let copied = target.copy_from(
                &source,
                &source_expressions,
                &mut target_expressions,
                reference,
            );
            assert_eq!(target.operand_origin(copied), origin);
            let receiving = target.insert(TypeReferenceNode::Unit);
            target.substitute_from(receiving, copied).unwrap();
            assert_eq!(target.operand_origin(receiving), origin);
            let lexical = target.insert(TypeReferenceNode::Unit);
            target.substitute_from(receiving, lexical).unwrap();
            assert_eq!(target.operand_origin(receiving), TypeOperandOrigin::Lexical);
        }

        #[test]
        fn type_operand_origin_changes_retained_input_and_revision() {
            let mut table = TypeReferenceTable::new();
            let reference = table.insert(TypeReferenceNode::Unit);
            let before = table.clone();
            let revision = table.revision();
            table.set_operand_origin(
                reference,
                TypeOperandOrigin::MachineParameter {
                    instance: symbols::SymbolHandle::from_parts(1, 1),
                    parameter: symbols::SymbolHandle::from_parts(2, 1),
                },
            );
            assert_ne!(table, before);
            assert_ne!(table.revision(), revision);
            assert_ne!(
                table.operand_origin(reference),
                before.operand_origin(reference)
            );
            assert_eq!(
                table.type_reference(reference),
                before.type_reference(reference)
            );
        }

        #[test]
        fn arithmetic_rows_preserve_first_match_and_follow_mutation() {
            use numerics::arithmetic::ArithmeticDomain::{Saturating, Trapping, Wrapping};
            let mut builder = SymbolTableBuilder::default();
            let carrier = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("u64"));
            let replacement = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("u32"));
            let mut table = TypeReferenceTable::new();
            macro_rules! found {
                ($carrier:expr => $domain:expr => $expected:expr) => {
                    assert_eq!(
                        table.find_arithmetic_result_type_reference($carrier, $domain),
                        $expected
                    );
                };
            }
            let base_type = table.insert(TypeReferenceNode::Named {
                symbol: carrier,
                name: Identifier::from("u64"),
            });
            found!(carrier => Wrapping => None);
            let constraints =
                table.insert_constraints([TypeConstraintNode::ArithmeticDomain(Wrapping)]);
            let first = table.insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            });
            let duplicate = table.insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            });
            let before_lookup = table.clone();
            let before_rendering = format!("{table:?}");
            found!(carrier => Wrapping => Some(first));
            assert_eq!(table, before_lookup);
            assert_eq!(format!("{table:?}"), before_rendering);

            table.substitute_node(first, TypeReferenceNode::Unit);
            found!(carrier => Wrapping => Some(duplicate));
            table.set_constraint_at_offset(
                constraints,
                0,
                TypeConstraintNode::ArithmeticDomain(Saturating),
            );
            found!(carrier => Wrapping => None);
            found!(carrier => Saturating => Some(duplicate));
            table.constraints_mut(constraints)[0] = TypeConstraintNode::ArithmeticDomain(Trapping);
            found!(carrier => Saturating => None);
            found!(carrier => Trapping => Some(duplicate));

            table.substitute_node(
                base_type,
                TypeReferenceNode::Named {
                    symbol: replacement,
                    name: Identifier::from("u32"),
                },
            );
            found!(carrier => Trapping => None);
            found!(replacement => Trapping => Some(duplicate));
            let extra = table.insert_constraints([
                TypeConstraintNode::ArithmeticDomain(Trapping),
                TypeConstraintNode::Named(Identifier::from("Extra")),
            ]);
            table.set_constraint_span(duplicate, extra);
            found!(replacement => Trapping => None);
        }

        #[test]
        fn slice_view_index_agrees_with_a_row_scan_through_appends_and_rewrites() {
            let mut builder = SymbolTableBuilder::default();
            let carrier = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("u8"));
            let mut table = TypeReferenceTable::new();
            let element = table.insert(TypeReferenceNode::Named {
                symbol: carrier,
                name: Identifier::from("u8"),
            });
            let other = table.insert(TypeReferenceNode::Unit);
            let scan = |table: &TypeReferenceTable, element| {
                let mut present = [false; 2];
                for (_, access, candidate) in table.slice_view_references() {
                    match access {
                        _ if candidate != element => {}
                        ReferenceAccess::Shared => present[0] = true,
                        ReferenceAccess::Mutable => present[1] = true,
                        ReferenceAccess::WriteOnly => {}
                    }
                }
                present
            };
            macro_rules! agree {
            ($($element:expr => $expected:expr),+) => {
                $(
                    assert_eq!(table.slice_view_accesses($element), $expected);
                    assert_eq!(scan(&table, $element), $expected);
                )+
            };
        }
            agree!(element => [false, false], other => [false, false]);
            let slice = table.insert(TypeReferenceNode::Slice {
                element_type: element,
            });
            table.insert(TypeReferenceNode::Reference {
                referee: slice,
                access: ReferenceAccess::Shared,
                lifetime: None,
            });
            agree!(element => [true, false], other => [false, false]);
            let mutable = table.insert(TypeReferenceNode::Reference {
                referee: slice,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
            table.insert(TypeReferenceNode::Reference {
                referee: slice,
                access: ReferenceAccess::WriteOnly,
                lifetime: None,
            });
            agree!(element => [true, true]);

            table.substitute_node(mutable, TypeReferenceNode::Unit);
            agree!(element => [true, false]);
            table.substitute_node(
                slice,
                TypeReferenceNode::Slice {
                    element_type: other,
                },
            );
            agree!(element => [false, false], other => [true, false]);

            let mut without_index = table.clone();
            without_index.slice_views = SliceViewIndex::default();
            assert_eq!(table, without_index);
            assert_eq!(format!("{table:?}"), format!("{without_index:?}"));
        }

        #[test]
        fn policy_shell_index_agrees_with_a_row_scan_through_appends_and_rewrites() {
            use numerics::arithmetic::ArithmeticDomain::{self, Saturating, Trapping, Wrapping};
            let mut table = TypeReferenceTable::new();
            let target = table.insert(TypeReferenceNode::Unit);
            let other = table.insert(TypeReferenceNode::Unit);
            let scan = |table: &TypeReferenceTable, target, domain| {
                table.type_references.iter().find_map(|(handle, node)| {
                    let TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } = node
                    else {
                        return None;
                    };
                    (*base_type == target
                        && matches!(table.constraint_span(*constraints)?,
                        [TypeConstraintNode::ArithmeticDomain(retained)] if *retained == domain))
                    .then_some(handle)
                })
            };
            macro_rules! agree {
            ($($target:expr, $domain:expr => $expected:expr);+) => {
                $(
                    assert_eq!(table.find_policy_qualified_type_reference($target, $domain), $expected);
                    assert_eq!(scan(&table, $target, $domain), $expected);
                )+
            };
        }
            let shell = |table: &mut TypeReferenceTable, base_type, domain: ArithmeticDomain| {
                let constraints =
                    table.insert_constraints([TypeConstraintNode::ArithmeticDomain(domain)]);
                let handle = table.insert(TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                });
                (handle, constraints)
            };
            agree!(target, Wrapping => None; other, Wrapping => None);
            let (first, first_constraints) = shell(&mut table, target, Wrapping);
            agree!(target, Wrapping => Some(first); target, Saturating => None);
            let (repeat, _) = shell(&mut table, target, Wrapping);
            let (saturating, _) = shell(&mut table, target, Saturating);
            let (on_other, _) = shell(&mut table, other, Wrapping);
            agree!(
                target, Wrapping => Some(first);
                target, Saturating => Some(saturating);
                other, Wrapping => Some(on_other)
            );

            table.set_constraint_at_offset(
                first_constraints,
                0,
                TypeConstraintNode::ArithmeticDomain(Trapping),
            );
            agree!(target, Wrapping => Some(repeat); target, Trapping => Some(first));
            table.substitute_node(on_other, TypeReferenceNode::Unit);
            agree!(other, Wrapping => None; target, Trapping => Some(first));

            let mut without_index = table.clone();
            without_index.policy_shells = PolicyShellIndex::default();
            assert_eq!(table, without_index);
            assert_eq!(format!("{table:?}"), format!("{without_index:?}"));
        }

        #[test]
        fn collection_result_views_are_added_once_per_element() {
            let mut builder = SymbolTableBuilder::default();
            let carrier = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("u8"));
            let symbols = builder.finish();
            let mut table = TypeReferenceTable::new();
            let element = table.insert(TypeReferenceNode::Named {
                symbol: carrier,
                name: Identifier::from("u8"),
            });
            let array = table.insert(TypeReferenceNode::FixedArray {
                element_type: element,
                length: FixedArrayLength::Literal(4),
            });
            table.retain_collection_result_types(&symbols, array);
            assert_eq!(table.slice_view_accesses(element), [true, true]);
            let rows = table.type_reference_count();
            table.retain_collection_result_types(&symbols, array);
            assert_eq!(table.type_reference_count(), rows);
        }

        #[test]
        fn instantiation_rebuilds_only_rows_that_name_a_binder() {
            let mut builder = SymbolTableBuilder::default();
            let binder = builder.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
            let item = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Item"));
            let other = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Other"));
            let mut table = TypeReferenceTable::new();
            let formal = table.insert(TypeReferenceNode::Named {
                symbol: binder,
                name: Identifier::from("T"),
            });
            let authored = TypeReferenceNode::Reference {
                referee: formal,
                access: ReferenceAccess::Shared,
                lifetime: None,
            };
            let borrowed = table.insert(authored.clone());
            let unrelated = table.insert(TypeReferenceNode::Named {
                symbol: other,
                name: Identifier::from("Other"),
            });
            let actual = table.insert(TypeReferenceNode::Named {
                symbol: item,
                name: Identifier::from("Item"),
            });
            let substitutions = [(binder, actual)];

            let instantiated = table
                .instantiate(borrowed, &substitutions)
                .expect("a reference to the binder instantiates");
            assert_ne!(instantiated, borrowed);
            assert_eq!(
                table.type_reference(instantiated),
                &TypeReferenceNode::Reference {
                    referee: actual,
                    access: ReferenceAccess::Shared,
                    lifetime: None,
                }
            );
            assert_eq!(table.type_reference(borrowed), &authored);
            assert_eq!(table.instantiate(unrelated, &substitutions), Ok(unrelated));
            assert_eq!(table.instantiate(formal, &substitutions), Ok(actual));
        }

        #[test]
        fn instantiation_refuses_a_binder_spelling_an_array_length() {
            let mut builder = SymbolTableBuilder::default();
            let binder = builder.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("N"));
            let element = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Item"));
            let mut table = TypeReferenceTable::new();
            let element = table.insert(TypeReferenceNode::Named {
                symbol: element,
                name: Identifier::from("Item"),
            });
            let array = table.insert(TypeReferenceNode::FixedArray {
                element_type: element,
                length: FixedArrayLength::ConstParameter {
                    symbol: binder,
                    name: Identifier::from("N"),
                },
            });
            assert!(table.instantiate(array, &[(binder, element)]).is_err());
        }

        /// `u64 in Live<T>` and its refused twin `u64 in Live<[Item; T]>`,
        /// both applied at `T := Item`.
        #[test]
        fn instantiation_applies_the_substitution_to_domain_arguments() {
            let mut builder = SymbolTableBuilder::default();
            let binder = builder.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Static("T"));
            let item = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Item"));
            let carrier = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static("Carrier"));
            let mut table = TypeReferenceTable::new();
            let mut named = |symbol, name: &'static str| {
                table.insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::from(name),
                })
            };
            let formal = named(binder, "T");
            let actual = named(item, "Item");
            let carrier = named(carrier, "Carrier");
            let constrained = |table: &mut TypeReferenceTable, argument| {
                let constraints = table.insert_constraints([
                    TypeConstraintNode::Named(Identifier::from("Plain")),
                    TypeConstraintNode::Domain(crate::types::DomainConstraint {
                        name: Identifier::from("Live"),
                        arguments: vec![argument, carrier],
                        ..Default::default()
                    }),
                ]);
                let reference = table.insert(TypeReferenceNode::Constrained {
                    base_type: carrier,
                    constraints,
                });
                (reference, constraints)
            };
            let substitutions = [(binder, actual)];

            let (open, authored) = constrained(&mut table, formal);
            let applied = table
                .instantiate(open, &substitutions)
                .expect("a domain argument naming the binder instantiates");
            assert_ne!(applied, open);
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = table.type_reference(applied).clone()
            else {
                panic!("the applied type keeps its constraint")
            };
            assert_eq!(base_type, carrier);
            let domain_arguments = |table: &TypeReferenceTable, span| {
                let [
                    TypeConstraintNode::Named(_),
                    TypeConstraintNode::Domain(domain),
                ] = table.constraints(span)
                else {
                    panic!("both constraints stay in their authored order")
                };
                domain.arguments.clone()
            };
            assert_eq!(domain_arguments(&table, constraints), [actual, carrier]);
            // The template keeps the arguments it was written with.
            assert_eq!(domain_arguments(&table, authored), [formal, carrier]);

            // A constraint that names no binder is the same row afterwards.
            let (closed, _) = constrained(&mut table, actual);
            assert_eq!(table.instantiate(closed, &substitutions), Ok(closed));

            // The binder as an array length inside the argument has no type
            // to become, so the whole application is refused.
            let array = table.insert(TypeReferenceNode::FixedArray {
                element_type: actual,
                length: FixedArrayLength::ConstParameter {
                    symbol: binder,
                    name: Identifier::from("T"),
                },
            });
            let (unsubstitutable, _) = constrained(&mut table, array);
            assert_eq!(
                table.instantiate(unsubstitutable, &substitutions),
                Err("a trait parameter as an array length has no instantiation")
            );
        }
    }
}

pub use type_predicates::CaseMembershipTypeFacts;

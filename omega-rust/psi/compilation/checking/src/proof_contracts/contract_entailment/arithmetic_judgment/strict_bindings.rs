//! Strict-arithmetic symbol bindings: engine construction from declared
//! strict bindings, scoped-value normalization under the original declaration
//! and selected arithmetic policy, and admitted operator occurrence bindings.
use super::super::{
    BigInt, BinaryOperator, ExpressionHandle, ExpressionNode, Machine,
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding, StrictArithmeticSymbolBinding,
    SymbolHandle,
};
use super::{Engine, OpaqueTerm, Polynomial};

impl<'program> Engine<'program> {
    /// Actuals are completed in the caller before this complete formal roster
    /// becomes visible. Source operations then simplify their applied values,
    /// never a callee binder read through caller facts. Occurrence bindings
    /// belong to the caller invocation and cannot enter the formal syntax.
    pub(in crate::proof_contracts::contract_entailment) fn with_formal_arguments<Output>(
        &mut self,
        owner: SymbolHandle,
        arguments: &[(SymbolHandle, Polynomial)],
        query: impl FnOnce(&mut Self) -> Option<Output>,
    ) -> Option<Output> {
        if !owner.is_valid() || arguments.iter().any(|(symbol, _)| !symbol.is_valid()) {
            return None;
        }
        let previous_owner = self.source_owner;
        let previous_arguments = self.formal_arguments.replace(arguments.to_vec());
        let previous_occurrences = std::mem::take(&mut self.strict_occurrences);
        let previous_conditions = std::mem::take(&mut self.saved_conditions);
        self.source_owner = owner;
        let value = query(self);
        self.source_owner = previous_owner;
        self.formal_arguments = previous_arguments;
        self.strict_occurrences = previous_occurrences;
        self.saved_conditions = previous_conditions;
        value
    }

    pub(in crate::proof_contracts::contract_entailment) fn integer_value_fit(
        &mut self,
        value: &Polynomial,
        minimum: &BigInt,
        maximum: &Polynomial,
        maximum_is_exclusive: bool,
        target_address_model: bool,
    ) -> super::super::operand_fits::CarrierFit {
        use super::super::operand_fits::CarrierFit;
        if self.requires_unsatisfiable {
            return CarrierFit::Unknown;
        }
        let value = self.substituted(value);
        let maximum = self.substituted(maximum);
        if self.prove_at_least(&value, minimum)
            && self.prove_at_least(
                &maximum.sub(&value),
                &BigInt::from_i64(i64::from(maximum_is_exclusive)),
            )
        {
            return CarrierFit::Proven;
        }
        if self.prove_at_least(
            &Polynomial::constant(minimum.clone()).sub(&value),
            &BigInt::from_i64(1),
        ) || self.prove_at_least(
            &value.sub(&maximum),
            &BigInt::from_i64(i64::from(!maximum_is_exclusive)),
        ) {
            return CarrierFit::Refuted;
        }
        if target_address_model
            && maximum_is_exclusive
            && minimum == &BigInt::zero()
            && self
                .address_model_bound()
                .is_some_and(|bound| bound == maximum)
            && let Some(value) = value.constant_value()
            && value.to_u64().is_some()
        {
            return CarrierFit::Pending {
                low: value.clone(),
                high: value,
            };
        }
        CarrierFit::Unknown
    }

    pub(in crate::proof_contracts::contract_entailment) fn strict_for_declaration(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
    ) -> Self {
        let mut engine = Self::for_explicit_bindings(program);
        engine.machine_symbol = owner;
        engine.source_owner = owner;
        engine
    }

    /// Start from the supplied bindings alone: they define the complete atom
    /// roster, so no machine parameter atom is scanned and then discarded.
    pub(in crate::proof_contracts::contract_entailment) fn strict(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        bindings: &[StrictArithmeticSymbolBinding],
    ) -> Self {
        let mut engine = Self::for_owner(program, machine.symbol, None);
        let mut resolved = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let polynomial = match &binding.value {
                StrictArithmeticBindingValue::Atom { identity, unsigned } => {
                    engine.declare_atom(identity, *unsigned);
                    Polynomial::atom(identity.clone())
                }
                StrictArithmeticBindingValue::Integer(value) => Polynomial::constant(value.clone()),
            };
            if let Some((_, existing)) = resolved
                .iter()
                .find(|(symbol, _)| *symbol == binding.symbol)
            {
                if existing != &polynomial {
                    // A contradictory binding set must invalidate even a
                    // constant-only goal, not degrade to an empty table.
                    engine.strict_symbol_bindings_valid = false;
                    return engine;
                }
                continue;
            }
            resolved.push((binding.symbol, polynomial));
        }
        engine.strict_symbol_bindings = Some(resolved);
        engine
    }

    /// Install one match-arm integer. A bare name replaces that symbol, including
    /// a saved initializer of the same local. A field replaces the canonical
    /// place so every occurrence of that field in the arm sees the pattern.
    /// Other subjects are left untouched: a window that does not read them
    /// still has its ordinary extent.
    pub(in crate::proof_contracts::contract_entailment) fn bind_matched_integer_subject(
        &mut self,
        expression: ExpressionHandle,
        value: BigInt,
    ) {
        if let Some(symbol) = crate::single_segment_name_symbol(self.program, expression) {
            self.replace_strict_symbol(symbol, Polynomial::constant(value));
            return;
        }
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) {
            return;
        }
        let atom = format!(
            "\0{}",
            super::integer_source_identity(self.program, expression)
        );
        self.substitutions.insert(atom, Polynomial::constant(value));
    }

    fn replace_strict_symbol(&mut self, symbol: SymbolHandle, value: Polynomial) {
        let Some(bindings) = &mut self.strict_symbol_bindings else {
            return;
        };
        if let Some((_, existing)) = bindings.iter_mut().find(|(bound, _)| *bound == symbol) {
            *existing = value;
            return;
        }
        bindings.push((symbol, value));
    }

    pub(in crate::proof_contracts::contract_entailment) fn strict_symbol_bindings_are_valid(
        &self,
    ) -> bool {
        self.strict_symbol_bindings_valid
    }

    /// Read a coordinate from this exact strict declaration frame. A caller
    /// must first establish that the completed observation names this symbol.
    pub(in crate::proof_contracts::contract_entailment) fn strict_symbol_value(
        &self,
        symbol: SymbolHandle,
    ) -> Option<Polynomial> {
        if !self.strict_symbol_bindings_valid {
            return None;
        }
        self.strict_symbol_bindings
            .as_ref()?
            .iter()
            .find(|(bound, _)| *bound == symbol)
            .map(|(_, value)| value.clone())
    }

    /// Copy captured closed values into a fresh judgment without its live
    /// hypotheses. Reading a saved initializer may retain an immutable bound
    /// value, but later guards cannot retroactively describe its evaluation.
    pub(in crate::proof_contracts::contract_entailment) fn closed_symbol_bindings(
        &self,
    ) -> Option<Vec<StrictArithmeticSymbolBinding>> {
        if !self.strict_symbol_bindings_valid {
            return None;
        }
        Some(
            self.strict_symbol_bindings
                .iter()
                .flatten()
                .filter_map(|(symbol, value)| {
                    Some(StrictArithmeticSymbolBinding {
                        symbol: *symbol,
                        value: StrictArithmeticBindingValue::Integer(value.constant_value()?),
                    })
                })
                .collect(),
        )
    }

    /// Normalize a scoped value without manufacturing a source expression for
    /// an implicit successor or a current storage observation.
    pub(in crate::proof_contracts::contract_entailment) fn normalize_scoped_value(
        &mut self,
        value: &super::super::ScopedArithmeticValue,
    ) -> Option<Polynomial> {
        use super::super::ScopedArithmeticValue;
        Some(match value {
            ScopedArithmeticValue::TypeReference(_) => return None,
            ScopedArithmeticValue::Atom { identity, unsigned } => {
                self.declare_atom(identity, *unsigned);
                Polynomial::atom(identity.clone())
            }
            ScopedArithmeticValue::BoundedInteger {
                value,
                minimum,
                maximum,
            } => {
                let polynomial = self.normalize_scoped_value(value)?;
                if let Some(minimum) = minimum {
                    self.bounds.push((polynomial.clone(), minimum.clone()));
                }
                if let Some(maximum) = maximum {
                    self.bounds.push((polynomial.neg(), maximum.negate()));
                }
                polynomial
            }
            ScopedArithmeticValue::Integer(value) => Polynomial::constant(value.clone()),
            ScopedArithmeticValue::IntegerOperation {
                operator,
                left,
                right,
            } => {
                if !matches!(
                    operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                ) {
                    return None;
                }
                let left = self.normalize_scoped_value(left)?;
                let right = self.normalize_scoped_value(right)?;
                self.integer_binary_value(*operator, left, right)?
            }
            ScopedArithmeticValue::Term(term) => {
                self.with_scoped_expression(term, |engine| engine.normalize(term.expression))?
            }
            // Collection observations must be prepared by the selected-call
            // owner. A descriptor is not an integer source expression.
            ScopedArithmeticValue::CollectionLength(_) => return None,
            ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
                self.with_scoped_expression(term, |engine| {
                    // This occurrence was admitted by the caller's selected-value
                    // owner; bare policy syntax receives no such authority.
                    engine.bind_strict_integer_division(term.expression)?;
                    engine.normalize(term.expression)
                })?
            }
        })
    }

    fn with_scoped_expression(
        &mut self,
        term: &super::super::ScopedArithmeticExpression,
        query: impl FnOnce(&mut Self) -> Option<Polynomial>,
    ) -> Option<Polynomial> {
        let previous_owner = self.source_owner;
        let previous_types = self.source_type_bindings.clone();
        let value = (|| {
            self.install_scoped_expression(term).then_some(())?;
            query(self)
        })();
        self.source_owner = previous_owner;
        self.source_type_bindings = previous_types;
        value
    }

    /// A scoped expression keeps its own declaration when its roster changes.
    /// Actual values are normalized before the formal roster is published.
    pub(in crate::proof_contracts::contract_entailment) fn install_scoped_expression(
        &mut self,
        expression: &super::super::ScopedArithmeticExpression,
    ) -> bool {
        if !expression.owner.is_valid() {
            return false;
        }
        self.source_owner = expression.owner;
        self.install_scoped_bindings(&expression.bindings)
    }

    /// Source types come from the exact declaration, independently of scoped
    /// substitutions. A polynomial cannot lend its type to a selected node.
    fn declared_source_result_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if let Some(parameters) =
            crate::proof_contracts::contract_results::source_contract_parameters(
                self.program,
                self.source_owner,
                expression,
            )
        {
            return crate::parameter_expression_result_type_reference(
                self.program,
                self.source_owner,
                parameters,
                expression,
            );
        }
        if let Some(domain) = self.program.domain_definition(self.source_owner) {
            return crate::value_custody::expression_types::domain_expression_result_type_reference(
                self.program,
                domain,
                expression,
            );
        }
        if let Some((_, signature)) = self.program.trait_machine_signature(self.source_owner) {
            return crate::parameter_expression_result_type_reference(
                self.program,
                signature.symbol,
                self.program.state_signature_parameters(signature),
                expression,
            );
        }
        crate::value_custody::expression_types::declaration_expression_result_type_reference(
            self.program,
            self.source_owner,
            &|place| {
                crate::proof_contracts::proof_embeddings::expression_type_reference(
                    self.program,
                    place,
                )
            },
            expression,
        )
    }

    pub(super) fn source_result_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if !self.source_type_bindings.is_empty()
            && let Some(binary) = self.program.expression_table.binary_node(expression)
        {
            let operands =
                [binary.left, binary.right].map(|operand| self.source_result_type(operand));
            if let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    self.source_owner,
                    expression,
                    spelling,
                    &operands,
                )
                && let Some(reference) =
                    crate::value_custody::expression_types::builtin_binary_result_type_reference(
                        self.program,
                        binary,
                        operands,
                        &mut |operand| self.source_result_type(operand),
                    )
            {
                return Some(reference);
            }
        }
        let reference = self.declared_source_result_type(expression)?;
        Some(typed_trees::static_applications::project_type_operand(
            self.program,
            reference,
            &self.source_type_bindings,
        ))
    }

    /// Interpret only the selected builtin operation. Fixed-carrier arithmetic
    /// additionally retains its actual policy: an Exact `+ - *` result is
    /// its mathematical value; Wrapping and Saturating retain their completed
    /// carrier representatives in `selected_policy_result`. Trapping can
    /// transfer control and forms no term. Primitive formation remains an
    /// independent obligation; a coercion cannot replace a source payload.
    pub(in crate::proof_contracts::contract_entailment) fn selected_binary_meaning(
        &self,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        let owner = self.source_owner;
        let operands = [binary.left, binary.right].map(|operand| self.source_result_type(operand));
        if matches!(
            binary.operator,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
        ) {
            crate::proof_contracts::bound_expression_meaning::builtin_arithmetic_for_operand_types(
                self.program,
                owner,
                expression,
                &operands,
            ) && (super::super::proof_integer::proof_integer_expression(self.program, expression)
                || self
                    .source_result_type(expression)
                    .is_some_and(|reference| {
                        use numerics::arithmetic::ArithmeticDomain;
                        let admitted_policies = [
                            ArithmeticDomain::Exact,
                            ArithmeticDomain::Wrapping,
                            ArithmeticDomain::Saturating,
                        ];
                        self.program
                            .primitive_type_reference(reference)
                            .is_some_and(|primitive| {
                                primitive.is_fixed_integer()
                                    || (primitive == typed_trees::types::PrimitiveType::Addr
                                        && self
                                            .program
                                            .arithmetic_domain_for_type_reference(reference)
                                            == ArithmeticDomain::Exact)
                            })
                            && admitted_policies.contains(
                                &self.program.arithmetic_domain_for_type_reference(reference),
                            )
                    }))
        } else {
            let Some(spelling) =
                crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                    binary.operator,
                )
            else {
                return true;
            };
            typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                owner,
                expression,
                spelling,
                &operands,
            )
        }
    }

    /// Import an already formed pattern observation under its own declaration.
    /// Boolean patterns select a condition's truth; integer patterns contribute
    /// exact equality or its integer complement. Neither spelling nor a shared
    /// runtime carrier supplies the selected equality law.
    pub(super) fn install_dispatch_selection(
        &mut self,
        selection: crate::proof_contracts::arithmetic_domains::DispatchSelection,
    ) {
        let operands =
            [selection.subject, selection.pattern].map(|operand| self.source_result_type(operand));
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.source_owner,
            selection.expression,
            language_core::OperatorSpelling::Equal,
            &operands,
        ) {
            return;
        }
        let mut comparisons = Vec::new();
        if let ExpressionNode::Boolean(value) =
            self.program.expression_table.expression(selection.pattern)
        {
            self.collect_condition_comparisons(
                selection.subject,
                selection.matched == *value,
                &mut comparisons,
            );
        } else if let (Some(left), Some(right)) = (
            self.normalize(selection.subject),
            self.normalize(selection.pattern),
        ) {
            comparisons.push((
                if selection.matched {
                    BinaryOperator::Equal
                } else {
                    BinaryOperator::NotEqual
                },
                left,
                right,
            ));
        }
        self.install_hypotheses(comparisons);
    }

    fn collect_condition_comparisons(
        &mut self,
        expression: ExpressionHandle,
        positive: bool,
        comparisons: &mut Vec<(BinaryOperator, Polynomial, Polynomial)>,
    ) {
        match self.program.expression_table.expression(expression).clone() {
            ExpressionNode::Boolean(value) => {
                self.requires_unsatisfiable |= value != positive;
                return;
            }
            ExpressionNode::Unary(unary)
                if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
            {
                self.collect_condition_comparisons(unary.operand, !positive, comparisons);
                return;
            }
            ExpressionNode::Binary(binary) if self.selected_binary_meaning(expression, &binary) => {
                if binary.operator == BinaryOperator::And && positive
                    || binary.operator == BinaryOperator::Or && !positive
                {
                    self.collect_condition_comparisons(binary.left, positive, comparisons);
                    self.collect_condition_comparisons(binary.right, positive, comparisons);
                    return;
                }
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) {
                    let wrapped = match (
                        self.program.expression_table.expression(binary.left),
                        self.program.expression_table.expression(binary.right),
                    ) {
                        (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                        (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                        _ => None,
                    };
                    if let Some((operand, value)) = wrapped {
                        self.collect_condition_comparisons(
                            operand,
                            positive == (value == (binary.operator == BinaryOperator::Equal)),
                            comparisons,
                        );
                        return;
                    }
                }
            }
            _ => {}
        }
        if let Some((operator, left, right)) = self.comparison_polynomials(expression)
            && let Some(operator) = if positive {
                Some(operator)
            } else {
                operator.complement()
            }
        {
            comparisons.push((operator, left, right));
        }
    }

    /// Replace the strict symbol, result and occurrence tables with one scoped
    /// roster. Every bound term is read under its own nested roster, so
    /// no term can see the roster it is bound into. Declared atoms, installed
    /// hypotheses and bounds persist: the rosters of one implication share
    /// each atom by identity. Returns false for an invalid symbol, a binder
    /// bound twice to different values, or a term outside the language.
    pub(in crate::proof_contracts::contract_entailment) fn install_scoped_bindings(
        &mut self,
        bindings: &[super::super::ScopedArithmeticBinding],
    ) -> bool {
        use super::super::ScopedArithmeticBinder;
        if !self.strict_symbol_bindings_valid {
            return false;
        }
        self.strict_occurrences.clear();
        let mut symbols: Vec<(SymbolHandle, Polynomial)> = Vec::with_capacity(bindings.len());
        self.saved_conditions.clear();
        let mut occurrences = Vec::new();
        let mut types = Vec::new();
        let mut result = None;
        for binding in bindings {
            if let ScopedArithmeticBinder::Type(symbol) = binding.binder {
                let super::super::ScopedArithmeticValue::TypeReference(reference) = binding.value
                else {
                    return false;
                };
                if !symbol.is_valid()
                    || !self
                        .program
                        .type_reference_table
                        .contains_type_reference(reference)
                {
                    return false;
                }
                if let Some((_, existing)) = types.iter().find(|(formal, _)| *formal == symbol) {
                    if *existing != reference {
                        return false;
                    }
                } else {
                    types.push((symbol, reference));
                }
                continue;
            }
            let Some(polynomial) = self.normalize_scoped_value(&binding.value) else {
                return false;
            };
            match binding.binder {
                ScopedArithmeticBinder::Type(_) => unreachable!("type metadata handled above"),
                ScopedArithmeticBinder::Symbol(symbol) => {
                    if !symbol.is_valid() {
                        return false;
                    }
                    if let Some((_, existing)) =
                        symbols.iter().find(|(candidate, _)| *candidate == symbol)
                    {
                        if *existing != polynomial {
                            return false;
                        }
                        continue;
                    }
                    symbols.push((symbol, polynomial));
                }
                ScopedArithmeticBinder::Result => {
                    if result
                        .as_ref()
                        .is_some_and(|existing| *existing != polynomial)
                    {
                        return false;
                    }
                    result = Some(polynomial);
                }
                ScopedArithmeticBinder::Occurrence(expression) => {
                    occurrences.push((expression, polynomial));
                }
            }
        }
        self.source_type_bindings = types;
        self.strict_symbol_bindings = Some(symbols);
        self.strict_result_binding = result;
        // Nested terms installed their own occurrence tables. Publish only
        // this roster, including when it has no occurrence bindings at all.
        self.strict_occurrences.clear();
        for (expression, polynomial) in occurrences {
            if !self.bind_strict_occurrence(expression, polynomial) {
                return false;
            }
        }
        true
    }

    /// The ranking owner binds a checked member projection. Keep its narrower
    /// boundary while scoped owners may transport other exact value occurrences.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_projection(
        &mut self,
        expression: ExpressionHandle,
        value: Polynomial,
    ) -> bool {
        matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) && self.bind_strict_occurrence(expression, value)
    }

    /// The rank-range owner has checked selected Exact integer meaning; the
    /// bound mathematical term does not excuse its separate formation proof.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_integer_division(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<()> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        let dividend = self.normalize(binary.left)?;
        let divisor = self.normalize(binary.right)?;
        if !matches!(
            binary.operator,
            BinaryOperator::Divide | BinaryOperator::Modulo
        ) {
            return None;
        }
        let value = self.integer_binary_value(binary.operator, dividend, divisor)?;
        self.bind_strict_occurrence(expression, value).then_some(())
    }

    /// The rank-range owner has checked the selected Exact integer meaning and
    /// supplies the shifted carrier's width; the bound mathematical term does
    /// not excuse the endpoint's separate count-within-width and
    /// representability proofs.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_integer_shift(
        &mut self,
        expression: ExpressionHandle,
        count_bits: u32,
    ) -> Option<()> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        let value = self.normalize(binary.left)?;
        let count = self.normalize(binary.right)?;
        let value = match binary.operator {
            BinaryOperator::ShiftLeft => self.integer_shift_left(value, count, count_bits)?,
            BinaryOperator::ShiftRight => self.integer_shift_right(value, count, count_bits)?,
            _ => return None,
        };
        self.bind_strict_occurrence(expression, value).then_some(())
    }

    /// The rank-range owner has checked the selected builtin integer
    /// meaning and supplies the shared carrier's shape; the bound
    /// mathematical term does not excuse the endpoint's own operand
    /// agreement and landing proofs. Bitwise operations are total inside their carrier,
    /// so unlike division or a shift it attaches no operand obligation of
    /// its own.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_integer_bitwise(
        &mut self,
        expression: ExpressionHandle,
        bits: u32,
        signed: bool,
    ) -> Option<()> {
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return None;
        };
        let left = self.normalize(binary.left)?;
        let right = self.normalize(binary.right)?;
        let value = match binary.operator {
            BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
                self.integer_bitwise(binary.operator, left, right, bits, signed)?
            }
            _ => return None,
        };
        self.bind_strict_occurrence(expression, value).then_some(())
    }

    /// A completed operand may supply a captured value of an exact builtin
    /// integer place. The flow owner authenticates its evidence epoch; this
    /// facade preserves the source place/type/selected-meaning boundary before
    /// installing an occurrence in the private arithmetic namespace.
    pub(in crate::proof_contracts::contract_entailment) fn bind_captured_integer_place(
        &mut self,
        machine: &Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        value: BigInt,
    ) -> bool {
        let Some(reference) =
            crate::declared_place_type_raw(self.program, machine, Some(state), expression)
        else {
            return false;
        };
        crate::canonical_place_from_expression(self.program, expression).is_some()
            && self
                .program
                .primitive_type_reference(reference)
                .is_some_and(|primitive| primitive.is_fixed_integer())
            && crate::has_builtin_bound_expression_meaning(
                self.program,
                machine,
                Some(state),
                expression,
            )
            && self.bind_strict_occurrence(expression, Polynomial::constant(value))
    }

    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_occurrence(
        &mut self,
        expression: ExpressionHandle,
        value: Polynomial,
    ) -> bool {
        if self.strict_symbol_bindings.is_none()
            || !self.strict_symbol_bindings_valid
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return false;
        }
        if let Some((_, existing)) = self
            .strict_occurrences
            .iter()
            .find(|(candidate, _)| *candidate == expression)
        {
            return *existing == value;
        }
        self.strict_occurrences.push((expression, value));
        true
    }

    /// The caller establishes exact builtin operand meaning and live captures.
    /// Every actual resolves in the caller's namespace before any formal is
    /// bound, and a formal named twice must receive one polynomial.
    pub(in crate::proof_contracts::contract_entailment) fn exact_argument_values(
        &mut self,
        arguments: &[StrictArithmeticExpressionBinding],
    ) -> Option<Vec<(SymbolHandle, Polynomial)>> {
        if !self.strict_symbol_bindings_valid || self.strict_symbol_bindings.is_none() {
            return None;
        }
        let resolved = arguments
            .iter()
            .map(|argument| {
                argument.symbol.is_valid().then_some(())?;
                Some((argument.symbol, self.normalize(argument.expression)?))
            })
            .collect::<Option<Vec<_>>>();
        let mut formals: Vec<(SymbolHandle, Polynomial)> = Vec::new();
        for (symbol, polynomial) in resolved? {
            match formals.iter().find(|(candidate, _)| *candidate == symbol) {
                Some((_, existing)) if *existing != polynomial => return None,
                Some(_) => {}
                None => formals.push((symbol, polynomial)),
            }
        }
        Some(formals)
    }

    /// The goal is the callee's clause, so each formal denotes its actual.
    /// A self-call's formals are the caller's own parameter symbols: they
    /// shadow those bindings here, after the caller's hypotheses were
    /// normalized under the caller's meaning.
    pub(in crate::proof_contracts::contract_entailment) fn bind_goal_formals(
        &mut self,
        formals: Vec<(SymbolHandle, Polynomial)>,
    ) {
        // Occurrences belong to the caller invocation just as its formals do.
        // Hypotheses and captured actuals already hold their normalized values;
        // the destination must install its own projected readings afterward.
        self.strict_occurrences.clear();
        let Some(bindings) = &mut self.strict_symbol_bindings else {
            self.saved_conditions.clear();
            return;
        };
        self.saved_conditions.clear();
        for (symbol, polynomial) in formals {
            match bindings
                .iter_mut()
                .find(|(candidate, _)| *candidate == symbol)
            {
                Some((_, existing)) => *existing = polynomial,
                None => bindings.push((symbol, polynomial)),
            }
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_arguments(
        &mut self,
        arguments: &[StrictArithmeticExpressionBinding],
    ) -> bool {
        if !self.strict_symbol_bindings_valid || self.strict_symbol_bindings.is_none() {
            return false;
        }
        // Resolve every actual before extending the table: callee formals are
        // not caller values, even when another argument binds them first.
        let Some(resolved) = arguments
            .iter()
            .map(|argument| {
                argument.symbol.is_valid().then_some(())?;
                Some((argument.symbol, self.normalize(argument.expression)?))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        let Some(bindings) = &mut self.strict_symbol_bindings else {
            return false;
        };
        for (symbol, polynomial) in resolved {
            if let Some((_, existing)) = bindings.iter().find(|(candidate, _)| *candidate == symbol)
            {
                if existing != &polynomial {
                    return false;
                }
            } else {
                bindings.push((symbol, polynomial));
            }
        }
        true
    }

    /// Replace one saved binding with the initializer read under the hypotheses
    /// installed since the first bind. A wrapping subtraction bound before
    /// `eax > 0` is an opaque residue; the backedge reads the decreased
    /// register only after those hypotheses prove the result lands.
    pub(in crate::proof_contracts::contract_entailment) fn rebind_saved_expression(
        &mut self,
        symbol: SymbolHandle,
        expression: ExpressionHandle,
    ) -> bool {
        if !self.strict_symbol_bindings_valid || self.strict_symbol_bindings.is_none() {
            return false;
        }
        let previous = self.strict_symbol_bindings.as_mut().and_then(|bindings| {
            bindings
                .iter()
                .position(|(candidate, _)| *candidate == symbol)
                .map(|index| bindings.remove(index).1)
        });
        match self.normalize(expression) {
            Some(polynomial) => {
                let Some(bindings) = &mut self.strict_symbol_bindings else {
                    return false;
                };
                bindings.push((symbol, polynomial));
                true
            }
            None => {
                if let Some(previous) = previous
                    && let Some(bindings) = &mut self.strict_symbol_bindings
                {
                    bindings.push((symbol, previous));
                }
                false
            }
        }
    }

    /// Record the term an opaque atom was minted from. One atom string names
    /// one canonical operand, so a repeated mint keeps the first entry.
    pub(in crate::proof_contracts::contract_entailment) fn register_opaque_term(
        &mut self,
        atom: String,
        term: OpaqueTerm,
    ) {
        if self.opaque_terms.iter().any(|(minted, _)| *minted == atom) {
            return;
        }
        self.opaque_terms.push((atom, term));
    }
}

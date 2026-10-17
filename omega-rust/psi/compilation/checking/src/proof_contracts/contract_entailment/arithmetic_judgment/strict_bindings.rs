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
    pub(in crate::proof_contracts::contract_entailment) fn strict_for_declaration(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
    ) -> Self {
        let mut engine = Self::for_explicit_bindings(program);
        engine.machine_symbol = owner;
        engine.source_owner = owner;
        engine
    }

    pub(in crate::proof_contracts::contract_entailment) fn strict(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        bindings: &[StrictArithmeticSymbolBinding],
    ) -> Self {
        let mut engine = Self::new(program, machine);
        engine.parameter_atoms.clear();
        engine.unsigned_atoms.clear();
        let mut resolved = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let polynomial = match &binding.value {
                StrictArithmeticBindingValue::Atom { identity, unsigned } => {
                    if !engine.parameter_atoms.contains(identity) {
                        engine.parameter_atoms.push(identity.clone());
                    }
                    if *unsigned && !engine.unsigned_atoms.contains(identity) {
                        engine.unsigned_atoms.push(identity.clone());
                    }
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

    /// Register a private atom of a scoped roster. Unsigned atoms carry the
    /// same implicit `>= 0` as unsigned parameters; the matrix seeds it on the
    /// next hypothesis installation.
    fn declare_atom(&mut self, identity: &str, unsigned: bool) {
        if !self.parameter_atoms.iter().any(|atom| atom == identity) {
            self.parameter_atoms.push(identity.to_owned());
        }
        if unsigned && !self.unsigned_atoms.iter().any(|atom| atom == identity) {
            self.unsigned_atoms.push(identity.to_owned());
        }
    }

    /// Normalize a scoped value without manufacturing a source expression for
    /// an implicit successor or a current storage observation.
    pub(in crate::proof_contracts::contract_entailment) fn normalize_scoped_value(
        &mut self,
        value: &super::super::ScopedArithmeticValue,
    ) -> Option<Polynomial> {
        use super::super::ScopedArithmeticValue;
        Some(match value {
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
        let value = (|| {
            self.install_scoped_expression(term).then_some(())?;
            query(self)
        })();
        self.source_owner = previous_owner;
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
    pub(super) fn source_result_type(
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

    /// Interpret only the selected builtin operation. Fixed-carrier arithmetic
    /// additionally retains its actual policy: an Exact `+ - *` result is
    /// its mathematical value, a Wrapping or Saturating one is that value only
    /// under the carrier-landing proof `selected_policy_result` owes, and a
    /// Trapping one can transfer control and forms no term. Division keeps
    /// its Exact requirement. A coercion around a Wrapping or Saturating
    /// result cannot replace the operation's payload.
    pub(super) fn strict_binary_meaning(
        &self,
        expression: ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        if self.strict_symbol_bindings.is_none() || self.proof_integer_formation {
            return true;
        }
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
                        let admitted_policies: &[ArithmeticDomain] = if matches!(
                            binary.operator,
                            BinaryOperator::Divide | BinaryOperator::Modulo
                        ) {
                            &[ArithmeticDomain::Exact]
                        } else {
                            &[
                                ArithmeticDomain::Exact,
                                ArithmeticDomain::Wrapping,
                                ArithmeticDomain::Saturating,
                            ]
                        };
                        self.program
                            .primitive_type_reference(reference)
                            .is_some_and(|primitive| primitive.is_fixed_integer())
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
        let mut occurrences = Vec::new();
        let mut result = None;
        for binding in bindings {
            let Some(polynomial) = self.normalize_scoped_value(&binding.value) else {
                return false;
            };
            match binding.binder {
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
    /// agreement and landing proofs. `a & b` is total inside its carrier,
    /// so unlike division or a shift it attaches no operand obligation of
    /// its own.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_integer_bitwise_and(
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
            BinaryOperator::BitwiseAnd => self.integer_bitwise_and(left, right, bits, signed)?,
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
            return;
        };
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

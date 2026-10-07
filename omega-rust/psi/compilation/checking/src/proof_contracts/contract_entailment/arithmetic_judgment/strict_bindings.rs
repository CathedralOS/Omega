//! Strict-arithmetic symbol bindings: engine construction from declared
//! strict bindings, scoped-value normalization, and strict operator
//! projection/division/shift/bitwise-and binding rules.
use super::super::{
    BigInt, BinaryOperator, ExpressionHandle, ExpressionNode, Machine,
    StrictArithmeticBindingValue, StrictArithmeticExpressionBinding, StrictArithmeticSymbolBinding,
    SymbolHandle, TypedTrees,
};
use super::{Engine, OpaqueTerm, Polynomial};

impl<'program> Engine<'program> {
    pub(in crate::proof_contracts::contract_entailment) fn strict_with_symbol_bindings(
        program: &'program TypedTrees,
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

    pub(in crate::proof_contracts::contract_entailment) fn strict_symbol_bindings_are_valid(
        &self,
    ) -> bool {
        self.strict_symbol_bindings_valid
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
            ScopedArithmeticValue::Integer(value) => Polynomial::constant(value.clone()),
            ScopedArithmeticValue::IntegerOperation {
                operator,
                left,
                right,
            } => {
                if !matches!(
                    operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                ) {
                    return None;
                }
                let left = self.normalize_scoped_value(left)?;
                let right = self.normalize_scoped_value(right)?;
                match operator {
                    BinaryOperator::Add => left.add(&right),
                    BinaryOperator::Subtract => left.sub(&right),
                    BinaryOperator::Multiply => left.checked_mul(&right)?,
                    _ => return None,
                }
            }
            ScopedArithmeticValue::Term(term) => {
                if !self.install_scoped_bindings(&term.bindings) {
                    return None;
                }
                self.normalize(term.expression)?
            }
            ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
                if !self.install_scoped_bindings(&term.bindings) {
                    return None;
                }
                self.bind_strict_integer_division(term.expression)?;
                self.normalize(term.expression)?
            }
        })
    }

    /// Replace the strict symbol, result and projection tables with one scoped
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
        self.strict_projections.clear();
        let mut symbols: Vec<(SymbolHandle, Polynomial)> = Vec::with_capacity(bindings.len());
        let mut projections = Vec::new();
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
                ScopedArithmeticBinder::Projection(expression) => {
                    projections.push((expression, polynomial));
                }
            }
        }
        self.strict_symbol_bindings = Some(symbols);
        self.strict_result_binding = result;
        // Nested terms installed their own occurrence tables. Publish only
        // this roster, including when it has no occurrence bindings at all.
        self.strict_projections.clear();
        for (expression, polynomial) in projections {
            if !self.bind_strict_projection(expression, polynomial) {
                return false;
            }
        }
        true
    }

    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_projection(
        &mut self,
        expression: ExpressionHandle,
        value: Polynomial,
    ) -> bool {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        ) {
            return false;
        }
        self.bind_strict_occurrence(expression, value)
    }

    /// The rank-range owner has checked selected Exact integer meaning; the
    /// bound mathematical term does not excuse its separate formation proof.
    pub(in crate::proof_contracts::contract_entailment) fn bind_strict_integer_division(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<()> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let dividend = self.normalize(binary.left)?;
        let divisor = self.normalize(binary.right)?;
        let positive_divisor = self.prove_at_least(&divisor, &BigInt::from_i64(1));
        let nonnegative_dividend = self.prove_at_least(&dividend, &BigInt::zero());
        let value = match binary.operator {
            BinaryOperator::Divide => self.integer_quotient(dividend, divisor)?,
            BinaryOperator::Modulo => {
                let value = self.integer_remainder(dividend, divisor.clone(), true)?;
                // The admitted source owner established an unsigned remainder
                // operation. Re-derive its correlated strict upper bound only
                // after the scoped hypotheses independently prove both
                // unsigned nonnegativity and a positive exact divisor subject.
                if positive_divisor && nonnegative_dividend {
                    self.bounds.push((divisor.sub(&value), BigInt::from_i64(1)));
                }
                value
            }
            _ => return None,
        };
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
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
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
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
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

    fn bind_strict_occurrence(&mut self, expression: ExpressionHandle, value: Polynomial) -> bool {
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
            .strict_projections
            .iter()
            .find(|(candidate, _)| *candidate == expression)
        {
            return *existing == value;
        }
        self.strict_projections.push((expression, value));
        true
    }

    /// The caller establishes exact builtin operand meaning and live captures.
    /// Widenings are transparent only while those actuals are normalized; a
    /// strict symbol namespace alone does not license this reading elsewhere.
    /// Every actual resolves in the caller's namespace before any formal is
    /// bound, and a formal named twice must receive one polynomial.
    pub(in crate::proof_contracts::contract_entailment) fn exact_argument_values(
        &mut self,
        arguments: &[StrictArithmeticExpressionBinding],
    ) -> Option<Vec<(SymbolHandle, Polynomial)>> {
        if !self.strict_symbol_bindings_valid || self.strict_symbol_bindings.is_none() {
            return None;
        }
        self.exact_argument_widening = true;
        let resolved = arguments
            .iter()
            .map(|argument| {
                argument.symbol.is_valid().then_some(())?;
                Some((argument.symbol, self.normalize(argument.expression)?))
            })
            .collect::<Option<Vec<_>>>();
        self.exact_argument_widening = false;
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

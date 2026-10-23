//! Scalar operand terms: integer and float operands, literal and
//! zero values, integer operations, and open terms denote into the shared
//! mathematical carriers.

use numerics::bignum::BigInt;
use numerics::literals::{FloatFormat, FloatLiteral, IntegerLiteral, LandedIntegerType};

use proof_admission::{Term, TermHandle};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use typed_trees::types::TypeReferenceHandle;

use super::{Elaborator, ScalarCarrier};

/// Construction-time custody of one opaque source operation. The selected
/// source expression, declaration scope and complete result reference define
/// identity; matching syntax never supplies an operation/value equality.
pub(super) struct SourceOperationDeclaration {
    expression: ExpressionHandle,
    owner: symbols::SymbolHandle,
    source_type: TypeReferenceHandle,
    declaration: u32,
}

impl<'a> Elaborator<'a> {
    /// One operand of a relation or connective, denoted at the shared
    /// `carrier` — literals and zero values take it, while names, calls
    /// and compound arithmetic must already inhabit it.
    pub(super) fn scalar_operand_term(
        &mut self,
        handle: ExpressionHandle,
        carrier: ScalarCarrier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Integer(literal) => {
                let Some(value) = literal.value_bignum() else {
                    return Err(
                        self.refuse(format!("integer literal `{}` has no exact value", literal))
                    );
                };
                let position = self.carrier_position(carrier);
                self.numeric_literal(position, value)
            }
            ExpressionNode::Boolean(value) => {
                if carrier != ScalarCarrier::Boolean {
                    return Err(self.refuse(format!(
                        "boolean literal is not a `{}` operand",
                        self.carrier_name(carrier)
                    )));
                }
                Ok(self.boolean_literal(*value))
            }
            ExpressionNode::Float(literal) => {
                let literal = literal.clone();
                self.float_operand_term(&literal, carrier)
            }
            ExpressionNode::ZeroValue(reference) => {
                let reference = *reference;
                match self.scalar_carrier_of_type_reference(reference) {
                    Some(determined) if determined == carrier => self.zero_literal(carrier),
                    _ => Err(self.refuse(format!(
                        "zero value of `{}` is not a `{}` operand",
                        self.program.display_type_reference(reference),
                        self.carrier_name(carrier)
                    ))),
                }
            }
            ExpressionNode::Binary(binary) if binary.operator.is_arithmetic() => {
                let binary = *binary;
                match carrier {
                    ScalarCarrier::Integer => self.integer_operation_term(handle, &binary),
                    _ => match self.operand_carrier(handle) {
                        // A float-carried operation (`x * y` inside
                        // `x * y == z`) interns opaquely at it.
                        Some(determined) if determined == carrier => self.open_term(handle),
                        determined => Err(self.unexpected_carrier_error(determined, handle)),
                    },
                }
            }
            ExpressionNode::Name(..)
            | ExpressionNode::Call(..)
            | ExpressionNode::Unary(..)
            | ExpressionNode::Member(..)
            | ExpressionNode::Cast(..)
            | ExpressionNode::StructLiteral(..)
            | ExpressionNode::Match(..) => match self.operand_carrier(handle) {
                Some(determined) if determined == carrier => self.elaborate_expression(handle),
                determined => Err(self.unexpected_carrier_error(determined, handle)),
            },
            _ => Err(self.unsupported_expression(handle)),
        }
    }

    /// An `Int`-valued operand: a literal, a name or call at an integer
    /// carrier, or compound integer arithmetic.
    pub(super) fn integer_operand(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        self.scalar_operand_term(handle, ScalarCarrier::Integer)
    }

    /// A literal in term position denotes at its landing carrier —
    /// `addr`-landed literals land at the address carrier, every other
    /// integer literal at `Int` — interned by exact value.
    pub(super) fn literal_term(
        &mut self,
        literal: &IntegerLiteral,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let Some(value) = literal.value_bignum() else {
            return Err(self.refuse(format!("integer literal `{}` has no exact value", literal)));
        };
        let carrier = match literal.landing() {
            Some(landing) if landing.landed_type == LandedIntegerType::Addr => {
                self.address_carrier()
            }
            _ => self.integer_carrier(),
        };
        self.numeric_literal(carrier, value)
    }

    /// A float literal in term position denotes at its landing carrier,
    /// interned by its exact value read at that format — `1.5f32` lands
    /// at `f32`. An unlanded literal has no demanded carrier to defer
    /// to and refuses rather than guessing a width.
    pub(super) fn float_literal_term(
        &mut self,
        literal: &FloatLiteral,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let Some(landing) = literal.landing() else {
            return Err(self.refuse(format!(
                "cannot determine the float carrier of literal `{}`",
                literal.text()
            )));
        };
        let carrier = match landing {
            FloatFormat::F32 => ScalarCarrier::Float32,
            FloatFormat::F64 => ScalarCarrier::Float64,
        };
        self.float_operand_term(literal, carrier)
    }

    /// A float literal demanded at `carrier`: a landed literal must
    /// match the format exactly, an unlanded one adopts it — anonymous
    /// literals deferring to the demanded carrier, mirroring the
    /// integer rule. The constant interns by the literal's exact value
    /// read at the demanded format, so `1.5` and `1.50` share.
    pub(super) fn float_operand_term(
        &mut self,
        literal: &FloatLiteral,
        carrier: ScalarCarrier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let format = match carrier {
            ScalarCarrier::Float32 => FloatFormat::F32,
            ScalarCarrier::Float64 => FloatFormat::F64,
            _ => {
                return Err(self.refuse(format!(
                    "float literal is not a `{}` operand",
                    self.carrier_name(carrier)
                )));
            }
        };
        if let Some(landing) = literal.landing()
            && landing != format
        {
            return Err(self.refuse(format!(
                "a `{}` float literal is not a `{}` operand",
                landing.name(),
                self.carrier_name(carrier)
            )));
        }
        // The format's own correct rounding of the spelling — an f32
        // read never routes through f64 (`FloatLiteral::f32_bits`).
        let bits = match format {
            FloatFormat::F32 => u64::from(literal.f32_bits()),
            FloatFormat::F64 => literal.value_f64().to_bits(),
        };
        let position = self.carrier_position(carrier);
        Ok(self.float_literal(position, bits))
    }

    /// One float literal's constant at `carrier` position, interned by
    /// `(carrier, exact bits)` — denotation by value, matching
    /// `numeric_literal`.
    pub(super) fn float_literal(&mut self, carrier: u32, bits: u64) -> TermHandle {
        interned_literal!(self.float_literals, (carrier, bits), carrier)
    }

    /// The zero value of a scalar carrier.
    pub(super) fn zero_literal(
        &mut self,
        carrier: ScalarCarrier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match carrier {
            ScalarCarrier::Boolean => Ok(self.boolean_literal(false)),
            ScalarCarrier::Float32 | ScalarCarrier::Float64 => {
                // +0.0's bit pattern at either width.
                let position = self.carrier_position(carrier);
                Ok(self.float_literal(position, 0))
            }
            ScalarCarrier::Carrier(..) => Err(self.refuse(
                "the zero value of a non-numeric carrier has no bounded denotation".to_owned(),
            )),
            _ => {
                let position = self.carrier_position(carrier);
                self.numeric_literal(position, BigInt::from_u64(0))
            }
        }
    }

    /// `ZeroValue` denotes the zero literal at its carrier.
    pub(super) fn zero_value_term(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.scalar_carrier_of_type_reference(reference) {
            Some(carrier) => self.zero_literal(carrier),
            None => Err(self.refuse(format!(
                "the zero value of `{}` has no scalar denotation",
                self.program.display_type_reference(reference)
            ))),
        }
    }

    /// Closed integer arithmetic evaluates to the exact literal `Int`
    /// constant — `2 + 0` and `2` name one denotation. Open `+`/`-`
    /// share `IntAdd`/`IntSub` applied to their operands; every other
    /// open machine operation composes no function (the bounded
    /// vocabulary holds none for it) and interns opaquely at `Int`.
    pub(super) fn integer_operation_term(
        &mut self,
        handle: ExpressionHandle,
        binary: &TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        // The closed evaluator reads rationals — an expression with a
        // float leaf (`4.0 / 2.0` -> `2`) is not `Int` arithmetic and
        // must not collapse to an integer constant.
        if !self.expression_has_float_leaf(handle)
            && let Some(value) = self.program.closed_integer_expression_value(handle)
        {
            let carrier = self.integer_carrier();
            return self.numeric_literal(carrier, value);
        }
        if validation::mathematical_applications::operand_type_reference(self.program, handle)
            .is_some_and(|reference| {
                self.program
                    .primitive_type_reference(reference)
                    .is_some_and(|primitive| primitive.is_fixed_integer())
                    && self.program.arithmetic_domain_for_type_reference(reference)
                        != numerics::arithmetic::ArithmeticDomain::Exact
            })
        {
            return self.open_term(handle);
        }
        let references = [binary.left, binary.right].map(|operand| {
            validation::mathematical_applications::operand_type_reference(self.program, operand)
        });
        let spelling = match binary.operator {
            BinaryOperator::Add => language_core::OperatorSpelling::Add,
            BinaryOperator::Subtract => language_core::OperatorSpelling::Subtract,
            BinaryOperator::Multiply if self.integer_multiply.is_some() => {
                language_core::OperatorSpelling::Multiply
            }
            _ => return self.open_term(handle),
        };
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.source_owner,
            handle,
            spelling,
            &references,
        ) {
            return self.open_term(handle);
        }
        let position = match binary.operator {
            BinaryOperator::Add => self.integer_add(),
            BinaryOperator::Subtract => self.integer_subtract(),
            BinaryOperator::Multiply if self.integer_multiply.is_some() => {
                self.integer_multiply.unwrap()
            }
            _ => return self.open_term(handle),
        };
        let left = self.integer_operand(binary.left)?;
        let right = self.integer_operand(binary.right)?;
        let operation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: operation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// A source operation is a function of its free captured values. The
    /// checked kernel applies that dependency spine during beta substitution;
    /// an operation in one declaration never becomes a scope-free scalar atom.
    pub(super) fn open_term(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        // An opaque operation's declared result can be the first occurrence
        // of a data carrier. Elaborate that exact source domain before asking
        // which carrier it denotes; signature occurrence is not authority.
        let source_type =
            validation::mathematical_applications::operand_type_reference(self.program, handle);
        let source_domain = source_type
            .map(|reference| self.elaborate_source_operand_type(reference))
            .transpose()?;
        let Some(carrier) = self.operand_carrier(handle) else {
            return Err(self.unsupported_expression(handle));
        };
        if let Some(meaning) =
            validation::mathematical_applications::integer_embedding_meaning(self.program, handle)
        {
            return self.embedding_term(handle, meaning);
        }
        let result_type = match source_domain {
            Some(domain) => domain,
            None => self.carrier_term(carrier),
        };
        let mut dependencies = Vec::new();
        // A result domain can mention a generic binder that no value operand
        // reaches. Keep those exact type dependencies in the same telescope.
        super::dependencies::free_positions(
            &self.arena,
            result_type,
            self.scope.len(),
            &mut dependencies,
        );
        let mut pending = vec![handle];
        let mut seen = Vec::new();
        while let Some(expression) = pending.pop() {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
                || seen.contains(&expression)
            {
                continue;
            }
            seen.push(expression);
            if let Some((_, position)) = self
                .captured_expressions
                .iter()
                .find(|(captured, _)| *captured == expression)
            {
                if !dependencies.contains(position) {
                    dependencies.push(*position);
                }
                continue;
            }
            let node = self.program.expression_table.expression(expression);
            if let ExpressionNode::Name(path) = node {
                let name = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .last();
                if let Some(position) = self.scope.iter().rposition(|entry| {
                    if path.symbol.is_valid() {
                        entry.symbol == Some(path.symbol)
                    } else {
                        entry.symbol.is_none() && name.is_some_and(|name| *name == entry.name)
                    }
                }) {
                    if !dependencies.contains(&position) {
                        dependencies.push(position);
                    }
                }
            }
            validation::expression_traversal::children(self.program, node, |child| {
                pending.push(child)
            });
        }
        let mut index = 0;
        while index < dependencies.len() {
            let position = dependencies[index];
            super::dependencies::free_positions(
                &self.arena,
                self.scope[position].domain,
                position,
                &mut dependencies,
            );
            index += 1;
        }
        dependencies.sort_unstable();
        let source_type = source_type.unwrap_or_default();
        let position = match self.source_operations.iter().find(|operation| {
            operation.owner == self.source_owner
                && operation.source_type == source_type
                && (operation.expression == handle
                    || validation::mathematical_applications::source_operations_have_same_meaning(
                        self.program,
                        self.source_owner,
                        operation.expression,
                        handle,
                    ))
        }) {
            Some(operation) => operation.declaration,
            None => {
                let mut domains = Vec::new();
                for (index, position) in dependencies.iter().enumerate() {
                    let old = self.scope[..*position]
                        .iter()
                        .map(|entry| entry.domain)
                        .collect::<Vec<_>>();
                    domains.push(super::dependencies::rebind_domain(
                        &mut self.arena,
                        self.scope[*position].domain,
                        &old,
                        &dependencies[..index],
                    ));
                }
                let old_domains = self
                    .scope
                    .iter()
                    .map(|entry| entry.domain)
                    .collect::<Vec<_>>();
                let mut ty = super::dependencies::rebind_domain(
                    &mut self.arena,
                    result_type,
                    &old_domains,
                    &dependencies,
                );
                for domain in domains.iter().rev() {
                    ty = self.arena.insert(Term::Pi {
                        domain: *domain,
                        codomain: ty,
                    });
                }
                let position = self.push_assumption(ty);
                self.source_operations.push(SourceOperationDeclaration {
                    expression: handle,
                    owner: self.source_owner,
                    source_type,
                    declaration: position,
                });
                self.prepared.constants.append(super::PreparedConstant {
                    declaration: position,
                    meaning: super::PreparedConstantMeaning::SourceOperation {
                        expression: handle,
                        source_type,
                    },
                });
                position
            }
        };
        let mut function = self.constant(position);
        for position in dependencies {
            let argument = self.scope_variable(position);
            function = self.arena.insert(Term::Apply { function, argument });
        }
        Ok(function)
    }

    fn embedding_term(
        &mut self,
        handle: ExpressionHandle,
        meaning: super::PreparedConstantMeaning,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let (_, argument) = validation::integer_embedding_argument(self.program, handle)
            .ok_or_else(|| self.unsupported_expression(handle))?;
        let source = self
            .operand_carrier(argument)
            .ok_or_else(|| self.unsupported_expression(argument))?;
        let argument = self.scalar_operand_term(argument, source)?;
        if self.integer_theory.is_some() && source == ScalarCarrier::Integer {
            // The independently selected fixed-integer builtin is inclusion
            // into the shared Int carrier. Its denotation is its operand:
            // introducing an identity function/application here makes the
            // dependent W checker repeatedly convert an administrative redex.
            // Source width/bounds remain with the checked operand and builtin
            // meaning; no authored function or assumption is unfolded. The
            // removed identity has only Int dependencies, already present in
            // that operand's type. Address inclusions keep their separate
            // carrier and assumption below.
            return Ok(argument);
        }
        let domain = self.carrier_term(source);
        let codomain = self.integer_constant();
        let ty = self.arena.insert(Term::Pi { domain, codomain });
        let position = self.push_assumption(ty);
        self.prepared.constants.append(super::PreparedConstant {
            declaration: position,
            meaning,
        });
        let function = self.constant(position);
        Ok(self.arena.insert(Term::Apply { function, argument }))
    }
}

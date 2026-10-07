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
                    return Err(self.refuse(format!(
                        "integer literal `{}` has no exact value",
                        literal.text()
                    )));
                };
                let position = self.carrier_position(carrier);
                Ok(self.numeric_literal(position, value))
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
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                        | BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseOr
                        | BinaryOperator::BitwiseXor
                ) =>
            {
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
            return Err(self.refuse(format!(
                "integer literal `{}` has no exact value",
                literal.text()
            )));
        };
        let carrier = match literal.landing() {
            Some(landing) if landing.landed_type == LandedIntegerType::Addr => {
                self.address_carrier()
            }
            _ => self.integer_carrier(),
        };
        Ok(self.numeric_literal(carrier, value))
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
        let key = (carrier, bits);
        let position = match self.float_literals.get(&key) {
            Some(&position) => position,
            None => {
                let ty = self.constant(carrier);
                let position = self.push_assumption(ty);
                self.float_literals.insert(key, position);
                position
            }
        };
        self.constant(position)
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
                Ok(self.numeric_literal(position, BigInt::from_u64(0)))
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
            return Ok(self.numeric_literal(carrier, value));
        }
        let position = match binary.operator {
            BinaryOperator::Add => self.integer_add(),
            BinaryOperator::Subtract => self.integer_subtract(),
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

    /// The denotation for a machine expression the composing vocabulary
    /// does not name: an opaque assumption at the expression's
    /// determined scalar carrier, interned by the expression's own
    /// structure — the `MathTermKey::Open`/`scalar_integer_terms` rule
    /// of the bounded denotation. `x * y` and `x * y` name one constant
    /// while `x * y` and `x * z` do not; an expression whose carrier
    /// cannot be determined refuses.
    pub(super) fn open_term(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let Some(carrier) = self.operand_carrier(handle) else {
            return Err(self.unsupported_expression(handle));
        };
        let mut key = Vec::new();
        self.write_carrier_key(carrier, &mut key);
        self.write_expression_key(handle, &mut key);
        let position = match self.open_terms.get(&key) {
            Some(&position) => position,
            None => {
                let ty = self.carrier_term(carrier);
                let position = self.push_assumption(ty);
                self.open_terms.insert(key, position);
                position
            }
        };
        Ok(self.constant(position))
    }
}

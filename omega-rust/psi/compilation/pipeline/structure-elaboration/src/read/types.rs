/*
The type family of the reader: which typed-tree type is a scalar, by the
`builtin_type_atom` route, which is a record, by its data declaration
(`places`), and the type and value an integer literal lands on.
*/

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};
use symbols::BuiltinTypeAtom;
use terminal_check_tables::{Checked, Construct};
use typed_trees::closed_numeric;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;
use crate::body::ValueType;
use crate::{Unserved, refuse};

fn integer(sign: IntegerSign, bits: u16, machine: MachineId) -> Checked<ScalarType> {
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| refuse(machine, Construct::Machine, Unserved::Type("integer width")))
}

/// The scalar type of a builtin atom, by its compiler-installed slot.
pub(super) fn atom_type(atom: BuiltinTypeAtom, machine: MachineId) -> Checked<ScalarType> {
    match atom {
        BuiltinTypeAtom::Bool => Ok(ScalarType::Boolean),
        BuiltinTypeAtom::I8 => integer(IntegerSign::Signed, 8, machine),
        BuiltinTypeAtom::I16 => integer(IntegerSign::Signed, 16, machine),
        BuiltinTypeAtom::I32 => integer(IntegerSign::Signed, 32, machine),
        BuiltinTypeAtom::I64 => integer(IntegerSign::Signed, 64, machine),
        BuiltinTypeAtom::U8 => integer(IntegerSign::Unsigned, 8, machine),
        BuiltinTypeAtom::U16 => integer(IntegerSign::Unsigned, 16, machine),
        BuiltinTypeAtom::U32 => integer(IntegerSign::Unsigned, 32, machine),
        BuiltinTypeAtom::U64 => integer(IntegerSign::Unsigned, 64, machine),
        BuiltinTypeAtom::Address => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("addr")))
        }
        BuiltinTypeAtom::F32 | BuiltinTypeAtom::F64 => {
            Err(refuse(machine, Construct::Machine, Unserved::Type("float")))
        }
        BuiltinTypeAtom::Slice => Err(refuse(machine, Construct::Machine, Unserved::Type("slice"))),
        BuiltinTypeAtom::SyscallResult => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("syscall result"),
        )),
        BuiltinTypeAtom::Terminal | BuiltinTypeAtom::Never => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("terminal or never"),
        )),
        BuiltinTypeAtom::UInt | BuiltinTypeAtom::Int => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("anonymous integer"),
        )),
        BuiltinTypeAtom::AtomicBool | BuiltinTypeAtom::AtomicU32 | BuiltinTypeAtom::AtomicU64 => {
            Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("atomic"),
            ))
        }
    }
}

/// The integer type a literal landed on, through the same width table.
fn landed_type(
    landed: numerics::literals::LandedIntegerType,
    machine: MachineId,
) -> Checked<ScalarType> {
    use numerics::literals::LandedIntegerType as Landed;
    match landed {
        Landed::I8 => integer(IntegerSign::Signed, 8, machine),
        Landed::I16 => integer(IntegerSign::Signed, 16, machine),
        Landed::I32 => integer(IntegerSign::Signed, 32, machine),
        Landed::I64 => integer(IntegerSign::Signed, 64, machine),
        Landed::U8 => integer(IntegerSign::Unsigned, 8, machine),
        Landed::U16 => integer(IntegerSign::Unsigned, 16, machine),
        Landed::U32 => integer(IntegerSign::Unsigned, 32, machine),
        Landed::U64 => integer(IntegerSign::Unsigned, 64, machine),
        Landed::Addr => Err(refuse(
            machine,
            Construct::Machine,
            Unserved::Type("addr literal"),
        )),
    }
}

impl Program<'_> {
    pub(super) fn scalar_type(
        &self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ScalarType> {
        let typed = self.typed;
        if !handle.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Type("missing type"),
            ));
        }
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Type(what)));
        match typed.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Named { symbol, .. } => {
                if !symbol.is_valid() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Spelling("type without a symbol"),
                    ));
                }
                match typed.symbols.builtin_type_atom(*symbol) {
                    Some(atom) => atom_type(atom, id),
                    None => unserved("nominal type"),
                }
            }
            TypeReferenceNode::Reference { .. } => unserved("reference"),
            TypeReferenceNode::Constrained { .. } => unserved("constrained"),
            TypeReferenceNode::FixedArray { .. } => unserved("fixed array"),
            TypeReferenceNode::Slice { .. } => unserved("slice"),
            TypeReferenceNode::Generic { .. } => unserved("generic application"),
            TypeReferenceNode::ConstExpression(_) => unserved("const expression"),
            TypeReferenceNode::DynamicTrait { .. } => unserved("dynamic trait"),
            TypeReferenceNode::Callable { .. } => unserved("runtime callable"),
            TypeReferenceNode::Unit => unserved("unit as a value"),
        }
    }

    /// Whether two closed types are the same type, when both are builtin
    /// scalar types: decided by the compiler-installed atom each symbol
    /// names, never by a spelling. A type parameter the machine being read
    /// is checked at (`Reading::Alternative`, or a generic machine applied
    /// at a call's type arguments) is the scalar type the scope fixes it
    /// to. `None` for any other operand (a rigid type parameter, a data
    /// type, an application), which is not decided here. generics.md,
    /// "Static type equality": "`==` between type expressions states exact
    /// normalized type equality".
    fn closed_type_equation(
        &self,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> Option<bool> {
        enum Closed {
            Atom(BuiltinTypeAtom),
            Scalar(ScalarType),
        }
        let closed = |handle: TypeReferenceHandle| {
            if !handle.is_valid() {
                return None;
            }
            match self.typed.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    if let Some((_, bound)) = self.scope.iter().find(|(binder, _)| binder == symbol)
                    {
                        return match bound {
                            ValueType::Scalar(scalar_type) => Some(Closed::Scalar(*scalar_type)),
                            ValueType::Rigid(_) | ValueType::Record(_) => None,
                        };
                    }
                    self.typed
                        .symbols
                        .builtin_type_atom(*symbol)
                        .map(Closed::Atom)
                }
                _ => None,
            }
        };
        // The identity only names a refusal, which is discarded here.
        let scalar = |atom: BuiltinTypeAtom| {
            MachineId::new(1).and_then(|machine| atom_type(atom, machine).ok())
        };
        Some(match (closed(left)?, closed(right)?) {
            (Closed::Atom(left), Closed::Atom(right)) => left == right,
            (Closed::Scalar(left), Closed::Scalar(right)) => left == right,
            (Closed::Scalar(fixed), Closed::Atom(atom))
            | (Closed::Atom(atom), Closed::Scalar(fixed)) => scalar(atom)? == fixed,
        })
    }

    /// Whether closed type equations make `expression` false read left to
    /// right: it is decided false, or it is a conjunction one of whose
    /// conjuncts is. generics.md, "Static type equality": "A type equation
    /// in a contract guards what follows it, as a case test does ...
    /// Conjuncts are read left to right; a disjunct whose type equation is
    /// false for the instantiation holds vacuously." So in the machine for
    /// `u16`, the disjunct `T == u8 && result == 7u8` contributes nothing,
    /// and `result == 7u8`, which is not well-typed there, is never read.
    pub(super) fn guarded_false(&self, expression: ExpressionHandle) -> bool {
        use typed_trees::expression::BinaryOperator;
        if self.static_truth(expression) == Some(false) {
            return true;
        }
        if !expression.is_valid() {
            return false;
        }
        match self.typed.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                self.guarded_false(binary.left) || self.guarded_false(binary.right)
            }
            _ => false,
        }
    }

    /// The truth of an expression that closed type equations decide: an
    /// equation between two closed builtin types, a local bound to such a
    /// truth, and `==`, `&&`, `||` and `!` over those, where `==` may also
    /// compare one with a Boolean literal. `None` for every other
    /// expression, a lone Boolean literal included: only a type equation
    /// makes a condition static. generics.md, "Static type equality": "a
    /// branch with established `T == u32` may use that equality to check its
    /// operations and values as u32", so in a machine generated for `u32`
    /// the branch `u32 == u64` is one no value reaches and no rule types.
    pub(super) fn static_truth(&self, expression: ExpressionHandle) -> Option<bool> {
        use typed_trees::expression::{BinaryOperator, UnaryOperator};
        if !expression.is_valid() {
            return None;
        }
        let table = &self.typed.expression_table;
        match table.expression(expression) {
            ExpressionNode::Name(path) => {
                if !path.symbol.is_valid() || table.name_path_members(path.members).len() != 1 {
                    return None;
                }
                self.static_truths
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == path.symbol)
                    .map(|(_, truth)| *truth)
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                self.static_truth(unary.operand).map(|truth| !truth)
            }
            ExpressionNode::Binary(binary) => {
                let literal = |handle: ExpressionHandle| match handle
                    .is_valid()
                    .then(|| table.expression(handle))
                {
                    Some(ExpressionNode::Boolean(value)) => Some(*value),
                    _ => None,
                };
                match binary.operator {
                    BinaryOperator::Equal => {
                        if let (
                            Some(ExpressionNode::TypeExpression(left)),
                            Some(ExpressionNode::TypeExpression(right)),
                        ) = (
                            binary
                                .left
                                .is_valid()
                                .then(|| table.expression(binary.left)),
                            binary
                                .right
                                .is_valid()
                                .then(|| table.expression(binary.right)),
                        ) {
                            return self.closed_type_equation(*left, *right);
                        }
                        match (
                            self.static_truth(binary.left),
                            self.static_truth(binary.right),
                        ) {
                            (Some(left), Some(right)) => Some(left == right),
                            (Some(truth), None) => {
                                literal(binary.right).map(|value| truth == value)
                            }
                            (None, Some(truth)) => literal(binary.left).map(|value| truth == value),
                            (None, None) => None,
                        }
                    }
                    BinaryOperator::And => {
                        Some(self.static_truth(binary.left)? && self.static_truth(binary.right)?)
                    }
                    BinaryOperator::Or => {
                        Some(self.static_truth(binary.left)? || self.static_truth(binary.right)?)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// A formal's, local's, result's or field's type: a record when the
    /// type is a `Named` data declaration, else whatever the scalar route
    /// says, including its refusals.
    pub(super) fn value_type(
        &mut self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        // A type parameter in scope is what the scope says it stands for,
        // found by the binder's symbol.
        if handle.is_valid()
            && let TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(handle)
            && symbol.is_valid()
            && let Some((_, bound)) = self.scope.iter().find(|(binder, _)| binder == symbol)
        {
            return Ok(*bound);
        }
        if handle.is_valid()
            && let TypeReferenceNode::Named { symbol, .. } =
                self.typed.type_reference_table.type_reference(handle)
            && symbol.is_valid()
            && self.typed.symbols.builtin_type_atom(*symbol).is_none()
            && self.data_declaration(*symbol).is_some()
        {
            return self.record(id, *symbol);
        }
        self.scalar_type(id, handle).map(ValueType::Scalar)
    }

    /// A state's result: the invalid handle and `Unit` are both a Unit
    /// machine, because that is how the typed trees record "no result".
    pub(super) fn result_type(
        &mut self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        if !handle.is_valid() {
            return Ok(None);
        }
        // Every other node kind goes through `scalar_type`'s exhaustive match.
        if matches!(
            self.typed.type_reference_table.type_reference(handle),
            TypeReferenceNode::Unit
        ) {
            return Ok(None);
        }
        self.value_type(id, handle).map(Some)
    }

    /// An integer literal that stage 04 left without a landed type: it lands
    /// in the type its context expects (`wiki/spec/language/domains.md`,
    /// "Literals and expected types").
    /// Whether `expression` is wholly anonymous: an integer or decimal
    /// literal no suffix landed, or a binary operation over two such
    /// operands. numeric_values.md: "Decimal literals and wholly anonymous
    /// arithmetic denote exact rationals", so such a tree has no carrier of
    /// its own and lands, complete, in its context's type
    /// (`anonymous_value`). Its partner in a binary operation is therefore
    /// read first.
    pub(super) fn wholly_anonymous(&self, expression: ExpressionHandle) -> bool {
        let table = &self.typed.expression_table;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() {
                return false;
            }
            match table.expression(expression) {
                ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
                ExpressionNode::Float(literal) if literal.landing().is_none() => {}
                ExpressionNode::Binary(binary) => {
                    pending.extend([binary.left, binary.right]);
                }
                _ => return false,
            }
        }
        true
    }

    /// The value a wholly anonymous binary tree lands as in `expected`.
    /// numeric_values.md: anonymous integer-literal division "is rational
    /// division, not truncation", and the tree keeps "exact rational
    /// intermediates until landing requires an integral, in-range value".
    /// So `7 / 2` is `7/2`, never an `i32` division by two that yields 3,
    /// and `(2147483647 + 1) - 1` is an `i32` value although its
    /// intermediate is not. The tree is evaluated exactly, by the same
    /// typed-tree evaluator stage 05 lands it with, and only a value that is
    /// an integer of `expected`, with no fractional intermediate, becomes a
    /// literal. Every other tree is refused, so the current checker decides
    /// the compilation: one whose operator has no anonymous meaning
    /// (remainder, bitwise, shift, comparison: "Builtin remainder requires
    /// integer operand meaning before folding"), one whose value or any
    /// intermediate is fractional, and one outside `expected`.
    pub(super) fn anonymous_value(
        &self,
        id: MachineId,
        expression: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<(IntegerValue, IntegerType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let typed = self.typed;
        let Some(evaluated) = closed_numeric::evaluate_anonymous_value::<true>(
            typed,
            expression,
            &[],
            &mut |expression| closed_numeric::has_anonymous_operator_meaning(typed, expression),
        ) else {
            return unserved("anonymous operation without an exact meaning");
        };
        let Some(value) = evaluated.value.to_integer_exact() else {
            return unserved("fractional anonymous value");
        };
        // "integer landing requires an integral in-range result and
        // explicit `as` when acknowledging fractional intermediates": `(7 /
        // 2) * 2` is 7, but lands only through an `as`, which is outside the
        // subset.
        if evaluated.fractional_origin.is_valid() {
            return unserved("anonymous value with a fractional intermediate");
        }
        let Some(ScalarType::Integer(integer)) = expected else {
            return unserved("anonymous value without an expected integer type");
        };
        let value = match integer.sign() {
            IntegerSign::Signed => value
                .to_i64()
                .map(|value| IntegerValue::Signed(i128::from(value))),
            IntegerSign::Unsigned => value
                .to_u64()
                .map(|value| IntegerValue::Unsigned(u128::from(value))),
        };
        match value {
            Some(value) if integer.admits(value) => Ok((value, integer)),
            Some(_) | None => unserved("anonymous value outside its type"),
        }
    }

    /// An integer literal's value and type: the type stage 04 landed it in,
    /// or else the type its context expects. Both, when present, must agree.
    pub(super) fn literal(
        &self,
        id: MachineId,
        literal: &numerics::literals::IntegerLiteral,
        expected: Option<ScalarType>,
    ) -> Checked<(IntegerValue, IntegerType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Expression(what)));
        let landed = match literal.landing() {
            Some(landing) => {
                if landing.domain != numerics::arithmetic::ArithmeticDomain::Exact {
                    return unserved("integer literal in a non-exact domain");
                }
                Some(landed_type(landing.landed_type, id)?)
            }
            None => None,
        };
        let scalar_type = match (landed, expected) {
            (Some(landed), Some(expected)) if landed != expected => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing("literal landed type"),
                ));
            }
            (Some(scalar_type), _) | (None, Some(scalar_type)) => scalar_type,
            (None, None) => return unserved("anonymous integer literal without an expected type"),
        };
        let ScalarType::Integer(integer) = scalar_type else {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("integer literal in a non-integer place"),
            ));
        };
        let value = match (integer.sign(), literal.is_negative()) {
            (IntegerSign::Signed, _) => literal
                .value_i64()
                .map(|value| IntegerValue::Signed(i128::from(value))),
            (IntegerSign::Unsigned, false) => literal
                .value_u64()
                .map(|value| IntegerValue::Unsigned(u128::from(value))),
            (IntegerSign::Unsigned, true) => None,
        };
        match value {
            Some(value) if integer.admits(value) => Ok((value, integer)),
            Some(_) | None => unserved("integer literal outside its type"),
        }
    }
}

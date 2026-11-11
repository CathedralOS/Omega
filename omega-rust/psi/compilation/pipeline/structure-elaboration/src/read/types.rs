/*
The type family of the reader: which typed-tree type is a scalar, by the
`builtin_type_atom` route, and the type and value an integer literal lands on.
*/

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType};
use symbols::BuiltinTypeAtom;
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::Program;
use crate::{Unserved, refuse};

fn integer(sign: IntegerSign, bits: u16, machine: MachineId) -> Checked<ScalarType> {
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| refuse(machine, Construct::Machine, Unserved::Type("integer width")))
}

/// The scalar type of a builtin atom, by its compiler-installed slot.
fn atom_type(atom: BuiltinTypeAtom, machine: MachineId) -> Checked<ScalarType> {
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
            TypeReferenceNode::Unit => unserved("unit as a value"),
        }
    }

    /// A state's result: the invalid handle and `Unit` are both a Unit
    /// machine, because that is how the typed trees record "no result".
    pub(super) fn result_type(
        &self,
        id: MachineId,
        handle: TypeReferenceHandle,
    ) -> Checked<Option<ScalarType>> {
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
        self.scalar_type(id, handle).map(Some)
    }

    /// An integer literal that stage 04 left without a landed type: it lands
    /// in the type its context expects (`wiki/spec/language/domains.md`,
    /// "Literals and expected types").
    pub(super) fn anonymous_literal(&self, expression: ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                ExpressionNode::Integer(literal) if literal.landing().is_none()
            )
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

//! Exact primitive and scalar representation types.

use symbols::BuiltinTypeAtom;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// Observation permission belongs to the resolved storage atom, not its bits.
/// Constraints do not turn an ordinary integer into an atomic cell.
pub(super) fn scalar_is_atomic(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<bool> {
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    exact_scalar_representation_type(program, reference)?;
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    Some(program.symbols.builtin_type_atom(*symbol)?.is_atomic())
}

pub(crate) fn exact_primitive_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let atom = program.symbols.builtin_type_atom(*symbol)?;
    let primitive = match atom {
        BuiltinTypeAtom::Bool => PrimitiveType::Bool,
        BuiltinTypeAtom::I8 => PrimitiveType::I8,
        BuiltinTypeAtom::I16 => PrimitiveType::I16,
        BuiltinTypeAtom::I32 => PrimitiveType::I32,
        BuiltinTypeAtom::I64 => PrimitiveType::I64,
        BuiltinTypeAtom::U8 => PrimitiveType::U8,
        BuiltinTypeAtom::U16 => PrimitiveType::U16,
        BuiltinTypeAtom::U32 => PrimitiveType::U32,
        BuiltinTypeAtom::U64 => PrimitiveType::U64,
        BuiltinTypeAtom::Address => PrimitiveType::Addr,
        BuiltinTypeAtom::F32 => PrimitiveType::F32,
        BuiltinTypeAtom::F64 => PrimitiveType::F64,
        _ => return None,
    };
    (name.as_str() == atom.symbol_name()).then_some(primitive)
}

pub(crate) fn exact_scalar_representation_type(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return exact_scalar_representation_type(program, *base_type);
        }
        TypeReferenceNode::Named { symbol, name } => (symbol, name),
        _ => return None,
    };
    let atom = program.symbols.builtin_type_atom(*symbol)?;
    let primitive = match atom {
        BuiltinTypeAtom::Bool | BuiltinTypeAtom::AtomicBool => PrimitiveType::Bool,
        BuiltinTypeAtom::I8 => PrimitiveType::I8,
        BuiltinTypeAtom::I16 => PrimitiveType::I16,
        BuiltinTypeAtom::I32 => PrimitiveType::I32,
        BuiltinTypeAtom::I64 => PrimitiveType::I64,
        BuiltinTypeAtom::U8 => PrimitiveType::U8,
        BuiltinTypeAtom::U16 => PrimitiveType::U16,
        BuiltinTypeAtom::U32 | BuiltinTypeAtom::AtomicU32 => PrimitiveType::U32,
        BuiltinTypeAtom::U64 | BuiltinTypeAtom::AtomicU64 => PrimitiveType::U64,
        BuiltinTypeAtom::Address => PrimitiveType::Addr,
        BuiltinTypeAtom::F32 => PrimitiveType::F32,
        BuiltinTypeAtom::F64 => PrimitiveType::F64,
        _ => return None,
    };
    (name.as_str() == atom.symbol_name()).then_some(primitive)
}

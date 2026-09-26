//! Exact primitive and scalar representation types.

use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

pub(crate) fn exact_primitive_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    let atom = program.symbols.builtin_type_atom(*symbol)?;
    let primitive = match atom {
        symbols::BuiltinTypeAtom::Bool => PrimitiveType::Bool,
        symbols::BuiltinTypeAtom::I8 => PrimitiveType::I8,
        symbols::BuiltinTypeAtom::I16 => PrimitiveType::I16,
        symbols::BuiltinTypeAtom::I32 => PrimitiveType::I32,
        symbols::BuiltinTypeAtom::I64 => PrimitiveType::I64,
        symbols::BuiltinTypeAtom::U8 => PrimitiveType::U8,
        symbols::BuiltinTypeAtom::U16 => PrimitiveType::U16,
        symbols::BuiltinTypeAtom::U32 => PrimitiveType::U32,
        symbols::BuiltinTypeAtom::U64 => PrimitiveType::U64,
        symbols::BuiltinTypeAtom::Address => PrimitiveType::Addr,
        symbols::BuiltinTypeAtom::F32 => PrimitiveType::F32,
        symbols::BuiltinTypeAtom::F64 => PrimitiveType::F64,
        _ => return None,
    };
    (name.as_str() == atom.symbol_name()).then_some(primitive)
}

pub(crate) fn exact_scalar_representation_type(
    program: &TypedTrees,
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
        symbols::BuiltinTypeAtom::Bool | symbols::BuiltinTypeAtom::AtomicBool => {
            PrimitiveType::Bool
        }
        symbols::BuiltinTypeAtom::I8 => PrimitiveType::I8,
        symbols::BuiltinTypeAtom::I16 => PrimitiveType::I16,
        symbols::BuiltinTypeAtom::I32 => PrimitiveType::I32,
        symbols::BuiltinTypeAtom::I64 => PrimitiveType::I64,
        symbols::BuiltinTypeAtom::U8 => PrimitiveType::U8,
        symbols::BuiltinTypeAtom::U16 => PrimitiveType::U16,
        symbols::BuiltinTypeAtom::U32 | symbols::BuiltinTypeAtom::AtomicU32 => PrimitiveType::U32,
        symbols::BuiltinTypeAtom::U64 | symbols::BuiltinTypeAtom::AtomicU64 => PrimitiveType::U64,
        symbols::BuiltinTypeAtom::Address => PrimitiveType::Addr,
        symbols::BuiltinTypeAtom::F32 => PrimitiveType::F32,
        symbols::BuiltinTypeAtom::F64 => PrimitiveType::F64,
        _ => return None,
    };
    (name.as_str() == atom.symbol_name()).then_some(primitive)
}

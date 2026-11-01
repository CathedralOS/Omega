//! Source reference descriptor shape, independently of pointee storage layout.

use crate::{TypedTrees, types::TypeReferenceHandle, types::TypeReferenceNode};
use symbols::{BuiltinTypeAtom, SymbolKind};

impl TypedTrees {
    /// Words in a source reference to this referent. Qualifications do not add
    /// metadata; an unresolved type may still become a slice and stays unknown.
    /// This query establishes no byte geometry, formation or access authority.
    pub fn reference_descriptor_word_count(&self, mut referee: TypeReferenceHandle) -> Option<u64> {
        let mut visited = Vec::new();
        loop {
            if !self.type_reference_table.contains_type_reference(referee)
                || visited.contains(&referee)
            {
                return None;
            }
            visited.push(referee);
            match self.type_reference_table.type_reference(referee) {
                TypeReferenceNode::Constrained { base_type, .. } => referee = *base_type,
                TypeReferenceNode::Slice { .. } => return Some(2),
                TypeReferenceNode::FixedArray { .. }
                | TypeReferenceNode::Reference { .. }
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => return Some(1),
                // Legacy consumers still disagree on nominal `string` versus
                // their spelling-based unsized text form. Do not certify a
                // descriptor until that ambiguity has an identity-based owner.
                TypeReferenceNode::Named { name, .. } if name.as_str() == "string" => return None,
                TypeReferenceNode::Named { symbol, .. }
                | TypeReferenceNode::Generic {
                    base_symbol: symbol,
                    ..
                } => {
                    if !symbol.is_valid() {
                        return None;
                    }
                    if let Some(atom) = self.symbols.builtin_type_atom(*symbol) {
                        // The bare Slice constructor is not an applied slice
                        // descriptor; only the Slice node above establishes it.
                        return (atom != BuiltinTypeAtom::Slice).then_some(1);
                    }
                    return match self.symbols.get(*symbol).kind {
                        SymbolKind::Data
                            if self.unique_data_definition_by_symbol(*symbol).is_some() =>
                        {
                            Some(1)
                        }
                        SymbolKind::Machine if self.machine_by_symbol(*symbol).is_some() => Some(1),
                        _ => None,
                    };
                }
                TypeReferenceNode::ConstExpression(_) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data::{DataDefinition, TypeParameter, TypeParameterKind},
        name::Identifier,
    };
    use symbols::{SymbolHandle, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn descriptor_shape_requires_known_referent_kind_without_pointee_geometry() {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("descriptors"));
        let children = symbols.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::Data, SymbolNameRef::Static("Open")),
                (SymbolKind::Data, SymbolNameRef::Static("string")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("u64")),
                (SymbolKind::BuiltinType, SymbolNameRef::Static("Slice")),
            ]),
        );
        let data = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let string = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 1);
        let unknown = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 2);
        let primitive_spelling = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 3);
        let fake_slice = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32 + 4);
        let marker = symbols
            .insert_children(
                data,
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("N"))],
            )
            .handle_at(0);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let named = |program: &mut TypedTrees, symbol: SymbolHandle, name: &str| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                })
        };
        let u64_type = named(
            &mut program,
            children.handle_at(BuiltinTypeAtom::U64.ordinal() as u32),
            "u64",
        );
        let marker_type = named(&mut program, marker, "N");
        let parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: marker,
            name: Identifier::generated("N"),
            kind: TypeParameterKind::Const {
                type_reference: u64_type,
            },
            ..Default::default()
        }]);
        program.push_data_definition(DataDefinition {
            symbol: data,
            name: Identifier::generated("Open"),
            type_parameters: parameters,
            ..Default::default()
        });
        program.push_data_definition(DataDefinition {
            symbol: string,
            name: Identifier::generated("string"),
            ..Default::default()
        });
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([marker_type]);
        let open = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: data,
                base_name: Identifier::generated("Open"),
                lifetime_arguments: Vec::new(),
                arguments,
            });
        assert!(program.closed_data_definition(open).is_none());
        assert_eq!(program.reference_descriptor_word_count(open), Some(1));
        assert_eq!(program.reference_descriptor_word_count(u64_type), Some(1));
        let unknown = named(&mut program, unknown, "T");
        assert_eq!(program.reference_descriptor_word_count(unknown), None);
        let primitive_spelling = named(&mut program, primitive_spelling, "u64");
        assert_eq!(
            program.reference_descriptor_word_count(primitive_spelling),
            None
        );
        let string = named(&mut program, string, "string");
        assert_eq!(program.reference_descriptor_word_count(string), None);
        let unresolved = named(&mut program, SymbolHandle::invalid(), "string");
        assert_eq!(program.reference_descriptor_word_count(unresolved), None);
        let fake_slice = named(&mut program, fake_slice, "Slice");
        assert_eq!(program.reference_descriptor_word_count(fake_slice), None);
        let builtin_slice = named(
            &mut program,
            children.handle_at(BuiltinTypeAtom::Slice.ordinal() as u32),
            "Slice",
        );
        assert_eq!(program.reference_descriptor_word_count(builtin_slice), None);
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: unknown,
            });
        let constrained = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: slice,
                constraints: Default::default(),
            });
        assert_eq!(program.reference_descriptor_word_count(slice), Some(2));
        assert_eq!(
            program.reference_descriptor_word_count(constrained),
            Some(2)
        );
        let reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: slice,
                access: language_semantics::ReferenceAccess::Shared,
                lifetime: None,
            });
        assert_eq!(program.reference_descriptor_word_count(reference), Some(1));
        assert_eq!(
            program.reference_descriptor_word_count(TypeReferenceHandle::invalid()),
            None
        );
        assert_eq!(
            program.reference_descriptor_word_count(TypeReferenceHandle::from_parts(
                open.arena_index(),
                open.generation() + 1
            )),
            None
        );
    }
}

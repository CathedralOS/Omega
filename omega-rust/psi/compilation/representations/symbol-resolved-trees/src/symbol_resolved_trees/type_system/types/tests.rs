use super::{
    ConstExpressionTypeReference, ConstrainedTypeReference, ConstrainedTypeReferenceStorage,
    FixedArrayLength, FixedArrayTypeReference, FixedArrayTypeReferenceStorage,
    GenericTypeReference, GenericTypeReferenceStorage, TypeConstraint, TypeConstraintNode,
    TypeReference, TypeReferenceNode, TypeReferenceTable,
};
use crate::expression::ExpressionTable;
use crate::name::DiagnosticName;
use arena::Arena;
use source::{SourceId, SourceSpan, Span};
use symbols::SymbolHandle;

#[test]
fn type_reference_table_stores_nested_typed_references_as_handles() {
    let mut source_arguments = Arena::<TypeReference>::new();
    let fixed_array_element = source_arguments.append(TypeReference::Named {
        supplied_operand_owner: Default::default(),
        symbol: SymbolHandle::invalid(),
        name: DiagnosticName::generated("u8"),
    });
    let arguments = source_arguments.insert_many([
        TypeReference::Named {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("usize"),
        },
        TypeReference::FixedArray(FixedArrayTypeReference {
            storage: FixedArrayTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                element_type: fixed_array_element,
                length: FixedArrayLength::Literal(16),
            },
        }),
    ]);
    let type_reference = TypeReference::Generic(GenericTypeReference {
        storage: GenericTypeReferenceStorage {
            supplied_operand_owner: Default::default(),
            base_symbol: SymbolHandle::invalid(),
            base_name: DiagnosticName::generated("Result"),
            lifetime_arguments: Vec::new(),
            arguments,
        },
    });

    let source_constraints = Arena::<TypeConstraint>::new();
    let source_expressions = ExpressionTable::new();
    let mut expressions = ExpressionTable::new();
    let mut types = TypeReferenceTable::new();
    let root = types.insert_tree(
        &type_reference,
        &mut expressions,
        &source_arguments,
        &source_constraints,
        &source_expressions,
    );

    assert_eq!(types.type_reference_count(), 4);
    let TypeReferenceNode::Generic { arguments, .. } = types.type_reference(root) else {
        panic!("root type reference should be generic");
    };

    assert_eq!(arguments.count(), 2);
}

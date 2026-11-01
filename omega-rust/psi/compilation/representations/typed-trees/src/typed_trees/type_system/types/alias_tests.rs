//! Storage controls for source obligations through real type-copy boundaries.

use super::*;
use crate::type_aliases::TypeAliasApplication;

fn application(
    table: &mut TypeReferenceTable,
    declaration: SymbolHandle,
    argument: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let arguments = table.insert_type_reference_handles([argument]);
    table.insert(TypeReferenceNode::Generic {
        base_symbol: declaration,
        base_name: Identifier::generated("Alias"),
        lifetime_arguments: Vec::new(),
        arguments,
    })
}

fn retain(
    table: &mut TypeReferenceTable,
    instance: TypeReferenceHandle,
    application: TypeReferenceHandle,
    declaration: SymbolHandle,
) {
    let TypeReferenceNode::Generic {
        arguments,
        lifetime_arguments,
        ..
    } = table.type_reference(application)
    else {
        panic!("application");
    };
    let (arguments, lifetime_arguments) = (*arguments, lifetime_arguments.clone());
    table.retain_type_alias_application(TypeAliasApplication {
        instance,
        application,
        declaration,
        arguments,
        lifetime_arguments,
        ..Default::default()
    });
}

#[test]
fn alias_obligations_survive_copy_actual_substitution_and_complete_applied_views() {
    let parameter = SymbolHandle::from_parts(1, 3);
    let receiving_alias = SymbolHandle::from_parts(11, 2);
    let supplied_alias = SymbolHandle::from_parts(12, 4);
    let mut source = TypeReferenceTable::new();
    let input = source.insert(TypeReferenceNode::Named {
        symbol: parameter,
        name: Identifier::generated("T"),
    });
    let written = source.fresh_occurrence(input);
    let original_application = application(&mut source, receiving_alias, input);
    retain(&mut source, written, original_application, receiving_alias);
    let mut program = crate::TypedTrees::default();
    program.type_reference_table = source.clone();
    let operand = program.static_operand_from_type_reference(written).unwrap();
    assert_eq!(operand.type_reference, written);
    assert!(operand.path.is_empty());
    let mut target = TypeReferenceTable::new();
    let mut expressions = crate::expression::ExpressionTable::default();
    let copied = target.copy_from(
        &source,
        &crate::expression::ExpressionTable::default(),
        &mut expressions,
        written,
    );
    let copied_obligation = target
        .type_alias_applications(copied)
        .next()
        .unwrap()
        .clone();
    assert_eq!(copied_obligation.declaration, receiving_alias);
    let TypeReferenceNode::Generic { arguments, .. } =
        target.type_reference(copied_obligation.application)
    else {
        panic!("copied application");
    };
    assert_eq!(*arguments, copied_obligation.arguments);
    assert!(
        matches!(target.type_reference(target.type_reference_handles(*arguments)[0]), TypeReferenceNode::Named { symbol, .. } if *symbol == parameter)
    );

    let supplied_value = target.insert(TypeReferenceNode::Unit);
    let actual = target.fresh_occurrence(supplied_value);
    let supplied_application = application(&mut target, supplied_alias, supplied_value);
    retain(&mut target, actual, supplied_application, supplied_alias);
    target.substitute_from(copied, actual).unwrap();
    assert_eq!(
        target
            .type_alias_applications(copied)
            .map(|row| row.declaration)
            .collect::<Vec<_>>(),
        [receiving_alias, supplied_alias]
    );
    target.substitute_shared_children(0, parameter, actual);
    let receiving = target
        .type_alias_applications(copied)
        .find(|row| row.declaration == receiving_alias)
        .unwrap();
    assert_eq!(target.type_reference_handles(receiving.arguments), [actual]);

    // A captured-callable producer supplies a complete new roster; installing
    // it must not retain the old receiving frame alongside its applied form.
    let applied = target.fresh_occurrence(actual);
    let applied_application = application(&mut target, receiving_alias, actual);
    retain(&mut target, applied, applied_application, receiving_alias);
    let expected = target
        .type_alias_applications(applied)
        .cloned()
        .collect::<Vec<_>>();
    target.substitute_applied_from(copied, applied).unwrap();
    let actual_rows = target
        .type_alias_applications(copied)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(actual_rows.len(), expected.len());
    for (actual, mut expected) in actual_rows.into_iter().zip(expected) {
        expected.instance = copied;
        assert_eq!(actual, expected);
    }
}

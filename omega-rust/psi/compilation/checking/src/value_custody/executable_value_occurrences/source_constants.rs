//! Original declared Const argument occurrences for source selection.
//! Live type uses select the receiving telescope; their expression graphs keep
//! their own authored call/static lexical custody. A completed actual keeps its
//! source provenance for binding validation; this inventory schedules no jobs.
//! Normalization archives and fixed-array extent sidecars are not new roots.

use checked_trees::{CheckedValueFact, CheckedValueFacts, CheckedValueOrigin};
use typed_trees::data::TypeParameterKind;
use typed_trees::references::DeclarationSelection;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn append_origins(
    program: &dyn crate::CheckingInput,
    roots: &DeclarationSelection,
    facts: &mut CheckedValueFacts,
) {
    let typed = program.typed_program();
    let references = typed_trees::references::collect_selected_program_references(typed, roots);
    for application in references.type_references {
        let original = program
            .type_reference_table
            .data_application_origin(application);
        let source = if original.is_valid() {
            original
        } else {
            application
        };
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(source)
        else {
            continue;
        };
        let parameters = if let Some(data) = typed.data_definition_by_symbol(*base_symbol) {
            typed.data_type_parameters(data)
        } else if let Some(machine) = typed.machine(*base_symbol) {
            typed.machine_type_parameters(machine)
        } else if let Some(declaration) = typed.trait_definition_by_symbol(*base_symbol) {
            typed.trait_type_parameters(declaration)
        } else {
            continue;
        };
        for (argument, parameter) in program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .zip(parameters)
        {
            if !matches!(parameter.kind, TypeParameterKind::Const { .. })
                || !parameter.symbol.is_valid()
                || !base_symbol.is_valid()
            {
                continue;
            }
            // A materialized application and its original source frame are
            // two views of one authored slot. Preserve that source coordinate
            // so retained provenance does not create a competing occurrence.
            let origin = CheckedValueOrigin::TypeConstantArgument {
                declaration: *base_symbol,
                parameter: parameter.symbol,
                application: source,
                argument: *argument,
            };
            append_argument(program, *argument, origin, facts);
        }
    }
}

fn append_argument(
    program: &dyn crate::CheckingInput,
    argument: TypeReferenceHandle,
    origin: CheckedValueOrigin,
    facts: &mut CheckedValueFacts,
) {
    if !argument.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::ConstExpression(expression) => {
            if expression.is_valid()
                && !facts
                    .values
                    .iter()
                    .any(|(_, fact)| fact.expression == *expression && fact.origin == origin)
            {
                facts.values.insert(CheckedValueFact {
                    expression: *expression,
                    origin,
                    type_reference: TypeReferenceHandle::invalid(),
                    primitive_type: None,
                    integer_range: None,
                });
            }
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            append_argument(program, *base_type, origin, facts)
        }
        // Nested applications have their own declared parameter coordinates;
        // type/value slots, reference/collection elements and extent evidence
        // do not inherit the enclosing Const slot as their source frame.
        _ => {}
    }
}

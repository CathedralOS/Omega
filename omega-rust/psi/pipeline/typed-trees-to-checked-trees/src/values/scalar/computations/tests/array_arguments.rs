use super::*;
use checked_trees::{CheckedArrayConstructionSource, CheckedScalarComputationStructuralArgument};

#[test]
fn computation_array_arguments_retain_shape_leaves_and_one_owner() {
    for (array_type, literal) in [
        ("[u8; 2]", "[identity(7u8), 9u8]"),
        ("[u8; 0]", "[]"),
        ("[[u8; 0]; 2]", "[[], []]"),
        ("[[u8; 2]; 2]", "[[7u8, 9u8], [1u8, 2u8]]"),
    ] {
        let checked = checked_source(
            &format!(
                "machine identity(value: u8) -> u8 {{ value }}
             machine answer(row: {array_type}, value: u8) -> u8 {{ value }}
             machine keep(row: [u8; 2]) -> [u8; 2] {{ row }}
             machine selected(value: u8) -> [u8; 2] {{ keep([answer({literal}, value), 9u8]) }}"
            ),
            false,
        );
        let selected = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "selected")
            .unwrap();
        let answer = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "answer")
            .unwrap();
        let plans = &checked.facts.values.scalar_computations;
        let (source_call, arguments) = plans
            .nodes
            .iter()
            .find_map(|(_, node)| match &node.kind {
                CheckedScalarComputationKind::Call {
                    target_machine,
                    source_call,
                    structural_arguments,
                    ..
                } if *target_machine == answer.symbol => {
                    Some((*source_call, *structural_arguments))
                }
                _ => None,
            })
            .expect("array-consuming computation call");
        let [
            CheckedScalarComputationStructuralArgument::Array {
                expression,
                type_reference,
                elements,
            },
        ] = plans.structural_arguments.span(arguments).unwrap()
        else {
            panic!("one computation-owned array actual");
        };
        let target = &checked.machine_states(answer)[0];
        assert_eq!(
            *type_reference,
            checked.state_parameters(target)[0].type_reference
        );
        let expected = validation::scalar_array_elements(
            &checked.typed,
            selected.symbol,
            *expression,
            *type_reference,
        )
        .unwrap();
        let leaves = plans.operands.span(*elements).unwrap();
        assert_eq!(leaves.len(), expected.elements.len());
        for (leaf, (expression, primitive_type)) in leaves.iter().zip(expected.elements) {
            assert_eq!(plans.nodes.get(*leaf).authored_root, expression);
            assert_eq!(plans.nodes.get(*leaf).primitive_type, primitive_type);
        }
        let source = checked.facts.flow.control.calls.get(source_call);
        assert!(
            !plans
                .roots
                .iter()
                .any(|(_, root)| root.machine == selected.symbol
                    && root.statement_ordinal as usize == source.statement_index
                    && matches!(root.role, CheckedScalarExpressionRole::ArrayElement {
                source: CheckedArrayConstructionSource::CallArgument { call_ordinal, .. }, ..
            } if call_ordinal as usize == source.call_ordinal)),
            "computation array has no independent statement roots"
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(selected.symbol)
                .is_some(),
            "all nested leaf calls belong to the retained sequence"
        );
    }
}

#[test]
fn computation_array_parameters_reuse_owned_place_arguments() {
    for array_type in ["[u8; 2]", "[u8; 0]", "[[u8; 0]; 2]"] {
        let checked = checked_source(&format!(
            "machine answer(row: {array_type}, value: u8) -> u8 {{ value }}
             machine selected(row: {array_type}, value: u8) -> [u8; 2] {{ [answer(row, value), 9u8] }}"
        ), false);
        let plans = &checked.facts.values.scalar_computations;
        assert!(
            plans.structural_arguments.iter().any(|(_, argument)| {
                matches!(argument, CheckedScalarComputationStructuralArgument::Place(place)
                if place.source_parameter_index() == Some(0)
                    && place.access == checked_trees::CheckedStructuralAccess::Owned
                    && place.path.is_empty())
            }),
            "{array_type}: whole parameter stays in the existing place namespace"
        );
    }
}

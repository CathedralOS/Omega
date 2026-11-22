#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, LoweredPackage, OperationKind, OperationResult, Parameter, ReturnEdge, ScalarLiteral};
use omega_preparation::{PreparedValue, prepare_source_package};
use omega_syntax::NodeKind;
use omega_typer::NodeSemantics;

#[test]
fn qualified_boolean_patterns_arrive_with_their_selected_declarations_and_values() {
    let text = r"machine choose(x:bool)->u8 {
        match x { east\ON -> 17u8, _ -> 23u8, }
    }
    machine other(x:bool)->u8 {
        match x { west\ON -> 17u8, _ -> 23u8, }
    }";

    for east in [true, false] {
        let eastern = format!("public const ON:bool = {east};");
        let western = format!("public const ON:bool = {};", !east);
        let package = sources::resolve(&[
            ("main.omg", text),
            ("east/value.omg", &eastern),
            ("west/value.omg", &western),
        ]);
        let prepared = prepare_source_package(&package).unwrap();
        let tree = prepared
            .typed()
            .trees()
            .find(|tree| tree.resolved().file.path == "main.omg")
            .unwrap();
        let file = tree.resolved().file;
        let mut selected = Vec::new();

        for pattern in file
            .nodes
            .iter()
            .filter(|node| node.kind == NodeKind::ValuePattern)
        {
            let [operand] = pattern.children[..] else {
                panic!("expected one pattern operand")
            };
            let references: Vec<_> = tree
                .semantics_at(operand)
                .filter_map(|fact| match fact {
                    NodeSemantics::Constant(id) => Some(id),
                    _ => None,
                })
                .collect();
            let [id] = references[..] else {
                panic!("expected one selected constant on the pattern operand")
            };
            let spelling = file.span(operand).slice(text.as_bytes()).unwrap();
            let expected = match spelling {
                br"east\ON" => east,
                br"west\ON" => !east,
                _ => panic!("unexpected pattern operand"),
            };
            assert_eq!(prepared.constant(id), Some(&PreparedValue::Bool(expected)));
            selected.push(id.clone());
        }
        assert_eq!(selected.len(), 2);
        assert_ne!(selected[0], selected[1]);
        let lowered = omega_lowering::lower_package(&prepared).unwrap();
        assert_branches(&lowered, east);

        // Keep both source declarations and their initializers. Change only the
        // values the consumer receives, as if a faulty producer swapped them.
        let swapped = [
            (selected[0].clone(), PreparedValue::Bool(!east)),
            (selected[1].clone(), PreparedValue::Bool(east)),
        ];
        let altered = omega_preparation::PreparedTrees::from_preparation(
            prepared.into_typed(),
            Default::default(),
            swapped.into_iter().collect(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            None,
            Default::default(),
            Default::default(),
            Vec::new(),
        );
        assert_branches(&omega_lowering::lower_package(&altered).unwrap(), !east);
    }
}

fn assert_branches(
    lowered: &LoweredPackage,
    eastern_value: bool,
) {
    for (name, pattern) in [("choose", eastern_value), ("other", !eastern_value)] {
        let body = &lowered
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
            .body;
        let [Parameter::Scalar(subject)] = body.parameters[..] else {
            panic!("expected a Boolean input")
        };
        let BlockTerminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &body.blocks[0].terminator
        else {
            panic!("expected a Boolean branch");
        };
        assert_eq!(*condition, subject);
        for (edge, input) in [(when_true, true), (when_false, false)] {
            let block = &body.blocks[edge.target.get() - 1];
            let [value] = &body.operations[block.operations.clone()] else {
                panic!("expected an arm literal")
            };
            let want = if input == pattern { 17 } else { 23 };
            assert_eq!(value.kind, OperationKind::Constant(ScalarLiteral::Integer(want.into())));
            assert!(matches!(block.terminator, BlockTerminator::Return { result: ReturnEdge::Scalar(v), .. } if value.result == OperationResult::Scalar(v)));
        }
    }
}

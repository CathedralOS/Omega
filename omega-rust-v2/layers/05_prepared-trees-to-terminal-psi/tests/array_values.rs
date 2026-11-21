#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, OperationKind, OperationResult, Parameter, PlaceOrigin, PredicateOperator, PredicateStep, ReturnEdge, ScalarLiteral,
};

#[test]
fn rebinding_an_array_local_preserves_its_earlier_copy() {
    let source = "machine f(left:[u8;2], right:[u8;2], i:u64)->u8 {
        let mutable row=left; let old=row; row=right; old[i] +% row[i]
    }";
    let package = sources::lower(&[("locals.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let [
        Parameter::Structural(left),
        Parameter::Structural(right),
        Parameter::Scalar(i),
    ] = body.parameters[..]
    else {
        panic!("expected two arrays and a selector")
    };
    let [initial, saved, replaced, old_read, current_read, add] = body.operations.as_slice() else {
        panic!("expected three bindings, two reads and addition")
    };
    let (OperationResult::Structural(initial_place), OperationResult::Structural(saved_place), OperationResult::Structural(current_place)) =
        (initial.result, saved.result, replaced.result)
    else {
        panic!("expected three place versions")
    };
    for (copy, root) in [(initial, left), (saved, initial_place), (replaced, right)] {
        spec_corpus::observe_handoff!(
            psi "psi-output/whole-array-copy",
            assert_eq!(copy.kind, OperationKind::StructuralLeafCopy { source: root, path: vec![] }),
        );
        assert_ne!(copy.result, OperationResult::Structural(root));
    }
    assert_ne!(saved_place, current_place);
    for (read, root) in [(old_read, saved_place), (current_read, current_place)] {
        assert!(matches!(&read.kind, OperationKind::PrimitiveScalarRead {source,path} if *source == root && path.len()==1 && path[0].selector == i));
    }
    let OperationKind::Integer { operands, .. } = &add.kind else {
        panic!("expected addition")
    };
    assert_eq!(
        operands
            .iter()
            .copied()
            .map(OperationResult::Scalar)
            .collect::<Vec<_>>(),
        [old_read.result, current_read.result]
    );
}

#[test]
fn each_match_arm_copies_its_selected_array_to_the_join_read_by_the_index() {
    let source = "machine f(t:bool,x:[u8;2],y:[u8;2],i:u64)->u8{(match t {false -> y, _ -> x,})[i]}";
    let package = sources::lower(&[("join.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let [
        Parameter::Scalar(flag),
        Parameter::Structural(x),
        Parameter::Structural(y),
        Parameter::Scalar(i),
    ] = body.parameters[..]
    else {
        panic!("expected flag, arrays and index")
    };
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &body.blocks[0].terminator
    else {
        panic!("expected dispatch")
    };
    assert_eq!(*condition, flag);
    assert_eq!(body.operations.len(), 1);
    for (edge, selected) in [(when_true, x), (when_false, y)] {
        let BlockTerminator::Jump(arrival) = &body.blocks[edge.target.get() - 1].terminator else {
            panic!("expected join")
        };
        assert!(arrival.arguments.is_empty());
        assert_eq!(arrival.structural_arguments, [omega_lowering::ValueOperand::CopyWhole(selected)]);
        let join = &body.blocks[arrival.target.get() - 1];
        let [parameter] = join.structural_parameters[..] else {
            panic!("expected one structural block parameter")
        };
        assert_eq!(body.places[parameter.index()].origin, PlaceOrigin::BlockParameter(join.id));
        assert_eq!(body.places[parameter.index()].ty, body.places[selected.index()].ty);
        let [read] = &body.operations[join.operations.clone()] else {
            panic!("expected one indexed read in the join")
        };
        assert!(matches!(&read.kind, OperationKind::PrimitiveScalarRead {source,path} if *source==parameter && path.len()==1 && path[0].selector==i));
    }
}

#[test]
fn folding_row_length_keeps_the_index_obligation_even_when_no_row_exists() {
    let source = "machine f(matrix:[[u8;3];0], i:u64)->u64{matrix[i].length}";
    let package = sources::lower(&[("bound.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(matrix), Parameter::Scalar(index)] = body.parameters[..] else {
        panic!("expected matrix and index")
    };
    let [read, len] = body.operations.as_slice() else {
        panic!("expected a checked selection before length")
    };
    let OperationKind::StructuralLeafCopy { source: root, path } = &read.kind else {
        panic!("expected a row copy")
    };
    assert_eq!(*root, matrix);
    let [segment] = path.as_slice() else { panic!("expected one bound") };
    assert_eq!(segment.selector, index);
    assert_eq!(
        body.diagnostics.bounds[segment.obligation.get() - 1]
            .span
            .slice(source.as_bytes()),
        Some(b"i".as_slice())
    );
    assert!(matches!(
        package.types.get(body.places[matrix.index()].ty),
        Some(omega_lowering::TypeShape::FixedArray { length: 0, .. })
    ));
    assert_eq!(len.kind, OperationKind::Constant(ScalarLiteral::Integer(3.into())));
}

#[test]
fn an_element_read_keeps_the_bounds_obligation_of_its_own_selector() {
    // L5 lowers the read only when the index node carries the selected core
    // read and its precondition. The twins differ only in the written selector.
    for selected in ["i", "j"] {
        let source = format!("machine f(items:[u8;4], i:u64, j:u64)->u8{{items[{selected}]}}");
        let package = sources::lower(&[("select.omg", &source)]).unwrap();
        let body = &package.machines[0].body;
        let [
            Parameter::Structural(items),
            Parameter::Scalar(i),
            Parameter::Scalar(j),
        ] = body.parameters[..]
        else {
            panic!("expected the array and two selectors")
        };
        let [read] = body.operations.as_slice() else {
            panic!("expected one checked element read")
        };
        let OperationKind::PrimitiveScalarRead { source: root, path } = &read.kind else {
            panic!("expected an element read")
        };
        assert_eq!(*root, items);
        let [segment] = path.as_slice() else { panic!("expected one bound") };
        assert_eq!(body.diagnostics.bounds.len(), 1);
        let clause = body.diagnostics.bounds[segment.obligation.get() - 1]
            .span
            .slice(source.as_bytes());
        let selector = if selected == "i" { i } else { j };
        spec_corpus::observe_handoff!(
            "selected-element-read-precondition/consumer-2",
            assert_eq!((segment.selector, clause), (selector, Some(selected.as_bytes())))
        );
    }
}

#[test]
fn frozen_nested_arrays_retain_their_elements_and_order() {
    let source = "const A:[[u8;2];2]=[[4u8,9u8],[2u8,7u8]]; machine f()->[[u8;2];2]{A}";
    let package = sources::lower(&[("constant.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let quoted = r#"machine f()->[[u8;2];2]{["\x04\x09","\x02\x07"]}"#;
    let direct = sources::lower(&[("quoted.omg", quoted)]).unwrap();
    assert_eq!(direct.machines[0].body.operations, body.operations);
    assert_eq!(direct.machines[0].body.blocks[0].terminator, body.blocks[0].terminator);
    let [four, nine, first, two, seven, second, matrix] = body.operations.as_slice() else {
        panic!("expected four scalars, two rows and a matrix")
    };
    for (op, value) in [(four, 4), (nine, 9), (two, 2), (seven, 7)] {
        spec_corpus::observe_handoff!("frozen-array-elements/consumer", assert_eq!(op.kind, OperationKind::Constant(ScalarLiteral::Integer(value.into()))));
    }
    for (row, a, b) in [(first, four, nine), (second, two, seven)] {
        let OperationKind::EstablishScalarArray { elements } = &row.kind else {
            panic!("expected row")
        };
        assert_eq!(
            elements
                .iter()
                .copied()
                .map(OperationResult::Scalar)
                .collect::<Vec<_>>(),
            [a.result, b.result]
        );
    }
    let OperationKind::EstablishStructuralArray { elements } = &matrix.kind else {
        panic!("expected matrix")
    };
    assert_eq!(
        elements
            .iter()
            .copied()
            .map(OperationResult::Structural)
            .collect::<Vec<_>>(),
        [first.result, second.result]
    );
    assert!(
        matches!(body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if OperationResult::Structural(source)==matrix.result)
    );
}

#[test]
fn owned_byte_literals_lower_like_the_frozen_constant_of_the_same_bytes() {
    let cases: [(&str, &[u8]); 3] = [
        (r#""hi""#, b"hi"),
        (r#""""#, b""),
        (r#""\x00\"é""#, b"\0\"\xc3\xa9"),
    ];
    for (spelling, bytes) in cases {
        let n = bytes.len();
        let frozen = format!("const S:[u8;{n}]={spelling}; machine f()->[u8;{n}]{{S}}");
        let frozen = sources::lower(&[("frozen.omg", frozen.as_str())]).unwrap();
        let expected = &frozen.machines[0].body;
        for body_text in [spelling.to_string(), format!("({spelling})")] {
            let source = format!("machine f()->[u8;{n}]{{{body_text}}}");
            let package = sources::lower(&[("literal.omg", source.as_str())]).unwrap();
            let body = &package.machines[0].body;
            spec_corpus::observe_handoff!(
                psi "psi-output/literal-array-operations",
                assert_eq!(body.operations, expected.operations, "{source}"),
            );
            assert_eq!(body.values, expected.values, "{source}");
            assert_eq!(package.types, frozen.types, "{source}");
            let (array, scalars) = body.operations.split_last().unwrap();
            let literals: Vec<_> = scalars.iter().map(|op| op.kind.clone()).collect();
            let carried: Vec<_> = bytes
                .iter()
                .map(|&byte| OperationKind::Constant(ScalarLiteral::Integer(byte.into())))
                .collect();
            assert_eq!(literals, carried, "{source}");
            let OperationKind::EstablishScalarArray { elements } = &array.kind else {
                panic!("expected a byte array")
            };
            assert_eq!(
                elements
                    .iter()
                    .copied()
                    .map(OperationResult::Scalar)
                    .collect::<Vec<_>>(),
                scalars.iter().map(|op| op.result).collect::<Vec<_>>()
            );
            let OperationResult::Structural(place) = array.result else {
                panic!("expected an owned array place")
            };
            assert!(matches!(
                package.types.get(body.places[place.index()].ty),
                Some(omega_lowering::TypeShape::FixedArray { length, .. }) if *length == n as u64
            ));
            assert!(
                matches!(body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==place),
                "{source}"
            );
        }
    }
}

#[test]
fn each_match_arm_returns_its_own_owned_byte_literal() {
    let source = r#"machine f(b:bool)->[u8;2]{match b {true->("é"),false->"ab",}}"#;
    let package = sources::lower(&[("arms.omg", source)]).unwrap();
    let body = &package.machines[0].body;
    let BlockTerminator::Conditional { when_true, when_false, .. } = &body.blocks[0].terminator else {
        panic!("expected dispatch")
    };
    for (edge, bytes) in [(when_true, [0xc3u8, 0xa9]), (when_false, *b"ab")] {
        let arm = &body.blocks[edge.target.get() - 1];
        let [first, second, array] = &body.operations[arm.operations.clone()] else {
            panic!("expected two bytes and their array")
        };
        for (op, byte) in [(first, bytes[0]), (second, bytes[1])] {
            assert_eq!(op.kind, OperationKind::Constant(ScalarLiteral::Integer(byte.into())));
        }
        let (OperationResult::Scalar(a), OperationResult::Scalar(b)) = (first.result, second.result) else {
            panic!("expected two scalar byte values")
        };
        assert_eq!(array.kind, OperationKind::EstablishScalarArray { elements: vec![a, b] });
        let OperationResult::Structural(place) = array.result else {
            panic!("expected an owned array place")
        };
        assert!(matches!(arm.terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==place));
    }
}

#[test]
fn fixed_length_preserves_receiver_evaluation_and_contract_subjects() {
    let source = "machine f(x:[u8;3],y:[u8;5])->u64 requires x.length < y.length {pick(y).length}
        machine pick(y:[u8;5])->[u8;5] ensures result.length == 5u64 {y}";
    let package = sources::lower(&[("len.omg", source)]).unwrap();
    let [f, pick] = package.machines.as_slice() else {
        panic!("expected two machines")
    };
    assert_eq!(
        f.contract.requires[0].predicate,
        [
            PredicateStep::Formal(0),
            PredicateStep::FixedArrayLength,
            PredicateStep::Formal(1),
            PredicateStep::FixedArrayLength,
            PredicateStep::Apply(PredicateOperator::LessThan)
        ]
    );
    assert_eq!(
        pick.contract.ensures[0].predicate,
        [
            PredicateStep::Result,
            PredicateStep::FixedArrayLength,
            PredicateStep::Integer(5.into()),
            PredicateStep::Apply(PredicateOperator::Equal)
        ]
    );
    let [call, len] = f.body.operations.as_slice() else {
        panic!("expected receiver call before length")
    };
    let Parameter::Structural(y) = f.body.parameters[1] else {
        panic!("expected an array")
    };
    assert!(matches!(&call.kind,OperationKind::Call {callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==pick.id && arguments==&[CallArgument::CopyWhole(y)]));
    assert_eq!(len.kind, OperationKind::Constant(ScalarLiteral::Integer(5.into())));
    assert!(matches!(f.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Scalar(value),..} if len.result==OperationResult::Scalar(value)));
}

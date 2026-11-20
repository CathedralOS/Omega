#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, OperationKind, OperationResult, Parameter, ReturnEdge, TypeShape};

#[test]
fn indexing_a_constructed_matrix_preserves_element_order_and_the_temporary_root() {
    let source = "machine observe(x:u8)->u8{x}
                  machine pick(a:u8, b:u8, i:u64, j:u64)->u8{[[observe(b), a], [b, observe(a)]][i][j]}";
    let package = sources::lower(&[("literal.omg", source)]).unwrap();
    let body = &package.machines[1].body;
    let [
        Parameter::Scalar(a),
        Parameter::Scalar(b),
        Parameter::Scalar(i),
        Parameter::Scalar(j),
    ] = body.parameters[..]
    else {
        panic!("expected four scalar formals")
    };
    let [call_b, first, call_a, second, matrix, read] = body.operations.as_slice() else {
        panic!("expected two calls, three array constructions and one read")
    };
    for (call, argument) in [(call_b, b), (call_a, a)] {
        assert!(matches!(&call.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(callee), arguments, .. }
            if *callee == package.machines[0].id && arguments == &[omega_lowering::CallArgument::Scalar(argument)]));
    }
    let (OperationResult::Scalar(observed_b), OperationResult::Scalar(observed_a)) = (call_b.result, call_a.result) else {
        panic!("expected scalar call results")
    };
    assert_eq!(first.kind, OperationKind::EstablishScalarArray { elements: vec![observed_b, a] });
    assert_eq!(second.kind, OperationKind::EstablishScalarArray { elements: vec![b, observed_a] });
    let (OperationResult::Structural(first_row), OperationResult::Structural(second_row)) = (first.result, second.result) else {
        panic!("expected owned rows")
    };
    assert_eq!(
        matrix.kind,
        OperationKind::EstablishStructuralArray {
            elements: vec![first_row, second_row]
        }
    );
    let OperationKind::PrimitiveScalarRead { source, path } = &read.kind else {
        panic!("expected a scalar read")
    };
    assert_eq!(matrix.result, OperationResult::Structural(*source));
    let [outer, inner] = path.as_slice() else {
        panic!("expected two path segments")
    };
    assert_eq!((outer.selector, inner.selector), (i, j));
}

#[test]
fn empty_arrays_keep_the_element_carrier_even_without_operands() {
    let source = "machine bytes()->[u8; 0]{[]} machine rows()->[[u8; 3]; 0]{[]}";
    let package = sources::lower(&[("empty.omg", source)]).unwrap();
    let [bytes, rows] = package.machines.as_slice() else {
        panic!("expected both machines")
    };
    let [byte_array] = bytes.body.operations.as_slice() else {
        panic!("expected an empty byte array")
    };
    let [row_array] = rows.body.operations.as_slice() else {
        panic!("expected an empty row array")
    };
    assert_eq!(byte_array.kind, OperationKind::EstablishScalarArray { elements: vec![] });
    assert_eq!(row_array.kind, OperationKind::EstablishStructuralArray { elements: vec![] });
    assert_ne!(bytes.result_type, rows.result_type);
    for machine in [bytes, rows] {
        let OperationResult::Structural(result) = machine.body.operations[0].result else {
            panic!("expected an owned array")
        };
        assert_eq!(machine.body.places[result.index()].ty, machine.result_type.unwrap());
    }
}

#[test]
fn selected_subarray_is_copied_to_a_fresh_place_before_the_call() {
    let source = "machine first(row:[u8; 3])->u8{row[0]}
                  machine pick(left:[[u8; 3]; 2], right:[[u8; 3]; 2], index:u64)->u8{first(right[index])}";
    let lowered = sources::lower(&[("subarray.omg", source)]).unwrap();
    let [first, pick] = lowered.machines.as_slice() else {
        panic!("expected two machines")
    };
    let [read, call] = pick.body.operations.as_slice() else {
        panic!("expected a copy followed by a call")
    };
    let [
        Parameter::Structural(left),
        Parameter::Structural(right),
        Parameter::Scalar(index),
    ] = pick.body.parameters[..]
    else {
        panic!("expected two matrices and one selector")
    };
    let OperationKind::StructuralLeafCopy { source: root, path } = &read.kind else {
        panic!("expected a structural leaf copy")
    };
    assert_eq!(*root, right);
    let [segment] = path.as_slice() else { panic!("expected one bound") };
    assert_eq!(segment.selector, index);
    assert_eq!(
        pick.body.diagnostics.bounds[segment.obligation.get() - 1]
            .span
            .slice(source.as_bytes()),
        Some(b"index".as_slice())
    );
    let OperationResult::Structural(row) = read.result else {
        panic!("expected an owned row")
    };
    assert_ne!(row, left);
    assert_ne!(row, right);
    assert_eq!(pick.body.places[row.index()].origin, omega_lowering::PlaceOrigin::Operation(read.id));
    assert_eq!(pick.body.places[row.index()].ty, first.parameter_types[0]);
    assert!(matches!(&call.kind, OperationKind::Call { callee: omega_lowering::CallTarget::Machine(callee), arguments, .. }
        if *callee == first.id && arguments == &[omega_lowering::CallArgument::CopyWhole(row)]));
}

#[test]
fn nested_subarray_return_preserves_selector_calls_bounds_and_result_place() {
    let source = "machine outer(i:u64)->u64{i}
                  machine inner(i:u64)->u64{i}
                  machine row(matrix:[[[bool; 2]; 3]; 4], x:u64, y:u64)->[bool; 2]{(matrix[outer(x)])[inner(y)]}";
    let lowered = sources::lower(&[("return.omg", source)]).unwrap();
    let [inner, outer, row] = lowered.machines.as_slice() else {
        panic!("expected both selectors and their caller")
    };
    let body = &row.body;
    let [
        Parameter::Structural(matrix),
        Parameter::Scalar(x),
        Parameter::Scalar(y),
    ] = body.parameters[..]
    else {
        panic!("expected a matrix and two indices")
    };
    let [first, second, copy] = body.operations.as_slice() else {
        panic!("expected both selector calls before the copy")
    };
    for (op, machine, argument) in [(first, outer, x), (second, inner, y)] {
        assert!(matches!(&op.kind, OperationKind::Call {callee: omega_lowering::CallTarget::Machine(callee), arguments, ..}
            if *callee == machine.id && arguments == &[omega_lowering::CallArgument::Scalar(argument)]));
    }
    let OperationKind::StructuralLeafCopy { source: root, path } = &copy.kind else {
        panic!("expected a row copy")
    };
    assert_eq!(*root, matrix);
    let [first_index, second_index] = path.as_slice() else {
        panic!("expected two bounds")
    };
    assert_ne!(first_index.obligation, second_index.obligation);
    for (segment, call, occurrence) in [
        (first_index, first, b"outer(x)".as_slice()),
        (second_index, second, b"inner(y)".as_slice()),
    ] {
        assert_eq!(call.result, OperationResult::Scalar(segment.selector));
        assert_eq!(
            body.diagnostics.bounds[segment.obligation.get() - 1]
                .span
                .slice(source.as_bytes()),
            Some(occurrence)
        );
    }
    let OperationResult::Structural(copied) = copy.result else {
        panic!("expected a structural result")
    };
    let BlockTerminator::Return {
        result: ReturnEdge::Structural { source, result },
        ..
    } = body.blocks[0].terminator
    else {
        panic!("expected a structural return")
    };
    assert_eq!(source, copied);
    assert_ne!(result, copied);
    assert_eq!(body.places[result.index()].ty, row.result_type.unwrap());
}

#[test]
fn nested_indices_keep_their_own_root_selectors_and_bounds() {
    let source = "machine read(left:[[u8; 2]; 3], right:[[u8; 2]; 3], row:u64, col:u64)->u8{right[row][col]}";
    let lowered = sources::lower(&[("access.omg", source)]).unwrap();
    let block = &lowered.machines[0].body;
    let [
        Parameter::Structural(left),
        Parameter::Structural(right),
        Parameter::Scalar(row),
        Parameter::Scalar(col),
    ] = block.parameters.as_slice()
    else {
        panic!("expected owned arrays and scalar selectors");
    };
    assert_ne!(left, right);
    let [operation] = block.operations.as_slice() else {
        panic!("expected one scalar read");
    };
    let OperationKind::PrimitiveScalarRead { source: root, path } = &operation.kind else {
        panic!("expected primitive scalar read");
    };
    assert_eq!(root, right);
    let [outer, inner] = path.as_slice() else {
        panic!("expected both index segments");
    };
    assert_eq!((outer.selector, inner.selector), (*row, *col));
    assert_ne!(outer.obligation, inner.obligation);
    let OperationResult::Scalar(result) = operation.result else {
        panic!("expected scalar read result");
    };
    assert!(matches!(
        block.blocks[0].terminator,
        BlockTerminator::Return { result: ReturnEdge::Scalar(value), .. } if value == result
    ));

    let root_type = block.places[root.index()].ty;
    let Some(TypeShape::FixedArray {
        length: 3,
        element: inner_type,
    }) = lowered.types.get(root_type)
    else {
        panic!("the first bound comes from the outer extent");
    };
    assert!(matches!(lowered.types.get(*inner_type), Some(TypeShape::FixedArray { length: 2, .. })));
    let bound_source = |id: omega_lowering::BoundsObligationId| {
        block.diagnostics.bounds[id.get() - 1]
            .span
            .slice(source.as_bytes())
    };
    assert_eq!(bound_source(outer.obligation), Some(b"row".as_slice()));
    assert_eq!(bound_source(inner.obligation), Some(b"col".as_slice()));
}

#[test]
fn a_contract_on_one_view_cannot_supply_another_views_read_length() {
    use omega_lowering::{PredicateOperator, PredicateStep};
    for (read_view, slot) in [("left", 0), ("right", 1)] {
        let text = format!("machine get(left:@[u8],right:@[u8],i:u64)->u8 requires i < left.len {{ {read_view}[i] }}");
        let package = sources::lower(&[("views.omg", &text)]).unwrap();
        let machine = &package.machines[0];
        let body = &machine.body;
        assert_eq!(
            machine.contract.requires[0].predicate,
            [
                PredicateStep::Formal(2),
                PredicateStep::Formal(0),
                PredicateStep::ByteSequenceLength,
                PredicateStep::Apply(PredicateOperator::LessThan)
            ]
        );
        let Parameter::Structural(selected) = body.parameters[slot] else {
            panic!("expected shared byte view")
        };
        let Parameter::Scalar(selector) = body.parameters[2] else {
            panic!("expected index")
        };
        let [measure, read] = body.operations.as_slice() else {
            panic!("expected measure and read")
        };
        assert_eq!(measure.kind, OperationKind::ByteSequenceLength { source: selected });
        let OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
        } = read.kind
        else {
            panic!("expected byte read")
        };
        assert_eq!((source, index), (selected, selector));
        assert_eq!(OperationResult::Scalar(length), measure.result);
        assert_eq!(
            body.diagnostics.bounds[obligation.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(&b"i"[..])
        );
    }
}

#[test]
fn an_explicit_byte_length_reads_the_descriptor_forwarded_through_a_join() {
    let text = "machine count(items:@[u8])->u64 { items.len }
              machine choose(flag:bool,left:@[u8],right:@[u8])->u64 {
                  let selected = match flag {true -> left, false -> right,}; count(selected)
              }";
    let package = sources::lower(&[("count.omg", text)]).unwrap();
    let [choose, count] = package.machines.as_slice() else {
        panic!("expected both machines")
    };
    let [length] = count.body.operations.as_slice() else {
        panic!("expected one length read")
    };
    let [Parameter::Structural(items)] = count.body.parameters[..] else {
        panic!("expected a view parameter")
    };
    assert_eq!(length.kind, OperationKind::ByteSequenceLength { source: items });
    let [call] = choose.body.operations.as_slice() else {
        panic!("expected one call")
    };
    let chosen = choose.body.blocks.last().unwrap().structural_parameters[0];
    assert!(
        matches!(&call.kind,OperationKind::Call{callee: omega_lowering::CallTarget::Machine(callee),arguments,..} if *callee==count.id && arguments==&[omega_lowering::CallArgument::CopyWhole(chosen)])
    );
}

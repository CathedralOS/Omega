use omega_lowering::{LoweredMachine, LoweredPackage, Parameter, PlaceOrigin, ReturnEdge, ScalarLiteral, ScalarType, TypeShape, ValueOrigin, lower_package};
use omega_preparation::prepare_source_package;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;

fn lower(source: &str) -> LoweredPackage {
    let tokens = omega_lexer::lex(source.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(source.as_bytes(), &tokens).unwrap();
    let package = resolve_source_package(&[StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: "returns.omg",
            text: source,
            tokens: &tokens,
            tree: &tree,
        },
    }])
    .unwrap();
    let prepared = prepare_source_package(&package).unwrap();
    lower_package(&prepared).unwrap()
}

fn assert_constant(
    machine: &LoweredMachine,
    expected: ScalarLiteral,
) {
    assert_eq!(machine.body.id.get(), 1);
    assert_eq!(machine.body.return_edge_id.get(), 1);
    let ReturnEdge::Scalar(result) = machine.body.return_edge else {
        panic!("expected a scalar return, got {:?}", machine.body.return_edge);
    };
    let [operation] = machine.body.operations.as_slice() else {
        panic!("expected exactly one constant operation");
    };
    assert_eq!(operation.id.get(), 1);
    assert_eq!(operation.result, result);
    assert_eq!(operation.literal, expected);
    let value = &machine.body.values[result.index()];
    assert_eq!(value.origin, ValueOrigin::Operation(operation.id));
    assert_eq!(Some(value.ty), machine.result_type);
}

#[test]
fn scalar_return_uses_the_exact_constant_or_boolean_literal() {
    let (first, second) = {
        let first_source = String::from(
            "const A:u8=1u8;
             const B:u8=2u8;
             const OFF:bool=false;
             machine integer(unused:u8)->u8{A}
             machine literal_bool()->bool{true}
             machine named_bool()->bool{OFF}
             machine unit(){}",
        );
        let second_source = first_source.replace("{A}", "{B}");
        (lower(&first_source), lower(&second_source))
    };
    for (package, integer) in [(&first, 1), (&second, 2)] {
        let [scalar, literal_bool, named_bool, unit] = package.machines.as_slice() else {
            panic!("expected the four source machines");
        };
        let declarations: Vec<_> = package
            .machines
            .iter()
            .map(|machine| (machine.id.get(), machine.declaration.path.as_str()))
            .collect();
        assert_eq!(
            declarations,
            [
                (1, "integer"),
                (2, "literal_bool"),
                (3, "named_bool"),
                (4, "unit")
            ]
        );
        assert_constant(scalar, ScalarLiteral::Integer(integer.into()));
        assert_eq!(scalar.body.values.len(), 2);
        assert_eq!(scalar.body.operations[0].result.get(), 2);
        assert_eq!(
            scalar.result_type.and_then(|id| package.types.get(id)),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 }))
        );
        assert_constant(literal_bool, ScalarLiteral::Boolean(true));
        assert_constant(named_bool, ScalarLiteral::Boolean(false));
        for boolean in [literal_bool, named_bool] {
            assert_eq!(boolean.body.values.len(), 1);
            assert_eq!(
                boolean.result_type.and_then(|id| package.types.get(id)),
                Some(&TypeShape::PrimitiveScalar(ScalarType::Boolean))
            );
        }
        assert_eq!(unit.result_type, None);
        assert_eq!(unit.body.id.get(), 1);
        assert_eq!(unit.body.return_edge_id.get(), 1);
        assert_eq!(unit.body.return_edge, ReturnEdge::Unit);
        assert!(unit.body.operations.is_empty());
        assert!(unit.body.values.is_empty());
        assert!(unit.body.places.is_empty());
    }
}

#[test]
fn scalar_returns_use_parameter_identity_across_structural_parameters() {
    let first = lower("machine select(a:u8, array:[u8; 3], b:u8)->u8{a}");
    let second = lower("machine select(a:u8, array:[u8; 3], b:u8)->u8{b}");
    let [left] = first.machines.as_slice() else {
        panic!("expected one machine");
    };
    let [right] = second.machines.as_slice() else {
        panic!("expected one machine");
    };
    assert_eq!(left.id, right.id);
    assert_eq!(left.body.parameters, right.body.parameters);
    assert_eq!(left.body.values, right.body.values);
    assert_eq!(left.body.places, right.body.places);

    let [
        Parameter::Scalar(a),
        Parameter::Structural(array),
        Parameter::Scalar(b),
    ] = left.body.parameters[..]
    else {
        panic!("expected two scalar parameters and one structural parameter in source order");
    };
    assert_eq!(a.get(), 1);
    assert_eq!(array.get(), 1);
    assert_eq!(b.get(), 2);
    for (machine, selected) in [(left, a), (right, b)] {
        assert_eq!(machine.body.id.get(), 1);
        assert_eq!(machine.body.return_edge_id.get(), 1);
        assert_eq!(machine.body.return_edge, ReturnEdge::Scalar(selected));
        let value = &machine.body.values[selected.index()];
        assert_eq!(value.origin, ValueOrigin::Parameter);
        assert_eq!(Some(value.ty), machine.result_type);
        let place = &machine.body.places[array.index()];
        assert_eq!(place.origin, PlaceOrigin::Parameter);
        assert_eq!(place.ty, machine.parameter_types[1]);
        assert!(machine.body.operations.is_empty());
    }
}

#[path = "support/source.rs"]
mod sources;

#[path = "support/returns.rs"]
mod returns;
use returns::sole_return;

use omega_lowering::{LoweredMachine, LoweredPackage, Parameter, PlaceOrigin, ReturnEdge, ScalarLiteral, ScalarType, TypeShape, ValueOrigin};

fn lower(source: &str) -> LoweredPackage {
    sources::lower(&[("returns.omg", source)]).unwrap()
}

fn assert_constant(
    machine: &LoweredMachine,
    expected: ScalarLiteral,
) {
    let ReturnEdge::Scalar(result) = sole_return(&machine.body) else {
        panic!("expected a scalar return, got {:?}", sole_return(&machine.body));
    };
    let [operation] = machine.body.operations.as_slice() else {
        panic!("expected exactly one constant operation");
    };
    assert_eq!(operation.id.get(), 1);
    assert_eq!(operation.result, omega_lowering::OperationResult::Scalar(result));
    assert_eq!(operation.kind, omega_lowering::OperationKind::Constant(expected));
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
        spec_corpus::observe_handoff!(
            psi "psi-output/package-machine-ids",
            assert_eq!(
                declarations,
                [
                    (1, "integer"),
                    (2, "literal_bool"),
                    (3, "named_bool"),
                    (4, "unit")
                ]
            ),
        );
        assert_constant(scalar, ScalarLiteral::Integer(integer.into()));
        assert_eq!(scalar.body.values.len(), 2);
        let omega_lowering::OperationResult::Scalar(result) = scalar.body.operations[0].result else {
            panic!("expected scalar")
        };
        assert_eq!(result.get(), 2);
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
        assert_eq!(sole_return(&unit.body), ReturnEdge::Unit);
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
        assert_eq!(sole_return(&machine.body), ReturnEdge::Scalar(selected));
        let value = &machine.body.values[selected.index()];
        assert_eq!(value.origin, ValueOrigin::Parameter);
        assert_eq!(Some(value.ty), machine.result_type);
        let place = &machine.body.places[array.index()];
        assert_eq!(place.origin, PlaceOrigin::Parameter);
        assert_eq!(place.ty, machine.parameter_types[1]);
        assert!(machine.body.operations.is_empty());
    }
}

// This probe checks one return operand after lowering has dropped the source.
// A successful lookup establishes only the operand's existence and type.
fn check_scalar_return_operand(machine: &LoweredMachine) -> Result<(), &'static str> {
    let ReturnEdge::Scalar(operand) = sole_return(&machine.body) else {
        return Err("expected a scalar return edge");
    };
    let value = machine
        .body
        .values
        .get(operand.index())
        .ok_or("return operand is absent from this machine")?;
    if Some(value.ty) != machine.result_type {
        return Err("return operand type differs from the declared result");
    }
    Ok(())
}

#[test]
fn scalar_return_consumer_checks_the_emitted_operand_in_its_machine() {
    let mut lowered = lower(
        "machine select(a:u8, b:u8, flag:bool)->u8{b}
         machine other(w:u8, x:u8, y:u8, z:u8)->u8{z}",
    );
    let missing_operand = &lowered
        .machines
        .iter()
        .find(|m| m.declaration.path == "other")
        .unwrap()
        .body;
    let missing_operand = sole_return(missing_operand);
    let select = lowered
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "select")
        .unwrap();
    let [
        Parameter::Scalar(a),
        Parameter::Scalar(b),
        Parameter::Scalar(flag),
    ] = select.body.parameters[..]
    else {
        panic!("expected three scalar parameters");
    };
    assert_eq!(sole_return(&select.body), ReturnEdge::Scalar(b));
    assert_eq!(check_scalar_return_operand(select), Ok(()));

    replace_return(select, ReturnEdge::Scalar(a));
    assert_eq!(check_scalar_return_operand(select), Ok(()));

    replace_return(select, ReturnEdge::Scalar(flag));
    assert_eq!(check_scalar_return_operand(select), Err("return operand type differs from the declared result"));

    // The second machine has a fourth value. The first has only three.
    replace_return(select, missing_operand);
    assert_eq!(check_scalar_return_operand(select), Err("return operand is absent from this machine"));
}
fn replace_return(
    machine: &mut LoweredMachine,
    operand: ReturnEdge,
) {
    let omega_lowering::BlockTerminator::Return { result, .. } = &mut machine.body.blocks[0].terminator else {
        panic!("expected a return");
    };
    *result = operand;
}

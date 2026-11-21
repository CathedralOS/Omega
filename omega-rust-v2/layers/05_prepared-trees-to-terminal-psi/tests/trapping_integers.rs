#[path = "support/returns.rs"]
mod returns;
#[path = "support/source.rs"]
mod source;
use omega_lowering::{BlockTerminator, CleanupAction, CrashCause, OperationKind, Parameter, PredicateOperator, PredicateStep, TrappingIntegerPrimitive};
use psi_handoff_consumer::{DivisionOutcome, observe_byte_division};

#[test]
fn division_traps_at_the_selected_operation_and_only_a_return_runs_cleanup() {
    for (guard, divisor, formal) in [("x", "x", 1), ("y", "y", 2), ("y", "x", 2)] {
        let program = format!(
            "data Lease dropped by finish {{mark:u8;}} machine finish(l:Lease){{let Lease{{mark as _}}=l;}}
            machine quotient(a:u8,x:u8,y:u8,lease:Lease)->u8 crashes Trap {guard} == 0 {{a /! {divisor}}}"
        );
        let resolved = source::resolve(&[("trap.omg", &program)]);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let typed = prepared.typed();
        let selection = typed
            .operations
            .iter()
            .find(|operation| operation.token == "/!")
            .unwrap();
        assert!(
            typed
                .obligations
                .iter()
                .any(|obligation| obligation.node == selection.node
                    && matches!(&obligation.kind,omega_typed::ObligationKind::CrashCoverage{machine,cause:CrashCause::Trap} if machine.path=="quotient"))
        );
        let package = omega_lowering::lower_package(&prepared).unwrap();
        drop(prepared);
        let m = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "quotient")
            .unwrap();
        let [route] = m.contract.crash_routes.as_slice() else {
            panic!("one published crash route")
        };
        assert_eq!(route.cause, CrashCause::Trap);
        assert_eq!(
            route.guard.as_ref().unwrap().predicate,
            [
                PredicateStep::Formal(formal),
                PredicateStep::Integer(0.into()),
                PredicateStep::Apply(PredicateOperator::Equal)
            ]
        );
        let [
            Parameter::Scalar(a),
            Parameter::Scalar(x),
            Parameter::Scalar(y),
            Parameter::Structural(lease),
        ] = m.body.parameters[..]
        else {
            panic!("fixture inputs")
        };
        let [op] = m.body.operations.as_slice() else { panic!("one operation") };
        let selected = if divisor == "x" { x } else { y };
        assert_eq!(
            op.kind,
            OperationKind::TrappingInteger {
                primitive: TrappingIntegerPrimitive::Divide,
                left: a,
                right: selected
            }
        );
        let [block] = m.body.blocks.as_slice() else {
            panic!("no separate crash block")
        };
        let BlockTerminator::Return { id, cleanup, .. } = &block.terminator else {
            panic!("ordinary completion")
        };
        assert_eq!(m.body.diagnostics.edges.len(), 1);
        let [CleanupAction::Invoke { place, target, .. }] = cleanup.as_slice() else {
            panic!("lease needs normal cleanup")
        };
        assert_eq!(place.root, lease);
        assert_eq!(
            *target,
            package
                .machines
                .iter()
                .find(|d| d.declaration.path == "finish")
                .unwrap()
                .id
        );
        let other = if selected == x { y } else { x };
        assert_eq!(
            observe_byte_division(&package, &m.body, &[(a, 12), (selected, 0), (other, 4)]),
            Some(DivisionOutcome::Crash {
                operation: op.id,
                cause: CrashCause::Trap
            })
        );
        assert_eq!(
            observe_byte_division(&package, &m.body, &[(a, 12), (selected, 4), (other, 0)]),
            Some(DivisionOutcome::Return { edge: *id, value: 3, cleanup })
        );
    }
}

#[test]
fn binary_trapping_forms_preserve_the_primitive_and_both_carriers() {
    use TrappingIntegerPrimitive::*;
    use omega_lowering::{ReturnEdge, ScalarType, TypeShape};
    for (token, primitive, right_type, signed, width) in [
        ("+!", Add, "i8", true, 8),
        ("-!", Subtract, "i8", true, 8),
        ("*!", Multiply, "i8", true, 8),
        ("/!", Divide, "i8", true, 8),
        ("%!", Remainder, "i8", true, 8),
        ("<<!", ShiftLeft, "u13", false, 13),
        (">>!", ShiftRight, "i13", true, 13),
    ] {
        let program = format!("machine apply(a:i8,b:{right_type})->i8 crashes Trap {{a {token} b}}");
        let package = source::lower(&[("primitive.omg", &program)]).unwrap();
        let body = &package.machines[0].body;
        let [op] = body.operations.as_slice() else { panic!("one operation") };
        let [Parameter::Scalar(left), Parameter::Scalar(right)] = body.parameters[..] else {
            panic!("two scalar inputs")
        };
        assert_eq!(op.kind, OperationKind::TrappingInteger { primitive, left, right });
        assert_eq!(
            package.types.get(body.values[right.index()].ty),
            Some(&TypeShape::PrimitiveScalar(ScalarType::Integer { signed, width }))
        );
        let omega_lowering::OperationResult::Scalar(result) = op.result else {
            panic!("a successful operation defines its result")
        };
        assert_eq!(body.values[result.index()].ty, body.values[left.index()].ty);
        assert_eq!(returns::sole_return(body), ReturnEdge::Scalar(result), "{token}");
    }
}

#[test]
fn exact_division_keeps_its_proof_obligations_and_has_no_trapping_operation() {
    let p = source::lower(&[("exact.omg", "machine divide(a:u8,b:u8)->u8 requires b != 0 crashes none {a / b}")]).unwrap();
    let [op] = p.machines[0].body.operations.as_slice() else {
        panic!("one operation")
    };
    let OperationKind::Integer { operation, .. } = &op.kind else {
        panic!("exact division")
    };
    assert_eq!(operation.obligations, [omega_lowering::IntegerObligation::NonzeroDivisor]);
    assert!(p.machines[0].contract.crash_routes.is_empty());
}

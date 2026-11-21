#[path = "support/returns.rs"]
mod returns;
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{OperationKind, OperationResult, ReturnEdge, ScalarLiteral, ValueOrigin};
use omega_preparation::{PreparationFailure, PreparationReason, RefusalCause, prepare_source_package_diagnosed};

const CALLBACKS: &str = "data Step=machine(n:u8)->u8;
    machine keep(n:u8)->u8{n}
    machine step(n:u8)->u8 crashes Trap n == 0 {n +% 2u8}
    machine apply<const F:Step>(n:u8)->u8{F(n)}";

#[test]
fn psi_returns_the_result_of_the_admitted_callback() {
    // All three constants are prepared together. Caching by operand alone would
    // give the same result to keep(7) and step(7); caching by callback alone
    // would conflate keep(7) and keep(0).
    let source = "const A:u8=apply::<keep>(7u8); const B:u8=apply::<step>(7u8); const C:u8=apply::<keep>(0u8);
        machine read_a()->u8{A} machine read_b()->u8{B} machine read_c()->u8{C}";
    let package = sources::resolve(&[("callbacks.omg", CALLBACKS), ("reader.omg", source)]);
    let prepared = prepare_source_package_diagnosed(&package).unwrap();
    let psi = omega_lowering::lower_package(&prepared).unwrap();
    for (constant, machine, expected) in [("A", "read_a", 7u8), ("B", "read_b", 9), ("C", "read_c", 0)] {
        let declaration = &prepared.typed().constants.iter().find(|c| c.declaration.path == constant).unwrap().declaration;
        assert_eq!(prepared.integer_constant(declaration), Some(&expected.into()), "{constant}");
        let read = psi.machines.iter().find(|m| m.declaration.path == machine).unwrap();
        let ReturnEdge::Scalar(value) = returns::sole_return(&read.body) else {
            panic!("expected scalar read")
        };
        let ValueOrigin::Operation(id) = read.body.values[value.index()].origin else {
            panic!("result has no defining operation")
        };
        let op = &read.body.operations[id.index()];
        assert_eq!(op.result, OperationResult::Scalar(value));
        spec_corpus::observe_handoff!(
            "static-invocation-admission/psi-result",
            assert_eq!(
                op.kind,
                OperationKind::Constant(ScalarLiteral::Integer(expected.into())),
                "{machine} reads {constant}"
            )
        );
    }

    // Zero was admissible for keep. At the very same operand step's published
    // guard fires before its wrapping addition. This control never reaches L5.
    let source = "const RESULT:u8=apply::<step>(0u8); machine read()->u8{RESULT}";
    let package = sources::resolve(&[("callbacks.omg", CALLBACKS), ("reader.omg", source)]);
    let PreparationFailure::Preparation(refusal) = prepare_source_package_diagnosed(&package).unwrap_err() else {
        panic!("expected invocation admission to reject the selected guard")
    };
    assert_eq!(refusal.cause, RefusalCause::Preparation(PreparationReason::TrapRoutePossible));
    let error = refusal.diagnostic;
    assert_eq!((error.package.as_str(), error.file.as_str()), ("app", "callbacks.omg"));
    assert_eq!(error.span.slice(CALLBACKS.as_bytes()), Some(b"F(n)".as_slice()));
}

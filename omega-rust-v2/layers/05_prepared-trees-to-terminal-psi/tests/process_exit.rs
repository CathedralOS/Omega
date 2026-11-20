//! External process exit is a no-successor terminator for the exact canonical
//! requirement only. A same-spelled ordinary declaration stays an ordinary call.
#[path = "support/source.rs"]
mod source;
use omega_lowering::{BlockTerminator, CallArgument, ExternalCompletionKind, MemberRealization, Parameter};

#[test]
fn the_canonical_requirement_lowers_to_a_terminal_exit_with_its_exact_operand() {
    let template = "use omega\\language\\core;
        machine quit(code:i32, other:i32) -> core\\Never reaches core\\ProcessExit { core\\ProcessExit::exit_process(code) }";
    for (chosen, index) in [("code", 0), ("other", 1)] {
        let program = template.replace("exit_process(code)", &format!("exit_process({chosen})"));
        let package = source::lower(&[("quit.omg", &program)]).unwrap();
        let quit = &package.machines[0];
        let declared = quit.result_type.expect("the wrapper publishes Never");
        let Some(omega_lowering::TypeShape::Nominal(id)) = package.types.get(declared) else {
            panic!("Never is ordinary nominal data")
        };
        let never = &package.data[id.get() - 1];
        assert_eq!(
            (never.declaration.package.as_str(), never.declaration.path.as_str()),
            ("@omega/toolchain", "language\\core\\Never")
        );
        let [requires] = never.requires.as_slice() else {
            panic!("Never retains its defining requirement")
        };
        assert_eq!(requires.predicate, [omega_lowering::PredicateStep::Boolean(false)]);
        assert!(quit.body.operations.is_empty(), "the operand is a parameter read, not a call");
        assert!(package.member_calls.is_empty(), "exit is not an ordinary member selection");
        let [block] = quit.body.blocks.as_slice() else {
            panic!("exit ends the only block")
        };
        let BlockTerminator::ExternalExit(exit) = &block.terminator else {
            panic!("expected a terminal external exit")
        };
        assert_eq!(psi_handoff_consumer::external_exits(&quit.body), [exit]);
        assert_eq!(exit.kind, ExternalCompletionKind::ProcessExit);
        assert_eq!(exit.owner.package, "@omega/toolchain");
        assert_eq!(exit.owner.path, "language\\core\\ProcessExit");
        assert_eq!(exit.requirement.package, "@omega/toolchain");
        assert_eq!(exit.requirement.path, "language\\core\\ProcessExit::exit_process");
        let Parameter::Scalar(selected) = quit.body.parameters[index] else {
            panic!("i32 parameters are scalar")
        };
        assert_eq!(exit.arguments, [CallArgument::Scalar(selected)]);
        assert!(exit.requirements.is_empty());
        let site = &quit.body.diagnostics.edges[exit.id.get() - 1];
        assert_eq!(
            site.span.slice(program.as_bytes()),
            Some(format!("core\\ProcessExit::exit_process({chosen})").as_bytes())
        );
    }
}

#[test]
fn a_grouped_exit_statement_has_no_normal_continuation() {
    let text = "use omega\\language\\core; machine quit(code:i32)->i32 reaches core\\ProcessExit { (core\\ProcessExit::exit_process(code)); 7 }";
    let package = source::lower(&[("quit.omg", text)]).unwrap();
    let quit = &package.machines[0];
    let [block] = quit.body.blocks.as_slice() else {
        panic!("exit has no continuation")
    };
    assert!(matches!(block.terminator, BlockTerminator::ExternalExit(_)));
    assert!(quit.body.operations.is_empty(), "the trailing return value is unreachable");
}

#[test]
fn a_same_spelled_local_requirement_is_an_ordinary_boundary_call() {
    let text = "boundary trait ProcessExit { machine exit_process(return_code:i32)->i32 reaches ProcessExit crashes none terminates; }
        machine stop(code:i32)->i32 reaches ProcessExit { ProcessExit::exit_process(code) }";
    let package = source::lower(&[("stop.omg", text)]).unwrap();
    let stop = &package.machines[0];
    assert!(psi_handoff_consumer::external_exits(&stop.body).is_empty());
    let [call] = stop.body.operations.as_slice() else {
        panic!("expected one ordinary call")
    };
    let (selection, arguments) = psi_handoff_consumer::member_call(&package, &call.kind).expect("ordinary member selection");
    assert_eq!(selection.requirement.declaration.package, "app");
    assert_eq!(selection.requirement.declaration.path, "ProcessExit::exit_process");
    assert!(matches!(selection.target, MemberRealization::Boundary { .. }));
    let [Parameter::Scalar(code)] = stop.body.parameters.as_slice() else {
        panic!("one scalar parameter")
    };
    assert_eq!(arguments, &[CallArgument::Scalar(*code)]);
    assert!(matches!(stop.body.blocks.last().unwrap().terminator, BlockTerminator::Return { .. }));
}

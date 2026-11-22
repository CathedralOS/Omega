#[path = "support/source.rs"]
mod sources;

#[path = "support/returns.rs"]
mod returns;
use returns::sole_return;

use omega_lowering::{
    BlockTerminator, CallArgument, CallRequirement, CallSite, LoweredPackage, LoweringError, OperationKind, OperationResult, Parameter, PlaceOrigin,
    PsiOperation, ReturnEdge, ScalarLiteral, ValueId, ValueOrigin,
};

fn lower(source: &str) -> Result<LoweredPackage, LoweringError> {
    sources::lower(&[("calls.omg", source)])
}

fn scalar_result(operation: &PsiOperation) -> ValueId {
    let OperationResult::Scalar(value) = operation.result else {
        panic!("expected a scalar operation result");
    };
    value
}

#[test]
fn machine_contract_visibility_keeps_nested_helpers_private() {
    let package = lower(
        "public machine outer(x:u8)->u8{
             helper(x)
             public machine helper(x:u8)->u8{x}
         }",
    )
    .unwrap();
    spec_corpus::observe_handoff!(
        psi "psi-output/helper-visibility",
        assert_eq!(
            package
                .machines
                .iter()
                .map(|machine| (machine.declaration.path.as_str(), machine.contract.public))
                .collect::<Vec<_>>(),
            [("outer", true), ("outer::helper", false)]
        ),
    );
}

#[test]
fn direct_calls_keep_the_selected_callee_and_evaluate_each_argument_once_in_order() {
    let declarations = "
        machine after(a:u8, b:u8)->u8{b}
        machine echo(a:u8)->u8{a}
        machine before(a:u8, b:u8)->u8{a}";
    for target in ["after", "before"] {
        let lowered = {
            let source = format!("machine use_it(x:u8)->u8{{{target}(echo(x), echo(7u8))}} {declarations}");
            lower(&source).unwrap()
        };
        let machine = |path| {
            lowered
                .machines
                .iter()
                .find(|machine| machine.declaration.path == path)
                .unwrap()
        };
        let entry = &machine("use_it").body;
        let [Parameter::Scalar(parameter)] = entry.parameters[..] else {
            panic!("expected the scalar argument of use_it");
        };
        let [first, literal, second, combination] = entry.operations.as_slice() else {
            panic!("expected two inner calls, a literal, and the outer call");
        };
        assert_eq!(
            first.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(machine("echo").id),
                arguments: vec![CallArgument::Scalar(parameter)],
                requirements: Vec::new(),
            },
        );
        assert_eq!(literal.kind, OperationKind::Constant(ScalarLiteral::Integer(7u8.into())));
        assert_eq!(
            second.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(machine("echo").id),
                arguments: vec![CallArgument::Scalar(scalar_result(literal))],
                requirements: Vec::new(),
            },
        );
        spec_corpus::observe_handoff!(
            psi "psi-output/call-operands",
            assert_eq!(
                combination.kind,
                OperationKind::Call {
                    callee: omega_lowering::CallTarget::Machine(machine(target).id),
                    arguments: vec![
                        CallArgument::Scalar(scalar_result(first)),
                        CallArgument::Scalar(scalar_result(second))
                    ],
                    requirements: Vec::new(),
                },
            ),
        );
        assert_eq!(sole_return(entry), ReturnEdge::Scalar(scalar_result(combination)));
        let pseudo = machine("use_it").scalar_result.unwrap();
        assert_eq!(pseudo.owner, machine("use_it").id);
        assert_eq!(Some(pseudo.ty), machine("use_it").result_type);
        assert_eq!(entry.values.len(), 1 + entry.operations.len());
        for (ordinal, operation) in entry.operations.iter().enumerate() {
            let result = &entry.values[scalar_result(operation).index()];
            assert_eq!(result.origin, ValueOrigin::Operation(operation.id));
            assert_eq!(Some(result.ty), machine("use_it").result_type);
            assert_eq!(operation.id.get(), ordinal + 1);
        }
    }
}

#[test]
fn mixed_calls_copy_whole_places_and_define_fresh_structural_result_places() {
    let lowered = lower(
        "machine keep(array:[[u8; 2]; 3])->[[u8; 2]; 3]{array}
         machine choose(flag:bool, left:[[u8; 2]; 3], offset:u8, right:[[u8; 2]; 3])->[[u8; 2]; 3]{right}
         machine caller(array:[[u8; 2]; 3], other:[[u8; 2]; 3], offset:u8)->[[u8; 2]; 3]{
             choose(true, keep(array), offset, other)
         }",
    )
    .unwrap();
    let caller = &lowered.machines[0];
    assert_eq!(caller.declaration.path, "caller");
    assert_eq!(caller.scalar_result, None);
    let body = &caller.body;
    let [
        Parameter::Structural(array),
        Parameter::Structural(other),
        Parameter::Scalar(offset),
    ] = body.parameters[..]
    else {
        panic!("expected two array parameters followed by a scalar");
    };
    let [flag, first, second] = body.operations.as_slice() else {
        panic!("expected the flag and two ordered calls");
    };
    let OperationResult::Structural(first_place) = first.result else {
        panic!("expected the first call to define an array place");
    };
    let OperationResult::Structural(second_place) = second.result else {
        panic!("expected the second call to define an array place");
    };
    assert_eq!(
        first.kind,
        OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(lowered.machines[2].id),
            arguments: vec![CallArgument::CopyWhole(array)],
            requirements: Vec::new(),
        }
    );
    assert_eq!(
        second.kind,
        OperationKind::Call {
            callee: omega_lowering::CallTarget::Machine(lowered.machines[1].id),
            arguments: vec![
                CallArgument::Scalar(scalar_result(flag)),
                CallArgument::CopyWhole(first_place),
                CallArgument::Scalar(offset),
                CallArgument::CopyWhole(other),
            ],
            requirements: Vec::new(),
        }
    );
    for (operation, place) in [(first, first_place), (second, second_place)] {
        assert_eq!(body.places[place.index()].origin, PlaceOrigin::Operation(operation.id));
        assert_eq!(Some(body.places[place.index()].ty), caller.result_type);
    }
    let ReturnEdge::Structural { source, result } = sole_return(body) else {
        panic!("expected a structural return");
    };
    assert_eq!(source, second_place);
    assert_eq!(body.places[result.index()].origin, PlaceOrigin::Result);
    assert_ne!(source, result);
}

#[test]
fn unit_calls_keep_their_order_without_inventing_a_value_or_place() {
    let lowered = lower(
        "machine consume(value:bool){}
         machine caller(flag:bool){consume(flag); consume(flag)}",
    )
    .unwrap();
    let caller = &lowered.machines[0];
    assert_eq!(caller.declaration.path, "caller");
    assert_eq!(caller.scalar_result, None);
    let body = &caller.body;
    let [Parameter::Scalar(flag)] = body.parameters[..] else {
        panic!("expected a scalar flag");
    };
    assert_eq!(body.operations.len(), 2);
    for operation in &body.operations {
        assert_eq!(operation.result, OperationResult::Unit);
        assert_eq!(
            operation.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(lowered.machines[1].id),
                arguments: vec![CallArgument::Scalar(flag)],
                requirements: Vec::new(),
            },
        );
    }
    assert_eq!(body.values.len(), 1);
    assert!(body.places.is_empty());
    assert_eq!(sole_return(body), ReturnEdge::Unit);
}

#[test]
fn explicit_discard_evaluates_a_copyable_call_once_and_drops_only_its_result() {
    let lowered = lower("machine leaf(x:u8)->u8{x} machine caller(x:u8){_ = leaf(x);}").unwrap();
    let machine = |path| {
        lowered
            .machines
            .iter()
            .find(|machine| machine.declaration.path == path)
            .unwrap()
    };
    let (caller, leaf) = (machine("caller"), machine("leaf"));
    assert_eq!(caller.scalar_result, None);
    let body = &caller.body;
    let [Parameter::Scalar(x)] = body.parameters[..] else {
        panic!("expected the scalar operand of caller");
    };
    let [call] = body.operations.as_slice() else {
        panic!("expected exactly one evaluation of the discarded call");
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/discard-callee",
        assert_eq!(
            call.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(leaf.id),
                arguments: vec![CallArgument::Scalar(x)],
                requirements: Vec::new(),
            }
        ),
    );
    let dropped = scalar_result(call);
    assert_eq!(body.values[dropped.index()].origin, ValueOrigin::Operation(call.id));
    assert_eq!(Some(body.values[dropped.index()].ty), leaf.result_type);
    assert_eq!(body.values.len(), 2);
    assert!(body.places.is_empty());
    assert_eq!(sole_return(body), ReturnEdge::Unit);
}

#[test]
fn explicit_discard_keeps_statement_order_operand_order_and_call_requirements() {
    let lowered = lower(
        "machine caller(x:u8, flag:bool){note(flag); _ = checked(leaf(x)); note(flag);}
         machine checked(x:u8)->u8 requires x > 0 {x}
         machine leaf(x:u8)->u8{x}
         machine note(flag:bool){}",
    )
    .unwrap();
    let machine = |path| {
        lowered
            .machines
            .iter()
            .find(|machine| machine.declaration.path == path)
            .unwrap()
    };
    let body = &machine("caller").body;
    let [Parameter::Scalar(x), Parameter::Scalar(flag)] = body.parameters[..] else {
        panic!("expected two scalar formals");
    };
    let [before, inner, discarded, after] = body.operations.as_slice() else {
        panic!("expected the note, the discarded operand's two calls once each, and the note");
    };
    for note in [before, after] {
        assert_eq!(note.result, OperationResult::Unit);
        assert_eq!(
            note.kind,
            OperationKind::Call {
                callee: omega_lowering::CallTarget::Machine(machine("note").id),
                arguments: vec![CallArgument::Scalar(flag)],
                requirements: Vec::new(),
            }
        );
    }
    // L5 accepts a precondition record only at the call node L3 marked. The
    // requirement belongs to the outer `checked` call, not the nested `leaf` call.
    spec_corpus::observe_handoff!(
        "call-precondition-obligations/consumer-2",
        assert_eq!(
            (&inner.kind, &discarded.kind),
            (
                &OperationKind::Call {
                    callee: omega_lowering::CallTarget::Machine(machine("leaf").id),
                    arguments: vec![CallArgument::Scalar(x)],
                    requirements: Vec::new(),
                },
                &OperationKind::Call {
                    callee: omega_lowering::CallTarget::Machine(machine("checked").id),
                    arguments: vec![CallArgument::Scalar(scalar_result(inner))],
                    requirements: vec![CallRequirement {
                        site: CallSite::Operation(discarded.id),
                        clause: 0
                    }],
                }
            )
        )
    );
    let dropped = scalar_result(discarded);
    assert_eq!(body.values[dropped.index()].origin, ValueOrigin::Operation(discarded.id));
    assert_eq!(Some(body.values[dropped.index()].ty), machine("checked").result_type);
    assert_eq!(body.values.len(), 2 + 2);
    assert_eq!(sole_return(body), ReturnEdge::Unit);
}

#[test]
fn explicit_discard_publishes_the_same_discard_for_an_affine_and_a_linear_temporary() {
    // Only the declared multiplicity differs. The checker judges the discard
    // against it: affine storage may be discarded, linear storage may not.
    let source = "data Handle {} machine caller(){_ = Handle{};}";
    let linear = source.replace("Handle {}", "Handle [linear] {}");
    for (text, multiplicity) in [
        (source, omega_lowering::Multiplicity::Affine),
        (linear.as_str(), omega_lowering::Multiplicity::Linear),
    ] {
        let package = lower(text).unwrap();
        let body = &package.machines[0].body;
        let [operation] = body.operations.as_slice() else {
            panic!("discarded construction expected")
        };
        let OperationResult::Structural(place) = operation.result else {
            panic!("owned result expected")
        };
        let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
            panic!("disposal must precede the continuation")
        };
        assert_eq!(
            body.diagnostics.edges[edge.id.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(b"Handle{}".as_slice())
        );
        let Some(omega_lowering::TypeShape::Nominal(id)) = package.types.get(body.places[place.index()].ty) else {
            panic!("nominal temporary expected")
        };
        let data = &package.data[id.get() - 1];
        assert_eq!(
            (data.declaration.path.as_str(), data.cleanup_target),
            ("Handle", None)
        );
        if multiplicity == omega_lowering::Multiplicity::Linear {
            spec_corpus::observe_handoff!(
                psi "psi-output/linear-temporary-discard",
                assert_eq!((edge.cleanup.as_slice(), data.multiplicity), ([omega_lowering::CleanupAction::DiscardRoot(place)].as_slice(), multiplicity)),
            );
        } else {
            assert_eq!(
                (edge.cleanup.as_slice(), data.multiplicity),
                ([omega_lowering::CleanupAction::DiscardRoot(place)].as_slice(), multiplicity)
            );
        }
        assert!(matches!(&body.blocks[edge.target.get()-1].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));
    }
}

#[test]
fn ordinary_and_terminal_calls_keep_the_module_of_same_spelled_machines() {
    for (module, expected_index) in [("east", 0), ("west", 1)] {
        for imported in [false, true] {
            let (import, callee) = if imported {
                (format!("use {module}\\pick;"), String::from("pick"))
            } else {
                (String::new(), format!("{module}\\pick"))
            };
            let caller = format!(
                "{import}
                 machine ordinary(a:u8, b:u8)->u8{{{callee}(a, b)}}
                 machine terminal(a:u8, b:u8)->u8{{transition {{_ -> {callee}(a, b),}}}}"
            );
            let package = sources::lower(&[
                ("west/pick.omg", "public machine pick(a:u8, b:u8)->u8{b}"),
                ("main.omg", &caller),
                ("east/pick.omg", "public machine pick(a:u8, b:u8)->u8{a}"),
            ])
            .unwrap();
            let machine = |path: &str| {
                package
                    .machines
                    .iter()
                    .find(|machine| machine.declaration.path == path)
                    .unwrap()
            };
            let selected = machine(&format!("{module}\\pick"));
            let [Parameter::Scalar(first), Parameter::Scalar(second)] = selected.body.parameters[..] else {
                panic!("expected two scalar parameters");
            };
            assert_eq!(sole_return(&selected.body), ReturnEdge::Scalar([first, second][expected_index]));
            let ordinary = &machine("ordinary").body;
            let [Parameter::Scalar(first), Parameter::Scalar(second)] = ordinary.parameters[..] else {
                panic!("expected two scalar parameters");
            };
            let [call] = ordinary.operations.as_slice() else {
                panic!("expected one ordinary call");
            };
            assert_eq!(
                call.kind,
                OperationKind::Call {
                    callee: omega_lowering::CallTarget::Machine(selected.id),
                    arguments: vec![CallArgument::Scalar(first), CallArgument::Scalar(second)],
                    requirements: Vec::new(),
                }
            );
            assert_eq!(sole_return(ordinary), ReturnEdge::Scalar(scalar_result(call)));
            let terminal = &machine("terminal").body;
            let [Parameter::Scalar(first), Parameter::Scalar(second)] = terminal.parameters[..] else {
                panic!("expected two scalar parameters");
            };
            assert!(terminal.operations.is_empty());
            // An unconditional path can forward directly or through a jump.
            let mut forward = &terminal.blocks[0];
            for _ in 0..terminal.blocks.len() {
                let omega_lowering::BlockTerminator::Jump(edge) = &forward.terminator else {
                    break;
                };
                forward = &terminal.blocks[edge.target.get() - 1];
            }
            let omega_lowering::BlockTerminator::TerminalCall {
                callee: omega_lowering::CallTarget::Machine(callee),
                arguments,
                requirements,
                ..
            } = &forward.terminator
            else {
                panic!("expected terminal completion forwarding");
            };
            assert_eq!(*callee, selected.id);
            assert_eq!(*arguments, [CallArgument::Scalar(first), CallArgument::Scalar(second)]);
            assert!(requirements.is_empty());
        }
    }
}

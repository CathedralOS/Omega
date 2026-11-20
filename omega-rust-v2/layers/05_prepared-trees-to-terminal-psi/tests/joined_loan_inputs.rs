#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CompletionContinuation, MachineBody, OperationKind, OperationResult, Parameter, PlaceId,
    ReferenceAccess, ReturnEdge,
};
use omega_resolved::{NameRole, Symbol};

const INSPECT: &str = "machine inspect(bytes:@[u8])->u64 suspends { bytes.len }";
const HEADER: &str = "machine forward(outer:bool,inner:bool,left:@[u8],right:@[u8],dead:@[u8])->u64 suspends";

#[test]
fn swapping_same_typed_match_arms_swaps_the_retained_backing() {
    for (positive, negative, expected) in [("left", "right", [2, 3]), ("right", "left", [3, 2])] {
        let text = format!("{INSPECT} {HEADER} {{ let selected = match outer {{ true -> {positive}, false -> {negative}, }}; suspend inspect(selected) }}");
        let package = prepare_and_lower(&text, &[positive, negative]);
        assert_call_backing(&package, &[(true, false, expected[0]), (false, false, expected[1])], false);
    }
}

#[test]
fn nested_joins_forward_the_same_backing_through_ordinary_and_terminal_calls() {
    for terminal in [false, true] {
        let completion = if terminal {
            "transition { _ -> suspend inspect(selected), }"
        } else {
            "let count:u64 = suspend inspect(selected); count"
        };
        let text = format!(
            "{INSPECT} {HEADER} {{
                let selected = match outer {{ true -> match inner {{ true -> right, false -> left, }}, false -> left, }};
                {completion}
            }}"
        );
        let package = prepare_and_lower(&text, &["right", "left", "left"]);
        assert_call_backing(
            &package,
            &[
                (true, true, 3),
                (true, false, 2),
                (false, true, 2),
                (false, false, 2),
            ],
            terminal,
        );

        for (direct, formal) in [("left", 2), ("right", 3)] {
            let text = format!("{INSPECT} {HEADER} {{ {} }}", completion.replace("selected", direct));
            let package = source::lower(&[("direct_loan_inputs.omg", &text)]).unwrap();
            assert_call_backing(&package, &[(true, true, formal)], terminal);
        }
    }
}

fn prepare_and_lower(
    text: &str,
    expected_arms: &[&str],
) -> omega_lowering::LoweredPackage {
    let resolved = source::resolve(&[("joined_loan_inputs.omg", text)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let call = typed
        .calls
        .iter()
        .find(|call| call.machine.path == "inspect")
        .unwrap();
    let [actual] = call.operands.as_slice() else {
        panic!("expected one typed actual")
    };
    assert!(call.acknowledgements.suspend);
    assert_eq!(actual.site.span.slice(text.as_bytes()), Some(b"selected".as_slice()));

    let typed_tree = typed.trees().next().unwrap();
    let tree = typed_tree.resolved();
    let Some(Symbol::Binding { span, .. }) = tree.symbol(actual.node.node) else {
        panic!("call actual should resolve to its binding")
    };
    assert_eq!(span.get_start_offset(), text.find("selected =").unwrap());

    let mut arms = Vec::new();
    for occurrence in tree.occurrences() {
        if occurrence.role != NameRole::Reference || occurrence.node == actual.node {
            continue;
        }
        let Some(name @ (b"left" | b"right")) = occurrence.span.slice(text.as_bytes()) else {
            continue;
        };
        let Symbol::Binding { span, .. } = &occurrence.target else {
            panic!("branch result should resolve to an incoming formal")
        };
        let name = std::str::from_utf8(name).unwrap();
        assert_eq!(span.get_start_offset(), text.find(&format!("{name}:@[u8]")).unwrap());
        arms.push(name);
    }
    assert_eq!(arms, expected_arms);

    let package = omega_lowering::lower_package(&prepared).unwrap();
    for machine in &package.machines {
        source::control_flow::check_operation_ownership(&machine.body).unwrap();
    }
    package
}

fn assert_call_backing(
    package: &omega_lowering::LoweredPackage,
    scenarios: &[(bool, bool, usize)],
    terminal: bool,
) {
    let caller = package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "forward")
        .unwrap();
    let callee = package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "inspect")
        .unwrap();
    let body = &caller.body;
    let [site] = body.suspension_sites.as_slice() else {
        panic!("expected one crossing")
    };
    assert_eq!(site.callee, CallTarget::Machine(callee.id));
    assert!(site.activation.is_empty());
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one input descriptor should remain live")
    };
    assert_eq!(loan.ty, callee.parameter_types[0]);
    assert_eq!(loan.access, ReferenceAccess::Shared);
    assert!(loan.needed_by_callee);
    assert!(!loan.needed_after_completion);

    for &(outer, inner, formal) in scenarios {
        let (call, target, actual, backing, continuation) = follow_call(body, [outer, inner]);
        assert_eq!(
            (call, target, actual, continuation),
            (site.call, site.callee, loan.descriptor, site.continuation)
        );
        assert_eq!(matches!(call, CallSite::Edge(_)), terminal);
        assert_eq!(body.parameters[formal], Parameter::Structural(backing));
    }
}

/// Replay only the Boolean dispatch and borrowed-view forwarding used by these programs.
fn follow_call(
    body: &MachineBody,
    choices: [bool; 2],
) -> (CallSite, CallTarget, PlaceId, PlaceId, CompletionContinuation) {
    let [Parameter::Scalar(outer), Parameter::Scalar(inner), formals @ ..] = body.parameters.as_slice() else {
        panic!("expected two Boolean selectors")
    };
    let mut backing = vec![None; body.places.len()];
    for &formal in formals {
        let Parameter::Structural(place) = formal else {
            panic!("expected a view formal")
        };
        backing[place.index()] = Some(place);
    }
    let mut block = &body.blocks[0];
    for _ in 0..body.blocks.len() {
        let operations = &body.operations[block.operations.clone()];
        for operation in operations {
            if let OperationKind::Call { callee, arguments, .. } = &operation.kind {
                let [CallArgument::CopyWhole(actual)] = arguments.as_slice() else {
                    panic!("expected a copied descriptor")
                };
                assert_eq!(operations.last().unwrap().id, operation.id);
                assert!(matches!(
                    (&block.terminator, operation.result),
                    (BlockTerminator::Return { result: ReturnEdge::Scalar(returned), cleanup, .. }, OperationResult::Scalar(result))
                        if *returned == result && cleanup.is_empty()
                ));
                return (
                    CallSite::Operation(operation.id),
                    *callee,
                    *actual,
                    backing[actual.index()].unwrap(),
                    CompletionContinuation::Terminator(block.id),
                );
            }
        }
        let edge = match &block.terminator {
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let selected = if condition == outer {
                    choices[0]
                } else {
                    assert_eq!(condition, inner);
                    choices[1]
                };
                if selected { when_true } else { when_false }
            }
            BlockTerminator::Jump(edge) => edge,
            BlockTerminator::TerminalCall {
                id,
                callee,
                arguments,
                cleanup,
                ..
            } => {
                let [CallArgument::CopyWhole(actual)] = arguments.as_slice() else {
                    panic!("expected a copied descriptor")
                };
                assert!(cleanup.is_empty());
                return (
                    CallSite::Edge(*id),
                    *callee,
                    *actual,
                    backing[actual.index()].unwrap(),
                    CompletionContinuation::Return,
                );
            }
            other => panic!("expected a path to the selected call, got {other:?}"),
        };
        let target = &body.blocks[edge.target.get() - 1];
        assert_eq!(target.structural_parameters.len(), edge.structural_arguments.len());
        for (&parameter, operand) in target
            .structural_parameters
            .iter()
            .zip(&edge.structural_arguments)
        {
            let CallArgument::CopyWhole(input) = operand else {
                panic!("a view join must forward the borrowed descriptor")
            };
            backing[parameter.index()] = Some(backing[input.index()].expect("the incoming view is established"));
        }
        block = target;
    }
    panic!("expected an acyclic path to a call")
}

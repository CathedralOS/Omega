#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CleanupAction, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceId,
    PlaceOrigin, PlaceProjection, ProjectedPlace, ReturnEdge, StructuralStoreDisposition,
};
use psi_handoff_consumer::sum_cleanup::{NormalStep, normal_steps, original_place};

const TYPES: &str = "
data Token { v:u8; }
data Lease dropped by release { mark:u8; }
machine release(x:Lease) { let Lease{mark as _}=x; }
data Choice { case A(left:Token,right:Lease); case B(left:Token,right:Token); }
";

fn lower(source: &str) -> LoweredPackage {
    let program = format!("{TYPES}{source}");
    sources::lower(&[("case.omg", &program)]).unwrap()
}

fn machine<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == name)
        .unwrap()
}

fn structural_parameter(
    machine: &LoweredMachine,
    position: usize,
) -> PlaceId {
    let Parameter::Structural(place) = machine.body.parameters[position] else {
        panic!("expected a structural parameter")
    };
    place
}

fn cleanup(steps: &[NormalStep<'_>]) -> Vec<CleanupAction> {
    steps
        .iter()
        .filter_map(|step| match step {
            NormalStep::Cleanup(action) => Some((*action).clone()),
            _ => None,
        })
        .collect()
}

fn path_names(path: &[PlaceProjection]) -> Vec<String> {
    path.iter()
        .map(|projection| match projection {
            PlaceProjection::Case(declaration) | PlaceProjection::Field(declaration) => declaration.path.clone(),
            PlaceProjection::FixedIndex(index) => index.to_string(),
            unexpected => panic!("unexpected cleanup projection {unexpected:?}"),
        })
        .collect()
}

fn invocation_paths(
    steps: &[NormalStep<'_>],
    root: PlaceId,
    target: omega_lowering::MachineId,
) -> Vec<Vec<String>> {
    steps
        .iter()
        .filter_map(|step| match step {
            NormalStep::Cleanup(CleanupAction::Invoke { place, target: selected, .. }) => {
                assert_eq!((place.root, *selected), (root, target));
                Some(path_names(&place.path))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn cleanup_dispatch_descends_through_records_arrays_and_active_payloads() {
    let program = lower(
        "
        data Wrap { before:Token; inner:[Choice;2]; after:Lease; }
        data Outer { case Present(inner:Wrap); case Absent; }
        machine discard(x:Outer) {}
    ",
    );
    let discard = machine(&program, "discard");
    let root = structural_parameter(discard, 0);
    let release = machine(&program, "release").id;
    let tags = |place: &ProjectedPlace| match path_names(&place.path).last().map(String::as_str) {
        None => Some("Outer::Present"),
        Some("0") => Some("Choice::A"),
        Some("1") => Some("Choice::B"),
        _ => None,
    };
    let steps = normal_steps(discard, tags).unwrap();
    sources::control_flow::check_operation_ownership(&discard.body).unwrap();
    let paths = cleanup(&steps)
        .into_iter()
        .map(|action| {
            let place = match action {
                CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } => place,
                CleanupAction::RetireHome(place) => ProjectedPlace { root: place, path: vec![] },
                unexpected => panic!("unexpected cleanup action {unexpected:?}"),
            };
            let original = original_place(discard, &place);
            assert_eq!(original.root, root);
            path_names(&original.path)
        })
        .collect::<Vec<_>>();
    let expected = [
        vec!["Outer::Present", "Outer::Present::inner", "Wrap::after"],
        vec![
            "Outer::Present",
            "Outer::Present::inner",
            "Wrap::inner",
            "1",
            "Choice::B",
            "Choice::B::right",
        ],
        vec![
            "Outer::Present",
            "Outer::Present::inner",
            "Wrap::inner",
            "1",
            "Choice::B",
            "Choice::B::left",
        ],
        vec!["Outer::Present", "Outer::Present::inner", "Wrap::inner", "1"],
        vec![
            "Outer::Present",
            "Outer::Present::inner",
            "Wrap::inner",
            "0",
            "Choice::A",
            "Choice::A::right",
        ],
        vec![
            "Outer::Present",
            "Outer::Present::inner",
            "Wrap::inner",
            "0",
            "Choice::A",
            "Choice::A::left",
        ],
        vec!["Outer::Present", "Outer::Present::inner", "Wrap::inner", "0"],
        vec!["Outer::Present", "Outer::Present::inner", "Wrap::before"],
        vec![],
    ];
    spec_corpus::observe_handoff!(psi "psi-output/nested-sum-cleanup-order", assert_eq!(paths, expected));
    let invoked = |steps: &[NormalStep<'_>]| -> Vec<Vec<String>> {
        steps
            .iter()
            .filter_map(|step| match step {
                NormalStep::Cleanup(CleanupAction::Invoke { place, target, .. }) => {
                    let original = original_place(discard, place);
                    assert_eq!((original.root, *target), (root, release));
                    Some(path_names(&original.path))
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(invoked(&steps), [expected[0].clone(), expected[4].clone()]);
    let steps = normal_steps(discard, |place| place.path.is_empty().then_some("Outer::Absent")).unwrap();
    assert_eq!(cleanup(&steps), [CleanupAction::RetireHome(root)]);
    let twin = normal_steps(discard, |place| match path_names(&place.path).last().map(String::as_str) {
        None => Some("Outer::Present"),
        Some("0") => Some("Choice::B"),
        Some("1") => Some("Choice::A"),
        _ => None,
    })
    .unwrap();
    assert_eq!(invoked(&twin)[1][3], "1");
}

#[test]
fn terminal_call_keeps_its_identity_and_returns_its_result_after_selected_cleanup() {
    let program = lower(
        "
        machine done(v:u8)->u8 suspends { v }
        machine discard(x:Choice,v:u8)->u8 suspends { transition { _ -> suspend done(v), } }
    ",
    );
    let discard = machine(&program, "discard");
    let root = structural_parameter(discard, 0);
    let Parameter::Scalar(actual) = discard.body.parameters[1] else {
        panic!("expected scalar argument")
    };
    let done = machine(&program, "done").id;
    let (call, entry) = discard
        .body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall {
                id,
                callee: CallTarget::Machine(target),
                arguments,
                cleanup,
                completion: Some(entry),
                ..
            } if *target == done && cleanup.is_empty() && arguments == &[CallArgument::Scalar(actual)] => Some((*id, *entry)),
            _ => None,
        })
        .expect("original terminal call with the completion path");
    let [result] = discard.body.blocks[entry.get() - 1]
        .scalar_parameters
        .as_slice()
    else {
        panic!("call result binds its completion entrance")
    };
    let a = normal_steps(discard, |_| Some("Choice::A")).unwrap();
    let b = normal_steps(discard, |_| Some("Choice::B")).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/terminal-sum-cleanup-completion", assert!(
        matches!(a.first(), Some(NormalStep::Terminal { edge, .. }) if *edge == call)
            && matches!(a.last(), Some(NormalStep::Return(ReturnEdge::Scalar(returned))) if returned == result)
            && invocation_paths(&a, root, machine(&program, "release").id) == [vec!["Choice::A".to_string(), "Choice::A::right".to_string()]]
            && invocation_paths(&b, root, machine(&program, "release").id).is_empty()
    ));
    assert_eq!(discard.body.call_acknowledgements[0].call, CallSite::Edge(call));
    sources::control_flow::check_operation_ownership(&discard.body).unwrap();

    let plain = lower("machine done(v:u8)->u8{v} machine discard(x:Token,v:u8)->u8{transition{_ -> done(v),}}");
    assert!(
        machine(&plain, "discard")
            .body
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, BlockTerminator::TerminalCall { completion: None, .. }))
    );
}

#[test]
fn replacement_evaluates_selector_and_rhs_once_before_destroying_only_the_old_payload() {
    let program = lower(
        "
        machine index(i:u64)->u64 { i }
        machine carry(y:Choice)->Choice { move y }
        machine replace(x:@mutable [Choice],i:u64,y:Choice) { x[index(i)]=carry(move y); }
    ",
    );
    let replace = machine(&program, "replace");
    let steps = normal_steps(replace, |_| Some("Choice::A")).unwrap();
    let [index, length, rhs, take, test, install] = steps
        .iter()
        .filter_map(|step| match step {
            NormalStep::Operation(operation) if !matches!(operation.kind, OperationKind::MoveStructural { .. }) => Some(*operation),
            _ => None,
        })
        .collect::<Vec<_>>()[..]
    else {
        panic!("one selector, length, rhs, displacement, test, and install: {steps:?}")
    };
    assert!(matches!(index.kind, OperationKind::Call { callee: CallTarget::Machine(target), .. } if target == machine(&program, "index").id));
    assert!(matches!(rhs.kind, OperationKind::Call { callee: CallTarget::Machine(target), .. } if target == machine(&program, "carry").id));
    let OperationKind::BorrowedMove { source: captured } = &take.kind else {
        panic!("displacement uses a captured borrowed place")
    };
    let OperationResult::Structural(old) = take.result else {
        panic!("displacement produces the old sum")
    };
    let OperationResult::Scalar(selector) = index.result else {
        panic!("selector has an SSA result")
    };
    assert!(matches!(length.kind, OperationKind::ElementViewLength { source } if source == structural_parameter(replace, 0)));
    assert!(matches!(captured.path.last(), Some(PlaceProjection::ViewElement { index, length: measured })
        if index.selector == selector && length.result == OperationResult::Scalar(*measured)));
    let OperationKind::StructuralCaseMembership { source, .. } = &test.kind else {
        panic!("inspect the displaced tag")
    };
    assert_eq!((source.root, source.path.as_slice()), (old, &[][..]));
    let OperationKind::StructuralStore {
        destination,
        value,
        disposition,
    } = &install.kind
    else {
        panic!("install the replacement")
    };
    let install_position = steps
        .iter()
        .position(|step| matches!(step, NormalStep::Operation(op) if op.id == install.id))
        .unwrap();
    let release = machine(&program, "release").id;
    spec_corpus::observe_handoff!(psi "psi-output/replacement-sum-cleanup", assert!(
        destination == captured && rhs.result == OperationResult::Structural(*value) && *disposition == StructuralStoreDisposition::FillVacancy
        && invocation_paths(&steps[..install_position], old, release) == [vec!["Choice::A".to_string(), "Choice::A::right".to_string()]]
        && cleanup(&steps[install_position..]).is_empty()
    ));
    let twin = normal_steps(replace, |_| Some("Choice::B")).unwrap();
    assert!(invocation_paths(&twin, old, release).is_empty());
    assert!(matches!(replace.body.places[old.index()].origin, PlaceOrigin::Operation(id) if id == take.id));
    sources::control_flow::check_operation_ownership(&replace.body).unwrap();
}

#[test]
fn owned_replacement_reopens_the_same_field_and_keeps_the_containing_home() {
    let program = lower(
        "
        data Wrap { before:Token; inner:Choice; }
        machine carry(y:Choice)->Choice { move y }
        machine replace(x:Wrap,y:Choice)->Wrap { let mutable home=move x; home.inner=carry(move y); move home }
    ",
    );
    let replace = machine(&program, "replace");
    let steps = normal_steps(replace, |_| Some("Choice::A")).unwrap();
    let install = steps
        .iter()
        .position(|step| matches!(step, NormalStep::Operation(operation) if matches!(operation.kind, OperationKind::StructuralStore { .. })))
        .unwrap();
    let NormalStep::Operation(store) = &steps[install] else { unreachable!() };
    let OperationKind::StructuralStore {
        destination,
        value,
        disposition: StructuralStoreDisposition::FillVacancy,
    } = &store.kind
    else {
        panic!("expected an exact vacancy fill")
    };
    let old = steps[..install]
        .iter()
        .find_map(|step| match step {
            NormalStep::Operation(operation) if matches!(&operation.kind, OperationKind::MoveStructural { source } if source == destination) => {
                let OperationResult::Structural(old) = operation.result else {
                    panic!("expected a captured old value")
                };
                Some(old)
            }
            _ => None,
        })
        .expect("the destination is moved into a cleanup temporary exactly once");
    assert!(steps[..install].iter().any(
        |step| matches!(step, NormalStep::Operation(op) if matches!(op.kind, OperationKind::Call { .. }) && op.result == OperationResult::Structural(*value))
    ));
    assert_eq!(path_names(&destination.path), ["Wrap::inner"]);
    assert_eq!(
        invocation_paths(&steps[..install], old, machine(&program, "release").id),
        [vec!["Choice::A", "Choice::A::right"]]
    );
    assert_eq!(cleanup(&steps[install..]), []);
    assert!(steps[install..].iter().any(|step| matches!(step, NormalStep::Operation(op) if matches!(&op.kind, OperationKind::MoveStructural { source } if source.root == destination.root && source.path.is_empty()))));
    sources::control_flow::check_operation_ownership(&replace.body).unwrap();
}
#[test]
fn partially_moved_replacement_disposes_the_live_complement_before_reusing_the_home() {
    let program = lower(
        "
        data Wrap { taken:Token; live:Choice; tail:Lease; }
        machine replace(x:Wrap,y:Wrap)->Wrap {
            let mutable home=move x;
            let removed=move home.taken;
            home=move y;
            move home
        }
    ",
    );
    let replace = machine(&program, "replace");
    let root = structural_parameter(replace, 0);
    let steps = normal_steps(replace, |_| Some("Choice::A")).unwrap();
    let (store_at, destination) = steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            NormalStep::Operation(operation) => match &operation.kind {
                OperationKind::StructuralStore {
                    destination,
                    disposition: StructuralStoreDisposition::Dispose(actions),
                    ..
                } if actions.is_empty() => Some((index, destination)),
                _ => None,
            },
            _ => None,
        })
        .expect("reuse the partially vacant home after its live complement disposes");
    assert!(destination.path.is_empty());
    assert_eq!(original_place(replace, destination).root, root);
    let invoked = steps[..store_at]
        .iter()
        .filter_map(|step| match step {
            NormalStep::Cleanup(CleanupAction::Invoke { place, target, .. }) => {
                assert_eq!(*target, machine(&program, "release").id);
                let original = original_place(replace, place);
                assert_eq!(original.root, root);
                Some(path_names(&original.path))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        invoked,
        [
            vec!["Wrap::tail"],
            vec!["Wrap::live", "Choice::A", "Choice::A::right"]
        ]
    );
    assert!(!steps[..store_at].iter().any(|step| {
        match step {
            NormalStep::Cleanup(CleanupAction::RetireHome(place)) => *place == destination.root,
            NormalStep::Cleanup(CleanupAction::DiscardResidual { place, .. }) => path_names(&original_place(replace, place).path)
                .first()
                .is_some_and(|path| path == "Wrap::taken"),
            _ => false,
        }
    }));
    assert!(
        steps[store_at..]
            .iter()
            .any(|step| matches!(step, NormalStep::Operation(op) if matches!(&op.kind, OperationKind::MoveStructural { source } if source == destination)))
    );
    sources::control_flow::check_operation_ownership(&replace.body).unwrap();
}

#[test]
fn an_unrepresentably_large_dispatched_array_refuses_before_expansion() {
    let program = format!("{TYPES}machine discard(x:[Choice;9223372036854775807]){{}}");
    let error = sources::lower(&[("case.omg", &program)]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("insufficient storage for the fixed-array residual schedule")
    );
}
#[test]
fn an_owned_empty_array_retires_its_home_without_calling_an_element_cleanup() {
    let program = lower("machine discard(x:[Lease;0]){}");
    let discard = machine(&program, "discard");
    let root = structural_parameter(discard, 0);
    let trace = normal_steps(discard, |_| None).unwrap();
    assert_eq!(
        trace,
        [
            NormalStep::Cleanup(&CleanupAction::DiscardRoot(root)),
            NormalStep::Return(ReturnEdge::Unit),
        ]
    );
}

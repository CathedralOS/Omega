#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    BlockId, BlockTerminator, CallArgument, CleanupAction, LoweredMachine, LoweredPackage, OperationKind, OperationResult, Parameter, PlaceId, PlaceProjection,
    ProjectedPlace, ReferenceAccess, ReturnEdge, SuccessorEdge,
};
use std::collections::BTreeSet;

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

fn subject(machine: &LoweredMachine) -> PlaceId {
    let Parameter::Structural(place) = machine.body.parameters[0] else {
        panic!("structural case subject")
    };
    place
}

fn case_field(
    place: &ProjectedPlace,
    root: PlaceId,
    case: &str,
    field: &str,
    borrowed: bool,
) -> bool {
    if place.root != root {
        return false;
    }
    let path = if borrowed {
        let [PlaceProjection::Referent, rest @ ..] = place.path.as_slice() else {
            return false;
        };
        rest
    } else {
        &place.path
    };
    matches!(path, [PlaceProjection::Case(selected), PlaceProjection::Field(member)]
        if selected.path == case && member.path == field)
}

fn case_arms(machine: &LoweredMachine) -> [&SuccessorEdge; 2] {
    let BlockTerminator::Conditional { when_true, when_false, .. } = &machine.body.blocks[0].terminator else {
        panic!("one case test with two selected successors")
    };
    assert!(when_true.selected_case.is_some() && when_false.selected_case.is_some());
    [when_true, when_false]
}

const LINEAR: &str = "data Token [linear] { value:u64; } data Outcome { case Success(token:Token); case Failure(token:Token); }";

#[test]
fn each_capture_moves_the_exact_selected_field_and_returns_only_its_custody() {
    let src = format!(
        "{LINEAR} machine recover(x:Outcome)->Token {{transition x {{
        Outcome::Success{{token as delivered}} -> delivered,
        Outcome::Failure{{token}} -> token,
    }}}}"
    );
    let package = sources::lower(&[("case.omg", &src)]).unwrap();
    let recover = machine(&package, "recover");
    let root = subject(recover);
    for (edge, (case, field)) in case_arms(recover).into_iter().zip([
        ("Outcome::Success", "Outcome::Success::token"),
        ("Outcome::Failure", "Outcome::Failure::token"),
    ]) {
        let selected = edge.selected_case.as_ref().unwrap();
        assert_eq!((&selected.source, selected.case.path.as_str()), (&ProjectedPlace { root, path: vec![] }, case));
        let arm = &recover.body.blocks[edge.target.get() - 1];
        let [capture, returned] = &recover.body.operations[arm.operations.clone()] else {
            panic!("capture once, then move its binding to the return")
        };
        let OperationKind::MoveStructural { source } = &capture.kind else {
            panic!("the owning payload transfers")
        };
        let OperationResult::Structural(binding) = capture.result else {
            panic!("fresh capture place")
        };
        spec_corpus::observe_handoff!(psi "psi-output/owned-case-payload-binding", assert!(
            case_field(source, root, case, field, false)
            && matches!(&returned.kind, OperationKind::MoveStructural { source } if source.root == binding && source.path.is_empty())
            && matches!(&arm.terminator, BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, cleanup, .. }
                if returned.result == OperationResult::Structural(*source) && cleanup.is_empty())
        ));
    }
}

#[test]
fn a_borrowed_payload_keeps_the_case_path_and_never_extracts_its_linear_value() {
    for (mode, access) in [
        ("", ReferenceAccess::Shared),
        ("mutable ", ReferenceAccess::Mutable),
    ] {
        let src = format!(
            "{LINEAR} machine see(t:@{mode}Token)->u64{{t.value}} machine inspect(x:@{mode}Outcome)->u64{{transition x {{
            Outcome::Success{{token}} -> see(@{mode}token),
            Outcome::Failure{{token as found}} -> see(@{mode}found),
        }}}}"
        );
        let package = sources::lower(&[("case.omg", &src)]).unwrap();
        let inspect = machine(&package, "inspect");
        let root = subject(inspect);
        for (edge, (case, field)) in case_arms(inspect).into_iter().zip([
            ("Outcome::Success", "Outcome::Success::token"),
            ("Outcome::Failure", "Outcome::Failure::token"),
        ]) {
            let arm = &inspect.body.blocks[edge.target.get() - 1];
            spec_corpus::observe_handoff!(psi "psi-output/borrowed-case-payload-binding", assert!(
                arm.operations.is_empty()
                && matches!(&arm.terminator, BlockTerminator::TerminalCall { arguments, cleanup, .. }
                    if cleanup.is_empty()
                    && matches!(arguments.as_slice(), [CallArgument::Borrow { place, access: actual }]
                        if *actual == access && case_field(place, root, case, field, true)))
            ));
        }
    }

    let copy = "data Token [copy]{value:u64;} data Outcome{case Success(token:Token);case Failure(token:Token);}
        machine inspect(x:@Outcome)->Token{match x{Outcome::Success{token}->token,Outcome::Failure{token}->token}}";
    let package = sources::lower(&[("case.omg", copy)]).unwrap();
    let inspect = machine(&package, "inspect");
    for (edge, (case, field)) in case_arms(inspect).into_iter().zip([
        ("Outcome::Success", "Outcome::Success::token"),
        ("Outcome::Failure", "Outcome::Failure::token"),
    ]) {
        let ops = &inspect.body.operations[inspect.body.blocks[edge.target.get() - 1].operations.clone()];
        assert!(matches!(ops, [copy] if matches!(&copy.kind, OperationKind::CopyStructural { source }
            if case_field(source, subject(inspect), case, field, true))));
    }
}

fn outgoing(
    machine: &LoweredMachine,
    start: BlockId,
) -> Vec<&SuccessorEdge> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![start];
    let mut found = Vec::new();
    while let Some(block) = pending.pop() {
        if !visited.insert(block.get()) {
            continue;
        }
        let edges: Vec<_> = match &machine.body.blocks[block.get() - 1].terminator {
            BlockTerminator::Jump(edge) => vec![edge],
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true, when_false],
            _ => vec![],
        };
        pending.extend(edges.iter().map(|edge| edge.target));
        found.extend(edges);
    }
    found
}

#[test]
fn a_captured_owner_survives_nested_dispatch_and_disposes_on_its_own_join_edge() {
    let source = "
        data Token dropped by release { value:u64; }
        machine release(x:Token){let Token{value as _}=x;}
        data Outcome{case Success(token:Token);case Failure(token:Token);}
        machine read(x:Outcome, choose:bool)->u64{
            let selected=match x{
                Outcome::Success{token}->match choose{true->token.value,false->2u64},
                Outcome::Failure{token}->token.value,
            };
            selected
        }";
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let read = machine(&package, "read");
    let root = subject(read);
    let release = machine(&package, "release").id;
    let [left, right] = case_arms(read);
    let mut joins = Vec::new();
    for arm in [left, right] {
        let capture = &read.body.operations[read.body.blocks[arm.target.get() - 1].operations.start];
        let OperationResult::Structural(binding) = capture.result else {
            panic!("structural capture")
        };
        let continuations = outgoing(read, arm.target);
        let [exit] = continuations
            .into_iter()
            .filter(|edge| !edge.cleanup.is_empty())
            .collect::<Vec<_>>()[..]
        else {
            panic!("exactly one cleanup continuation per arm")
        };
        spec_corpus::observe_handoff!(psi "psi-output/case-payload-arm-cleanup", assert!(
            matches!(exit.cleanup.as_slice(), [CleanupAction::Invoke { place, target, .. }, CleanupAction::RetireHome(home)]
                if place.root == binding && place.path.is_empty() && *target == release && *home == root)
        ));
        joins.push(exit.target);
    }
    assert_eq!(joins[0], joins[1]);
    assert!(matches!(&read.body.blocks[joins[0].get()-1].terminator, BlockTerminator::Return { cleanup, .. } if cleanup.is_empty()));
}

#[test]
fn a_return_edge_disposes_only_the_selected_cases_unbound_sibling() {
    let source = "
        data Token{value:u64;} data Spare dropped by release { value:u64; }
        machine release(x:Spare){let Spare{value as _}=x;}
        data Outcome{case Success(token:Token,spare:Spare);case Failure(token:Token);}
        machine recover(x:Outcome)->Token{transition x{
            Outcome::Success{token,spare as _}->token,
            Outcome::Failure{token}->token,
        }}";
    let package = sources::lower(&[("case.omg", source)]).unwrap();
    let recover = machine(&package, "recover");
    let root = subject(recover);
    let [a, b] = case_arms(recover).map(|edge| &recover.body.blocks[edge.target.get() - 1]);
    assert!(matches!(&a.terminator, BlockTerminator::Return { cleanup, .. }
        if matches!(cleanup.as_slice(), [CleanupAction::Invoke {place,target,..}]
            if *target == machine(&package,"release").id && case_field(place, root, "Outcome::Success", "Outcome::Success::spare", false))));
    assert!(matches!(&b.terminator, BlockTerminator::Return { cleanup, .. } if cleanup.is_empty()));
}

#[test]
fn a_second_use_does_not_recapture_the_moved_payload() {
    let prefix = format!("{LINEAR} machine use_two(a:Token,b:Token)->Token{{a}}");
    let make = |arguments| {
        format!(
            "{prefix} machine recover(x:Outcome,other:Token)->Token{{transition x{{
        Outcome::Success{{token}}->use_two({arguments}),
        Outcome::Failure{{token}}->use_two(token,other),
    }}}}"
        )
    };
    sources::lower(&[("case.omg", &make("token,other"))]).unwrap();
    let bad = make("token,token");
    let error = sources::lower(&[("case.omg", &bad)]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("owned transfer has no live source root")
    );
    let start = bad.find("token,token").unwrap() + "token,".len();
    assert_eq!(error.site.span.get_start_offset(), start);
}

#[test]
fn unequal_surviving_case_regions_do_not_inherit_the_first_arms_cleanup() {
    let make = |tail| {
        format!(
            "
        data Token{{value:u64;}} data Outcome{{case Success(token:Token,spare:Token);case Failure(token:Token);}}
        machine see(x:@Outcome)->u64{{0u64}}
        machine read(x:Outcome)->u64{{
          let selected=match x{{Outcome::Success{{token,spare as _}}->token.value,Outcome::Failure{{token}}->token.value}};
          {tail}
        }}"
        )
    };
    for tail in ["selected", "see(@x)"] {
        let error = sources::lower(&[("case.omg", &make(tail))]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("case payload join has different live owned subject regions")
        );
    }
}
#[test]
fn stored_shared_payloads_return_the_selected_descriptor_at_its_declared_region() {
    for (formal, borrowed) in [("Outcome<'a>", false), ("@Outcome<'a>", true)] {
        let source = format!(
            "
            data Token{{value:u64;}}
            data Outcome<'a>{{case Success(token:@'a Token);case Failure(token:@'a Token);}}
            machine pick<'a>(x:{formal})->@'a Token{{transition x{{
                Outcome::Success{{token}}->token,
                Outcome::Failure{{token}}->token,
            }}}}"
        );
        let package = sources::lower(&[("case.omg", &source)]).unwrap();
        let pick = machine(&package, "pick");
        let root = subject(pick);
        for (edge, (case, field)) in case_arms(pick).into_iter().zip([
            ("Outcome::Success", "Outcome::Success::token"),
            ("Outcome::Failure", "Outcome::Failure::token"),
        ]) {
            let arm = &pick.body.blocks[edge.target.get() - 1];
            let [copy] = &pick.body.operations[arm.operations.clone()] else {
                panic!("one selected descriptor copy")
            };
            assert!(matches!(&copy.kind, OperationKind::CopyStructural { source } if case_field(source, root, case, field, borrowed)));
            assert!(
                matches!(&arm.terminator, BlockTerminator::Return {result: ReturnEdge::Structural { source, .. }, .. }
                if copy.result == OperationResult::Structural(*source))
            );
        }
    }
}

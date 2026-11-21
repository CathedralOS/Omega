#[path = "support/source.rs"]
mod sources;

use omega_lowering::{BlockTerminator, CleanupAction, Parameter, PlaceProjection, TypeShape};
use psi_handoff_consumer::sum_cleanup::return_cleanup_for_case;

const CASE: &str = include_str!("fixtures/package_cases/intact-sum-with-case-dependent-cleanup/case/case.omg");
const CONTROL: &str = include_str!("fixtures/package_cases/intact-sum-with-case-dependent-cleanup/control/case.omg");

#[test]
fn the_dying_sums_tag_selects_reverse_payload_cleanup_and_home_retirement() {
    let mut pkg = sources::lower(&[("case.omg", CASE)]).unwrap();
    let target = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "release")
        .unwrap()
        .id;
    let m = pkg
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "discard")
        .unwrap();
    sources::control_flow::check_operation_ownership(&m.body).unwrap();
    let [Parameter::Structural(root)] = m.body.parameters[..] else {
        panic!("one dying sum")
    };
    let TypeShape::Nominal(id) = pkg.types.get(m.body.places[root.index()].ty).unwrap() else {
        panic!("expected Choice")
    };
    let declared = &pkg.data[id.get() - 1];
    let expected = |ordinal: usize| {
        let case = &declared.cases[ordinal];
        let mut actions = Vec::new();
        for field in case.fields.iter().rev() {
            let place = omega_lowering::ProjectedPlace {
                root,
                path: vec![
                    PlaceProjection::Case(case.declaration.clone()),
                    PlaceProjection::Field(field.declaration.clone()),
                ],
            };
            let ty = field.carrier;
            actions.push(if field.declaration.path == "Choice::A::right" {
                CleanupAction::Invoke { place, ty, target }
            } else {
                CleanupAction::DiscardResidual { place, ty }
            });
        }
        actions.push(CleanupAction::RetireHome(root));
        actions
    };
    let a = expected(0);
    let b = expected(1);
    spec_corpus::observe_handoff!(psi "psi-output/intact-sum-selected-return-cleanup", assert_eq!(
        (return_cleanup_for_case(m, 0, "Choice::A"), return_cleanup_for_case(m, 0, "Choice::B")),
        (Ok(a.clone()), Ok(b.clone()))
    ));
    // Swap two valid action lists without changing the sum, target or tag test.
    let (left, right) = m
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            BlockTerminator::Conditional { when_true, when_false, .. } => Some((&mut when_true.cleanup, &mut when_false.cleanup)),
            _ => None,
        })
        .unwrap();
    std::mem::swap(left, right);
    assert_eq!(return_cleanup_for_case(m, 0, "Choice::A"), Ok(b));
    assert_ne!(return_cleanup_for_case(m, 0, "Choice::A"), Ok(a));
    let pkg = sources::lower(&[("case.omg", CONTROL)]).unwrap();
    let m = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "discard")
        .unwrap();
    let [Parameter::Structural(plain)] = m.body.parameters[..] else {
        panic!("one plain sum")
    };
    assert!(m.body.operations.is_empty());
    assert_eq!(return_cleanup_for_case(m, 0, "Choice::A"), Ok(vec![CleanupAction::DiscardRoot(plain)]));
}

#[test]
fn a_domain_selected_whole_cleanup_precedes_any_payload_dispatch() {
    let qualified = CASE.replace(
        "machine discard(x:Choice){}",
        "machine finish(x:Choice){} domain Choice::Live dropped by finish; machine discard(x:Choice in Live){}",
    );
    let pkg = sources::lower(&[("case.omg", &qualified)]).unwrap();
    let finish = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "finish")
        .unwrap()
        .id;
    let m = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "discard")
        .unwrap();
    let [Parameter::Structural(root)] = m.body.parameters[..] else {
        panic!("one sum")
    };
    assert!(m.body.operations.is_empty(), "the whole qualified value transfers directly");
    let BlockTerminator::Return { cleanup, .. } = &m.body.blocks[0].terminator else {
        panic!("one returning edge")
    };
    assert!(
        matches!(cleanup.as_slice(), [CleanupAction::Invoke { place, ty, target }] if place.root == root && place.path.is_empty() && *ty == m.body.places[root.index()].ty && *target == finish)
    );
}

#[test]
fn an_empty_or_scalar_alternative_retires_only_the_home() {
    let source = CASE.replace("case B(left:Token,right:Token);", "case B(v:u8);case Empty;");
    let pkg = sources::lower(&[("case.omg", &source)]).unwrap();
    let m = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "discard")
        .unwrap();
    sources::control_flow::check_operation_ownership(&m.body).unwrap();
    let [Parameter::Structural(root)] = m.body.parameters[..] else {
        panic!("one dying sum")
    };
    for case in ["Choice::B", "Choice::Empty"] {
        assert_eq!(return_cleanup_for_case(m, 0, case), Ok(vec![CleanupAction::RetireHome(root)]));
    }
    assert!(matches!(
        return_cleanup_for_case(m, 0, "Choice::A").unwrap().first(),
        Some(CleanupAction::Invoke { .. })
    ));
}

#[test]
fn returning_a_value_captures_it_before_the_cleanup_and_preserves_other_roots_order() {
    let source = CASE.replace(
        "machine discard(x:Choice){}",
        "machine discard(first:Token,x:Choice,last:Lease,r:Token)->Token{move r}",
    );
    let pkg = sources::lower(&[("case.omg", &source)]).unwrap();
    let m = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "discard")
        .unwrap();
    sources::control_flow::check_operation_ownership(&m.body).unwrap();
    let [
        Parameter::Structural(first),
        Parameter::Structural(x),
        Parameter::Structural(last),
        Parameter::Structural(_),
    ] = m.body.parameters[..]
    else {
        panic!("four owned formals")
    };
    for case in ["Choice::A", "Choice::B"] {
        let schedule = return_cleanup_for_case(m, 1, case).unwrap();
        assert!(matches!(schedule.first(),Some(CleanupAction::Invoke { place, .. }) if place.root == last && place.path.is_empty()));
        assert_eq!(
            schedule[schedule.len() - 2..],
            [CleanupAction::RetireHome(x), CleanupAction::DiscardRoot(first)]
        );
    }
    let BlockTerminator::Jump(enter) = &m.body.blocks[0].terminator else {
        panic!("expected result capture")
    };
    assert!(matches!(enter.structural_arguments.as_slice(), [omega_lowering::CallArgument::MoveWhole(_)]));
    let [capture] = m.body.blocks[enter.target.get() - 1].structural_parameters[..] else {
        panic!("one captured result")
    };
    assert!(
        m.body
            .blocks
            .iter()
            .any(|b| matches!(b.terminator,BlockTerminator::Return { result: omega_lowering::ReturnEdge::Structural { source, .. }, .. } if source == capture))
    );
}

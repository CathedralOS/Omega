#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, Parameter, ReturnEdge};
use psi_handoff_consumer::returned_view_backing;

const MISSING_VIEW_EVIDENCE: &str = "suspension needs the exact backing and carry evidence of the live structural place";

#[test]
fn a_returned_view_keeps_the_actual_at_the_selected_formal_position() {
    for (element, returned, formal, backing) in [
        ("u8", "first", 0, 1),
        ("u8", "second", 1, 0),
        ("u16", "first", 0, 1),
        ("u16", "second", 1, 0),
    ] {
        let text = format!(
            "machine select(first:@[{element}],second:@[{element}])->@[{element}] {{ {returned} }}
             machine inspect(bytes:@[{element}])->u64 suspends {{ bytes.len }}
             machine forward(left:@[{element}],right:@[{element}])->u64 suspends {{
                 let chosen = select(right,left);
                 let count:u64 = suspend inspect(chosen);
                 count +% 1u64
             }}"
        );
        let mut package = source::lower(&[("returned_view.omg", &text)]).unwrap();
        let select_ty = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "select")
            .unwrap()
            .result_type
            .unwrap();
        let expected = if element == "u8" {
            omega_lowering::TypeShape::SharedByteView
        } else {
            omega_lowering::TypeShape::SharedElementView {
                element: omega_lowering::ScalarType::Integer { signed: false, width: 16 },
            }
        };
        assert_eq!(package.types.get(select_ty), Some(&expected));
        let observe = |package: &_| returned_view_backing(package, "forward", "inspect", "select");
        assert_eq!(observe(&package), Ok((formal, backing)));

        // Keep the two actuals and the retained result unchanged. Only replace
        // the selected formal on the returning callee's normal edge.
        let select = package
            .machines
            .iter_mut()
            .find(|machine| machine.declaration.path == "select")
            .unwrap();
        let Parameter::Structural(other) = select.body.parameters[1 - formal] else {
            panic!("expected the other view formal");
        };
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source, .. },
            ..
        } = &mut select.body.blocks[0].terminator
        else {
            panic!("expected the normal structural return");
        };
        *source = other;
        assert_eq!(observe(&package), Ok((1 - formal, 1 - backing)));
    }
}

#[test]
fn only_an_actual_that_can_supply_the_return_needs_backing_at_the_later_crossing() {
    for selected in ["known", "external"] {
        let text = format!(
            "boundary trait Device{{machine view(p:@[u8])->@[u8] reaches Device crashes none;}} machine pick(known:@[u8],external:@[u8])->@[u8] {{{selected}}} machine inspect(bytes:@[u8]) suspends {{}} machine forward(input:@[u8]) suspends reaches Device {{let external=Device::view(input); let selected=pick(input,external); suspend inspect(selected)}}"
        );
        let files = [("selected_origin.omg", text.as_str())];
        if selected == "external" {
            let err = source::refuse(
                &files,
                "selected_origin.omg",
                source::unique_span(&text, "inspect(selected)"),
                MISSING_VIEW_EVIDENCE,
            );
            assert!(
                err.site
                    .span
                    .slice(text.as_bytes())
                    .unwrap()
                    .ends_with(b"inspect(selected)")
            );
        } else {
            let pkg = source::lower(&files).unwrap();
            let forward = pkg
                .machines
                .iter()
                .find(|m| m.declaration.path == "forward")
                .unwrap();
            let [site] = forward.body.suspension_sites.as_slice() else {
                panic!("one crossing")
            };
            let [loan] = site.retained_loans.as_slice() else {
                panic!("one selected view")
            };
            let omega_lowering::PlaceOrigin::Operation(id) = forward.body.places[loan.descriptor.index()].origin else {
                panic!("completed pick result")
            };
            let pick = pkg
                .machines
                .iter()
                .find(|m| m.declaration.path == "pick")
                .unwrap();
            assert!(
                matches!(forward.body.operations[id.index()].kind,omega_lowering::OperationKind::Call {callee:omega_lowering::CallTarget::Machine(target),..} if target==pick.id)
            );
        }
    }
}

#[test]
fn a_returned_view_carries_the_producing_calls_unproved_precondition() {
    let text = "machine pick(p:@[u8])->@[u8] requires false {p} machine inspect(bytes:@[u8]) suspends {} machine forward(input:@[u8]) suspends {let selected=pick(input); suspend inspect(selected)}";
    let pkg = source::lower(&[("guarded_view.omg", text)]).unwrap();
    let forward = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let pick = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap();
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("one crossing")
    };
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one returned view")
    };
    let omega_lowering::PlaceOrigin::Operation(id) = forward.body.places[loan.descriptor.index()].origin else {
        panic!("completed pick result")
    };
    let omega_lowering::OperationKind::Call { callee, requirements, .. } = &forward.body.operations[id.index()].kind else {
        panic!("producing call")
    };
    assert_eq!(*callee, omega_lowering::CallTarget::Machine(pick.id));
    let [required] = pick.contract.requires.as_slice() else {
        panic!("one entry condition")
    };
    assert_eq!(required.predicate, [omega_lowering::PredicateStep::Boolean(false)]);
    assert_eq!(
        requirements.as_slice(),
        [omega_lowering::CallRequirement {
            site: omega_lowering::CallSite::Operation(id),
            clause: 0
        }]
    );
}

#[test]
fn a_local_conformance_return_uses_the_selected_implementations_body() {
    let text = "trait Views {machine view(x:@[u8],y:@[u8])->@[u8];} machine second(x:@[u8],y:@[u8])->@[u8]{y} Local satisfies Views{Views::view=second;} machine relay<const E:Views>(x:@[u8],y:@[u8])->@[u8]{E::view(x,y)} machine inspect(v:@[u8]) suspends{} machine forward(a:@[u8],b:@[u8]) suspends{let selected=relay::<Local>(a,b); suspend inspect(selected)}";
    let pkg = source::lower(&[("member_view.omg", text)]).unwrap();
    let second = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "second")
        .unwrap();
    let relay = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let [member] = relay.body.operations.as_slice() else {
        panic!("one member call")
    };
    let omega_lowering::OperationKind::Call {
        callee: omega_lowering::CallTarget::Member(slot),
        ..
    } = member.kind
    else {
        panic!("selected member")
    };
    assert!(matches!(pkg.member_calls[slot].target,omega_lowering::MemberRealization::Conformance{machine,..} if machine==second.id));
    let Parameter::Structural(y) = second.body.parameters[1] else {
        panic!("second formal")
    };
    assert!(matches!(second.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==y));
    let forward = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let [site] = forward.body.suspension_sites.as_slice() else {
        panic!("one parked call")
    };
    let [loan] = site.retained_loans.as_slice() else { panic!("one loan") };
    let omega_lowering::PlaceOrigin::Operation(id) = forward.body.places[loan.descriptor.index()].origin else {
        panic!("completed call")
    };
    assert!(
        matches!(forward.body.operations[id.index()].kind,omega_lowering::OperationKind::Call{callee:omega_lowering::CallTarget::Machine(target),..} if target==relay.id)
    );
}

#[test]
fn a_recursive_result_cannot_supply_its_own_backing() {
    let text = "machine cycle(v:@[u8])->@[u8]{cycle(v)} machine inspect(v:@[u8]) suspends{} machine forward(input:@[u8]) suspends {let selected=cycle(input); suspend inspect(selected)}";
    let err = source::refuse(
        &[("cycle.omg", text)],
        "cycle.omg",
        source::unique_span(text, "inspect(selected)"),
        MISSING_VIEW_EVIDENCE,
    );
    assert!(
        err.site
            .span
            .slice(text.as_bytes())
            .unwrap()
            .ends_with(b"inspect(selected)")
    );
}

#[path = "support/source.rs"]
mod source;

use psi_handoff_consumer::check_parked_byte_views;

#[test]
fn the_callee_and_continuation_retain_their_own_incoming_byte_views() {
    for (selected, resumed, selected_formal, resumed_formal) in [("left", "right", 0, 1), ("right", "left", 1, 0)] {
        let text = format!(
            "machine inspect(bytes:@[u8])->u64 suspends {{ bytes.len }}
             machine forward(left:@[u8],right:@[u8])->u64 suspends {{
                 let count:u64 = suspend inspect({selected});
                 count +% {resumed}.len
             }}"
        );
        let package = {
            let resolved = source::resolve(&[("parked_views.omg", &text)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let call = typed
                .calls
                .iter()
                .find(|c| c.machine.path == "inspect")
                .unwrap();
            assert!(call.acknowledgements.suspend);
            let [operand] = call.operands.as_slice() else {
                panic!("expected one typed actual")
            };
            assert_eq!(operand.site.span.slice(text.as_bytes()), Some(selected.as_bytes()));
            let typed_tree = typed.trees().next().unwrap();
            let tree = typed_tree.resolved();
            let Some(omega_resolved::Symbol::Binding { span, .. }) = tree.symbol(operand.node.node) else {
                panic!("the selected call operand must resolve to its caller's formal")
            };
            let start = text.find(&format!("{selected}:@[u8]")).unwrap();
            assert_eq!(span.get_start_offset(), start);
            omega_lowering::lower_package(&prepared).unwrap()
        };
        let caller = package
            .machines
            .iter()
            .find(|machine| machine.declaration.path == "forward")
            .unwrap();
        let [site] = caller.body.suspension_sites.as_slice() else {
            panic!("expected one parked call");
        };
        let check = |site| check_parked_byte_views(&package, "forward", "inspect", selected_formal, resumed_formal, site);
        assert_eq!(check(site), Ok(()));

        // Keep the rows sorted and swap only which descriptor each row describes.
        // The types and access modes still agree, so checking those alone would pass.
        let mut forged = site.clone();
        let [first, second] = forged.retained_loans.as_mut_slice() else {
            panic!("expected two retained descriptors");
        };
        std::mem::swap(&mut first.descriptor, &mut second.descriptor);
        forged.retained_loans.reverse();
        assert_eq!(check(&forged), Err("retained loan has the wrong role"));
    }
}

#[test]
fn a_view_used_by_both_the_callee_and_the_continuation_has_one_retained_loan() {
    let text = "machine inspect(bytes:@[u8])->u64 suspends {bytes.len} machine forward(unused:@[u8],selected:@[u8])->u64 suspends {let count:u64=suspend inspect(selected); count +% selected.len}";
    let pkg = source::lower(&[("shared-role.omg", text)]).unwrap();
    let caller = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let [site] = caller.body.suspension_sites.as_slice() else {
        panic!("one crossing")
    };
    let [loan] = site.retained_loans.as_slice() else {
        panic!("one incoming loan serves both roles")
    };
    assert_eq!(omega_lowering::Parameter::Structural(loan.descriptor), caller.body.parameters[1]);
    assert!(loan.needed_by_callee && loan.needed_after_completion);
}

#[test]
fn a_suspending_reborrow_keeps_the_selected_field_of_the_incoming_parent() {
    use omega_lowering::{CallArgument, OperationKind, Parameter, PlaceProjection, ReferenceAccess};
    for field in ["left", "right"] {
        let text = format!(
            "data Cell{{v:u8;}} data Pair{{left:Cell;right:Cell;}} machine inspect(c:@Cell) suspends {{}} machine use_field(p:@Pair) suspends {{suspend inspect(@p.{field})}}"
        );
        let pkg = source::lower(&[("field.omg", &text)]).unwrap();
        let body = &pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "use_field")
            .unwrap()
            .body;
        let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let [loan] = site.retained_loans.as_slice() else { panic!("one parent loan") };
        let omega_lowering::CallSite::Operation(id) = site.call else {
            panic!("ordinary call")
        };
        let OperationKind::Call { arguments, .. } = &body.operations[id.index()].kind else {
            panic!("call")
        };
        let [CallArgument::Borrow { place, access }] = arguments.as_slice() else {
            panic!("projected child borrow")
        };
        assert_eq!(Parameter::Structural(loan.descriptor), body.parameters[0]);
        assert_eq!(place.root, loan.descriptor);
        assert_eq!((*access, loan.access), (ReferenceAccess::Shared, ReferenceAccess::Shared));
        assert!(matches!(&place.path[..], [PlaceProjection::Referent,PlaceProjection::Field(selected)] if selected.path==format!("Pair::{field}")));
    }
}

#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, LoweredPackage, LoweringError, OperationKind, OwnedCustody, Parameter, PlaceProjection,
    ReferenceAccess, RetainedOwner, SuspensionSite,
};
use omega_resolved::Symbol;
use omega_typed::{Access, ObligationKind, Projection};

const DATA: &str = "data Cell {value:u8;} data Pair {left:Cell; right:Cell;}";

#[test]
fn a_disjoint_move_preserves_the_exact_owned_backing_of_either_borrowed_field() {
    for (selected, sibling) in [("left", "right"), ("right", "left")] {
        for terminal in [false, true] {
            let invocation = if terminal {
                format!("transition{{_ -> suspend park(@p.{selected}),}}")
            } else {
                format!("suspend park(@p.{selected})")
            };
            let text = format!(
                "{DATA} machine take(c:Cell){{}} machine park(c:@Cell) suspends{{}}
                 machine forward(p:Pair) suspends{{ take(move p.{sibling}); {invocation} }}"
            );
            let package = prepare_borrow(&text, "p", selected, Access::Shared).unwrap();
            let (site, argument) = parked_borrow(&package);
            let CallArgument::Borrow { place, access } = argument else {
                panic!("expected the original field borrow")
            };
            let [owner] = site.retained_owners.as_slice() else {
                panic!("expected only the live caller-owned remainder")
            };
            let body = &package
                .machines
                .iter()
                .find(|machine| machine.declaration.path == "forward")
                .unwrap()
                .body;
            assert_eq!(Parameter::Structural(place.root), body.parameters[0]);
            assert_eq!(*access, ReferenceAccess::Shared);
            assert!(matches!(place.path.as_slice(), [PlaceProjection::Field(field)] if field.path == format!("Pair::{selected}")));
            assert_eq!(
                (owner.place, owner.ty, owner.custody),
                (place.root, body.places[place.root.index()].ty, OwnedCustody::Caller)
            );
            assert!(
                matches!(owner.moved.as_slice(), [path] if matches!(path.as_slice(), [PlaceProjection::Field(field)] if field.path == format!("Pair::{sibling}")))
            );
            assert_eq!(matches!(site.call, CallSite::Edge(_)), terminal);

            // Change only the earlier transfer. The later borrow has the same
            // typed path, and the owner row now records that path as moved.
            // The checker, not lowering, decides that the loan has lost its backing.
            let text = text.replace(&format!("take(move p.{sibling})"), &format!("take(move p.{selected})"));
            let package = prepare_borrow(&text, "p", selected, Access::Shared).unwrap();
            let (site, argument) = parked_borrow(&package);
            let CallArgument::Borrow { place: overlapping, .. } = argument else {
                panic!("expected the original field borrow")
            };
            let [owner] = site.retained_owners.as_slice() else {
                panic!("expected the partially moved caller-owned root")
            };
            assert_eq!((owner.place, owner.custody), (overlapping.root, OwnedCustody::Caller));
            assert_eq!(owner.moved, std::slice::from_ref(&overlapping.path));
        }
    }
}

#[test]
fn a_mutable_projected_argument_keeps_the_original_writable_home_parked() {
    let text = format!(
        "{DATA} machine park(c:@mut Cell) suspends{{c.value=9u8;}}
         machine forward(p:Pair) suspends{{let mut q=move p; suspend park(@mut q.right)}}"
    );
    let package = prepare_borrow(&text, "q", "right", Access::Mut).unwrap();
    let (site, argument) = parked_borrow(&package);
    let CallArgument::Borrow { place, access } = argument else {
        panic!("expected an original writable place")
    };
    let body = &package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "forward")
        .unwrap()
        .body;
    assert_eq!(body.mutable_homes, [place.root]);
    assert_ne!(Parameter::Structural(place.root), body.parameters[0]);
    assert_eq!(*access, ReferenceAccess::Mutable);
    assert!(matches!(place.path.as_slice(), [PlaceProjection::Field(field)] if field.path == "Pair::right"));
    assert_eq!(
        site.retained_owners,
        [RetainedOwner {
            place: place.root,
            ty: body.places[place.root.index()].ty,
            moved: Vec::new(),
            custody: OwnedCustody::Caller,
            embedded_loans: Vec::new(),
        }]
    );

    let pure = text.replace(" suspends", "").replace("suspend park", "park");
    let package = source::lower(&[("owned_borrow_inputs.omg", &pure)]).unwrap();
    assert!(
        package
            .machines
            .iter()
            .all(|machine| machine.body.suspension_sites.is_empty())
    );
}

fn prepare_borrow(
    text: &str,
    binding: &str,
    field: &str,
    access: Access,
) -> Result<LoweredPackage, LoweringError> {
    let resolved = source::resolve(&[("owned_borrow_inputs.omg", text)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let call = typed
        .calls
        .iter()
        .find(|call| call.machine.path == "park")
        .unwrap();
    let [actual] = call.operands.as_slice() else {
        panic!("expected one typed loan actual")
    };
    assert!(call.acknowledgements.suspend);
    assert_eq!(actual.parameter.span.slice(text.as_bytes()), Some(b"c".as_slice()));

    let loan = typed
        .obligations
        .iter()
        .find_map(|obligation| match &obligation.kind {
            ObligationKind::Borrow { access: selected, place } if obligation.node == actual.node && *selected == access => Some(place),
            _ => None,
        })
        .expect("the prepared operand carries the original typed borrow obligation");
    assert_eq!(loan.span.slice(text.as_bytes()), Some(format!("{binding}.{field}").as_bytes()));

    let typed_tree = typed.trees().next().unwrap();
    let tree = typed_tree.resolved();
    let [operand] = tree.file.nodes[actual.node.node].children.as_slice() else {
        panic!("expected a borrow of an original place");
    };
    let (base, path) = typed.place_path(tree.file.node_id(*operand));
    let Some(Symbol::Binding { span, .. }) = tree.symbol(base.node) else {
        panic!("the borrow base should resolve to the local ownership binder");
    };
    let binder = if binding == "p" { "p:Pair" } else { "q=move" };
    assert_eq!(span.get_start_offset(), text.find(binder).unwrap());
    assert!(matches!(
        path.as_slice(),
        [step] if step.through.is_none() && matches!(&step.projection, Some(Projection::Field(declaration)) if declaration.path == format!("Pair::{field}"))
    ));

    let package = omega_lowering::lower_package(&prepared)?;
    for machine in &package.machines {
        source::control_flow::check_operation_ownership(&machine.body).unwrap();
    }
    Ok(package)
}

fn parked_borrow(package: &LoweredPackage) -> (&SuspensionSite, &CallArgument) {
    let caller = package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "forward")
        .unwrap();
    let callee = package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "park")
        .unwrap();
    let [site] = caller.body.suspension_sites.as_slice() else {
        panic!("only the selected call parks")
    };
    let (target, arguments) = match site.call {
        CallSite::Operation(id) => {
            let OperationKind::Call { callee, arguments, .. } = &caller.body.operations[id.index()].kind else {
                panic!("expected ordinary call")
            };
            (callee, arguments)
        }
        CallSite::Edge(id) => caller
            .body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                BlockTerminator::TerminalCall {
                    id: edge, callee, arguments, ..
                } if *edge == id => Some((callee, arguments)),
                _ => None,
            })
            .expect("expected terminal call"),
    };
    let [argument] = arguments.as_slice() else {
        panic!("expected one direct borrowed argument")
    };
    assert_eq!((*target, site.callee), (CallTarget::Machine(callee.id), CallTarget::Machine(callee.id)));
    assert!(site.retained_loans.is_empty());
    (site, argument)
}

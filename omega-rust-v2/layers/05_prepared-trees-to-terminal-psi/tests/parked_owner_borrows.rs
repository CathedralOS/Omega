#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, CallArgument, CallSite, OperationKind, PlaceProjection};
use psi_handoff_consumer::check_parked_owner_borrow;

#[test]
fn a_parked_call_borrows_the_selected_field_without_transferring_its_owner() {
    for (field, other) in [("left", "right"), ("right", "left")] {
        for (body, terminal) in [
            (format!("let result:u64 = suspend inspect(tag,@p.{field}); result +% 1u64"), false),
            (format!("transition {{ _ -> suspend inspect(tag,@p.{field}), }}"), true),
        ] {
            let text = format!(
                "data Cell{{v:u8;}} data Pair{{left:Cell;right:Cell;}}
                 machine inspect(tag:u64,c:@Cell)->u64 suspends {{tag}}
                 machine forward(p:Pair,tag:u64)->u64 suspends {{ {body} }}"
            );
            let mut package = source::lower(&[("parked_owner_borrows.omg", &text)]).unwrap();
            let expected = format!("Pair::{field}");
            assert_eq!(check_parked_owner_borrow(&package, &expected, terminal), Ok(()));

            let pair = package
                .data
                .iter()
                .find(|data| data.declaration.path == "Pair")
                .unwrap();
            let alternative = format!("Pair::{other}");
            let sibling = pair
                .fields
                .iter()
                .find(|item| item.declaration.path == alternative)
                .unwrap()
                .declaration
                .clone();
            let caller = package
                .machines
                .iter_mut()
                .find(|machine| machine.declaration.path == "forward")
                .unwrap();
            let body = &mut caller.body;
            let arguments = match body.suspension_sites[0].call {
                CallSite::Operation(id) => {
                    let OperationKind::Call { arguments, .. } = &mut body.operations[id.index()].kind else {
                        panic!("expected the ordinary call");
                    };
                    arguments
                }
                CallSite::Edge(id) => body
                    .blocks
                    .iter_mut()
                    .find_map(|block| match &mut block.terminator {
                        BlockTerminator::TerminalCall { id: edge, arguments, .. } if *edge == id => Some(arguments),
                        _ => None,
                    })
                    .expect("expected the terminal call"),
            };
            let CallArgument::Borrow { place, .. } = &mut arguments[1] else {
                panic!("expected the loan actual");
            };
            place.path[0] = PlaceProjection::Field(sibling);
            assert_eq!(
                check_parked_owner_borrow(&package, &expected, terminal),
                Err("the parked borrow selects a different sibling")
            );
        }
    }
}

#[test]
fn a_named_field_loan_keeps_its_source_while_the_call_or_continuation_needs_it() {
    use omega_lowering::{CallSite, OwnedCustody, Parameter, PlaceOrigin, ReferenceAccess};
    for (body_text, callee, continuation) in [
        ("suspend inspect(loan);", true, false),
        ("suspend tick(); observe(loan);", false, true),
        ("suspend inspect(loan); observe(loan);", true, true),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Holder{{token:Token;other:Token;}} machine inspect(t:@Token) suspends{{}} machine observe(t:@Token){{}} machine tick() suspends{{}} machine take(t:Token){{}} machine forward(h:Holder) suspends{{let loan:@Token=@h.token; take(move h.other); {body_text}}}"
        );
        let pkg = source::lower(&[("named_owner.omg", &text)]).unwrap();
        let body = &pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let [owner] = site.retained_owners.as_slice() else {
            panic!("one backing owner")
        };
        let [loan] = site.retained_loans.as_slice() else {
            panic!("one live named descriptor")
        };
        assert_eq!((owner.custody, Parameter::Structural(owner.place)), (OwnedCustody::Caller, body.parameters[0]));
        assert!(matches!(&owner.moved[..],[hole] if matches!(&hole[..],[PlaceProjection::Field(field)] if field.path=="Holder::other")));
        assert_eq!(
            (loan.access, loan.needed_by_callee, loan.needed_after_completion),
            (ReferenceAccess::Shared, callee, continuation)
        );
        let PlaceOrigin::Operation(def) = body.places[loan.descriptor.index()].origin else {
            panic!("loan definition")
        };
        let OperationKind::BorrowPlace { source, access } = &body.operations[def.index()].kind else {
            panic!("captured borrow")
        };
        assert_eq!((source.root, *access), (owner.place, ReferenceAccess::Shared));
        assert!(matches!(source.path.as_slice(),[PlaceProjection::Field(field)] if field.path=="Holder::token"));
        if callee {
            let CallSite::Operation(id) = site.call else { panic!("ordinary call") };
            assert!(
                matches!(&body.operations[id.index()].kind,OperationKind::Call{arguments,..} if matches!(&arguments[..],[CallArgument::Borrow{place,..}] if place.root==loan.descriptor && place.path==[PlaceProjection::Referent]))
            );
        }
        let unavailable = text.replace("take(move h.other)", "take(move h.token)");
        let err = source::lower(&[("named_owner.omg", &unavailable)]).unwrap_err();
        assert_eq!(err.message, "parked borrow has no live owned backing for its selected place");
        let call = if callee { "inspect(loan)" } else { "tick()" };
        assert!(
            err.site
                .span
                .slice(unavailable.as_bytes())
                .unwrap()
                .ends_with(call.as_bytes())
        );
    }
}

#[test]
fn transferring_the_backing_owner_in_the_same_call_leaves_no_parked_field_loan() {
    for (params, args) in [
        ("t:@Token,h:Holder", "@h.token,move h"),
        ("h:Holder,t:@Token", "move h,@h.token"),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Holder{{token:Token;}} machine park({params}) suspends{{}} machine forward(h:Holder) suspends{{suspend park({args})}}"
        );
        let err = source::lower(&[("simultaneous.omg", &text)]).unwrap_err();
        assert_eq!(err.message, "parked borrow has no live owned backing for its selected place");
        assert!(
            err.site
                .span
                .slice(text.as_bytes())
                .unwrap()
                .ends_with(format!("park({args})").as_bytes())
        );
    }
}

#[test]
fn a_borrow_and_a_moved_subtree_cannot_contain_one_another_at_suspension() {
    for (take_ty, taken, borrow_ty, borrowed) in [
        ("Pair", "h.pair", "Token", "h.pair.first"),
        ("Token", "h.pair.first", "Pair", "h.pair"),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Pair{{first:Token;second:Token;}} data Holder{{pair:Pair;other:Token;}} machine consume(t:{take_ty}){{}} machine park(t:@{borrow_ty}) suspends{{}} machine forward(h:Holder) suspends{{consume(move {taken}); suspend park(@{borrowed})}}"
        );
        let err = source::lower(&[("nested_overlap.omg", &text)]).unwrap_err();
        assert_eq!(err.message, "parked borrow has no live owned backing for its selected place");
        assert!(
            err.site
                .span
                .slice(text.as_bytes())
                .unwrap()
                .ends_with(format!("park(@{borrowed})").as_bytes())
        );
    }
}

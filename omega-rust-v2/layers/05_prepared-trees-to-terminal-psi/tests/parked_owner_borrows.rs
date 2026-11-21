#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, CallArgument, CallSite, LoweredPackage, MachineBody, OperationKind, PlaceProjection};
use psi_handoff_consumer::check_parked_owner_borrow;

const NO_BACKING: &str = "parked borrow has no live owned backing for its selected place";

fn forward_body(package: &mut LoweredPackage) -> &mut MachineBody {
    &mut package.machines.iter_mut().find(|m| m.declaration.path == "forward").unwrap().body
}

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
            let sibling = pair.fields.iter().find(|item| item.declaration.path == alternative).unwrap();
            let cell_ty = sibling.carrier;
            let sibling = sibling.declaration.clone();

            // The signature and retention row still say Pair, but the actual storage says Cell.
            let body = forward_body(&mut package);
            let root = body.suspension_sites[0].retained_owners[0].place.index();
            let pair_ty = std::mem::replace(&mut body.places[root].ty, cell_ty);
            assert!(check_parked_owner_borrow(&package, &expected, terminal).is_err(), "mistyped backing place accepted");
            forward_body(&mut package).places[root].ty = pair_ty;

            // Keeping the borrowed field live does not keep the separate scalar actual live.
            let activation = std::mem::take(&mut forward_body(&mut package).suspension_sites[0].activation);
            assert!(check_parked_owner_borrow(&package, &expected, terminal).is_err(), "missing tag accepted");
            let body = forward_body(&mut package);
            body.suspension_sites[0].activation = activation;
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
        let call = if callee { "inspect(loan)" } else { "tick()" };
        let err = source::refuse(
            &[("named_owner.omg", &unavailable)],
            "named_owner.omg",
            source::span_within(&unavailable, &format!("suspend {call}"), call),
            NO_BACKING,
        );
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
fn a_parked_field_reborrow_traces_to_the_same_incoming_loan_named_or_inline() {
    use omega_lowering::{CallTarget, OperationResult, Parameter, PlaceOrigin, ReferenceAccess, TypeShape};
    for (binding, actual) in [("", "@h.token"), ("let loan:@Token=@h.token;", "loan")] {
        for (pause, callee_needs) in [("suspend inspect", true), ("suspend tick(); observe", false)] {
            let text = format!(
                "data Token [carry(suspension:allowed,cpu:same,thread:any,address:stable,)]{{v:u8;}} data Holder{{token:Token;other:Token;}}
                 machine inspect(t:@Token) suspends{{}} machine tick() suspends{{}} machine observe(t:@Token){{}}
                 machine forward(h:@Holder) suspends{{{binding} {pause}({actual});}}"
            );
            let pkg = source::lower(&[("incoming_field.omg", &text)]).unwrap();
            let machine = |name| {
                pkg.machines
                    .iter()
                    .find(|m| m.declaration.path == name)
                    .unwrap()
            };
            let body = &machine("forward").body;
            let [Parameter::Structural(h)] = body.parameters[..] else {
                panic!("incoming loan")
            };
            let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
            let [loan] = site.retained_loans.as_slice() else {
                panic!("one live descriptor")
            };
            assert!(site.retained_owners.is_empty() && site.activation.is_empty());
            assert_eq!(
                (loan.access, loan.needed_by_callee, loan.needed_after_completion),
                (ReferenceAccess::Shared, callee_needs, !callee_needs)
            );
            assert_eq!(loan.ty, body.places[loan.descriptor.index()].ty);
            let CallSite::Operation(call) = site.call else { panic!("ordinary call") };
            let user = machine(if callee_needs { "inspect" } else { "observe" });
            assert_eq!(site.callee, CallTarget::Machine(if callee_needs { user.id } else { machine("tick").id }));
            let use_id = if callee_needs {
                call
            } else {
                let omega_lowering::CompletionContinuation::Operation(resume) = site.continuation else {
                    panic!("tick must resume at the operation that consumes the retained loan");
                };
                resume
            };
            let use_op = &body.operations[use_id.index()];
            let OperationKind::Call { callee, arguments, .. } = &use_op.kind else {
                panic!("loan use")
            };
            assert_eq!(*callee, CallTarget::Machine(user.id));
            let [
                CallArgument::Borrow {
                    place,
                    access: ReferenceAccess::Shared,
                },
            ] = arguments.as_slice()
            else {
                panic!("shared actual")
            };
            assert_eq!(place.root, loan.descriptor);
            let backing = if binding.is_empty() {
                assert_eq!(loan.descriptor, h);
                place
            } else {
                assert_eq!(place.path, [PlaceProjection::Referent]);
                assert_eq!(user.parameter_types, [loan.ty]);
                let PlaceOrigin::Operation(id) = body.places[place.root.index()].origin else {
                    panic!("named definition")
                };
                let op = &body.operations[id.index()];
                assert_eq!(op.result, OperationResult::Structural(loan.descriptor));
                let OperationKind::BorrowPlace {
                    source,
                    access: ReferenceAccess::Shared,
                } = &op.kind
                else {
                    panic!("named loan source")
                };
                source
            };
            assert_eq!(backing.root, h);
            let [PlaceProjection::Referent, PlaceProjection::Field(field)] = backing.path.as_slice() else {
                panic!("field of incoming referent")
            };
            let Some(TypeShape::NominalReference { referent: holder, .. }) = pkg.types.get(body.places[h.index()].ty) else {
                panic!("incoming holder")
            };
            let selected = &pkg.data[holder.get() - 1].fields[0];
            assert_eq!(field, &selected.declaration);
            assert_eq!(field.path, "Holder::token");
            let Some(TypeShape::NominalReference { referent: token, .. }) = pkg.types.get(user.parameter_types[0]) else {
                panic!("token formal")
            };
            let subject = &pkg.data[token.get() - 1];
            assert_eq!(selected.carrier, subject.carrier);
            let carry = subject.carry.as_ref().unwrap();
            assert!(carry.allows_suspension && carry.requires_same_cpu && carry.requires_stable_address && !carry.requires_same_thread);
        }
    }
}

#[test]
fn naming_a_field_reborrow_cannot_hide_forbidden_backing() {
    for (binding, actual) in [("", "@h.token"), ("let loan:@Token=@h.token;", "loan")] {
        let text = format!(
            "data Token [carry(suspension:forbidden,cpu:same,thread:any,address:stable,)]{{v:u8;}} data Holder{{token:Token;}}
             machine inspect(t:@Token) suspends{{}} machine forward(h:@Holder) suspends{{{binding} suspend inspect({actual});}}"
        );
        source::refuse(
            &[("forbidden.omg", &text)],
            "forbidden.omg",
            source::unique_span(&text, &format!("inspect({actual})")),
            "parked field reborrow has suspension-forbidden backing",
        );
    }
}

#[test]
fn a_named_field_reborrow_cannot_pick_one_of_two_incoming_loans() {
    let text = "data Token{v:u8;} data Holder{token:Token;} machine inspect(t:@Token) suspends{} machine forward(flag:bool,left:@Holder,right:@Holder) suspends{let h=match flag{true->left,false->right,}; let loan:@Token=@h.token; suspend inspect(loan);}";
    source::refuse(
        &[("joined_field.omg", text)],
        "joined_field.omg",
        source::unique_span(text, "match flag{true->left,false->right,}"),
        "interior match requires a supported result carrier: Match",
    );
}

#[test]
fn an_incoming_record_is_not_the_backing_of_an_unknown_loan_stored_in_it() {
    for (binding, actual) in [
        ("", "@h.inner.token"),
        ("let loan:@Token=@h.inner.token;", "loan"),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Inner{{token:Token;}} data Holder{{inner:@Inner;}} machine inspect(t:@Token) suspends{{}} machine forward(h:@Holder) suspends{{{binding} suspend inspect({actual});}}"
        );
        source::refuse(
            &[("unknown.omg", &text)],
            "unknown.omg",
            source::unique_span(&text, &format!("inspect({actual})")),
            "suspension needs the exact backing and carry evidence of the live structural place",
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
        let err = source::refuse(
            &[("simultaneous.omg", &text)],
            "simultaneous.omg",
            source::unique_span(&text, &format!("park({args})")),
            NO_BACKING,
        );
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
        let err = source::refuse(
            &[("nested_overlap.omg", &text)],
            "nested_overlap.omg",
            source::unique_span(&text, &format!("park(@{borrowed})")),
            NO_BACKING,
        );
        assert!(
            err.site
                .span
                .slice(text.as_bytes())
                .unwrap()
                .ends_with(format!("park(@{borrowed})").as_bytes())
        );
    }
}

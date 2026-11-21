#[path = "support/source.rs"]
mod source;

use omega_lowering::{BlockTerminator, CallArgument, CallSite, LoweredPackage, MachineBody, OperationKind, PlaceProjection};
use omega_lowering::{OwnedCustody, ReferenceAccess};
use psi_handoff_consumer::{ParkedFieldLoan, ParkedLoanBacking, check_parked_owner_borrow, parked_field_loans};

fn field_loan(
    slot: Option<usize>,
    retained: Option<(bool, bool)>,
    path: &[&str],
    backing: ParkedLoanBacking,
) -> ParkedFieldLoan {
    ParkedFieldLoan {
        slot,
        retained,
        access: ReferenceAccess::Shared,
        path: path.iter().map(|step| step.to_string()).collect(),
        backing,
    }
}

fn moved(path: &[&str]) -> ParkedLoanBacking {
    ParkedLoanBacking::Moved(path.iter().map(|step| step.to_string()).collect())
}

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
                CallSite::Cleanup(_) => panic!("a cleanup invocation does not suspend"),
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
    use omega_lowering::{CallSite, Parameter, PlaceOrigin};
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
        let slot = callee.then_some(0);
        assert_eq!(
            parked_field_loans(&pkg, "forward"),
            Ok(vec![field_loan(slot, Some((callee, continuation)), &["Holder::token"], ParkedLoanBacking::Intact)])
        );

        // Moving the borrowed field instead of its sibling changes only the
        // owner's hole. Lowering publishes the crossing as it does without a
        // suspension; the checker decides whether the live loan permits the move.
        let unavailable = text.replace("take(move h.other)", "take(move h.token)");
        let pkg = source::lower(&[("named_owner.omg", &unavailable)]).unwrap();
        spec_corpus::observe_handoff!(
            psi "psi-output/parked-named-loan-moved-source",
            assert_eq!(
                parked_field_loans(&pkg, "forward"),
                Ok(vec![field_loan(slot, Some((callee, continuation)), &["Holder::token"], moved(&["Holder::token"]))])
            ),
        );
        let pure = unavailable
            .replace(" suspends", "")
            .replace("suspend tick(); ", "")
            .replace("suspend inspect(loan);", "observe(loan);");
        let pkg = source::lower(&[("named_owner.omg", &pure)]).unwrap();
        assert!(pkg.machines.iter().all(|machine| machine.body.suspension_sites.is_empty()));
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

/// Token forbids suspension, so the checker must reject parking a loan of
/// `h.token` whether the call borrows it inline or through a named reborrow.
/// Lowering publishes the crossing instead of judging it: the parked loan leads
/// to the Token schema and its declared prohibition. The allowed twin differs
/// only in that axis, and a floor on Holder stays on the path for the checker.
/// The same holds when the record arrives through a join or behind a
/// descriptor stored in the incoming record.
#[test]
fn naming_a_field_reborrow_cannot_hide_forbidden_backing() {
    use omega_lowering::{Carry, TypeShape};
    use psi_handoff_consumer::{ParkedFieldReborrow, parked_field_loan_carry, parked_field_reborrow};
    let carry = |allows_suspension| Carry {
        allows_suspension,
        requires_same_cpu: true,
        requires_same_thread: false,
        requires_stable_address: true,
    };
    let (forbidden, allowed) = (carry(false), carry(true));
    for (binding, actual) in [("", "@h.token"), ("let loan:@Token=@h.token;", "loan")] {
        for (permission, file, expected) in [("forbidden", "forbidden.omg", &forbidden), ("allowed", "allowed.omg", &allowed)] {
            let text = format!(
                "data Token [carry(suspension:{permission},cpu:same,thread:any,address:stable,)]{{v:u8;}} data Holder{{token:Token;}}
             machine inspect(t:@Token) suspends{{}} machine forward(h:@Holder) suspends{{{binding} suspend inspect({actual});}}"
            );
            let mut package = source::lower(&[(file, &text)]).unwrap();
            if permission == "forbidden" {
                spec_corpus::observe_handoff!(
                    psi "psi-output/parked-field-carry",
                    assert_eq!(parked_field_loan_carry(&package), Ok(vec![("Holder", None), ("Token", Some(&forbidden))])),
                );
            }
            assert_eq!(parked_field_loan_carry(&package), Ok(vec![("Holder", None), ("Token", Some(expected))]));

            // The reader observes which schema on the loan's path holds the restriction.
            let token = package.data.iter().position(|d| d.declaration.path == "Token").unwrap();
            let holder = package.data.iter().position(|d| d.declaration.path == "Holder").unwrap();
            package.data[holder].carry = package.data[token].carry.take();
            assert_eq!(parked_field_loan_carry(&package), Ok(vec![("Holder", Some(expected)), ("Token", None)]));
        }

        let text = format!(
            "data Token{{v:u8;}} data Holder [carry(suspension:forbidden,cpu:same,thread:any,address:stable,)]{{token:Token;}}
             machine inspect(t:@Token) suspends{{}} machine forward(h:@Holder) suspends{{{binding} suspend inspect({actual});}}"
        );
        let package = source::lower(&[("holder_floor.omg", &text)]).unwrap();
        assert_eq!(parked_field_loan_carry(&package), Ok(vec![("Holder", Some(&forbidden)), ("Token", None)]));
    }

    // A joined record and a descriptor stored in the incoming record keep the
    // same handoff. The reborrow reader recovers the exact loan path down to a
    // field whose type is inspect's formal referent, and that Token schema
    // keeps the declared restriction. Each twin differs only in the suspension
    // axis; the records on the path declare no floor of their own.
    let joined = (
        "data Holder{token:Token;}",
        "flag:bool,left:@Holder,right:@Holder",
        "let h=match flag{true->left,false->right,};",
    );
    let stored = ("data Inner{token:Token;} data Holder{inner:@Inner;}", "h:@Holder", "");
    let reborrow = |incoming: Vec<usize>, crossings: &[&str], projection: &str| ParkedFieldReborrow {
        incoming,
        crossings: crossings.iter().map(|c| vec![c.to_string()]).collect(),
        projection: vec![projection.to_string()],
    };
    for ((records, formals, join), binding, actual, path) in [
        (joined, "let loan:@Token=@h.token;", "loan", reborrow(vec![1, 2], &[], "Holder::token")),
        (stored, "", "@h.inner.token", reborrow(vec![0], &["Holder::inner"], "Inner::token")),
        (stored, "let loan:@Token=@h.inner.token;", "loan", reborrow(vec![0], &["Holder::inner"], "Inner::token")),
    ] {
        for (permission, file, expected) in [("forbidden", "forbidden.omg", &forbidden), ("allowed", "allowed.omg", &allowed)] {
            let text = format!(
                "data Token [carry(suspension:{permission},cpu:same,thread:any,address:stable,)]{{v:u8;}} {records}
             machine inspect(t:@Token) suspends{{}} machine forward({formals}) suspends{{{join}{binding} suspend inspect({actual});}}"
            );
            let package = source::lower(&[(file, &text)]).unwrap();
            assert_eq!(parked_field_reborrow(&package, "forward", "inspect").as_ref(), Ok(&path), "{text}");
            let inspect = package.machines.iter().find(|m| m.declaration.path == "inspect").unwrap();
            let Some(TypeShape::NominalReference { referent, .. }) = package.types.get(inspect.parameter_types[0]) else {
                panic!("token formal")
            };
            let carries: Vec<_> = package
                .data
                .iter()
                .map(|d| (d.declaration.path.as_str(), d.carry.as_ref()))
                .collect();
            let token = &package.data[referent.get() - 1];
            assert_eq!((token.declaration.path.as_str(), token.carry.as_ref()), ("Token", Some(expected)), "{text}");
            assert!(carries.iter().all(|(name, carry)| *name == "Token" || carry.is_none()), "{carries:?}");
        }
    }
}

#[test]
fn a_named_field_reborrow_through_a_joined_reference_names_the_arriving_loan() {
    use psi_handoff_consumer::{ParkedFieldReborrow, parked_field_reborrow};
    for (positive, negative, formals) in [("left", "right", [1, 2]), ("right", "left", [2, 1])] {
        let text = format!(
            "data Token{{v:u8;}} data Holder{{token:Token;}} machine inspect(t:@Token) suspends{{}}
             machine forward(flag:bool,left:@Holder,right:@Holder) suspends{{let h=match flag{{true->{positive},false->{negative},}}; let loan:@Token=@h.token; suspend inspect(loan);}}"
        );
        let mut package = source::lower(&[("joined_field.omg", &text)]).unwrap();
        let expected = |incoming: [usize; 2]| {
            Ok(ParkedFieldReborrow {
                incoming: incoming.to_vec(),
                crossings: Vec::new(),
                projection: vec!["Holder::token".to_string()],
            })
        };
        spec_corpus::observe_handoff!(
            psi "psi-output/joined-field-reborrow",
            assert_eq!(parked_field_reborrow(&package, "forward", "inspect"), expected(formals)),
        );

        // Exchanging the two arriving loans changes which one the parked borrow holds.
        let body = forward_body(&mut package);
        let [
            _,
            omega_lowering::Parameter::Structural(left),
            omega_lowering::Parameter::Structural(right),
        ] = body.parameters[..]
        else {
            panic!("expected the incoming loans");
        };
        for block in &mut body.blocks {
            if let BlockTerminator::Jump(edge) = &mut block.terminator
                && let [CallArgument::CopyWhole(loan)] = edge.structural_arguments.as_mut_slice()
            {
                *loan = if *loan == left { right } else { left };
            }
        }
        assert_eq!(parked_field_reborrow(&package, "forward", "inspect"), expected([formals[1], formals[0]]));
    }
}

#[test]
fn a_reborrow_through_a_reference_stored_in_an_incoming_record_names_the_stored_loan() {
    use psi_handoff_consumer::{ParkedFieldReborrow, parked_field_reborrow};
    for (binding, actual) in [
        ("", "@h.inner.token"),
        ("let loan:@Token=@h.inner.token;", "loan"),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Inner{{token:Token;}} data Holder{{inner:@Inner;}} machine inspect(t:@Token) suspends{{}} machine forward(h:@Holder) suspends{{{binding} suspend inspect({actual});}}"
        );
        let mut package = source::lower(&[("unknown.omg", &text)]).unwrap();
        // h lends the storage of Holder::inner; the @Inner stored there is the parent of Inner::token.
        spec_corpus::observe_handoff!(
            psi "psi-output/stored-field-reborrow",
            assert_eq!(
                parked_field_reborrow(&package, "forward", "inspect"),
                Ok(ParkedFieldReborrow {
                    incoming: vec![0],
                    crossings: vec![vec!["Holder::inner".to_string()]],
                    projection: vec!["Inner::token".to_string()],
                })
            ),
        );

        // Without the crossing, Inner::token would be read as a field of h's own record.
        let body = forward_body(&mut package);
        let captured = body
            .operations
            .iter_mut()
            .find_map(|op| match &mut op.kind {
                OperationKind::BorrowPlace { source, .. } => Some(&mut source.path),
                OperationKind::Call { arguments, .. } => match arguments.as_mut_slice() {
                    [CallArgument::Borrow { place, .. }] if place.path.len() > 1 => Some(&mut place.path),
                    _ => None,
                },
                _ => None,
            })
            .expect("the captured place");
        assert_eq!(captured.remove(2), PlaceProjection::Referent);
        assert_eq!(
            parked_field_reborrow(&package, "forward", "inspect"),
            Err("each step must select a field of the reached record or cross a stored descriptor")
        );
    }
}

#[test]
fn a_same_call_owner_transfer_publishes_the_field_loan_beside_callee_custody() {
    use omega_lowering::{Parameter, PlaceOrigin};
    for (params, args, borrow, owner) in [
        ("t:@Token,h:Holder", "@h.token,move h", 0, 1),
        ("h:Holder,t:@Token", "move h,@h.token", 1, 0),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Holder{{token:Token;}} machine park({params}) suspends{{}} machine forward(h:Holder) suspends{{suspend park({args})}}"
        );
        let mut pkg = source::lower(&[("simultaneous.omg", &text)]).unwrap();
        // The callee receives the owner and a loan into its field in one call.
        // Psi records both; whether they may coexist is the checker's judgment.
        spec_corpus::observe_handoff!(
            psi "psi-output/parked-loan-callee-custody",
            assert_eq!(
                parked_field_loans(&pkg, "forward"),
                Ok(vec![field_loan(Some(borrow), None, &["Holder::token"], ParkedLoanBacking::Callee(owner))])
            ),
        );
        // `move h` transfers h into a fresh place before the call. h keeps no
        // row; the fresh place's row gives the moving formal custody, and the
        // crossing retains no separate loan or scalar.
        let body = forward_body(&mut pkg);
        let [Parameter::Structural(h)] = body.parameters[..] else { panic!("one owned formal") };
        let site = &body.suspension_sites[0];
        assert!(site.retained_loans.is_empty() && site.activation.is_empty());
        let [row] = site.retained_owners.as_slice() else { panic!("one owner row") };
        let PlaceOrigin::Operation(transfer) = body.places[row.place.index()].origin else {
            panic!("the moved place comes from the transfer")
        };
        assert!(matches!(&body.operations[transfer.index()].kind, OperationKind::MoveStructural { source } if source.root == h && source.path.is_empty()));
        assert!(row.moved.is_empty() && row.embedded_loans.is_empty() && row.ty == body.places[h.index()].ty);
        forward_body(&mut pkg).suspension_sites[0].retained_owners[0].custody = OwnedCustody::Caller;
        assert_eq!(parked_field_loans(&pkg, "forward"), Err("the moved root reaches no callee custody at the crossing"));
        forward_body(&mut pkg).suspension_sites[0].retained_owners[0].custody = OwnedCustody::CalleeArgument(borrow);
        assert_eq!(
            parked_field_loans(&pkg, "forward"),
            Err("callee custody names an argument that does not move the owner")
        );

        // Moving a second holder instead leaves the borrowed owner with the caller.
        let twin = text
            .replace("forward(h:Holder)", "forward(h:Holder,g:Holder)")
            .replace("move h", "move g");
        let pkg = source::lower(&[("simultaneous.omg", &twin)]).unwrap();
        assert_eq!(
            parked_field_loans(&pkg, "forward"),
            Ok(vec![field_loan(Some(borrow), None, &["Holder::token"], ParkedLoanBacking::Intact)])
        );
        let owners = &forward_body_of(&pkg).suspension_sites[0].retained_owners;
        assert_eq!(
            owners.iter().map(|row| row.custody).collect::<Vec<_>>(),
            [OwnedCustody::Caller, OwnedCustody::CalleeArgument(owner)]
        );
    }

    // An earlier whole transfer to another machine leaves no row for h at the
    // crossing. Lowering still publishes the borrow of h.token for the checker.
    let earlier = "data Token{v:u8;} data Holder{token:Token;} machine consume(h:Holder){} machine park(t:@Token) suspends{} machine forward(h:Holder) suspends{consume(move h); suspend park(@h.token)}";
    let pkg = source::lower(&[("simultaneous.omg", earlier)]).unwrap();
    let site = &forward_body_of(&pkg).suspension_sites[0];
    assert!(site.retained_owners.is_empty() && site.retained_loans.is_empty() && site.activation.is_empty());
    assert_eq!(parked_field_loans(&pkg, "forward"), Err("the moved root reaches no callee custody at the crossing"));
}

#[test]
fn a_borrow_and_a_moved_subtree_that_contain_one_another_both_reach_the_crossing() {
    for (take_ty, taken, borrow_ty, borrowed, path, hole) in [
        ("Pair", "h.pair", "Token", "h.pair.first", &["Holder::pair", "Pair::first"][..], &["Holder::pair"][..]),
        ("Token", "h.pair.first", "Pair", "h.pair", &["Holder::pair"][..], &["Holder::pair", "Pair::first"][..]),
    ] {
        let text = format!(
            "data Token{{v:u8;}} data Pair{{first:Token;second:Token;}} data Holder{{pair:Pair;other:Token;}} machine consume(t:{take_ty}){{}} machine park(t:@{borrow_ty}) suspends{{}} machine forward(h:Holder) suspends{{consume(move {taken}); suspend park(@{borrowed})}}"
        );
        let pkg = source::lower(&[("nested_overlap.omg", &text)]).unwrap();
        spec_corpus::observe_handoff!(
            psi "psi-output/parked-loan-moved-overlap",
            assert_eq!(parked_field_loans(&pkg, "forward"), Ok(vec![field_loan(Some(0), None, path, moved(hole))])),
        );
        // Moving the disjoint sibling leaves the selected subtree intact.
        let twin = text
            .replace(&format!("consume(t:{take_ty})"), "consume(t:Token)")
            .replace(&format!("move {taken}"), "move h.other");
        let pkg = source::lower(&[("nested_overlap.omg", &twin)]).unwrap();
        assert_eq!(parked_field_loans(&pkg, "forward"), Ok(vec![field_loan(Some(0), None, path, ParkedLoanBacking::Intact)]));
        assert_eq!(
            forward_body_of(&pkg).suspension_sites[0].retained_owners[0].moved,
            [vec![PlaceProjection::Field(field(&pkg, "Holder", "Holder::other"))]]
        );
    }
}

fn forward_body_of(package: &LoweredPackage) -> &MachineBody {
    &package.machines.iter().find(|m| m.declaration.path == "forward").unwrap().body
}

fn field(
    package: &LoweredPackage,
    data: &str,
    path: &str,
) -> omega_lowering::DeclarationId {
    let data = package.data.iter().find(|item| item.declaration.path == data).unwrap();
    data.fields.iter().find(|item| item.declaration.path == path).unwrap().declaration.clone()
}

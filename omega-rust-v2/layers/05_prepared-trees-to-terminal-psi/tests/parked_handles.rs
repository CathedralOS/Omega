#[path = "support/source.rs"]
mod source;

use psi_handoff_consumer::check_parked_handles;

#[test]
fn parked_calls_keep_the_exact_caller_and_callee_owners() {
    for (selected, formal) in [("left", 0), ("right", 1)] {
        for (body, terminal) in [
            (format!("let count:u64 = suspend step(move {selected}); count +% 1u64"), false),
            (format!("transition {{ _ -> suspend step(move {selected}), }}"), true),
        ] {
            let text = format!(
                "data Handle dropped by release {{mark:u8;}}
                 machine release(h:Handle) {{let Handle{{mark as _}}=h;}}
                 machine step(h:Handle)->u64 suspends {{7u64}}
                 machine forward(left:Handle,right:Handle)->u64 suspends {{ {body} }}"
            );
            let mut package = source::lower(&[("parked_handles.omg", &text)]).unwrap();
            spec_corpus::observe_handoff!(
                psi "psi-output/parked-custody",
                assert_eq!(check_parked_handles(&package, formal, terminal), Ok(())),
            );

            let caller = package
                .machines
                .iter_mut()
                .find(|machine| machine.declaration.path == "forward")
                .unwrap();
            let [first, second] = caller.body.suspension_sites[0].retained_owners.as_mut_slice() else {
                panic!("expected two retained owners");
            };
            std::mem::swap(&mut first.custody, &mut second.custody);
            assert_eq!(
                check_parked_handles(&package, formal, terminal),
                Err("parked custody must name the exact two whole handles")
            );
        }
    }
}

#[test]
fn a_parked_record_keeps_only_its_live_complement_after_a_transfer() {
    use omega_lowering::{BlockTerminator, CleanupAction, OwnedCustody, Parameter, PlaceProjection};
    for (removed, remaining) in [("left", "right"), ("right", "left")] {
        for tail in [false, true] {
            let finish = if tail { "transition{_ -> suspend wait(),}" } else { "suspend wait()" };
            let text = format!(
                "data Token{{v:u8;}} data Pair{{left:Token;right:Token;}} machine eat(t:Token){{}} machine wait() suspends{{}} machine forward(p:Pair) suspends{{eat(move p.{removed});{finish}}}"
            );
            let resolved = source::resolve(&[("partial.omg", &text)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let movement = typed
                .obligations
                .iter()
                .find_map(|o| match &o.kind {
                    omega_typed::ObligationKind::Move { place, .. } => Some(place),
                    _ => None,
                })
                .unwrap();
            assert_eq!(movement.span.slice(text.as_bytes()).unwrap(), format!("p.{removed}").as_bytes());
            let pkg = omega_lowering::lower_package(&prepared).unwrap();
            let body = &pkg
                .machines
                .iter()
                .find(|m| m.declaration.path == "forward")
                .unwrap()
                .body;
            let Parameter::Structural(p) = body.parameters[0] else { panic!("owned pair") };
            let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
            let [owner] = site.retained_owners.as_slice() else {
                panic!("only the residual pair stays live")
            };
            assert_eq!((owner.place, owner.ty, owner.custody), (p, body.places[p.index()].ty, OwnedCustody::Caller));
            let [hole] = owner.moved.as_slice() else { panic!("one transferred field") };
            assert!(matches!(hole.as_slice(), [PlaceProjection::Field(f)] if f.path==format!("Pair::{removed}")));
            let cleanup = body
                .blocks
                .iter()
                .find_map(|b| match &b.terminator {
                    BlockTerminator::TerminalCall { cleanup, .. } if tail => Some(cleanup),
                    BlockTerminator::Return { cleanup, .. } if !tail => Some(cleanup),
                    _ => None,
                })
                .unwrap();
            let [CleanupAction::DiscardResidual { place, .. }] = cleanup.as_slice() else {
                panic!("only the surviving sibling is disposed")
            };
            assert_eq!(place.root, p);
            assert!(matches!(place.path.as_slice(),[PlaceProjection::Field(f)] if f.path==format!("Pair::{remaining}")));
        }
    }
}

#[test]
fn fully_transferred_storage_has_no_claim_at_a_later_suspension() {
    let text = "data Token{v:u8;} data Pair{left:Token;right:Token;} machine eat(t:Token){} machine wait() suspends{} machine forward(p:Pair) suspends{eat(move p.left);eat(move p.right);suspend wait()}";
    let pkg = source::lower(&[("exhausted.omg", text)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    assert!(site.retained_owners.is_empty());
    assert!(matches!(&body.blocks.last().unwrap().terminator,omega_lowering::BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));
}

#[test]
fn a_parked_aggregate_preserves_its_live_fields_declared_carry_policy() {
    use omega_lowering::{OwnedCustody, Parameter, TypeShape};
    for (permission, allowed) in [("allowed", true), ("forbidden", false)] {
        let text = format!(
            "data Lease [linear,carry(suspension:{permission},cpu:same,thread:any,address:stable,)]{{key:u64;}} data Held [linear]{{lease:Lease;}} machine wait() suspends{{}} machine forward(held:Held)->Held suspends{{suspend wait();held}}"
        );
        let resolved = source::resolve(&[("carry.omg", &text)]);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let (_, properties) = prepared
            .typed()
            .data_properties
            .iter()
            .find(|(decl, _)| decl.path == "Lease")
            .unwrap();
        let input = properties.carry.as_ref().unwrap();
        assert_eq!(
            (
                input.allows_suspension,
                input.requires_same_cpu,
                input.requires_same_thread,
                input.requires_stable_address
            ),
            (allowed, true, false, true)
        );
        let pkg = omega_lowering::lower_package(&prepared).unwrap();
        let body = &pkg
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let [owner] = site.retained_owners.as_slice() else {
            panic!("one aggregate held for completion")
        };
        assert_eq!(body.parameters[0], Parameter::Structural(owner.place));
        assert!(owner.moved.is_empty() && owner.custody == OwnedCustody::Caller);
        let Some(TypeShape::Nominal(outer)) = pkg.types.get(owner.ty) else {
            panic!("nominal root")
        };
        let outer = &pkg.data[outer.get() - 1];
        assert!(outer.carry.is_none());
        let [field] = outer.fields.as_slice() else { panic!("one live field") };
        let Some(TypeShape::Nominal(lease)) = pkg.types.get(field.carrier) else {
            panic!("expected lease")
        };
        let output = pkg.data[lease.get() - 1].carry.as_ref().unwrap();
        assert_eq!(output, input);
    }
}

#[test]
fn parking_an_aggregate_cannot_erase_an_embedded_loan() {
    let text = "data Holder<'r>{r:@'r u8;} machine wait() suspends{} machine forward<'r>(h:Holder<'r>) suspends{suspend wait()}";
    let package = source::lower(&[("embedded.omg", text)]).unwrap();
    assert_eq!(psi_handoff_consumer::check_incoming_fields(&package, &[&["Holder::r"]]), Ok(()));
}

#[test]
fn identical_descriptor_types_keep_their_own_incoming_formal_and_nested_path() {
    let text = "data Pair<'r>{a:@'r u8;b:@'r u8;} data Envelope<'r>{pair:Pair<'r>;} machine wait() suspends{} machine forward<'r>(x:Envelope<'r>,y:Envelope<'r>) suspends{suspend wait()}";
    let resolved = source::resolve(&[("nested.omg", text)]);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let pair = prepared
        .typed()
        .record_fields
        .iter()
        .find(|(id, _)| id.path == "Pair")
        .unwrap()
        .1;
    assert_eq!(
        pair.iter()
            .map(|field| field.declaration.path.as_str())
            .collect::<Vec<_>>(),
        ["Pair::a", "Pair::b"]
    );
    assert_eq!(pair[0].ty, pair[1].ty);
    let mut package = omega_lowering::lower_package(&prepared).unwrap();
    drop(prepared);
    drop(resolved);
    let paths: &[&[&str]] = &[&["Envelope::pair", "Pair::a"], &["Envelope::pair", "Pair::b"]];
    assert_eq!(psi_handoff_consumer::check_incoming_fields(&package, paths), Ok(()));
    let body = &mut package
        .machines
        .iter_mut()
        .find(|machine| machine.declaration.path == "forward")
        .unwrap()
        .body;
    let [first, second] = body.suspension_sites[0].retained_owners.as_mut_slice() else {
        panic!("two owners")
    };
    std::mem::swap(&mut first.embedded_loans[0].origins, &mut second.embedded_loans[1].origins);
    assert_eq!(psi_handoff_consumer::check_incoming_fields(&package, paths), Err("wrong embedded loan"));
}

#[test]
fn moving_one_record_before_wait_leaves_only_the_sibling_descriptor() {
    use omega_lowering::{BlockTerminator, CleanupAction, EmbeddedLoanOrigin, EmbeddedLoanSource, Parameter, PlaceProjection};
    for (gone, live) in [("left", "right"), ("right", "left")] {
        let text = format!(
            "data Holder<'r>{{r:@'r u8;}} data Pair<'a,'b>{{left:Holder<'a>;right:Holder<'b>;}} machine take<'r>(h:Holder<'r>){{}} machine wait() suspends{{}} machine forward<'a,'b>(p:Pair<'a,'b>) suspends{{take(move p.{gone});suspend wait()}}"
        );
        let package = source::lower(&[("siblings.omg", &text)]).unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap()
            .body;
        let Parameter::Structural(formal) = body.parameters[0] else {
            panic!("owned pair")
        };
        let [owner] = body.suspension_sites[0].retained_owners.as_slice() else {
            panic!("residual pair")
        };
        let [hole] = owner.moved.as_slice() else { panic!("one moved field") };
        assert!(matches!(hole.as_slice(), [PlaceProjection::Field(f)] if f.path == format!("Pair::{gone}")));
        let [loan] = owner.embedded_loans.as_slice() else {
            panic!("one surviving reference")
        };
        assert_eq!(owner.place, formal);
        assert!(matches!(&loan.path[..], [PlaceProjection::Field(a), PlaceProjection::Field(r)] if a.path == format!("Pair::{live}") && r.path == "Holder::r"));
        assert_eq!(
            loan.origins,
            [EmbeddedLoanOrigin {
                arrival: Vec::new(),
                reborrows: Vec::new(),
                source: EmbeddedLoanSource::Formal {
                    formal,
                    path: loan.path.clone()
                },
            }]
        );
        let BlockTerminator::Return { cleanup, .. } = &body.blocks.last().unwrap().terminator else {
            panic!("return after wait")
        };
        assert!(matches!(cleanup.as_slice(), [CleanupAction::DiscardResidual { place, .. }] if place.root == formal && place.path == loan.path[..1]));
    }
}

#[test]
fn owner_transfers_keep_their_positions_among_scalar_call_arguments() {
    use omega_lowering::{CallArgument, CallSite, OperationKind, OwnedCustody};
    let text = "data Handle{v:u8;} machine step(tag:u8,first:Handle,flag:bool,second:Handle) suspends{} machine forward(a:Handle,b:Handle) suspends{suspend step(7u8,move b,true,move a)}";
    let pkg = source::lower(&[("positions.omg", text)]).unwrap();
    let body = &pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap()
        .body;
    let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
    let CallSite::Operation(id) = site.call else { panic!("ordinary call") };
    let OperationKind::Call { arguments, .. } = &body.operations[id.index()].kind else {
        panic!("expected call")
    };
    assert_eq!(site.retained_owners.len(), 2);
    for slot in [1, 3] {
        let CallArgument::MoveWhole(actual) = arguments[slot] else {
            panic!("owned actual")
        };
        assert!(
            site.retained_owners
                .iter()
                .any(|owner| owner.place == actual && owner.custody == OwnedCustody::CalleeArgument(slot))
        );
    }
}

#[path = "support/source.rs"]
mod source;

use omega_psi::{
    BlockTerminator, CallSite, CleanupAction, CompletionContinuation, OperationKind, OperationResult, OwnedCustody, PlaceOrigin, PlaceProjection, ReturnEdge,
};

#[test]
fn a_structural_result_is_established_only_after_its_producing_call_completes() {
    for attributes in [" [copy]", ""] {
        let program = format!(
            "data Item{attributes} {{ key:u64; }}
             machine load()->Item suspends {{ Item{{key:7u64}} }}
             machine wait() suspends {{}}
             machine read()->u64 suspends {{ let item = suspend load(); suspend wait(); item.key }}"
        );
        let package = source::lower(&[("completion.omg", &program)]).unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let [load, wait, read] = body.operations.as_slice() else {
            panic!("two calls followed by a field read")
        };
        let OperationResult::Structural(result) = load.result else {
            panic!("record result")
        };
        let OperationResult::Scalar(key) = read.result else { panic!("scalar field") };
        assert_eq!(body.places[result.index()].origin, PlaceOrigin::Operation(load.id));
        let [first, second] = body.suspension_sites.as_slice() else {
            panic!("two parked calls")
        };
        assert_eq!(first.call, CallSite::Operation(load.id));
        assert_eq!(first.continuation, CompletionContinuation::Operation(wait.id));
        assert!(first.retained_homes.is_empty() && first.retained_owners.is_empty());
        assert!(first.retained_loans.is_empty() && first.retained_callables.is_empty() && first.activation.is_empty());

        assert_eq!(second.call, CallSite::Operation(wait.id));
        assert_eq!(second.continuation, CompletionContinuation::Operation(read.id));
        let OperationKind::ProjectedScalarRead { source, .. } = &read.kind else {
            panic!("read completed record")
        };
        assert_eq!(source.root, result);
        assert!(matches!(source.path.as_slice(), [PlaceProjection::Field(field)] if field.path == "Item::key"));
        let [block] = body.blocks.as_slice() else { panic!("straight-line body") };
        let BlockTerminator::Return {
            result: ReturnEdge::Scalar(returned),
            cleanup,
            ..
        } = &block.terminator
        else {
            panic!("scalar return")
        };
        assert_eq!(*returned, key);
        let retained = if attributes.is_empty() {
            let [owner] = second.retained_owners.as_slice() else {
                panic!("one affine owner")
            };
            assert_eq!(owner.custody, OwnedCustody::Caller);
            assert!(owner.moved.is_empty() && second.retained_homes.is_empty());
            assert_eq!(cleanup, &[CleanupAction::DiscardRoot(result)]);
            owner.place
        } else {
            let observe = |site: &_| psi_handoff_consumer::parked_copyable_homes(&package, body, site);
            assert!(observe(first).unwrap().is_empty());
            let [observed] = observe(second).unwrap().try_into().unwrap();
            assert_eq!(observed.place, result);
            let mut premature = first.clone();
            premature.retained_homes.clone_from(&second.retained_homes);
            assert!(observe(&premature).is_err());
            let [home] = second.retained_homes.as_slice() else {
                panic!("one copyable home")
            };
            assert!(!home.needed_by_callee && home.needed_after_completion);
            assert!(second.retained_owners.is_empty() && cleanup.is_empty());
            home.place
        };
        spec_corpus::observe_handoff!(psi "psi-output/parked-structural-result", assert_eq!(retained, result));
    }
}

#[test]
fn a_terminal_call_forwards_the_record_on_completion_and_keeps_the_unpassed_owner() {
    use omega_psi::{CallArgument, CallTarget, Parameter};
    for selected in [0, 1] {
        let parameter = ["left", "right"][selected];
        let program = format!(
            "data Item {{ key:u64; }}
            machine relay(input:Item)->Item suspends {{ input }}
            machine forward(left:Item, right:Item)->Item suspends {{ transition {{ _ -> suspend relay(move {parameter}), }} }}"
        );
        let package = source::lower(&[("terminal.omg", &program)]).unwrap();
        let forward = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "forward")
            .unwrap();
        let relay = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "relay")
            .unwrap();
        let body = &forward.body;
        let Parameter::Structural(kept) = body.parameters[1 - selected] else {
            panic!("remaining input")
        };
        let Parameter::Structural(input) = body.parameters[selected] else {
            panic!("selected input")
        };
        let [site] = body.suspension_sites.as_slice() else {
            panic!("one terminal call")
        };
        let terminal = body.blocks.iter().find_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall {
                id,
                callee,
                arguments,
                cleanup,
                ..
            } => Some((id, callee, arguments, cleanup)),
            _ => None,
        });
        let (id, target, arguments, cleanup) = terminal.expect("forwarded call");
        assert_eq!(site.call, CallSite::Edge(*id));
        assert_eq!(site.continuation, CompletionContinuation::Return);
        assert_eq!(*target, CallTarget::Machine(relay.id));
        assert_eq!(site.callee, *target);
        assert_eq!(forward.result_type, Some(body.places[input.index()].ty));
        assert_eq!(forward.result_type, relay.result_type);
        let [CallArgument::MoveWhole(moved)] = arguments.as_slice() else {
            panic!("one transfer")
        };
        let PlaceOrigin::Operation(op) = body.places[moved.index()].origin else {
            panic!("move operation")
        };
        assert!(matches!(&body.operations[op.index()].kind, OperationKind::MoveStructural { source } if source.root == input && source.path.is_empty()));
        assert_eq!(site.retained_owners.len(), 2);
        assert!(
            site.retained_owners
                .iter()
                .any(|owner| owner.place == kept && owner.custody == OwnedCustody::Caller)
        );
        assert!(
            site.retained_owners
                .iter()
                .any(|owner| owner.place == *moved && owner.custody == OwnedCustody::CalleeArgument(0))
        );
        assert_eq!(cleanup, &[CleanupAction::DiscardRoot(kept)]);
    }
}

#[test]
fn a_returned_reference_still_names_the_borrowed_field_after_completion() {
    use omega_psi::{CallArgument, CallTarget, Parameter, ReferenceAccess};
    for field in ["first", "second"] {
        let program = format!(
            "data Item [copy] {{ first:u64; second:u64; }}
            machine pick(k:@u64)->@u64 suspends {{ k }}
            machine read(x:Item)->u64 suspends {{ let loan = suspend pick(@x.{field}); $loan }}"
        );
        let package = source::lower(&[("borrowed.omg", &program)]).unwrap();
        let pick = package
            .machines
            .iter()
            .find(|m| m.declaration.path == "pick")
            .unwrap();
        let body = &package
            .machines
            .iter()
            .find(|m| m.declaration.path == "read")
            .unwrap()
            .body;
        let [Parameter::Structural(input)] = body.parameters.as_slice() else {
            panic!("one input")
        };
        let [call, read] = body.operations.as_slice() else {
            panic!("call followed by dereference")
        };
        let [site] = body.suspension_sites.as_slice() else { panic!("one crossing") };
        let OperationResult::Structural(result) = call.result else {
            panic!("reference result")
        };
        assert_eq!(site.call, CallSite::Operation(call.id));
        assert_eq!(site.callee, CallTarget::Machine(pick.id));
        assert_eq!(site.continuation, CompletionContinuation::Operation(read.id));
        assert!(site.retained_loans.is_empty());
        let [home] = site.retained_homes.as_slice() else {
            panic!("backing for the call")
        };
        assert_eq!(home.place, *input);
        assert!(home.needed_by_callee && !home.needed_after_completion);
        let OperationKind::Call { arguments, .. } = &call.kind else { panic!("call") };
        let [
            CallArgument::Borrow {
                place,
                access: ReferenceAccess::Shared,
            },
        ] = arguments.as_slice()
        else {
            panic!("borrow operand")
        };
        assert_eq!(place.root, home.place);
        assert!(matches!(place.path.as_slice(), [PlaceProjection::Field(selected)] if selected.path == format!("Item::{field}")));
        let [Parameter::Structural(formal)] = pick.body.parameters.as_slice() else {
            panic!("borrowed formal")
        };
        assert!(matches!(pick.body.blocks[0].terminator, BlockTerminator::Return { result: ReturnEdge::Structural {source, ..}, .. } if source == *formal));
        assert!(matches!(&read.kind, OperationKind::ProjectedScalarRead {source, ..} if source.root == result && source.path == [PlaceProjection::Referent]));
    }
}

#[test]
fn a_parked_terminal_call_keeps_the_dying_sum_and_binds_its_result_only_on_completion() {
    use omega_psi::{CallTarget, Parameter};
    let program = "data Token { value:u8; }
        data Lease dropped by release { id:u8; }
        machine release(x:Lease) { let Lease{id as _}=x; }
        data Choice { case A(x:Lease); case B(x:Token); }
        machine relay(x:Token)->Token suspends { x }
        machine forward(kept:Choice, input:Token)->Token suspends {
            transition { _ -> suspend relay(move input), }
        }";
    let pkg = source::lower(&[("terminal.omg", program)]).unwrap();
    let forward = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "forward")
        .unwrap();
    let relay = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let body = &forward.body;
    let Parameter::Structural(kept) = body.parameters[0] else {
        panic!("owned sum")
    };
    let [site] = body.suspension_sites.as_slice() else { panic!("one park") };
    let (edge, target) = body
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            BlockTerminator::TerminalCall {
                id,
                callee,
                completion: Some(target),
                cleanup,
                ..
            } => {
                assert_eq!(*callee, CallTarget::Machine(relay.id));
                assert!(cleanup.is_empty());
                Some((*id, *target))
            }
            _ => None,
        })
        .expect("terminal call has a normal completion path");
    assert_eq!(site.call, CallSite::Edge(edge));
    assert_eq!(site.continuation, CompletionContinuation::Block(target));
    let [result] = body.blocks[target.get() - 1].structural_parameters[..] else {
        panic!("returned token")
    };
    assert_eq!(body.places[result.index()].origin, PlaceOrigin::BlockParameter(target));
    assert_eq!(Some(body.places[result.index()].ty), relay.result_type);
    assert!(body.blocks.iter().any(|block| matches!(block.terminator,
        BlockTerminator::Return { result: ReturnEdge::Structural { source, .. }, .. } if source == result)));
    assert!(site.retained_homes.iter().all(|row| row.place != result));
    assert!(site.retained_owners.iter().all(|row| row.place != result));
    assert!(
        site.retained_owners
            .iter()
            .any(|row| row.place == kept && row.custody == OwnedCustody::Caller)
    );
    let observe = |site: &_| psi_handoff_consumer::parked_homes(body, site);
    let mut wrong = site.clone();
    wrong.continuation = CompletionContinuation::Return;
    spec_corpus::observe_handoff!(psi "psi-output/parked-terminal-cleanup-result", assert!(
        observe(site).is_ok() && observe(&wrong).is_err()
    ));
}

#[test]
fn temporary_loans_keep_their_producers_in_argument_order_and_cleanup_on_completion() {
    use omega_psi::{CallArgument, CallTarget, ReferenceAccess};
    let text = "data C dropped by release { v:u8; } machine release(x:C){let C{v as _}=move x;}
                machine first()->C{C{v:5u8}} machine second()->C{C{v:9u8}}
                machine compare(a:@C,b:@u8)->bool suspends {a.v == $b}
                machine after(){} machine test()->bool suspends {
                    let value = suspend compare(@first(),@second().v); after(); value
                }";
    let package = source::lower(&[("temp.omg", text)]).unwrap();
    let machine = |name: &str| {
        package
            .machines
            .iter()
            .find(|m| m.declaration.path == name)
            .unwrap()
    };
    let body = &machine("test").body;
    let [first, second, compare, after] = body.operations.as_slice() else {
        panic!("four calls in order")
    };
    for (op, name) in [
        (first, "first"),
        (second, "second"),
        (compare, "compare"),
        (after, "after"),
    ] {
        assert!(matches!(&op.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == machine(name).id));
    }
    let (OperationResult::Structural(left), OperationResult::Structural(right)) = (first.result, second.result) else {
        panic!("two fresh owners")
    };
    let OperationKind::Call { arguments, .. } = &compare.kind else {
        unreachable!()
    };
    let [
        CallArgument::Borrow {
            place: a,
            access: ReferenceAccess::Shared,
        },
        CallArgument::Borrow {
            place: b,
            access: ReferenceAccess::Shared,
        },
    ] = &arguments[..]
    else {
        panic!("two loans")
    };
    assert!(a.root == left && a.path.is_empty());
    assert!(b.root == right && matches!(&b.path[..], [PlaceProjection::Field(f)] if f.path == "C::v"));
    let [park] = body.suspension_sites.as_slice() else {
        panic!("one suspended consumer")
    };
    assert_eq!(park.call, CallSite::Operation(compare.id));
    assert_eq!(
        park.retained_owners
            .iter()
            .filter(|o| o.custody == OwnedCustody::Caller)
            .map(|o| o.place)
            .collect::<Vec<_>>(),
        [left, right]
    );
    let [before, next, resumed] = body.blocks.as_slice() else {
        panic!("two reverse-order dispositions")
    };
    assert!(before.operations.contains(&compare.id.index()) && resumed.operations.contains(&after.id.index()));
    for (block, target, owner) in [(before, next, right), (next, resumed, left)] {
        let BlockTerminator::Jump(edge) = &block.terminator else {
            panic!("cleanup on continuation")
        };
        assert_eq!(edge.target, target.id);
        assert!(matches!(&edge.cleanup[..], [CleanupAction::Invoke { place, target, .. }]
            if place.root == owner && place.path.is_empty() && *target == machine("release").id));
    }
    assert!(matches!(&resumed.terminator, BlockTerminator::Return { cleanup, .. } if cleanup.is_empty()));
}

#[test]
fn a_terminal_consumer_cleans_its_anonymous_owner_after_completion() {
    use omega_psi::{CallArgument, ReferenceAccess};
    let text = "data C dropped by release { v:u8; } machine release(x:C){let C{v as _}=move x;}
                machine produce()->C{C{v:7u8}} machine read(x:@C)->u8{x.v}
                machine go()->u8 { transition { _ -> read(@produce()) } }";
    let package = source::lower(&[("temp.omg", text)]).unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "go")
        .unwrap()
        .body;
    let [producer] = body.operations.as_slice() else {
        panic!("one completed producer")
    };
    let OperationResult::Structural(root) = producer.result else {
        panic!("owned result")
    };
    let (arguments, cleanup) = body
        .blocks
        .iter()
        .find_map(|b| match &b.terminator {
            BlockTerminator::TerminalCall { arguments, cleanup, .. } => Some((arguments, cleanup)),
            _ => None,
        })
        .unwrap();
    assert!(matches!(&arguments[..], [CallArgument::Borrow { place, access: ReferenceAccess::Shared }] if place.root == root && place.path.is_empty()));
    assert!(matches!(&cleanup[..], [CleanupAction::Invoke { place, target, .. }]
        if place.root == root && place.path.is_empty() && package.machines.iter().any(|m| m.id == *target && m.declaration.path == "release")));
}

#[test]
fn a_returned_child_cannot_trigger_cleanup_at_its_producing_call() {
    let text = "data Item { v:u8; }
                machine make()->Item {Item{v:7u8}}
                machine retain(item:@Item)->@Item {item}
                machine test()->u8 {let saved=retain(@make()); saved.v}";
    source::refuse(
        &[("child.omg", text)],
        "child.omg",
        source::unique_span(text, "retain(@make())"),
        "a loan carried beyond this call needs its temporary backing on the later continuation",
    );
}

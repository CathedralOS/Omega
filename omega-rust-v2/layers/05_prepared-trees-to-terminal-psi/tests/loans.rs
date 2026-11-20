#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge};
use omega_typed::{Access, ObligationKind};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

#[test]
fn returned_reference_keeps_the_selected_input_and_projected_field() {
    for (root, index) in [("a", 0), ("b", 1)] {
        let src = format!("data Cell {{value:u8;}} machine select(a:@Cell,b:@Cell)->@u8 {{ @{}.value }}", root);
        let lowered = pipeline::lower(&src);
        let body = &lowered.package.machines[0].body;
        let Parameter::Structural(parameter) = body.parameters[index] else {
            panic!("a borrowed input needs its descriptor");
        };
        let [op] = body.operations.as_slice() else {
            panic!("one reborrow");
        };
        let OperationKind::BorrowPlace { source, access } = &op.kind else {
            panic!("expected an explicit borrow");
        };
        assert_eq!(*access, ReferenceAccess::Shared);
        assert_eq!(source.root, parameter);
        assert_eq!(psi_handoff_consumer::returned_reborrow(body), Some(source));
        assert!(matches!(source.path.as_slice(),[PlaceProjection::Referent,PlaceProjection::Field(field)] if field.path=="Cell::value"));
        let BlockTerminator::Return {
            result: ReturnEdge::Structural { source, .. },
            ..
        } = body.blocks[0].terminator
        else {
            panic!("borrowed return expected");
        };
        assert_eq!(op.result, OperationResult::Structural(source));

        // The prepared borrow and returned-loan obligations sit on this occurrence.
        // The operation retained at that site is the shared borrow of the selected
        // formal's field, and the return edge carries its descriptor.
        let borrows = lowered.obligations("Borrow");
        let [borrow] = borrows.as_slice() else {
            panic!("{root}: expected one borrow, found {borrows:?}")
        };
        assert_eq!(lowered.spelling(&borrow.site), format!("@{root}.value"));
        let ObligationKind::Borrow { access: Access::Shared, place } = &borrow.kind else {
            panic!("{root}: expected a shared borrow, found {:?}", borrow.kind)
        };
        assert_eq!(lowered.spelling(place), format!("{root}.value"));
        assert_eq!(
            pipeline::operation_at(body, &borrow.site),
            op,
            "{root}: the borrow site is the borrow operation"
        );
        let loans = lowered.obligations("ReturnBorrow");
        let [returned] = loans.as_slice() else {
            panic!("{root}: expected one returned loan, found {loans:?}")
        };
        let ObligationKind::ReturnBorrow { machine } = &returned.kind else {
            unreachable!()
        };
        assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, "select"));
        assert_eq!(lowered.machine("select").declaration.path, machine.path);
        let carried = pipeline::operation_at(body, &returned.site);
        assert!(
            matches!(pipeline::returns(body).as_slice(), [ReturnEdge::Structural { source, .. }] if carried.result == OperationResult::Structural(*source)),
            "{root}: the returned loan is the descriptor produced at its occurrence"
        );
    }
    observed("obligation-output/borrow-return");
}

#[test]
fn forwarding_a_shared_descriptor_keeps_its_input_identity_without_reading_storage() {
    let src = "data Cell{v:u8;} machine f(a:@Cell,b:@Cell)->@Cell{let x=b; x}";
    let pkg = sources::lower(&[("loan.omg", src)]).unwrap();
    let body = &pkg.machines[0].body;
    assert!(body.operations.is_empty());
    let Parameter::Structural(a) = body.parameters[1] else {
        panic!("descriptor");
    };
    assert!(matches!(body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if a==source));
}

#[test]
fn exclusive_move_and_replacement_retain_both_exact_paths() {
    // `taken` is the place L3 types at the move; `field` is the assignment destination.
    for (taken, field) in [("left", "left"), ("left", "right"), ("right", "right")] {
        let src = format!("data Cell {{v:u8;}} data Pair {{left:Cell;right:Cell;}} machine edit(p:@mut Pair) {{let old=move p.{taken}; p.{field}=move old;}}");
        let lowered = pipeline::lower(&src);
        let body = &lowered.package.machines[0].body;
        let moved = body
            .operations
            .iter()
            .find_map(|o| match &o.kind {
                OperationKind::BorrowedMove { source } => Some((source, o.result)),
                _ => None,
            })
            .unwrap();
        // `negative_controls.rs` turns this refill into a replacement and requires these witnesses to reject it.
        let stored = psi_handoff_consumer::product_witness::vacancy_refill(body).unwrap();
        if taken == field {
            psi_handoff_consumer::product_witness::restores_moved_place(body).unwrap();
        }
        assert_eq!(body.parameters, [Parameter::Structural(moved.0.root)]);
        assert_eq!(moved.0.root, stored.0.root);
        spec_corpus::observe_handoff!(
            "borrowed-move-place/consumer",
            assert!(matches!(moved.0.path.as_slice(),[PlaceProjection::Referent,PlaceProjection::Field(f)] if f.path==format!("Pair::{taken}")))
        );
        assert!(matches!(stored.0.path.as_slice(),[PlaceProjection::Referent,PlaceProjection::Field(f)] if f.path==format!("Pair::{}",field)));
        assert_eq!(moved.0 == stored.0, taken == field);
        let takes: Vec<_> = body
            .operations
            .iter()
            .filter(|o| matches!(o.kind, OperationKind::BorrowedMove { .. }))
            .collect();
        let BlockTerminator::Return { cleanup, .. } = &body.blocks.last().unwrap().terminator else {
            panic!("normal exit")
        };
        if taken == field {
            assert_eq!(takes.len(), 1);
            assert!(cleanup.is_empty());
        } else {
            let [_, displaced] = takes.as_slice() else {
                panic!("the sibling write displaces its own value")
            };
            assert!(matches!(&displaced.kind,OperationKind::BorrowedMove{source} if source==stored.0));
            assert!(cleanup.is_empty());
            let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
                panic!("old value needs a cleanup continuation");
            };
            assert!(
                matches!(edge.cleanup.as_slice(),[omega_lowering::CleanupAction::DiscardRoot(place)] if OperationResult::Structural(*place)==displaced.result)
            );
            assert!(
                body.operations[body.blocks[edge.target.get() - 1].operations.clone()]
                    .iter()
                    .any(|o| matches!(o.kind, OperationKind::StructuralStore { .. }))
            );
        }
        let expected = moved.1;
        let supplied = OperationResult::Structural(stored.1);
        assert!(expected==supplied || body.operations.iter().any(|op| op.result==supplied && matches!(&op.kind,OperationKind::MoveStructural{source} if source.path.is_empty() && OperationResult::Structural(source.root)==expected)));

        // One restoration obligation names the extracted place `p.{taken}`. The move
        // whose operand is that place lowers to the exclusive extraction at the move's
        // own site, and a store fills that exact vacancy only when the source restores it.
        let restores = lowered.obligations("RestoreBorrowed");
        let [restore] = restores.as_slice() else {
            panic!("{taken}: expected one restoration, found {restores:?}")
        };
        assert_eq!(lowered.spelling(&restore.site), format!("p.{taken}"));
        let ObligationKind::RestoreBorrowed { place, ty } = &restore.kind else {
            unreachable!()
        };
        assert_eq!(*place, restore.node, "{taken}: the restoration names its own occurrence");
        assert_eq!(ty.nominal_parts().unwrap().0.path, "Cell");
        let moves: Vec<_> = lowered
            .obligations("Move")
            .into_iter()
            .filter(|m| matches!(&m.kind, ObligationKind::Move { place, .. } if *place == restore.site))
            .collect();
        let [extraction] = moves.as_slice() else {
            panic!("{taken}: expected one move of the restored place, found {moves:?}")
        };
        let extracted = pipeline::operation_at(body, &extraction.site);
        assert!(
            matches!(&extracted.kind, OperationKind::BorrowedMove { source } if source == moved.0),
            "{taken}: {:?}",
            extracted.kind
        );
        assert_eq!(extracted.result, moved.1);
        let OperationResult::Structural(claim) = extracted.result else {
            panic!("{taken}: an extraction yields a place")
        };
        let cell = lowered
            .package
            .data
            .iter()
            .find(|d| d.declaration.path == "Cell")
            .unwrap();
        assert_eq!(body.places[claim.index()].ty, cell.carrier);
        let refills = body
            .operations
            .iter()
            .filter(|op| {
                matches!(&op.kind, OperationKind::StructuralStore { destination, disposition: omega_lowering::StructuralStoreDisposition::FillVacancy, .. } if destination == moved.0)
            })
            .count();
        assert_eq!(refills, usize::from(taken == field), "{taken} -> {field}: refills of the restored place");
    }
    observed("obligation-output/restore-borrowed");
}

#[test]
fn branch_and_direct_call_retain_the_actual_returned_input() {
    let src = "data Cell{v:u8;} machine pick(a:@Cell,b:@Cell,c:bool)->@Cell {match c {true -> b,false -> a,}} machine relay(first:@Cell,second:@Cell,c:bool)->@Cell {pick(second,first,c)}";
    let pkg = sources::lower(&[("loan.omg", src)]).unwrap();
    let pick = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "pick")
        .unwrap();
    let relay = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let BlockTerminator::Conditional { when_true, when_false, .. } = &pick.body.blocks[0].terminator else {
        panic!("two exits")
    };
    assert_ne!(when_true.target, when_false.target);
    // `negative_controls.rs` swaps these targets and requires this witness to reject them.
    psi_handoff_consumer::product_witness::branch_returns_formals(&pick.body, 1, 0).unwrap();
    let [operation] = relay.body.operations.as_slice() else {
        panic!("one call");
    };
    let OperationKind::Call { callee, arguments, .. } = &operation.kind else {
        panic!("local call")
    };
    assert_eq!(*callee, omega_lowering::CallTarget::Machine(pick.id));
    for (argument, formal) in arguments.iter().zip([1, 0]) {
        let Parameter::Structural(input) = relay.body.parameters[formal] else {
            panic!("descriptor")
        };
        assert!(matches!(argument,omega_lowering::CallArgument::Borrow{place,..} if place.root==input && place.path==[PlaceProjection::Referent]));
    }
    assert!(
        matches!(relay.body.blocks[0].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if operation.result==OperationResult::Structural(source))
    );
}

#[test]
fn nested_reference_call_borrows_the_descriptor_returned_by_the_inner_call() {
    let src = "data Cell{v:u8;} machine identity(x:@Cell)->@Cell{x} machine relay(first:@Cell,second:@Cell)->@Cell{ (identity(identity(second))) }";
    let pkg = sources::lower(&[("loan.omg", src)]).unwrap();
    let relay = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let [inner, outer] = relay.body.operations.as_slice() else {
        panic!("two ordered calls");
    };
    let OperationResult::Structural(returned) = inner.result else {
        panic!("inner result")
    };
    let Parameter::Structural(second) = relay.body.parameters[1] else {
        panic!("second descriptor")
    };
    for (op, expected) in [(inner, second), (outer, returned)] {
        let OperationKind::Call { arguments, .. } = &op.kind else {
            panic!("reference call")
        };
        assert!(
            matches!(arguments.as_slice(),[omega_lowering::CallArgument::Borrow{place,..}] if place.root==expected && place.path==[PlaceProjection::Referent])
        );
    }
}

#[test]
fn a_restored_view_element_allows_a_later_window_at_another_selector() {
    let source = "data Cell{v:u8;} machine edit(p:@mut [Cell], i:u64, j:u64) { let first=move p[i]; p[i]=move first; let second=move p[j]; p[j]=move second; }";
    let pkg = sources::lower(&[("loan.omg", source)]).unwrap();
    let body = &pkg.machines[0].body;
    let selectors: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| {
            let OperationKind::BorrowedMove { source } = &op.kind else { return None };
            match source.path.last() {
                Some(PlaceProjection::ViewElement { index, .. }) => Some(Parameter::Scalar(index.selector)),
                _ => None,
            }
        })
        .collect();
    assert_eq!(selectors, &body.parameters[1..]);
    assert_eq!(
        body.operations
            .iter()
            .filter(|op| matches!(
                op.kind,
                OperationKind::StructuralStore {
                    disposition: omega_lowering::StructuralStoreDisposition::FillVacancy,
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn replacing_a_known_disjoint_view_element_disposes_only_its_old_value() {
    let src = "data Cell{v:u8;} machine edit(p:@mut [Cell], replacement:Cell) {let old=move p[0]; p[1]=move old; p[0]=move replacement;}";
    let pkg = sources::lower(&[("loan.omg", src)]).unwrap();
    let body = &pkg.machines[0].body;
    let moves: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::BorrowedMove { .. }))
        .collect();
    assert_eq!(moves.len(), 2);
    let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
        panic!("displaced sibling needs normal cleanup")
    };
    assert!(matches!(edge.cleanup.as_slice(),[omega_lowering::CleanupAction::DiscardRoot(id)] if moves[1].result==OperationResult::Structural(*id)));
    assert!(matches!(&body.blocks[edge.target.get()-1].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));
}

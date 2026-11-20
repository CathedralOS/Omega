#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, OperationKind, OperationResult, Parameter, PlaceProjection, ReferenceAccess, ReturnEdge};

#[test]
fn returned_reference_keeps_the_selected_input_and_projected_field() {
    for (root, index) in [("a", 0), ("b", 1)] {
        let src = format!("data Cell {{value:u8;}} machine select(a:@Cell,b:@Cell)->@u8 {{ @{}.value }}", root);
        let pkg = sources::lower(&[("loan.omg", &src)]).unwrap();
        let body = &pkg.machines[0].body;
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
    }
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
    for field in ["left", "right"] {
        let src = format!(
            "data Cell {{v:u8;}} data Pair {{left:Cell;right:Cell;}} machine edit(p:@mut Pair) {{let old=move p.left; p.{}=move old;}}",
            field
        );
        let pkg = sources::lower(&[("loan.omg", &src)]).unwrap();
        let body = &pkg.machines[0].body;
        let moved = body
            .operations
            .iter()
            .find_map(|o| match &o.kind {
                OperationKind::BorrowedMove { source } => Some((source, o.result)),
                _ => None,
            })
            .unwrap();
        let stored = body
            .operations
            .iter()
            .find_map(|o| match &o.kind {
                OperationKind::StructuralStore { destination, value, .. } => Some((destination, *value)),
                _ => None,
            })
            .unwrap();
        assert_eq!(moved.0.root, stored.0.root);
        assert!(matches!(moved.0.path.as_slice(),[PlaceProjection::Referent,PlaceProjection::Field(f)] if f.path=="Pair::left"));
        assert!(matches!(stored.0.path.as_slice(),[PlaceProjection::Referent,PlaceProjection::Field(f)] if f.path==format!("Pair::{}",field)));
        assert_eq!(moved.0 == stored.0, field == "left");
        let takes: Vec<_> = body
            .operations
            .iter()
            .filter(|o| matches!(o.kind, OperationKind::BorrowedMove { .. }))
            .collect();
        let BlockTerminator::Return { cleanup, .. } = &body.blocks.last().unwrap().terminator else {
            panic!("normal exit")
        };
        if field == "left" {
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
    }
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
    for (edge, formal) in [(when_true, 1), (when_false, 0)] {
        let Parameter::Structural(input) = pick.body.parameters[formal] else {
            panic!("descriptor")
        };
        assert!(
            matches!(pick.body.blocks[edge.target.get()-1].terminator,BlockTerminator::Return{result:ReturnEdge::Structural{source,..},..} if source==input)
        );
    }
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

//! Fixed-array elements can be copied at a runtime index. An owned transfer
//! also needs the selected ordinal so cleanup can identify the other elements.
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, CallArgument, CleanupAction, OperationKind, OperationResult, Parameter, PlaceProjection, ProjectedPlace};
const TOKEN: &str = "data Token { value:u8; }";

fn ordinals(place: &ProjectedPlace) -> Vec<u64> {
    place
        .path
        .iter()
        .map(|step| match step {
            PlaceProjection::FixedIndex(ordinal) => *ordinal,
            other => panic!("expected only fixed ordinals, found {other:?}"),
        })
        .collect()
}

fn residual_ordinals(
    cleanup: &[CleanupAction],
    root: omega_lowering::PlaceId,
) -> Vec<Vec<u64>> {
    cleanup
        .iter()
        .map(|action| match action {
            CleanupAction::DiscardResidual { place, .. } if place.root == root => ordinals(place),
            other => panic!("expected a residual of the array root, found {other:?}"),
        })
        .collect()
}

fn lowering_error(source: &str) -> String {
    sources::lower(&[("arrays.omg", source)]).unwrap_err().message
}

#[test]
fn moving_sibling_zero_or_one_returns_that_ordinal_and_disposes_only_the_other() {
    for (selected, remaining) in [(0u64, 1u64), (1, 0)] {
        let source = format!("{TOKEN} machine pick(items:[Token;2])->Token{{move items[{selected}]}}");
        let package = {
            let resolved = sources::resolve(&[("pick.omg", &source)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let tree = typed.trees().next().unwrap();
            let file = tree.resolved().file;
            let operand = typed
                .obligations
                .iter()
                .find_map(|o| match &o.kind {
                    omega_typed::ObligationKind::Move { ty, .. } if ty.nominal_parts().is_some_and(|(id, _)| id.path == "Token") => {
                        Some(file.nodes[o.node.node].children[0])
                    }
                    _ => None,
                })
                .expect("the element transfer keeps its move obligation");
            assert!(
                tree.semantics_at(operand)
                    .all(|row| !matches!(row, omega_typed::NodeSemantics::CollectionAccess(_))),
                "an owned element transfer selects no copying collection read"
            );
            let (capture, steps) = typed.place_path(file.node_id(operand));
            assert_eq!(file.span(capture.node).slice(source.as_bytes()), Some(b"items".as_slice()));
            let [step] = steps.as_slice() else { panic!("one indexed step") };
            assert!(step.through.is_none());
            let Some(omega_typed::Projection::Index(selector)) = &step.projection else {
                panic!("the step selects by index")
            };
            assert_eq!(file.span(selector.node).slice(source.as_bytes()), Some(selected.to_string().as_bytes()));
            omega_lowering::lower_package(&prepared).unwrap()
        };
        let pick = &package.machines[0];
        let [Parameter::Structural(root)] = pick.body.parameters[..] else {
            panic!("owned array parameter expected")
        };
        let (taken, cleanup) = psi_handoff_consumer::returned_move(&pick.body, &pick.body.blocks[0]).expect("the result is the selected element");
        assert_eq!(taken.root, root);
        assert_eq!(taken.path, [PlaceProjection::FixedIndex(selected)]);
        let [CleanupAction::DiscardResidual { place, ty }] = cleanup else {
            panic!("only the live sibling is disposed, found {cleanup:?}")
        };
        assert_eq!(place.root, root);
        assert_eq!(place.path, [PlaceProjection::FixedIndex(remaining)]);
        assert_eq!(Some(*ty), pick.result_type);
    }
}

#[test]
fn residuals_list_every_remaining_ordinal_in_decreasing_order() {
    let source = format!("{TOKEN} machine pick(items:[Token;4])->Token{{move items[1]}}");
    let package = sources::lower(&[("order.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let (_, cleanup) = psi_handoff_consumer::returned_move(body, &body.blocks[0]).unwrap();
    assert_eq!(residual_ordinals(cleanup, root), [vec![3], vec![2], vec![0]]);
}

#[test]
fn an_untouched_owned_array_is_disposed_as_one_root() {
    let source = format!("{TOKEN} machine keep(items:[Token;3]){{}}");
    let package = sources::lower(&[("whole.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("return expected")
    };
    assert_eq!(cleanup, &[CleanupAction::DiscardRoot(root)]);
}

#[test]
fn a_frozen_constant_selector_names_the_same_static_ordinal() {
    let source = format!("{TOKEN} const SECOND:u64=1; machine pick(items:[Token;3])->Token{{move items[SECOND]}}");
    let package = sources::lower(&[("frozen.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let (taken, cleanup) = psi_handoff_consumer::returned_move(body, &body.blocks[0]).unwrap();
    assert_eq!(taken.path, [PlaceProjection::FixedIndex(1)]);
    assert_eq!(residual_ordinals(cleanup, root), [vec![2], vec![0]]);
}

#[test]
fn a_consumer_receives_the_element_and_the_edge_disposes_only_the_sibling() {
    let source = format!("{TOKEN} machine take(t:Token){{}} machine apply(items:[Token;2]){{take(move items[1]);}}");
    let package = sources::lower(&[("consumer.omg", &source)]).unwrap();
    let take = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "take")
        .unwrap();
    let [Parameter::Structural(token)] = take.body.parameters[..] else {
        panic!("owned token parameter expected")
    };
    let BlockTerminator::Return { cleanup, .. } = &take.body.blocks[0].terminator else {
        panic!("return expected")
    };
    assert_eq!(
        cleanup,
        &[CleanupAction::DiscardRoot(token)],
        "the consumer disposes the transferred element once"
    );
    let apply = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "apply")
        .unwrap();
    let [Parameter::Structural(root)] = apply.body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let [transfer, call] = apply.body.operations.as_slice() else {
        panic!("expected one element transfer and one call")
    };
    assert!(matches!(&transfer.kind, OperationKind::MoveStructural { source } if source.root == root && source.path == [PlaceProjection::FixedIndex(1)]));
    assert!(
        matches!(&call.kind, OperationKind::Call { arguments, .. } if matches!(arguments.as_slice(), [CallArgument::MoveWhole(place)] if transfer.result == OperationResult::Structural(*place)))
    );
    let BlockTerminator::Return { cleanup, .. } = &apply.body.blocks[0].terminator else {
        panic!("return expected")
    };
    assert_eq!(residual_ordinals(cleanup, root), [vec![0]]);
}

#[test]
fn each_return_arm_owns_only_its_sibling_complement() {
    let source = format!("{TOKEN} machine pick(items:[Token;2],first:bool)->Token{{match first {{true->move items[0],false->move items[1],}}}}");
    let package = sources::lower(&[("arms.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root), _] = body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let residuals: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => Some(residual_ordinals(cleanup, root)),
            _ => None,
        })
        .collect();
    assert_eq!(residuals, [vec![vec![1]], vec![vec![0]]]);
}

#[test]
fn reassembling_every_ordinal_consumes_each_element_once() {
    let source = format!("{TOKEN} machine swap(items:[Token;2])->[Token;2]{{[move items[1], move items[0]]}}");
    let package = sources::lower(&[("swap.omg", &source)]).unwrap();
    let body = &package.machines[0].body;
    let [Parameter::Structural(root)] = body.parameters[..] else {
        panic!("owned array parameter expected")
    };
    let moved: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match &op.kind {
            OperationKind::MoveStructural { source } if source.root == root => Some((ordinals(source), op.result)),
            _ => None,
        })
        .collect();
    assert_eq!(moved.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(), [vec![1], vec![0]]);
    let elements = body
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            OperationKind::EstablishStructuralArray { elements } => Some(elements),
            _ => None,
        })
        .expect("the literal assembles the transferred elements");
    assert_eq!(
        elements
            .iter()
            .map(|p| OperationResult::Structural(*p))
            .collect::<Vec<_>>(),
        moved.iter().map(|(_, result)| *result).collect::<Vec<_>>()
    );
    let BlockTerminator::Return { cleanup, .. } = &body.blocks[0].terminator else {
        panic!("return expected")
    };
    assert!(cleanup.is_empty(), "every ordinal transferred; nothing remains to dispose: {cleanup:?}");
}

#[test]
fn transferring_one_ordinal_twice_rejects() {
    let source = format!("{TOKEN} machine twice(items:[Token;2])->[Token;2]{{[move items[0], move items[0]]}}");
    assert_eq!(lowering_error(&source), "owned transfer overlaps previously moved storage");
}

#[test]
fn a_partially_transferred_array_cannot_leave_whole() {
    let source = format!("{TOKEN} machine whole(items:[Token;2])->[Token;2]{{let first=move items[0]; items}}");
    assert_eq!(lowering_error(&source), "owned transfer overlaps previously moved storage");
}

#[test]
fn a_runtime_selector_supplies_no_static_ordinal() {
    let source = format!("{TOKEN} machine pick(items:[Token;2], i:u64)->Token{{move items[i]}}");
    assert_eq!(lowering_error(&source), "owned element transfer requires a literal or frozen constant selector");
}

#[test]
fn an_ordinal_outside_the_declared_extent_rejects() {
    let source = format!("{TOKEN} machine pick(items:[Token;2])->Token{{move items[2]}}");
    let error = sources::lower(&[("arrays.omg", &source)]).unwrap_err();
    assert_eq!(error.message, "owned element transfer selects outside its fixed array");
    assert_eq!(error.site.file, "arrays.omg");
    let selector = source.rfind("[2]").unwrap() + 1;
    assert_eq!((error.site.span.get_start_offset(), error.site.span.get_end_offset()), (selector, selector + 1));
}

#[test]
fn nested_fixed_selection_preserves_every_ordinal_and_the_maximal_live_subtrees() {
    for expression in ["move matrix[1][0]", "(move matrix[1][0])"] {
        let source = format!("{TOKEN} machine f(matrix:[[Token;2];2])->Token {{ {expression} }}");
        let pkg = sources::lower(&[("matrix.omg", &source)]).unwrap();
        let body = &pkg.machines[0].body;
        let Parameter::Structural(root) = body.parameters[0] else {
            panic!("matrix input")
        };
        let (source, cleanup) = psi_handoff_consumer::returned_move(body, &body.blocks[0]).expect("returned matrix element");
        assert_eq!(source.root, root);
        assert_eq!(ordinals(source), [1, 0]);
        assert_eq!(residual_ordinals(cleanup, root), [vec![1, 1], vec![0]]);
    }
}

#[test]
fn a_whole_owned_array_transfers_through_a_binding_and_a_call() {
    let source = format!(
        "{TOKEN} machine consume(items:[Token;2])->Token {{move items[1]}} machine relay(input:[Token;2])->Token {{let local=move input; consume(move local)}}"
    );
    let product = sources::lower(&[("whole-array.omg", &source)]).unwrap();
    let relay = product
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    let consume = product
        .machines
        .iter()
        .find(|m| m.declaration.path == "consume")
        .unwrap();
    let [first, second, call] = relay.body.operations.as_slice() else {
        panic!("local transfer, argument transfer, call")
    };
    let Parameter::Structural(input) = relay.body.parameters[0] else {
        panic!("owned input")
    };
    assert!(matches!(&first.kind,OperationKind::MoveStructural{source} if source.root==input && source.path.is_empty()));
    assert!(matches!(&second.kind,OperationKind::MoveStructural{source} if OperationResult::Structural(source.root)==first.result && source.path.is_empty()));
    assert!(
        matches!(&call.kind,OperationKind::Call{callee:omega_lowering::CallTarget::Machine(id),arguments,..} if *id==consume.id && matches!(arguments.as_slice(),[CallArgument::MoveWhole(p)] if OperationResult::Structural(*p)==second.result))
    );
    assert!(matches!(&relay.body.blocks[0].terminator,BlockTerminator::Return{cleanup,..} if cleanup.is_empty()));
}

#[test]
fn an_intact_array_invokes_each_elements_cleanup_in_reverse_establishment_order() {
    let src = "data Lease dropped by finish {v:u8;} machine finish(x:Lease) {let Lease{v as _}=x;} machine caller(items:[Lease;3]) {}";
    let pkg = sources::lower(&[("hook-array.omg", src)]).unwrap();
    let caller = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "caller")
        .unwrap();
    let hook = pkg
        .machines
        .iter()
        .find(|m| m.declaration.path == "finish")
        .unwrap();
    let Parameter::Structural(root) = caller.body.parameters[0] else {
        panic!("array input")
    };
    let BlockTerminator::Return { cleanup, .. } = &caller.body.blocks[0].terminator else {
        panic!("normal exit")
    };
    assert_eq!(cleanup.len(), 3);
    for (action, ordinal) in cleanup.iter().zip([2, 1, 0]) {
        assert!(
            matches!(action,CleanupAction::Invoke{place,ty,target} if *target==hook.id && *ty==hook.parameter_types[0] && place.root==root && place.path==[PlaceProjection::FixedIndex(ordinal)])
        );
    }
}

#[test]
fn reading_a_field_of_an_indexed_record_preserves_the_selector_and_bounds_obligation() {
    for (selector, slot) in [("left", 1), ("right", 2)] {
        let text = format!("data Cell [copy] {{ v:u8; }} machine read(cells:[Cell;2],left:u64,right:u64)->u8 {{ cells[{selector}].v }}");
        let pkg = {
            let resolved = sources::resolve(&[("indexed.omg", &text)]);
            let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let [access] = &typed.collection_accesses[..] else {
                panic!("one indexed access")
            };
            assert_eq!(access.primitive, omega_toolchain::CollectionPrimitive::Read);
            assert_eq!(access.element.nominal_parts().unwrap().0.path, "Cell");
            assert_eq!(access.result, access.element);
            assert_eq!(access.site.span.slice(text.as_bytes()), Some(format!("cells[{selector}]").as_bytes()));
            omega_lowering::lower_package(&prepared).unwrap()
        };
        let body = &pkg.machines[0].body;
        let Parameter::Structural(root) = body.parameters[0] else {
            panic!("expected array parameter")
        };
        let [copy, read] = body.operations.as_slice() else {
            panic!("expected an indexed copy followed by a field read")
        };
        let OperationKind::StructuralLeafCopy { source, path } = &copy.kind else {
            panic!("expected a record copy")
        };
        let [step] = path.as_slice() else { panic!("expected one checked index") };
        assert_eq!(*source, root);
        let OperationResult::Structural(copied) = copy.result else {
            panic!("expected copied element")
        };
        assert!(
            matches!(pkg.types.get(body.places[root.index()].ty), Some(omega_psi::TypeShape::FixedArray { element, length: 2 })
            if *element == body.places[copied.index()].ty)
        );
        assert_eq!(Parameter::Scalar(step.selector), body.parameters[slot]);
        assert_eq!(
            body.diagnostics.bounds[step.obligation.get() - 1]
                .span
                .slice(text.as_bytes()),
            Some(selector.as_bytes())
        );
        assert!(matches!(&read.kind, OperationKind::ProjectedScalarRead { source }
            if OperationResult::Structural(source.root) == copy.result && matches!(&source.path[..], [PlaceProjection::Field(field)] if field.path == "Cell::v")));
        assert!(
            matches!(&body.blocks[0].terminator, BlockTerminator::Return { result: omega_psi::ReturnEdge::Scalar(value), cleanup, .. }
            if read.result == OperationResult::Scalar(*value) && cleanup.is_empty())
        );
    }
}

#[test]
fn a_field_read_uses_the_record_returned_by_one_call() {
    let text = "data Cell [copy] { v:u8; } machine make(n:u8)->Cell { Cell {v:n} } machine read(n:u8)->u8 { make(n).v }";
    let package = sources::lower(&[("returned.omg", text)]).unwrap();
    let make = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "make")
        .unwrap();
    let body = &package
        .machines
        .iter()
        .find(|m| m.declaration.path == "read")
        .unwrap()
        .body;
    let [call, read] = body.operations.as_slice() else {
        panic!("the receiver must execute once")
    };
    assert!(matches!(call.kind, OperationKind::Call {callee:omega_psi::CallTarget::Machine(id),..} if id == make.id));
    assert!(matches!(&read.kind, OperationKind::ProjectedScalarRead { source } if
        OperationResult::Structural(source.root) == call.result && matches!(&source.path[..], [PlaceProjection::Field(field)] if field.path=="Cell::v")));
    assert!(
        matches!(body.blocks[0].terminator, BlockTerminator::Return { result: omega_psi::ReturnEdge::Scalar(value), .. }
        if read.result == OperationResult::Scalar(value))
    );
}

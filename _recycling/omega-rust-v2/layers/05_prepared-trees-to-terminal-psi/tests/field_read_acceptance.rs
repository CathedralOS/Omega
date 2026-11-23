//! Positive source programs must produce the selected field and preserve its owner.
#[path = "support/control_flow.rs"]
mod control_flow;
#[path = "sample_baseline/pipeline.rs"]
mod pipeline;

use omega_psi::{
    BlockTerminator, CallArgument, CallTarget, CleanupAction, LoweredPackage, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, PlaceProjection,
    ProjectedPlace, ReturnEdge,
};

#[track_caller]
fn lower(
    declarations: &str,
    parameters: &str,
    body: &str,
) -> LoweredPackage {
    let text = format!("{declarations} machine read({parameters})->u8 {{ {body} }}");
    let package = match pipeline::lower(&[("field.omg", text.as_bytes().to_vec())]) {
        Ok(package) => package,
        Err(error) => panic!("{error}\nsource bytes {}..{} in {}\n{text}",
            error.span.get_start_offset(), error.span.get_end_offset(), error.package),
    };
    for machine in &package.machines {
        if let Err(reason) = control_flow::check_operation_ownership(&machine.body) {
            panic!("{}: {reason:?}\n{text}", machine.declaration.path);
        }
    }
    package
}

fn reader(package: &LoweredPackage) -> &MachineBody {
    &package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "read")
        .unwrap()
        .body
}

fn returned_operation(body: &MachineBody) -> &omega_psi::PsiOperation {
    let returned: Vec<_> = body
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            BlockTerminator::Return {
                result: ReturnEdge::Scalar(value),
                ..
            } => Some(value),
            _ => None,
        })
        .collect();
    let [value] = returned.as_slice() else { panic!("expected one scalar return") };
    body.operations
        .iter()
        .find(|op| op.result == OperationResult::Scalar(*value))
        .unwrap()
}

fn returned_field(body: &MachineBody) -> &ProjectedPlace {
    let read = returned_operation(body);
    let OperationKind::ProjectedScalarRead { source } = &read.kind else {
        panic!("wrong source of the returned field: {:?}", read.kind)
    };
    source
}

fn check_cleanup(
    body: &MachineBody,
    owned: Option<PlaceId>,
) {
    let mut block_index = 0;
    let mut saw_read = false;
    let mut expected = owned.map(CleanupAction::DiscardRoot);
    let read = returned_operation(body).id;
    for _ in 0..body.blocks.len() {
        let block = &body.blocks[block_index];
        saw_read |= body.operations[block.operations.clone()]
            .iter()
            .any(|op| op.id == read);
        let (actions, next) = match &block.terminator {
            BlockTerminator::Jump(edge) => (&edge.cleanup, Some(edge.target.get() - 1)),
            BlockTerminator::Return { cleanup, .. } => (cleanup, None),
            other => panic!("unexpected exit in straight-line field read: {other:?}"),
        };
        for action in actions {
            assert!(saw_read, "receiver was disposed before its field was read");
            assert_eq!(Some(action), expected.take().as_ref(), "unexpected cleanup on this edge");
        }
        let Some(next) = next else {
            assert!(saw_read && expected.is_none(), "return skipped the read or the receiver's cleanup");
            return;
        };
        block_index = next;
    }
    panic!("straight-line field read has no return");
}

const COPY: &str = "data Cell [copy] { v:u8; w:u8; }";
const OWNED: &str = "data Cell { v:u8; w:u8; }";

#[test]
#[should_panic(expected = "receiver was disposed before its field was read")]
fn the_cleanup_check_detects_disposing_the_receiver_on_an_earlier_edge() {
    let declarations = format!("{OWNED} machine make(n:u8)->Cell {{ Cell {{v:n,w:n}} }}");
    let mut package = lower(&declarations, "cell:Cell,n:u8", "_ = cell.v; _ = make(n); cell.w");
    let body = &mut package
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == "read")
        .unwrap()
        .body;
    let root = returned_field(body).root;
    let BlockTerminator::Jump(edge) = &mut body.blocks[0].terminator else {
        panic!("missing temporary cleanup edge")
    };
    // Keep one disposal for the receiver, but put it before its field read.
    edge.cleanup = vec![CleanupAction::DiscardRoot(root)];
    for block in &mut body.blocks {
        if let BlockTerminator::Return { cleanup, .. } = &mut block.terminator {
            cleanup.clear();
        }
    }
    check_cleanup(body, Some(root));
}

#[test]
fn reading_an_owned_parameter_leaves_cleanup_with_its_owner() {
    let package = lower(OWNED, "cell:Cell", "cell.w");
    let body = reader(&package);
    let place = returned_field(body);
    assert_eq!(Parameter::Structural(place.root), body.parameters[0]);
    assert!(matches!(&place.path[..], [PlaceProjection::Field(f)] if f.path=="Cell::w"));
    check_cleanup(body, Some(place.root));
}

#[test]
fn a_nested_read_keeps_the_path_from_the_original_record() {
    let declarations = format!("{COPY} data Pair [copy] {{ left:Cell; right:Cell; }}");
    for (expression, selected) in [("p.left.w", "Pair::left"), ("p.right.w", "Pair::right")] {
        let package = lower(&declarations, "p:Pair", expression);
        let body = reader(&package);
        let place = returned_field(body);
        assert_eq!(Parameter::Structural(place.root), body.parameters[0]);
        assert!(matches!(&place.path[..], [PlaceProjection::Field(first),PlaceProjection::Field(second)]
            if first.path == selected && second.path=="Cell::w"));
        check_cleanup(body, None);
    }
}

#[test]
fn parameter_field_names_the_selected_root_and_field() {
    for (expression, slot, field) in [("left.v", 0, "Cell::v"), ("right.w", 1, "Cell::w")] {
        let package = lower(COPY, "left:Cell,right:Cell", expression);
        let body = reader(&package);
        let place = returned_field(body);
        assert_eq!(Parameter::Structural(place.root), body.parameters[slot]);
        assert!(matches!(&place.path[..], [PlaceProjection::Field(f)] if f.path == field));
        check_cleanup(body, None);
    }
}

fn call_result(
    declaration: &str,
    owned: bool,
) {
    let declarations = format!("{declaration} machine make(v:u8,w:u8)->Cell {{ Cell {{v:v,w:w}} }}");
    let package = lower(&declarations, "first:u8,last:u8", "make(last,first).w");
    let body = reader(&package);
    let place = returned_field(body);
    let make = package
        .machines
        .iter()
        .find(|m| m.declaration.path == "make")
        .unwrap();
    let calls: Vec<_> = body
        .operations
        .iter()
        .filter(|op| matches!(op.kind, OperationKind::Call { .. }))
        .collect();
    let [call] = calls.as_slice() else {
        panic!("expected one evaluation of make")
    };
    assert!(matches!(&call.kind, OperationKind::Call {callee:CallTarget::Machine(id),arguments,..}
        if *id == make.id && matches!(arguments.as_slice(),[CallArgument::Scalar(a),CallArgument::Scalar(b)]
            if [Parameter::Scalar(*a),Parameter::Scalar(*b)] == [body.parameters[1],body.parameters[0]])));
    assert_eq!(call.result, OperationResult::Structural(place.root));
    assert!(matches!(&place.path[..], [PlaceProjection::Field(f)] if f.path=="Cell::w"));
    check_cleanup(body, owned.then_some(place.root));
}

#[test]
fn returned_copyable_record_preserves_call_arguments_without_cleanup() {
    call_result(COPY, false);
}

#[test]
fn returned_owned_record_disposes_its_temporary_after_reading_the_field() {
    call_result(OWNED, true);
}

fn constructed(
    declaration: &str,
    owned: bool,
) {
    let package = lower(declaration, "first:u8,last:u8", "(Cell {v:last,w:first}).w");
    let body = reader(&package);
    let place = returned_field(body);
    let operation = body
        .operations
        .iter()
        .find(|op| op.result == OperationResult::Structural(place.root))
        .unwrap();
    let OperationKind::EstablishRecord { fields } = &operation.kind else {
        panic!("receiver must be the constructed record")
    };
    assert_eq!(fields.len(), 2);
    for (name, slot) in [("Cell::v", 1), ("Cell::w", 0)] {
        let field = fields.iter().find(|field| field.field.path == name).unwrap();
        assert!(matches!(field.operand,CallArgument::Scalar(value) if Parameter::Scalar(value)==body.parameters[slot]));
    }
    assert!(matches!(&place.path[..], [PlaceProjection::Field(f)] if f.path=="Cell::w"));
    check_cleanup(body, owned.then_some(place.root));
}

#[test]
fn constructed_copyable_record_supplies_the_returned_field() {
    constructed(COPY, false);
}

#[test]
fn constructed_owned_record_disposes_its_temporary_after_the_field_read() {
    constructed(OWNED, true);
}

#[test]
fn a_field_passed_to_a_call_survives_cleanup_of_its_temporary_receiver() {
    for hook in ["finish", "release"] {
        let declarations = format!("data Cell dropped by {hook} {{ mark:u8; }}
            machine finish(x:Cell) {{let Cell {{mark as _}}=x;}}
            machine release(x:Cell) {{let Cell {{mark as _}}=x;}}
            machine keep(n:u8)->u8 {{n}}");
        let package = lower(&declarations, "n:u8", "keep((Cell {mark:n}).mark)");
        let body = reader(&package);
        let selected = |name: &str| package.machines.iter().find(|m| m.declaration.path == name).unwrap().id;
        let [before, after] = &body.blocks[..] else { panic!("expected the temporary's cleanup edge") };
        let [construct, read] = &body.operations[before.operations.clone()] else { panic!("expected construction and field read before cleanup") };
        let OperationResult::Structural(root) = construct.result else { panic!("expected a structural temporary") };
        assert!(matches!(&read.kind, OperationKind::ProjectedScalarRead {source} if source.root == root
            && matches!(&source.path[..], [PlaceProjection::Field(f)] if f.path == "Cell::mark")));
        let BlockTerminator::Jump(edge) = &before.terminator else { panic!("missing cleanup edge") };
        assert_eq!(edge.target, after.id);
        assert!(matches!(&edge.cleanup[..], [CleanupAction::Invoke {place, ty, target}] if place.root == root
            && place.path.is_empty() && *ty == body.places[root.index()].ty && *target == selected(hook)));
        let [call] = &body.operations[after.operations.clone()] else { panic!("expected the consuming call") };
        assert!(matches!(&call.kind, OperationKind::Call {callee: CallTarget::Machine(id), arguments, ..} if *id == selected("keep")
            && matches!(&arguments[..], [CallArgument::Scalar(v)] if OperationResult::Scalar(*v) == read.result)));
        assert_eq!(returned_operation(body).id, call.id);
        assert!(matches!(&after.terminator, BlockTerminator::Return {cleanup, ..} if cleanup.is_empty()));
    }
}

fn joined(
    binding: bool,
    swapped: bool,
) {
    let (first, second, slots) = if swapped { ("right", "left", [2, 1]) } else { ("left", "right", [1, 2]) };
    let selection = format!("match flag {{true -> {first}, false -> {second},}}");
    let expression = if binding {
        format!("let chosen = {selection}; chosen.w")
    } else {
        format!("({selection}).w")
    };
    let package = lower(COPY, "flag:bool,left:Cell,right:Cell", &expression);
    let body = reader(&package);
    let place = returned_field(body);
    assert!(matches!(&place.path[..], [PlaceProjection::Field(f)] if f.path=="Cell::w"));
    let BlockTerminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &body.blocks[0].terminator
    else {
        panic!("expected branch")
    };
    assert_eq!(Parameter::Scalar(*condition), body.parameters[0]);
    for (edge, slot) in [when_true, when_false].into_iter().zip(slots) {
        let BlockTerminator::Jump(join) = &body.blocks[edge.target.get() - 1].terminator else {
            panic!("arm must supply the selected value")
        };
        let destination = &body.blocks[join.target.get() - 1];
        let position = destination
            .structural_parameters
            .iter()
            .position(|p| *p == place.root)
            .expect("field reads the joined record");
        assert!(matches!(join.structural_arguments[position],CallArgument::CopyWhole(root) if Parameter::Structural(root)==body.parameters[slot]));
    }
}

#[test]
fn bound_branch_result_selects_left_on_true() {
    joined(true, false);
}
#[test]
fn bound_branch_result_selects_right_on_true() {
    joined(true, true);
}
#[test]
fn inline_branch_result_selects_left_on_true() {
    joined(false, false);
}
#[test]
fn inline_branch_result_selects_right_on_true() {
    joined(false, true);
}

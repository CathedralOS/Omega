//! An exclusive descriptor chosen by a `match` arrives at the join by transfer.
//! Each predecessor moves its own incoming `@mut` descriptor into the join
//! parameter (terminal-psi/byte_views.md#calls-and-control-transfers), so the
//! checker can tell which loan each arrival carries and that none was copied.
#[path = "support/source.rs"]
mod source;

use omega_lowering::{
    BlockId, BlockTerminator, CallArgument, LoweredPackage, MachineBody, OperationKind, Parameter, PlaceOrigin, ReferenceAccess, ScalarType, TypeShape,
};
use psi_handoff_consumer::{ExclusiveJoin, JoinedUse, exclusive_join};

/// The program from the L5 coverage probe `exclusive view as a match result`, byte for byte.
const ORIGINAL: &str =
    include_str!("fixtures/package_cases/exclusive-view-match-result/case/case.omg");
const U32: ScalarType = ScalarType::Integer { signed: false, width: 32 };

fn lower(text: &str) -> LoweredPackage {
    source::lower(&[("case.omg", text)]).unwrap()
}

fn pick(package: &LoweredPackage) -> &MachineBody {
    &package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == "pick")
        .expect("pick")
        .body
}

fn pick_mut(package: &mut LoweredPackage) -> &mut MachineBody {
    &mut package
        .machines
        .iter_mut()
        .find(|machine| machine.declaration.path == "pick")
        .expect("pick")
        .body
}

/// The join block and, in edge order, the operand each jump into it supplies.
fn arrivals(body: &MachineBody) -> (BlockId, Vec<CallArgument>) {
    let join = body
        .blocks
        .iter()
        .find(|block| !block.structural_parameters.is_empty())
        .expect("a structural join")
        .id;
    let operands = body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Jump(edge) if edge.target == join => Some(edge.structural_arguments.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    (join, operands)
}

/// The operand on each jump into the join, in edge order, for corrupting a copy of the product.
fn edges(body: &mut MachineBody) -> Vec<&mut CallArgument> {
    let join = arrivals(body).0;
    body.blocks
        .iter_mut()
        .filter_map(|block| match &mut block.terminator {
            BlockTerminator::Jump(edge) if edge.target == join => edge.structural_arguments.first_mut(),
            _ => None,
        })
        .collect()
}

#[test]
fn an_exclusive_view_chosen_by_match_transfers_each_arriving_formal() {
    let mut package = lower(ORIGINAL);
    let body = pick(&package);
    let [
        Parameter::Scalar(_),
        Parameter::Structural(left),
        Parameter::Structural(right),
    ] = body.parameters[..]
    else {
        panic!("expected a flag and two exclusive views")
    };
    assert_ne!(left, right);
    let (join, operands) = arrivals(body);
    assert_eq!(operands, [CallArgument::MoveWhole(left), CallArgument::MoveWhole(right)]);
    let join = &body.blocks[join.get() - 1];
    let [joined] = join.structural_parameters[..] else {
        panic!("one joined view")
    };
    assert_eq!(body.places[joined.index()].origin, PlaceOrigin::BlockParameter(join.id));
    let ty = body.places[joined.index()].ty;
    assert_eq!(package.types.get(ty), Some(&TypeShape::MutableScalarElementView { element: U32 }));
    assert_eq!([body.places[left.index()].ty, body.places[right.index()].ty], [ty, ty]);
    assert_eq!(body.operations[join.operations.start].kind, OperationKind::ElementViewLength { source: joined });

    let expected = ExclusiveJoin {
        arrivals: [(true, 1), (false, 2)],
        descriptor: TypeShape::MutableScalarElementView { element: U32 },
        consumer: JoinedUse::Length,
        reused_after_transfer: vec![],
    };
    spec_corpus::observe_handoff!(psi "psi-output/exclusive-view-join", assert_eq!(exclusive_join(&package, "pick"), Ok(expected)));

    // The same bytes with the arms exchanged supply the other formal on each arrival.
    let swapped = ORIGINAL.replace("true -> left, false -> right", "true -> right, false -> left");
    assert_eq!(exclusive_join(&lower(&swapped), "pick").map(|j| j.arrivals), Ok([(true, 2), (false, 1)]));

    // Corrupted predecessors: exchanging the transferred loans changes what the reader recovers.
    for operand in edges(pick_mut(&mut package)) {
        *operand = match *operand {
            CallArgument::MoveWhole(p) if p == left => CallArgument::MoveWhole(right),
            CallArgument::MoveWhole(p) if p == right => CallArgument::MoveWhole(left),
            ref other => other.clone(),
        };
    }
    assert_eq!(exclusive_join(&package, "pick").map(|j| j.arrivals), Ok([(true, 2), (false, 1)]));
    // Copying an exclusive loan onto the edge would duplicate it.
    let mut package = lower(ORIGINAL);
    *edges(pick_mut(&mut package))[1] = CallArgument::CopyWhole(right);
    assert_eq!(exclusive_join(&package, "pick"), Err("an exclusive descriptor arrives by copy"));
    // A consumer that measures a formal instead of the join no longer follows the chosen view.
    let mut package = lower(ORIGINAL);
    let body = pick_mut(&mut package);
    let start = body.blocks[arrivals(body).0.get() - 1].operations.start;
    body.operations[start].kind = OperationKind::ElementViewLength { source: left };
    assert_eq!(exclusive_join(&package, "pick"), Err("the join's consumer must use the join parameter"));
}

#[test]
fn the_shared_control_still_copies_each_arriving_view() {
    let shared = include_str!("fixtures/package_cases/exclusive-view-match-result/control/case.omg");
    let package = lower(shared);
    let body = pick(&package);
    let [_, Parameter::Structural(left), Parameter::Structural(right)] = body.parameters[..] else {
        panic!("expected two shared views")
    };
    assert_eq!(arrivals(body).1, [CallArgument::CopyWhole(left), CallArgument::CopyWhole(right)]);
    assert_eq!(
        package.types.get(body.places[left.index()].ty),
        Some(&TypeShape::SharedElementView { element: U32 })
    );
    assert_eq!(exclusive_join(&package, "pick"), Err("an exclusive descriptor arrives by copy"));
}

#[test]
fn an_exclusive_reference_match_result_transfers_the_same_way() {
    let text = "data Cell { v: u32; } machine pick(flag: bool, left: @mut Cell, right: @mut Cell) -> u32 { let chosen = match flag {true -> left, false -> right,}; chosen.v }";
    let package = lower(text);
    let join = exclusive_join(&package, "pick").unwrap();
    assert_eq!(join.arrivals, [(true, 1), (false, 2)]);
    assert!(matches!(
        join.descriptor,
        TypeShape::NominalReference {
            access: ReferenceAccess::Mutable,
            ..
        }
    ));
    assert_eq!(join.consumer, JoinedUse::ReferentField(vec!["Cell::v".into()]));
    // The shared reference twin keeps copying its loan onto each edge.
    let shared = lower(&text.replace("@mut Cell", "@Cell"));
    assert_eq!(exclusive_join(&shared, "pick"), Err("an exclusive descriptor arrives by copy"));
}

#[test]
fn a_read_through_the_joined_view_measures_and_reads_the_join_parameter() {
    let text =
        "machine pick(flag: bool, left: @mut [u32], right: @mut [u32], i: u64) -> u32 { let chosen = match flag {true -> left, false -> right,}; chosen[i] }";
    let package = lower(text);
    let body = pick(&package);
    let [_, _, _, Parameter::Scalar(selector)] = body.parameters[..] else {
        panic!("expected an index formal")
    };
    let join = &body.blocks[arrivals(body).0.get() - 1];
    let [joined] = join.structural_parameters[..] else {
        panic!("one joined view")
    };
    let [measure, read] = &body.operations[join.operations.clone()] else {
        panic!("expected a length observation immediately before the read")
    };
    assert_eq!(measure.kind, OperationKind::ElementViewLength { source: joined });
    let OperationKind::ElementViewRead {
        source, index, length, path, ..
    } = &read.kind
    else {
        panic!("expected an element read")
    };
    assert_eq!((*source, *index, path.as_slice()), (joined, selector, &[][..]));
    assert_eq!(
        Some(*length),
        match measure.result {
            omega_lowering::OperationResult::Scalar(value) => Some(value),
            _ => None,
        }
    );
    assert_eq!(
        exclusive_join(&package, "pick").map(|j| (j.arrivals, j.consumer)),
        Ok(([(true, 1), (false, 2)], JoinedUse::Length))
    );
}

#[test]
fn both_arms_may_transfer_the_same_formal() {
    let text = "machine pick(flag: bool, left: @mut [u32]) -> u64 { let chosen = match flag {true -> left, false -> left,}; chosen.len }";
    assert_eq!(exclusive_join(&lower(text), "pick").map(|j| j.arrivals), Ok([(true, 1), (false, 1)]));
}

#[test]
fn a_transferred_formal_used_after_the_join_is_left_for_the_checker() {
    // On the true arrival `left` was transferred into `chosen`. Lowering keeps
    // the later read of `left` so the checker can refuse it on that arrival.
    let text = "machine pick(flag: bool, left: @mut [u32], right: @mut [u32]) -> u64 { let chosen = match flag {true -> left, false -> right,}; let n = chosen.len; n +% left.len }";
    let join = exclusive_join(&lower(text), "pick").unwrap();
    assert_eq!(join.arrivals, [(true, 1), (false, 2)]);
    assert_eq!(join.reused_after_transfer, [1]);
}

#[test]
fn binding_an_existing_exclusive_view_transfers_the_descriptor() {
    let original = "machine pick(left: @mut [u32]) -> u64 { let chosen = left; chosen.len }";
    let mut package = lower(original);
    let body = pick(&package);
    let [Parameter::Structural(formal)] = body.parameters[..] else {
        panic!("expected a descriptor formal")
    };
    let (source, destination, reads) = psi_handoff_consumer::local_descriptor_move(body).unwrap();
    spec_corpus::observe_handoff!(psi "psi-output/exclusive-local-transfer", assert_eq!((source, reads), (formal, vec![destination])));
    let body = pick_mut(&mut package);
    let movement = body
        .operations
        .iter_mut()
        .find(|op| matches!(op.kind, OperationKind::MoveStructural { .. }))
        .unwrap();
    let OperationKind::MoveStructural { source } = &movement.kind else {
        unreachable!()
    };
    movement.kind = OperationKind::CopyStructural { source: source.clone() };
    assert_eq!(psi_handoff_consumer::local_descriptor_move(body), Err("missing descriptor move"));
}

#[test]
fn naming_a_join_result_twice_keeps_the_transfer_and_the_later_use_separate() {
    let program = ORIGINAL.replace("chosen.len }", "let other = chosen; other.len + chosen.len }");
    let package = lower(&program);
    let body = pick(&package);
    let joined = body
        .blocks
        .iter()
        .find_map(|block| block.structural_parameters.first().copied())
        .unwrap();
    let (source, destination, reads) = psi_handoff_consumer::local_descriptor_move(body).unwrap();
    assert_eq!(source, joined);
    spec_corpus::observe_handoff!(psi "psi-output/exclusive-local-reuse", assert_eq!(reads, [destination, joined]));
    let single_name = lower(&ORIGINAL.replace("chosen.len }", "chosen.len + chosen.len }"));
    assert_eq!(psi_handoff_consumer::local_descriptor_move(pick(&single_name)), Err("missing descriptor move"));
}

#[test]
fn both_bindings_of_one_exclusive_record_reference_name_their_transfer_source() {
    let package = lower("data Cell { n:u32; } machine pick(l:@mut Cell)->u32 { let a=l; let b=l; a.n + b.n }");
    let body = pick(&package);
    let [Parameter::Structural(formal)] = body.parameters[..] else {
        panic!("expected the exclusive formal")
    };
    let moves: Vec<_> = body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::MoveStructural { source }, omega_lowering::OperationResult::Structural(destination)) => Some((source, destination)),
            _ => None,
        })
        .collect();
    let [(first, a), (second, b)] = moves.as_slice() else {
        panic!("expected both transfers")
    };
    assert_eq!((first.root, second.root), (formal, formal));
    assert!(first.path.is_empty() && second.path.is_empty());
    assert_ne!(a, b);
    assert_ne!(*a, formal);
    assert_ne!(*b, formal);
}

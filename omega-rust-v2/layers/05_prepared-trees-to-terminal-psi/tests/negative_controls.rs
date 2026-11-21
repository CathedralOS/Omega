//! In-suite product-corruption controls. Each control lowers the same source as its
//! positive test, confirms that the shared witness in `psi_handoff_consumer::product_witness`
//! accepts the actual Psi, applies one corruption that keeps the graph well formed, and
//! requires the same witness to reject the result. Weakening a witness therefore fails here.
//!
//! | Control | Positive test using the same witness |
//! | --- | --- |
//! | swapped branch targets | `loans::branch_and_direct_call_retain_the_actual_returned_input` |
//! | swapped runtime-dispatch arms | `runtime_callable::the_invocation_after_a_join_uses_the_arriving_descriptor` |
//! | swapped equal-typed scalars | `member_calls::boundary_demand_preserves_the_requirement_and_authored_argument_interleave` |
//! | restoration turned into replacement | `loans::exclusive_move_and_replacement_retain_both_exact_paths` |
//! | residual cleanup redirected or dropped | `ownership::transferring_a_field_to_a_call_does_not_dispose_the_argument_twice` |
//! | arm remainder disposed or arm transfer redirected before a join | `ownership::transferring_an_owned_match_result_uses_the_join_parameter` |
//! | premise operator flipped, premised store redirected or displacement kept | `view_loans::an_authored_selector_premise_chooses_the_store_the_checker_must_justify` |
//! | rank claim or back-edge actuals taken from the sibling predecessor | `ranking::each_back_edge_into_a_ranked_cycle_keeps_its_own_actuals_cleanup_and_claim` |
//! | back-edge cleanup dropped, or copied onto its guard edge | `ranking::each_back_edge_into_a_ranked_cycle_keeps_its_own_actuals_cleanup_and_claim` |
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, CallArgument, CleanupAction, LoweredPackage, MachineBody, OperationKind, StructuralStoreDisposition};
use psi_handoff_consumer::product_witness::{self, Witness};

/// Sources copied verbatim from the positive tests named above.
const BRANCH: &str = "data Cell{v:u8;} machine pick(a:@Cell,b:@Cell,c:bool)->@Cell {match c {true -> b,false -> a,}} machine relay(first:@Cell,second:@Cell,c:bool)->@Cell {pick(second,first,c)}";
/// `the_invocation_after_a_join_uses_the_arriving_descriptor` with its `alias(..)` calls expanded.
const DISPATCH: &str = "data Left = machine(x:u8)->u8 reaches none crashes none; data Right = machine(x:u8)->u8 reaches none crashes none; data Chosen = machine(x:u8)->u8 reaches none crashes none; machine storage(a:[u8;2])->[u8;2]{a} machine choose(flag:bool, left:Left, right:Right, n:u8)->u8 { let chosen:Chosen = match flag {false -> left, true -> right,}; chosen(n) }";
const BOUNDARY: &str = "boundary trait Device { machine read(n:u8, bytes:@[u8], last:u8)->u8 reaches Device crashes none; machine other(n:u8,bytes:@[u8],last:u8)->u8 reaches Device crashes none; } machine run(n:u8,b:@[u8],last:u8)->u8 reaches Device { Device::read(last,b,n) }";
const RESTORATION: &str = "data Cell {v:u8;} data Pair {left:Cell;right:Cell;} machine edit(p:@mut Pair) {let old=move p.left; p.left=move old;}";
const PAIR: &str = "data Token { value:u8; } data Pair { left:Token; right:Token; }";

fn body_mut<'a>(
    package: &'a mut LoweredPackage,
    name: &str,
) -> &'a mut MachineBody {
    &mut package
        .machines
        .iter_mut()
        .find(|m| m.declaration.path == name)
        .unwrap_or_else(|| panic!("missing machine {name}"))
        .body
}

fn body<'a>(
    package: &'a LoweredPackage,
    name: &str,
) -> &'a MachineBody {
    &package
        .machines
        .iter()
        .find(|m| m.declaration.path == name)
        .unwrap_or_else(|| panic!("missing machine {name}"))
        .body
}

/// The corruption must survive the structural graph check, so only the semantic witness can catch it.
fn rejects(
    label: &str,
    corrupted: &MachineBody,
    verdict: Witness,
) {
    sources::control_flow::check_operation_ownership(corrupted).unwrap_or_else(|e| panic!("{label}: corruption broke graph structure: {e}"));
    let Err(reason) = verdict else {
        panic!("{label}: the witness accepted a corrupted product")
    };
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_NEGATIVE_CONTROL\t{label}\trejected\t{}", reason.replace(['\r', '\n', '\t'], " "));
    }
}

#[test]
fn swapped_branch_targets_fail_the_returned_input_witness() {
    let mut package = sources::lower(&[("loan.omg", BRANCH)]).unwrap();
    product_witness::branch_returns_formals(body(&package, "pick"), 1, 0).expect("actual product");
    let pick = body_mut(&mut package, "pick");
    let BlockTerminator::Conditional { when_true, when_false, .. } = &mut pick.blocks[0].terminator else {
        panic!("two exits")
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    let pick = &*pick;
    rejects("branch-target/swapped", pick, product_witness::branch_returns_formals(pick, 1, 0));
}

/// Swapping the edges of the branch that selects the descriptor leaves every arm, the join
/// and the call unchanged, so each value keeps its type and the call keeps `Chosen`'s contract.
/// Only the witness's assignment of formals to edges sees that `flag` false now invokes `right`.
#[test]
fn swapped_dispatch_arms_fail_the_arriving_descriptor_witness() {
    let mut package = sources::lower(&[("runtime_callable.omg", DISPATCH)]).unwrap();
    let witness = |body: &MachineBody| product_witness::runtime_call_after_branch(body, 0, 2, 1, &[3]);
    witness(body(&package, "choose")).expect("actual product");
    let call = |body: &MachineBody| {
        body.operations
            .iter()
            .find_map(|op| match &op.kind {
                OperationKind::Call { callee, arguments, .. } => Some(format!("{callee:?} {arguments:?}")),
                _ => None,
            })
            .expect("runtime call")
    };
    let before = call(body(&package, "choose"));
    let choose = body_mut(&mut package, "choose");
    let BlockTerminator::Conditional { when_true, when_false, .. } = &mut choose.blocks[0].terminator else {
        panic!("flag selects the descriptor")
    };
    std::mem::swap(&mut when_true.target, &mut when_false.target);
    let choose = &*choose;
    assert_eq!(call(choose), before, "the corruption must leave the call and its contract unchanged");
    rejects("runtime-dispatch/swapped-arms", choose, witness(choose));
}

#[test]
fn swapped_equal_typed_boundary_scalars_fail_the_operand_witness() {
    let mut package = sources::lower(&[("device.omg", BOUNDARY)]).unwrap();
    let read = |package: &LoweredPackage| {
        let run = body(package, "run");
        let [op] = run.operations.as_slice() else {
            panic!("expected a single boundary call")
        };
        let (_, arguments) = psi_handoff_consumer::member_call(package, &op.kind).expect("lost boundary selection");
        product_witness::call_reads_formals(run, arguments, &[2, 1, 0])
    };
    read(&package).expect("actual product");
    let run = body_mut(&mut package, "run");
    let OperationKind::Call { arguments, .. } = &mut run.operations[0].kind else {
        panic!("boundary call")
    };
    let [CallArgument::Scalar(first), CallArgument::CopyWhole(_), CallArgument::Scalar(last)] = arguments.as_slice() else {
        panic!("scalar, view, scalar operands")
    };
    // Equal carriers: a type check cannot notice the swap, only the formal assignment can.
    assert_eq!(run.values[first.index()].ty, run.values[last.index()].ty);
    arguments.swap(0, 2);
    let verdict = read(&package);
    rejects("scalar-operand/swapped", body(&package, "run"), verdict);
}

#[test]
fn restoration_turned_into_replacement_fails_the_vacancy_witness() {
    for corruption in ["replace-unrestricted", "dispose-live"] {
        let mut package = sources::lower(&[("loan.omg", RESTORATION)]).unwrap();
        let edit = body_mut(&mut package, "edit");
        product_witness::restores_moved_place(edit).expect("actual product");
        let (destination, value, disposition) = edit
            .operations
            .iter_mut()
            .find_map(|op| match &mut op.kind {
                OperationKind::StructuralStore {
                    destination,
                    value,
                    disposition,
                } => Some((destination.clone(), *value, disposition)),
                _ => None,
            })
            .expect("restoring store");
        *disposition = match corruption {
            "replace-unrestricted" => StructuralStoreDisposition::ReplaceUnrestricted,
            _ => StructuralStoreDisposition::Dispose(vec![CleanupAction::DiscardResidual {
                place: destination,
                ty: edit.places[value.index()].ty,
            }]),
        };
        let (edit, label) = (&*edit, format!("restoration/{corruption}"));
        rejects(&format!("{label}/vacancy"), edit, product_witness::vacancy_refill(edit).map(|_| ()));
        rejects(&format!("{label}/moved-place"), edit, product_witness::restores_moved_place(edit));
    }
}

#[test]
fn residual_cleanup_on_the_transferred_field_fails_the_sibling_witness() {
    for (field, sibling) in [("right", "left"), ("left", "right")] {
        let source = format!("{PAIR} machine take(t:Token) {{}} machine apply(p:Pair){{take(move p.{field});}}");
        let (field, sibling) = (format!("Pair::{field}"), format!("Pair::{sibling}"));
        for corruption in ["transferred-field", "whole-root", "omitted"] {
            let mut package = sources::lower(&[("call.omg", &source)]).unwrap();
            let apply = body_mut(&mut package, "apply");
            product_witness::transfer_leaves_sibling(apply, &field, &sibling).expect("actual product");
            let OperationKind::MoveStructural { source: transferred } = &apply.operations[0].kind else {
                panic!("field transfer")
            };
            let transferred = transferred.clone();
            let BlockTerminator::Return { cleanup, .. } = &mut apply.blocks[0].terminator else {
                panic!("return expected")
            };
            match corruption {
                "transferred-field" => {
                    let [CleanupAction::DiscardResidual { place, .. }] = cleanup.as_mut_slice() else {
                        panic!("one residual")
                    };
                    place.path = transferred.path;
                }
                "whole-root" => *cleanup = vec![CleanupAction::DiscardRoot(transferred.root)],
                _ => cleanup.clear(),
            }
            let apply = &*apply;
            rejects(
                &format!("residual/{field}/{corruption}"),
                apply,
                product_witness::transfer_leaves_sibling(apply, &field, &sibling),
            );
        }
    }
}

/// The checker compares what each arm leaves of `p` at the join. A product that
/// quietly disposes an arm's remainder before arriving, or that moves the same
/// field in both arms, would hide that disagreement, so the arrival witness must reject both.
#[test]
fn a_normalized_or_redirected_arm_fails_the_join_arrival_witness() {
    let source = format!("{PAIR} machine select(p:Pair,flag:bool)->Token{{let chosen=match flag{{true->move p.left,false->move p.right,}};chosen}}");
    for corruption in ["disposed-remainder", "same-field"] {
        let mut package = sources::lower(&[("join.omg", &source)]).unwrap();
        let select = body_mut(&mut package, "select");
        product_witness::join_arrivals_move_fields(select, "Pair::left", "Pair::right").expect("actual product");
        let BlockTerminator::Conditional { when_true, when_false, .. } = &select.blocks[0].terminator else {
            panic!("entry branch")
        };
        let (true_arm, false_arm) = (when_true.target.get() - 1, when_false.target.get() - 1);
        // Each arm's own transfer: its operation ordinal, moved place and claim type.
        let transfer = |body: &MachineBody, block: usize| {
            body.operations[body.blocks[block].operations.clone()]
                .iter()
                .find_map(|op| match (&op.kind, op.result) {
                    (OperationKind::MoveStructural { source }, omega_lowering::OperationResult::Structural(claim)) => {
                        Some((op.id.index(), source.clone(), body.places[claim.index()].ty))
                    }
                    _ => None,
                })
                .expect("arm transfer")
        };
        let (_, left, _) = transfer(select, true_arm);
        let (false_move, right, ty) = transfer(select, false_arm);
        match corruption {
            // Dispose `p.right`, the true arm's remainder, on its arriving edge.
            "disposed-remainder" => {
                let BlockTerminator::Jump(arrival) = &mut select.blocks[true_arm].terminator else {
                    panic!("true arm jumps to the join")
                };
                arrival.cleanup.push(CleanupAction::DiscardResidual { place: right, ty });
            }
            // Make the false arm move `p.left` as the true arm does.
            _ => {
                let OperationKind::MoveStructural { source } = &mut select.operations[false_move].kind else {
                    panic!("false arm transfer")
                };
                *source = left;
            }
        }
        let select = &*select;
        rejects(
            &format!("join-arrival/{corruption}"),
            select,
            product_witness::join_arrivals_move_fields(select, "Pair::left", "Pair::right"),
        );
    }
}

#[test]
fn a_premised_element_store_fails_its_witness_without_the_clause_or_its_selected_element() {
    // Sources copied verbatim from `view_loans::an_authored_selector_premise_chooses_the_store_the_checker_must_justify`.
    const EQUAL: &str = "data Cell{v:u8;} machine edit(p:@mut [Cell],c:Cell,i:u64,j:u64) requires i == j {let old=move p[i]; p[j]=move old;}";
    const DISTINCT: &str = "data Cell{v:u8;} machine edit(p:@mut [Cell],c:Cell,i:u64,j:u64) requires i != j {let old=move p[i]; p[j]=move c; p[i]=move old;}";
    use omega_lowering::{OperationResult, Parameter, PlaceProjection, PredicateOperator, PredicateStep};
    let fill = |m: &omega_lowering::LoweredMachine| product_witness::premised_vacancy_fill(m, 0, 2, 3);
    let displace = |m: &omega_lowering::LoweredMachine| product_witness::premised_sibling_displacement(m, 0, 2, 3);
    for (source, witness, flipped, corruption) in [
        (EQUAL, &fill as &dyn Fn(&omega_lowering::LoweredMachine) -> Witness, PredicateOperator::NotEqual, "clause-operator"),
        (EQUAL, &fill, PredicateOperator::NotEqual, "store-through-vacated-selector"),
        (DISTINCT, &displace, PredicateOperator::Equal, "clause-operator"),
        (DISTINCT, &displace, PredicateOperator::Equal, "displaced-element-kept"),
    ] {
        let mut package = sources::lower(&[("view.omg", source)]).unwrap();
        let machine = &mut package.machines[0];
        witness(machine).expect("actual product");
        match corruption {
            // The same body under the opposite premise.
            "clause-operator" => machine.contract.requires[0].predicate[2] = PredicateStep::Apply(flipped),
            // The store names the vacancy's own formal: congruent, so no premise would be needed.
            "store-through-vacated-selector" => {
                let Parameter::Scalar(i) = machine.body.parameters[2] else {
                    panic!("formal i is a scalar")
                };
                let store = machine
                    .body
                    .operations
                    .iter_mut()
                    .find_map(|op| match &mut op.kind {
                        OperationKind::StructuralStore { destination, .. } => Some(destination),
                        _ => None,
                    })
                    .expect("the premised store");
                let Some(PlaceProjection::ViewElement { index, .. }) = store.path.last_mut() else {
                    panic!("the store selects one view element")
                };
                index.selector = i;
            }
            // The displaced sibling silently survives.
            _ => {
                let displaced = machine
                    .body
                    .operations
                    .iter()
                    .filter_map(|op| match (&op.kind, op.result) {
                        (OperationKind::BorrowedMove { .. }, OperationResult::Structural(place)) => Some(place),
                        _ => None,
                    })
                    .nth(1)
                    .expect("the displacement");
                for block in &mut machine.body.blocks {
                    if let BlockTerminator::Jump(edge) = &mut block.terminator {
                        edge.cleanup.retain(|action| *action != CleanupAction::DiscardRoot(displaced));
                    }
                }
            }
        }
        let label = format!("premised-selector/{}/{corruption}", if source == EQUAL { "equal" } else { "distinct" });
        rejects(&label, &machine.body, witness(machine));
    }
}

/// Copied verbatim from `ranking::RETURNING_CYCLE`: two back edges enter `count`
/// with different actuals, and each completion of the activation disposes `lease`.
const CYCLE: &str = r#"use omega\language\core;
data Lease dropped by finish {mark:u8;}
machine finish(l:Lease) {let Lease{mark as _}=l;}
machine count(n:u64, m:u64, d:u64)->u64 crashes Trap d == 0 terminates by n {
    let lease = Lease{mark:0};
    transition {
        n == 0 -> n /! d,
        n < m -> count(n - 1, m, d),
        _ -> count(m - 1, n, d),
    }
}
"#;

fn machine_id(package: &LoweredPackage, name: &str) -> omega_lowering::MachineId {
    package.machines.iter().find(|m| m.declaration.path == name).unwrap_or_else(|| panic!("missing machine {name}")).id
}

/// Every edge stays claimed once; only which predecessor's actuals a claim uses changes.
#[test]
fn a_claim_or_actual_from_the_sibling_back_edge_fails_the_cycle_witness() {
    for corruption in ["sibling-successor", "sibling-actuals"] {
        let mut package = sources::lower(&[("cycle.omg", CYCLE)]).unwrap();
        let count = machine_id(&package, "count");
        product_witness::cycle_claims(&package, count).expect("actual product");
        let machine = package.machines.iter_mut().find(|m| m.id == count).unwrap();
        if corruption == "sibling-successor" {
            let [first, second] = machine.contract.ranking.as_mut().unwrap().edges.as_mut_slice() else {
                panic!("two back-edge claims")
            };
            std::mem::swap(&mut first.successor, &mut second.successor);
        } else {
            let [first, second]: [&mut Vec<CallArgument>; 2] = machine
                .body
                .blocks
                .iter_mut()
                .filter_map(|block| match &mut block.terminator {
                    BlockTerminator::TerminalCall { arguments, .. } => Some(arguments),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("two back edges");
            std::mem::swap(first, second);
        }
        let package = &package;
        let body = &package.machines.iter().find(|m| m.id == count).unwrap().body;
        rejects(&format!("cycle-claim/{corruption}"), body, product_witness::cycle_claims(package, count));
    }
}

/// The `n < m` back edge loses its cleanup, or its guard edge also disposes `lease`,
/// so that path disposes it twice. Only the guard-edge rule rejects the second.
#[test]
fn a_back_edge_cleanup_dropped_or_copied_to_its_guard_fails_the_completion_witness() {
    for corruption in ["omitted", "guard-edge"] {
        let mut package = sources::lower(&[("cycle.omg", CYCLE)]).unwrap();
        let finish = machine_id(&package, "finish");
        let count = body_mut(&mut package, "count");
        let lease = count
            .operations
            .iter()
            .find_map(|op| match (&op.kind, op.result) {
                (OperationKind::EstablishRecord { .. }, omega_lowering::OperationResult::Structural(place)) => Some(place),
                _ => None,
            })
            .expect("the Lease local");
        product_witness::completion_cleanup(count, lease, finish).expect("actual product");
        let back_edge = count
            .blocks
            .iter()
            .position(|block| matches!(block.terminator, BlockTerminator::TerminalCall { .. }))
            .expect("the first back edge");
        let BlockTerminator::TerminalCall { cleanup, .. } = &mut count.blocks[back_edge].terminator else {
            unreachable!()
        };
        let actions = if corruption == "omitted" { std::mem::take(cleanup) } else { cleanup.clone() };
        if corruption == "guard-edge" {
            let target = count.blocks[back_edge].id;
            let guard = count
                .blocks
                .iter_mut()
                .find_map(|block| match &mut block.terminator {
                    BlockTerminator::Conditional { when_true, .. } if when_true.target == target => Some(when_true),
                    _ => None,
                })
                .expect("the n < m guard edge");
            guard.cleanup = actions;
        }
        let count = &*count;
        rejects(&format!("cycle-cleanup/{corruption}"), count, product_witness::completion_cleanup(count, lease, finish).map(drop));
    }
}

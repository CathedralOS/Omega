//! In-suite product-corruption controls. Each control lowers the same source as its
//! positive test, confirms that the shared witness in `psi_handoff_consumer::product_witness`
//! accepts the actual Psi, applies one corruption that keeps the graph well formed, and
//! requires the same witness to reject the result. Weakening a witness therefore fails here.
//!
//! | Control | Positive test using the same witness |
//! | --- | --- |
//! | swapped branch targets | `loans::branch_and_direct_call_retain_the_actual_returned_input` |
//! | swapped equal-typed scalars | `member_calls::boundary_demand_preserves_the_requirement_and_authored_argument_interleave` |
//! | restoration turned into replacement | `loans::exclusive_move_and_replacement_retain_both_exact_paths` |
//! | residual cleanup redirected or dropped | `ownership::transferring_a_field_to_a_call_does_not_dispose_the_argument_twice` |
//! | arm remainder disposed or arm transfer redirected before a join | `ownership::transferring_an_owned_match_result_uses_the_join_parameter` |
#[path = "support/source.rs"]
mod sources;
use omega_lowering::{BlockTerminator, CallArgument, CleanupAction, LoweredPackage, MachineBody, OperationKind, StructuralStoreDisposition};
use psi_handoff_consumer::product_witness::{self, Witness};

/// Sources copied verbatim from the positive tests named above.
const BRANCH: &str = "data Cell{v:u8;} machine pick(a:@Cell,b:@Cell,c:bool)->@Cell {match c {true -> b,false -> a,}} machine relay(first:@Cell,second:@Cell,c:bool)->@Cell {pick(second,first,c)}";
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

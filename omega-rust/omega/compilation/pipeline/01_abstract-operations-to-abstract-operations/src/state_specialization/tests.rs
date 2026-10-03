//! Optimizer module role: test leaf. Constant state-argument specialization admission, realization, and custody evidence through the rule.
//!
//! Every fixture is driven through the one live route: the
//! `StateSpecialization` pass runs `StateArgumentSpecializationRule` to its
//! fixed point, the committed candidate carries the proposed plan, the run's
//! session carries the fused unit, and forged rows are replayed against
//! `validate_state_argument_specialization_candidate` — the independent
//! validator the pass manager itself consults.

use crate::rules::StateArgumentSpecializationRule;
use crate::{
    AnalysisProduct, OptimizationRun, PsiOptimizationCommit, VerifiedPsiOptimizationSession,
    compute_analysis, run_psi_pipeline,
};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_core::{
    AnalysisKind, Optimization, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget,
};
use optimization_unit::{
    NodeLocation, OptimizationEdge, PsiOptimizationUnit, PsiProvenance, PsiRewriteCandidate,
    PsiRewriteCandidateError, PsiRewritePatch, StateArgumentSpecializationRewrite,
    recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::{
    OptimizationUnitValidationError, validate_state_argument_specialization_candidate,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ValueId};
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

/// One shared dispatch state `choose` entered by an unconditional jump that
/// binds a literal `true` and by the entry conditional's unfused arm that binds
/// the still-variable machine parameter. Only the literal edge specializes.
const SINGLE_EDGE_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> warm(mode)
            _ -> choose(flag, mode)
        }
        state warm(m: u32 in Wrapping) {
            let on: bool = true;
            transition { _ -> choose(on, m) }
        }
        state choose(f: bool, m: u32 in Wrapping) {
            transition f {
                true -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// One shared dispatch state `choose` whose literal-supplied incoming edge is
/// one arm of a `Conditional` predecessor rather than an unconditional jump:
/// `relay`'s `g` parameter is proven `true` by `warm`'s only binding of it,
/// so `relay`'s `true` arm — a conditional successor edge that binds `f`
/// directly to the parameter `g` — specializes while its sibling arm keeps
/// routing to `right`. `variable`'s jump keeps a still-variable path into the
/// dispatch, so exactly the conditional arm fuses.
const CONDITIONAL_EDGE_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> warm(mode)
            _ -> variable(flag, mode)
        }
        state warm(m: u32 in Wrapping) {
            let on: bool = true;
            transition { _ -> relay(on, m) }
        }
        state relay(g: bool, m: u32 in Wrapping) {
            transition g {
                true -> choose(g, m)
                _ -> right(m)
            }
        }
        state variable(f: bool, m: u32 in Wrapping) {
            transition { _ -> choose(f, m) }
        }
        state choose(f: bool, m: u32 in Wrapping) {
            transition f {
                true -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// Both arms of one `Conditional` predecessor supply different literals to the
/// same dispatch state — `relay`'s `g` and `gg` parameters are proven `true`
/// and `false` by `warm`'s only bindings — while `variable`'s jump keeps the
/// dispatch reachable: both arms specialize atomically in one candidate
/// through a single node reconstruction.
const BOTH_CONDITIONAL_ARMS_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> warm(mode)
            _ -> variable(flag, mode)
        }
        state warm(m: u32 in Wrapping) {
            let on: bool = true;
            let off: bool = false;
            transition { _ -> relay(on, off, m) }
        }
        state relay(g: bool, gg: bool, m: u32 in Wrapping) {
            transition g {
                true -> choose(g, m)
                _ -> choose(gg, m)
            }
        }
        state variable(f: bool, m: u32 in Wrapping) {
            transition { _ -> choose(f, m) }
        }
        state choose(f: bool, m: u32 in Wrapping) {
            transition f {
                true -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

#[test]
fn both_arms_of_one_conditional_specialize_together() {
    let unit = lowered_unit(BOTH_CONDITIONAL_ARMS_SOURCE, "both-arms specialization");
    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    let [first, second] = patch.edges.as_slice() else {
        panic!("two specialized incoming edges")
    };
    // Both rows name the same predecessor conditional site but different arm
    // edges; they fold into a single node reconstruction, not two overwrites.
    assert_eq!(first.predecessor, second.predecessor);
    assert_ne!(first.incoming_edge, second.incoming_edge);
    assert_ne!(first.constant, second.constant);
    assert_ne!(first.resolved_target, second.resolved_target);

    let function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .expect("machine retained");
    let predecessor = first.predecessor;
    let output_node = &function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .expect("predecessor block retained")
        .nodes[usize::try_from(predecessor.node).expect("node index")];
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &output_node.operation
    else {
        panic!("the predecessor keeps its conditional shape")
    };
    let fused_true = output_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == when_true.psi_edge)
        .expect("when_true edge exists");
    let fused_false = output_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == when_false.psi_edge)
        .expect("when_false edge exists");
    let rows = [first, second];
    for (arm, edge) in [(when_true, fused_true), (when_false, fused_false)] {
        let row = rows
            .iter()
            .find(|row| row.incoming_edge == arm.psi_edge)
            .expect("every arm fused");
        assert_eq!(arm.target, row.resolved_target);
        assert_eq!(edge.target, row.resolved_target);
    }
    // Only the entry's unfused arm still enters the dispatch.
    assert_eq!(
        incoming_edge_count(function, patch.dispatch),
        1,
        "only the variable edge still enters"
    );
    let [record] = run.transformation_ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.provenance, commit.provenance);
    assert_eq!(
        record.provenance.len(),
        6,
        "three custody rows per fused edge"
    );
}

#[test]
fn replay_rejects_forged_conditional_arm_rows() {
    let unit = lowered_unit(CONDITIONAL_EDGE_SOURCE, "conditional-arm specialization");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, patch) = single_commit(&run);
    let row = patch.edges[0];
    let predecessor = edge_owner(&input, patch.machine, row.incoming_edge);
    let predecessor_node = &input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .and_then(|function| {
            function
                .blocks
                .iter()
                .find(|block| block.id == predecessor.block)
        })
        .expect("predecessor block exists")
        .nodes[usize::try_from(predecessor.node).expect("node index")];
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &predecessor_node.operation
    else {
        panic!("the fused predecessor site is a conditional")
    };
    let sibling = if when_true.psi_edge == row.incoming_edge {
        when_false
    } else {
        when_true
    };

    // A forged supplying edge naming the conditional's sibling arm — the arm
    // that does not enter the dispatch at all.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].incoming_edge = sibling.psi_edge;
            },
        ),
    );

    // A forged predecessor coordinate.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].predecessor.node += 1;
            },
        ),
    );

    // A forged resolved arm edge.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].taken_edge = patch.edges[0].rejected_edge;
            },
        ),
    );

    // A duplicated row — the same arm claimed twice — cannot validate: the
    // replayed plan carries each admissible edge exactly once.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges.push(row);
            },
        ),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_state_argument_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let unit = lowered_unit(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A candidate pinned to a revision that is not the input.
    let stale = rebuild(
        commit.declaration.rewrite().expect("rewrite declaration"),
        OptimizationUnitIdentity::from_canonical_bytes(b"stale-input"),
        |_| {},
    )
    .expect("a stale input identity is still a well-formed declaration");
    assert_eq!(
        validate_state_argument_specialization_candidate(&input, &stale).err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );

    // Committing moves the revision; the original declaration is stale
    // against the transformed unit afterward.
    assert_eq!(
        validate_state_argument_specialization_candidate(
            run.session().unit(),
            commit.declaration.rewrite().expect("rewrite declaration")
        )
        .err(),
        Some(OptimizationUnitValidationError::CandidateInputMismatch)
    );
}

/// The committed unit revalidates independently, while a unit whose fused
/// edge drops the resolved arm's fuel settlement — settling fewer sources
/// than the custody it names — is refused by transformed validation. A
/// forged commit output identity is refused by publication replay in
/// `pass_manager::tests::evidence_matrix::state_specialization::forged_run_axes_fail_publication_replay`.
#[test]
fn transformed_unit_rejects_forged_fused_edge_custody() {
    let unit = lowered_unit(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let row = &patch.edges[0];
    let (incoming_edge, taken_edge, machine) = (row.incoming_edge, row.taken_edge, patch.machine);
    let verified_input = run.session().input().clone();

    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            verified_input.clone(),
            run.session().unit().clone(),
        )
        .is_ok(),
        "the committed fused revision revalidates independently"
    );

    let mut malformed = run.session().unit().clone();
    let fused = malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .flat_map(|node| &mut node.successors)
        .find(|edge| edge.psi_edge == incoming_edge)
        .expect("fused edge exists");
    assert_eq!(
        fused.provenance,
        vec![
            PsiProvenance::Edge(incoming_edge),
            PsiProvenance::Edge(taken_edge),
        ]
    );
    fused.fuel.pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));
}

/// An integer literal-match dispatch: `choose` reads its own u64 parameter
/// `i` through an in-block `i == 0` comparison — `transition i { 0 -> left,
/// _ -> right }` lowers to `[IntegerConstant, IntegerEqual, Conditional]`
/// inside one block. `warm` binds `i` to a proven `0`, so its jump edge
/// resolves the `when_true` arm; the entry's `_` arm keeps the still-variable
/// machine parameter and the dispatch reachable.
const INT_MATCH_TAKEN_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> choose(idx, mode)
        }
        state warm(m: u32 in Wrapping) {
            let z: u64 = 0;
            transition { _ -> choose(z, m) }
        }
        state choose(i: u64, m: u32 in Wrapping) {
            transition i {
                0 -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// Both jump edges entering the integer dispatch supply proven integers
/// (`0` and `5`), so fusing every incoming edge would orphan the dispatch
/// state; the family declines the site entirely.
const INT_MATCH_ALL_CONSTANT_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> chill(mode)
        }
        state warm(m: u32 in Wrapping) {
            let z: u64 = 0;
            transition { _ -> choose(z, m) }
        }
        state chill(m: u32 in Wrapping) {
            let z: u64 = 5;
            transition { _ -> choose(z, m) }
        }
        state choose(i: u64, m: u32 in Wrapping) {
            transition i {
                0 -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// The literal-on-left operand order: `choose` reads `i` through `4 <= i`,
/// lowering to `[IntegerConstant(4), IntegerLessOrEqual(4, i), Conditional]`.
/// `warm` binds `i` to a proven `3`, so `4 <= 3` fails and the edge resolves
/// the `when_false` arm — operand order is honored by the comparison's own
/// `IntegerType::compare`, not by assuming the parameter sits on the left.
const INT_LITERAL_LEFT_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> choose(idx, mode)
        }
        state warm(m: u32 in Wrapping) {
            let z: u64 = 3;
            transition { _ -> choose(z, m) }
        }
        state choose(i: u64, m: u32 in Wrapping) {
            transition 4 <= i { true -> left(m) _ -> right(m) }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

#[test]
fn integer_literal_left_less_or_equal_specializes() {
    let unit = lowered_unit(INT_LITERAL_LEFT_SOURCE, "literal-left specialization");
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (dispatch, parameter) = integer_dispatch(&input, machine).expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&input, machine, dispatch).expect("unconditional incoming edge");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.edges.as_slice() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge, incoming_edge.psi_edge);
    assert_eq!(row.parameter, parameter);
    // `4 <= 3` fails, so the when_false arm is taken — the parameter sat on
    // the comparison's right and the literal bound on its left.
    assert!(!row.constant);
    let (taken_edge, rejected_edge, _) = dispatch_arms(&input, machine, dispatch, row.constant);
    assert_eq!(row.taken_edge, taken_edge.psi_edge);
    assert_eq!(row.rejected_edge, rejected_edge.psi_edge);

    let predecessor = edge_owner(&input, machine, incoming_edge.psi_edge);
    let output_function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let fused = fused_jump_edge(output_function, predecessor);
    assert_eq!(fused.target, taken_edge.target);
}

/// An unsupported condition shape: `i != 4` lowers to
/// `[IntegerConstant, IntegerEqual, BooleanNot, Conditional]` — the
/// comparison does not produce the condition (the `BooleanNot` sits between),
/// so the block is not a `parameter CMP literal` dispatch and the site
/// declines even though a proven literal flows in.
const INT_NOT_EQUAL_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> choose(idx, mode)
        }
        state warm(m: u32 in Wrapping) {
            let z: u64 = 3;
            transition { _ -> choose(z, m) }
        }
        state choose(i: u64, m: u32 in Wrapping) {
            transition i != 4 { true -> left(m) _ -> right(m) }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

#[test]
fn integer_not_equal_dispatch_declines() {
    // The condition is a BooleanNot of the comparison, not the comparison
    // itself.
    assert_declines(lowered_unit(INT_NOT_EQUAL_SOURCE, "not-equal decline"));
}

#[test]
fn all_constant_integer_edges_decline_to_orphan_the_dispatch() {
    let unit = lowered_unit(
        INT_MATCH_ALL_CONSTANT_SOURCE,
        "integer all-constant decline",
    );
    let input = unit.unit().clone();
    let machine = input.functions[0].machine;
    let (dispatch, _) = integer_dispatch(&input, machine).expect("integer dispatch exists");
    // Fusing every constant-supplied incoming edge would orphan the dispatch
    // state, so the plan for the site is reported with no edges.
    assert_orphaning_plan_is_empty(&input, machine, dispatch);
    assert_declines(unit);
}

#[test]
fn replay_rejects_forged_integer_arm_rows() {
    let unit = lowered_unit(INT_MATCH_TAKEN_SOURCE, "integer taken specialization");
    let input = unit.unit().clone();
    let run = specialize(unit);
    let (commit, _) = single_commit(&run);

    // A forged constant verdict — `i := 0` really does satisfy `i == 0`, so
    // flipping the resolved arm cannot replay.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].constant = !patch.edges[0].constant;
            },
        ),
    );

    // A forged resolved arm edge.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].taken_edge = patch.edges[0].rejected_edge;
            },
        ),
    );

    // A forged supplying edge.
    assert_rejects_rows(
        &input,
        forged(
            commit.declaration.rewrite().expect("rewrite declaration"),
            |patch| {
                patch.edges[0].incoming_edge = patch.edges[0].rejected_edge;
            },
        ),
    );

    // The untampered declaration still validates to the committed output.
    let validated = validate_state_argument_specialization_candidate(
        &input,
        commit.declaration.rewrite().expect("rewrite declaration"),
    )
    .expect("the exact candidate still validates");
    assert_eq!(validated.unit().identity, commit.output);
}

/// The integer member of the result specialization: `warm` binds `i` to the
/// `u64` result of `Root::size()` — a constant-result callee — and the
/// dispatch reads `i` through `i < 4`. The proven `3` satisfies the bound,
/// so `warm`'s edge resolves the `when_true` arm.
const CALL_RESULT_INTEGER_SOURCE: &str = r#"
    data Root {}

    machine Root::size() -> u64 { 3 }

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> choose(idx, mode)
        }
        state warm(m: u32 in Wrapping) {
            transition { _ -> choose(Root::size(), m) }
        }
        state choose(i: u64, m: u32 in Wrapping) {
            transition i < 4 { true -> left(m) _ -> right(m) }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// A callee whose result is not a single proven constant: `pick` carries two
/// `Return` nodes across its `yes`/`no` states, so its result is not one
/// exact constant and `warm`'s call-result edge cannot specialize.
const CALL_RESULT_MULTI_RETURN_SOURCE: &str = r#"
    data Root {}

    machine Root::pick(b: bool) -> bool {
        transition b {
            true -> yes()
            _ -> no()
        }
        state yes() -> bool { true }
        state no() -> bool { false }
    }

    machine Root::run(mode: u32 in Wrapping, sel: bool)
    {
        transition sel {
            true -> warm(mode, sel)
            _ -> choose(sel, mode)
        }
        state warm(m: u32 in Wrapping, b: bool) {
            transition { _ -> choose(Root::pick(b), m) }
        }
        state choose(f: bool, m: u32 in Wrapping) {
            transition f {
                true -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// A callee whose single `Return` carries a still-variable parameter: `echo`
/// returns exactly what it is given, so its result is not a proven constant
/// and `warm`'s call-result edge cannot specialize.
const CALL_RESULT_VARIABLE_SOURCE: &str = r#"
    data Root {}

    machine Root::echo(b: bool) -> bool { b }

    machine Root::run(mode: u32 in Wrapping, sel: bool)
    {
        transition sel {
            true -> warm(mode, sel)
            _ -> choose(sel, mode)
        }
        state warm(m: u32 in Wrapping, b: bool) {
            transition { _ -> choose(Root::echo(b), m) }
        }
        state choose(f: bool, m: u32 in Wrapping) {
            transition f {
                true -> left(m)
                _ -> right(m)
            }
        }
        state left(x: u32 in Wrapping) {}
        state right(x: u32 in Wrapping) {}
    }
"#;

/// The result specialization inside a machine carrying an authenticated
/// cyclic component: `warm`'s call-result edge would fuse the proven `true`
/// result to the `when_true` arm, but `spin`'s self-recursion freezes the
/// whole machine byte-exact — the constant-result callee `flag` stays
/// acyclic and unaffected.
const CALL_RESULT_CYCLIC_SOURCE: &str = r#"
    data Root {}

    machine Root::flag() -> bool { true }

    machine Root::spin(seed: bool, mode: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition seed {
            true -> warm(mode, remaining)
            _ -> choose(seed, mode, remaining)
        }
        state warm(m: u32 in Wrapping, r: u32 [0..=5]) {
            transition { _ -> choose(Root::flag(), m, r) }
        }
        state choose(f: bool, m: u32 in Wrapping, r: u32 [0..=5]) {
            transition f {
                true -> again(f, m, r)
                _ -> right(m)
            }
        }
        state again(go: bool, s: u32 in Wrapping, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> spin(go, s, pending - 1)
                _ -> right(s)
            }
        }
        state right(x: u32 in Wrapping) {}
    }
"#;

#[test]
fn integer_call_result_specializes_the_comparison_dispatch() {
    let unit = lowered_unit(
        CALL_RESULT_INTEGER_SOURCE,
        "integer call-result specialization",
    );
    let input = unit.unit().clone();
    let (machine, dispatch, parameter) = input
        .functions
        .iter()
        .find_map(|function| {
            integer_dispatch(&input, function.machine)
                .map(|(dispatch, parameter)| (function.machine, dispatch, parameter))
        })
        .expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&input, machine, dispatch).expect("unconditional incoming edge");

    let run = specialize(unit);
    let (_, patch) = single_commit(&run);
    let [row] = patch.edges.as_slice() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge, incoming_edge.psi_edge);
    assert_eq!(row.parameter, parameter);
    // `n := 3` satisfies `i < 4`, so the when_true arm is taken.
    assert!(row.constant);
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&input, machine, dispatch, row.constant);
    assert_eq!(row.taken_edge, taken_edge.psi_edge);
    assert_eq!(row.rejected_edge, rejected_edge.psi_edge);
    assert_eq!(row.resolved_target, resolved_target);

    let predecessor = edge_owner(&input, machine, incoming_edge.psi_edge);
    let output_function = run
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let fused = fused_jump_edge(output_function, predecessor);
    assert_eq!(fused.target, taken_edge.target);
    assert_eq!(
        fused.provenance,
        vec![
            PsiProvenance::Edge(incoming_edge.psi_edge),
            PsiProvenance::Edge(taken_edge.psi_edge),
        ]
    );
}

#[test]
fn multi_return_callee_result_yields_no_candidate() {
    // A callee with two Return nodes is not one exact constant result.
    assert_declines(lowered_unit(
        CALL_RESULT_MULTI_RETURN_SOURCE,
        "multi-return callee decline",
    ));
}

#[test]
fn variable_callee_result_yields_no_candidate() {
    // A callee returning its own parameter is not a proven constant result.
    assert_declines(lowered_unit(
        CALL_RESULT_VARIABLE_SOURCE,
        "variable callee result decline",
    ));
}

#[test]
fn call_result_specialization_stays_frozen_in_cyclic_machines() {
    let unit = lowered_unit_entry(
        CALL_RESULT_CYCLIC_SOURCE,
        "cyclic call-result decline",
        "Root::spin",
    );
    let input = unit.unit().clone();
    let session =
        VerifiedPsiOptimizationSession::new(unit.clone()).expect("the cyclic fixture re-admits");
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    let (machine, dispatch, parameter) = input
        .functions
        .iter()
        .find_map(|function| {
            parameter_dispatch(&input, function.machine)
                .map(|(dispatch, parameter)| (function.machine, dispatch, parameter))
        })
        .expect("spin's dispatch state exists");
    // The call-result edge inside frozen territory does not specialize, and
    // the validator refuses it independently of the rule's skip: `flag`'s
    // proven result is `true`.
    assert_frozen_dispatch_is_refused(&input, &[machine], machine, dispatch, parameter, true);
    assert_declines(unit);
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, 64).expect("budget")
}

fn selections() -> OptimizationSelections {
    OptimizationSelections::new([Optimization::StateSpecialization])
        .expect("state-specialization selection")
}

/// Runs the state-specialization pass — this family's one rule — to its
/// fixed point through the public pipeline entrance.
fn specialize(unit: VerifiedPsiOptimizationUnit) -> OptimizationRun {
    run_psi_pipeline(unit, &selections(), budget()).expect("the selected pass runs")
}

/// The run's single commit — the pass reached its fixed point after one
/// candidate — with its state-argument patch.
fn single_commit(
    run: &OptimizationRun,
) -> (&PsiOptimizationCommit, StateArgumentSpecializationRewrite) {
    let [commit] = run.commits() else {
        panic!("exactly one commit: one candidate covers the dispatch")
    };
    assert_eq!(
        commit.rule,
        StateArgumentSpecializationRule::contract().identity()
    );
    let PsiRewritePatch::SpecializeStateArgument(patch) = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .patch()
    else {
        panic!("a state-argument patch")
    };
    (commit, patch)
}

/// The whole selected pass declines the unit and leaves it byte-exact.
fn assert_declines(unit: VerifiedPsiOptimizationUnit) {
    let input_identity = unit.unit().identity;
    let run = specialize(unit);
    assert!(run.commits().is_empty(), "no candidate specializes");
    assert_eq!(run.session().unit().identity, input_identity);
}

/// The sparse constant lattice the plan resolves bound arguments against.
fn scalar_constants(unit: &PsiOptimizationUnit) -> crate::ScalarConstantAnalysis {
    let Some(AnalysisProduct::ScalarConstants(constants)) =
        compute_analysis(unit, AnalysisKind::ScalarConstants)
    else {
        panic!("the scalar constant analysis computes")
    };
    constants
}

/// The dispatch is an eligible site whose every incoming edge qualifies:
/// admission reports the plan with no edges rather than orphaning the state.
fn assert_orphaning_plan_is_empty(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    dispatch: BlockId,
) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let plan = super::propose::plan(unit, function, dispatch, &scalar_constants(unit))
        .expect("the dispatch is an eligible site");
    assert_eq!(plan.machine, machine);
    assert_eq!(plan.dispatch, dispatch);
    assert!(
        plan.edges.is_empty(),
        "fusing every incoming edge would orphan the dispatch state"
    );
}

/// The row the admission predicates would derive for the constant-supplied
/// jump into `dispatch` were the machine acyclic — the sparse lattice
/// withholds derived facts inside a machine holding an authenticated cyclic
/// component, so the rule never proposes it — is refused by the independent
/// validator's own cyclic-machine freeze before any plan replay.
fn assert_frozen_dispatch_is_refused(
    unit: &PsiOptimizationUnit,
    frozen_machines: &[MachineId],
    machine: MachineId,
    dispatch: BlockId,
    parameter: ValueId,
    constant: bool,
) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    // The constant-supplied edge is the one entering the dispatch from
    // outside the entry block; the entry's own arm keeps the still-variable
    // machine parameter.
    let mut supplied = function
        .blocks
        .iter()
        .filter(|block| block.id != function.entry)
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch);
    let incoming = supplied.next().expect("a constant-supplied incoming edge");
    assert!(
        supplied.next().is_none(),
        "exactly one edge enters the dispatch from outside the entry"
    );
    let predecessor = edge_owner(unit, machine, incoming.psi_edge);
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(unit, machine, dispatch, constant);
    let plan = StateArgumentSpecializationRewrite {
        machine,
        dispatch,
        edges: vec![optimization_unit::SpecializedStateEdgeRow {
            incoming_edge: incoming.psi_edge,
            predecessor,
            parameter,
            argument: bound_argument(incoming, parameter),
            constant,
            taken_edge: taken_edge.psi_edge,
            rejected_edge: rejected_edge.psi_edge,
            resolved_target,
        }],
    };
    let provenance = super::accounting::provenance_rows(function, &plan).expect("edges exist");
    let mut affected_blocks = vec![dispatch, predecessor.block];
    affected_blocks.sort_unstable();
    affected_blocks.dedup();
    let candidate = PsiRewriteCandidate::new_state_argument_specialization(
        unit.identity,
        StateArgumentSpecializationRule::contract(),
        affected_blocks,
        provenance,
        -1,
        plan,
    )
    .expect("the frozen row is a well-formed declaration");
    // The validator refuses the row before any plan replay: unit validation
    // names the first frozen machine it meets, whichever machine the
    // candidate targets, and the family's own freeze rejects the machine's
    // patch.
    let refused = match validate_state_argument_specialization_candidate(unit, &candidate) {
        Err(OptimizationUnitValidationError::ControlCycle {
            machine: rejected, ..
        }) => frozen_machines.contains(&rejected),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch) => true,
        _ => false,
    };
    assert!(
        refused,
        "the validator freezes the cyclic machine independently of the rule"
    );
}

/// The committed declaration rebuilt with `input` as its revision and
/// `mutate` applied to its patch, keeping every other declared axis.
fn rebuild(
    declaration: &PsiRewriteCandidate,
    input: OptimizationUnitIdentity,
    mutate: impl FnOnce(&mut StateArgumentSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    let PsiRewritePatch::SpecializeStateArgument(mut patch) = declaration.patch() else {
        panic!("a state-argument patch")
    };
    mutate(&mut patch);
    PsiRewriteCandidate::new_state_argument_specialization(
        input,
        StateArgumentSpecializationRule::contract(),
        declaration.affected_blocks().to_vec(),
        declaration.provenance().to_vec(),
        declaration.predicted_cost_delta(),
        patch,
    )
}

fn forged(
    declaration: &PsiRewriteCandidate,
    mutate: impl FnOnce(&mut StateArgumentSpecializationRewrite),
) -> Result<PsiRewriteCandidate, PsiRewriteCandidateError> {
    rebuild(declaration, declaration.input(), mutate)
}

/// A forged row is refused either at candidate construction — the patch
/// invariants already disagree — or by the independent replay, which
/// re-admits every row against `input` rather than trusting it.
fn assert_rejects_rows(
    input: &PsiOptimizationUnit,
    forged: Result<PsiRewriteCandidate, PsiRewriteCandidateError>,
) {
    let Ok(candidate) = forged else {
        return;
    };
    assert!(matches!(
        validate_state_argument_specialization_candidate(input, &candidate),
        Err(OptimizationUnitValidationError::CandidatePatchMismatch
            | OptimizationUnitValidationError::CandidateProvenanceMismatch
            | OptimizationUnitValidationError::CandidateLocationMissing
            | OptimizationUnitValidationError::CandidateOutsideRegionMismatch)
    ));
}

/// The number of edges still entering `dispatch`.
fn incoming_edge_count(
    function: &optimization_unit::PsiOptimizationFunction,
    dispatch: BlockId,
) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch)
        .count()
}

/// The single successor edge of the `Jump` at `predecessor` after fusion.
fn fused_jump_edge(
    function: &optimization_unit::PsiOptimizationFunction,
    predecessor: NodeLocation,
) -> &OptimizationEdge {
    function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists")
}

/// The integer-comparison dispatch block and the own scalar parameter its
/// condition compares against a literal.
fn integer_dispatch(unit: &PsiOptimizationUnit, machine: MachineId) -> Option<(BlockId, ValueId)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    function.blocks.iter().find_map(|block| {
        let (last, prefix) = block.nodes.split_last()?;
        let AbstractOperation::Conditional { condition, .. } = &last.operation else {
            return None;
        };
        prefix.iter().find_map(|node| {
            let (result, left, right) = match &node.operation {
                AbstractOperation::IntegerEqual {
                    result,
                    left,
                    right,
                    ..
                }
                | AbstractOperation::IntegerLessThan {
                    result,
                    left,
                    right,
                    ..
                }
                | AbstractOperation::IntegerLessOrEqual {
                    result,
                    left,
                    right,
                    ..
                } => (*result, *left, *right),
                _ => return None,
            };
            if result != *condition {
                return None;
            }
            block
                .parameters
                .iter()
                .find(|parameter| parameter.value == left || parameter.value == right)
                .map(|parameter| (block.id, parameter.value))
        })
    })
}

/// The single-node parameter dispatch block and its condition parameter.
fn parameter_dispatch(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
) -> Option<(BlockId, ValueId)> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    function.blocks.iter().find_map(|block| {
        let [node] = block.nodes.as_slice() else {
            return None;
        };
        let AbstractOperation::Conditional { condition, .. } = &node.operation else {
            return None;
        };
        block
            .parameters
            .iter()
            .any(|parameter| parameter.value == *condition)
            .then_some((block.id, *condition))
    })
}

/// The unconditional edge that enters `dispatch`, required to be the only
/// Jump-owned incoming edge in these fixtures.
fn jump_edge_to(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    dispatch: BlockId,
) -> Option<&OptimizationEdge> {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)?;
    let mut matches = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| {
            if !matches!(node.operation, AbstractOperation::Jump { .. }) {
                return Vec::new();
            }
            node.successors
                .iter()
                .filter(|edge| edge.target == dispatch)
                .collect::<Vec<_>>()
        });
    let edge = matches.next()?;
    matches.next().is_none().then_some(edge)
}

/// The dispatch conditional's taken and rejected arm edges for `constant`,
/// plus the resolved target block.
fn dispatch_arms(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    dispatch: BlockId,
    constant: bool,
) -> (&OptimizationEdge, &OptimizationEdge, BlockId) {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let block = function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .expect("dispatch exists");
    let node = block.nodes.last().expect("dispatch terminator node");
    let AbstractOperation::Conditional {
        when_true,
        when_false,
        ..
    } = &node.operation
    else {
        panic!("dispatch is a conditional")
    };
    let resolved = if constant { when_true } else { when_false };
    let rejected = if constant { when_false } else { when_true };
    let taken_edge = node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == resolved.psi_edge)
        .expect("taken arm edge exists");
    let rejected_edge = node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == rejected.psi_edge)
        .expect("rejected arm edge exists");
    (taken_edge, rejected_edge, resolved.target)
}

fn bound_argument(edge: &OptimizationEdge, parameter: ValueId) -> ValueId {
    edge.bindings
        .iter()
        .find(|binding| binding.parameter == parameter)
        .expect("state argument binding exists")
        .argument
}

/// Owning site of an edge identity inside one machine.
fn edge_owner(unit: &PsiOptimizationUnit, machine: MachineId, edge: EdgeId) -> NodeLocation {
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            if node
                .successors
                .iter()
                .any(|successor| successor.psi_edge == edge)
            {
                return NodeLocation {
                    machine,
                    block: block.id,
                    node: u32::try_from(node_index).expect("node index fits u32"),
                };
            }
        }
    }
    panic!("edge has an owner")
}

fn lowered_unit(source: &str, label: &str) -> VerifiedPsiOptimizationUnit {
    lowered_unit_entry(source, label, "Root::run")
}

fn lowered_unit_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name(entry),
    )
    .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("encode {label} semantics: {error:?}")),
            proof_bytes: &terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap_or_else(|error| panic!("encode {label} proof: {error:?}")),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"))
}

//! Optimizer module role: test leaf. Constant state-argument specialization proposal, replay, and custody evidence.

use super::super::VerifiedPsiOptimizationSession;
use crate::{
    StateArgumentSpecializationCandidate, StateArgumentSpecializationError,
    apply_state_argument_specialization, optimize_abstract_operations,
    propose_state_argument_specializations, validate_state_argument_specialization,
};
use abstract_operations::AbstractOperation;
use optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use optimization_unit::{
    NodeLocation, OptimizationEdge, ProvenanceDisposition, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, recompute_psi_optimization_unit_identity,
};
use optimization_unit_semantics::OptimizationUnitValidationError;
use semantic_vocabulary::{BlockId, EdgeId, MachineId, ValueId};

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

/// Two unconditional edges supply different constants to the same dispatch
/// state while a third edge keeps the still-variable parameter, so both
/// constant paths specialize atomically in one candidate.
const TWO_EDGE_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> warm(mode)
            _ -> triage(flag, mode)
        }
        state triage(g: bool, m: u32 in Wrapping) {
            transition g {
                true -> chill(m)
                _ -> choose(g, m)
            }
        }
        state warm(m: u32 in Wrapping) {
            let on: bool = true;
            transition { _ -> choose(on, m) }
        }
        state chill(m: u32 in Wrapping) {
            let off: bool = false;
            transition { _ -> choose(off, m) }
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

/// The same dispatch shape with no constant-supplied argument: every edge
/// binds the still-variable parameter, so nothing may specialize.
const VARIABLE_EDGE_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> relay(flag, mode)
            _ -> choose(flag, mode)
        }
        state relay(f: bool, m: u32 in Wrapping) {
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

/// Both incoming edges supply constants, so specializing all of them would
/// orphan the dispatch state; the family declines the site entirely.
const ALL_CONSTANT_SOURCE: &str = r#"
    data Root {}

    machine Root::run(flag: bool, mode: u32 in Wrapping)
    {
        transition flag {
            true -> warm(mode)
            _ -> chill(mode)
        }
        state warm(m: u32 in Wrapping) {
            let on: bool = true;
            transition { _ -> choose(on, m) }
        }
        state chill(m: u32 in Wrapping) {
            let off: bool = false;
            transition { _ -> choose(off, m) }
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

/// Both arms of one `Conditional` predecessor are the dispatch's only incoming
/// edges and both supply proven constants, so specializing all of them would
/// orphan the dispatch state; the family declines the site entirely.
const ALL_CONSTANT_CONDITIONAL_SOURCE: &str = r#"
    data Root {}

    machine Root::run(mode: u32 in Wrapping)
    {
        transition { _ -> warm(mode) }
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

/// The same specialization shape inside a machine carrying an authenticated
/// cyclic component: the frozen-territory gate declines every dispatch there
/// even though an incoming edge binds a literal constant.
const CYCLIC_SOURCE: &str = r#"
    data Root {}

    machine Root::icyc(idx: u64, mode: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition idx {
            0 -> iwarm(mode, remaining)
            _ -> ichoose(idx, mode, remaining)
        }
        state iwarm(m: u32 in Wrapping, r: u32 [0..=5]) {
            let z: u64 = 0;
            transition { _ -> ichoose(z, m, r) }
        }
        state ichoose(i: u64, m: u32 in Wrapping, r: u32 [0..=5]) {
            transition i {
                0 -> ispin(i, m, r)
                _ -> iright(m)
            }
        }
        state ispin(k: u64, s: u32 in Wrapping, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> icyc(k, s, pending - 1)
                _ -> iright(s)
            }
        }
        state iright(x: u32 in Wrapping) {}
    }

    machine Root::scan(flag: bool, mode: u32 in Wrapping, remaining: u32 [0..=5])
    {
        transition flag {
            true -> warm(mode, remaining)
            _ -> choose(flag, mode, remaining)
        }
        state warm(m: u32 in Wrapping, r: u32 [0..=5]) {
            let on: bool = true;
            transition { _ -> choose(on, m, r) }
        }
        state choose(f: bool, m: u32 in Wrapping, r: u32 [0..=5]) {
            transition f {
                true -> spin(f, m, r)
                _ -> right(m)
            }
        }
        state spin(go: bool, s: u32 in Wrapping, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> scan(go, s, pending - 1)
                _ -> right(s)
            }
        }
        state right(x: u32 in Wrapping) {
            Root::icyc(0, x, 5);
        }
    }
"#;

#[test]
fn constant_state_argument_edge_specializes_the_dispatch() {
    let session = lowered_session(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (dispatch, parameter) = parameter_dispatch(&unit, machine).expect("dispatch state exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.dispatch(), dispatch);
    assert_eq!(candidate.input(), unit.identity);
    assert_ne!(candidate.output(), unit.identity);
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    assert_eq!(row.argument(), bound_argument(incoming_edge, parameter));
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), taken_edge.target);
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    assert_eq!(row.predecessor(), predecessor);

    // The proposal is deterministic and the supplying edge identity is bound
    // into the candidate identity.
    let replayed = propose_state_argument_specializations(&session, 4).expect("replay runs");
    assert_eq!(replayed, candidates);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let next = applied.session();
    let output_function = next
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");

    // The fused edge keeps its own Psi identity, targets the resolved arm's
    // block directly, and carries both source edges' custody in order.
    let fused = output_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
    assert_eq!(fused.psi_edge, incoming_edge.psi_edge);
    assert_eq!(fused.target, taken_edge.target);
    assert_eq!(
        fused.provenance,
        vec![
            PsiProvenance::Edge(incoming_edge.psi_edge),
            PsiProvenance::Edge(taken_edge.psi_edge),
        ]
    );
    assert_eq!(
        fused.fuel,
        vec![
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(incoming_edge.psi_edge),
                units: 1,
            },
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(taken_edge.psi_edge),
                units: 1,
            },
        ]
    );
    // The resolved arm's parameter binding is composed through the incoming
    // edge: the dispatch parameter never reaches the successor.
    let resolved_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == resolved_target)
        .expect("resolved target retained");
    assert_eq!(fused.bindings.len(), resolved_block.parameters.len());
    assert!(
        fused
            .bindings
            .iter()
            .all(|binding| binding.argument != parameter),
        "the state argument itself is consumed by the specialization"
    );

    // The dispatch state and both its arm edges survive unchanged for the
    // remaining incoming path, and it keeps exactly that one predecessor.
    let output_dispatch = output_function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .expect("dispatch state retained");
    let input_dispatch = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| function.blocks.iter().find(|block| block.id == dispatch))
        .expect("input dispatch state");
    assert_eq!(output_dispatch, input_dispatch);
    let remaining_incoming = output_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch)
        .count();
    assert_eq!(remaining_incoming, 1);

    // The ledger records exact edge custody: the incoming edge's retained
    // occurrence and the resolved arm's fan-out into the fused site plus its
    // surviving dispatch occurrence.
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(record.input, unit.identity);
    assert_eq!(record.output, next.unit().identity);
    let mut expected = vec![
        optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Edge {
                machine,
                edge: incoming_edge.psi_edge,
            },
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                machine,
                edge: incoming_edge.psi_edge,
            }),
            sources: incoming_edge.provenance.clone(),
            fuel: incoming_edge.fuel.clone(),
        },
        optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Edge {
                machine,
                edge: taken_edge.psi_edge,
            },
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                machine,
                edge: incoming_edge.psi_edge,
            }),
            sources: taken_edge.provenance.clone(),
            fuel: taken_edge.fuel.clone(),
        },
        optimization_unit::ProvenanceRewrite {
            input: PsiRealizationSite::Edge {
                machine,
                edge: taken_edge.psi_edge,
            },
            disposition: ProvenanceDisposition::RealizedAt(PsiRealizationSite::Edge {
                machine,
                edge: taken_edge.psi_edge,
            }),
            sources: taken_edge.provenance.clone(),
            fuel: taken_edge.fuel.clone(),
        },
    ];
    expected.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    assert_eq!(record.provenance, expected);

    // The applied session is an exact fixed point for this family.
    assert!(
        propose_state_argument_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn constant_edges_on_both_arms_specialize_together() {
    let session = lowered_session(TWO_EDGE_SOURCE, "two-edge specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate covering both constant edges")
    };
    let [first, second] = candidate.specializations() else {
        panic!("two specialized incoming edges")
    };
    assert_ne!(first.constant(), second.constant());
    assert_ne!(first.resolved_target(), second.resolved_target());
    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine())
        .expect("machine retained");
    let dispatch = function
        .blocks
        .iter()
        .find(|block| block.id == candidate.dispatch())
        .expect("dispatch state retained");
    let remaining = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == candidate.dispatch())
        .count();
    assert_eq!(remaining, 1, "only the variable edge still enters");
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(
        record.provenance.len(),
        6,
        "three custody rows per fused edge"
    );
    let dispatch_node = &dispatch.nodes[0];
    assert_eq!(
        dispatch_node.successors.len(),
        2,
        "both dispatch arms remain for the variable path"
    );
}

#[test]
fn conditional_arm_state_argument_edge_specializes_the_dispatch() {
    let session = lowered_session(CONDITIONAL_EDGE_SOURCE, "conditional-arm specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let dispatch = candidate.dispatch();
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    let parameter = row.parameter();
    let predecessor = edge_owner(&unit, machine, row.incoming_edge());
    assert_eq!(row.predecessor(), predecessor);
    let input_function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let predecessor_node = &input_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
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
    let sibling = if when_true.psi_edge == row.incoming_edge() {
        when_false
    } else {
        assert_eq!(when_false.psi_edge, row.incoming_edge());
        when_true
    };
    let incoming = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == row.incoming_edge())
        .expect("admitted arm edge exists");
    assert_eq!(row.argument(), bound_argument(incoming, parameter));
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), taken_edge.target);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let output_node = &output_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .expect("predecessor block retained")
        .nodes[usize::try_from(predecessor.node).expect("node index")];

    // The predecessor stays a conditional: the admitted arm retargets to the
    // resolved dispatch arm's block while the sibling arm stays byte-exact.
    let AbstractOperation::Conditional {
        when_true: fused_true,
        when_false: fused_false,
        ..
    } = &output_node.operation
    else {
        panic!("the predecessor keeps its conditional shape")
    };
    let (fused_arm, kept_arm) = if fused_true.psi_edge == row.incoming_edge() {
        (fused_true, fused_false)
    } else {
        (fused_false, fused_true)
    };
    assert_eq!(fused_arm.target, resolved_target);
    assert_eq!(kept_arm, sibling, "the sibling arm is byte-exact");
    let fused_edge = output_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == row.incoming_edge())
        .expect("fused edge exists");
    assert_eq!(fused_edge.target, resolved_target);
    assert_eq!(
        fused_edge.provenance,
        vec![
            PsiProvenance::Edge(row.incoming_edge()),
            PsiProvenance::Edge(taken_edge.psi_edge),
        ]
    );
    assert_eq!(
        fused_edge.fuel,
        vec![
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(row.incoming_edge()),
                units: 1,
            },
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(taken_edge.psi_edge),
                units: 1,
            },
        ]
    );
    let kept_edge = output_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == sibling.psi_edge)
        .expect("sibling edge retained");
    let input_sibling_edge = predecessor_node
        .successors
        .iter()
        .find(|edge| edge.psi_edge == sibling.psi_edge)
        .expect("input sibling edge");
    assert_eq!(
        kept_edge, input_sibling_edge,
        "the sibling edge is byte-exact"
    );
    assert_eq!(output_node.successors.len(), 2);

    // The dispatch keeps exactly its one unfused incoming path — the
    // still-variable `variable` jump.
    let remaining_incoming = output_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch)
        .count();
    assert_eq!(remaining_incoming, 1);
    assert!(
        propose_state_argument_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn both_arms_of_one_conditional_specialize_together() {
    let session = lowered_session(BOTH_CONDITIONAL_ARMS_SOURCE, "both-arms specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one candidate covering both constant arms")
    };
    let [first, second] = candidate.specializations() else {
        panic!("two specialized incoming edges")
    };
    // Both rows name the same predecessor conditional site but different arm
    // edges; they fold into a single node reconstruction, not two overwrites.
    assert_eq!(first.predecessor(), second.predecessor());
    assert_ne!(first.incoming_edge(), second.incoming_edge());
    assert_ne!(first.constant(), second.constant());
    assert_ne!(first.resolved_target(), second.resolved_target());

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine())
        .expect("machine retained");
    let predecessor = first.predecessor();
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
            .find(|row| row.incoming_edge() == arm.psi_edge)
            .expect("every arm fused");
        assert_eq!(arm.target, row.resolved_target());
        assert_eq!(edge.target, row.resolved_target());
    }
    // Only the entry's unfused arm still enters the dispatch.
    let remaining = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == candidate.dispatch())
        .count();
    assert_eq!(remaining, 1, "only the variable edge still enters");
    let [record] = applied.ledger().records() else {
        panic!("one transformation record")
    };
    assert_eq!(
        record.provenance.len(),
        6,
        "three custody rows per fused edge"
    );
    assert!(
        propose_state_argument_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn all_constant_conditional_arms_decline_to_orphan_the_dispatch() {
    let session = lowered_session(
        ALL_CONSTANT_CONDITIONAL_SOURCE,
        "all-constant conditional decline",
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "fusing every incoming edge — both arms of the only conditional \
         predecessor — would orphan the dispatch state"
    );
}

#[test]
fn replay_rejects_forged_conditional_arm_rows() {
    let session = lowered_session(CONDITIONAL_EDGE_SOURCE, "conditional-arm specialization");
    let unit = session.unit().clone();
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    let row = candidate.specializations()[0].clone();
    let predecessor = edge_owner(&unit, candidate.machine(), row.incoming_edge());
    let predecessor_node = &unit
        .functions
        .iter()
        .find(|function| function.machine == candidate.machine())
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
    let sibling = if when_true.psi_edge == row.incoming_edge() {
        when_false
    } else {
        when_true
    };

    // A forged supplying edge naming the conditional's sibling arm — the arm
    // that does not enter the dispatch at all.
    let mut forged = candidate.clone();
    forged.specializations[0].incoming_edge = sibling.psi_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged predecessor coordinate.
    let mut forged = candidate.clone();
    forged.specializations[0].predecessor.node += 1;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged resolved arm edge.
    let mut forged = candidate.clone();
    forged.specializations[0].taken_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged output revision.
    let mut forged = candidate.clone();
    forged.output =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"forged-output");
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A duplicated row — the same arm claimed twice — cannot validate: the
    // replayed plan carries each admissible edge exactly once.
    let mut forged = candidate.clone();
    forged.specializations.push(row);
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_state_argument_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

#[test]
fn variable_state_argument_yields_no_candidate() {
    let session = lowered_session(VARIABLE_EDGE_SOURCE, "variable-edge decline");
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
}

#[test]
fn fully_constant_incoming_declines_to_orphan_the_dispatch() {
    let session = lowered_session(ALL_CONSTANT_SOURCE, "all-constant decline");
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (dispatch, _) = parameter_dispatch(unit, machine).expect("dispatch state exists");
    let candidate = StateArgumentSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-orphan-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        dispatch,
        specializations: Vec::new(),
    };
    assert_eq!(
        validate_state_argument_specialization(&session, &candidate).err(),
        Some(StateArgumentSpecializationError::AlreadySpecialized)
    );
}

#[test]
fn component_machine_declines_specialization() {
    let session = lowered_session_entry(CYCLIC_SOURCE, "cyclic-machine decline", "Root::scan");
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "no dispatch inside frozen territory specializes"
    );
    let unit = session.unit();
    // `scan` is the machine carrying the single-node Boolean dispatch.
    let (machine, dispatch) = unit
        .functions
        .iter()
        .find_map(|function| {
            parameter_dispatch(unit, function.machine)
                .map(|(dispatch, _)| (function.machine, dispatch))
        })
        .expect("scan's dispatch state exists");
    let candidate = StateArgumentSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-cyclic-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        dispatch,
        specializations: Vec::new(),
    };
    assert_eq!(
        validate_state_argument_specialization(&session, &candidate).err(),
        Some(StateArgumentSpecializationError::UnknownDispatch)
    );

    // The freeze covers the integer family too: `icyc`'s `i == 0` dispatch
    // sits inside the second cyclic machine, so it neither proposes nor
    // validates even though `iwarm` supplies a proven literal.
    let integer_machine = unit
        .functions
        .iter()
        .map(|function| function.machine)
        .find(|candidate| *candidate != machine)
        .expect("second cyclic machine");
    let (integer_dispatch, _) =
        integer_dispatch(unit, integer_machine).expect("integer dispatch exists");
    let forged = StateArgumentSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-icyc-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine: integer_machine,
        dispatch: integer_dispatch,
        specializations: Vec::new(),
    };
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::UnknownDispatch)
    );
}

#[test]
fn replay_rejects_forged_specialization_rows() {
    let session = lowered_session(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged resolved arm edge.
    let mut forged = candidate.clone();
    forged.specializations[0].taken_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged constant verdict.
    let mut forged = candidate.clone();
    forged.specializations[0].constant = !forged.specializations[0].constant;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged supplying edge.
    let mut forged = candidate.clone();
    forged.specializations[0].incoming_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged predecessor coordinate.
    let mut forged = candidate.clone();
    forged.specializations[0].predecessor.node += 1;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged candidate identity.
    let mut forged = candidate.clone();
    forged.identity =
        optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(b"forged-identity");
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged output revision.
    let mut forged = candidate.clone();
    forged.output =
        optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"forged-output");
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_state_argument_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

#[test]
fn replay_rejects_stale_candidate_revision() {
    let session = lowered_session(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    let mut stale = candidate.clone();
    stale.input = optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"stale-input");
    assert_eq!(
        validate_state_argument_specialization(&session, &stale).err(),
        Some(StateArgumentSpecializationError::StaleCandidateRevision {
            candidate: stale.input,
            current: session.unit().identity,
        })
    );

    // Applying moves the revision; the original candidate is stale afterward.
    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    assert_eq!(
        validate_state_argument_specialization(applied.session(), candidate).err(),
        Some(StateArgumentSpecializationError::StaleCandidateRevision {
            candidate: candidate.input(),
            current: applied.session().unit().identity,
        })
    );
}

#[test]
fn candidate_budget_is_exact() {
    let session = lowered_session(SINGLE_EDGE_SOURCE, "single-edge specialization");
    assert_eq!(
        propose_state_argument_specializations(&session, 0).err(),
        Some(StateArgumentSpecializationError::CandidateBudgetExhausted {
            required: 1,
            limit: 0,
        })
    );
    assert_eq!(
        propose_state_argument_specializations(&session, 1)
            .expect("proposal runs")
            .len(),
        1
    );
}

#[test]
fn transformed_replay_rejects_forged_fused_edge_custody() {
    let session = lowered_session(SINGLE_EDGE_SOURCE, "single-edge specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };
    let row = &candidate.specializations()[0];
    let (incoming_edge, taken_edge, rejected_edge, machine) = (
        row.incoming_edge(),
        row.taken_edge(),
        row.rejected_edge(),
        candidate.machine(),
    );
    let verified_input = session.input().clone();
    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");

    // A unit whose fused edge claims a different second custody source — the
    // rejected arm's edge rather than the resolved arm's — is still a
    // well-formed edge roster, but it is not the specialization this
    // candidate pins: replay rebuilds the plan's own output and the forged
    // revision identity mismatches.
    let mut corrupted = validated.output.clone();
    let fused = corrupted
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
    fused.provenance[1] = PsiProvenance::Edge(rejected_edge);
    fused.fuel[1].site = PsiProvenance::Edge(rejected_edge);
    corrupted.identity = recompute_psi_optimization_unit_identity(&corrupted);
    let mut forged = candidate.clone();
    forged.output = corrupted.identity;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // Dropping the resolved arm's fuel settlement while keeping its custody
    // claim leaves a unit whose edge settles fewer sources than it names:
    // transformed validation rejects the forged fuel/provenance pair.
    let mut malformed = validated.output.clone();
    malformed
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.nodes)
        .flat_map(|node| &mut node.successors)
        .find(|edge| edge.psi_edge == incoming_edge)
        .expect("fused edge exists")
        .fuel
        .pop();
    malformed.identity = recompute_psi_optimization_unit_identity(&malformed);
    assert!(matches!(
        VerifiedPsiOptimizationSession::from_transformed(verified_input, malformed),
        Err(
            OptimizationUnitValidationError::FuelDoesNotMatchProvenance { machine: rejected, .. }
        ) if rejected == machine
    ));

    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            applied.session().input().clone(),
            applied.session().unit().clone(),
        )
        .is_ok(),
        "the applied fused revision revalidates independently"
    );
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

/// The same integer dispatch shape, but `warm` binds `i` to a proven `5`:
/// `5 == 0` fails, so the incoming edge resolves the `when_false` arm — the
/// rejected literal arm is what "non-Boolean argument" means here: the state
/// value selects the arm, not the literal itself.
const INT_MATCH_REJECTED_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> warm(mode)
            _ -> choose(idx, mode)
        }
        state warm(m: u32 in Wrapping) {
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

/// The same integer dispatch shape with no constant-supplied argument: every
/// edge binds the still-variable machine parameter, so nothing may
/// specialize.
const INT_MATCH_VARIABLE_SOURCE: &str = r#"
    data Root {}

    machine Root::run(idx: u64, mode: u32 in Wrapping)
    {
        transition idx {
            0 -> relay(idx, mode)
            _ -> choose(idx, mode)
        }
        state relay(i: u64, m: u32 in Wrapping) {
            transition { _ -> choose(i, m) }
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

/// A computed-comparison dispatch: `choose` reads `i` through an in-block
/// `i < 4` — the transition subject lowers to `[IntegerConstant(4),
/// IntegerLessThan(i, 4), Conditional]` with the parameter on the left.
/// `warm` binds `i` to a proven `3`, satisfying the bound, so its jump edge
/// resolves the `when_true` arm.
const INT_LESS_THAN_SOURCE: &str = r#"
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
            transition i < 4 { true -> left(m) _ -> right(m) }
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
fn integer_literal_match_specializes_the_taken_arm() {
    let session = lowered_session(INT_MATCH_TAKEN_SOURCE, "integer taken specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (dispatch, parameter) = integer_dispatch(&unit, machine).expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.dispatch(), dispatch);
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    assert_eq!(row.argument(), bound_argument(incoming_edge, parameter));
    // `i := 0` satisfies `i == 0`, so the when_true arm is taken.
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), taken_edge.target);
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    assert_eq!(row.predecessor(), predecessor);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let next = applied.session();
    let output_function = next
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");

    // The fused edge keeps its own Psi identity, targets the resolved arm's
    // block directly, and carries both source edges' custody in order.
    let fused = output_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
    assert_eq!(fused.psi_edge, incoming_edge.psi_edge);
    assert_eq!(fused.target, taken_edge.target);
    assert_eq!(
        fused.provenance,
        vec![
            PsiProvenance::Edge(incoming_edge.psi_edge),
            PsiProvenance::Edge(taken_edge.psi_edge),
        ]
    );
    // The resolved arm's parameter binding is composed through the incoming
    // edge: the dispatch parameter never reaches the successor.
    let resolved_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == resolved_target)
        .expect("resolved target retained");
    assert_eq!(fused.bindings.len(), resolved_block.parameters.len());
    assert!(
        fused
            .bindings
            .iter()
            .all(|binding| binding.argument != parameter),
        "the state argument itself is consumed by the specialization"
    );

    // The dispatch state and both its arm edges survive unchanged for the
    // remaining incoming path.
    let output_dispatch = output_function
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .expect("dispatch state retained");
    let input_dispatch = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| function.blocks.iter().find(|block| block.id == dispatch))
        .expect("input dispatch state");
    assert_eq!(output_dispatch, input_dispatch);
    let remaining_incoming = output_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch)
        .count();
    assert_eq!(remaining_incoming, 1);

    // The fused revision revalidates independently and reaches a fixed point.
    assert!(
        VerifiedPsiOptimizationSession::from_transformed(
            applied.session().input().clone(),
            applied.session().unit().clone(),
        )
        .is_ok(),
        "the applied fused revision revalidates independently"
    );
    assert!(
        propose_state_argument_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn integer_literal_match_specializes_the_rejected_arm() {
    let session = lowered_session(INT_MATCH_REJECTED_SOURCE, "integer rejected specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (dispatch, parameter) = integer_dispatch(&unit, machine).expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    // `i := 5` fails `i == 0`, so the when_false arm is taken.
    assert!(!row.constant());
    let (taken_edge, rejected_edge, _) = dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    let fused = output_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
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
fn integer_less_than_dispatch_specializes() {
    let session = lowered_session(INT_LESS_THAN_SOURCE, "integer less-than specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (dispatch, parameter) = integer_dispatch(&unit, machine).expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    // `i := 3` satisfies `i < 4`, so the when_true arm is taken.
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), resolved_target);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    let fused = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
            function
                .blocks
                .iter()
                .find(|block| block.id == predecessor.block)
        })
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
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
fn integer_literal_left_less_or_equal_specializes() {
    let session = lowered_session(INT_LITERAL_LEFT_SOURCE, "literal-left specialization");
    let unit = session.unit().clone();
    let machine = unit.functions[0].machine;
    let (dispatch, parameter) = integer_dispatch(&unit, machine).expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    // `4 <= 3` fails, so the when_false arm is taken — the parameter sat on
    // the comparison's right and the literal bound on its left.
    assert!(!row.constant());
    let (taken_edge, rejected_edge, _) = dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    let fused = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
            function
                .blocks
                .iter()
                .find(|block| block.id == predecessor.block)
        })
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
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
    let session = lowered_session(INT_NOT_EQUAL_SOURCE, "not-equal decline");
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "the condition is a BooleanNot of the comparison, not the comparison itself"
    );
}

#[test]
fn integer_dispatch_variable_argument_yields_no_candidate() {
    let session = lowered_session(INT_MATCH_VARIABLE_SOURCE, "integer variable decline");
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty()
    );
}

#[test]
fn all_constant_integer_edges_decline_to_orphan_the_dispatch() {
    let session = lowered_session(
        INT_MATCH_ALL_CONSTANT_SOURCE,
        "integer all-constant decline",
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "fusing every constant-supplied incoming edge would orphan the dispatch state"
    );
    let unit = session.unit();
    let machine = unit.functions[0].machine;
    let (dispatch, _) = integer_dispatch(unit, machine).expect("integer dispatch exists");
    let candidate = StateArgumentSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-orphan-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        dispatch,
        specializations: Vec::new(),
    };
    assert_eq!(
        validate_state_argument_specialization(&session, &candidate).err(),
        Some(StateArgumentSpecializationError::AlreadySpecialized)
    );
}

#[test]
fn replay_rejects_forged_integer_arm_rows() {
    let session = lowered_session(INT_MATCH_TAKEN_SOURCE, "integer taken specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged constant verdict — `i := 0` really does satisfy `i == 0`, so
    // flipping the resolved arm cannot replay.
    let mut forged = candidate.clone();
    forged.specializations[0].constant = !forged.specializations[0].constant;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged resolved arm edge.
    let mut forged = candidate.clone();
    forged.specializations[0].taken_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged supplying edge.
    let mut forged = candidate.clone();
    forged.specializations[0].incoming_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_state_argument_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

/// The result specialization: `warm` binds the dispatch parameter `f` to the
/// scalar result of a direct `Root::flag()` call, spelled inline as the edge
/// argument. `flag` carries exactly one `Return` whose value the callee's
/// own lattice proves `true`, so the call's result is a proven constant even
/// though the sparse lattice leaves every call result overdefined — the
/// bound argument's constant is the callee's proven result. `warm`'s jump
/// edge fuses with the `when_true` arm while the entry's `_` arm keeps the
/// still-variable parameter and the dispatch reachable.
const CALL_RESULT_TAKEN_SOURCE: &str = r#"
    data Root {}

    machine Root::flag() -> bool { true }

    machine Root::run(mode: u32 in Wrapping, pick: bool)
    {
        transition pick {
            true -> warm(mode)
            _ -> choose(pick, mode)
        }
        state warm(m: u32 in Wrapping) {
            transition { _ -> choose(Root::flag(), m) }
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
fn call_result_state_argument_specializes_the_dispatch() {
    let session = lowered_session(CALL_RESULT_TAKEN_SOURCE, "call-result specialization");
    let unit = session.unit().clone();
    let (machine, dispatch, parameter) = unit
        .functions
        .iter()
        .find_map(|function| {
            parameter_dispatch(&unit, function.machine)
                .map(|(dispatch, parameter)| (function.machine, dispatch, parameter))
        })
        .expect("dispatch state exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    assert_eq!(candidate.machine(), machine);
    assert_eq!(candidate.dispatch(), dispatch);
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    assert_eq!(row.argument(), bound_argument(incoming_edge, parameter));
    // The bound argument reaches the dispatch through a single-predecessor
    // forwarding block: it is the owner block's own parameter, bound by the
    // unique incoming edge to the `Call` result — the specialization is
    // driven by the callee's proven result, not a caller-local literal.
    let input_function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine exists");
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    let owner_block = input_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .expect("owner block exists");
    assert!(
        owner_block
            .parameters
            .iter()
            .any(|parameter| parameter.value == row.argument()),
        "the bound argument is delivered through the forwarding block's parameter"
    );
    let delivered_by_call = input_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == predecessor.block)
        .flat_map(|edge| &edge.bindings)
        .filter(|binding| binding.parameter == row.argument())
        .any(|binding| {
            input_function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| {
                    matches!(
                        &node.operation,
                        AbstractOperation::Call { result, .. } if *result == binding.argument
                    )
                })
        });
    assert!(delivered_by_call, "the delivered value is a call result");
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), taken_edge.target);
    assert_eq!(row.predecessor(), predecessor);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let output_function = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .expect("machine retained");

    // The fused edge keeps its own Psi identity, targets the resolved arm's
    // block directly, and carries both source edges' custody in order — the
    // call still executes at the predecessor; only its proven result moved.
    let fused = output_function
        .blocks
        .iter()
        .find(|block| block.id == predecessor.block)
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
    assert_eq!(fused.psi_edge, incoming_edge.psi_edge);
    assert_eq!(fused.target, taken_edge.target);
    assert_eq!(
        fused.provenance,
        vec![
            PsiProvenance::Edge(incoming_edge.psi_edge),
            PsiProvenance::Edge(taken_edge.psi_edge),
        ]
    );
    assert_eq!(
        fused.fuel,
        vec![
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(incoming_edge.psi_edge),
                units: 1,
            },
            optimization_unit::FuelSettlement {
                site: PsiProvenance::Edge(taken_edge.psi_edge),
                units: 1,
            },
        ]
    );
    let resolved_block = output_function
        .blocks
        .iter()
        .find(|block| block.id == resolved_target)
        .expect("resolved target retained");
    assert_eq!(fused.bindings.len(), resolved_block.parameters.len());

    // The dispatch and both arms survive unchanged for the variable path.
    let remaining_incoming = output_function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .filter(|edge| edge.target == dispatch)
        .count();
    assert_eq!(remaining_incoming, 1);
    assert!(
        propose_state_argument_specializations(applied.session(), 4)
            .expect("fixed-point proposal runs")
            .is_empty(),
        "the specialization reaches a fixed point"
    );
}

#[test]
fn integer_call_result_specializes_the_comparison_dispatch() {
    let session = lowered_session(
        CALL_RESULT_INTEGER_SOURCE,
        "integer call-result specialization",
    );
    let unit = session.unit().clone();
    let (machine, dispatch, parameter) = unit
        .functions
        .iter()
        .find_map(|function| {
            integer_dispatch(&unit, function.machine)
                .map(|(dispatch, parameter)| (function.machine, dispatch, parameter))
        })
        .expect("integer dispatch exists");
    let incoming_edge =
        jump_edge_to(&unit, machine, dispatch).expect("unconditional incoming edge");

    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("exactly one specialization candidate")
    };
    let [row] = candidate.specializations() else {
        panic!("one specialized incoming edge")
    };
    assert_eq!(row.incoming_edge(), incoming_edge.psi_edge);
    assert_eq!(row.parameter(), parameter);
    // `n := 3` satisfies `i < 4`, so the when_true arm is taken.
    assert!(row.constant());
    let (taken_edge, rejected_edge, resolved_target) =
        dispatch_arms(&unit, machine, dispatch, row.constant());
    assert_eq!(row.taken_edge(), taken_edge.psi_edge);
    assert_eq!(row.rejected_edge(), rejected_edge.psi_edge);
    assert_eq!(row.resolved_target(), resolved_target);

    let validated =
        validate_state_argument_specialization(&session, candidate).expect("independent replay");
    let applied = apply_state_argument_specialization(session, validated).expect("apply");
    let predecessor = edge_owner(&unit, machine, incoming_edge.psi_edge);
    let fused = applied
        .session()
        .unit()
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
            function
                .blocks
                .iter()
                .find(|block| block.id == predecessor.block)
        })
        .and_then(|block| {
            block.nodes[usize::try_from(predecessor.node).expect("index")]
                .successors
                .first()
        })
        .expect("fused edge exists");
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
    let session = lowered_session(
        CALL_RESULT_MULTI_RETURN_SOURCE,
        "multi-return callee decline",
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "a callee with two Return nodes is not one exact constant result"
    );
}

#[test]
fn variable_callee_result_yields_no_candidate() {
    let session = lowered_session(
        CALL_RESULT_VARIABLE_SOURCE,
        "variable callee result decline",
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "a callee returning its own parameter is not a proven constant result"
    );
}

#[test]
fn call_result_specialization_stays_frozen_in_cyclic_machines() {
    let session = lowered_session_entry(
        CALL_RESULT_CYCLIC_SOURCE,
        "cyclic call-result decline",
        "Root::spin",
    );
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    assert!(
        propose_state_argument_specializations(&session, 4)
            .expect("proposal runs")
            .is_empty(),
        "the call-result edge inside frozen territory does not specialize"
    );
    let unit = session.unit();
    let (machine, dispatch, _) = unit
        .functions
        .iter()
        .find_map(|function| {
            parameter_dispatch(unit, function.machine)
                .map(|(dispatch, parameter)| (function.machine, dispatch, parameter))
        })
        .expect("spin's dispatch state exists");
    let forged = StateArgumentSpecializationCandidate {
        identity: optimization_core::OptimizationCandidateIdentity::from_canonical_bytes(
            b"forged-cyclic-result-candidate",
        ),
        input: unit.identity,
        output: unit.identity,
        machine,
        dispatch,
        specializations: Vec::new(),
    };
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::UnknownDispatch)
    );
}

#[test]
fn replay_rejects_forged_call_result_rows() {
    let session = lowered_session(CALL_RESULT_TAKEN_SOURCE, "call-result specialization");
    let candidates = propose_state_argument_specializations(&session, 4).expect("proposal runs");
    let [candidate] = candidates.as_slice() else {
        panic!("one specialization candidate")
    };

    // A forged constant verdict — `on` really is the proven `true` result of
    // `flag`, so flipping the resolved arm cannot replay.
    let mut forged = candidate.clone();
    forged.specializations[0].constant = !forged.specializations[0].constant;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged bound argument — the replayed plan re-derives the call
    // result's own value identity, so a drifted argument cannot match.
    let mut forged = candidate.clone();
    forged.specializations[0].argument = forged.specializations[0].parameter;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged resolved arm edge.
    let mut forged = candidate.clone();
    forged.specializations[0].taken_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged supplying edge.
    let mut forged = candidate.clone();
    forged.specializations[0].incoming_edge = forged.specializations[0].rejected_edge;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // A forged predecessor coordinate.
    let mut forged = candidate.clone();
    forged.specializations[0].predecessor.node += 1;
    assert_eq!(
        validate_state_argument_specialization(&session, &forged).err(),
        Some(StateArgumentSpecializationError::CandidateMismatch)
    );

    // The untampered candidate still validates.
    assert!(
        validate_state_argument_specialization(&session, candidate).is_ok(),
        "the exact candidate still validates"
    );
}

/// The board's acceptance chain witnessed end to end on a real source file:
/// the lowered `run` machine reaches the public `optimize_abstract_operations`
/// entrance under the exact `Optimization::StateSpecialization` selection,
/// commits through `omega.psi-rule.state-argument-specialization.v1`, and
/// publishes a validated plan whose unit moved — the candidate replayed
/// independently inside that same entrance, because publication runs the
/// registered validators rather than trusting the producer.
#[test]
fn source_produced_machine_selects_the_rule_through_the_public_entrance() {
    let session = lowered_session(CALL_RESULT_TAKEN_SOURCE, "source entrance");
    let input_identity = session.unit().identity;
    let input = session.input().clone();
    let selections = OptimizationSelections::new([Optimization::StateSpecialization])
        .expect("state-specialization selection");
    let plan = optimize_abstract_operations(
        input,
        &selections,
        &selections.project_psi(),
        OptimizationWorkBudget::new(96, 64, 64, 64, 64).expect("budget"),
    )
    .expect("the public entrance publishes a validated plan");
    assert_eq!(plan.commits().len(), 1);
    assert!(
        plan.commits()
            .iter()
            .any(|commit| commit.rule == super::rule_identity()),
        "the commit carries this family's exact rule identity"
    );
    assert_eq!(plan.selections(), &selections);
    assert_eq!(plan.psi_selections(), &selections);
    assert_ne!(plan.unit().identity, input_identity);
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

fn lowered_session(source: &str, label: &str) -> VerifiedPsiOptimizationSession {
    lowered_session_entry(source, label, "Root::run")
}

fn lowered_session_entry(source: &str, label: &str, entry: &str) -> VerifiedPsiOptimizationSession {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|error| panic!("tokenize {label}: {error:?}"));
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .unwrap_or_else(|error| panic!("parse {label}: {error:?}"));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap_or_else(|error| panic!("resolve {label}: {error:?}"));
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|error| panic!("type {label}: {error:?}"));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check {label}: {error:?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, entry)
        .unwrap_or_else(|error| panic!("lower {label}: {error:?}"));
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
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
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap_or_else(|error| panic!("optimizer-only {label} admission: {error:?}"));
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap_or_else(|error| panic!("build {label} optimizer unit: {error:?}"));
    VerifiedPsiOptimizationSession::new(verified)
        .unwrap_or_else(|error| panic!("verified {label} session: {error:?}"))
}

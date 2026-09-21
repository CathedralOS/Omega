//! Optimizer module role: test leaf. Constant state-argument specialization proposal, replay, and custody evidence.

use super::super::VerifiedPsiOptimizationSession;
use crate::{
    StateArgumentSpecializationCandidate, StateArgumentSpecializationError,
    apply_state_argument_specialization, propose_state_argument_specializations,
    validate_state_argument_specialization,
};
use abstract_operations::AbstractOperation;
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
        state right(x: u32 in Wrapping) {}
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
    let machine = unit.functions[0].machine;
    let (dispatch, _) = parameter_dispatch(unit, machine).expect("dispatch state exists");
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
    let [node] = block.nodes.as_slice() else {
        panic!("dispatch is a single-node block")
    };
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

//! Shared address joins bind a projected, readable root and pin it in scope.
use crate::tests::structural_cases::source_machine_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::{StructuralAccess, StructuralPathSegment};

/// `borrowed_results::PRIMITIVE_CALL_SOURCE`: `&a.left` and `&b.right` meet
/// in one shared `&u64` block parameter.
const PRIMITIVE_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &u64) -> u64 { value }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &u64 = match other { true -> &a.left, false -> &b.right };
        read(view)
    }";

/// `borrowed_results::RECORD_CALL_SOURCE`: whole locals meet in `&Payload`.
const RECORD_CALL_SOURCE: &str = "data Payload { left: u64; right: u64; }
    machine read(value: &Payload) -> u64 { value.left ^ value.right }
    machine choose(other: bool) -> u64 {
        let a: Payload = Payload { left: 1, right: 2 };
        let b: Payload = Payload { left: 3, right: 4 };
        let view: &Payload = match other { true -> &a, false -> &b };
        read(view)
    }";

/// Every (block, node) whose terminator binds the address join.
fn join_edges(unit: &PsiOptimizationUnit) -> Vec<(usize, usize)> {
    let function = &unit.functions[0];
    function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block_index, block)| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    node.successors
                        .iter()
                        .any(|edge| !edge.structural_bindings.is_empty())
                })
                .map(move |(node_index, _)| (block_index, node_index))
        })
        .collect()
}

fn unit(source: &str) -> PsiOptimizationUnit {
    let unit = source_machine_unit(source, "choose", true);
    assert!(
        unit.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| parameter.access == StructuralAccess::SharedBorrow),
        "customer keeps its shared join"
    );
    unit
}

fn mutate_first_binding(
    unit: &mut PsiOptimizationUnit,
    edit: impl Fn(&mut abstract_operations::AbstractStructuralBinding),
) {
    let (block, node) = join_edges(unit)[0];
    let AbstractOperation::Jump {
        structural_bindings,
        ..
    } = &mut unit.functions[0].blocks[block].nodes[node].operation
    else {
        panic!("join edges are ordinary jumps")
    };
    edit(&mut structural_bindings[0]);
    refresh_node_derivatives(unit, 0, block, node);
}

#[test]
fn write_only_state_store_keeps_the_arriving_storage_authority() {
    let candidate = source_machine_unit(
        "machine run(destination: &write i32) {
            transition { _ -> arrived(&write destination) }
            state arrived(destination: &write i32) { destination = 42; }
        }",
        "run",
        true,
    );
    validate_psi_optimization_unit(&candidate).expect("write-only state store preserves its loan");
    let mut widened = candidate.clone();
    let parameter = widened.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structural_parameters)
        .find(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
        .expect("write-only arrival");
    parameter.access = StructuralAccess::MutableBorrow;
    validate_psi_optimization_unit(&widened)
        .expect_err("an arrival cannot gain readable authority");
}

#[test]
fn projected_and_whole_address_joins_validate() {
    for source in [PRIMITIVE_CALL_SOURCE, RECORD_CALL_SOURCE] {
        let candidate = unit(source);
        assert_eq!(join_edges(&candidate).len(), 2);
        validate_psi_optimization_unit(&candidate).unwrap();
    }
}

#[test]
fn address_join_edges_reject_substituted_referents_widened_access_and_forged_roots() {
    let edits: [(
        &str,
        fn(&mut abstract_operations::AbstractStructuralBinding),
    ); 5] = [
        // `&a.right` still names a `u64`; `&a` names the whole record.
        ("whole root", |binding| binding.argument.path.clear()),
        ("missing field", |binding| {
            binding.argument.path = vec![StructuralPathSegment::Field("middle".into())]
        }),
        ("exclusive", |binding| {
            binding.argument.access = StructuralAccess::MutableBorrow
        }),
        ("owned", |binding| {
            binding.argument.access = StructuralAccess::Owned
        }),
        ("forged root", |binding| {
            binding.argument.place = semantic_vocabulary::PlaceId::new(9_999).unwrap()
        }),
    ];
    for (label, edit) in edits {
        let mut candidate = unit(PRIMITIVE_CALL_SOURCE);
        mutate_first_binding(&mut candidate, edit);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "{label} must not bind the address join"
        );
    }
    // A projected edge may not stand in for a whole-record join either.
    let mut record = unit(RECORD_CALL_SOURCE);
    mutate_first_binding(&mut record, |binding| {
        binding.argument.path = vec![StructuralPathSegment::Field("left".into())]
    });
    assert!(validate_psi_optimization_unit(&record).is_err());
}

#[test]
fn address_join_origins_stay_pinned_while_the_join_is_in_scope() {
    // Discarding a lent root on the join's own outgoing edge would end its
    // storage while the view is still nameable downstream.
    let mut candidate = unit(PRIMITIVE_CALL_SOURCE);
    let function = &candidate.functions[0];
    let join_block = function
        .blocks
        .iter()
        .position(|block| !block.structural_parameters.is_empty())
        .unwrap();
    let origin = function.blocks[join_edges(&candidate)[0].0].nodes[join_edges(&candidate)[0].1]
        .successors[0]
        .structural_bindings[0]
        .argument
        .place;
    let node = candidate.functions[0].blocks[join_block].nodes.len() - 1;
    let AbstractOperation::Jump {
        trivial_affine_discards,
        ..
    } = &mut candidate.functions[0].blocks[join_block].nodes[node].operation
    else {
        panic!("the join block continues to its call")
    };
    trivial_affine_discards.push(origin);
    refresh_node_derivatives(&mut candidate, 0, join_block, node);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { place, .. })
            if place == origin
    ));
}

/// A later state lends its own local `&mut` into a successor, which ends the
/// loan chain on its outgoing edges.
const LATER_STATE_LOAN_SOURCE: &str =
    "data Score { value: u64 in Wrapping; bonus: u64 in Wrapping; }
    machine run(start: u64) -> u64 {
        transition start > 2 {
            true -> prepare(start)
            _ -> finish(71)
        }
        state prepare(start: u64) -> u64 {
            let mut score: Score;
            score.value = start as u64 in Wrapping;
            transition { _ -> bump(&mut score) }
        }
        state bump(score: &mut Score) -> u64 {
            score.bonus = score.value + 67;
            transition score.bonus == 70 {
                true -> finish(70)
                _ -> finish(72)
            }
        }
        state finish(code: u64) -> u64 {
            transition { _ -> (code) }
        }
    }";

#[test]
fn an_exclusive_home_is_discarded_only_where_its_loan_chain_ends() {
    let candidate = source_machine_unit(LATER_STATE_LOAN_SOURCE, "run", true);
    let function = &candidate.functions[0];
    let (lend_block, lend_node) = join_edges(&candidate)[0];
    let home = function.blocks[lend_block].nodes[lend_node].successors[0].structural_bindings[0]
        .argument
        .place;
    // The chain ends on the borrowing state's own edges, which discard the
    // lender's home there rather than at a return.
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .flat_map(|node| &node.successors)
            .any(|edge| edge.trivial_affine_discards.contains(&home)),
        "the chain end discards the lent home on an edge"
    );
    validate_psi_optimization_unit(&candidate).unwrap();

    // Discarding the home on the edge that hands the loan to its reader ends
    // the storage while the exclusive view is still live downstream.
    let mut premature = candidate.clone();
    let AbstractOperation::Jump {
        trivial_affine_discards,
        ..
    } = &mut premature.functions[0].blocks[lend_block].nodes[lend_node].operation
    else {
        panic!("the lending state jumps to its successor")
    };
    trivial_affine_discards.push(home);
    refresh_node_derivatives(&mut premature, 0, lend_block, lend_node);
    assert!(validate_psi_optimization_unit(&premature).is_err());
}

const RELEASED_LOAN_SOURCE: &str = "data Counter { count: u64; }
    machine Counter::update(&mut self, bytes: &[u8]) {
        transition { _ -> snapshot(&self.count, bytes) }
        state snapshot(&mut self, count: &u64, bytes: &[u8]) {
            let saved: u64 = count;
            transition { _ -> writeback(saved, bytes) }
        }
        state writeback(&mut self, saved: u64, bytes: &[u8]) {
            self.count = ((saved as u64 in Wrapping) + (bytes.len as u64 in Wrapping)) as u64;
        }
    }";

/// Exercise the loan replay independently of unrelated scalar provenance and
/// control-cycle admission. The source fixture still passes all earlier
/// lowering stages; each test names any deliberate current-graph corruption.
pub(super) fn validate_address_join_ownership(
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let function = &unit.functions[0];
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect();
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .iter()
                    .flat_map(|node| &node.successors)
                    .map(|edge| edge.target)
                    .collect(),
            )
        })
        .collect();
    let functions = unit
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect();
    let boundary_machines = unit
        .boundary_machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect();
    let structural_types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    crate::current_ownership::validate_current_ownership_frontier(
        function,
        &blocks,
        &successors,
        &functions,
        &boundary_machines,
        &structural_types,
        &crate::current_ownership::CallAccesses::new(&functions, &structural_types),
    )
}

#[test]
fn a_dropped_address_join_releases_its_root_while_an_unrelated_view_is_forwarded() {
    let candidate = source_machine_unit(RELEASED_LOAN_SOURCE, "Counter::update", true);
    validate_psi_optimization_unit(&candidate).unwrap();
}

#[test]
fn a_join_used_after_its_apparent_release_keeps_the_root_pinned() {
    let mut candidate = source_machine_unit(RELEASED_LOAN_SOURCE, "Counter::update", true);
    let read = candidate.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::PrimitiveScalarRead { .. } => Some(node.operation.clone()),
            _ => None,
        })
        .expect("the snapshot reads the joined count");
    let (block, node) = candidate.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .nodes
                .iter()
                .enumerate()
                .find_map(|(node_index, node)| {
                    matches!(node.operation, AbstractOperation::ByteSequenceLength { .. })
                        .then_some((block_index, node_index))
                })
        })
        .expect("writeback measures the independent bytes");
    let AbstractOperation::ByteSequenceLength { result, .. } =
        &candidate.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    let result = result.clone();
    let mut stale_read = read;
    let AbstractOperation::PrimitiveScalarRead {
        result: read_result,
        ..
    } = &mut stale_read
    else {
        unreachable!()
    };
    *read_result = result;
    candidate.functions[0].blocks[block].nodes[node].operation = stale_read;
    refresh_node_derivatives(&mut candidate, 0, block, node);
    assert!(validate_psi_optimization_unit(&candidate).is_err());
    // The substituted read also changes scalar provenance. Bypass that
    // independent earlier check to prove that the loan owner itself rejects
    // mutation while this stale joined name remains observed.
    let result = validate_address_join_ownership(&candidate);
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "loan replay must retain the pin: {result:?}"
    );
}

#[test]
fn address_join_forwarding_through_a_backedge_preserves_then_releases_the_loan() {
    let source = "data Counter { count: u64; }
        machine Counter::update(&mut self, bytes: &[u8], repeat: bool) {
            transition { _ -> snapshot(&self.count, bytes, repeat) }
            state snapshot(&mut self, count: &u64, bytes: &[u8], repeat: bool) {
                let saved: u64 = count;
                transition repeat {
                    true -> snapshot(count, bytes, false)
                    false -> writeback(saved, bytes)
                }
            }
            state writeback(&mut self, saved: u64, bytes: &[u8]) {
                self.count = ((saved as u64 in Wrapping) + (bytes.len as u64 in Wrapping)) as u64;
            }
        }";
    let candidate = source_machine_unit(source, "Counter::update", true);
    // Unranked cycles are not admitted by the full current-graph validator.
    // Pin reconstruction must nevertheless handle its CFG backedges without
    // retaining the loan on the separate writeback exit.
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::ControlCycle { .. })
    ));
    validate_address_join_ownership(&candidate).unwrap();
}

pub(super) const STATE_RECORD_VIEW_SOURCE: &str = r#"
    boundary trait Host { machine exit(code: i32); }
    data Packet { items: [u8; 4]; marker: i32; }
    machine stamp(values: &mut [u8]) {
        transition values.len >= 4 { true -> write(values) _ -> skip() }
        state write(values: &mut [u8]) { values[0] = 70; values[3] = 5; }
        state skip() {}
    }
    machine main() reaches Host {
        let mut packet: Packet;
        transition { _ -> check(&mut packet) }
        state check(packet: &mut Packet) {
            stamp(&mut packet.items);
            let view: &[u8] = packet.items.as_slice();
            transition view.len == 4 && packet.items[0] == 70 && packet.items[3] == 5 && packet.marker == 0 {
                true -> done() _ -> failed()
            }
        }
        state done() { Host::exit(70); }
        state failed() { Host::exit(71); }
    }
"#;

#[test]
fn address_join_shared_attenuation_keeps_existing_view_observations() {
    let candidate = crate::tests::structural_cases::verified_source_machine_unit(
        STATE_RECORD_VIEW_SOURCE,
        "main",
    );
    validate_psi_optimization_unit(&candidate)
        .expect("read-only continuation keeps the original view and backing");
}

#[test]
fn address_join_later_exclusive_handoff_keeps_old_descriptor_suspended() {
    let mut candidate = crate::tests::structural_cases::verified_source_machine_unit(
        STATE_RECORD_VIEW_SOURCE,
        "main",
    );
    let (target, parameter, descriptor) = candidate.functions[0]
        .blocks
        .iter()
        .find_map(|block| {
            let parameter = block
                .structural_parameters
                .iter()
                .find(|parameter| parameter.access == StructuralAccess::SharedBorrow)?;
            let descriptor = block.nodes.iter().find_map(|node| match node.operation {
                AbstractOperation::ByteSequenceLength { source, .. } => Some(source),
                _ => None,
            })?;
            Some((block.id, parameter.place, descriptor))
        })
        .expect("the view observation follows a shared whole-record arrival");
    for block in &mut candidate.functions[0].blocks {
        if block.id == target {
            block
                .structural_parameters
                .iter_mut()
                .find(|candidate| candidate.place == parameter)
                .unwrap()
                .access = StructuralAccess::MutableBorrow;
        }
    }
    let mut changed = Vec::new();
    for (block_index, block) in candidate.functions[0].blocks.iter_mut().enumerate() {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            if let AbstractOperation::Jump {
                structural_bindings,
                ..
            } = &mut node.operation
            {
                for binding in structural_bindings {
                    if binding.parameter == parameter {
                        binding.argument.access = StructuralAccess::MutableBorrow;
                        changed.push((block_index, node_index));
                    }
                }
            }
        }
    }
    let (block_index, node_index) = changed[0];
    let handoff_block = candidate.functions[0].blocks[block_index].id;
    let handoff_parent = candidate.functions[0].blocks[block_index].nodes[node_index]
        .successors
        .iter()
        .flat_map(|edge| &edge.structural_bindings)
        .find(|binding| binding.parameter == parameter)
        .unwrap()
        .argument
        .place;
    assert_ne!(handoff_parent, descriptor);
    for (block, node) in changed {
        refresh_node_derivatives(&mut candidate, 0, block, node);
    }
    let error = validate_address_join_ownership(&candidate)
        .expect_err("the exclusive handoff conflicts with the still-observed shared child");
    assert!(
        matches!(error, OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { place, block, node, .. }
            if place == handoff_parent && block == handoff_block && node as usize == node_index),
        "{error:?}"
    );
}

const DISJOINT_STATE_FIELD_SOURCE: &str = "data Pair [copy] { left: u64; right: u64; }
    data Owner { pair: Pair; total: u64; }
    machine Owner::run(&mut self) -> u64 {
        transition { _ -> observe(&self.pair) }
        state observe(&mut self, pair: &Pair) -> u64 {
            transition { _ -> store(pair) }
        }
        state store(&mut self, pair: &Pair) -> u64 {
            self.total = pair.left;
            transition { _ -> (self.total ^ pair.right) }
        }
    }";

#[test]
fn shared_address_joins_allow_disjoint_stores_and_reject_overlap_after_forwarding() {
    let mut candidate = source_machine_unit(DISJOINT_STATE_FIELD_SOURCE, "Owner::run", true);
    validate_psi_optimization_unit(&candidate).unwrap();
    let (block, node) = candidate.functions[0]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.nodes.iter().enumerate().find_map(|(node, item)| {
                matches!(
                    item.operation,
                    AbstractOperation::StructuralScalarFieldStore { .. }
                )
                .then_some((block, node))
            })
        })
        .expect("the source writes the receiver's disjoint total field");
    let AbstractOperation::StructuralScalarFieldStore { path, field, .. } =
        &mut candidate.functions[0].blocks[block].nodes[node].operation
    else {
        unreachable!()
    };
    *path = vec![StructuralPathSegment::Field("pair".into())];
    *field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
    refresh_node_derivatives(&mut candidate, 0, block, node);
    assert!(
        matches!(
            validate_address_join_ownership(&candidate),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "the forwarded pair loan must still reject writes to its own fields"
    );
}

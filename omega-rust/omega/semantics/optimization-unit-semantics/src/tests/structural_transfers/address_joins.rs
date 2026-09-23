//! Shared address joins bind a projected, readable root and pin it in scope.
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::tests::refresh_node_derivatives;
use crate::tests::structural_cases::source_machine_unit;
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation;
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

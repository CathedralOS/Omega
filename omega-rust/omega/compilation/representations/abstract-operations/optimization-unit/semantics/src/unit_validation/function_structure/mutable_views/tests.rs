//! Graph-level replay controls. Whole-unit tests in structural_transfers also
//! exercise authority, projection types, and independently derived metadata.
use super::*;
use crate::tests::fixtures::scalar_units::unit;
use abstract_operations::AbstractStructuralBinding;
use optimization_unit::{OptimizationBlock, OptimizationEdge};
use semantic_vocabulary::{EdgeId, OperationId, ValueId};
use terminal_psi::{
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration,
};

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
fn block(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

fn parameter(value: u64, access: StructuralAccess) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(value),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn edge(target: u64, source: u64, destination: u64, projected: bool) -> OptimizationEdge {
    OptimizationEdge {
        psi_edge: EdgeId::new(target).unwrap(),
        target: block(target),
        bindings: Vec::new(),
        structural_bindings: vec![AbstractStructuralBinding {
            parameter: place(destination),
            argument: StructuralArgument {
                place: place(source),
                path: if projected {
                    vec![StructuralPathSegment::FixedIndex(0)]
                } else {
                    Vec::new()
                },
                access: StructuralAccess::SharedBorrow,
            },
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance: Vec::new(),
        fuel: Vec::new(),
    }
}

fn graph_block(
    template: &OptimizationBlock,
    identity: u64,
    parameters: Vec<StructuralParameterDeclaration>,
    edges: Vec<OptimizationEdge>,
) -> OptimizationBlock {
    let mut result = template.clone();
    result.id = block(identity);
    result.structural_parameters = parameters;
    result.nodes.truncate(1);
    result.nodes[0].successors = edges;
    result
}

fn view_type() -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "test::mutable-view".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        }),
    }
}

#[test]
fn parallel_edges_and_backedges_preserve_definite_and_possible_names() {
    let mut function = unit().functions.remove(0);
    let template = function.blocks[0].clone();
    function.entry = block(2);
    function.structural_parameters = vec![parameter(10, StructuralAccess::MutableBorrow)];
    // One parallel edge retires the whole name, the other lends a projection.
    // The stale use below must fail even though that name is possibly live.
    function.blocks = vec![
        graph_block(
            &template,
            9000,
            vec![parameter(12, StructuralAccess::SharedBorrow)],
            vec![edge(70, 11, 11, false)],
        ),
        graph_block(&template, 8000, Vec::new(), Vec::new()),
        graph_block(&template, 2, Vec::new(), vec![edge(70, 10, 11, false)]),
        graph_block(
            &template,
            70,
            vec![parameter(11, StructuralAccess::MutableBorrow)],
            vec![edge(9000, 11, 12, false), edge(9000, 11, 12, true)],
        ),
    ];
    let declaration = view_type();
    let types = BTreeMap::from([(declaration.id, &declaration)]);
    let replay = current_mutable_borrows(&function, &types);
    assert_eq!(replay.arrivals[&block(2)], BTreeSet::from([place(10)]));
    assert_eq!(replay.arrivals[&block(70)], BTreeSet::from([place(11)]));
    assert!(replay.arrivals[&block(9000)].is_empty());
    assert_eq!(
        replay.possible_arrivals[&block(9000)],
        BTreeSet::from([place(11)])
    );
    assert!(replay.arrivals[&block(8000)].is_empty());
    assert!(replay.possible_arrivals[&block(8000)].is_empty());
    function.blocks.reverse();
    let reordered = current_mutable_borrows(&function, &types);
    assert_eq!(replay.arrivals, reordered.arrivals);
    assert_eq!(replay.possible_arrivals, reordered.possible_arrivals);

    let join = function
        .blocks
        .iter_mut()
        .find(|row| row.id == block(9000))
        .unwrap();
    join.nodes[0].operation = O::ByteSequenceLength {
        psi_operation: OperationId::new(91).unwrap(),
        source: place(11),
        result: abstract_operations::AbstractResult {
            value: ValueId::new(92).unwrap(),
            scalar_type: function.result.scalar().unwrap().scalar_type,
        },
    };
    assert!(
        matches!(validate(&function, &types), Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { block: failed_block, place: failed_place, .. }) if failed_block == block(9000) && failed_place == place(11))
    );
}

#[test]
fn entry_seed_participates_in_backedge_intersection_and_union() {
    let mut function = unit().functions.remove(0);
    let template = function.blocks[0].clone();
    function.entry = block(2);
    function.structural_parameters = vec![parameter(10, StructuralAccess::MutableBorrow)];
    function.blocks = vec![graph_block(
        &template,
        2,
        vec![parameter(11, StructuralAccess::MutableBorrow)],
        vec![edge(2, 10, 11, false)],
    )];
    let declaration = view_type();
    let replay =
        current_mutable_borrows(&function, &BTreeMap::from([(declaration.id, &declaration)]));
    assert!(replay.arrivals[&block(2)].is_empty());
    assert_eq!(
        replay.possible_arrivals[&block(2)],
        BTreeSet::from([place(10), place(11)])
    );
}

#[test]
fn edge_preparation_retires_produced_names_before_generating_bindings() {
    let mut function = unit().functions.remove(0);
    let template = function.blocks[0].clone();
    function.blocks = vec![
        graph_block(
            &template,
            2,
            Vec::new(),
            vec![edge(70, 10, 11, false), edge(70, 10, 11, true)],
        ),
        graph_block(
            &template,
            70,
            vec![parameter(11, StructuralAccess::MutableBorrow)],
            Vec::new(),
        ),
    ];
    let mutable = BTreeSet::from([place(10), place(11)]);
    let produced = BTreeMap::from([
        (block(2), BTreeSet::from([place(10)])),
        (block(70), BTreeSet::new()),
    ]);
    let transfers = prepare_transfers(&function, &mutable, &mutable, &produced);
    assert_eq!(
        transfers.len(),
        2,
        "parallel edge occurrences must remain distinct"
    );
    assert_eq!(transfers[0].generated, BTreeSet::from([place(11)]));
    assert_eq!(transfers[1].generated, mutable);
    assert_eq!(transfers[0].retired, BTreeSet::from([place(10), place(11)]));
    assert_eq!(transfers[1].retired, BTreeSet::from([place(11)]));
}

#[test]
fn shared_record_attenuation_retires_block_names_not_invocation_resources() {
    let mut function = unit().functions.remove(0);
    let template = function.blocks[0].clone();
    function.structural_parameters = vec![parameter(10, StructuralAccess::MutableBorrow)];
    function.blocks = vec![
        graph_block(&template, 2, Vec::new(), vec![edge(70, 10, 11, false)]),
        graph_block(
            &template,
            70,
            vec![parameter(11, StructuralAccess::SharedBorrow)],
            Vec::new(),
        ),
    ];
    // Preparation takes already-classified sets. These exclusive names are
    // records, not descriptors: shared reborrowing preserves invocation custody.
    let mutable = BTreeSet::from([place(10)]);
    let produced = BTreeMap::from([(block(2), BTreeSet::new()), (block(70), BTreeSet::new())]);
    let transfers = prepare_transfers(&function, &mutable, &BTreeSet::new(), &produced);
    assert!(!transfers[0].retired.contains(&place(10)));
    function.structural_parameters.clear();
    function.blocks[0].structural_parameters = vec![parameter(10, StructuralAccess::MutableBorrow)];
    let transfers = prepare_transfers(&function, &mutable, &BTreeSet::new(), &produced);
    assert!(transfers[0].retired.contains(&place(10)));
}

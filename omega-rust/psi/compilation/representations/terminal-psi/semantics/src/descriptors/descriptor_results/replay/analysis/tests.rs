use super::*;
use crate::descriptors::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};
use semantic_vocabulary::{BlockId, ScalarType};
use terminal_psi::TerminalIdentityIndex;

#[test]
fn whole_payload_choices_keep_all_targets_without_inventing_a_single_address() {
    let boolean = StructuralTypeId::new(1).unwrap();
    let pointer = StructuralTypeId::new(2).unwrap();
    let types = [
        (
            boolean,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ),
        (
            pointer,
            StructuralTypeShape::Reference {
                referent: boolean,
                access: StructuralAccess::MutableBorrow,
            },
        ),
    ]
    .map(|(id, shape)| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        shape,
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
    });
    let location = |id| DescriptorLocation::whole(PlaceId::new(id).unwrap());
    let saved = location(1);
    let left = location(2);
    let right = location(3);
    let position = |id| (BlockId::new(id).unwrap(), 0);
    let edge = |id| DescriptorFlowEdge {
        target: position(id),
        parameters: Vec::new(),
        bindings: Vec::new(),
    };
    let id = MachineId::new(1).unwrap();
    let machine = DescriptorMachine {
        id,
        returns: None,
        places: [
            (saved.clone(), pointer),
            (left.clone(), boolean),
            (right.clone(), boolean),
        ]
        .into_iter()
        .enumerate()
        .map(|(position, (place, structural_type))| DescriptorPlace {
            place: place.root,
            definition: DescriptorDefinition::Parameter {
                position,
                access: StructuralAccess::Owned,
                structural_type,
            },
        })
        .collect(),
        place_types: BTreeMap::from([
            (saved.root, pointer),
            (left.root, boolean),
            (right.root, boolean),
        ]),
        entry: position(1),
        flow: BTreeMap::from([
            (
                position(1),
                DescriptorFlowStep {
                    successors: vec![edge(2), edge(3)],
                    ..Default::default()
                },
            ),
            (
                position(2),
                DescriptorFlowStep {
                    definitions: vec![saved.clone()],
                    derivations: vec![(left.clone(), saved.clone())],
                    successors: vec![edge(4)],
                    ..Default::default()
                },
            ),
            (
                position(3),
                DescriptorFlowStep {
                    definitions: vec![saved.clone()],
                    derivations: vec![(right.clone(), saved.clone())],
                    successors: vec![edge(4)],
                    ..Default::default()
                },
            ),
            (position(4), DescriptorFlowStep::default()),
        ]),
        storage_derivations: BTreeSet::from([
            (position(2), left.clone(), saved.clone()),
            (position(3), right.clone(), saved.clone()),
        ]),
        normal_exits: vec![position(4)],
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_stores: BTreeMap::new(),
        primitive_payload_writes: Default::default(),
        opaque_payload_results: Default::default(),
        payload_entry_predicates: None,
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        calls: BTreeMap::new(),
    };
    let query = DescriptorContentsQuery {
        root: saved.root,
        steps: vec![DescriptorContentsStep::Address(
            DescriptorPathSegment::Referent,
        )],
    };
    let mut graph = [machine];
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert_eq!(
        analysis.unchanged_payload_targets(id, position(4), &query),
        Some(vec![left.clone(), right.clone()])
    );
    assert_eq!(analysis.exact_payload_target(id, position(4), &query), None);
    drop(analysis);

    // An interpreted pointer keeps a conservative backing relation but cannot
    // certify this whole, unchanged payload on either singleton or MAY routes.
    let mut projection = Projection::new(&[]);
    projection.exact = false;
    graph[0]
        .projections
        .insert((position(3), right.clone(), saved.clone()), projection);
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert_eq!(
        analysis.unchanged_payload_targets(id, position(4), &query),
        None
    );
    assert_eq!(analysis.exact_payload_target(id, position(4), &query), None);
    drop(analysis);

    // One unavailable branch poisons the complete set; the known branch is
    // never treated as the whole possible write footprint.
    graph[0].projections.clear();
    graph[0]
        .flow
        .get_mut(&position(3))
        .unwrap()
        .derivations
        .clear();
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert_eq!(
        analysis.unchanged_payload_targets(id, position(4), &query),
        None
    );
    assert_eq!(analysis.exact_payload_target(id, position(4), &query), None);
}

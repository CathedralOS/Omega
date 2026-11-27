//! Pins a shared summary to the one an unshared replay finds. Two top
//! machines reach one middle machine, which calls a leaf. Every analysis
//! with the table must answer as a fresh analysis does and keep the same
//! summaries afterwards, whichever analysis replayed a callee first.

use super::*;
use crate::descriptors::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};
use crate::descriptors::descriptor_results::*;
use crate::descriptors::replay::engine::{PreparedStateCache, Replay};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::mem::{Discriminant, discriminant};
use terminal_psi::StructuralTypeShape;

// One slice, so the table and every analysis name the same nominal calls.
static NOMINAL: &[NominalCallType] = &[];

const TOP: u64 = 1;
const MIDDLE: u64 = 2;
const LEAF: u64 = 3;
const OTHER_TOP: u64 = 4;

fn id(number: u64) -> MachineId {
    MachineId::new(number).unwrap()
}

fn point(index: u32) -> DescriptorPosition {
    (BlockId::new(1).unwrap(), index)
}

fn types() -> Vec<StructuralTypeDeclaration> {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
    let scalar = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: StructuralTypeId::new(1).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        )),
        scalar_restriction: None,
    };
    let view = StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        identity: terminal_psi::TerminalIdentityIndex(2),
        shape: StructuralTypeShape::ElementView {
            element: scalar.id,
            access: None,
        },
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        ..scalar.clone()
    };
    vec![scalar, view]
}

/// A machine that returns one view: the leaf's is empty, and a caller's
/// is the one its callee returns.
fn returning(identity: u64, callee: Option<u64>) -> DescriptorMachine {
    let returned = DescriptorLocation::whole(PlaceId::new(1).unwrap());
    let mut machine = DescriptorMachine {
        id: id(identity),
        returns: Some(vec![(point(1), returned.clone())]),
        places: vec![DescriptorPlace {
            producer: Some(point(0)),
            place: returned.root,
            definition: DescriptorDefinition::Local,
        }],
        place_types: BTreeMap::from([(returned.root, StructuralTypeId::new(2).unwrap())]),
        entry: point(0),
        flow: BTreeMap::from([
            (
                point(0),
                DescriptorFlowStep {
                    definitions: vec![returned.clone()],
                    successors: vec![DescriptorFlowEdge {
                        borrowed_header_ends: Box::default(),
                        effects: Vec::new(),
                        edge: None,
                        definitions: Vec::new(),
                        target: point(1),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    }],
                    ..Default::default()
                },
            ),
            (point(1), DescriptorFlowStep::default()),
        ]),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: None,
        unsigned_index_literals: Vec::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        scalar_payload_writes: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        private_observation_inputs: Default::default(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: vec![point(1)],
    };
    match callee {
        None => {
            machine
                .originations
                .insert(point(0), vec![(returned, DescriptorCallOrigin::Empty)]);
        }
        Some(callee) => {
            machine
                .insert_call(
                    point(0),
                    DescriptorInvocation {
                        scalar_arguments: Vec::new(),
                        owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
                        target: DescriptorInvocationTarget::Machine(id(callee)),
                        arguments: Vec::new(),
                        argument_contents: Vec::new(),
                        result: Some(returned),
                    },
                )
                .unwrap();
        }
    }
    machine
}

fn graph() -> Vec<DescriptorMachine> {
    vec![
        returning(TOP, Some(MIDDLE)),
        returning(MIDDLE, Some(LEAF)),
        returning(LEAF, None),
        returning(OTHER_TOP, Some(MIDDLE)),
    ]
}

type Shape = Option<Vec<(usize, Vec<Discriminant<DescriptorStoredOrigin>>)>>;

/// What a summary tells its callers, without its storage frame.
fn shape(summary: &Summary) -> (Shape, Option<usize>, bool) {
    let slots = summary.result.slots().map(|slots| {
        slots
            .iter()
            .map(|slot| {
                (
                    slot.path.len(),
                    slot.origins.iter().map(discriminant).collect(),
                )
            })
            .collect()
    });
    let effects = summary.effects.observed().map(Vec::len);
    (slots, effects, summary.invalidates_contents)
}

type Left = Vec<(MachineId, Option<(Shape, Option<usize>, bool)>)>;

/// Every summary an analysis keeps after answering.
fn left(replay: &Replay<'_>) -> Left {
    replay
        .summaries
        .iter()
        .map(|(machine, summary)| (*machine, summary.as_ref().map(shape)))
        .collect()
}

fn table<'m>(
    types: &'m [StructuralTypeDeclaration],
    dynamic: Vec<MachineId>,
) -> ModuleSummaries<'m> {
    ModuleSummaries::over(
        types,
        NOMINAL,
        [TOP, MIDDLE, LEAF, OTHER_TOP].map(id),
        dynamic,
    )
}

fn replay<'a>(
    types: &'a [StructuralTypeDeclaration],
    graph: &'a [DescriptorMachine],
    shared: Option<&'a ModuleSummaries<'a>>,
) -> Replay<'a> {
    let mut replay = Replay::with_slot_kind(types, graph, DescriptorSlotKind::BorrowedStorage);
    replay.shared = shared;
    replay
}

fn unshared(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    root: u64,
    depth: usize,
) -> (Option<(Shape, Option<usize>, bool)>, Left) {
    let mut alone = replay(types, graph, None);
    let answer = alone.summary(id(root), depth).as_ref().map(shape);
    (answer, left(&alone))
}

fn mutable_parameter(identity: u64, ty: StructuralTypeId) -> DescriptorMachine {
    let mut machine = returning(identity, None);
    machine.returns = None;
    machine.originations.clear();
    machine.flow.get_mut(&point(0)).unwrap().definitions.clear();
    let parameter = &mut machine.places[0];
    parameter.producer = None;
    parameter.definition = DescriptorDefinition::Parameter {
        position: 0,
        structural_type: ty,
        access: terminal_psi::StructuralAccess::MutableBorrow,
    };
    machine.place_types.insert(parameter.place, ty);
    machine
}

#[test]
fn replay_requires_the_exact_opaque_entry_contents_even_after_summary_sharing() {
    let mut types = types();
    types.truncate(1);
    types[0].shape = StructuralTypeShape::TypeParameter;
    let parameter_type = types[0].id;
    for invalidate in [false, true] {
        let mut leaf = mutable_parameter(LEAF, parameter_type);
        if invalidate {
            leaf.unresolved_contents_writes.insert(point(0));
        }
        let graph = [leaf];
        let shared = table(&types, Vec::new());
        for summaries in [None, Some(&shared), Some(&shared)] {
            let mut replay = replay(&types, &graph, summaries);
            let effects = replay
                .summary(id(LEAF), 0)
                .and_then(|s| s.effects.into_observed());
            assert_eq!(
                effects.map(|effects| effects.len()),
                (!invalidate).then_some(0)
            );
        }
    }
}

#[test]
fn an_opaque_write_invalidates_its_argument_and_requires_concrete_loan_origins() {
    use crate::descriptors::descriptor_lifetimes::DescriptorContentsQuery;
    let mut types = types();
    let scalar = types[0].id;
    let view = types[1].id;
    let generic = StructuralTypeId::new(3).unwrap();
    types.push(StructuralTypeDeclaration {
        id: generic,
        identity: terminal_psi::TerminalIdentityIndex(3),
        shape: StructuralTypeShape::TypeParameter,
        ..types[0].clone()
    });
    for actual in [generic, scalar, view] {
        let mut leaf = mutable_parameter(LEAF, generic);
        leaf.unresolved_contents_writes.insert(point(0));
        let mut caller = mutable_parameter(TOP, actual);
        let argument = DescriptorLocation::whole(caller.places[0].place);
        caller
            .insert_call(
                point(0),
                DescriptorInvocation {
                    scalar_arguments: Vec::new(),
                    owner: DescriptorInvocationOwner::Operation(OperationId::new(1).unwrap()),
                    target: DescriptorInvocationTarget::Machine(id(LEAF)),
                    arguments: vec![Some(argument.clone())],
                    argument_contents: vec![Some(DescriptorContentsQuery::from_location(
                        &argument,
                    ))],
                    result: None,
                },
            )
            .unwrap();
        let graph = [leaf, caller];
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
        assert!(analysis.stored_effects(id(LEAF)).is_none());
        // A bare payload can be forgotten; concrete borrowed storage requires
        // an actual post-call backing, so its receipt cannot be discarded.
        assert_eq!(analysis.flow_arc(id(TOP)).is_ok(), actual != view);
        if actual == generic {
            let packet = DescriptorLocation {
                root: argument.root,
                path: vec![DescriptorPathSegment::UnknownContents],
            };
            let mut replay = replay(&types, &graph, None);
            let states = replay.states(id(TOP), 0).unwrap();
            assert!(
                replay
                    .read(&graph[1], &states[&point(0)], &packet)
                    .is_some()
            );
            assert!(
                replay
                    .read(&graph[1], &states[&point(1)], &packet)
                    .is_none()
            );
            assert!(analysis.stored_effects(id(TOP)).is_none());
        }
    }
}

#[test]
fn an_unknown_write_cannot_hide_an_observed_replacement_packet() {
    let mut types = types();
    types[0].shape = StructuralTypeShape::TypeParameter;
    let mut machine = mutable_parameter(LEAF, types[0].id);
    let packet = DescriptorLocation {
        root: machine.places[0].place,
        path: vec![DescriptorPathSegment::UnknownContents],
    };
    machine.unresolved_contents_writes.insert(point(0));
    machine.originations.insert(
        point(0),
        vec![(packet.clone(), DescriptorCallOrigin::Empty)],
    );
    let graph = [machine];
    let mut replay = replay(&types, &graph, None);
    let states = replay.states(id(LEAF), 0).unwrap();
    assert_eq!(
        replay.read(&graph[0], &states[&point(1)], &packet),
        Some(BTreeSet::from([
            crate::descriptors::replay::engine::Origin::Backing(DescriptorCallOrigin::Empty)
        ])),
    );
    assert!(replay.summary(id(LEAF), 0).is_none());
}

#[test]
fn an_adopted_summary_is_the_one_its_caller_would_replay() {
    let types = types();
    let graph = graph();
    let shared = table(&types, Vec::new());

    let mut first = replay(&types, &graph, Some(&shared));
    let answer = first.summary(id(TOP), 0).as_ref().map(shape);
    assert!(answer.as_ref().is_some_and(|(slots, _, _)| slots.is_some()));
    assert_eq!((answer, left(&first)), unshared(&types, &graph, TOP, 0));
    assert_eq!(shared.get(id(LEAF)).unwrap().height, Some(0));
    assert_eq!(shared.get(id(MIDDLE)).unwrap().height, Some(1));
    assert_eq!(&*shared.get(id(MIDDLE)).unwrap().callees, &[id(LEAF)]);
    assert_eq!(shared.get(id(TOP)).unwrap().height, Some(2));
    assert!(shared.get(id(OTHER_TOP)).is_none());

    // The second top machine adopts the middle machine and its leaf rather
    // than replaying their states, and keeps what a lone replay keeps.
    let mut second = replay(&types, &graph, Some(&shared));
    let answer = second.summary(id(OTHER_TOP), 0).as_ref().map(shape);
    assert_eq!(
        (answer, left(&second)),
        unshared(&types, &graph, OTHER_TOP, 0)
    );
    assert!(second.snapshots.contains_key(&id(OTHER_TOP)));
    assert!(!second.snapshots.contains_key(&id(MIDDLE)));
    assert!(!second.snapshots.contains_key(&id(LEAF)));
    // States it later needs for an adopted machine replay from the adopted
    // callee summaries, as after `release_storage_snapshots`.
    assert!(second.prepare_states(id(MIDDLE)).is_some());
}

#[test]
fn an_adoption_that_could_reach_the_depth_bound_replays_instead() {
    let types = types();
    let graph = graph();
    let shared = table(&types, Vec::new());
    replay(&types, &graph, Some(&shared)).summary(id(TOP), 0);

    // The middle machine replays states one call below itself. From 126 a
    // lone replay stays within the bound; from 127 its leaf reaches it.
    for (depth, adopts) in [(126, true), (127, false)] {
        let mut late = replay(&types, &graph, Some(&shared));
        let answer = late.summary(id(MIDDLE), depth).as_ref().map(shape);
        let expected = unshared(&types, &graph, MIDDLE, depth);
        assert_eq!(expected.0.is_some(), adopts);
        assert_eq!((answer, left(&late)), expected);
        assert_eq!(late.snapshots.contains_key(&id(MIDDLE)), !adopts);
    }
}

#[test]
fn a_callee_unavailable_here_refuses_adoption() {
    let types = types();
    let graph = graph();
    let shared = table(&types, Vec::new());
    replay(&types, &graph, Some(&shared)).summary(id(TOP), 0);

    // An analysis that recorded the leaf as unavailable, or as in progress,
    // answers the middle machine from that record, not from the table.
    let prior: [fn(&mut Replay<'_>); 2] = [
        |replay| {
            replay.summaries.insert(id(LEAF), None);
        },
        |replay| {
            replay
                .snapshots
                .insert(id(LEAF), PreparedStateCache::Preparing);
        },
    ];
    for record in prior {
        let mut alone = replay(&types, &graph, None);
        record(&mut alone);
        let expected = alone.summary(id(MIDDLE), 0).as_ref().map(shape);
        assert!(expected.is_none());
        let mut sharing = replay(&types, &graph, Some(&shared));
        record(&mut sharing);
        let answer = sharing.summary(id(MIDDLE), 0).as_ref().map(shape);
        assert_eq!((answer, left(&sharing)), (expected, left(&alone)));
    }
}

#[test]
fn only_rows_the_table_was_replayed_from_share_it() {
    let types = types();
    let graph = graph();
    let shared = table(&types, Vec::new());
    let other_types = types.clone();
    assert!(
        DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph)
            .share_summaries(&shared, NOMINAL)
    );
    // Other declarations, another slot kind, or a closure with a dynamic call.
    assert!(
        !DescriptorAnalysis::with_borrowed_storage_slots(&other_types, &graph)
            .share_summaries(&shared, NOMINAL)
    );
    assert!(
        !DescriptorAnalysis::with_reference_slots(&types, &graph).share_summaries(&shared, NOMINAL)
    );
    let dynamic = table(&types, vec![id(MIDDLE)]);
    assert!(
        !DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph)
            .share_summaries(&dynamic, NOMINAL)
    );
    // An analysis that already replayed keeps its own summaries.
    let mut started = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
    assert!(started.result_slots(id(LEAF)).is_some());
    assert!(!started.share_summaries(&shared, NOMINAL));
}

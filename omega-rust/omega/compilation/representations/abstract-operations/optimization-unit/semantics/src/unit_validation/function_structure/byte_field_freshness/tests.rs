//! Mutation controls isolate current-CFG freshness from source provenance checks.
use super::validate;
use crate::tests::indexed_byte_fields::indexed_field_unit;
use crate::tests::support::id;
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::{OptimizationBlock, OptimizationEdge};
use semantic_vocabulary::{BlockId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralAccess;

#[test]
fn view_backed_calls_invalidate_only_potentially_overlapping_fields() {
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
    use terminal_psi::{StructuralArgument, StructuralPathSegment as S};

    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let root = function.structural_parameters[0].place;
    let element = function.structural_parameters[0].structural_type;
    let view = id(990, PlaceId::new);
    let producer = id(991, OperationId::new);
    let mut view_type = unit.structural_types[0].clone();
    view_type.id = id(992, StructuralTypeId::new);
    view_type.shape = terminal_psi::StructuralTypeShape::ElementView { element };
    let mut types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    types.insert(view_type.id, &view_type);
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: view,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type: view_type.id,
            },
        });
    let mut establishment = function.blocks[0].nodes[0].clone();
    establishment.operation = O::EstablishElementView {
        psi_operation: producer,
        result: terminal_psi::StructuralOperationResult {
            place: view,
            structural_type: view_type.id,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        },
        destination: view,
        source: StructuralArgument {
            place: root,
            access: StructuralAccess::MutableBorrow,
            path: vec![S::Field("left".into())],
        },
        element,
    };
    function.blocks[0].nodes.push(establishment);
    let mut call = O::CallStructural {
        psi_operation: id(993, OperationId::new),
        callee: function.machine,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place: view,
            access: StructuralAccess::MutableBorrow,
            path: vec![S::FixedIndex(0)],
        }],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
    };
    // Exercise current-IR mutation classification, not shape admission: the
    // catalog and ownership validators independently check these projections.
    let observed = vec![
        S::Field("left".into()),
        S::FixedIndex(0),
        S::Field("out".into()),
    ];
    for (path, invalidated) in [
        (observed.clone(), true),
        (
            vec![
                S::Field("right".into()),
                S::FixedIndex(0),
                S::Field("out".into()),
            ],
            false,
        ),
        (
            vec![
                S::Field("left".into()),
                S::FixedIndex(1),
                S::Field("out".into()),
            ],
            false,
        ),
    ] {
        assert_eq!(
            super::super::structural_freshness::structural_metadata_may_change(
                &function, &types, root, &path, &call
            ),
            invalidated
        );
    }
    let O::CallStructural {
        structural_arguments,
        ..
    } = &mut call
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::SharedBorrow;
    assert!(
        !super::super::structural_freshness::structural_metadata_may_change(
            &function, &types, root, &observed, &call
        )
    );
    let O::CallStructural {
        structural_arguments,
        ..
    } = &mut call
    else {
        unreachable!()
    };
    structural_arguments[0].access = StructuralAccess::MutableBorrow;
    // A missing current producer must not turn an unresolved alias into a
    // separate owner, even when its declaration still names the old producer.
    function.blocks[0].nodes.pop();
    assert!(
        super::super::structural_freshness::structural_metadata_may_change(
            &function, &types, root, &observed, &call
        )
    );
}

#[test]
fn primitive_writes_invalidate_only_potentially_overlapping_projections() {
    use crate::tests::indexed_byte_fields::indexed_field_with_primitive_sibling_unit;
    use terminal_psi::StructuralPathSegment as S;

    let unit = indexed_field_with_primitive_sibling_unit();
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let primitive = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, O::WriteOnlyPrimitiveStore { .. }))
        .unwrap();
    let O::WriteOnlyPrimitiveStore {
        destination,
        path: written,
        ..
    } = &primitive.operation
    else {
        unreachable!();
    };
    assert!(matches!(
        written.as_slice(),
        [S::Field(_), S::RuntimeIndex { .. }]
    ));
    let root = destination.place();
    let field = written[0].clone();
    let other_runtime = S::RuntimeIndex {
        index: id(991, semantic_vocabulary::ValueId::new),
        obligation: id(992, semantic_vocabulary::ObligationId::new),
    };
    // These isolate mutation classification; complete operation typing and
    // runtime-index obligation checks remain the unit validator's job.
    for (observed, invalidated) in [
        (vec![S::Field("out".into())], false),
        (vec![], true),
        (vec![field.clone()], true),
        (written.clone(), true),
        (vec![field.clone(), S::FixedIndex(0)], true),
        (vec![field, other_runtime], true),
    ] {
        assert_eq!(
            super::super::structural_freshness::structural_metadata_may_change(
                function,
                &types,
                root,
                &observed,
                &primitive.operation
            ),
            invalidated,
            "observed projection: {observed:?}"
        );
    }
    let mut whole_root = primitive.operation.clone();
    let O::WriteOnlyPrimitiveStore { path, .. } = &mut whole_root else {
        unreachable!()
    };
    path.clear();
    assert!(
        super::super::structural_freshness::structural_metadata_may_change(
            function,
            &types,
            root,
            &[S::Field("out".into())],
            &whole_root
        )
    );
    assert!(
        !super::super::structural_freshness::structural_metadata_may_change(
            function,
            &types,
            id(999, semantic_vocabulary::PlaceId::new),
            &[],
            &whole_root
        )
    );
}

#[test]
fn indexed_byte_field_freshness_preserves_only_length_preserving_effects() {
    let unit = indexed_field_unit();
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let block = function
        .blocks
        .iter()
        .position(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    node.operation,
                    O::StructuralByteSequenceFieldByteStore { .. }
                )
            })
        })
        .unwrap();
    let store = function.blocks[block]
        .nodes
        .iter()
        .position(|node| {
            matches!(
                node.operation,
                O::StructuralByteSequenceFieldByteStore { .. }
            )
        })
        .unwrap();
    let mut replacements = function.blocks[block]
        .nodes
        .iter()
        .filter(|node| matches!(node.operation, O::StructuralByteSequenceFieldStore { .. }));
    let overlapping = replacements.next().unwrap().clone();
    let sibling = replacements.next().unwrap().clone();
    let indexed = function.blocks[block].nodes[store].clone();
    let O::StructuralByteSequenceFieldByteStore { destination, .. } = indexed.operation else {
        unreachable!();
    };
    for (intervening, accepted) in [
        (overlapping, false),
        (sibling, true),
        (indexed.clone(), true),
    ] {
        let mut changed = function.clone();
        changed.blocks[block].nodes.insert(store, intervening);
        assert_eq!(
            validate(&changed, &types, &BTreeMap::new()).is_ok(),
            accepted
        );
    }
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
        StructuralAccess::Owned,
    ] {
        let mut changed = function.clone();
        let mut call = indexed.clone();
        call.operation = O::CallStructural {
            psi_operation: id(991, OperationId::new),
            callee: function.machine,
            arguments: Vec::new(),
            structural_arguments: vec![terminal_psi::StructuralArgument {
                place: destination,
                access,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        };
        changed.blocks[block].nodes.insert(store, call);
        assert_eq!(
            validate(&changed, &types, &BTreeMap::new()).is_ok(),
            access == StructuralAccess::SharedBorrow
        );
    }
    let mut changed = function.clone();
    let mut unknown = indexed;
    unknown.operation = O::PortWrite {
        psi_operation: id(992, OperationId::new),
        service: id(993, semantic_vocabulary::ServiceId::new),
        port: 0,
        value: 0,
    };
    changed.blocks[block].nodes.insert(store, unknown);
    assert!(validate(&changed, &types, &BTreeMap::new()).is_err());
}

#[test]
fn indexed_byte_field_length_freshness_examines_backedges_and_missing_arrivals() {
    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let types = unit
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();
    let entry = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry)
        .unwrap();
    let store = function.blocks[entry]
        .nodes
        .iter()
        .position(|node| {
            matches!(
                node.operation,
                O::StructuralByteSequenceFieldByteStore { .. }
            )
        })
        .unwrap();
    let replacement = function.blocks[entry]
        .nodes
        .iter()
        .find(|node| matches!(node.operation, O::StructuralByteSequenceFieldStore { .. }))
        .unwrap()
        .clone();
    let indexed = function.blocks[entry].nodes[store].clone();
    let loop_block = id(994, BlockId::new);
    let edge = |identity| OptimizationEdge {
        psi_edge: id(identity, semantic_vocabulary::EdgeId::new),
        target: loop_block,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance: Vec::new(),
        fuel: Vec::new(),
    };
    // Keep the exact source observation in entry and put the indexed write
    // behind both entry and a cyclic arrival. These tests exercise the freshness
    // algorithm directly; complete CFG/provenance validation has separate owners.
    function.blocks[entry].nodes.truncate(store);
    function.blocks[entry].nodes.last_mut().unwrap().successors = vec![edge(995)];
    let mut backedge = indexed.clone();
    backedge.successors = vec![edge(996)];
    function.blocks.push(OptimizationBlock {
        id: loop_block,
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        nodes: vec![indexed, backedge],
    });
    let predecessors = BTreeMap::from([(loop_block, BTreeSet::from([function.entry, loop_block]))]);
    validate(&function, &types, &predecessors).unwrap();
    let loop_position = function.blocks.len() - 1;
    function.blocks[loop_position].nodes[1].operation = replacement.operation;
    assert!(validate(&function, &types, &predecessors).is_err());
    let missing = BTreeMap::from([(loop_block, BTreeSet::new())]);
    assert!(validate(&function, &types, &missing).is_err());
}

#[test]
fn indexed_descriptor_replacement_retires_the_previous_length() {
    let unit = crate::tests::indexed_byte_fields::indexed_field_source_unit(
        r#"
        domain [u8]::Utf8(bytes) requires valid_utf8(bytes);
        data Room { name: &[u8] in Utf8; }
        data Record { rooms: [Room; 3]; }
        machine Record::replace(root: &mut Record) {
            let index: u64 = 1;
            let room_slice: &mut [Room] = root.rooms.as_mut_slice();
            room_slice[index].name = "expected";
            transition room_slice[index].name == "expected" {
                true -> finish(root)
                false -> fail(root)
            }
            state finish(root: &mut Record) { }
            state fail(root: &mut Record) { }
        }
    "#,
    );
    crate::validate_psi_optimization_unit(&unit).expect("actual indexed descriptor source admits");
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let types = unit
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect::<BTreeMap<_, _>>();
    let mut predecessors = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    for block in &function.blocks {
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            predecessors
                .entry(edge.target)
                .or_default()
                .insert(block.id);
        }
    }
    validate(&function, &types, &predecessors).unwrap();
    let replacement = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, O::StructuralByteViewFieldStore { .. }))
        .unwrap()
        .clone();
    let (block, position) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, row)| {
            row.nodes
                .iter()
                .position(|node| {
                    matches!(node.operation, O::StructuralByteSequenceFieldRead { .. })
                })
                .map(|position| (block, position))
        })
        .unwrap();
    // Keep the original observation and byte-bound fact, but interpose a
    // descriptor replacement immediately before its read. This isolates
    // freshness from independent provenance/operation identity validation.
    function.blocks[block].nodes.insert(position, replacement);
    assert!(
        validate(&function, &types, &predecessors).is_err(),
        "a replacement can change extent even when the same carrier and selector survive"
    );
}

/// Isolate current-graph receiving freshness from immutable source provenance.
/// The declared entry owners dominate two ordinary control arms; each arm then
/// binds one owner to a named context, followed by an empty-telescope edge.
fn forwarded_field_observation(
    different_backing: bool,
) -> (
    optimization_unit::PsiOptimizationUnit,
    optimization_unit::PsiOptimizationFunction,
    BTreeMap<BlockId, BTreeSet<BlockId>>,
) {
    use semantic_vocabulary::{PlaceId, StructuralPlaceKind};

    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap()
        .clone();
    let root = function.structural_parameters[0].place;
    let other = id(980, PlaceId::new);
    let context = id(981, PlaceId::new);
    let left = id(982, BlockId::new);
    let right = id(983, BlockId::new);
    let joined = id(984, BlockId::new);
    let use_block = id(985, BlockId::new);
    let mut other_parameter = function.structural_parameters[0].clone();
    other_parameter.place = other;
    other_parameter.position = 1;
    other_parameter.is_self = false;
    function.structural_parameters.push(other_parameter.clone());
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: other,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    let mut context_parameter = other_parameter;
    context_parameter.place = context;
    context_parameter.position = 0;
    function
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: context,
            kind: StructuralPlaceKind::BlockParameter {
                block: joined,
                position: 0,
            },
        });
    let entry_position = function
        .blocks
        .iter()
        .position(|block| block.id == function.entry)
        .unwrap();
    let store_position = function.blocks[entry_position]
        .nodes
        .iter()
        .position(|node| {
            matches!(
                node.operation,
                O::StructuralByteSequenceFieldByteStore { .. }
            )
        })
        .unwrap();
    let mut indexed = function.blocks[entry_position].nodes[store_position].clone();
    let O::StructuralByteSequenceFieldByteStore { destination, .. } = &mut indexed.operation else {
        unreachable!()
    };
    *destination = context;
    indexed.successors.clear();
    let edge = |identity, target, argument: Option<PlaceId>| OptimizationEdge {
        psi_edge: id(identity, semantic_vocabulary::EdgeId::new),
        target,
        bindings: Vec::new(),
        structural_bindings: argument
            .map(|place| abstract_operations::AbstractStructuralBinding {
                parameter: context,
                argument: terminal_psi::StructuralArgument {
                    place,
                    access: StructuralAccess::MutableBorrow,
                    path: Vec::new(),
                },
            })
            .into_iter()
            .collect(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
        provenance: Vec::new(),
        fuel: Vec::new(),
    };
    let mut control = indexed.clone();
    control.operation = O::ReturnUnit {
        psi_edge: id(986, semantic_vocabulary::EdgeId::new),
        cleanup_actions: Vec::new(),
    };
    let mut entry = function.blocks[entry_position].clone();
    entry.nodes.truncate(store_position);
    entry.nodes.last_mut().unwrap().successors =
        vec![edge(987, left, None), edge(988, right, None)];
    let mut block = |identity, successors| {
        control.successors = successors;
        OptimizationBlock {
            id: identity,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
            nodes: vec![control.clone()],
        }
    };
    let left_block = block(left, vec![edge(989, joined, Some(root))]);
    let right_block = block(
        right,
        vec![edge(
            990,
            joined,
            Some(if different_backing { other } else { root }),
        )],
    );
    let mut joined_block = block(joined, vec![edge(991, use_block, None)]);
    joined_block.structural_parameters.push(context_parameter);
    function.blocks = vec![
        entry,
        left_block,
        right_block,
        joined_block,
        OptimizationBlock {
            id: use_block,
            structural_parameters: Vec::new(),
            parameters: Vec::new(),
            nodes: vec![indexed],
        },
    ];
    let predecessors = BTreeMap::from([
        (left, BTreeSet::from([function.entry])),
        (right, BTreeSet::from([function.entry])),
        (joined, BTreeSet::from([left, right])),
        (use_block, BTreeSet::from([joined])),
    ]);
    (unit, function, predecessors)
}

#[test]
fn forwarded_byte_field_observation_crosses_empty_control_edges() {
    let (unit, function, predecessors) = forwarded_field_observation(false);
    let types = unit
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    validate(&function, &types, &predecessors).expect(
        "a named context keeps its dominating field observation through ordinary control edges",
    );
}

#[test]
fn forwarded_byte_field_observation_rejects_another_backing_on_one_arrival() {
    let (unit, function, predecessors) = forwarded_field_observation(true);
    let types = unit
        .structural_types
        .iter()
        .map(|row| (row.id, row))
        .collect();
    assert!(
        validate(&function, &types, &predecessors).is_err(),
        "executing the observation on every arrival does not establish another owner's extent"
    );
}

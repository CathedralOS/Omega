//! Shared child observations freeze their exact parent region until the final use.

use super::address_joins::{STATE_RECORD_VIEW_SOURCE, validate_address_join_ownership};
use crate::tests::structural_cases::verified_source_machine_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;

fn current_child_source() -> String {
    STATE_RECORD_VIEW_SOURCE.replace(
        "view.len == 4 && packet.items[0] == 70 && packet.items[3] == 5 && packet.marker == 0",
        "view.len == 4",
    )
}

#[test]
fn address_join_fresh_child_observation_keeps_current_parent_authority() {
    let candidate = verified_source_machine_unit(&current_child_source(), "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the fresh shared child belongs to the current exclusive arrival");
}

#[test]
fn address_join_shared_child_freezes_until_the_last_observation() {
    let original = verified_source_machine_unit(&current_child_source(), "main");
    validate_psi_optimization_unit(&original).unwrap();
    for after_observation in [false, true] {
        let mut candidate = original.clone();
        let (block, parent) = candidate.functions[0]
            .blocks
            .iter()
            .enumerate()
            .find_map(|(position, block)| {
                block.nodes.iter().find_map(|node| match &node.operation {
                    O::EstablishElementView { source, .. } => Some((position, source.place)),
                    _ => None,
                })
            })
            .unwrap();
        let nodes = &mut candidate.functions[0].blocks[block].nodes;
        let call = nodes
            .iter()
            .position(|node| {
                matches!(&node.operation,
                    O::CallStructural { structural_arguments, .. }
                    if structural_arguments.iter().any(|argument| argument.place == parent)
                )
            })
            .expect("stamp mutates the same parent array");
        let stamp = nodes.remove(call);
        let insertion = nodes
            .iter()
            .position(|node| {
                if after_observation {
                    matches!(node.operation, O::ByteSequenceLength { .. })
                } else {
                    matches!(node.operation, O::EstablishElementView { .. })
                }
            })
            .unwrap()
            + 1;
        nodes.insert(insertion, stamp);
        for position in 0..nodes.len() {
            refresh_node_derivatives(&mut candidate, 0, block, position);
        }
        let result = validate_address_join_ownership(&candidate);
        if after_observation {
            result.expect("the last scalar observation ends this nonescaping cohort");
        } else {
            assert!(
                matches!(result,
                    Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { place, .. })
                    if place == parent
                ),
                "the child still needs its parent's array: {result:?}"
            );
        }
    }
}

#[test]
fn address_join_shared_child_does_not_freeze_a_sibling_field() {
    let source = current_child_source().replace(
        "let view: &[u8] = packet.items.as_slice();",
        "let view: &[u8] = packet.items.as_slice(); packet.marker = 9;",
    );
    let candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the child captures items, while marker is a disjoint sibling");
}

#[test]
fn address_join_read_only_call_does_not_escape_child() {
    let source = format!(
        "machine observed_length(values: &[u8]) -> u64 {{ transition {{ _ -> (values.len) }} }}\n{}",
        current_child_source().replace(
            "transition view.len == 4",
            "let length: u64 = observed_length(view); transition length == 4"
        )
    );
    let candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate)
        .expect("the completed read-only call leaves no escaping descriptor at owner cleanup");
}

fn closed_call_candidate(writing_callee: bool) -> optimization_unit::PsiOptimizationUnit {
    closed_call_with_observer("machine observe(values: &mut [u8]) { }", writing_callee)
}

fn closed_call_with_observer(
    observer: &str,
    writing_callee: bool,
) -> optimization_unit::PsiOptimizationUnit {
    let source = format!(
        "{observer}\n{}",
        current_child_source().replace(
            "stamp(&mut packet.items);",
            "observe(&mut packet.items); stamp(&mut packet.items);"
        )
    );
    let mut candidate = verified_source_machine_unit(&source, "main");
    validate_psi_optimization_unit(&candidate).unwrap();
    let block = candidate.functions[0]
        .blocks
        .iter()
        .position(|block| {
            block
                .nodes
                .iter()
                .any(|node| matches!(node.operation, O::EstablishElementView { .. }))
        })
        .unwrap();
    let nodes = &mut candidate.functions[0].blocks[block].nodes;
    let calls = nodes
        .iter()
        .enumerate()
        .filter_map(|(position, node)| match node.operation {
            O::CallStructural { callee, .. } => Some((position, callee)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].1, calls[1].1);
    let mut observation = nodes.remove(calls[0].0);
    if writing_callee && let O::CallStructural { callee, .. } = &mut observation.operation {
        *callee = calls[1].1;
    }
    let formation = nodes
        .iter()
        .position(|node| matches!(node.operation, O::EstablishElementView { .. }))
        .unwrap();
    nodes.insert(formation + 1, observation);
    for position in 0..nodes.len() {
        refresh_node_derivatives(&mut candidate, 0, block, position);
    }
    candidate
}

#[test]
fn closed_call_does_not_disturb_an_unwritten_mutable_input() {
    validate_address_join_ownership(&closed_call_candidate(false))
        .expect("the current ordinary callee has no write through its mutable formal");
}

#[test]
fn closed_call_uses_the_current_callee_writes() {
    assert!(
        matches!(
            validate_address_join_ownership(&closed_call_candidate(true)),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "a rewritten same-signature callee writes the live child's backing"
    );
}

#[test]
fn closed_call_boundary_without_structural_actuals_preserves_live_child() {
    let candidate = closed_call_with_observer(
        "machine observe(values: &mut [u8]) reaches Host { Host::exit(0); }",
        false,
    );
    validate_address_join_ownership(&candidate)
        .expect("the boundary receives no authority over the live child's backing");
}

#[test]
fn closed_call_boundary_exclusive_actual_disturbs_live_child() {
    let candidate = closed_call_with_observer(
        r#"
            boundary trait Input { machine read(values: &mut [u8]); }
            machine observe(values: &mut [u8]) reaches Input { Input::read(values); }
        "#,
        false,
    );
    assert!(
        candidate
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .any(|node| matches!(
                &node.operation,
                O::BoundaryCall { structural_arguments, .. }
                    if structural_arguments.iter().any(|argument|
                        argument.access == terminal_psi::StructuralAccess::MutableBorrow)
            )),
        "the actual boundary retains the callee's exclusive input"
    );
    assert!(
        matches!(
            validate_address_join_ownership(&candidate),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "a boundary may write every byte of its exact exclusive actual"
    );
}

fn guarded_window_unit() -> optimization_unit::PsiOptimizationUnit {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../../tests/omega/pass/borrows/borrow_guarded_window_write_and_call/main.omg"
    ));
    let source = source.split("machine main()").next().unwrap();
    let mut unit =
        crate::tests::structural_cases::source_machine_unit(source, "write_outside_window", true);
    let position = unit
        .functions
        .iter()
        .position(|function| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| matches!(node.operation, O::ElementViewSubslice { .. }))
        })
        .unwrap();
    unit.functions.swap(0, position);
    unit
}

#[test]
fn guarded_window_accepts_proven_disjoint_store_and_call() {
    validate_address_join_ownership(&guarded_window_unit())
        .expect("the current guard remains true through both simultaneous state bindings");
}

#[test]
fn guarded_window_rejects_overlapping_store_or_call() {
    for mutate_call in [false, true] {
        let mut unit = guarded_window_unit();
        let function = &mut unit.functions[0];
        let boundary = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find_map(|node| match node.operation {
                O::ElementViewSubslice { start, .. } => Some(start),
                _ => None,
            })
            .unwrap();
        let path = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
            .find_map(|node| match &mut node.operation {
                O::CallStructural {
                    structural_arguments,
                    ..
                } if mutate_call => structural_arguments
                    .first_mut()
                    .map(|argument| &mut argument.path),
                O::WriteOnlyPrimitiveStore { path, .. }
                | O::StructuralScalarFieldStore { path, .. }
                    if !mutate_call =>
                {
                    Some(path)
                }
                _ => None,
            })
            .unwrap();
        let index = path
            .iter_mut()
            .find_map(|segment| match segment {
                terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. } => Some(index),
                _ => None,
            })
            .unwrap();
        *index = boundary;
        assert!(
            matches!(
                validate_address_join_ownership(&unit),
                Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
            ),
            "an overlapping {} must remain frozen",
            if mutate_call { "call" } else { "store" }
        );
    }
}

#[test]
fn guarded_window_does_not_reuse_a_removed_comparison() {
    let mut unit = guarded_window_unit();
    let operation = unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| {
            matches!(node.operation, O::IntegerLessThan { .. }).then_some(&mut node.operation)
        })
        .unwrap();
    let O::IntegerLessThan { left, right, .. } = operation else {
        unreachable!()
    };
    std::mem::swap(left, right);
    assert!(
        matches!(
            validate_address_join_ownership(&unit),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "immutable source evidence cannot authorize a rewritten comparison"
    );
}

#[test]
fn guarded_window_empty_range_keeps_whole_backing_pinned() {
    let mut unit = guarded_window_unit();
    for node in unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
    {
        if let O::ElementViewSubslice { start, end, .. } = &mut node.operation {
            *end = *start;
        }
    }
    validate_address_join_ownership(&unit)
        .expect("the write precedes the captured start independently of the window's extent");
    let call = unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            O::CallStructural {
                structural_arguments,
                ..
            } => structural_arguments.first_mut(),
            _ => None,
        })
        .unwrap();
    call.path.clear();
    assert!(
        matches!(
            validate_address_join_ownership(&unit),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "empty windows retain backing; no selector relation authorizes a whole-root mutation"
    );
}

#[test]
fn guarded_window_rejects_duplicate_descriptor_disposal() {
    let mut unit = guarded_window_unit();
    let function = &mut unit.functions[0];
    let descriptor = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match node.operation {
            O::ElementViewSubslice { ref result, .. } => Some(result.place),
            _ => None,
        })
        .unwrap();
    for node in function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
    {
        if let O::Return {
            cleanup_actions, ..
        } = &mut node.operation
        {
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                descriptor,
            ));
        }
    }
    assert!(
        matches!(
            validate_address_join_ownership(&unit),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ),
        "an affine descriptor carrier must be disposed exactly once"
    );
}

fn mutable_field_range_unit() -> optimization_unit::PsiOptimizationUnit {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../../tests/omega/pass/slices/mutable_field_range_call_state_exit/main.omg"
    ));
    verified_source_machine_unit(source, "main")
}

/// The same source fixture crosses a mutable call and a mutable state edge;
/// the caller then reads the backing while descriptor cleanup is still owed.
#[test]
fn mutable_field_range_call_restores_backing_access_before_descriptor_cleanup() {
    validate_psi_optimization_unit(&mutable_field_range_unit())
        .expect("the completed mutable calls restore the caller's backing access");
}

#[test]
fn mutable_field_range_later_observation_prevents_parent_read_restoration() {
    let mut candidate = mutable_field_range_unit();
    validate_psi_optimization_unit(&candidate).unwrap();
    let function = &mut candidate.functions[0];
    let descriptor = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::ElementViewSubslice { result, .. } => Some(result.place),
            _ => None,
        })
        .expect("the actual caller's narrowed mutable view");
    let mut observation = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, O::ElementViewLength { .. }))
        .expect("the existing element-length operation shape")
        .clone();
    let O::ElementViewLength {
        psi_operation,
        result,
        source,
    } = &mut observation.operation
    else {
        unreachable!()
    };
    *psi_operation = semantic_vocabulary::OperationId::new(10_000).unwrap();
    result.value = semantic_vocabulary::ValueId::new(10_000).unwrap();
    *source = descriptor;
    let (block, insertion) = function.blocks.iter().enumerate().find_map(|(block, body)| {
        body.nodes.iter().position(|node| matches!(&node.operation,
            O::ReturnUnit { cleanup_actions, .. }
                if cleanup_actions.contains(&terminal_psi::TerminalAffineCleanupAction::DiscardRoot(descriptor))
        )).map(|position| (block, position))
    }).expect("the caller still owes this descriptor at its normal exit");
    function.blocks[block].nodes.insert(insertion, observation);
    for position in insertion..candidate.functions[0].blocks[block].nodes.len() {
        refresh_node_derivatives(&mut candidate, 0, block, position);
    }
    assert!(
        matches!(
            validate_address_join_ownership(&candidate),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "a real later descriptor observation keeps its parent's reads suspended"
    );
}

/// Build a current optimization graph directly, without source lowering or a
/// retained producer origin table. The callee returns its shared input; the
/// caller observes that returned descriptor and writes its mutable backing.
fn returned_descriptor_unit() -> optimization_unit::PsiOptimizationUnit {
    use crate::tests::support::id;
    use abstract_operations::{
        AbstractFunctionResult, AbstractResult, AbstractStructuralCallResult,
    };
    use semantic_vocabulary::{
        EdgeId, IntegerType, IntegerValue, ObligationId, OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};

    let mut unit = crate::tests::fixtures::structural_catalog::structural_result_call_unit();
    let input = unit.functions[0].structural_parameters[0].place;
    for function in &mut unit.functions {
        function.entry_claim_declarations.clear();
        function.content_entry_claims.clear();
        function.entry_claims.clear();
        function.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        function.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        if let AbstractFunctionResult::Structural(result) = &mut function.result {
            result.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        for node in &mut function.blocks[0].nodes {
            match &mut node.operation {
                O::CallStructural {
                    structural_arguments,
                    claim_transfers,
                    result:
                        AbstractStructuralCallResult::Structural {
                            result,
                            returned_claim_transfers,
                            ..
                        },
                    ..
                } => {
                    structural_arguments[0].access = StructuralAccess::SharedBorrow;
                    claim_transfers.clear();
                    returned_claim_transfers.clear();
                    result.claims.clear();
                    result.multiplicity = StructuralMultiplicity::Unrestricted;
                }
                O::ReturnStructural {
                    returned_claims, ..
                } => returned_claims.clear(),
                _ => {}
            }
        }
    }
    // The carrier declaration permits borrowed access; authority is the exact
    // parameter access and independently reconstructed result relation.
    unit.structural_types.make_mut()[0].shape = terminal_psi::StructuralTypeShape::ByteSequence(
        terminal_psi::ByteSequenceCarrier::BorrowedView { access: None },
    );
    unit.functions[0].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    unit.functions[0].result = AbstractFunctionResult::Unit;
    let O::CallStructural {
        result: AbstractStructuralCallResult::Structural { result, .. },
        ..
    } = &unit.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    let returned = result.place;
    let zero = id(901, ValueId::new);
    let byte = id(902, ValueId::new);
    let length = id(903, ValueId::new);
    let template = unit.functions[0].blocks[0].nodes[0].clone();
    let call = template.operation.clone();
    let operations = vec![
        O::IntegerConstant {
            psi_operation: id(901, OperationId::new),
            result: zero,
            scalar_type: ScalarType::Integer(
                IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).unwrap(),
            ),
            value: IntegerValue::Unsigned(0),
        },
        O::IntegerConstant {
            psi_operation: id(902, OperationId::new),
            result: byte,
            scalar_type: ScalarType::Integer(
                IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8).unwrap(),
            ),
            value: IntegerValue::Unsigned(7),
        },
        O::ByteSequenceLength {
            psi_operation: id(903, OperationId::new),
            source: input,
            result: AbstractResult {
                value: length,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
        },
        call,
        O::ByteSequenceLength {
            psi_operation: id(904, OperationId::new),
            source: returned,
            result: AbstractResult {
                value: id(904, ValueId::new),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
        },
        O::ByteSequenceWrite {
            psi_operation: id(905, OperationId::new),
            destination: input,
            index: zero,
            value: byte,
            length,
            obligation: id(901, ObligationId::new),
        },
        O::ReturnUnit {
            psi_edge: id(906, EdgeId::new),
            cleanup_actions: Vec::new(),
        },
    ];
    unit.functions[0].blocks[0].nodes = operations
        .into_iter()
        .map(|operation| {
            let mut node = template.clone();
            node.operation = operation;
            node
        })
        .collect();
    for function in 0..unit.functions.len() {
        for node in 0..unit.functions[function].blocks[0].nodes.len() {
            refresh_node_derivatives(&mut unit, function, 0, node);
        }
    }
    unit
}

#[test]
fn returned_descriptor_releases_backing_after_last_observation() {
    validate_address_join_ownership(&returned_descriptor_unit())
        .expect("a returned shared descriptor's completed observation ends its backing loan");
}

#[test]
fn returned_descriptor_blocks_write_before_last_observation() {
    let mut unit = returned_descriptor_unit();
    validate_address_join_ownership(&unit).unwrap();
    unit.functions[0].blocks[0].nodes.swap(4, 5);
    for node in [4, 5] {
        refresh_node_derivatives(&mut unit, 0, 0, node);
    }
    assert!(
        matches!(
            validate_address_join_ownership(&unit),
            Err(OptimizationUnitValidationError::CurrentSharedJoinOriginDisturbed { .. })
        ),
        "the same current callee result still observes its original backing"
    );
}

fn shared_returned_descriptor_call_plan() -> abstract_operations::AbstractOperationPlan {
    use crate::tests::support::id;
    use abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperationPlan,
        AbstractStructuralCallResult,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess as Access, StructuralMultiplicity as Multiplicity,
        StructuralTypeShape,
    };

    let descriptor = id(1200, StructuralTypeId::new);
    let caller = id(1201, MachineId::new);
    let relay = id(1202, MachineId::new);
    let mutable = id(1203, MachineId::new);
    let backing = id(1210, PlaceId::new);
    let returned = id(1211, PlaceId::new);
    let input = id(1212, PlaceId::new);
    let mutable_input = id(1213, PlaceId::new);
    let parameter = |place, access| terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type: descriptor,
        multiplicity: Multiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let function = |machine, block, parameters, result, operations| AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: Vec::new(),
        structural_parameters: parameters,
        result,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block,
            operation_offset: 0,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        }],
        operations,
    };
    let call = |operation, callee, place, access, result| O::CallStructural {
        psi_operation: id(operation, OperationId::new),
        callee,
        result,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place,
            path: Vec::new(),
            access,
        }],
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([43; 32]),
        },
        entry: caller,
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: descriptor,
            identity: "shared-return-permission".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            function(
                caller,
                id(1220, BlockId::new),
                vec![parameter(backing, Access::MutableBorrow)],
                AbstractFunctionResult::Unit,
                vec![
                    call(
                        1230,
                        relay,
                        backing,
                        Access::SharedBorrow,
                        AbstractStructuralCallResult::Structural {
                            result: terminal_psi::StructuralOperationResult {
                                place: returned,
                                structural_type: descriptor,
                                multiplicity: Multiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                qualification_establishments: Vec::new(),
                                claims: Vec::new(),
                            },
                            returned_claim_transfers: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                    ),
                    call(
                        1231,
                        mutable,
                        backing,
                        Access::MutableBorrow,
                        AbstractStructuralCallResult::Unit,
                    ),
                    O::ReturnUnit {
                        psi_edge: id(1240, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            ),
            function(
                relay,
                id(1221, BlockId::new),
                vec![parameter(input, Access::SharedBorrow)],
                AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
                    place: id(1214, PlaceId::new),
                    structural_type: descriptor,
                    multiplicity: Multiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    reference_sources: Vec::new(),
                }),
                vec![O::ReturnStructural {
                    psi_edge: id(1241, EdgeId::new),
                    source: input,
                    returned_claims: Vec::new(),
                    trivial_affine_locals: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                }],
            ),
            function(
                mutable,
                id(1222, BlockId::new),
                vec![parameter(mutable_input, Access::MutableBorrow)],
                AbstractFunctionResult::Unit,
                vec![O::ReturnUnit {
                    psi_edge: id(1242, EdgeId::new),
                    cleanup_actions: Vec::new(),
                }],
            ),
        ],
    }
}

#[test]
fn shared_returned_descriptor_cannot_supply_mutable_backing_permission() {
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &shared_returned_descriptor_call_plan(),
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let returned = unit.functions[0].blocks[0].nodes[0]
        .operation
        .structural_result()
        .unwrap()
        .1
        .place;
    validate_psi_optimization_unit(&unit).expect("original mutable input has backing permission");
    let O::CallStructural {
        structural_arguments,
        ..
    } = &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    structural_arguments[0].place = returned;
    refresh_node_derivatives(&mut unit, 0, 0, 1);
    assert!(
        validate_psi_optimization_unit(&unit).is_err(),
        "owning a shared descriptor carrier cannot create mutable backing permission"
    );
}

#[test]
fn unused_descriptor_return_rejects_owned_backing_escape() {
    use crate::tests::support::{id, refresh_identity};
    use semantic_vocabulary::{
        BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
        PlaceId, ScalarType, StructuralTypeId,
    };
    use terminal_psi::{
        StructuralAccess as Access, StructuralMultiplicity as Multiplicity, StructuralTypeShape,
    };
    let mut plan = shared_returned_descriptor_call_plan();
    let machine = id(1400, MachineId::new);
    let block = id(1401, BlockId::new);
    let input = id(1402, PlaceId::new);
    let result_place = id(1404, PlaceId::new);
    let element = id(1407, StructuralTypeId::new);
    let array = id(1408, StructuralTypeId::new);
    plan.structural_types.make_mut().extend([
        terminal_psi::StructuralTypeDeclaration {
            id: element,
            identity: "unused-return-element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            )),
        },
        terminal_psi::StructuralTypeDeclaration {
            id: array,
            identity: "unused-return-array".into(),
            shape: StructuralTypeShape::FixedArray { element, length: 4 },
        },
    ]);
    let mut unused = plan.functions[1].clone();
    unused.machine = machine;
    unused.entry = block;
    unused.structural_parameters[0].place = input;
    unused.structural_parameters[0].structural_type = array;
    unused.block_entries[0].block = block;
    let abstract_operations::AbstractFunctionResult::Structural(signature) = &mut unused.result
    else {
        unreachable!()
    };
    signature.place = id(1403, PlaceId::new);
    unused.operations = vec![
        O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            psi_operation: id(1405, OperationId::new),
            result: terminal_psi::StructuralOperationResult {
                place: result_place,
                structural_type: signature.structural_type,
                multiplicity: Multiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            destination: result_place,
            source: terminal_psi::StructuralArgument {
                place: input,
                path: Vec::new(),
                access: Access::SharedBorrow,
            },
            element,
        },
        O::ReturnStructural {
            psi_edge: id(1406, EdgeId::new),
            source: result_place,
            returned_claims: Vec::new(),
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    plan.functions.push(unused);
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    validate_psi_optimization_unit(&unit)
        .expect("an unused function may return its incoming borrowed array backing");
    unit.functions.last_mut().unwrap().structural_parameters[0].access = Access::Owned;
    refresh_identity(&mut unit);
    assert!(
        matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { machine: rejected, .. }) if rejected == machine
        ),
        "return admission must reject local owned backing even when no call queries this function"
    );
}

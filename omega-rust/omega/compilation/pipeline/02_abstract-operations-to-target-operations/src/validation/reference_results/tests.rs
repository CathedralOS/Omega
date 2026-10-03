//! Later ordinary subtree custody must not erase earlier call-reference evidence.
use super::*;
use abstract_operations::{AbstractBlockEntry, AbstractFunctionResult};
use semantic_vocabulary::{BlockId, EdgeId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, RecordFieldInitializer, StructuralAffineDiscard, StructuralFieldDeclaration,
    StructuralParameterDeclaration, StructuralResultDeclaration,
};

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
fn kind(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
fn path(name: &str) -> Vec<StructuralPathSegment> {
    vec![StructuralPathSegment::Field(name.into())]
}
fn result(value: u64, structural_type: u64) -> StructuralOperationResult {
    StructuralOperationResult {
        place: place(value),
        structural_type: kind(structural_type),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    }
}
fn field(value: u64, name: &str, structural_type: u64) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(value).unwrap(),
        identity: name.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(kind(structural_type)),
    }
}
fn catalog() -> Vec<StructuralTypeDeclaration> {
    [
        (1, vec![]),
        (2, vec![field(1, "left", 1), field(2, "right", 1)]),
        (3, vec![field(3, "child", 1)]),
    ]
    .into_iter()
    .map(|(identity, fields)| StructuralTypeDeclaration {
        id: kind(identity),
        identity: format!("type{identity}"),
        shape: StructuralTypeShape::Record { fields },
    })
    .collect()
}
fn function() -> AbstractFunction {
    AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: BlockId::new(1).unwrap(),
        parameters: vec![],
        structural_parameters: vec![],
        result: AbstractFunctionResult::Unit,
        entry_claims: vec![],
        published_service_ceiling: vec![],
        block_entries: vec![AbstractBlockEntry {
            block: BlockId::new(1).unwrap(),
            structural_parameters: vec![],
            parameters: vec![],
            operation_offset: 0,
        }],
        operations: vec![],
    }
}
fn parameter() -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(1),
        position: 0,
        is_self: false,
        structural_type: kind(2),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: vec![],
        projected_qualifications: vec![],
    }
}
fn initializer() -> RecordFieldInitializer {
    RecordFieldInitializer {
        field: StructuralFieldId::new(3).unwrap(),
        value: RecordFieldValue::Structural(StructuralArgument {
            place: place(1),
            path: path("left"),
            access: StructuralAccess::Owned,
        }),
    }
}
fn residual() -> StructuralAffineDiscard {
    StructuralAffineDiscard {
        place: place(1),
        path: path("right"),
        structural_type: kind(1),
    }
}

#[test]
fn reference_replay_retains_earlier_call_roster_across_projected_cleanup() {
    let declarations = catalog();
    let mut caller = function();
    caller.structural_parameters.push(parameter());
    let mut callee = function();
    callee.machine = MachineId::new(2).unwrap();
    callee.result = AbstractFunctionResult::Structural(StructuralResultDeclaration {
        place: place(10),
        structural_type: kind(1),
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        reference_sources: vec![],
    });
    caller.operations = vec![
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(12).unwrap(),
            callee: callee.machine,
            arguments: vec![],
            structural_arguments: vec![],
            dynamic_arguments: vec![],
            claim_transfers: vec![],
            result: AbstractStructuralCallResult::Structural {
                result: result(3, 1),
                returned_claim_transfers: vec![],
                selected_evidence: vec![],
            },
            requirement_obligations: vec![],
            crash_continuations: vec![],
        },
        AbstractOperation::EstablishRecord {
            psi_operation: OperationId::new(13).unwrap(),
            result: result(4, 3),
            fields: vec![initializer()],
        },
        AbstractOperation::Jump {
            psi_edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(2).unwrap(),
            bindings: vec![],
            structural_bindings: vec![],
            trivial_affine_discards: vec![],
            residual_affine_discards: vec![residual()],
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(2).unwrap(),
            cleanup_actions: vec![
                TerminalAffineCleanupAction::DiscardRoot(place(4)),
                TerminalAffineCleanupAction::DiscardRoot(place(3)),
            ],
        },
    ];
    caller.block_entries.push(AbstractBlockEntry {
        block: BlockId::new(2).unwrap(),
        structural_parameters: vec![],
        parameters: vec![],
        operation_offset: 3,
    });
    for at_return in [false, true] {
        let mut source = caller.clone();
        if at_return {
            let AbstractOperation::Jump {
                residual_affine_discards,
                ..
            } = &mut source.operations[2]
            else {
                unreachable!()
            };
            residual_affine_discards.clear();
            let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = &mut source.operations[3]
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardResidual(residual()));
        }
        let calls = expected(&source, &[callee.clone()], &declarations, &BTreeMap::new())
            .expect("ordinary projected custody preserves the independent call replay");
        assert_eq!(calls.len(), 1);
        assert!(calls[&OperationId::new(12).unwrap()].is_empty());
        let AbstractOperation::EstablishRecord { fields, .. } = &mut source.operations[1] else {
            unreachable!()
        };
        let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
            unreachable!()
        };
        argument.path = path("unknown");
        assert!(expected(&source, &[callee.clone()], &declarations, &BTreeMap::new()).is_none());
    }
}

#[test]
fn reference_replay_projected_constructor_preserves_other_live_leaves() {
    let mut declarations = catalog();
    declarations.push(StructuralTypeDeclaration {
        id: kind(4),
        identity: "Reference".into(),
        shape: StructuralTypeShape::Reference {
            referent: kind(1),
            access: StructuralAccess::SharedBorrow,
        },
    });
    let StructuralTypeShape::Record { fields } = &mut declarations[1].shape else {
        unreachable!()
    };
    fields[1].field_type = StructuralFieldType::Structural(kind(4));
    let source = function();
    let mut custody = Custody::default();
    custody
        .homes
        .insert(place(1), (kind(2), StructuralMultiplicity::Affine));
    custody.leaves.insert(
        (place(1), path("right")),
        Leaf {
            root: Root::Place(place(99)),
            structural_type: kind(2),
            multiplicity: StructuralMultiplicity::Affine,
        },
    );
    establish_fields(
        &source,
        &declarations,
        &mut custody,
        &result(4, 3),
        &[initializer()],
        None,
    )
    .expect("disjoint ordinary child construction leaves the reference sibling live");
    assert_eq!(
        custody.homes.get(&place(1)),
        Some(&(kind(2), StructuralMultiplicity::Affine))
    );
    assert_eq!(custody.leaves.len(), 1);
    assert_eq!(
        custody.leaves[&(place(1), path("right"))].root.place(),
        place(99)
    );
    assert!(
        discard_residual(
            &source,
            &declarations,
            &custody,
            &StructuralAffineDiscard {
                structural_type: kind(4),
                ..residual()
            }
        )
        .is_none(),
        "an existing reference leaf cannot be silently discarded"
    );
}

#[test]
fn reference_replay_residual_rejoins_exact_type_path_and_leaf_exclusion() {
    let declarations = catalog();
    let mut source = function();
    source.structural_parameters.push(parameter());
    let mut custody = Custody::default();
    discard_residual(&source, &declarations, &custody, &residual()).unwrap();
    for mutation in 0..3 {
        let mut changed = residual();
        match mutation {
            0 => changed.path.clear(),
            1 => changed.path = path("unknown"),
            _ => changed.structural_type = kind(2),
        }
        assert!(discard_residual(&source, &declarations, &custody, &changed).is_none());
    }
    custody.leaves.insert(
        (place(1), path("right")),
        Leaf {
            root: Root::Place(place(99)),
            structural_type: kind(1),
            multiplicity: StructuralMultiplicity::Affine,
        },
    );
    assert!(discard_residual(&source, &declarations, &custody, &residual()).is_none());
    let other = StructuralAffineDiscard {
        path: path("left"),
        ..residual()
    };
    discard_residual(&source, &declarations, &custody, &other).unwrap();
    assert_eq!(custody.leaves.len(), 1);
}

//! Field-replacement window pairs in machines with an unranked cycle.
//! `MoveStructuralField` opens a restoration hole beneath a mutable-borrowed
//! root and hands out the displaced subtree; `StoreStructuralField` reseats
//! that hole. The frontier replay compares every arrival's open windows and
//! owned places against the block's established entry, so the cycle fence
//! admits a pair once the displaced value's disposal is accounted for: an
//! unrestricted value owes none, an affine value outside every cycle runs
//! once per activation, and an affine value inside a cycle must be discarded
//! or reseated there before re-entry. A runtime-selected element window is
//! admitted only for a copy payload, and an unrestricted leaf copy is the
//! swap's custody-free repair value. A hole left open across a back edge
//! still rejects at the join.

use super::{
    AdmissionProfile, Block, BlockId, ControlCycleIneligibility, EdgeId, ModuleError, Operation,
    OperationId, OperationKind, OperationResult, PlaceId, ProofBundle, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TerminalModule, Terminator, ValueDeclaration, ValueId, id, unranked_scalar_cycle,
    validate_module, verify_module_for_interpretation,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, ScalarType, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, RecordFieldInitializer, RecordFieldValue, StructuralArgument,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
    StructuralPathSegment,
};

const LOOP: u64 = 1;
const EXIT: u64 = 4;
const PREFIX: u64 = 5;
const RECEIVER: u64 = 1;

fn cell() -> StructuralTypeId {
    id(1, StructuralTypeId::new)
}

fn envelope() -> StructuralTypeId {
    id(2, StructuralTypeId::new)
}

fn flag() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn left() -> StructuralFieldId {
    id(1, StructuralFieldId::new)
}

fn right() -> StructuralFieldId {
    id(2, StructuralFieldId::new)
}

fn field(
    field: StructuralFieldId,
    identity: &str,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: field,
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

/// `unranked_scalar_cycle`'s self-looping block 1 (back edge 10, exit edge 11
/// to the returning block 4) over a mutable-borrowed `Envelope { left: Cell,
/// right: Cell }` parameter at place 1, with no operations yet.
fn window_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: cell(),
            identity: "test::Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(
                    flag(),
                    "flag",
                    StructuralFieldType::Scalar(ScalarType::Boolean),
                )],
            },
        },
        StructuralTypeDeclaration {
            id: envelope(),
            identity: "test::Envelope".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(left(), "left", StructuralFieldType::Structural(cell())),
                    field(right(), "right", StructuralFieldType::Structural(cell())),
                ],
            },
        },
    ];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: id(RECEIVER, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type: envelope(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(RECEIVER, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    module
}

fn cell_result(place: u64, multiplicity: StructuralMultiplicity) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: id(place, PlaceId::new),
        structural_type: cell(),
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn declare_cell(module: &mut TerminalModule, place: u64, producer: u64) {
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: id(place, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(producer, OperationId::new),
                structural_type: cell(),
            },
        });
}

/// `MoveStructuralField` vacating `window` on the receiver into `place`.
fn extract(
    module: &mut TerminalModule,
    operation: u64,
    window: StructuralFieldId,
    place: u64,
    multiplicity: StructuralMultiplicity,
) -> Operation {
    declare_cell(module, place, operation);
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: cell_result(place, multiplicity),
        kind: OperationKind::MoveStructuralField {
            source: id(RECEIVER, PlaceId::new),
            path: Vec::new(),
            field: Some(window),
        },
    }
}

/// `StoreStructuralField` reseating `window` on the receiver with `value`.
fn repair(operation: u64, window: StructuralFieldId, value: u64) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::StoreStructuralField {
            destination: id(RECEIVER, PlaceId::new),
            path: Vec::new(),
            field: Some(window),
            value: StructuralArgument {
                place: id(value, PlaceId::new),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        },
    }
}

/// A fresh unrestricted `Cell` at `place`, preceded by its flag constant.
fn fresh_cell(module: &mut TerminalModule, operation: u64, place: u64) -> [Operation; 2] {
    declare_cell(module, place, operation);
    let flag_value = id(operation + 100, ValueId::new);
    [
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation + 100, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: flag_value,
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        },
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(operation, OperationId::new),
            result: cell_result(place, StructuralMultiplicity::Unrestricted),
            kind: OperationKind::EstablishRecord {
                fields: vec![RecordFieldInitializer {
                    field: flag(),
                    value: RecordFieldValue::Scalar {
                        value: flag_value,
                        range_obligation: None,
                    },
                }],
            },
        },
    ]
}

/// An unrestricted copy of the receiver's `sibling` subtree at `place`.
fn copy_sibling(
    module: &mut TerminalModule,
    operation: u64,
    sibling: &str,
    place: u64,
) -> Operation {
    declare_cell(module, place, operation);
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: cell_result(place, StructuralMultiplicity::Unrestricted),
        kind: OperationKind::StructuralLeafCopy {
            source: id(RECEIVER, PlaceId::new),
            path: vec![StructuralPathSegment::Field(sibling.into())],
        },
    }
}

fn loop_block(module: &mut TerminalModule) -> &mut Block {
    module.machines[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == id(LOOP, BlockId::new))
        .expect("the self-looping block")
}

fn fence_operation(module: &TerminalModule) -> Option<OperationId> {
    match validate_module(module) {
        Err(ModuleError::ControlCycleIneligible {
            reason: ControlCycleIneligibility::BlockOperation { operation, .. },
            ..
        }) => Some(operation),
        _ => None,
    }
}

fn assert_verifies(module: &TerminalModule, reason: &str) {
    validate_module(module).unwrap_or_else(|error| panic!("{reason}: {error:?}"));
    verify_module_for_interpretation(
        module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap_or_else(|error| panic!("{reason}: {error:?}"));
}

/// Both loop edges carry the same no-code discard roster.
fn discard_on_loop_edges(module: &mut TerminalModule, place: u64) {
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut loop_block(module).terminator
    else {
        unreachable!("the loop block branches")
    };
    when_true.trivial_affine_discards = vec![id(place, PlaceId::new)];
    when_false.trivial_affine_discards = vec![id(place, PlaceId::new)];
}

#[test]
fn cyclic_unrestricted_window_pair_owes_no_disposal() {
    let mut module = window_cycle();
    let mut operations = fresh_cell(&mut module, 21, 3).to_vec();
    operations.push(extract(
        &mut module,
        20,
        left(),
        2,
        StructuralMultiplicity::Unrestricted,
    ));
    operations.push(repair(22, left(), 3));
    loop_block(&mut module).operations = operations;
    assert_verifies(&module, "an unrestricted pair re-arms each traversal");
}

#[test]
fn cyclic_copied_sibling_reseats_the_window() {
    // `self.left = self.right`: copy the sibling, vacate the target, and
    // reseat it with the copy — the swap spelling dutch_flag's loop uses.
    let mut module = window_cycle();
    let operations = vec![
        copy_sibling(&mut module, 21, "right", 3),
        extract(
            &mut module,
            20,
            left(),
            2,
            StructuralMultiplicity::Unrestricted,
        ),
        repair(22, left(), 3),
    ];
    loop_block(&mut module).operations = operations;
    assert_verifies(&module, "a copied sibling is a custody-free repair value");
}

#[test]
fn affine_window_pair_outside_the_cycle_runs_once() {
    // The prefix block runs once per activation. Its displaced value is
    // disposed either on the edge into the loop or, live for the loop's
    // whole lifetime, at the final return.
    for dispose_on_entry_edge in [true, false] {
        let mut module = window_cycle();
        let mut operations = fresh_cell(&mut module, 21, 3).to_vec();
        operations.push(extract(
            &mut module,
            20,
            left(),
            2,
            StructuralMultiplicity::Affine,
        ));
        operations.push(repair(22, left(), 3));
        let displaced = vec![id(2, PlaceId::new)];
        let machine = &mut module.machines[0];
        machine.entry = id(PREFIX, BlockId::new);
        machine.blocks.push(Block {
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: id(PREFIX, BlockId::new),
            parameters: Vec::new(),
            operations,
            terminator: Terminator::Jump {
                edge: id(13, EdgeId::new),
                target: id(LOOP, BlockId::new),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: if dispose_on_entry_edge {
                    displaced.clone()
                } else {
                    Vec::new()
                },
                residual_affine_discards: Vec::new(),
            },
        });
        if !dispose_on_entry_edge {
            let exit = machine
                .blocks
                .iter_mut()
                .find(|block| block.id == id(EXIT, BlockId::new))
                .expect("the returning block");
            let Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } = &mut exit.terminator
            else {
                unreachable!("the exit returns")
            };
            *trivial_affine_discards = displaced;
        }
        assert_verifies(
            &module,
            &format!("an affine prefix pair (disposed on entry edge: {dispose_on_entry_edge})"),
        );
    }
}

/// An affine pair in the loop block reseated with a fresh unrestricted cell.
/// Nothing disposes the displaced affine value yet.
fn cyclic_affine_pair() -> TerminalModule {
    let mut module = window_cycle();
    let mut operations = fresh_cell(&mut module, 21, 3).to_vec();
    operations.push(extract(
        &mut module,
        20,
        left(),
        2,
        StructuralMultiplicity::Affine,
    ));
    operations.push(repair(22, left(), 3));
    loop_block(&mut module).operations = operations;
    module
}

#[test]
fn cyclic_affine_displaced_value_without_disposal_keeps_the_fence() {
    let module = cyclic_affine_pair();
    assert_eq!(
        fence_operation(&module),
        Some(id(20, OperationId::new)),
        "an undisposed affine displaced value would re-enter the loop: {:?}",
        validate_module(&module)
    );
}

#[test]
fn cyclic_affine_displaced_value_discarded_in_the_cycle_verifies() {
    let mut module = cyclic_affine_pair();
    discard_on_loop_edges(&mut module, 2);
    assert_verifies(&module, "every loop edge disposes the displaced value");
}

/// The fresh and displaced values both carry affine custody. The store
/// consumes the new case; the outgoing edges dispose the previous case.
fn cyclic_affine_case_replacement() -> TerminalModule {
    let mut module = cyclic_affine_pair();
    let selected_case = id(1, semantic_vocabulary::StructuralCaseId::new);
    module.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![terminal_psi::StructuralCaseDeclaration {
            id: selected_case,
            identity: "Present".into(),
            fields: vec![field(
                flag(),
                "flag",
                StructuralFieldType::Scalar(ScalarType::Boolean),
            )],
        }],
    };
    let producer = &mut loop_block(&mut module).operations[1];
    producer.result = cell_result(3, StructuralMultiplicity::Affine);
    producer.kind = OperationKind::EstablishScalarCase {
        result_case: selected_case,
        fields: vec![terminal_psi::ScalarCaseField {
            field: flag(),
            value: id(121, ValueId::new),
            range_obligation: None,
        }],
    };
    discard_on_loop_edges(&mut module, 2);
    module
}

#[test]
fn cyclic_affine_case_construction_is_consumed_by_field_replacement() {
    let module = cyclic_affine_case_replacement();
    assert_verifies(&module, "construction and replacement share affine custody");
}

#[test]
fn cyclic_affine_case_replacement_can_follow_a_block_transfer() {
    let mut module = cyclic_affine_case_replacement();
    let producer = loop_block(&mut module);
    let operations = producer.operations.split_off(2);
    let terminator = std::mem::replace(
        &mut producer.terminator,
        Terminator::Jump {
            edge: id(30, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    module.machines[0].blocks.push(Block {
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    });
    assert_verifies(
        &module,
        "the producer dominates a separately sequenced consumer",
    );
}

#[test]
fn cyclic_affine_case_replacement_rejects_unclosed_back_edge_custody() {
    let mut module = cyclic_affine_case_replacement();
    // A consumer exists in the cycle, but one branch skips it. Eligibility
    // must not substitute that consumer's existence for the frontier join.
    let store = loop_block(&mut module)
        .operations
        .pop()
        .expect("repair store");
    let mut terminator = loop_block(&mut module).terminator.clone();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut terminator
    else {
        panic!("loop branches");
    };
    when_true.edge = id(20, EdgeId::new);
    when_false.edge = id(21, EdgeId::new);
    when_true.trivial_affine_discards.clear();
    when_false.trivial_affine_discards.clear();
    let Terminator::Conditional { when_false, .. } = &mut loop_block(&mut module).terminator else {
        panic!("loop branches");
    };
    when_false.target = id(2, BlockId::new);
    module.machines[0].blocks.push(Block {
        id: id(2, BlockId::new),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![store],
        terminator,
    });
    assert_eq!(fence_operation(&module), None);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(_))
    ));
}

#[test]
fn cyclic_affine_case_replacement_rejects_reusing_the_consumed_value() {
    let mut module = cyclic_affine_case_replacement();
    // Both windows close, but the second store tries to consume the same
    // affine result again. This must fail at custody replay, not eligibility.
    let displaced = extract(&mut module, 23, right(), 4, StructuralMultiplicity::Affine);
    loop_block(&mut module)
        .operations
        .extend([displaced, repair(24, right(), 3)]);
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut loop_block(&mut module).terminator
    else {
        panic!("loop branches");
    };
    when_true.trivial_affine_discards.push(id(4, PlaceId::new));
    when_false.trivial_affine_discards.push(id(4, PlaceId::new));
    assert_eq!(fence_operation(&module), None);
    assert!(validate_module(&module).is_err());
}

#[test]
fn cyclic_affine_swap_reseats_each_displaced_value() {
    // Vacate both fields and reseat each hole with the other's displaced
    // value: each window store consumes the value it reseats.
    let mut module = window_cycle();
    let operations = vec![
        extract(&mut module, 20, left(), 2, StructuralMultiplicity::Affine),
        extract(&mut module, 21, right(), 3, StructuralMultiplicity::Affine),
        repair(22, left(), 3),
        repair(23, right(), 2),
    ];
    loop_block(&mut module).operations = operations;
    assert_verifies(&module, "a swap consumes both displaced values");

    // Reseating only one hole leaves the other displaced value undisposed.
    let mut module = window_cycle();
    let mut operations = fresh_cell(&mut module, 24, 4).to_vec();
    operations.extend([
        extract(&mut module, 20, left(), 2, StructuralMultiplicity::Affine),
        extract(&mut module, 21, right(), 3, StructuralMultiplicity::Affine),
        repair(22, left(), 3),
        repair(23, right(), 4),
    ]);
    loop_block(&mut module).operations = operations;
    assert_eq!(
        fence_operation(&module),
        Some(id(20, OperationId::new)),
        "the unreseated left value stays fenced: {:?}",
        validate_module(&module)
    );
}

#[test]
fn a_hole_left_open_across_the_back_edge_rejects_at_the_join() {
    // The loop block vacates `left` every traversal but only the exit block
    // reseats it: the exit leaves no debt, while the back edge arrives at the
    // loop block with an open window its established entry lacks.
    let mut module = window_cycle();
    let operation = extract(
        &mut module,
        20,
        left(),
        2,
        StructuralMultiplicity::Unrestricted,
    );
    loop_block(&mut module).operations = vec![operation];
    module.machines[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == id(EXIT, BlockId::new))
        .expect("the returning block")
        .operations
        .push(repair(22, left(), 2));
    assert_eq!(
        validate_module(&module).map(|_| ()),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(id(
            LOOP,
            BlockId::new
        ))),
    );
}

/// A loop-block window over `cells[i]`, a runtime-selected element of a
/// `[Cell; 2]` field, reseated with a fresh cell. Both loop edges discard the
/// displaced value, so only the element window's own shape decides the fence.
fn element_window_cycle(multiplicity: StructuralMultiplicity) -> TerminalModule {
    let mut module = window_cycle();
    let row = id(3, StructuralTypeId::new);
    let cells = id(3, StructuralFieldId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: row,
        identity: "test::Row".into(),
        shape: StructuralTypeShape::FixedArray {
            element: cell(),
            length: 2,
        },
    });
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!("Envelope is a record")
    };
    fields.push(field(cells, "cells", StructuralFieldType::Structural(row)));
    let index = id(30, ValueId::new);
    let element = |obligation: u64| {
        vec![
            StructuralPathSegment::Field("cells".into()),
            StructuralPathSegment::RuntimeIndex {
                index,
                obligation: id(obligation, ObligationId::new),
            },
        ]
    };
    let mut operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: index,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        }),
        kind: OperationKind::IntegerConstant {
            value: semantic_vocabulary::IntegerValue::Unsigned(1),
        },
    }];
    operations.extend(fresh_cell(&mut module, 21, 3));
    let mut vacate = extract(&mut module, 20, cells, 2, multiplicity);
    let OperationKind::MoveStructuralField { path, .. } = &mut vacate.kind else {
        unreachable!("extract moves a field")
    };
    *path = element(1);
    let mut reseat = repair(22, cells, 3);
    let OperationKind::StoreStructuralField { path, .. } = &mut reseat.kind else {
        unreachable!("repair stores a field")
    };
    *path = element(2);
    operations.extend([vacate, reseat]);
    loop_block(&mut module).operations = operations;
    if multiplicity == StructuralMultiplicity::Affine {
        discard_on_loop_edges(&mut module, 2);
    }
    module
}

#[test]
fn cyclic_element_windows_admit_only_copy_payloads() {
    // A runtime element's hole is its whole array prefix, so the repair store
    // cannot prove it reseats the element the move vacated. That is harmless
    // for a copy payload and fenced for an affine element.
    let copyable = element_window_cycle(StructuralMultiplicity::Unrestricted);
    validate_module(&copyable).expect("a copyable element window verifies");
    let affine = element_window_cycle(StructuralMultiplicity::Affine);
    assert_eq!(
        fence_operation(&affine),
        Some(id(20, OperationId::new)),
        "an affine element window keeps the fence: {:?}",
        validate_module(&affine)
    );
}

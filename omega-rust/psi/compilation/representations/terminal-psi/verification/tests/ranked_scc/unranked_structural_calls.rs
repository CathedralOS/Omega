//! Structural call results and call-scoped loans in machines with an
//! unranked cycle. `next(&mut self.left) -> Step` lends a projected field of
//! the mutable-borrowed receiver for the call alone and returns an affine,
//! reference-free, claim-free `Step`. The frontier replay compares every
//! arrival's owned places against the block's established entry, so the
//! cycle fence admits the call once the result's disposal is accounted for:
//! outside every cycle the call runs once per activation and the exit rosters
//! dispose of the result; inside a cycle an in-cycle edge must discard it or
//! move it into an owned successor parameter before re-entry. An undisposed
//! result keeps the fence closed, and a result carried across the back edge,
//! disposed twice, or bound over a live loop parameter rejects in the replay.
//! A scalar call may also borrow a call result and copy an unrestricted case
//! payload, as `bounds.is_opened()` and
//! `filter.element_count(MemoryAlignment::Alignment4)` do.

use super::{
    AdmissionProfile, Block, BlockId, ContractId, ControlCycleIneligibility, EdgeId, MachineId,
    ModuleError, Operation, OperationId, OperationKind, OperationResult, PlaceId, ProofBundle,
    ScalarType, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    ValueId, id, unranked_scalar_cycle, validate_module, verify_module_for_interpretation,
};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, StructuralArgument, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralFieldDeclaration, StructuralFieldType, StructuralOperationResult,
    StructuralPathSegment, StructuralResultDeclaration, SuccessorEdge,
};

const LOOP: u64 = 1;
const DISPATCH: u64 = 2;
const EXIT: u64 = 4;
const PREFIX: u64 = 5;
const RECEIVER: u64 = 1;
const CONDITION: u64 = 10;
const NEXT: u64 = 2;
const IS_REGION: u64 = 3;

fn cell() -> StructuralTypeId {
    id(1, StructuralTypeId::new)
}

fn envelope() -> StructuralTypeId {
    id(2, StructuralTypeId::new)
}

fn step() -> StructuralTypeId {
    id(3, StructuralTypeId::new)
}

fn region() -> StructuralCaseId {
    id(1, StructuralCaseId::new)
}

fn field(
    field: u64,
    identity: &str,
    field_type: StructuralFieldType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: id(field, StructuralFieldId::new),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn parameter(
    place: u64,
    position: u32,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position,
        is_self: false,
        structural_type,
        multiplicity,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn step_result(place: u64, multiplicity: StructuralMultiplicity) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: id(place, PlaceId::new),
        structural_type: step(),
        multiplicity,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn establish_region(operation: u64, place: u64, multiplicity: StructuralMultiplicity) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: step_result(place, multiplicity),
        kind: OperationKind::EstablishScalarCase {
            result_case: region(),
            fields: Vec::new(),
        },
    }
}

/// A callee cloned from the looping machine with `structural_parameters` and
/// one Boolean scalar parameter (value `100 * machine + 20`), whose body is
/// replaced by `block`. Place identities are module-wide, so each callee
/// numbers its own from `100 * machine`.
fn callee(
    module: &TerminalModule,
    machine: u64,
    structural_parameters: Vec<StructuralParameterDeclaration>,
    result: TerminalMachineResult,
    mut structural_places: Vec<StructuralPlaceDeclaration>,
    block: Block,
) -> terminal_psi::TerminalMachine {
    let mut callee = module.machines[0].clone();
    callee.id = id(machine, MachineId::new);
    callee.contract.id = id(machine, ContractId::new);
    callee.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(100 * machine + 20, ValueId::new),
        scalar_type: ScalarType::Boolean,
    }];
    for parameter in &structural_parameters {
        structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: false,
            },
        });
    }
    callee.structural_parameters = structural_parameters;
    callee.structural_places = structural_places;
    callee.result = result;
    callee.entry = block.id;
    callee.blocks = vec![block];
    callee
}

fn body(machine: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(100 * machine, BlockId::new),
        parameters: Vec::new(),
        operations,
        terminator,
    }
}

/// `unranked_scalar_cycle`'s self-looping block 1 (back edge 10, exit edge 11
/// to the returning block 4) over a mutable-borrowed `Envelope { left: Cell,
/// right: Cell }` receiver at place 1, with two callees:
/// `next(cell: &mut Cell, flag: bool) -> Step`, which returns an affine
/// `Step::Region`, and `is_region(step: &Step, expected: Step, flag: bool) ->
/// bool`, which returns its flag.
fn call_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: cell(),
            identity: "test::Cell".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(
                    1,
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
                    field(1, "left", StructuralFieldType::Structural(cell())),
                    field(2, "right", StructuralFieldType::Structural(cell())),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: step(),
            identity: "test::Step".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: region(),
                        identity: "Region".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: id(2, StructuralCaseId::new),
                        identity: "Finished".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        },
    ];
    let next = callee(
        &module,
        NEXT,
        vec![parameter(
            200,
            0,
            cell(),
            StructuralMultiplicity::Unrestricted,
            StructuralAccess::MutableBorrow,
        )],
        TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: id(201, PlaceId::new),
            structural_type: step(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        }),
        vec![
            StructuralPlaceDeclaration {
                id: id(201, PlaceId::new),
                kind: StructuralPlaceKind::Result,
            },
            StructuralPlaceDeclaration {
                id: id(202, PlaceId::new),
                kind: StructuralPlaceKind::OperationResult {
                    producer: id(200, OperationId::new),
                    structural_type: step(),
                },
            },
        ],
        body(
            NEXT,
            vec![establish_region(200, 202, StructuralMultiplicity::Affine)],
            Terminator::ReturnStructural {
                edge: id(200, EdgeId::new),
                source: id(202, PlaceId::new),
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        ),
    );
    let is_region = callee(
        &module,
        IS_REGION,
        vec![
            parameter(
                300,
                0,
                step(),
                StructuralMultiplicity::Unrestricted,
                StructuralAccess::SharedBorrow,
            ),
            parameter(
                301,
                1,
                step(),
                StructuralMultiplicity::Unrestricted,
                StructuralAccess::Owned,
            ),
        ],
        TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(321, ValueId::new),
            scalar_type: ScalarType::Boolean,
        }),
        Vec::new(),
        body(
            IS_REGION,
            Vec::new(),
            Terminator::Return {
                edge: id(300, EdgeId::new),
                value: id(320, ValueId::new),
                cleanup_actions: Vec::new(),
            },
        ),
    );
    module.machines.extend([next, is_region]);
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![parameter(
        RECEIVER,
        0,
        envelope(),
        StructuralMultiplicity::Unrestricted,
        StructuralAccess::MutableBorrow,
    )];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(RECEIVER, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    module
}

fn declare(module: &mut TerminalModule, place: u64, kind: StructuralPlaceKind) {
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: id(place, PlaceId::new),
            kind,
        });
}

fn declare_result(module: &mut TerminalModule, place: u64, producer: u64) {
    declare(
        module,
        place,
        StructuralPlaceKind::OperationResult {
            producer: id(producer, OperationId::new),
            structural_type: step(),
        },
    );
}

/// `next(&mut self.left, flag)` producing its affine `Step` at `place`.
fn next_call(module: &mut TerminalModule, operation: u64, place: u64) -> Operation {
    declare_result(module, place, operation);
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(operation, OperationId::new),
        result: step_result(place, StructuralMultiplicity::Affine),
        kind: OperationKind::CallStructuralWithScalarArguments {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: id(NEXT, MachineId::new),
            arguments: vec![id(CONDITION, ValueId::new)],
            structural_arguments: vec![StructuralArgument {
                place: id(RECEIVER, PlaceId::new),
                path: vec![StructuralPathSegment::Field("left".into())],
                access: StructuralAccess::MutableBorrow,
            }],
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    }
}

fn owned(place: u64) -> StructuralArgument {
    StructuralArgument {
        place: id(place, PlaceId::new),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    }
}

fn places(places: &[u64]) -> Vec<PlaceId> {
    places
        .iter()
        .map(|place| id(*place, PlaceId::new))
        .collect()
}

fn block_mut(module: &mut TerminalModule, block: u64) -> &mut Block {
    module.machines[0]
        .blocks
        .iter_mut()
        .find(|candidate| candidate.id == id(block, BlockId::new))
        .expect("the fixture declares the block")
}

/// The loop block's back and exit edges.
fn loop_edges(module: &mut TerminalModule) -> (&mut SuccessorEdge, &mut SuccessorEdge) {
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut block_mut(module, LOOP).terminator
    else {
        unreachable!("the loop block branches")
    };
    (when_true, when_false)
}

fn exit_discards(module: &mut TerminalModule) -> &mut Vec<PlaceId> {
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut block_mut(module, EXIT).terminator
    else {
        unreachable!("the exit returns")
    };
    trivial_affine_discards
}

/// A prefix block 5, now the machine entry, running `operations` once and
/// jumping into the loop along edge 13.
fn add_prefix(
    module: &mut TerminalModule,
    operations: Vec<Operation>,
    structural_arguments: Vec<StructuralArgument>,
    trivial_affine_discards: Vec<PlaceId>,
) {
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
            structural_arguments,
            trivial_affine_discards,
            residual_affine_discards: Vec::new(),
        },
    });
}

/// An owned affine `Step` parameter at `place` on `block`.
fn add_step_parameter(module: &mut TerminalModule, block: u64, place: u64) {
    declare(
        module,
        place,
        StructuralPlaceKind::BlockParameter {
            block: id(block, BlockId::new),
            position: 0,
        },
    );
    block_mut(module, block)
        .structural_parameters
        .push(parameter(
            place,
            0,
            step(),
            StructuralMultiplicity::Affine,
            StructuralAccess::Owned,
        ));
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

/// Rejected by the frontier replay, not by the cycle fence.
fn assert_replay_rejects(module: &TerminalModule, reason: &str) -> ModuleError {
    let error = validate_module(module).map(|_| ()).expect_err(reason);
    assert!(
        !matches!(error, ModuleError::ControlCycleIneligible { .. }),
        "{reason}: the fence, not the replay, rejected: {error:?}"
    );
    error
}

#[test]
fn affine_call_result_outside_the_cycle_runs_once() {
    // The prefix block runs once per activation. Its result is disposed
    // either on the edge into the loop or, live for the loop's whole
    // lifetime, at the final return.
    for dispose_on_entry_edge in [true, false] {
        let mut module = call_cycle();
        let call = next_call(&mut module, 30, 2);
        add_prefix(
            &mut module,
            vec![call],
            Vec::new(),
            if dispose_on_entry_edge {
                places(&[2])
            } else {
                Vec::new()
            },
        );
        if !dispose_on_entry_edge {
            *exit_discards(&mut module) = places(&[2]);
        }
        assert_verifies(
            &module,
            &format!("an affine prefix result (disposed on entry edge: {dispose_on_entry_edge})"),
        );
    }
}

/// `enum_step`/`enum_step_kind`: the loop block calls `next` and moves the
/// affine result along a jump into the dispatch block's owned parameter at
/// place 3. The dispatch block either selects on that parameter's case
/// (`Region` loops back along edge 14, `Finished` exits along edge 11) or
/// branches on the scalar condition; every dispatch edge discards the
/// parameter.
fn successor_move_cycle(dispatch_on_case: bool) -> TerminalModule {
    let mut module = call_cycle();
    let call = next_call(&mut module, 30, 2);
    let loop_block = block_mut(&mut module, LOOP);
    loop_block.operations = vec![call];
    loop_block.terminator = Terminator::Jump {
        edge: id(10, EdgeId::new),
        target: id(DISPATCH, BlockId::new),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![owned(2)],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    let terminator = if dispatch_on_case {
        let case_edge = |edge: u64, target: u64, case: u64| StructuralCaseSuccessorEdge {
            edge: id(edge, EdgeId::new),
            target: id(target, BlockId::new),
            case: id(case, StructuralCaseId::new),
            payload_fields: Vec::new(),
            trivial_affine_discards: places(&[3]),
        };
        Terminator::StructuralCase {
            source: id(3, PlaceId::new),
            cases: vec![case_edge(14, LOOP, 1), case_edge(11, EXIT, 2)],
        }
    } else {
        let edge = |edge: u64, target: u64| SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: id(edge, EdgeId::new),
            target: id(target, BlockId::new),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            trivial_affine_discards: places(&[3]),
        };
        Terminator::Conditional {
            condition: id(CONDITION, ValueId::new),
            when_true: edge(14, LOOP),
            when_false: edge(11, EXIT),
        }
    };
    module.machines[0].blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: id(DISPATCH, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    });
    add_step_parameter(&mut module, DISPATCH, 3);
    module
}

#[test]
fn cyclic_call_result_moves_into_the_successor_parameter() {
    for dispatch_on_case in [true, false] {
        let module = successor_move_cycle(dispatch_on_case);
        assert_verifies(
            &module,
            &format!(
                "the dispatch block owns and disposes the moved result before re-entry \
                 (case dispatch: {dispatch_on_case})"
            ),
        );
    }
}

#[test]
fn cyclic_call_result_carried_across_the_back_edge_rejects() {
    // Nothing in the cycle disposes the result: the fence stays closed.
    let mut module = call_cycle();
    let call = next_call(&mut module, 30, 2);
    block_mut(&mut module, LOOP).operations = vec![call];
    assert_eq!(
        fence_operation(&module),
        Some(id(30, OperationId::new)),
        "an undisposed cyclic result keeps the fence: {:?}",
        validate_module(&module)
    );

    // Only the exit edge disposes it. That satisfies the fence, which asks
    // only that some in-cycle edge schedules a disposal; the back edge still
    // arrives owning a result the loop's established entry lacks.
    let (_, exit) = loop_edges(&mut module);
    exit.trivial_affine_discards = places(&[2]);
    assert_eq!(
        assert_replay_rejects(&module, "the back edge carries the result"),
        ModuleError::OwnedStructuralFrontierJoinMismatch(id(LOOP, BlockId::new)),
    );

    // The dispatch block likewise must dispose of the moved parameter on
    // its back edge.
    let mut module = successor_move_cycle(false);
    let Terminator::Conditional { when_true, .. } =
        &mut block_mut(&mut module, DISPATCH).terminator
    else {
        unreachable!("the dispatch block branches")
    };
    when_true.trivial_affine_discards.clear();
    assert_eq!(
        assert_replay_rejects(&module, "the back edge carries the moved parameter"),
        ModuleError::OwnedStructuralFrontierJoinMismatch(id(LOOP, BlockId::new)),
    );

    // A case dispatch that keeps its parameter on the looping case is no
    // longer a confined dispatch, and the fence names its terminator.
    let mut module = successor_move_cycle(true);
    let Terminator::StructuralCase { cases, .. } = &mut block_mut(&mut module, DISPATCH).terminator
    else {
        unreachable!("the dispatch block selects on the case")
    };
    cases[0].trivial_affine_discards.clear();
    assert!(
        matches!(
            validate_module(&module),
            Err(ModuleError::ControlCycleIneligible {
                reason: ControlCycleIneligibility::BlockTerminator { block },
                ..
            }) if block == id(DISPATCH, BlockId::new)
        ),
        "an unconfined dispatch keeps the fence: {:?}",
        validate_module(&module)
    );
}

#[test]
fn a_call_result_disposed_twice_rejects() {
    // Moved into the dispatch parameter and discarded on the same edge.
    let mut module = successor_move_cycle(true);
    let Terminator::Jump {
        trivial_affine_discards,
        ..
    } = &mut block_mut(&mut module, LOOP).terminator
    else {
        unreachable!("the loop block jumps")
    };
    *trivial_affine_discards = places(&[2]);
    assert_eq!(
        assert_replay_rejects(&module, "a moved result cannot also be discarded"),
        ModuleError::EdgeAffineDiscardsInvalid {
            edge: id(10, EdgeId::new)
        },
    );

    // Discarded on the prefix's entry edge and again at the final return.
    let mut module = call_cycle();
    let call = next_call(&mut module, 30, 2);
    add_prefix(&mut module, vec![call], Vec::new(), places(&[2]));
    *exit_discards(&mut module) = places(&[2]);
    assert_eq!(
        assert_replay_rejects(&module, "a discarded result cannot be discarded again"),
        ModuleError::UnitReturnAffineDiscardsMismatch {
            machine: module.machines[0].id,
            block: id(EXIT, BlockId::new),
        },
    );
}

/// A loop-carried `Step` parameter at place 3 on the loop block, seeded by a
/// prefix call's result at place 4. Every traversal calls `next` again and
/// binds the fresh result at place 2 to the parameter along the back edge,
/// discarding the value it replaces; the exit edge discards both.
fn loop_carried_cycle() -> TerminalModule {
    let mut module = call_cycle();
    let seed = next_call(&mut module, 29, 4);
    add_prefix(&mut module, vec![seed], vec![owned(4)], Vec::new());
    add_step_parameter(&mut module, LOOP, 3);
    let call = next_call(&mut module, 30, 2);
    block_mut(&mut module, LOOP).operations = vec![call];
    let (back, exit) = loop_edges(&mut module);
    back.structural_arguments = vec![owned(2)];
    back.trivial_affine_discards = places(&[3]);
    exit.structural_arguments = Vec::new();
    // Results dispose before parameters, in the frontier's roster order.
    exit.trivial_affine_discards = places(&[2, 3]);
    module
}

#[test]
fn a_call_result_replaces_a_disposed_loop_parameter() {
    let module = loop_carried_cycle();
    assert_verifies(
        &module,
        "the back edge disposes the old parameter before binding the fresh result",
    );
}

#[test]
fn a_call_result_replacing_a_live_loop_parameter_rejects() {
    // The back edge binds the fresh result to the loop parameter while the
    // value it replaces is still owned: that one incoming edge would hold
    // two values for one parameter.
    let mut module = loop_carried_cycle();
    let (back, _) = loop_edges(&mut module);
    back.trivial_affine_discards.clear();
    assert_eq!(
        assert_replay_rejects(&module, "the replaced parameter is still owned"),
        ModuleError::InvalidStructuralSuccessorArgument {
            edge: id(10, EdgeId::new),
            place: id(3, PlaceId::new),
        },
    );
}

#[test]
fn cyclic_scalar_call_borrows_a_call_result_and_copies_a_case_payload() {
    // `is_region(&step, Step::Region, flag)`: a shared loan of the affine call
    // result for the call alone, and a copy of an unrestricted case payload
    // into an unrestricted owned parameter. The loop edges then discard the
    // borrowed result.
    let mut module = call_cycle();
    let call = next_call(&mut module, 30, 2);
    declare_result(&mut module, 5, 31);
    let payload = establish_region(31, 5, StructuralMultiplicity::Unrestricted);
    let observe = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(32, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(32, ValueId::new),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::CallStructuralScalar {
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            callee: id(IS_REGION, MachineId::new),
            arguments: vec![id(CONDITION, ValueId::new)],
            structural_arguments: vec![
                StructuralArgument {
                    place: id(2, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                owned(5),
            ],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    block_mut(&mut module, LOOP).operations = vec![call, payload, observe];
    let (back, exit) = loop_edges(&mut module);
    back.trivial_affine_discards = places(&[2]);
    exit.trivial_affine_discards = places(&[2]);
    assert_verifies(
        &module,
        "a borrowed result and a copied payload move no custody",
    );
}

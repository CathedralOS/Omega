//! A view local is an established view place exactly as a whole view
//! parameter is: it can be narrowed by a `let`, measured and indexed, and a
//! narrowed local can be narrowed again. Each program lowers to Terminal Psi,
//! round-trips through the codec, and is independently verified -- which
//! demands that every subslice's source is an established view dominating it
//! and that its length observes that exact source -- and executed where the
//! interpreter supports the view's establishment.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::IntegerValue;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
    TerminalScalarValue, interpret_terminal_artifact_measured,
};

#[derive(Default)]
struct Observed(Vec<i128>);

impl TerminalEffectHandler for Observed {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("expected an observation boundary");
        };
        let [TerminalScalarValue::Integer { value, .. }] = arguments.as_slice() else {
            panic!("expected one integer observation");
        };
        self.0.push(match value {
            IntegerValue::Unsigned(value) => i128::try_from(*value).unwrap(),
            IntegerValue::Signed(value) => *value,
        });
        Ok(())
    }
}

fn lower(source: &str) -> Result<lowered_psi::LoweredPsi, String> {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .map_err(|error| format!("{error:?}"))
}

fn execute(source: &str) -> Vec<i128> {
    let lowered = lower(source).expect("the view-local program lowers");
    let mut observed = Observed::default();
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut observed,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    observed.0
}

/// Count every view subslice in the lowered module whose source is itself an
/// operation result rather than a machine parameter: a narrowed view local,
/// not a narrowed formal.
fn subslices_of_established_views(lowered: &lowered_psi::LoweredPsi) -> usize {
    lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    let (terminal_psi::OperationKind::ElementViewSubslice { source, .. }
                    | terminal_psi::OperationKind::ByteSequenceSubslice { source, .. }) =
                        operation.kind
                    else {
                        return false;
                    };
                    machine.structural_places.iter().any(|place| {
                        place.id == source
                            && matches!(
                                place.kind,
                                semantic_vocabulary::StructuralPlaceKind::OperationResult { .. }
                            )
                    })
                })
                .count()
        })
        .sum()
}

const BYTES: &str = r#"
    boundary trait Output {
        machine observe(value: u64) reaches Output;
    }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) reaches Output {
        let tail: &[u8] = bytes[..];
        let window: &[u8] = tail[..];
        Output::observe(tail.len);
        Output::observe(window.len);
    }
    data Root {}
    machine Root::enter() reaches Output {
        Helper::measure("\x0A\x14\x1E");
        Helper::measure("");
    }
"#;

/// A view parameter narrowed by a `let` is a view local; narrowing that
/// local again reads an operation result, and each local measures its own
/// extent.
#[test]
fn a_byte_view_local_is_narrowed_measured_and_narrowed_again() {
    let lowered = lower(BYTES).expect("byte view-local subslices lower");
    // `window` narrows the local `tail`, never the `bytes` formal.
    assert_eq!(subslices_of_established_views(&lowered), 1);
    assert_eq!(execute(BYTES), vec![3, 3, 0, 0]);
}

/// An element view borrowed from a local array narrows, measures and reads
/// the same way: `tail` narrows the `as_slice` local and `window` narrows
/// `tail`, so both ranges read an established view.
#[test]
fn an_element_view_local_is_narrowed_read_and_verified() {
    let lowered = lower(ELEMENTS).expect("element view-local subslices lower");
    assert_eq!(subslices_of_established_views(&lowered), 2);
    // Reload the artifact and verify it independently: each subslice's
    // source must be an established view that dominates it, and each read
    // and length must observe the exact narrowed view. (The interpreter does
    // not yet execute `as_slice` over a local array, with or without a
    // subslice, so this case stops at verification.)
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a subslice of a view local verifies independently");
    let reads = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::ElementViewRead { .. }
                    | terminal_psi::OperationKind::ElementViewLength { .. }
            )
        })
        .count();
    assert!(reads >= 3, "two reads and a length observe the view locals");
}

const ELEMENTS: &str = r#"
    boundary trait Output {
        machine observe(value: i32) reaches Output;
        machine count(value: u64) reaches Output;
    }
    data Root {}
    machine Root::enter() reaches Output {
        let values: [i32; 4] = [11, 29, 37, 43];
        let view: &[i32] = values.as_slice();
        let tail: &[i32] = view[1..];
        let window: &[i32] = tail[1..3];
        Output::observe(tail[0]);
        Output::observe(window[1]);
        Output::count(window.len);
    }
"#;

const FIELD_RANGE: &str = r#"
    boundary trait Output {
        machine observe(value: i32) reaches Output;
        machine count(value: u64) reaches Output;
    }
    data Root {
        values: [i32; 5];
        hi: u64;
    }
    machine Root::enter(&mut self) reaches Output {
        self.values[1] = 29;
        self.hi = 4;
        let tail: &[i32] = self.values[1..self.hi];
        Output::count(tail.len);
    }
"#;

/// A literal field range inside a state graph: the view local is read and
/// measured, and the measure selects the successor state.
#[test]
fn a_field_range_local_measures_across_a_state_graph() {
    let source = r#"
        boundary trait Output {
            machine observe(value: i32) reaches Output;
            machine count(value: u64) reaches Output;
        }
        data Root {
            values: [i32; 5];
        }
        machine Root::enter(&mut self) reaches Output {
            self.values[1] = 29;
            let tail: &[i32] = self.values[1..4];
            Output::observe(tail[0]);
            let n: u64 = tail.len;
            transition n == 3 {
                true -> matched()
                false -> unmatched()
            }
            state matched(&mut self) { Output::count(3); }
            state unmatched(&mut self) { Output::count(0); }
        }
    "#;
    let lowered = lower(source).expect("a field range local lowers in a state graph");
    assert_eq!(subslices_of_established_views(&lowered), 1);
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a state-graph field range verifies against the array's extent");
}

const BYTE_FIELD_RANGE: &str = r#"
    boundary trait Output { machine count(value: u64) reaches Output; }
    data Root { input: [u8; 256]; alternate: [u8; 256]; }
    machine Root::enter(&mut self) reaches Output {
        transition { _ -> dispatch(3) }
        state dispatch(&mut self, count: u64) requires count <= 256 {
            let command: &[u8] = self.input[0..count];
            Output::count(command.len);
        }
    }
"#;

const BYTE_FIELD_GUARDS: &str = r#"
    boundary trait Output { machine count(value: u64) reaches Output; }
    data Root { input: [u8; 256]; }
    machine Root::enter(&mut self) reaches Output {
        transition { _ -> dispatch(3) }
        state dispatch(&mut self, count: u64) requires count <= 256 {
            let command: &[u8] = self.input[0..count];
            transition {
                command == "" -> empty()
                command == "quit" -> quit()
                _ -> other()
            }
        }
        state empty(&mut self) { Output::count(0); }
        state quit(&mut self) { Output::count(1); }
        state other(&mut self) { Output::count(2); }
    }
"#;

#[test]
fn a_byte_field_local_literal_guard_keeps_short_reads_conditional() {
    // The generic state permits zero or fewer than four live bytes: proving
    // every read requires the equality's exact length-first short circuit.
    let lowered = lower(BYTE_FIELD_GUARDS).expect("dynamic local byte guards lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("short views never require an out-of-bounds read");
}

#[test]
fn byte_field_literal_guard_replay_rejects_changed_local_and_read_type() {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar};
    fn corrupt(value: &mut Boolean, symbol: symbols::SymbolHandle, wrong_type: bool) -> bool {
        match value {
            Boolean::And { left, right } => {
                corrupt(left, symbol, wrong_type) | corrupt(right, symbol, wrong_type)
            }
            Boolean::IntegerComparison { left, .. } => match left.as_mut() {
                Scalar::StructuralParameterByteLength { root, .. }
                | Scalar::StructuralParameterIndexedRead { root, .. }
                    if !wrong_type =>
                {
                    *root = checked_trees::CheckedStorageRoot::ViewLocal { symbol };
                    true
                }
                Scalar::StructuralParameterIndexedRead { primitive_type, .. } if wrong_type => {
                    *primitive_type = checked_trees::types::PrimitiveType::U16;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }
    let original = crate::front_end::checked_program(BYTE_FIELD_GUARDS);
    for wrong_type in [false, true] {
        let mut changed = original.clone();
        let mut corrupted = false;
        for expression in &mut changed.facts.values.scalar_expressions.expressions {
            if expression.role == checked_trees::CheckedScalarExpressionRole::Guard
                && let Scalar::Boolean(boolean) = &mut expression.expression
            {
                corrupted |= corrupt(boolean, expression.state, wrong_type);
            }
        }
        assert!(
            corrupted,
            "the byte guards retain their ordinary scalar observations"
        );
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "source replay must reject wrong local/type (wrong_type={wrong_type})"
        );
    }
}

#[test]
fn a_byte_field_range_local_measures_in_a_named_state() {
    let lowered = lower(BYTE_FIELD_RANGE).expect("a byte field range uses its exact array backing");
    assert_eq!(subslices_of_established_views(&lowered), 1);
    assert_eq!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishElementView { .. }
            ))
            .count(),
        1,
        "one whole field view precedes the byte subslice"
    );
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload byte field module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload byte field proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("the byte range retains the array extent and its dynamic endpoint proof");
}

#[test]
fn byte_field_range_replay_rejects_changed_source_access_and_endpoint() {
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    let original = crate::front_end::checked_program(BYTE_FIELD_RANGE);
    for fault in 0..3 {
        let mut changed = original.clone();
        let operation = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
                )
            })
            .expect("the named state retains the byte range binding");
        let CheckedUnitEffectOperationPlan::EstablishViewSubslice { source, .. } = operation else {
            unreachable!()
        };
        match fault {
            0 => {
                source.path = vec![CheckedUnitStructuralPathSegment::Field(
                    "alternate".to_owned(),
                )]
            }
            1 => source.access = CheckedStructuralAccess::MutableBorrow,
            2 => {
                let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { end, .. } =
                    &mut source.source
                else {
                    panic!("the retained range must remain a byte range")
                };
                *end = None;
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "byte field replay must reject fault {fault}"
        );
    }
    let written = BYTE_FIELD_RANGE.replace(
        "Output::count(command.len);",
        "self.input[0] = 7; Output::count(command.len);",
    );
    assert!(
        crate::front_end::checked_program_result(&written).is_err(),
        "a write must not cross the live shared byte-field loan"
    );
    let wrong_element = BYTE_FIELD_RANGE.replace("input: [u8; 256]", "input: [u16; 256]");
    // The checked surface currently accepts this spelling, but cannot retain
    // an exact byte-field establishment plan for the mismatched element.
    assert!(
        lower(&wrong_element).is_err(),
        "a byte field view cannot reinterpret another element type"
    );
}

#[test]
fn a_byte_field_view_cannot_read_a_write_only_root() {
    // A direct entry has no incoming edge whose access mismatch could reject
    // the mutation before the byte establishment's own custody is checked.
    let source = r#"
        boundary trait Output { machine byte(value: u8) reaches Output; }
        data Root { input: [u8; 256]; }
        machine Root::enter(&mut self) reaches Output {
            let command: &[u8] = self.input[0..3];
            Output::byte(command[0]);
        }
    "#;
    let overrun = source.replace("0..3", "0..257");
    assert!(
        crate::front_end::checked_program_result(&overrun).is_err(),
        "a byte field range beyond fixed backing must not check"
    );
    let mut lowered = lower(source).expect("the readable field view lowers");
    let machine = lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::EstablishElementView { .. }
                    )
                })
        })
        .unwrap();
    let root = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if let terminal_psi::OperationKind::EstablishElementView { source, .. } =
                &operation.kind
            {
                Some(source.place)
            } else {
                None
            }
        })
        .unwrap();
    let parameter = machine
        .structural_parameters
        .iter_mut()
        .find(|parameter| parameter.place == root)
        .expect("the established field retains its exact parameter root");
    parameter.access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "shared argument spelling cannot grant a write-only root read access"
    );
}

#[test]
fn byte_field_subslice_cannot_share_an_edge_with_its_mutable_backing() {
    use semantic_vocabulary::{
        BlockId, EdgeId, OperationId, PlaceId, StructuralPlaceKind, ValueId,
    };
    use terminal_psi::{
        Block, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralParameterDeclaration, StructuralPlaceDeclaration, Terminator,
    };

    let lowered = lower(BYTE_FIELD_RANGE).expect("the byte field range lowers");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("the unchanged emitted module validates");
    let mut shared_only = lowered.semantic_module;
    let machine_index = shared_only
        .machines
        .iter()
        .position(|machine| {
            machine.blocks.iter().any(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::EstablishElementView { .. })
                })
            })
        })
        .unwrap();
    let machine = &mut shared_only.machines[machine_index];
    let source_block_index = machine
        .blocks
        .iter()
        .position(|block| {
            block.operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::EstablishElementView { .. })
            })
        })
        .unwrap();
    let source_block = &machine.blocks[source_block_index];
    let (backing, whole_view) = source_block
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::EstablishElementView { source, .. } => {
                Some((source.place, operation.result.structural().unwrap().place))
            }
            _ => None,
        })
        .unwrap();
    let subslice = source_block
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ByteSequenceSubslice { source, .. } if source == whole_view => {
                operation.result.structural().cloned()
            }
            _ => None,
        })
        .expect("the emitted subslice narrows the established field view");
    let establishment = source_block
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::EstablishElementView { .. }))
        .unwrap()
        .clone();
    let length_observation = source_block
        .operations
        .iter()
        .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceLength { .. }))
        .unwrap()
        .clone();
    let mut backing_parameter = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == backing)
        .unwrap()
        .clone();

    // Fresh test coordinates leave every emitted operation, source access,
    // extent obligation and existing state binding unchanged. The positive
    // validation below also checks that these coordinates do not collide.
    let receiving_block = BlockId::new(u64::MAX).unwrap();
    let shared_place = PlaceId::new(u64::MAX).unwrap();
    let mutable_place = PlaceId::new(u64::MAX - 1).unwrap();
    let edge = EdgeId::new(u64::MAX).unwrap();
    let return_terminator = std::mem::replace(
        &mut machine.blocks[source_block_index].terminator,
        Terminator::Jump {
            edge,
            target: receiving_block,
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: subslice.place,
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    assert!(matches!(return_terminator, Terminator::ReturnUnit { .. }));
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: shared_place,
        kind: StructuralPlaceKind::BlockParameter {
            block: receiving_block,
            position: 0,
        },
    });
    machine.blocks.push(Block {
        id: receiving_block,
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: vec![StructuralParameterDeclaration {
            place: shared_place,
            position: 0,
            is_self: false,
            structural_type: subslice.structural_type,
            multiplicity: subslice.multiplicity,
            access: StructuralAccess::SharedBorrow,
            qualifications: subslice.qualifications.clone(),
            projected_qualifications: subslice.projected_qualifications.clone(),
        }],
        operations: Vec::new(),
        terminator: return_terminator,
    });
    terminal_verifier::validate_module(&shared_only)
        .expect("an established byte subslice can enter a shared-only successor");

    backing_parameter.place = mutable_place;
    backing_parameter.position = 1;
    backing_parameter.is_self = false;
    assert_eq!(backing_parameter.access, StructuralAccess::MutableBorrow);
    // The whole-view control already carries its backing origin. Narrowing
    // that view must not erase the same simultaneous-loan conflict.
    for (description, shared_source) in [("whole view", whole_view), ("subslice", subslice.place)] {
        let mut overlapping = shared_only.clone();
        let machine = &mut overlapping.machines[machine_index];
        machine
            .blocks
            .last_mut()
            .unwrap()
            .structural_parameters
            .push(backing_parameter.clone());
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: mutable_place,
            kind: StructuralPlaceKind::BlockParameter {
                block: receiving_block,
                position: 1,
            },
        });
        let Terminator::Jump {
            structural_arguments,
            ..
        } = &mut machine.blocks[source_block_index].terminator
        else {
            unreachable!()
        };
        structural_arguments[0].place = shared_source;
        structural_arguments.push(StructuralArgument {
            place: backing,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        });
        let result = terminal_verifier::validate_module(&overlapping).map(|_| ());
        assert!(
            matches!(&result, Err(terminal_verifier::ModuleError::InvalidStructuralSuccessorArgument {
                edge: rejected_edge, ..
            }) if *rejected_edge == edge),
            "{description} cannot accompany its mutable backing on the same edge: {result:?}"
        );
    }

    // Passing only the mutable backing ends the old state's shared views.
    // Their descriptor definitions still dominate, but dominance alone must
    // not authorize observations after the exclusive name has transferred.
    let mut mutable_only = shared_only.clone();
    let machine = &mut mutable_only.machines[machine_index];
    backing_parameter.place = shared_place;
    backing_parameter.position = 0;
    machine.blocks.last_mut().unwrap().structural_parameters[0] = backing_parameter;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[source_block_index].terminator
    else {
        unreachable!()
    };
    structural_arguments[0] = StructuralArgument {
        place: backing,
        path: Vec::new(),
        access: StructuralAccess::MutableBorrow,
    };
    terminal_verifier::validate_module(&mutable_only)
        .expect("the mutable backing alone can enter its successor");

    let fresh_place = PlaceId::new(u64::MAX - 2).unwrap();
    let observation_id = OperationId::new(u64::MAX).unwrap();
    for (description, observed_place) in [
        ("fresh current-root view", fresh_place),
        ("stale whole view", whole_view),
        ("stale subslice", subslice.place),
    ] {
        let mut observed = mutable_only.clone();
        let machine = &mut observed.machines[machine_index];
        if observed_place == fresh_place {
            let mut fresh = establishment.clone();
            fresh.id = OperationId::new(u64::MAX - 1).unwrap();
            let OperationKind::EstablishElementView {
                destination,
                source,
                ..
            } = &mut fresh.kind
            else {
                unreachable!()
            };
            *destination = fresh_place;
            source.place = shared_place;
            let OperationResult::Structural(result) = &mut fresh.result else {
                unreachable!()
            };
            result.place = fresh_place;
            machine.structural_places.push(StructuralPlaceDeclaration {
                id: fresh_place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: fresh.id,
                    structural_type: result.structural_type,
                },
            });
            machine.blocks.last_mut().unwrap().operations.push(fresh);
        }
        let mut length = length_observation.clone();
        length.id = observation_id;
        length.result.scalar_mut().unwrap().id = ValueId::new(u64::MAX).unwrap();
        length.kind = OperationKind::ByteSequenceLength {
            source: observed_place,
        };
        machine.blocks.last_mut().unwrap().operations.push(length);
        let result = terminal_verifier::validate_module(&observed).map(|_| ());
        if observed_place == fresh_place {
            result.expect("a fresh view of the successor's current mutable root remains readable");
        } else {
            assert!(
                matches!(&result, Err(terminal_verifier::ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation, place,
                }) if *operation == observation_id && *place == observed_place),
                "{description} cannot observe backing handed to an exclusive successor: {result:?}"
            );
        }
    }
}

#[test]
fn a_byte_field_range_proof_is_bound_to_its_backing_extent() {
    let mut lowered = lower(BYTE_FIELD_RANGE).expect("the dynamic byte range lowers");
    let backing = lowered
        .semantic_module
        .structural_types
        .iter_mut()
        .find(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::FixedArray { length: 256, .. }
            )
        })
        .expect("the exact fixed backing remains represented");
    let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut backing.shape else {
        unreachable!()
    };
    *length = 0;
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "the original bound proof must not authorize a different backing extent"
    );
}

/// Controls for the field range: an end past the array's extent cannot be
/// proved, and writing the array while the narrowed view is live breaks its
/// shared loan. Checking rejects both before lowering.
#[test]
fn a_field_range_that_overruns_or_outlives_a_write_is_refused() {
    let overrun = FIELD_RANGE.replace("self.hi = 4;", "self.hi = 6;");
    assert!(
        crate::front_end::checked_program_result(&overrun).is_err(),
        "a field range past the array's extent must not check"
    );
    let written = FIELD_RANGE.replace(
        "Output::count(tail.len);",
        "self.values[2] = 7;\n        Output::count(tail.len);",
    );
    assert!(
        crate::front_end::checked_program_result(&written).is_err(),
        "writing the array while its field range is live must not check"
    );
}

const RECORD_ELEMENTS: &str = r#"
    boundary trait Output {
        machine observe(value: i32) reaches Output;
    }
    data Entry {
        weight: u64;
        value: i32;
    }
    data Root {
        entries: [Entry; 4];
    }
    machine Root::enter(&mut self) reaches Output {
        self.entries[2].value = 37;
        let view: &[Entry] = self.entries.as_slice();
        let tail: &[Entry] = view[1..];
        Output::observe(tail[1].value);
    }
"#;

/// A record element is read at one scalar leaf: the element read carries the
/// field path inside the selected element, round-trips through the codec,
/// and the verifier resolves that leaf against the view's element type and
/// checks the index against the view's own length.
#[test]
fn a_record_element_field_is_read_through_a_view_local() {
    let lowered = lower(RECORD_ELEMENTS).expect("a record element field read lowers");
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a record element field read verifies independently");
    let reads = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::ElementViewRead { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads,
        vec![vec![terminal_psi::StructuralPathSegment::Field(
            "value".to_owned()
        )]],
        "one read projects the selected element's `value` leaf"
    );
    // The verifier resolves the leaf itself: an undeclared field, a leaf of
    // another scalar type, or a path that leaves the element is refused.
    for forged in [
        vec![terminal_psi::StructuralPathSegment::Field(
            "missing".to_owned(),
        )],
        vec![terminal_psi::StructuralPathSegment::Field(
            "weight".to_owned(),
        )],
        vec![terminal_psi::StructuralPathSegment::Referent],
    ] {
        let mut module = module.clone();
        for operation in module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            if let terminal_psi::OperationKind::ElementViewRead { path, .. } = &mut operation.kind {
                *path = forged.clone();
            }
        }
        assert!(
            terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
                .is_err(),
            "an element read at {forged:?} must not verify"
        );
    }
}

/// A whole `[copy]` record element copies through its scalar fields: one
/// element read per field at the copy's selector, then a fresh owned record
/// from those leaves. Each read carries the view's own bound, and the verifier
/// checks every leaf against the element's declaration. An affine element is
/// not copied this way: by-value access through a shared view requires `[copy]`.
#[test]
fn a_record_element_copies_through_its_field_reads() {
    let source = RECORD_ELEMENTS
        .replace("data Entry {", "data Entry [copy] {")
        .replace(
            "Output::observe(tail[1].value);",
            "let index: u64 = 1;\n        let chosen: Entry = tail[index];\n        Output::observe(chosen.value);",
        );
    let lowered = lower(&source).expect("a record element copy lowers");
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a record element copy verifies independently");
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let leaves = operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::ElementViewRead { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        leaves,
        vec![
            vec![terminal_psi::StructuralPathSegment::Field(
                "weight".to_owned()
            )],
            vec![terminal_psi::StructuralPathSegment::Field(
                "value".to_owned()
            )],
        ],
        "the copy reads each field of the selected element once, in declaration order"
    );
    assert!(
        operations.iter().any(|operation| matches!(
            &operation.kind,
            terminal_psi::OperationKind::EstablishRecord { fields } if fields.len() == 2
        )),
        "the leaves establish one fresh record"
    );
}

/// A field under an arithmetic policy copies the same way: a policy changes
/// operations, not stored bits, so the element read and the fresh record
/// carry the plain scalar. A ranged field still declines, because its range
/// needs evidence that a plain element read does not carry.
#[test]
fn a_policy_field_record_element_copies_through_its_field_reads() {
    let copied = |weight: &str| {
        RECORD_ELEMENTS
            .replace("data Entry {", "data Entry [copy] {")
            .replace("weight: u64;", &format!("weight: {weight};"))
            .replace(
                "Output::observe(tail[1].value);",
                "let index: u64 = 1;\n        let chosen: Entry = tail[index];\n        Output::observe(chosen.value);",
            )
    };
    let lowered = lower(&copied("u64 in Wrapping")).expect("a policy-field element copy lowers");
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a policy-field element copy verifies independently");
    let operations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::ElementViewRead { .. }
            ))
            .count(),
        2,
        "the copy reads each field of the selected element once"
    );
    assert!(
        operations.iter().any(|operation| matches!(
            &operation.kind,
            terminal_psi::OperationKind::EstablishRecord { fields } if fields.len() == 2
        )),
        "the leaves establish one fresh record"
    );
    assert!(
        lower(&copied("u64 [0..=9]")).is_err(),
        "a ranged field keeps the element copy declined"
    );
}

/// A copied `[copy]` element feeds a guard in a state graph: the copy's
/// field is an ordinary local record field read.
#[test]
fn a_copied_record_element_selects_a_successor() {
    let source = r#"
        boundary trait Output {
            machine count(value: u64) reaches Output;
        }
        data Entry [copy] {
            value: i32;
        }
        data Root {
            entries: [Entry; 4];
        }
        machine Root::enter(&mut self) reaches Output {
            self.entries[0].value = 7;
            let entry_view: &[Entry] = self.entries.as_slice();
            let index: u64 = 0;
            let chosen: Entry = entry_view[index];
            transition chosen.value == 7 {
                true -> good()
                false -> bad()
            }
            state good(&mut self) { Output::count(51); }
            state bad(&mut self) { Output::count(52); }
        }
    "#;
    let lowered = lower(source).expect("a copied element guard lowers");
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("a copied element guard verifies independently");
}

const LITERAL_LOCALS: &str = r#"
    boundary trait Output { machine observe(value: u64) reaches Output; }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) reaches Output { Output::observe(bytes.len); }
    data Root {}
    machine Root::enter() reaches Output {
        let mut count: u64 = 1;
        let bytes: &[u8] = "abc";
        count = bytes.len;
        Output::observe(count);
        Helper::measure(bytes);
        let window: &[u8] = bytes[..];
        Helper::measure(window);
        let other: &[u8] = "z";
        Helper::measure(other);
        Helper::measure("more");
    }
"#;

const COMPUTED_LITERAL_CALLS: &str = r#"
    boundary trait Output { machine observe(value: u64) reaches Output; }
    data Helper {}
    machine Helper::measure(bytes: &[u8]) -> u64 reaches Output {
        Output::observe(bytes.len);
        bytes.len
    }
    machine Helper::show(bytes: &[u8]) reaches Output { Output::observe(bytes.len); }
    machine Helper::pair(left: &[u8], right: &[u8]) -> u64 reaches Output {
        Output::observe(left.len);
        Output::observe(99);
        Output::observe(right.len);
        right.len
    }
    machine identity(value: u64) -> u64 { value }
    data Root {}
    machine Root::enter() reaches Output {
        let named: &[u8] = "q";
        Helper::show(named);
        Helper::show("x");
        let first: u64 = Helper::measure("abc");
        Output::observe(first);
        let repeated: u64 = Helper::measure("abc");
        Output::observe(repeated);
        let empty: u64 = Helper::measure("");
        Output::observe(empty);
        let nested: u64 = identity(Helper::measure("nested"));
        Output::observe(nested);
        let pair: u64 = Helper::pair("twin", "twin");
        Output::observe(pair);
    }
"#;

fn assert_computed_literal_declarations(module: &terminal_psi::TerminalModule) -> usize {
    let mut count = 0;
    for machine in &module.machines {
        let mut ordinals = Vec::new();
        for declaration in &machine.structural_places {
            let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } = declaration.kind
            else {
                continue;
            };
            ordinals.push(declaration_ordinal);
            let producers = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(operation.kind,
                    terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. }
                        if destination == declaration.id)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                producers.len(),
                1,
                "each literal occurrence owns one establishment"
            );
            assert_eq!(producers[0].result, terminal_psi::OperationResult::Unit);
            count += 1;
        }
        ordinals.sort_unstable();
        assert_eq!(
            ordinals,
            (0..u32::try_from(ordinals.len()).unwrap()).collect::<Vec<_>>()
        );
    }
    count
}

#[test]
fn computed_literal_calls_share_unit_storage_with_named_and_ordinary_literals() {
    let lowered = lower(COMPUTED_LITERAL_CALLS).expect("computed literal operands lower");
    assert_eq!(
        assert_computed_literal_declarations(&lowered.semantic_module),
        8
    );
    assert_eq!(
        execute(COMPUTED_LITERAL_CALLS),
        vec![1, 1, 3, 3, 3, 3, 0, 0, 6, 6, 4, 99, 4, 4]
    );
    let (caller, arguments) = lowered
        .semantic_module
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match &operation.kind {
                    terminal_psi::OperationKind::CallStructuralScalar {
                        structural_arguments,
                        ..
                    } if structural_arguments.len() == 2 => Some((machine, structural_arguments)),
                    _ => None,
                })
        })
        .expect("two literal operands at one scalar call");
    assert_ne!(arguments[0].place, arguments[1].place);
    for argument in arguments {
        assert!(
            caller
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(&operation.kind,
                terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, bytes, .. }
                    if *destination == argument.place && bytes.as_slice() == b"twin")
                })
        );
    }
    let mut missing = lowered.semantic_module;
    let producer = missing
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishByteSequenceLiteral { .. }
                )
            })
        })
        .expect("literal establishment");
    let index = producer
        .operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishByteSequenceLiteral { .. }
            )
        })
        .unwrap();
    producer.operations.remove(index);
    assert!(matches!(
        terminal_verifier::validate_module(&missing),
        Err(terminal_verifier::ModuleError::ByteSequenceLiteralEstablishmentMismatch(_))
    ));
}

#[test]
fn computed_literal_calls_establish_distinct_places_across_unit_states() {
    let source = r#"
        boundary trait Output { machine observe(value: u64) reaches Output; }
        data Helper {}
        machine Helper::measure(bytes: &[u8]) -> u64 reaches Output {
            Output::observe(bytes.len);
            bytes.len
        }
        data Root {}
        machine Root::enter() reaches Output {
            let first: u64 = Helper::measure("abc");
            transition { _ -> next(first) }
            state next(previous: u64) {
                Output::observe(previous);
                let second: u64 = Helper::measure("abc");
                Output::observe(second);
                Output::observe(Helper::measure(""));
            }
        }
    "#;
    let lowered = lower(source).expect("each Unit state prepares its computed literal operands");
    assert_eq!(
        assert_computed_literal_declarations(&lowered.semantic_module),
        3
    );
    assert_eq!(execute(source), vec![3, 3, 3, 3, 0, 0]);
}

#[test]
fn computed_literal_calls_reject_changed_source_bytes_type_and_access() {
    let original = crate::front_end::checked_program(COMPUTED_LITERAL_CALLS);
    for mutation in 0..3 {
        let mut changed = original.clone();
        let arguments = &mut changed
            .facts
            .values
            .scalar_computations
            .structural_arguments;
        let handle = arguments.iter().find_map(|(handle, argument)| match argument {
            checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)
                if matches!(argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }) => Some(handle),
            _ => None,
        }).expect("computed literal source");
        let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) =
            arguments.get_mut(handle)
        else {
            panic!("literal place argument");
        };
        match mutation {
            0 => {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                    bytes,
                } = &mut argument.source
                else {
                    panic!("literal bytes");
                };
                bytes.push(b'!');
            }
            1 => argument.type_identity = "u64".into(),
            _ => argument.access = checked_trees::CheckedStructuralAccess::Owned,
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "computed literal source mutation {mutation}"
        );
    }
}

#[test]
fn byte_literal_locals_compose_with_primitive_storage_calls_and_subslices() {
    let lowered = lower(LITERAL_LOCALS).expect("ordinary literal locals lower");
    assert_eq!(execute(LITERAL_LOCALS), vec![3, 3, 3, 1, 4]);
    let mut found = 0;
    for machine in &lowered.semantic_module.machines {
        let mut ordinals = Vec::new();
        for declaration in &machine.structural_places {
            let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                ..
            } = declaration.kind
            else {
                continue;
            };
            ordinals.push(declaration_ordinal);
            let producers = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| {
                    matches!(operation.kind,
                    terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. }
                        if destination == declaration.id)
                })
                .collect::<Vec<_>>();
            assert_eq!(producers.len(), 1);
            assert_eq!(producers[0].result, terminal_psi::OperationResult::Unit);
            found += 1;
        }
        ordinals.sort_unstable();
        assert_eq!(
            ordinals,
            (0..u32::try_from(ordinals.len()).unwrap()).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        found, 3,
        "two named literals and the call's literal share one ordinal space"
    );
}

#[test]
fn a_byte_literal_local_flows_through_a_state_edge() {
    let source = r#"
        boundary trait Output { machine observe(value: u64) reaches Output; }
        data Helper {}
        machine Helper::measure(bytes: &[u8]) reaches Output { Output::observe(bytes.len); }
        data Root {}
        machine Root::enter() reaches Output {
            let bytes: &[u8] = "abc";
            Helper::measure(bytes);
            transition { _ -> next(bytes) }
            state next(bytes: &[u8]) { Output::observe(bytes.len); }
        }
    "#;
    assert_eq!(execute(source), vec![3, 3]);
}

#[test]
fn byte_literal_local_replay_rejects_changed_payload_source_type_and_ordinal() {
    use checked_trees::CheckedUnitEffectOperationPlan;
    let original = crate::front_end::checked_program(LITERAL_LOCALS);
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("the untampered source lowers");
    for fault in 0..6 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| {
                plan.operations.iter().any(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                    )
                })
            })
            .expect("literal locals use the ordinary operation route");
        let index = plan
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishByteSequence { .. }
                )
            })
            .unwrap();
        if fault == 5 {
            plan.operations
                .insert(index + 1, plan.operations[index].clone());
        } else {
            let CheckedUnitEffectOperationPlan::EstablishByteSequence { result, value, .. } =
                &mut plan.operations[index]
            else {
                unreachable!()
            };
            match fault {
                0 => {
                    let checked_trees::CheckedByteSequenceValue::Literal(bytes) =
                        value.nodes.get_mut(value.root)
                    else {
                        unreachable!()
                    };
                    bytes[0] = b'X';
                }
                1 => result.statement_index = 0,
                2 => result.type_identity = "u64".to_owned(),
                3 => result.binding_ordinal += 1,
                4 => result.multiplicity = language_semantics::Multiplicity::Linear,
                _ => unreachable!(),
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "literal source replay must reject fault {fault}"
        );
    }
}

#[test]
fn a_multistate_byte_literal_local_is_observed_by_a_boundary() {
    #[derive(Default)]
    struct ObservedBytes(Vec<Vec<Option<Vec<u8>>>>);
    impl TerminalEffectHandler for ObservedBytes {
        fn handle_effect(
            &mut self,
            effect: &TerminalEffect,
        ) -> Result<(), TerminalEffectRejection> {
            let TerminalEffect::BoundaryCall {
                arguments,
                byte_sequence_arguments,
                ..
            } = effect
            else {
                panic!("expected a byte observation boundary");
            };
            assert!(arguments.is_empty());
            self.0.push(byte_sequence_arguments.clone());
            Ok(())
        }
    }
    let source = r#"
        boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
        data Root {}
        machine Root::enter() reaches Output {
            transition { _ -> establish() }
            state establish() {
                let bytes: &[u8] = "\x00\x80\xFF";
                Output::write(bytes);
                transition { _ -> forwarded(bytes) }
            }
            state forwarded(bytes: &[u8]) { Output::write(bytes); }
        }
    "#;
    let lowered = lower(source).expect("a graph boundary borrows a named literal local");
    assert_literal_successor_custody(&lowered.semantic_module);
    let mut observed = ObservedBytes::default();
    let result = interpret_terminal_artifact_measured(
        &encode_module(&lowered.semantic_module).unwrap(),
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut observed,
    )
    .unwrap();
    assert_eq!(result.value(), TerminalExecutionResult::Unit);
    assert_eq!(
        observed.0,
        vec![vec![Some(vec![0, 128, 255])]; 2],
        "the boundary observes the same literal bytes before and after the state edge"
    );
}

#[test]
fn byte_literal_call_operands_reject_owned_access_and_sibling_bindings() {
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    let original = crate::front_end::checked_program(LITERAL_LOCALS);
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("the named literal's ordinary call is valid");
    for fault in 0..5 {
        let mut changed = original.clone();
        let argument = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .filter_map(CheckedUnitEffectOperationPlan::call_mut)
            .filter(|call| {
                !call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
            })
            .find_map(|call| {
                call.structural_arguments.iter_mut().find(|argument| {
                    argument.source
                        == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 0,
                        })
                })
            })
            .expect("the helper call retains its named literal binding");
        match fault {
            0 => argument.access = CheckedStructuralAccess::Owned,
            1 => argument.access = CheckedStructuralAccess::MutableBorrow,
            2 => {
                argument.source =
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: 1 }
            }
            3 => argument.path.push(CheckedUnitStructuralPathSegment::Field(
                "missing".to_owned(),
            )),
            4 => argument.type_identity = "u64".to_owned(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "literal call operand fault {fault} must not gain ownership or name another producer"
        );
    }
}

/// Check the semantic artifact directly so rejection cannot come merely from
/// an unchanged proof digest. The dominance controls remove the earlier
/// boundary observation, leaving the edge itself as the first literal use.
fn assert_literal_successor_custody(module: &terminal_psi::TerminalModule) {
    use terminal_psi::{OperationKind, StructuralAccess, StructuralPathSegment, Terminator};
    use terminal_verifier::ModuleError;
    terminal_verifier::validate_module(module).expect("the baseline literal successor is valid");
    let (machine_index, source_index, literal, edge, target) = module
        .machines
        .iter()
        .enumerate()
        .find_map(|(machine_index, machine)| {
            machine
                .blocks
                .iter()
                .enumerate()
                .find_map(|(source_index, block)| {
                    let Terminator::Jump {
                        edge,
                        target,
                        structural_arguments,
                        ..
                    } = &block.terminator
                    else {
                        return None;
                    };
                    let literal = structural_arguments.iter().find_map(|argument| {
                        machine
                            .structural_places
                            .iter()
                            .find(|place| {
                                place.id == argument.place
                                    && matches!(place.kind,
                    semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. })
                            })
                            .map(|place| place.id)
                    })?;
                    Some((machine_index, source_index, literal, *edge, *target))
                })
        })
        .expect("the artifact lends a literal on an explicit state edge");
    for fault in 0..5 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[machine_index];
        if fault < 3 {
            let Terminator::Jump {
                structural_arguments,
                ..
            } = &mut machine.blocks[source_index].terminator
            else {
                unreachable!()
            };
            let argument = structural_arguments
                .iter_mut()
                .find(|argument| argument.place == literal)
                .unwrap();
            match fault {
                0 => argument.access = StructuralAccess::Owned,
                1 => argument.access = StructuralAccess::MutableBorrow,
                2 => argument.path.push(StructuralPathSegment::FixedIndex(0)),
                _ => unreachable!(),
            }
        } else {
            let source = &mut machine.blocks[source_index];
            source
                .operations
                .retain(|operation| !matches!(operation.kind, OperationKind::BoundaryCall { .. }));
            let producer_index = source.operations.iter().position(|operation| matches!(operation.kind,
                OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == literal)).unwrap();
            let producer = source.operations.remove(producer_index);
            if fault == 4 {
                machine
                    .blocks
                    .iter_mut()
                    .find(|block| block.id == target)
                    .unwrap()
                    .operations
                    .insert(0, producer);
            }
        }
        let error = terminal_verifier::validate_module(&changed)
            .expect_err("forged literal successor must reject");
        if fault == 3 {
            assert_eq!(
                error,
                ModuleError::ByteSequenceLiteralEstablishmentMismatch(
                    changed.machines[machine_index].id
                ),
                "the literal declaration requires its unique establishment"
            );
        } else {
            assert_eq!(
                error,
                ModuleError::InvalidStructuralSuccessorArgument {
                    edge,
                    place: literal
                },
                "fault {fault} must fail the edge's whole shared dominating literal custody"
            );
        }
    }
}

//! Copy-valued fields become owned operands without moving their backing
//! storage, whether the source root is owned or borrowed.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{
    TerminalProductionCustody, TerminalProductionRequest, TerminalProductionTimings,
};

const MUTABLE_LOCAL_WITNESS: &str = r#"
    data Settings [copy] { count: u32; }
    machine make() -> Settings { Settings { count: 1 } }
    machine set(settings: &mut Settings, count: u32) { settings.count = count; }
    machine capture(output: &mut u32, settings: Settings) { output = settings.count; }
    machine run(output: &mut u32) {
        let mut settings: Settings = make();
        set(&mut settings, 17);
        capture(output, settings);
        set(&mut settings, 25);
        capture(output, settings);
    }
"#;

#[test]
fn nested_copied_operand_retains_its_own_consuming_call() {
    let source = r#"
        data Direction [copy] { case East; case West; }
        machine identity(direction: Direction) -> Direction { direction }
        machine consume(first: Direction, second: Direction) {}
        machine run(direction: Direction) { consume(identity(direction), direction); }
    "#;
    let mut checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("nested and outer copies keep different consuming call coordinates");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("authored caller")
        .symbol;
    let source = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == machine)
        .expect("caller plan")
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                operand_source:
                    Some(
                        source @ checked_trees::CheckedArrayConstructionSource::CallArgument {
                            call_ordinal: 1,
                            ..
                        },
                    ),
                ..
            } => Some(source),
            _ => None,
        })
        .expect("nested copy names its actual consumer");
    *source = checked_trees::CheckedArrayConstructionSource::CallArgument {
        call_ordinal: 0,
        parameter_position: 0,
    };
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect_err("an earlier operation is insufficient: the exact consumer must agree");
}

#[test]
fn copied_local_operand_rejects_initializer_and_other_snapshot_substitution() {
    let checked = crate::front_end::checked_program(MUTABLE_LOCAL_WITNESS);
    let artifact = TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("run"))
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("each mutable-local read retains copy custody")
        .into_artifact();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode module"),
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("decode proof"),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("reloaded snapshots retain independently verified copy custody");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("authored caller")
        .symbol;
    let operations = &checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("caller operations")
        .operations;
    let initializer = operations
        .iter()
        .filter_map(|operation| operation.call())
        .find_map(|call| call.structural_result())
        .expect("named local initializer")
        .binding_ordinal;
    let copies = operations
        .iter()
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                result,
                operand_source:
                    Some(checked_trees::CheckedArrayConstructionSource::CallArgument { .. }),
                ..
            } => Some(result.binding_ordinal),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2, "separate snapshots around mutation");
    for substituted in [initializer, copies[0]] {
        let mut changed = crate::front_end::checked_program(MUTABLE_LOCAL_WITNESS);
        let argument = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("caller plan")
            .operations
            .iter_mut()
            .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
            .flat_map(|call| &mut call.structural_arguments)
            .find(|argument| argument.source_structural_result_binding_ordinal() == Some(copies[1]))
            .expect("second copied operand");
        argument.source =
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: substituted,
            };
        checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("run"),
        )
        .expect_err("equal types and shared backing storage cannot substitute the exact read");
    }
}

const WITNESS: &str = r#"
    domain [u8; 16]::Text requires valid_utf8(self);
    data Label [copy] { text: [u8; 16] in Text; }
    data SourceId [copy] { value: u32; labels: [Label; 2]; }
    data Source { id: SourceId; length: u32; }
    data Holder { source: Source; }
    data Target { seen: u32; }
    machine Target::initialize(&mut self, id: SourceId, length: u64) {
        self.seen = id.value;
    }
    data Main { holder: Holder; target: Target; }
    machine Main::main(&mut self) {
        self.target.initialize(self.holder.source.id, self.holder.source.length as u64);
    }
"#;

const ASSIGNMENT_WITNESS: &str = r#"
    data SourceId [copy] { value: u32; }
    data Target { seen: u32; }
    machine Target::read(&self, id: SourceId) -> u32 { id.value }
    data Main { source: SourceId; target: Target; result: u32; }
    machine Main::main(&mut self) {
        self.target.seen = 9;
        self.result = self.target.read(self.source);
        self.target.seen = 7;
    }
"#;

const BOUNDARY_ARRAY_WITNESS: &str = r#"
    boundary trait Reader {
        machine read(&mut self, prefix: u32, first: [u8; 2], middle: u32, second: [u8; 2]) -> u32;
    }
    data Main<'a> {
        reader: &'a mut Reader;
        first: [u8; 2];
        second: [u8; 2];
        result: u32;
    }
    machine Main::main(&mut self) reaches Reader {
        self.result = self.reader.read(11, self.first, 13, self.second);
    }
"#;

#[test]
fn copied_boundary_arrays_retain_authored_positions_across_receiver_erasure() {
    let checked = crate::front_end::checked_program(BOUNDARY_ARRAY_WITNESS);
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::main"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("copied arrays preserve scalar interleave after the service receiver is erased")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("reloaded boundary operands preserve exact array custody");

    let mut changed = checked;
    let arguments = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|plan| &mut plan.operations)
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::call_mut)
        .find(|call| call.is_boundary() && call.scalar_result().is_some())
        .map(|call| &mut call.structural_arguments)
        .expect("one scalar-returning boundary call");
    assert_eq!(arguments.len(), 2);
    arguments.swap(0, 1);
    checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect_err("equal array types cannot replace the exact authored operand occurrence");
}

#[test]
fn copied_nested_byte_records_retain_owned_predicates() {
    let _ = assert_copied_operand(WITNESS);
}

#[test]
fn copied_bounded_byte_field_retains_owned_predicates() {
    let artifact = assert_copied_operand(
        r#"
        domain [u8; 16]::Text requires valid_utf8(self);
        data Target { seen: u64; }
        machine Target::capture(&mut self, text: [u8; 16] in Text) {
            self.seen = 42;
        }
        data Main { text: [u8; 16] in Text; target: Target; }
        machine Main::main(&mut self) {
            self.target.capture(self.text);
        }
        "#,
    );
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::StructuralLeafCopy { .. }
                    )
                })
            })
        })
        .unwrap();
    let copied_place = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::StructuralLeafCopy { .. }
            )
        })
        .unwrap()
        .result
        .structural()
        .unwrap()
        .place;
    let consumer = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(&operation.kind,
            terminal_psi::OperationKind::CallUnit { structural_arguments, .. }
                if structural_arguments.iter().any(|argument| argument.place == copied_place))
        })
        .unwrap();
    assert!(
        terminal_semantics::byte_copied_field_predicates_at_call(
            &module,
            caller,
            copied_place,
            consumer.id,
        )
        .is_some()
    );
    let mut clobbered = caller.clone();
    let mut extra_call = consumer.clone();
    extra_call.id = semantic_vocabulary::OperationId::new(u64::MAX).unwrap();
    clobbered.blocks[0].operations.push(extra_call);
    assert!(
        terminal_semantics::byte_copied_field_predicates_at_call(
            &module,
            &clobbered,
            copied_place,
            consumer.id,
        )
        .is_none(),
        "another owned use cannot inherit immutable-snapshot evidence"
    );
    let mut changed = module.clone();
    let field = changed
        .structural_types
        .iter_mut()
        .find_map(|declaration| {
            let terminal_psi::StructuralTypeShape::Record { fields } = &mut declaration.shape
            else {
                return None;
            };
            fields.iter_mut().find(|field| {
                matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::PredicateByteSequence { .. }
                )
            })
        })
        .expect("qualified source byte field");
    let carrier = field.field_type.byte_sequence_carrier().unwrap();
    field.field_type = terminal_psi::StructuralFieldType::ByteSequence(carrier);
    assert!(matches!(
        terminal_verifier::validate_module(&changed),
        Err(terminal_verifier::ModuleError::StructuralArgumentMissingQualification { .. })
    ));
    terminal_verifier::verify_module(
        &changed,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("equal capacity cannot replace the source field's UTF-8 invariant");

    let mut changed = module;
    let caller = changed
        .machines
        .iter_mut()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::StructuralLeafCopy { .. }
                    )
                })
            })
        })
        .expect("copying caller");
    let source = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralLeafCopy { source, .. } => Some(source),
            _ => None,
        })
        .unwrap();
    caller
        .structural_parameters
        .iter_mut()
        .find(|parameter| parameter.place == source)
        .expect("borrowed source parameter")
        .access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
    terminal_verifier::verify_module(
        &changed,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect_err("qualified byte storage does not grant read access to a write-only root");
}

#[test]
fn copied_call_result_stores_compose_with_state_transitions() {
    let scalar = ASSIGNMENT_WITNESS.replace(
        "self.target.seen = 7;",
        "transition { _ -> done() } state done(&mut self) { self.target.seen = 7; }",
    );
    let _ = assert_copied_operand(&scalar);
    let structural = scalar
        .replace("-> u32 { id.value }", "-> SourceId { id }")
        .replace("result: u32", "result: SourceId");
    let _ = assert_copied_operand(&structural);
}

#[test]
fn assignment_call_cannot_move_a_noncopy_borrowed_field() {
    let source = ASSIGNMENT_WITNESS.replace("SourceId [copy]", "SourceId");
    crate::front_end::checked_program_result(&source)
        .expect_err("a result store does not authorize moving its argument out of a borrow");
}

#[test]
fn copied_assignment_operand_rejects_substituted_call_coordinates() {
    for corrupt_formal in [false, true] {
        let mut checked = crate::front_end::checked_program(ASSIGNMENT_WITNESS);
        let source = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    operand_source: Some(source),
                    ..
                } => Some(source),
                _ => None,
            })
            .expect("the copied argument retains its call coordinate");
        let checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        } = source
        else {
            panic!("the copied value belongs to a call argument");
        };
        if corrupt_formal {
            *parameter_position = 0;
        } else {
            *call_ordinal = u32::MAX;
        }
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::main"),
        )
        .expect_err("source replay must retain the exact call and formal");
    }
}

fn assert_copied_operand(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let checked = crate::front_end::checked_program(source);
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("Main::main"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("an ordinary call accepts a copied field from its borrowed receiver")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification accepts the copied operand and retained loan");
    let copies = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::StructuralLeafCopy { .. }
            )
        })
        .count();
    assert_eq!(copies, 1, "the argument has one fresh owned copy");
    artifact
}

#[test]
fn noncopy_field_cannot_be_moved_out_of_a_borrowed_receiver() {
    let source = WITNESS.replace("SourceId [copy]", "SourceId");
    crate::front_end::checked_program_result(&source)
        .expect_err("removing copy semantics must reject the borrowed-field move");
}

#[test]
fn copied_operand_rejects_a_substituted_source_path() {
    let mut checked = crate::front_end::checked_program(WITNESS);
    let plans = &mut checked.facts.values.structural_values;
    let copy = plans
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
            )
            .then_some(handle)
        })
        .expect("the call argument retains its checked copy source");
    let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
        &mut plans.nodes.get_mut(copy).kind
    else {
        unreachable!();
    };
    assert!(!source.path.is_empty());
    source.path.clear();
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect_err("independent source replay rejects an exchanged copy path");
}

#[test]
fn owned_nested_copy_rejects_a_same_typed_sibling_source() {
    let mut checked = crate::front_end::checked_program(
        "domain [u8; 8]::Text requires valid_utf8(self);
         data Leaf [copy] { name: [u8; 8] in Text; value: u64; }
         data Event { chosen: Leaf; sibling: Leaf; }
         data Room { event: Event; }
         machine copy_room(source: Room, destination: &mut Room) {
             destination.event.chosen = source.event.chosen;
             destination.event.sibling = source.event.sibling;
             destination.event.chosen.value = source.event.chosen.value;
         }",
    );
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("copy_room"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("an affine owned root permits copying its unrestricted nested leaves")
            .into_artifact();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode module"),
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("decode proof"),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("nested qualified leaves preserve copy custody and source readability");

    let copies = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .filter_map(|(handle, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } => {
                Some((handle, source.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        copies.len(),
        2,
        "each field assignment snapshots its source"
    );
    let [(selected, chosen), (_, sibling)] = copies.as_slice() else {
        unreachable!();
    };
    assert_eq!(chosen.source, sibling.source, "both fields share one owner");
    assert_eq!(chosen.type_identity, sibling.type_identity);
    assert_eq!(chosen.path.len(), 2);
    assert_eq!(sibling.path.len(), 2);
    assert_eq!(chosen.path[0], sibling.path[0]);
    assert_ne!(chosen.path[1], sibling.path[1]);

    let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
        &mut checked
            .facts
            .values
            .structural_values
            .nodes
            .get_mut(*selected)
            .kind
    else {
        unreachable!();
    };
    source.path[1] = sibling.path[1].clone();
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("copy_room"),
    )
    .expect_err("a valid same-typed sibling cannot replace the authored copy source");
}

/// A whole `[copy]` case local passed to an unrestricted scalar call is
/// snapshotted into a fresh scalar case before the call. The interpreter keeps
/// that snapshot in scalar-case storage, and the callee binds the snapshot,
/// whose producer is the copy rather than the local's construction.
#[test]
fn copied_case_local_operand_executes_from_its_snapshot() {
    let checked = crate::front_end::checked_program(
        "data Choice [copy] { case Some(value: u8); case Empty; }
         machine answer(choice: Choice, fallback: u8) -> u8 {
             transition choice { Choice::Some { value } -> (value) Choice::Empty -> (fallback) }
         }
         machine selected(value: u8) -> u8 {
             let choice: Choice = Choice::Some { value: value };
             answer(choice, 9u8)
         }",
    );
    let artifact =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("selected"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("the copied case operand lowers")
            .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                &operation.kind,
                terminal_psi::OperationKind::StructuralLeafCopy { path, .. } if path.is_empty()
            )),
        "the case operand is a whole copy"
    );
    let byte = |value: u8| terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            8,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
    };
    for value in [0u8, 42] {
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[byte(value)],
            )
            .expect("the snapshot is an ordinary scalar-case actual"),
            terminal_interpreter::TerminalExecutionResult::Scalar(byte(value))
        );
    }
}

//! Byte-field reads retain their exact readable owner and current length.

use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{OperationKind, StructuralAccess, StructuralTypeShape};

#[test]
fn qualified_record_state_copy_replays_exact_source_field() {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
        CheckedUnitStructuralPathSegment,
    };

    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        domain [u8; 16]::Utf8 requires valid_utf8(self);
        data Record [copy] { text: [u8; 8] in Utf8; sibling: u64; }
        data Root { record: Record; alternate: Record; output: [u8; 16] in Utf8; }
        machine Root::enter(&mut self) {
            self.record.text = "Gate";
            self.alternate.text = "Other";
            transition { _ -> take(self.record) }
            state take(&mut self, record: Record) {
                self.output = record.text + "!";
            }
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("qualified record crosses a state edge with its exact contents");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("state-owned byte field read verifies independently");

    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| checked.symbols.display_path(plan.machine, "::") == "Root::enter")
        .expect("named-state graph");
    let alternate = checked
        .data_definitions()
        .iter()
        .find(|definition| checked.symbols.display_path(definition.symbol, "::") == "Root")
        .and_then(|definition| {
            checked.data_members(definition).iter().find_map(|member| {
                let checked_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.name.as_str() == "alternate").then(|| field.path_identity())
            })
        })
        .expect("same-typed alternative source field");

    for clear_path in [false, true] {
        let mut changed = checked.clone();
        let plan = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index];
        let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
            &mut plan.states[0].terminator
        else {
            panic!("entry has one authored named-state jump")
        };
        let subject = successor
            .transfers
            .iter_mut()
            .find_map(|transfer| match &mut transfer.source {
                CheckedStructuralControlTransferSourcePlan::ProjectedCopy { subject } => {
                    Some(subject)
                }
                _ => None,
            })
            .expect("record argument retains projected copy custody");
        assert_eq!(subject.path.len(), 1, "one exact source field");
        if clear_path {
            subject.path.clear();
        } else {
            subject.path[0] = CheckedUnitStructuralPathSegment::Field(alternate.clone());
        }
        lower_machine(&changed, TerminalMachineSelection::Name("Root::enter"))
            .expect_err("missing path or same-typed sibling cannot replace authored record source");
    }
}

#[test]
fn record_byte_field_concatenation_replays_owner_field_and_length() {
    for access in ["", "&", "&mut "] {
        let checked = crate::front_end::checked_program(&format!(
            r#"
            domain [u8; 8]::Utf8 requires valid_utf8(self);
            domain [u8; 16]::Utf8 requires valid_utf8(self);
            data Record [copy] {{
                text: [u8; 8] in Utf8;
                alternate: [u8; 8] in Utf8;
                sibling: u64;
            }}
            machine fill(record: {access}Record, output: &mut [u8; 16] in Utf8) {{
                output = record.text + "!";
            }}
            "#,
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
            .unwrap_or_else(|error| panic!("readable record access {access:?}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("exact qualified byte-field read verifies independently");

        let module = &lowered.semantic_module;
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("selected fill machine");
        let (concatenation, source, field, literal_length) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                OperationKind::EstablishByteSequenceConcatenation {
                    left: ByteSequenceOperand::Field { source, field, .. },
                    right,
                    ..
                } => Some((operation.id, source.place, *field, right.length())),
                _ => None,
            })
            .expect("field concatenation retains its original owner");
        let parameter = machine
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == source)
            .expect("byte field is rooted at the record formal");
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .expect("record declaration");
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            panic!("record formal")
        };
        let selected = fields
            .iter()
            .find(|candidate| candidate.id == field)
            .unwrap();
        let alternate = fields
            .iter()
            .find(|candidate| candidate.id != field && candidate.field_type == selected.field_type)
            .expect("same-qualified sibling field")
            .id;

        for corruption in ["write-only owner", "sibling field", "unrelated length"] {
            let mut changed = module.clone();
            let entry = changed.entry;
            let machine = changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == entry)
                .unwrap();
            if corruption == "write-only owner" {
                machine
                    .structural_parameters
                    .iter_mut()
                    .find(|parameter| parameter.place == source)
                    .unwrap()
                    .access = StructuralAccess::WriteOnlyBorrow;
            } else {
                let operation = machine
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| operation.id == concatenation)
                    .unwrap();
                let OperationKind::EstablishByteSequenceConcatenation {
                    left: ByteSequenceOperand::Field { field, length, .. },
                    ..
                } = &mut operation.kind
                else {
                    panic!("exact field concatenation")
                };
                if corruption == "sibling field" {
                    *field = alternate;
                } else {
                    *length = literal_length;
                }
            }
            // Semantic replay must reject the actual content/length mismatch,
            // not merely the old proof bundle's identity after a mutation.
            assert!(
                matches!(
                    terminal_verifier::validate_module(&changed),
                    Err(terminal_verifier::ModuleError::InvalidByteSequenceConcatenation(operation))
                        if operation == concatenation
                ),
                "access {access:?}, corruption {corruption} must reject at concatenation"
            );
        }
    }
}

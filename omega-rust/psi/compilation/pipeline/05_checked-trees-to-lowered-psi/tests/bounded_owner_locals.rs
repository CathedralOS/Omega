//! A bounded byte local built from a literal owns fresh backing and lends it
//! to a static machine that forwards the qualified shared loan across states.
//!
//! `let text: [u8; 8] in Utf8 = "150";` is a bounded owner with live length
//! 3. Lowering copies the literal into owned backing through the ordinary
//! concatenation materialization, lends the owner whole to the callee's
//! `&[u8; 8] in Utf8` formal, and the callee forwards that loan to a
//! successor block with the same qualifications. Terminal verification
//! replays the local's predicates from its operands and the successor's from
//! its exact forwarded source, so either link rejects when corrupted.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{ByteSequenceCarrier, OperationKind, StructuralAccess, StructuralTypeShape};

const SOURCE: &str = r#"
domain [u8]::Utf8 requires valid_utf8(self);
domain [u8; 8]::Utf8 requires valid_utf8(self);

machine forwarded_length(text: &[u8; 8] in Utf8) -> u64 {
    transition { _ -> measure(text) }

    state measure(text: &[u8; 8] in Utf8) -> u64 {
        transition { _ -> (text.len) }
    }
}

machine main() -> u64 {
    let text: [u8; 8] in Utf8 = "150";
    let length: u64 = forwarded_length(&text);
    transition { _ -> (length) }
}
"#;

#[test]
fn owned_record_byte_predicates_survive_state_signatures() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        data Room [copy] { title: [u8; 8] in Utf8; }
        data Level [copy] { rooms: [Room; 2]; count: u64; }
        machine edit(level: Level, output: &mut [u8; 8] in Utf8, choose: bool)
        ensures output in Utf8 {
            transition choose {
                true -> first(level, output)
                false -> second(level, output)
            }
            state first(level: Level, text: &mut [u8; 8] in Utf8) {
                text = "first";
                transition { _ -> finish(level, text) }
            }
            state second(level: Level, text: &mut [u8; 8] in Utf8) {
                text = "second";
                transition { _ -> finish(level, text) }
            }
            state finish(level: Level, text: &mut [u8; 8] in Utf8) {
                text = "done";
            }
        }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("edit"),
    )
    .expect("established records retain their represented byte predicates through states");
    verify(&lowered).expect("record transfers retain exact incoming value custody");
    let module = &lowered.semantic_module;
    assert!(
        module.structural_types.iter().any(|array| {
            let StructuralTypeShape::FixedArray { element, length: 2 } = array.shape else {
                return false;
            };
            module.structural_types.iter().any(|room| {
                room.id == element
                    && matches!(&room.shape, StructuralTypeShape::Record { fields }
                if fields.iter().any(|field| matches!(&field.field_type,
                    terminal_psi::StructuralFieldType::PredicateByteSequence {
                        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 8 }, predicates
                    } if predicates.predicates().any(|predicate|
                        predicate == terminal_psi::ByteSequencePredicate::ValidUtf8))))
            })
        }),
        "the array extent, bounded storage and nested UTF-8 predicate survive together"
    );

    let mut changed_access = module.clone();
    let machine = changed_access
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structural_parameters)
        .find(|parameter| parameter.access == StructuralAccess::Owned)
        .expect("owned Level state parameter")
        .access = StructuralAccess::SharedBorrow;
    assert!(
        terminal_verifier::validate_module(&changed_access).is_err(),
        "a state cannot reinterpret its incoming owned value as a borrowed parameter"
    );
}

#[test]
fn mutable_qualified_byte_owner_survives_branch_and_joined_state_transfers() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        machine edit(output: &mut [u8; 8] in Utf8, choose: bool)
        ensures output in Utf8 {
            transition choose {
                true -> first(output)
                false -> second(output)
            }
            state first(text: &mut [u8; 8] in Utf8) {
                text = "first";
                transition { _ -> finish(text) }
            }
            state second(text: &mut [u8; 8] in Utf8) {
                text = "second";
                transition { _ -> finish(text) }
            }
            state finish(text: &mut [u8; 8] in Utf8) {
                text = "done";
            }
        }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("edit"),
    )
    .expect("each state retains the same mutable qualified owner");
    verify(&lowered).expect("every incoming edge and replacement preserves the byte invariant");

    let mut stale_entry = lowered.semantic_module.clone();
    let entry = stale_entry.entry;
    let machine = stale_entry
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap();
    let old_name = machine.structural_parameters[0].place;
    let store = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::ByteSequenceStore { destination, .. } => Some(destination),
            _ => None,
        })
        .expect("state replacement");
    *store = old_name;
    assert!(
        matches!(
            terminal_verifier::validate_module(&stale_entry),
            Err(terminal_verifier::ModuleError::ByteSequenceViewNotEstablished { .. })
        ),
        "a transferred entry name cannot write the buffer again"
    );

    let mut weakened = lowered.semantic_module.clone();
    weakened
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structural_parameters)
        .find(|parameter| !parameter.qualifications.is_empty())
        .unwrap()
        .qualifications
        .clear();
    assert!(
        terminal_verifier::validate_module(&weakened).is_err(),
        "a mutable state loan cannot erase its incoming invariant"
    );

    let mut invalid_bytes = lowered.semantic_module.clone();
    let bytes = invalid_bytes
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::EstablishByteSequenceLiteral { bytes, .. } if !bytes.is_empty() => {
                Some(bytes)
            }
            _ => None,
        })
        .expect("authored replacement bytes");
    bytes[0] = 0x80;
    assert!(
        terminal_verifier::validate_module(&invalid_bytes).is_err(),
        "state-local storage still requires valid UTF-8 replacement bytes"
    );
}

#[test]
fn owned_literal_call_argument_establishes_its_declared_byte_predicates() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 16]::Text requires valid_utf8(self);
        data Player { name: [u8; 16] in Text; }
        data Factory {}
        data Main { factory: Factory; player: Player; }
        machine Factory::create(&mut self, name: [u8; 16] in Text, player: &mut Player) {
            player.name = name;
        }
        machine Main::run(&mut self) {
            self.factory.create("Wanderer", &mut self.player);
            self.factory.create("Wanderer", &mut self.player);
        }
        "#,
    );
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::run"),
    )
    .expect("the owned literal has its own backing before the qualified call");
    verify(&lowered).expect("the literal's actual bytes establish its declared predicates");
    let owned_places = lowered
        .semantic_module
        .machines
        .iter()
        .filter(|machine| machine.id == lowered.semantic_module.entry)
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::EstablishByteSequenceConcatenation { .. } => {
                operation.result.structural().map(|result| result.place)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        owned_places.len(),
        2,
        "each call materializes an owned operand"
    );
    assert_ne!(
        owned_places[0], owned_places[1],
        "equal literals are not one owned argument"
    );
    let mut corrupted = lowered.semantic_module.clone();
    let bytes = corrupted
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::EstablishByteSequenceLiteral { bytes, .. } if bytes == b"Wanderer" => {
                Some(bytes)
            }
            _ => None,
        })
        .expect("the exact authored literal supplies the owned operand");
    // Preserve length/capacity: rejection must not come from stale bounds.
    bytes[0] = 0x80;
    let error = terminal_verifier::verify_module(
        &corrupted,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect_err("changing the literal to invalid UTF-8 cannot retain its qualification");
    assert!(
        format!("{error:?}").contains("InvalidByteSequenceConcatenation"),
        "semantic predicate replay, not a changed proof identity, rejects: {error:?}"
    );
}

#[test]
fn whole_byte_parameter_store_replays_declared_access_and_qualifications() {
    let checked = crate::front_end::checked_program(
        r#"
        domain [u8; 16]::Text requires valid_utf8(self);
        data Destination { text: [u8; 16] in Text; }
        machine copy(text: [u8; 16] in Text, destination: &mut Destination) {
            destination.text = text;
        }
    "#,
    );
    let selected = checked_trees_to_lowered_psi::select_terminal_machine(
        &checked,
        TerminalMachineSelection::Name("copy"),
    )
    .expect("whole-byte store selection")
    .machine;
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("copy"),
    )
    .expect("authored whole owned byte parameter lowers");
    verify(&lowered).expect("declared whole byte source verifies independently");
    for mutation in ["access", "qualifications"] {
        let mut changed = checked.clone();
        let source = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == selected)
            .expect("ordinary copy plan")
            .structural_parameters
            .iter_mut()
            .find(|parameter| parameter.position == 0)
            .expect("owned byte formal");
        match mutation {
            "access" => source.access = checked_trees::CheckedStructuralAccess::SharedBorrow,
            "qualifications" => source.qualifications.clear(),
            _ => unreachable!(),
        }
        let error = checked_trees_to_lowered_psi::lower_machine(
            &changed,
            TerminalMachineSelection::Name("copy"),
        )
        .expect_err("same-layout source cannot change its authored custody");
        assert!(
            format!("{error:?}").contains("whole byte checked formal changed its authored custody"),
            "exact source rejoin rejects {mutation}: {error:?}"
        );
    }
}

fn lower() -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("main"))
        .expect("a lent bounded owner local lowers to Terminal Psi")
}

fn verify(lowered: &lowered_psi::LoweredPsi) -> Result<(), String> {
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .map(|_| ())
    .map_err(|error| format!("{error:?}"))
}

fn bounded_owner(
    module: &terminal_psi::TerminalModule,
    id: semantic_vocabulary::StructuralTypeId,
) -> bool {
    module.structural_types.iter().any(|row| {
        row.id == id
            && row.shape
                == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 8,
                })
    })
}

#[test]
fn literal_local_owns_its_backing_and_its_loan_is_forwarded_qualified() {
    let lowered = lower();
    verify(&lowered).expect("the owned local and its forwarded loan verify");
    let module = &lowered.semantic_module;
    let owned = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(
            |operation| match (&operation.kind, operation.result.structural()) {
                (OperationKind::EstablishByteSequenceConcatenation { .. }, Some(result)) => {
                    Some(result.structural_type)
                }
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert!(
        matches!(owned.as_slice(), [owner] if bounded_owner(module, *owner)),
        "the local is one owned bounded concatenation of its declared carrier: {owned:?}"
    );
    let forwarded = module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter().skip(1))
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| {
            parameter.access == StructuralAccess::SharedBorrow
                && !parameter.qualifications.is_empty()
                && bounded_owner(module, parameter.structural_type)
        })
        .count();
    assert_eq!(
        forwarded, 1,
        "the successor state receives the qualified owner loan"
    );
}

#[test]
fn a_successor_loan_cannot_drop_or_change_its_qualifications() {
    let mut lowered = lower();
    let module = &mut lowered.semantic_module;
    let parameter = module
        .machines
        .iter_mut()
        .flat_map(|machine| machine.blocks.iter_mut().skip(1))
        .flat_map(|block| block.structural_parameters.iter_mut())
        .find(|parameter| !parameter.qualifications.is_empty())
        .expect("qualified successor loan");
    parameter.qualifications.clear();
    let error = verify(&lowered).expect_err("a successor loan cannot forget its qualifications");
    assert!(
        error.contains("InvalidStructuralSuccessorArgument"),
        "the forwarding edge rejects: {error}"
    );
}

/// The local declares `in Utf8`, so its owned backing must establish that
/// predicate from its operands before any loan of it reaches the formal. A
/// same-length replacement keeps every length fact intact: valid UTF-8 still
/// verifies, and invalid UTF-8 rejects at the owner's establishment.
#[test]
fn an_owned_local_must_establish_every_predicate_its_formal_requires() {
    for (replacement, valid) in [(b"151".to_vec(), true), (vec![0x80, b'5', b'0'], false)] {
        let mut lowered = lower();
        let literal = lowered
            .semantic_module
            .machines
            .iter_mut()
            .flat_map(|machine| machine.blocks.iter_mut())
            .flat_map(|block| block.operations.iter_mut())
            .find_map(|operation| match &mut operation.kind {
                OperationKind::EstablishByteSequenceLiteral { bytes, .. } if bytes == b"150" => {
                    Some(bytes)
                }
                _ => None,
            })
            .expect("the local's literal operand");
        *literal = replacement;
        if valid {
            verify(&lowered).expect("another valid UTF-8 literal establishes the owner");
            continue;
        }
        let error = verify(&lowered).expect_err("invalid UTF-8 cannot establish a Utf8 owner");
        assert!(
            error.contains("InvalidByteSequenceConcatenation"),
            "the owner establishment rejects: {error}"
        );
    }
}

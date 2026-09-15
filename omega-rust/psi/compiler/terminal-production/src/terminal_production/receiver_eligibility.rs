//! Source eligibility cannot be recovered from native layout: identical record
//! shapes may have different zero gates and nominal cleanup. Derive this private
//! correspondence before source erasure and bind it to the exact Terminal
//! attachment and retained or erased receiver projection.
//! It is not a standalone proof from source-free Terminal bytes or root authority.
//! Byte carriers erase their domain predicates, so empty-buffer eligibility must
//! rejoin the actual source field and evaluate every understood value constraint.

use checked_trees::data::{DataDefinition, DataMember};
use checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{CheckedTerminalMachineSelection, CheckedTrees};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    CheckedProgramEntryFusedServiceField, CheckedProgramEntryReceiverEligibility,
    CheckedProgramEntryReceiverProjection, StructuralAccess, StructuralFieldType,
    StructuralTypeShape, TerminalModule,
};

pub(super) fn derive(
    checked: &CheckedTrees,
    selection: &CheckedTerminalMachineSelection,
    module: &TerminalModule,
) -> Option<CheckedProgramEntryReceiverEligibility> {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.machine)?;
    let state = checked.machine_states(machine).first()?;
    let mut receivers = checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.is_self);
    let receiver = receivers.next()?;
    if receivers.next().is_some() || !receiver.is_mutable {
        return None;
    }
    let source_position = u32::try_from(
        checked
            .state_parameters(state)
            .iter()
            .position(|parameter| parameter.is_self)?,
    )
    .ok()?;
    let mut owned = receiver.type_reference;
    loop {
        match checked.type_reference_table.type_reference(owned) {
            TypeReferenceNode::Reference { referee, .. } => owned = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => owned = *base_type,
            _ => break,
        }
    }
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(owned)
    else {
        return None;
    };
    let definition = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == machine.attached_data_symbol)?;
    // Resolved Self names its enclosing machine, not the data declaration.
    // Explicit nominal receivers name the data directly. Both must rejoin the
    // exact attachment; a matching source display name is insufficient.
    if (*symbol != machine.symbol && *symbol != definition.symbol)
        || machine.attached_data.as_ref() != Some(&definition.name)
        || !checked.data_type_parameters(definition).is_empty()
        || !checked
            .proof_facts
            .span_or_empty(definition.where_facts)
            .is_empty()
        || validation::data_requires_establishment(&checked.typed, definition)
        || validation::data_requires_nominal_drop(&checked.typed, definition)
    {
        return None;
    }
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)?;
    let terminal_receiver_type = entry.attachment?;
    let mut parameters = entry
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self);
    let projection = match parameters.next() {
        Some(parameter) => {
            if parameters.next().is_some()
                || parameter.access != StructuralAccess::MutableBorrow
                || parameter.position != source_position
                || parameter.structural_type != terminal_receiver_type
            {
                return None;
            }
            CheckedProgramEntryReceiverProjection::Retained {
                terminal_self: parameter.place,
                source_position,
            }
        }
        None => {
            // Clearing the marker on a surviving parameter is not erasure.
            // Structural positions retain their authored source positions,
            // independently of the compact scalar parameter partition.
            if entry
                .structural_parameters
                .iter()
                .any(|parameter| parameter.position == source_position)
            {
                return None;
            }
            CheckedProgramEntryReceiverProjection::Erased { source_position }
        }
    };
    let mut structural_types = module
        .structural_types
        .iter()
        .filter(|declaration| declaration.id == terminal_receiver_type);
    let structural_type = structural_types.next()?;
    if structural_types.next().is_some() {
        return None;
    }
    let owner_path = checked.symbols.display_path(definition.symbol, "::");
    let mut owned_receiver_type_identity = String::from("named(name(");
    for character in owner_path.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            owned_receiver_type_identity.push('\\');
        }
        owned_receiver_type_identity.push(character);
    }
    owned_receiver_type_identity.push_str("))");
    let StructuralTypeShape::Record { fields } = &structural_type.shape else {
        return None;
    };
    // Plain records and fixed primitive arrays need no initialization program.
    // Array eligibility follows its complete semantic element chain, including
    // empty dimensions; a zero byte count never excuses an invalid element.
    // Erased qualification establishment remains an independent installed
    // occurrence obligation; this correspondence supplies no Bound authority.
    if structural_type.identity != owned_receiver_type_identity
        || !zero_valid_record_storage(
            checked,
            &module.structural_types,
            terminal_receiver_type,
            Some(definition),
            &mut Vec::new(),
        )
    {
        return None;
    }
    let mut fused_service_fields = Vec::new();
    for member in checked.data_members(definition) {
        let DataMember::Field(field) = member else {
            continue;
        };
        let Some(carrier) = checked
            .bound_service_parameter_carrier(field.type_reference)
            .ok()?
        else {
            continue;
        };
        let field_identity = field
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| field.name.as_str().to_owned());
        let mut terminal_fields = fields
            .iter()
            .filter(|candidate| candidate.identity == field_identity);
        let terminal_field = terminal_fields.next()?;
        if terminal_fields.next().is_some()
            || !matches!(
                &terminal_field.field_type,
                StructuralFieldType::Erased { type_identity }
                    if type_identity == &carrier.carrier_type_identity
            )
        {
            return None;
        }
        fused_service_fields.push(CheckedProgramEntryFusedServiceField::new(
            field_identity,
            carrier.carrier_type_identity,
        ));
    }
    fused_service_fields.sort_by(|left, right| left.field_identity().cmp(right.field_identity()));
    if fused_service_fields
        .windows(2)
        .any(|pair| pair[0].field_identity() == pair[1].field_identity())
    {
        return None;
    }
    Some(CheckedProgramEntryReceiverEligibility::new(
        checked
            .normalized_type_identity(receiver.type_reference)
            .into_string(),
        owned_receiver_type_identity,
        projection,
        terminal_receiver_type,
        fused_service_fields,
    ))
}

fn zero_valid_record_storage(
    checked: &CheckedTrees,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    source: Option<&DataDefinition>,
    visiting: &mut Vec<StructuralTypeId>,
) -> bool {
    if visiting.contains(&structural_type) {
        return false;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let Some(declaration) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return terminal_semantics::scalar_array_leaf_shape(declarations.iter(), structural_type)
            .is_some();
    };
    let is_receiver = visiting.is_empty();
    visiting.push(structural_type);
    let valid = fields.iter().all(|field| {
        (is_receiver || !field.relevance.is_erased())
            && match field.field_type {
                StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => true,
                StructuralFieldType::BoundedInteger(integer) => {
                    integer.contains(semantic_vocabulary::IntegerValue::Signed(0))
                        || integer.contains(semantic_vocabulary::IntegerValue::Unsigned(0))
                }
                StructuralFieldType::Structural(child) => {
                    let reference = source_field_type(checked, source, field);
                    let source = source_record(checked, reference).filter(|_| {
                        declarations.iter().any(|declaration| {
                            declaration.id == child
                                && declaration.identity
                                    == checked.normalized_type_identity(reference).into_string()
                        })
                    });
                    zero_valid_record_storage(checked, declarations, child, source, visiting)
                }
                StructuralFieldType::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ) => zero_valid_byte_field(
                    checked,
                    source_field_type(checked, source, field),
                    capacity,
                ),
                // Only the top-level receiver joins erased service establishment.
                StructuralFieldType::Erased { .. } => is_receiver,
                StructuralFieldType::ByteSequence(_) => false,
            }
    });
    visiting.pop();
    valid
}

fn source_field_type(
    checked: &CheckedTrees,
    source: Option<&DataDefinition>,
    field: &terminal_psi::StructuralFieldDeclaration,
) -> TypeReferenceHandle {
    let Some(source) = source else {
        return TypeReferenceHandle::invalid();
    };
    checked
        .data_members(source)
        .iter()
        .find_map(|member| {
            let DataMember::Field(candidate) = member else {
                return None;
            };
            let matches = match candidate.identity {
                Some(identity) => field.identity == format!("#{identity}"),
                None => field.identity == candidate.name.as_str(),
            };
            matches.then_some(candidate.type_reference)
        })
        .unwrap_or_default()
}

fn source_record(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<&DataDefinition> {
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(reference)
    else {
        // Refined nominal paths need their own zero-membership evidence. Do
        // not strip those constraints merely to reach a nested byte field.
        return None;
    };
    checked.data_definitions().iter().find(|definition| {
        definition.symbol == *symbol
            && checked.data_type_parameters(definition).is_empty()
            && checked
                .proof_facts
                .span_or_empty(definition.where_facts)
                .is_empty()
    })
}

fn zero_valid_byte_field(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
    capacity: u64,
) -> bool {
    // Stable contents classify every constraint and exclude authority-bearing,
    // routed and unknown refinements. Stability alone is not membership: the
    // same resolved predicate denotation used by source checking must accept
    // the empty value for every domain, not merely one named Utf8.
    if !validation::has_stable_observable_contents(&checked.typed, reference) {
        return false;
    }
    let predicates =
        checked_trees::byte_predicates::type_reference_domain_predicates(&checked.typed, reference);
    if predicates.is_empty()
        || !predicates
            .iter()
            .all(|(_, predicate)| predicate.is_some_and(|predicate| predicate.holds_for(&[])))
    {
        return false;
    }
    let mut carrier = reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        checked.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    matches!(
        checked.type_reference_table.type_reference(carrier),
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if u64::try_from(*length).ok() == Some(capacity)
            && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    )
}

#[cfg(test)]
mod tests {
    use super::{
        CheckedProgramEntryReceiverProjection, CheckedTrees, StructuralFieldType,
        StructuralTypeShape, derive,
    };
    use crate::TerminalProductionRequest;

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
    }

    const SOURCE: &str =
        "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }";

    #[test]
    fn byte_receiver_zero_eligibility_rejoins_direct_and_named_nested_fields() {
        for (capacity, predicate) in [(3, "valid_utf8"), (9, "ascii_only"), (0, "no_nul")] {
            let checked = check_source(&format!(
                "domain [u8; {capacity}]::SafeBytes requires {predicate}(self);
                 data Child {{ #7 bytes: [u8; {capacity}] in SafeBytes; }}
                 data Main {{ value: i32; direct: [u8; {capacity}] in SafeBytes; child: Child; }}
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(&checked, "Main::run")
                .produce_program_entry([7; 32])
                .unwrap();
            assert!(
                produced.receipt().receiver_eligibility().is_some(),
                "{predicate}"
            );
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let selection =
                checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run")
                    .unwrap();
            for corruption in 0..4 {
                let mut changed = module.clone();
                let declaration = changed
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.identity == "named(name(Child))")
                    .unwrap();
                let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
                    panic!("child record");
                };
                match corruption {
                    0 => fields[0].identity = "bytes".into(),
                    1 => {
                        fields[0].field_type = StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                capacity: capacity + 1,
                            },
                        )
                    }
                    2 => {
                        fields[0].field_type = StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView,
                        )
                    }
                    _ => declaration.identity = "named(name(OtherChild))".into(),
                }
                assert!(
                    derive(&checked, selection, &changed).is_none(),
                    "{predicate}: {corruption}"
                );
            }
        }
    }

    #[test]
    fn byte_receiver_cannot_gain_domain_membership_from_a_relabelled_raw_array() {
        let checked = check_source(
            "data Main { value: i32; bytes: [u8; 3]; }
             machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([7; 32])
            .unwrap();
        assert!(produced.receipt().receiver_eligibility().is_some());
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let declaration = module
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.identity == "named(name(Main))")
            .unwrap();
        let StructuralTypeShape::Record { fields } = &mut declaration.shape else {
            panic!("receiver record");
        };
        fields
            .iter_mut()
            .find(|field| field.identity == "bytes")
            .unwrap()
            .field_type =
            StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 3,
            });
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run").unwrap();
        assert!(derive(&checked, selection, &module).is_none());
    }

    #[test]
    fn byte_receiver_zero_eligibility_requires_every_resolved_domain_predicate() {
        for (domains, constraint, eligible) in [
            (
                "domain [u8; 3]::First requires valid_utf8(self); domain [u8; 3]::Second requires no_nul(self);",
                "First & Second",
                true,
            ),
            (
                "domain [u8; 3]::Utf8 requires non_empty(self);",
                "Utf8",
                false,
            ),
            (
                "domain [u8; 3]::First requires valid_utf8(self); domain [u8; 3]::Second requires non_empty(self);",
                "First & Second",
                false,
            ),
            (
                "domain [u8; 3]::Extra requires valid_utf8(self); non_empty(self);",
                "Extra",
                false,
            ),
        ] {
            let checked = check_source(&format!(
                "{domains}
                 data Child {{ bytes: [u8; 3] in {constraint}; }}
                 data Main {{ value: i32; child: Child; }}
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(&checked, "Main::run")
                .produce_program_entry([7; 32])
                .unwrap();
            assert_eq!(
                produced.receipt().receiver_eligibility().is_some(),
                eligible,
                "{domains}"
            );
        }
    }

    #[test]
    fn nested_receiver_storage_requires_complete_zero_valid_records() {
        let checked = check_source(
            "data Counter { value: i32; bytes: [u8; 4]; } data Pair { first: Counter; second: Counter; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
        );
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([7; 32])
            .unwrap();
        assert!(produced.receipt().receiver_eligibility().is_some());
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run").unwrap();
        let counter = module
            .structural_types
            .iter()
            .position(|declaration| declaration.identity == "named(name(Counter))")
            .unwrap();
        for corruption in 0..4 {
            let mut changed = module.clone();
            let counter_id = changed.structural_types[counter].id;
            if corruption == 0 {
                changed.structural_types.remove(counter);
            } else {
                let StructuralTypeShape::Record { fields } =
                    &mut changed.structural_types[counter].shape
                else {
                    panic!("record");
                };
                fields[0].field_type = match corruption {
                    1 => StructuralFieldType::Structural(counter_id),
                    2 => StructuralFieldType::Erased {
                        type_identity: "nested-service".into(),
                    },
                    _ => StructuralFieldType::BoundedInteger(
                        semantic_vocabulary::BoundedIntegerType::new(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Signed,
                                32,
                            )
                            .unwrap(),
                            semantic_vocabulary::IntegerValue::Signed(1),
                            semantic_vocabulary::IntegerValue::Signed(9),
                        )
                        .unwrap(),
                    ),
                };
            }
            assert!(
                derive(&checked, selection, &changed).is_none(),
                "corruption {corruption}"
            );
        }
    }

    #[test]
    fn receiver_array_eligibility_checks_the_complete_element_chain() {
        for array in ["[u8; 256]", "[[u16; 3]; 2]", "[u8; 0]"] {
            let checked = check_source(&format!(
                "data Main {{ value: i32; values: {array}; }} \
                 machine Main::run(&mut self) {{ self.value = 7; }}"
            ));
            let produced = TerminalProductionRequest::new(&checked, "Main::run")
                .produce_program_entry([7; 32])
                .unwrap();
            assert!(
                produced.receipt().receiver_eligibility().is_some(),
                "{array}"
            );
            let mut module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let array = module
                .structural_types
                .iter_mut()
                .find(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                })
                .unwrap();
            let StructuralTypeShape::FixedArray { element, .. } = &mut array.shape else {
                unreachable!();
            };
            *element = array.id;
            let selection =
                checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run")
                    .unwrap();
            assert!(
                derive(&checked, selection, &module).is_none(),
                "cyclic elements reject even beneath an empty dimension"
            );
        }
    }

    #[test]
    fn source_receiver_eligibility_rejoins_exact_terminal_self() {
        let checked = check_source(SOURCE);
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([7; 32])
            .unwrap();
        let receipt = produced.receipt();
        let eligible = receipt
            .receiver_eligibility()
            .expect("plain source record needs no executable cleanup");
        assert!(matches!(
            eligible.projection(),
            CheckedProgramEntryReceiverProjection::Retained {
                source_position: 0,
                ..
            }
        ));
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run").unwrap();
        assert_eq!(
            derive(&checked, selection, &module).as_ref(),
            Some(eligible)
        );
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        entry.structural_parameters[0].position += 1;
        assert!(derive(&checked, selection, &module).is_none());
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        entry.structural_parameters[0].is_self = false;
        assert!(
            derive(&checked, selection, &module).is_none(),
            "a retained receiver cannot become erased by losing its self marker"
        );
        let mut module =
            terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        module
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == eligible.terminal_receiver_type())
            .unwrap()
            .identity = "different-owner".into();
        assert!(derive(&checked, selection, &module).is_none());
    }

    #[test]
    fn erased_receiver_eligibility_preserves_exact_attachment_without_a_place() {
        for source in [
            "data Main {} machine Main::run(&mut self) {}",
            "data Main { value: i32; bytes: [u8; 4]; } machine Main::run(&mut self) {}",
        ] {
            let checked = check_source(source);
            let produced = TerminalProductionRequest::new(&checked, "Main::run")
                .produce_program_entry([9; 32])
                .unwrap();
            let eligible = produced.receipt().receiver_eligibility().unwrap();
            assert_eq!(
                eligible.projection(),
                CheckedProgramEntryReceiverProjection::Erased { source_position: 0 }
            );
            assert_eq!(eligible.owned_receiver_type_identity(), "named(name(Main))");
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let entry = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            assert_eq!(entry.attachment, Some(eligible.terminal_receiver_type()));
            assert!(
                entry
                    .structural_parameters
                    .iter()
                    .all(|parameter| !parameter.is_self)
            );
            let selection =
                checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run")
                    .unwrap();
            for corruption in 0..3 {
                let mut changed = module.clone();
                if corruption == 0 {
                    changed
                        .machines
                        .iter_mut()
                        .find(|machine| machine.id == changed.entry)
                        .unwrap()
                        .attachment = None;
                } else {
                    let declaration = changed
                        .structural_types
                        .iter_mut()
                        .find(|declaration| declaration.id == eligible.terminal_receiver_type())
                        .unwrap();
                    if corruption == 1 {
                        declaration.identity = "different-owner".into();
                    } else {
                        let duplicate = declaration.clone();
                        changed.structural_types.push(duplicate);
                    }
                }
                assert!(
                    derive(&checked, selection, &changed).is_none(),
                    "erased receiver attachment corruption {corruption}"
                );
            }
        }
    }

    #[test]
    fn ordinary_erased_fields_require_no_fused_service_establishment() {
        for body in ["", "self.value = 7;"] {
            let checked = check_source(&format!(
                "data Evidence {{}} data Main {{ value: i32; proof [erased]: Evidence; }} \
                 machine Main::run(&mut self) {{ {body} }}"
            ));
            let produced = TerminalProductionRequest::new(&checked, "Main::run")
                .produce_program_entry([9; 32])
                .unwrap();
            let eligible = produced.receipt().receiver_eligibility().unwrap();
            assert!(eligible.fused_service_fields().is_empty());
            let module =
                terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == eligible.terminal_receiver_type())
                .unwrap();
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                panic!("receiver remains a record");
            };
            assert!(
                fields.iter().any(|field| {
                    matches!(field.field_type, StructuralFieldType::Erased { .. })
                })
            );
        }
    }

    #[test]
    fn erased_receivers_cannot_hide_nominal_cleanup_or_nonzero_gates() {
        let checked = check_source("data Main { value: i32; } machine Main::run(&mut self) {}");
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([9; 32])
            .unwrap();
        let module = terminal_codec::decode_module(produced.artifact().semantic_bytes()).unwrap();
        // The erased Terminal attachment cannot establish source obligations;
        // derive must inspect the actual nominal owner even without a self place.
        for source in [
            "data Helper {} machine Helper::finish() {} data Main { value: i32; } machine Main::drop(&mut self) { Helper::finish(); } machine Main::run(&mut self) {}",
            "data Child {} machine Child::drop(&mut self) {} data Main { value: i32; child: Child; } machine Main::run(&mut self) {}",
            "data Main { value: i32 [1..=9]; } machine Main::run(&mut self) {}",
            "data Child { value: i32 [1..=9]; } data Main { value: i32; child: Child; } machine Main::run(&mut self) {}",
        ] {
            let checked = check_source(source);
            let selection =
                checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run")
                    .unwrap();
            assert!(derive(&checked, selection, &module).is_none(), "{source}");
        }
    }

    #[test]
    fn source_nominal_cleanup_and_zero_gates_cannot_acquire_receiver_eligibility() {
        let base = check_source(SOURCE);
        let artifact = TerminalProductionRequest::new(&base, "Main::run")
            .produce_program_entry([7; 32])
            .unwrap();
        let module = terminal_codec::decode_module(artifact.artifact().semantic_bytes()).unwrap();
        // A same-named Terminal record cannot prove source cleanup absence.
        // Both owning forms must be checked on the actual source type graph.
        for source in [
            "data Main { value: i32; } machine Main::drop(&mut self) {} machine Main::run(&mut self) { self.value = 7; }",
            "data Child {} machine Child::drop(&mut self) {} data Main { value: i32; child: Child; } machine Main::run(&mut self) { self.value = 7; }",
            "data Main { value: i32 [1..=9]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Main { value: i32; values: [i32 [1..=9]; 2]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32; } machine Child::drop(&mut self) {} data Main { value: i32; values: [Child; 2]; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32; } machine Child::drop(&mut self) {} data Pair { child: Child; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
            "data Child { value: i32 [1..=9]; } data Pair { child: Child; } data Main { value: i32; pair: Pair; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let selection =
                checked_trees_to_lowered_psi::select_terminal_machine(&checked, "Main::run")
                    .unwrap();
            assert!(derive(&checked, selection, &module).is_none());
        }
        let mut missing = check_source(SOURCE);
        missing.typed.roots.data_definitions = Default::default();
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&missing, "Main::run").unwrap();
        assert!(derive(&missing, selection, &module).is_none());
    }

    #[test]
    fn receiver_eligibility_follows_attachment_not_unrelated_cleanup() {
        for source in [
            "data Main { value: i32; } machine Main::run(&mut self) { self.value = 8; }",
            "data Other {} machine Other::drop(&mut self) {} data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }",
        ] {
            let checked = check_source(source);
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::run")
                .unwrap();
            let produced = TerminalProductionRequest::for_machine_symbol(&checked, machine.symbol)
                .produce_program_entry([9; 32])
                .unwrap();
            let eligible = produced.receipt().receiver_eligibility().unwrap();
            assert_eq!(eligible.owned_receiver_type_identity(), "named(name(Main))");
            assert_ne!(
                eligible.source_receiver_type_identity(),
                eligible.owned_receiver_type_identity()
            );
        }
        let checked = check_source("data Main {} machine Main::run() {}");
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([9; 32])
            .unwrap();
        assert!(
            produced.receipt().receiver_eligibility().is_none(),
            "a free entry does not acquire an implicit receiver"
        );
    }
}

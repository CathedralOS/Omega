//! Source eligibility cannot be recovered from native layout: identical record
//! shapes may have different zero gates and nominal cleanup. Derive this private
//! correspondence before source erasure and bind it to the exact Terminal self.
//! It is not a standalone proof from source-free Terminal bytes or root authority.

use checked_trees::types::TypeReferenceNode;
use checked_trees::{CheckedTerminalMachineSelection, CheckedTrees};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralFieldType, StructuralTypeShape, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgramEntryReceiverEligibility {
    source_receiver_type_identity: String,
    owned_receiver_type_identity: String,
    terminal_self: PlaceId,
    terminal_receiver_type: StructuralTypeId,
}

impl CheckedProgramEntryReceiverEligibility {
    pub fn source_receiver_type_identity(&self) -> &str {
        &self.source_receiver_type_identity
    }

    pub fn owned_receiver_type_identity(&self) -> &str {
        &self.owned_receiver_type_identity
    }

    pub const fn terminal_self(&self) -> PlaceId {
        self.terminal_self
    }

    pub const fn terminal_receiver_type(&self) -> StructuralTypeId {
        self.terminal_receiver_type
    }
}

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
    let source_position = checked
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.is_self)?;
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
    let mut parameters = entry
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.is_self);
    let parameter = parameters.next()?;
    if parameters.next().is_some()
        || parameter.access != StructuralAccess::MutableBorrow
        || usize::try_from(parameter.position).ok()? != source_position
    {
        return None;
    }
    let structural_type = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)?;
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
    // Scalar fields and fixed primitive arrays need no initialization program.
    // Array eligibility follows its complete semantic element chain, including
    // empty dimensions; a zero byte count never excuses an invalid element.
    // Erased qualification establishment remains an independent installed
    // occurrence obligation; this correspondence supplies no Bound authority.
    if entry.attachment != Some(parameter.structural_type)
        || structural_type.identity != owned_receiver_type_identity
        || fields.iter().any(|field| match field.field_type {
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::Erased { .. } => false,
            StructuralFieldType::Structural(structural_type) => {
                terminal_semantics::scalar_array_leaf_shape(
                    module.structural_types.iter(),
                    structural_type,
                )
                .is_none()
            }
            _ => true,
        })
    {
        return None;
    }
    Some(CheckedProgramEntryReceiverEligibility {
        source_receiver_type_identity: checked
            .normalized_type_identity(receiver.type_reference)
            .into_string(),
        owned_receiver_type_identity,
        terminal_self: parameter.place,
        terminal_receiver_type: parameter.structural_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerminalProductionRequest;

    fn check_source(source: &str) -> CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
    }

    const SOURCE: &str =
        "data Main { value: i32; } machine Main::run(&mut self) { self.value = 7; }";

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
        module
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == eligible.terminal_receiver_type())
            .unwrap()
            .identity = "different-owner".into();
        assert!(derive(&checked, selection, &module).is_none());
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
        let checked = check_source("data Main {} machine Main::run(&mut self) {}");
        let produced = TerminalProductionRequest::new(&checked, "Main::run")
            .produce_program_entry([9; 32])
            .unwrap();
        assert!(
            produced.receipt().receiver_eligibility().is_none(),
            "an erased self requires an explicit source-to-Terminal projection, not a fabricated place"
        );
    }
}

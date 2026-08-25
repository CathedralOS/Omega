use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
use psi_language_semantics::{MachineSupplyMode, TerminationGuarantee, TerminationInterface};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContractKind;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_cleanup_machine_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines().iter().filter(|machine| {
        machine.attached_data.is_some() && machine.name.as_str().ends_with("::drop")
    }) {
        validate_cleanup_machine(program, machine, diagnostics);
    }
}

/// Reject authored access to the one compiler-selected cleanup hook attached
/// to a nominal owner. Automatic edge cleanup is deliberately absent from the
/// authored-selection ledger, so matching an exact retained declaration here
/// cannot reject compiler-planned cleanup. Spelling is never sufficient: an
/// unattached ordinary machine named `drop` remains callable.
pub(crate) fn collect_reserved_cleanup_selection_diagnostics(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for selection in program.authored_declaration_selections() {
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            continue;
        };
        let Some(cleanup) = reserved_cleanup_selected_by(program, target.selected_symbol()) else {
            continue;
        };
        diagnostics.push(
            Diagnostic::error(format!(
                "reserved cleanup machine `{}` is compiler-selected and cannot be selected by source; consume the value with `omega::core::drop(value)` or call an ordinary owner-published protocol operation",
                cleanup.name,
            ))
            .with_source_span(selection.source_span()),
        );
    }
}

fn reserved_cleanup_selected_by(
    program: &TypedTrees,
    selected_symbol: SymbolHandle,
) -> Option<&Machine> {
    program.machines().iter().find(|machine| {
        let owner = machine.attached_data_symbol;
        machine.attached_data.is_some()
            && owner.is_valid()
            && machine.name.as_str().rsplit("::").next() == Some("drop")
            && program
                .data_definitions()
                .iter()
                .any(|data| data.symbol == owner)
            && (machine.symbol == selected_symbol
                || program
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == selected_symbol))
    })
}

fn validate_cleanup_machine(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(_attached_name) = machine.attached_data.as_ref() else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` has no entry state; reserved cleanup has shape `drop(&mut self) -> ()`",
            machine.name
        )));
        return;
    };
    let parameters = program.state_parameters(entry);
    let receiver_is_exact = matches!(parameters, [receiver]
    if receiver.is_self
        && receiver.is_mutable
        && !receiver.is_const
            && mutable_reference_targets(
                program,
                receiver.type_reference,
                machine.symbol,
            ));
    if !receiver_is_exact {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must have exactly the receiver `&mut self` and no positional parameters",
            machine.name
        )));
    }
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare method-local lifetime or type parameters",
            machine.name
        )));
    }
    if entry.return_type.is_valid()
        && !matches!(
            program
                .type_reference_table
                .type_reference(entry.return_type),
            TypeReferenceNode::Unit
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must return Unit; fallible cleanup is an explicit consuming machine",
            machine.name
        )));
    }
    if machine.suspends || machine.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` must be non-suspending and nonblocking",
            machine.name
        )));
    }
    if program
        .machine_contracts(machine)
        .iter()
        .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    {
        diagnostics.push(Diagnostic::error(format!(
            "cleanup machine `{}` may not declare a crash outcome",
            machine.name
        )));
    }
    if machine.supply_mode != MachineSupplyMode::CheckedBody
        && !matches!(
            machine.termination_plan.interface,
            TerminationInterface::Published(TerminationGuarantee::Terminates { .. })
        )
    {
        diagnostics.push(Diagnostic::error(format!(
            "bodyless cleanup machine `{}` must publish `terminates;`",
            machine.name
        )));
    }
}

fn mutable_reference_targets(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expected_self_symbol: psi_symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            mutable_reference_targets(program, *base_type, expected_self_symbol)
        }
        TypeReferenceNode::Reference {
            referee, access, ..
        } if access.is_exclusive() => named_type_matches(program, *referee, expected_self_symbol),
        _ => false,
    }
}

fn named_type_matches(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    expected_self_symbol: psi_symbols::SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            named_type_matches(program, *base_type, expected_self_symbol)
        }
        TypeReferenceNode::Named { symbol, name } => {
            *symbol == expected_self_symbol && name.as_str() == "Self"
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::collect_reserved_cleanup_selection_diagnostics;
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    };
    use psi_source::{SourceId, SourceSpan, Span};
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::DataDefinition;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::state::State;

    fn source_span(index: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(index, index + 1))
    }

    fn machine_with_entry(
        program: &mut TypedTrees,
        machine_symbol: u32,
        entry_symbol: u32,
        name: &'static str,
        owner: Option<(u32, &'static str)>,
    ) -> (SymbolHandle, SymbolHandle) {
        let machine_symbol = SymbolHandle::from_arena_index(machine_symbol);
        let entry_symbol = SymbolHandle::from_arena_index(entry_symbol);
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated_static(name),
            attached_data: owner.map(|(_, owner_name)| Identifier::generated_static(owner_name)),
            attached_data_symbol: owner.map_or_else(SymbolHandle::invalid, |(symbol, _)| {
                SymbolHandle::from_arena_index(symbol)
            }),
            ..Machine::default()
        };
        program.push_machine_state(
            &mut machine,
            State {
                symbol: entry_symbol,
                name: Identifier::generated_static("entry"),
                ..State::default()
            },
        );
        program.push_machine(machine);
        (machine_symbol, entry_symbol)
    }

    fn record_selection(
        program: &mut TypedTrees,
        index: usize,
        kind: AuthoredDeclarationSelectionKind,
        symbol: SymbolHandle,
    ) {
        program
            .record_resolved_authored_declaration_selection_once(
                source_span(index),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                kind,
                symbol,
            )
            .expect("fixture selection should enter the authored ledger");
    }

    #[test]
    fn source_selection_of_exact_owner_attached_drop_rejects_in_every_retained_form() {
        let mut program = TypedTrees::default();
        let owner_symbol = SymbolHandle::from_arena_index(10);
        program.push_data_definition(DataDefinition {
            symbol: owner_symbol,
            name: Identifier::generated_static("Resource"),
            ..DataDefinition::default()
        });
        let (cleanup_machine, cleanup_entry) = machine_with_entry(
            &mut program,
            20,
            21,
            "Resource::drop",
            Some((10, "Resource")),
        );

        // Qualified and receiver calls share the checked Call selection kind;
        // depending on resolution timing they retain the machine or its entry.
        record_selection(
            &mut program,
            1,
            AuthoredDeclarationSelectionKind::Call,
            cleanup_machine,
        );
        record_selection(
            &mut program,
            2,
            AuthoredDeclarationSelectionKind::Call,
            cleanup_entry,
        );
        record_selection(
            &mut program,
            3,
            AuthoredDeclarationSelectionKind::StaticArgument,
            cleanup_entry,
        );
        record_selection(
            &mut program,
            4,
            AuthoredDeclarationSelectionKind::StaticPathSegment,
            cleanup_machine,
        );

        let mut diagnostics = Vec::new();
        collect_reserved_cleanup_selection_diagnostics(&program, &mut diagnostics);

        assert_eq!(diagnostics.len(), 4);
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic
                .message
                .contains("reserved cleanup machine `Resource::drop` is compiler-selected")
        }));
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.source_span)
                .collect::<Vec<_>>(),
            (1..=4).map(source_span).map(Some).collect::<Vec<_>>()
        );
    }

    #[test]
    fn ordinary_drop_machines_and_core_drop_remain_authored_callables() {
        let mut program = TypedTrees::default();
        let (ordinary_drop, ordinary_entry) =
            machine_with_entry(&mut program, 30, 31, "drop", None);
        let (core_drop, core_entry) =
            machine_with_entry(&mut program, 40, 41, "omega::core::drop", None);
        let owner_symbol = SymbolHandle::from_arena_index(50);
        program.push_data_definition(DataDefinition {
            symbol: owner_symbol,
            name: Identifier::generated_static("Resource"),
            ..DataDefinition::default()
        });
        let (drop_counter, drop_counter_entry) = machine_with_entry(
            &mut program,
            60,
            61,
            "Resource::drop_counter",
            Some((50, "Resource")),
        );

        for (index, symbol) in [
            ordinary_drop,
            ordinary_entry,
            core_drop,
            core_entry,
            drop_counter,
            drop_counter_entry,
        ]
        .into_iter()
        .enumerate()
        {
            record_selection(
                &mut program,
                index + 1,
                AuthoredDeclarationSelectionKind::Call,
                symbol,
            );
        }

        let mut diagnostics = Vec::new();
        collect_reserved_cleanup_selection_diagnostics(&program, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }
}

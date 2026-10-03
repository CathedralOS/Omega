use super::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, validate_receiver_eligibility,
};

#[test]
fn static_context_receipt_settles_without_attachment_and_rejects_projection_drift() {
    for name in ["run", "Namespace::run"] {
        let source = format!(
            "data Context {{ count: u32; }} data Namespace {{}}
            machine {name}(context: &mut Context) {{ context.count = 7; }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap();
        let selected = checked_trees_to_lowered_psi::TerminalMachineSelection::Name(name);
        let selection =
            checked_trees_to_lowered_psi::select_terminal_machine(&checked, selected).unwrap();
        let source = provider_planning::program_entry::validate_selected_program_entry_shape(
            &checked,
            provider_planning::program_entry::SelectedProgramEntry {
                machine_name: name,
                machine_symbol: selection.machine,
                slot: target::TargetProfile::WindowsX64.program_entry_slot(),
            },
        )
        .unwrap();
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, selected).unwrap();
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == lowered.semantic_module.entry)
                .unwrap()
                .attachment
                .is_none()
        );
        let receipt = lowered_psi_to_terminal_psi::prepare_program_entry_receipt(
            &checked,
            selection,
            source.identity().bytes(),
            &lowered,
        )
        .unwrap();
        let semantic_identity =
            terminal_codec::terminal_psi_identity(&lowered.semantic_module).unwrap();
        let settlement =
            NativeProgramEntrySettlement::new(&source, None, &[]).with_checked_entry(&receipt);
        validate_receiver_eligibility(&lowered.semantic_module, &receipt, settlement).unwrap();

        let eligibility = receipt
            .receiver_eligibility()
            .expect("explicit context has checked eligibility");
        let terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
            terminal_self,
            source_position,
        } = eligibility.projection()
        else {
            panic!("mutated context must be retained");
        };
        for projection in [
            terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
                terminal_self,
                source_position: source_position + 1,
            },
            terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
                terminal_self: semantic_vocabulary::PlaceId::new(999).unwrap(),
                source_position,
            },
        ] {
            let changed = terminal_psi::CheckedProgramEntryTerminalReceipt::new(
                source.identity().bytes(),
                name.into(),
                selection.machine,
                semantic_identity,
                lowered.semantic_module.entry,
                Some(terminal_psi::CheckedProgramEntryReceiverEligibility::new(
                    eligibility.source_receiver_type_identity().into(),
                    eligibility.owned_receiver_type_identity().into(),
                    projection,
                    eligibility.terminal_receiver_type(),
                    Vec::new(),
                    eligibility.cleanup(),
                )),
            );
            assert!(matches!(
                validate_receiver_eligibility(
                    &lowered.semantic_module,
                    &changed,
                    NativeProgramEntrySettlement::new(&source, None, &[])
                        .with_checked_entry(&changed),
                ),
                Err(NativeProgramEntrySettlementError::ReceiverEligibilityDrift)
            ));
        }
    }
}

//! Byte replacement preserves exact capacity evidence and the original storage effect.

use super::{
    AbstractOperation, OptimizationUnitValidationError, TerminalFuelSchedule, refresh_identity,
    validate_transformed_psi_optimization_unit, validate_verified_psi_optimization_unit,
};
#[test]
fn indexed_byte_field_rejoins_original_field_even_when_current_bounds_match() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; sibling: [u8;3] in Utf8; }
        machine Record::replace(&mut self, position: u64 [0..=2], byte: u8 [0..=127]) {
            self.out = "XXX";
            self.sibling = "YYY";
            self.out[position] = byte;
        }
    "#;
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
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let terminal =
        checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace").unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&terminal.semantic_module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(
                &terminal.semantic_module,
                &terminal.proof_bundle,
            )
            .unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, baseline) = verified.into_parts();
    let mut changed = baseline.clone();
    let function = changed
        .functions
        .iter_mut()
        .find(|function| function.machine == baseline.entry)
        .unwrap();
    let destination_type = function.structural_parameters[0].structural_type;
    let terminal_psi::StructuralTypeShape::Record { fields } = &baseline
        .structural_types
        .iter()
        .find(|declaration| declaration.id == destination_type)
        .unwrap()
        .shape
    else {
        unreachable!();
    };
    let sibling = fields[1].id;
    for node in function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
    {
        match &mut node.operation {
            AbstractOperation::StructuralByteSequenceFieldByteStore { field, .. }
            | AbstractOperation::StructuralByteSequenceFieldLength { field, .. } => {
                *field = sibling
            }
            _ => {}
        }
    }
    refresh_identity(&mut changed);
    assert_ne!(baseline.identity, changed.identity);
    optimization_unit_semantics::validate_psi_optimization_unit(&changed).unwrap();
    assert!(validate_transformed_psi_optimization_unit(&input, &changed).is_err());
}

#[test]
fn byte_field_replacement_rejoins_capacity_and_verified_destination() {
    for literal in ["", "X", "XXX"] {
        let source = format!(
            r#"
            domain [u8; 3]::Utf8 requires valid_utf8(self);
            data Record {{ out: [u8; 3] in Utf8; sibling: [u8; 3] in Utf8; }}
            machine Record::replace(&mut self) {{ self.out = "{literal}"; }}
        "#
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .unwrap();
        let terminal =
            checked_trees_to_lowered_psi::lower_machine(&checked, "Record::replace").unwrap();
        let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &terminal_codec::encode_module(&terminal.semantic_module).unwrap(),
                proof_bytes: &terminal_codec::encode_proof_section(
                    &terminal.semantic_module,
                    &terminal.proof_bundle,
                )
                .unwrap(),
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .and_then(|admitted| admitted.try_into_optimization_input())
        .unwrap();
        let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap();
        validate_verified_psi_optimization_unit(&verified).unwrap();
        let (input, baseline) = verified.into_parts();
        let (function_position, block_position, node_position) = baseline
            .functions
            .iter()
            .enumerate()
            .find_map(|(function_position, function)| {
                function
                    .blocks
                    .iter()
                    .enumerate()
                    .find_map(|(block_position, block)| {
                        block
                            .nodes
                            .iter()
                            .position(|node| {
                                matches!(
                                    node.operation,
                                    AbstractOperation::StructuralByteSequenceFieldStore { .. }
                                )
                            })
                            .map(|node_position| (function_position, block_position, node_position))
                    })
            })
            .expect("retained replacement operation");
        let destination_type =
            baseline.functions[function_position].structural_parameters[0].structural_type;
        let declaration_position = baseline
            .structural_types
            .iter()
            .position(|declaration| declaration.id == destination_type)
            .unwrap();
        let terminal_psi::StructuralTypeShape::Record { fields } =
            &baseline.structural_types[declaration_position].shape
        else {
            panic!("record destination");
        };
        let sibling = fields[1].id;
        let mut redirected = baseline.clone();
        let AbstractOperation::StructuralByteSequenceFieldStore { field, .. } =
            &mut redirected.functions[function_position].blocks[block_position].nodes
                [node_position]
                .operation
        else {
            unreachable!()
        };
        *field = sibling;
        refresh_identity(&mut redirected);
        assert_ne!(redirected.identity, baseline.identity);
        // Both fields have identical capacity, type and permission. Current-IR
        // validation passes; only the retained original effect rejects redirection.
        optimization_unit_semantics::validate_psi_optimization_unit(&redirected).unwrap();
        assert!(matches!(
            validate_transformed_psi_optimization_unit(&input, &redirected),
            Err(OptimizationUnitValidationError::OperationObligationOwnerMismatch { .. })
        ));

        let mut changed_capacity = baseline.clone();
        let terminal_psi::StructuralTypeShape::Record { fields } =
            &mut changed_capacity.structural_types.make_mut()[declaration_position].shape
        else {
            unreachable!()
        };
        fields[0].field_type = terminal_psi::StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 4 },
        );
        refresh_identity(&mut changed_capacity);
        assert!(matches!(
            optimization_unit_semantics::validate_psi_optimization_unit(&changed_capacity),
            Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
        ));

        let mut missing_proof = baseline.clone();
        missing_proof.accepted_obligation_facts.clear();
        refresh_identity(&mut missing_proof);
        assert!(matches!(
            optimization_unit_semantics::validate_psi_optimization_unit(&missing_proof),
            Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
        ));

        let mut wrong_source = baseline.clone();
        let AbstractOperation::StructuralByteSequenceFieldStore {
            source,
            destination,
            ..
        } = &mut wrong_source.functions[function_position].blocks[block_position].nodes
            [node_position]
            .operation
        else {
            unreachable!()
        };
        *source = *destination;
        refresh_identity(&mut wrong_source);
        assert!(matches!(
            optimization_unit_semantics::validate_psi_optimization_unit(&wrong_source),
            Err(OptimizationUnitValidationError::InvalidByteSequenceRead { .. })
        ));
    }
}

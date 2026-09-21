//! Current-IR indexed field custody, scalar identity, and exact bounds.
use super::id;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationUnit;
use crate::tests::refresh_node_derivatives;
use crate::validate_psi_optimization_unit;
use abstract_operations::AbstractOperation;

pub(crate) fn indexed_field_unit() -> PsiOptimizationUnit {
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
    let semantic = terminal_codec::encode_module(&terminal.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
    .into_parts()
    .1
}

#[test]
fn indexed_byte_field_rejects_recomputed_subject_and_bound_corruption() {
    let baseline = indexed_field_unit();
    validate_psi_optimization_unit(&baseline).unwrap();
    let function = baseline
        .functions
        .iter()
        .position(|function| function.machine == baseline.entry)
        .unwrap();
    let (block, store) = baseline.functions[function]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, owner)| {
            owner
                .nodes
                .iter()
                .position(|node| {
                    matches!(
                        node.operation,
                        AbstractOperation::StructuralByteSequenceFieldByteStore { .. }
                    )
                })
                .map(|store| (block, store))
        })
        .unwrap();
    for corruption in [
        "index",
        "value",
        "length",
        "field",
        "path",
        "obligation",
        "access",
        "missing fact",
        "fuel",
    ] {
        let mut changed = baseline.clone();
        let AbstractOperation::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            field,
            path,
            obligation,
            ..
        } = &mut changed.functions[function].blocks[block].nodes[store].operation
        else {
            unreachable!();
        };
        match corruption {
            "index" => *index = *length,
            "value" => *value = *index,
            "length" => *length = *index,
            "field" => *field = id(9001, semantic_vocabulary::StructuralFieldId::new),
            "path" => path.push(terminal_psi::StructuralPathSegment::Field("missing".into())),
            "obligation" => *obligation = id(9002, semantic_vocabulary::ObligationId::new),
            "access" => {
                changed.functions[function].structural_parameters[0].access =
                    terminal_psi::StructuralAccess::SharedBorrow
            }
            "missing fact" => changed.accepted_obligation_facts.clear(),
            "fuel" => changed.functions[function].blocks[block].nodes[store]
                .fuel
                .clear(),
            _ => unreachable!(),
        }
        refresh_node_derivatives(&mut changed, function, block, store);
        assert_ne!(changed.identity, baseline.identity, "{corruption}");
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "{corruption}"
        );
    }
    let mut changed = baseline.clone();
    let AbstractOperation::StructuralByteSequenceFieldByteStore { index, length, .. } =
        &mut changed.functions[function].blocks[block].nodes[store].operation
    else {
        unreachable!();
    };
    *index = *length;
    refresh_node_derivatives(&mut changed, function, block, store);
    assert!(matches!(
        validate_psi_optimization_unit(&changed),
        Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
    ));
}

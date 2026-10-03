//! Current-IR indexed field custody, scalar identity, and exact bounds.
use crate::tests::support::{id, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use optimization_unit::PsiOptimizationUnit;

pub(crate) fn indexed_field_unit() -> PsiOptimizationUnit {
    let source = r#"
        domain [u8;3]::Utf8(bytes) requires valid_utf8(bytes);
        data Record { out: [u8;3] in Utf8; sibling: [u8;3] in Utf8; }
        machine Record::replace(record: &mut Record, position: u64 [0..=2], byte: u8 [0..=127]) {
            record.out = "XXX";
            record.sibling = "YYY";
            record.out[position] = byte;
        }
    "#;
    indexed_field_source_unit(source)
}

pub(crate) fn local_byte_field_snapshot_unit() -> PsiOptimizationUnit {
    indexed_field_source_unit(
        r#"
        domain [u8;3]::Utf8(bytes) requires valid_utf8(bytes);
        data Record [copy] { out: [u8;3] in Utf8; }
        machine fill(record: &mut Record) { record.out = "é"; }
        machine Record::replace(record: &mut Record) {
            let local: Record;
            fill(&mut local);
            record.out = local.out;
        }
        "#,
    )
}

#[test]
fn local_byte_field_snapshot_preserves_current_owned_storage() {
    let unit = local_byte_field_snapshot_unit();
    validate_psi_optimization_unit(&unit).unwrap();
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    assert!(function.blocks.iter().flat_map(|block| &block.nodes).any(|node| {
        matches!(&node.operation, AbstractOperation::EstablishByteSequenceConcatenation { left, .. }
            if matches!(left, terminal_psi::control_flow::ByteSequenceOperand::Field { .. }))
    }));
}

pub(crate) fn indexed_field_with_primitive_sibling_unit() -> PsiOptimizationUnit {
    indexed_field_source_unit(
        r#"
        domain [u8;3]::Utf8(bytes) requires valid_utf8(bytes);
        data Record { out: [u8;3] in Utf8; marked: [i32;3]; }
        machine Record::replace(record: &mut Record, position: u64 [0..=2], byte: u8 [0..=127]) {
            record.out = "XXX";
            record.marked[position] = 1;
            record.out[position] = byte;
        }
        "#,
    )
}

pub(crate) fn indexed_field_source_unit(source: &str) -> PsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
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
    let terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Record::replace"),
    )
    .unwrap();
    let semantic = terminal_codec::encode_module(&terminal.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&terminal.semantic_module, &terminal.proof_bundle)
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
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
fn indexed_byte_field_preserves_ascii_across_primitive_sibling_write() {
    let unit = indexed_field_with_primitive_sibling_unit();
    assert!(
        unit.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .any(|node| matches!(
                node.operation,
                AbstractOperation::WriteOnlyPrimitiveStore { .. }
            ))
    );
    validate_psi_optimization_unit(&unit).unwrap();
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

/// A store into a UTF-8 field was admitted with its stored byte's ASCII leg
/// beside the bound. An accepted fact that proves only the bound -- the goal a
/// plain carrier owes -- is not authority for the qualified store, even with a
/// canonical identity.
#[test]
fn indexed_utf8_field_store_requires_its_accepted_ascii_goal() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
    };
    let baseline = indexed_field_unit();
    validate_psi_optimization_unit(&baseline).unwrap();
    let (operation, obligation, index, length) = baseline
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .find_map(|node| match node.operation {
            AbstractOperation::StructuralByteSequenceFieldByteStore {
                psi_operation,
                obligation,
                index,
                length,
                ..
            } => Some((psi_operation, obligation, index, length)),
            _ => None,
        })
        .unwrap();
    let count = |value: ValueId| {
        ScalarTerm::value(
            value,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        )
    };
    let bound_only = terminal_codec::canonical_proposition_order_key(&Proposition::LessThan(
        count(index),
        count(length),
    ))
    .unwrap();
    let rebuilt = |proposition: Option<Vec<u8>>| {
        let mut changed = baseline.clone();
        let fact = changed
            .accepted_obligation_facts
            .iter_mut()
            .find(|fact| fact.operation == operation && fact.obligation == obligation)
            .expect("the store's accepted goal");
        *fact = optimization_unit::AcceptedObligationFact::new(
            fact.psi,
            fact.proof_bundle_fingerprint,
            fact.machine,
            fact.operation,
            fact.obligation,
            proposition.unwrap_or_else(|| fact.proposition.clone()),
        );
        crate::tests::support::refresh_identity(&mut changed);
        changed
    };
    // Rebuilding the admitted row unchanged is a no-op control.
    validate_psi_optimization_unit(&rebuilt(None)).unwrap();
    assert!(matches!(
        validate_psi_optimization_unit(&rebuilt(Some(bound_only))),
        Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
    ));
}

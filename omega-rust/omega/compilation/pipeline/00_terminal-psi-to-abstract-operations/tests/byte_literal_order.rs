//! Literal declaration identity must not prescribe evaluation order.

use abstract_operations::AbstractOperation;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalModule,
};
use terminal_psi_to_abstract_operations::{ArtifactSections, lower_artifact};
use terminal_verifier::ProofBundle;

fn literal_module() -> (TerminalModule, ProofBundle) {
    let tokens = source_files_to_tokens::Lexer::new("machine entry() {}")
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
    let mut terminal = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("entry"),
    )
    .unwrap();
    let module = &mut terminal.semantic_module;
    let structural_type = StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "literal-order::bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        }),
    });
    let machine = &mut module.machines[0];
    machine.structural_places = (0..2)
        .map(|ordinal| StructuralPlaceDeclaration {
            id: PlaceId::new(u64::from(ordinal) + 1).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: ordinal,
                structural_type,
            },
        })
        .collect();
    machine.blocks[0].operations = [2, 1]
        .into_iter()
        .enumerate()
        .map(|(ordinal, destination)| Operation {
            id: OperationId::new(ordinal as u64 + 1).unwrap(),
            result: OperationResult::Unit,
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: PlaceId::new(destination).unwrap(),
                bytes: vec![destination as u8],
                qualifications: Vec::new(),
            },
        })
        .collect();
    (terminal.semantic_module, terminal.proof_bundle)
}

#[test]
fn literal_evaluation_order_preserves_declaration_identity() {
    let (module, proof) = literal_module();
    assert_literal_order(&module, &proof);
}

#[test]
fn literals_in_distinct_blocks_preserve_declaration_identity() {
    let (mut module, proof) = literal_module();
    let machine = &mut module.machines[0];
    let mut successor = machine.blocks[0].clone();
    successor.id = semantic_vocabulary::BlockId::new(2).unwrap();
    successor.operations = machine.blocks[0].operations.split_off(1);
    machine.blocks[0].terminator = terminal_psi::Terminator::Jump {
        edge: semantic_vocabulary::EdgeId::new(2).unwrap(),
        target: successor.id,
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    machine.blocks.push(successor);
    assert_literal_order(&module, &proof);
}

fn assert_literal_order(module: &TerminalModule, proof: &ProofBundle) {
    let profile = AdmissionProfile::default();
    terminal_verifier::verify_module(module, proof, &profile)
        .expect("literal evaluation order is independent of declaration order");
    let semantic_bytes = terminal_codec::encode_module(module).unwrap();
    let proof_bytes = terminal_codec::encode_proof_section(module, proof).unwrap();
    let admitted = lower_artifact(
        ArtifactSections {
            semantic_bytes: &semantic_bytes,
            proof_bytes: &proof_bytes,
            obligation_ledger_bytes: None,
        },
        &profile,
    )
    .expect("verified literals lower in their original evaluation order");
    let plan = admitted.into_plan();
    let literals = plan.functions[0]
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::EstablishByteSequenceLiteral { place, bytes, .. } => {
                Some((place.id, bytes.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        literals,
        vec![
            (PlaceId::new(2).unwrap(), vec![2]),
            (PlaceId::new(1).unwrap(), vec![1]),
        ],
    );
    let optimization = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&optimization)
        .expect("independent current-IR validation preserves the literal roster");
}

#[test]
fn malformed_literal_rosters_and_establishments_still_reject() {
    let (module, proof) = literal_module();
    let profile = AdmissionProfile::default();
    for mutation in 0..3 {
        let mut invalid = module.clone();
        let machine = &mut invalid.machines[0];
        match mutation {
            0 => machine.structural_places[1].kind = machine.structural_places[0].kind,
            1 => {
                machine.blocks[0].operations.pop();
            }
            2 => {
                machine.blocks[0].operations[1].kind = machine.blocks[0].operations[0].kind.clone();
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(&invalid, &proof, &profile).is_err(),
            "invalid literal mutation {mutation} must reject",
        );
    }
}

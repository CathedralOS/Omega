//! Mutable carrier access remains distinct at every erased reborrow edge.
use super::*;
use checked_trees::BorrowAccessKind;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

fn nested_source() -> String {
    source(
        "forward(destination: &mut [Container; 2], value: u16)",
        "let parent: &mut Container = &mut destination[1];
         let middle: &mut [Record; 2] = &mut parent.records;
         let child: &write Record = &write middle[0];",
        "child.replace(17); child.replace(value);",
    )
}

#[test]
fn mutable_chain_retains_each_access_and_exact_projected_calls() {
    let checked = checked_from_source(&nested_source());
    let artifact = terminal_production::produce_terminal_artifact(&checked, "forward").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    let [handoff] = module.reborrow_root_handoffs.as_slice() else {
        panic!("one exact state-exit handoff");
    };
    assert_eq!(handoff.direct_root_access, StructuralAccess::MutableBorrow);
    assert_eq!(handoff.lineage.len(), 2);
    assert_eq!(
        handoff.lineage[0].child_access,
        StructuralAccess::MutableBorrow
    );
    assert_eq!(
        handoff.lineage[1].child_access,
        StructuralAccess::WriteOnlyBorrow
    );
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(
        caller.structural_parameters[0].access,
        StructuralAccess::MutableBorrow
    );
    let mut paths = Vec::new();
    for operation in &caller.blocks[0].operations {
        if let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &operation.kind
        {
            assert_eq!(
                structural_arguments[0].access,
                StructuralAccess::WriteOnlyBorrow
            );
            paths.push(&structural_arguments[0].path);
        }
    }
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], paths[1]);
    assert_eq!(paths[0].len(), 3);
    assert_eq!(
        paths[0][0],
        terminal_psi::StructuralPathSegment::FixedIndex(1)
    );
    assert_eq!(
        paths[0][2],
        terminal_psi::StructuralPathSegment::FixedIndex(0)
    );

    let mut widened = module.clone();
    widened.reborrow_root_handoffs[0].direct_root_access = StructuralAccess::WriteOnlyBorrow;
    assert!(
        terminal_verifier::verify_module(
            &widened,
            &proof,
            &proof_admission::AdmissionProfile::default()
        )
        .is_err(),
        "a mutable child cannot descend from write-only custody"
    );
}

#[test]
fn mutable_alias_source_replay_rejects_access_path_and_lifetime_substitution() {
    let original = checked_from_source(&nested_source());
    let _artifact = terminal_production::produce_terminal_artifact(&original, "forward").unwrap();
    let (direct_handle, direct) = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "parent")
        .unwrap();
    let (middle_handle, middle) = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "middle")
        .unwrap();
    let (child_handle, child) = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "child")
        .unwrap();
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.symbol == direct.machine_symbol)
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let StatementNode::LocalData(parent) =
        &original.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("parent declaration");
    };
    let StatementNode::LocalData(leaf) =
        &original.statement_table.statements(state.statement_nodes)[2]
    else {
        panic!("child declaration");
    };
    for mutation in 0..11 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(direct_handle)
                    .access = BorrowAccessKind::WriteOnly
            }
            1 => checked.facts.borrow.loans.get_mut(direct.loan).kind = BorrowAccessKind::WriteOnly,
            2 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(middle_handle)
                    .access = BorrowAccessKind::WriteOnly
            }
            3 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child_handle)
                    .parent_access = BorrowAccessKind::WriteOnly
            }
            4 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child_handle)
                    .access = BorrowAccessKind::Mutable
            }
            5 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(child.loan)
                    .source_owner_symbol = direct.owner_symbol
            }
            6 => checked
                .facts
                .borrow
                .reborrow_loan_resources
                .get_mut(middle_handle)
                .captured_place
                .segments
                .clear(),
            7 => {
                checked
                    .facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get_mut(child.parent_end_status.child_weakening)
                    .source =
                    checked_trees::FlowInvalidationSource::Statement { statement_index: 3 }
            }
            8 => {
                checked
                    .facts
                    .borrow
                    .loans
                    .get_mut(middle.loan)
                    .statement_index = 0
            }
            9 | 10 => {
                let initializer = if mutation == 9 {
                    parent.initial_value
                } else {
                    leaf.initial_value
                };
                let ExpressionNode::Borrow(borrow) =
                    checked.typed.expression_table.expression_mut(initializer)
                else {
                    panic!("borrow initializer");
                };
                borrow.access = if mutation == 9 {
                    language_semantics::ReferenceAccess::WriteOnly
                } else {
                    language_semantics::ReferenceAccess::Mutable
                };
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::produce_terminal_artifact(&checked, "forward").is_err(),
            "mutable alias mutation {mutation}"
        );
    }
}

//! Direct exact-add proof custody, selected policy, target constraints, and corruption rejection.

use crate::tests::*;

#[test]
fn exact_add_selection_retains_proof_policy_and_target_constraints() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_exact_add_conditional(target);

        let plan = staged.selected().plan();
        let binary_index = plan.functions[0].blocks[1]
            .instructions
            .iter()
            .position(|row| matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. }))
            .unwrap();
        let false_binary_index = plan.functions[0].blocks[2]
            .instructions
            .iter()
            .position(|row| matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. }))
            .unwrap();
        let function = &plan.functions[0];

        let accepted = &staged
            .optimized_target()
            .optimized()
            .unit()
            .accepted_obligation_facts;
        assert_eq!(accepted.len(), 2);
        for (block, expected_obligation) in function.blocks[1..].iter().zip([
            ObligationId::new(5_031).unwrap(),
            ObligationId::new(5_032).unwrap(),
        ]) {
            let add = block
                .instructions
                .iter()
                .find(|row| matches!(row.kind, SelectedInstructionKind::ExactAddI64 { .. }))
                .unwrap();
            let SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            } = add.kind
            else {
                panic!("leaf arithmetic must retain exact-add semantics")
            };
            assert_eq!(obligation, expected_obligation);
            let fact = accepted
                .iter()
                .find(|fact| fact.identity == accepted_fact)
                .expect("selected fact must remain verifier-owned");
            assert_eq!(fact.operation, add.provenance.operations[0]);
            assert_eq!(fact.obligation, obligation);
            assert_eq!(
                add.constraint,
                staged.register_environment().selected_keys().add_i64
            );
            assert_eq!(
                add.operands
                    .iter()
                    .map(|operand| operand.access)
                    .collect::<Vec<_>>(),
                vec![
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Use,
                    RegisterOperandAccess::Def,
                ]
            );
            assert!(
                add.operands
                    .iter()
                    .all(|operand| operand.fixed_view.is_none())
            );
            assert!(add.operands.iter().all(|operand| operand.tied_to.is_none()));
            assert!(add.implicit_uses.is_empty());
            assert!(add.implicit_defs.is_empty());
            assert!(add.clobbers.is_empty());
            assert_eq!(add.provenance.operations.len(), 1);
            assert_eq!(add.provenance.values.len(), 3);
            assert_eq!(add.provenance.obligations, vec![obligation]);
            assert_eq!(add.provenance.fuel.len(), 1);
        }

        let original_identity = staged.selected().receipt().identity();
        let mut corrupted = plan.clone();
        let SelectedInstructionKind::ExactAddI64 { obligation, .. } =
            &mut corrupted.functions[0].blocks[1].instructions[binary_index].kind
        else {
            unreachable!()
        };
        *obligation = ObligationId::new(9_501).unwrap();
        assert_ne!(
            selected_instruction_plan_identity(&corrupted),
            original_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = plan.clone();
        let false_fact =
            match corrupted.functions[0].blocks[2].instructions[false_binary_index].kind {
                SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } => accepted_fact,
                _ => unreachable!(),
            };
        let SelectedInstructionKind::ExactAddI64 { accepted_fact, .. } =
            &mut corrupted.functions[0].blocks[1].instructions[binary_index].kind
        else {
            unreachable!()
        };
        *accepted_fact = false_fact;
        assert_ne!(
            selected_instruction_plan_identity(&corrupted),
            original_identity
        );
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[binary_index]
            .provenance
            .obligations[0] = ObligationId::new(9_502).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[binary_index]
            .operands
            .swap(0, 1);
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[binary_index].constraint =
            staged.register_environment().selected_keys().copy_i64;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
                | Err(SelectedInstructionError::ConstraintOperandMismatch { .. })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[binary_index]
            .provenance
            .operations[0] = OperationId::new(9_503).unwrap();
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
        ));

        let mut corrupted = plan.clone();
        corrupted.functions[0].blocks[1].instructions[binary_index]
            .provenance
            .fuel[0]
            .units += 1;
        assert!(matches!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::FunctionProjectionMismatch { function: 0 })
                | Err(SelectedInstructionError::ProvenancePartitionMismatch { .. })
        ));
    }
}

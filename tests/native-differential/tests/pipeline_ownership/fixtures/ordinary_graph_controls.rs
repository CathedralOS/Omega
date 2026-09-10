//! Common source and selected corruption controls over genuine admitted fixtures.
use crate::tests::*;
use legalized_operations::LegalizedScalarInstructionKind as Kind;

pub(crate) fn assert_ordinary_graph_custody(staged: &StagedOptimizedSelectedInstructions) {
    let original = staged.legalized().plan();
    let validate = |raw| {
        validate_legalized_operations(
            staged.optimized_target().target_operations(),
            staged.optimized_target().optimized().plan(),
            staged.optimized_target().optimized().unit(),
            raw,
        )
    };
    assert_eq!(
        validate(original.clone()).unwrap().receipt(),
        staged.legalized().receipt()
    );
    for (function_index, function) in original.scalar_functions.iter().enumerate() {
        for parameter_index in 0..function.parameters.len() {
            let mut raw = original.clone();
            raw.scalar_functions[function_index].parameters[parameter_index].definition_site =
                ValueDefinitionSite::FunctionParameter(999);
            assert!(validate(raw).is_err());
        }
        for (block_index, block) in function.blocks.iter().enumerate() {
            let mut raw = original.clone();
            match &mut raw.scalar_functions[function_index].blocks[block_index].terminator {
                legalized_operations::LegalizedScalarTerminator::StructuralCase {
                    source, ..
                } => {
                    // Dispatch binds a result or incoming state parameter, not
                    // an invented result operation for a transported owned value.
                    match source {
                        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                            operation,
                            ..
                        } => *operation = OperationId::new(999_999).unwrap(),
                        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                            block,
                            ..
                        } => *block = BlockId::new(999_999).unwrap(),
                    }
                }
                legalized_operations::LegalizedScalarTerminator::Return(returned) => {
                    returned.fuel.push(FuelSettlement {
                        site: PsiProvenance::Edge(returned.edge),
                        units: 999,
                    });
                }
                legalized_operations::LegalizedScalarTerminator::Jump { successor, .. } => {
                    successor.target = BlockId::new(999_999).unwrap();
                }
                legalized_operations::LegalizedScalarTerminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    std::mem::swap(when_true, when_false);
                }
            }
            assert!(validate(raw).is_err());
            for (node_index, node) in block.instructions.iter().enumerate() {
                for corruption in 0..6 {
                    let mut raw = original.clone();
                    let nodes =
                        &mut raw.scalar_functions[function_index].blocks[block_index].instructions;
                    match corruption {
                        0 => {
                            nodes.remove(node_index);
                        }
                        1 => nodes[node_index].operation = OperationId::new(999_999).unwrap(),
                        2 => {
                            if let Some(result) = &mut nodes[node_index].result {
                                result.value = ValueId::new(999_999).unwrap();
                            } else {
                                nodes[node_index].result =
                                    Some(legalized_operations::LegalizedValueDefinition {
                                        value: ValueId::new(999_999).unwrap(),
                                        scalar_type: semantic_vocabulary::ScalarType::Integer(
                                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                                        ),
                                        definition_site: ValueDefinitionSite::FunctionParameter(
                                            999,
                                        ),
                                    });
                            }
                        }
                        3 => {
                            if let Some(result) = &mut nodes[node_index].result {
                                result.definition_site =
                                    ValueDefinitionSite::FunctionParameter(999);
                            } else {
                                nodes[node_index].effect.input += 1;
                            }
                        }
                        4 => nodes[node_index].fuel.push(FuelSettlement {
                            site: PsiProvenance::Operation(node.operation),
                            units: 999,
                        }),
                        _ => {
                            if let Some(result) = &mut nodes[node_index].result {
                                result.scalar_type = semantic_vocabulary::ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                                );
                            } else {
                                nodes[node_index].effect.output += 1;
                            }
                        }
                    }
                    assert!(
                        validate(raw).is_err(),
                        "node {node_index}, corruption {corruption}"
                    );
                }
                if let Kind::ExactBinary {
                    accepted_fact,
                    obligation,
                    ..
                } = node.kind
                {
                    let fact = staged
                        .optimized_target()
                        .optimized()
                        .unit()
                        .accepted_obligation_facts
                        .iter()
                        .find(|fact| fact.identity == accepted_fact)
                        .expect("verifier-owned exact fact");
                    assert_eq!(fact.operation, node.operation);
                    assert_eq!(fact.obligation, obligation);
                    for corruption in 0..3 {
                        let mut raw = original.clone();
                        let Kind::ExactBinary {
                            accepted_fact,
                            obligation,
                            left,
                            right,
                            ..
                        } = &mut raw.scalar_functions[function_index].blocks[block_index]
                            .instructions[node_index]
                            .kind
                        else {
                            unreachable!()
                        };
                        match corruption {
                            0 => {
                                *accepted_fact =
                                    optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                        [99; 32],
                                    )
                            }
                            1 => *obligation = ObligationId::new(999_999).unwrap(),
                            _ => *left = *right,
                        }
                        if raw != *original {
                            assert!(validate(raw).is_err());
                        }
                    }
                }
                if let Kind::IntegerWiden { .. } = node.kind {
                    let mut raw = original.clone();
                    let Kind::IntegerWiden { source_type, .. } =
                        &mut raw.scalar_functions[function_index].blocks[block_index].instructions
                            [node_index]
                            .kind
                    else {
                        unreachable!()
                    };
                    *source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
                    assert!(validate(raw).is_err());
                }
            }
            if block.instructions.len() > 1 {
                let mut raw = original.clone();
                raw.scalar_functions[function_index].blocks[block_index]
                    .instructions
                    .swap(0, 1);
                assert!(validate(raw).is_err());
            }
        }
    }
    let selected = staged.selected().plan();
    validate_raw_selection(staged, selected.clone()).unwrap();
    for (function_index, function) in selected.functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for node_index in 0..block.instructions.len() {
                let mut raw = selected.clone();
                raw.functions[function_index].blocks[block_index].instructions[node_index]
                    .provenance
                    .operations
                    .push(OperationId::new(999_999).unwrap());
                assert!(validate_raw_selection(staged, raw).is_err());
                for operand_index in 0..block.instructions[node_index].operands.len() {
                    let mut raw = selected.clone();
                    raw.functions[function_index].blocks[block_index].instructions[node_index]
                        .operands[operand_index]
                        .virtual_register = VirtualRegisterId(u32::MAX);
                    assert!(validate_raw_selection(staged, raw).is_err());
                }
            }
            if matches!(
                block.terminator,
                SelectedTerminator::ConditionalBranch { .. }
            ) {
                let mut raw = selected.clone();
                let SelectedTerminator::ConditionalBranch {
                    when_nonzero,
                    when_zero,
                    ..
                } = &mut raw.functions[function_index].blocks[block_index].terminator
                else {
                    unreachable!()
                };
                std::mem::swap(when_nonzero, when_zero);
                assert!(validate_raw_selection(staged, raw).is_err());
            }
        }
    }
}

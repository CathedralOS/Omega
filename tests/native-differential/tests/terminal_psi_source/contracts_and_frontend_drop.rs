use super::*;

#[test]
fn checked_source_survives_frontend_drop_as_verified_psi() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let lowered = lower_machine(&checked, "terminal_constant")
        .expect("accepted source slice should lower to terminal Psi");

    assert_eq!(lowered.semantic_module.proposition_declarations.len(), 2);
    assert_eq!(lowered.semantic_module.proposition_applications.len(), 1);
    let relation = lowered
        .semantic_module
        .proposition_declarations
        .iter()
        .find(|declaration| declaration.name == "terminal_relation")
        .expect("primitive proposition should retain terminal identity");
    assert_eq!(relation.binders.len(), 3);
    assert!(matches!(
        relation.evidence,
        psi_terminal::PropositionEvidence::FactOnly
    ));
    let witness = lowered
        .semantic_module
        .proposition_declarations
        .iter()
        .find(|declaration| declaration.name == "terminal_witness")
        .expect("witness-bearing proposition should retain terminal identity");
    assert!(matches!(
        &witness.evidence,
        psi_terminal::PropositionEvidence::Witness { evidence_type }
            if evidence_type == "TerminalEvidence"
    ));
    let application = &lowered.semantic_module.proposition_applications[0];
    assert_eq!(application.declaration, relation.id);
    assert_eq!(application.binder_arguments.len(), 3);
    assert_eq!(application.arguments, ["7"]);

    drop(checked);

    let canonical_bytes = encode_module(&lowered.semantic_module)
        .expect("source-produced terminal Psi should encode canonically");
    let original_identity = terminal_psi_identity(&lowered.semantic_module)
        .expect("source-produced terminal Psi should have a semantic identity");
    let canonical_proof_bytes = encode_proof_bundle(&lowered.proof_bundle)
        .expect("source-produced proof bundle should encode canonically");
    let canonical_debug_bytes = encode_debug_map(
        &lowered.semantic_module,
        lowered
            .debug_map
            .as_ref()
            .expect("the source producer should retain its debug map"),
    )
    .expect("source-produced debug map should encode canonically");
    let artifact_manifest = build_artifact_manifest(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        None,
        Some(&canonical_debug_bytes),
    )
    .expect("source-produced terminal sections should have a manifest");
    drop(lowered);
    let semantic_module = decode_module(&canonical_bytes)
        .expect("canonical source-produced terminal Psi should decode");
    let proof_bundle = decode_proof_bundle(&canonical_proof_bytes)
        .expect("canonical source-produced proof bundle should decode");
    let debug_map = decode_debug_map(&semantic_module, &canonical_debug_bytes)
        .expect("canonical source-produced debug map should decode");
    validate_artifact_manifest(
        &semantic_module,
        &proof_bundle,
        None,
        Some(&canonical_debug_bytes),
        artifact_manifest,
    )
    .expect("decoded source-produced sections should match their manifest");
    assert_eq!(artifact_manifest.semantic(), original_identity);
    assert!(artifact_manifest.debug().is_some());
    assert_eq!(debug_map.semantic, original_identity);
    assert_eq!(debug_map.files.len(), 1);
    assert!(debug_map.files[0].path.ends_with("main.omg"));
    assert!(
        debug_map
            .sites
            .iter()
            .any(|site| matches!(site.subject, DebugSubject::Machine(_)))
    );
    assert!(
        debug_map
            .sites
            .iter()
            .any(|site| matches!(site.subject, DebugSubject::Operation(_)))
    );
    let source_text = std::fs::read_to_string(source_canary()).expect("read source debug canary");
    let snippets = |subject: fn(DebugSubject) -> bool| {
        debug_map
            .sites
            .iter()
            .filter(|site| subject(site.subject))
            .map(|site| {
                &source_text[usize::try_from(site.span.start).unwrap()
                    ..usize::try_from(site.span.end).unwrap()]
            })
            .collect::<Vec<_>>()
    };
    assert!(
        snippets(|subject| matches!(subject, DebugSubject::Operation(_)))
            .iter()
            .all(|snippet| *snippet == "7i32")
    );
    let edge_snippets = snippets(|subject| matches!(subject, DebugSubject::Edge(_)));
    assert!(edge_snippets.contains(&"7i32"));
    assert!(edge_snippets.contains(&"->"));
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity
    );

    let verified = verify_module(
        &semantic_module,
        &proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("source-produced terminal Psi and its proof should verify");
    let fixed_fuel = derive_fixed_entry_fuel(&verified, semantic_module.entry)
        .expect("straight-line source module should have a fixed-fuel certificate");
    validate_fixed_entry_fuel(&verified, &fixed_fuel)
        .expect("source-independent consumer should recompute the certificate");
    assert_eq!(fixed_fuel.terminal_psi(), original_identity);
    assert_eq!(fixed_fuel.ceiling_units(), 4);
    let abstract_operations = lower_artifact_sections(
        &canonical_bytes,
        &canonical_proof_bytes,
        &AdmissionProfile::default(),
    )
    .expect("canonical artifact sections should lower without producer state");
    let measured = interpret_terminal_artifact_measured(
        &canonical_bytes,
        &canonical_proof_bytes,
        &AdmissionProfile::default(),
        &[],
    )
    .expect("canonical artifact sections should execute with fuel");
    assert_eq!(measured.usage().schedule().marker(), 1);
    assert_eq!(measured.usage().total_units(), fixed_fuel.ceiling_units());
    assert_eq!(
        terminal_psi_identity(&semantic_module).unwrap(),
        original_identity,
        "fuel accounting must not change semantic identity"
    );
    let result = measured.value();
    drop(verified);
    drop(semantic_module);
    drop(proof_bundle);

    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    assert_eq!(
        abstract_operations,
        AbstractOperationPlan {
            psi: original_identity,
            entry: MachineId::new(1).expect("machine"),
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine: MachineId::new(1).expect("machine"),
                attachment: None,
                entry: BlockId::new(1).expect("entry block"),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: omega_abstract_operations::AbstractFunctionResult::Scalar(
                    omega_abstract_operations::AbstractResult {
                        value: ValueId::new(4).expect("machine result"),
                        scalar_type: ScalarType::Integer(i32_type),
                    },
                ),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: BlockId::new(1).expect("entry block"),
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: BlockId::new(2).expect("return block"),
                        parameters: vec![AbstractParameter {
                            value: ValueId::new(1).expect("block parameter"),
                            scalar_type: ScalarType::Integer(i32_type),
                        }],
                        operation_offset: 2,
                    },
                ],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(1).expect("operation"),
                        result: ValueId::new(2).expect("jump constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                        value: IntegerValue::Signed(7),
                    },
                    AbstractOperation::Jump {
                        psi_edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("return block"),
                        bindings: vec![ValueBinding {
                            parameter: ValueId::new(1).expect("block parameter"),
                            argument: ValueId::new(2).expect("jump constant"),
                            scalar_type: ScalarType::Integer(i32_type),
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(2).expect("operation"),
                        result: ValueId::new(3).expect("return constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                        value: IntegerValue::Signed(7),
                    },
                    AbstractOperation::Return {
                        cleanup_actions: Vec::new(),
                        psi_edge: EdgeId::new(2).expect("return edge"),
                        result: ValueId::new(4).expect("machine result"),
                        value: ValueId::new(3).expect("return constant"),
                        scalar_type: ScalarType::Integer(i32_type),
                    },
                ],
            }],
        }
    );
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: i32_type,
            value: IntegerValue::Signed(7),
        })
    );
}

#[test]
fn terminal_scalar_contract_consumes_the_source_independent_checked_plan() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked scalar contract should lower");

    let mut without_contract_expressions = checked.clone();
    let contract_expressions = {
        let machine = without_contract_expressions
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        without_contract_expressions
            .machine_contracts(machine)
            .iter()
            .flat_map(|contract| {
                without_contract_expressions
                    .proof_facts
                    .span_or_empty(contract.facts)
            })
            .filter_map(|fact| match fact {
                psi_typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for expression in contract_expressions {
        *without_contract_expressions
            .typed
            .expression_table
            .expression_mut(expression) =
            psi_checked_trees::expression::ExpressionNode::Boolean(false);
    }

    let actual = lower_machine(&without_contract_expressions, "terminal_constant")
        .expect("terminal production must not reopen checked contract expressions");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_contract = checked;
    let terminal_constant = without_checked_contract
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "terminal_constant")
        .expect("terminal constant machine")
        .symbol;
    without_checked_contract
        .facts
        .contract_plans
        .machines
        .iter_mut()
        .find(|plan| plan.machine == terminal_constant)
        .expect("terminal constant contract plan")
        .closed_scalar_values = Default::default();
    assert_eq!(
        lower_machine(&without_checked_contract, "terminal_constant")
            .expect_err("terminal production must fail without checked scalar contract values"),
        LoweringError::Unsupported("machine must have exactly one requires and one ensures clause")
    );
}

#[test]
fn terminal_scalar_body_consumes_the_source_independent_checked_plan() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected =
        lower_machine(&checked, "terminal_constant").expect("the checked scalar body should lower");

    let return_expression = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        checked
            .machine_states(machine)
            .iter()
            .flat_map(|state| {
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
            })
            .find_map(|statement| match statement {
                psi_checked_trees::statement::StatementNode::Expression(expression) => {
                    Some(*expression)
                }
                _ => None,
            })
            .expect("terminal constant return expression")
    };
    let mut without_typed_return = checked.clone();
    *without_typed_return
        .typed
        .expression_table
        .expression_mut(return_expression) =
        psi_checked_trees::expression::ExpressionNode::Boolean(false);

    let actual = lower_machine(&without_typed_return, "terminal_constant")
        .expect("terminal production must not reopen the checked return expression");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_scalar_body = checked;
    without_checked_scalar_body.facts.values.scalar_expressions = Default::default();
    assert_eq!(
        lower_machine(&without_checked_scalar_body, "terminal_constant")
            .expect_err("terminal production must fail without the checked scalar body"),
        LoweringError::Unsupported(
            "scalar expression has no source-independent checked value plan"
        )
    );
}

#[test]
fn terminal_scalar_control_consumes_the_source_independent_checked_plan() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked scalar control plan should lower");

    let replacement_transition = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_path_guarded_trap")
            .expect("guarded trap machine");
        checked
            .machine_states(machine)
            .iter()
            .flat_map(|state| {
                checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
            })
            .find(|statement| {
                matches!(
                    statement,
                    psi_checked_trees::statement::StatementNode::Transition(_)
                )
            })
            .expect("a valid replacement transition")
            .clone()
    };
    let constant_statements = {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "terminal_constant")
            .expect("terminal constant machine");
        checked
            .machine_states(machine)
            .first()
            .expect("terminal constant entry state")
            .statement_nodes
    };
    let mut without_typed_control = checked.clone();
    let [statement] = without_typed_control
        .typed
        .statement_table
        .statements_mut(constant_statements)
    else {
        panic!("terminal constant must have one statement");
    };
    *statement = replacement_transition;

    let actual = lower_machine(&without_typed_control, "terminal_constant")
        .expect("terminal production must not reopen checked statement topology");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_control = checked;
    without_checked_control.facts.flow.terminal_scalar_graphs = Default::default();
    assert_eq!(
        lower_machine(&without_checked_control, "terminal_constant")
            .expect_err("terminal production must fail without checked scalar control"),
        LoweringError::Unsupported("machine has no source-independent checked scalar control plan")
    );
}

#[test]
fn terminal_machine_selection_consumes_the_source_independent_checked_plan() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked machine selection should lower");
    let replacement_name = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() != "terminal_constant")
        .expect("a replacement machine name")
        .name
        .clone();

    let mut without_typed_selection = checked.clone();
    let source_machine = without_typed_selection
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "terminal_constant")
        .expect("terminal constant machine");
    source_machine.name = replacement_name;
    source_machine.supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;

    let actual = lower_machine(&without_typed_selection, "terminal_constant")
        .expect("terminal production must not reopen typed machine selection or eligibility");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_selection = checked;
    without_checked_selection.facts.flow.terminal_machines = Default::default();
    assert_eq!(
        lower_machine(&without_checked_selection, "terminal_constant")
            .expect_err("terminal production must fail without checked machine selection"),
        LoweringError::MachineNotFound("terminal_constant".to_owned())
    );
}

#[test]
fn terminal_production_survives_complete_typed_frontend_drop() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the complete checked terminal plan should lower");

    let mut without_typed_frontend = checked.clone();
    without_typed_frontend.typed = Default::default();
    let actual = lower_machine(&without_typed_frontend, "terminal_constant")
        .expect("terminal production must survive complete typed-tree disposal");
    assert_eq!(actual, expected);

    let mut without_debug_presentation = checked;
    without_debug_presentation.facts.flow.terminal_debug = Default::default();
    let without_debug = lower_machine(&without_debug_presentation, "terminal_constant")
        .expect("debug presentation must be optional at the terminal boundary");
    assert_eq!(without_debug.semantic_module, expected.semantic_module);
    assert_eq!(without_debug.proof_bundle, expected.proof_bundle);
    assert_eq!(without_debug.debug_map, None);
}

#[test]
fn terminal_proposition_vocabulary_consumes_checked_proof_facts() {
    let checked = compile_to_checked(&source_canary(), None).unwrap_or_else(|diagnostics| {
        panic!(
            "terminal-Psi source canary should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let expected = lower_machine(&checked, "terminal_constant")
        .expect("the checked proposition vocabulary should lower");
    assert!(!expected.semantic_module.proposition_declarations.is_empty());
    assert!(!expected.semantic_module.proposition_applications.is_empty());

    let mut without_typed_declarations = checked.clone();
    without_typed_declarations.typed.roots.propositions = Default::default();
    let actual = lower_machine(&without_typed_declarations, "terminal_constant")
        .expect("terminal production must not reopen typed proposition declarations");
    assert_eq!(actual.semantic_module, expected.semantic_module);
    assert_eq!(actual.proof_bundle, expected.proof_bundle);

    let mut without_checked_vocabulary = checked;
    without_checked_vocabulary
        .facts
        .proof
        .proposition_vocabulary = Default::default();
    let absent = lower_machine(&without_checked_vocabulary, "terminal_constant")
        .expect("an intentionally empty checked proposition vocabulary remains valid");
    assert!(absent.semantic_module.proposition_declarations.is_empty());
    assert!(absent.semantic_module.proposition_applications.is_empty());
}

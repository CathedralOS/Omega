use typed_trees::TypedTrees;

const NOMINAL_SCHEMA_FORWARDING: &str = r#"
    boundary trait Console { machine ping(); }
    boundary trait Family { machine call<const Number: u64>(value: u64) -> u64 reaches Console; }
    machine selected<const Count: u64>(value: u64) -> u64 satisfies Family::call reaches Console { Count }
    machine inner<machine Schema>(value: u64) -> u64
    where machine Schema satisfies Family::call;
    { Schema<3>(value) }
    machine outer<machine Schema>(value: u64) -> u64
    where machine Schema satisfies Family::call;
    { inner<Schema>(value) }
    pub machine enter(value: u64) -> u64 { outer<selected>(value) }
"#;

#[test]
fn nominal_generic_family_satisfaction_survives_private_specialization() {
    crate::lower_typed_trees(typed(NOMINAL_SCHEMA_FORWARDING))
        .expect("exact generic satisfaction survives nested private forwarding");
}

#[test]
fn nominal_generic_family_rejects_a_different_const_carrier() {
    let source = NOMINAL_SCHEMA_FORWARDING
        .replace("selected<const Count: u64>", "selected<const Count: u32>")
        .replace("reaches Console { Count }", "reaches Console { value }");
    assert!(crate::lower_typed_trees(typed(&source)).is_err());
}

#[test]
fn nominal_generic_family_specialization_keeps_concrete_refinement_checks() {
    let mut original = typed(NOMINAL_SCHEMA_FORWARDING);
    crate::specialize_static_machine_calls(&mut original).expect("closed schema calls");
    validation::validate_program(&original).expect("valid concrete family application");
    let template = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .unwrap()
        .symbol;
    let application = original
        .machine_specializations
        .iter()
        .find(|application| application.template == template)
        .unwrap()
        .clone();
    let instance_position = original
        .machines()
        .iter()
        .position(|machine| machine.symbol == application.instance)
        .unwrap();

    for mutation in 0..7 {
        let mut changed = original.clone();
        let instance = changed.machines()[instance_position].clone();
        let unit = changed
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let expected = match mutation {
            0 => {
                changed.machine_states_mut(&instance)[0].return_type = unit;
                "expected return"
            }
            1 => {
                let parameters = changed.machine_states(&instance)[0].parameters;
                changed.state_parameters.span_mut_or_empty(parameters)[0].type_reference = unit;
                "parameter `value`"
            }
            2 => {
                changed.machines_mut()[instance_position].blocks = true;
                "operational ceiling"
            }
            3 => {
                changed
                    .machine_specializations
                    .retain(|retained| retained.instance != instance.symbol);
                "callable generic parameter"
            }
            4 => {
                changed.machine_specializations.push(application.clone());
                "exact original family and application"
            }
            5 => {
                let retained = changed
                    .machine_specializations
                    .iter_mut()
                    .find(|retained| retained.instance == instance.symbol)
                    .unwrap();
                retained.template_parameters = arena::HandleSpan::empty();
                "exact original family and application"
            }
            _ => {
                changed
                    .machine_trait_conformances
                    .span_mut_or_empty(instance.satisfies)[0]
                    .alias = Some("different".into());
                "exact original family and application"
            }
        };
        let diagnostics =
            validation::validate_program(&changed).expect_err("mutated private contract rejects");
        assert!(
            format!("{diagnostics:?}").contains(expected),
            "mutation {mutation}: {diagnostics:?}"
        );
    }
}

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("typing")
}

fn specialized() -> TypedTrees {
    let source = "boundary trait Console {}\n\
        machine quiet(value: u64) -> u64 terminates { value }\n\
        machine alternate(value: u64) -> u64 terminates { value }\n\
        machine traverse<machine Step>(value: u64) -> u64\n\
        where machine Step(value: u64) -> u64 reaches Console terminates;\n\
        terminates; { Step(value) }\n\
        machine first() -> u64 { traverse<quiet>(7) }\n\
        machine second() -> u64 { traverse<alternate>(9) }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolution");
    let mut program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typing");
    crate::specialize_static_machine_calls(&mut program).expect("specializations");
    program
}

fn validate(program: &TypedTrees) -> Result<(), diagnostics::Diagnostic> {
    validation::validate_static_machine_call_contracts(
        program,
        &validation::infer_operational_may(program),
    )
}

#[test]
fn retained_static_calls_reject_missing_duplicate_and_wrong_selection_joins() {
    let program = specialized();
    assert_eq!(program.machine_specializations.len(), 2);
    validate(&program).expect("both selected instances retain exact binder custody");

    let mut missing = program.clone();
    missing.machine_specializations.remove(0);
    assert!(validate(&missing).is_err(), "missing owner must reject");

    let mut duplicate = program.clone();
    duplicate
        .machine_specializations
        .push(duplicate.machine_specializations[0].clone());
    assert!(validate(&duplicate).is_err(), "duplicate owner must reject");

    let mut wrong_selection = program.clone();
    wrong_selection.machine_specializations[0].machine_arguments[0] =
        program.machine_specializations[1].machine_arguments[0];
    assert!(
        validate(&wrong_selection).is_err(),
        "selected entry must match the call target"
    );

    let mut wrong_binder = program.clone();
    let call = wrong_binder
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            call.static_machine_parameter.is_valid().then_some(handle)
        })
        .expect("retained binder call");
    let typed_trees::expression::ExpressionNode::Call(call) =
        wrong_binder.expression_table.expression_mut(call)
    else {
        panic!("selected call");
    };
    call.static_machine_parameter = call.target_symbol;
    assert!(
        validate(&wrong_binder).is_err(),
        "ordinary target is not a template binder"
    );
}

#[test]
fn deleting_static_call_binders_changes_the_specialization_commitment() {
    let checked = crate::lower_typed_trees(specialized()).expect("checked static calls");
    let mut missing = checked.clone();
    let calls: Vec<_> = missing
        .typed
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            call.static_machine_parameter.is_valid().then_some(handle)
        })
        .collect();
    assert_eq!(calls.len(), 2);
    for handle in calls {
        let typed_trees::expression::ExpressionNode::Call(call) =
            missing.typed.expression_table.expression_mut(handle)
        else {
            panic!("retained call");
        };
        call.static_machine_parameter = symbols::SymbolHandle::invalid();
    }
    for specialization in &missing.typed.machine_specializations {
        let replay = crate::recompute_machine_specialization_commitment(
            &missing.typed,
            &missing.facts.contract_plans,
            specialization,
        )
        .expect("remaining joins are individually valid");
        assert_ne!(
            replay, specialization.commitment,
            "deleting custody must change the application commitment"
        );
        let independent = validation::recompute_checked_machine_specialization_commitment(
            &missing,
            specialization.instance,
        )
        .expect("independent replay reconstructs the changed footprint");
        assert_eq!(independent, replay.as_bytes());
    }
    let contracts = missing.facts.contract_plans.clone();
    assert!(
        super::bind_specialization_contract_identities(&mut missing.typed, &contracts).is_err(),
        "the old claimed commitment cannot authorize the changed calls"
    );
}

#[test]
fn retained_static_parameter_operational_contract_mutations_reject() {
    let program = specialized();
    let binder = program
        .data_type_parameters
        .iter()
        .find_map(|(handle, parameter)| {
            matches!(
                parameter.kind,
                typed_trees::data::TypeParameterKind::Machine { .. }
            )
            .then_some(handle)
        })
        .expect("machine binder");
    for change_suspend in [false, true] {
        let mut changed = program.clone();
        let parameter = changed.data_type_parameters.get_mut(binder);
        let typed_trees::data::TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::Structural(signature),
        } = &mut parameter.kind
        else {
            panic!("structural binder");
        };
        if change_suspend {
            signature.suspends = !signature.suspends;
        } else {
            signature.service_reach_row = Default::default();
        }
        assert!(
            validate(&changed).is_err(),
            "retained reader inputs must match committed bytes"
        );
    }
}

#[test]
fn forwarded_nominal_binders_retain_exact_owners_and_selected_reach() {
    let checked = crate::lower_typed_trees(typed(
        "boundary trait Console {}\n\
         trait Task { machine run(value: u64) -> u64 reaches Console; }\n\
         machine quiet(value: u64) -> u64 satisfies Task::run { value }\n\
         machine loud(value: u64) -> u64 satisfies Task::run reaches Console { value }\n\
         machine traverse<machine Step>(value: u64) -> u64\n\
         where machine Step satisfies Task::run; { Step(value) }\n\
         machine outer<machine Selected>(value: u64) -> u64\n\
         where machine Selected satisfies Task::run; { traverse<Selected>(value) }\n\
         machine first() -> u64 { outer<quiet>(7) }\n\
         machine second() -> u64 { outer<loud>(9) }",
    ))
    .expect("nested closed callback applications");
    validate(&checked.typed).expect("forwarded binders retain their own template custody");
    let reaches = &checked.facts.service_reaches;
    let console = reaches.services.id_for_name("Console").expect("Console");
    for family in ["outer", "traverse"] {
        let instances: Vec<_> = checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == family
                })
            })
            .collect();
        assert_eq!(instances.len(), 2, "both closed selections of {family}");
        for specialization in instances {
            let selected = specialization.machine_arguments[0];
            let owner = checked
                .machines()
                .iter()
                .find(|machine| {
                    checked
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == selected)
                })
                .expect("exact selected entry owner");
            let summary = reaches
                .for_machine(specialization.instance)
                .expect("instance row");
            let expected = if owner.name.as_str() == "quiet" {
                Vec::new()
            } else {
                vec![console]
            };
            assert_eq!(
                reaches.rows.services(summary.effective),
                expected.as_slice()
            );
            assert!(matches!(owner.name.as_str(), "quiet" | "loud"));
        }
    }
}

#[test]
fn statement_and_named_tail_calls_retain_binders_without_internal_transfer_calls() {
    for (result, body, selected_body) in [
        (
            "",
            "Work(); transition { _ -> finish() } state finish() {}",
            "",
        ),
        (
            "-> u64",
            "transition { _ -> finish() } state finish() -> u64 { transition { _ -> Work() } }",
            "0",
        ),
    ] {
        let checked = crate::lower_typed_trees(typed(&format!(
            "boundary trait Audit {{}}\n\
             machine quiet() {result} {{ {selected_body} }}\n\
             machine invoke<machine Work>() {result}\n\
             where machine Work() {result} reaches Audit; {{ {body} }}\n\
             machine caller() {result} {{ invoke<quiet>() }}"
        )))
        .expect("selected static call with an internal state transfer");
        validate(&checked.typed).expect("exact retained selection");
        let specialization = checked
            .machine_specializations
            .first()
            .expect("invoke instance");
        let operational = validation::infer_operational_may(&checked.typed);
        let owner = operational
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("invoke owner");
        let calls: Vec<_> = operational
            .states
            .span_or_empty(owner.states)
            .iter()
            .flat_map(|state| operational.calls.span_or_empty(state.calls))
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "the internal state transfer is not a machine call"
        );
        assert!(calls[0].static_machine_parameter.is_valid());
        assert_eq!(
            calls[0].target_state_symbol,
            specialization.machine_arguments[0]
        );
        let reaches = &checked.facts.service_reaches;
        let audit = reaches.services.id_for_name("Audit").expect("Audit");
        let summary = reaches
            .for_machine(specialization.instance)
            .expect("structural row");
        assert_eq!(reaches.rows.services(summary.effective), &[audit]);
    }
}

#[test]
fn quiet_selections_do_not_change_fixed_suspension_acknowledgements() {
    for nominal in [false, true] {
        for (requirement_suspends, marker, accepted) in [
            (true, "suspend ", true),
            (true, "", false),
            (false, "suspend ", false),
        ] {
            let suspension = if requirement_suspends { "suspends" } else { "" };
            let contract = if nominal {
                "machine Step satisfies Task::run;".to_owned()
            } else {
                format!("machine Step() {suspension};")
            };
            let source = format!(
                "trait Task {{ machine run() {suspension}; }}\n\
                 machine quiet() satisfies Task::run {{}}\n\
                 machine invoke<machine Step>() where {contract} requires true; suspends; {{ {marker}Step(); }}\n\
                 machine caller() suspends; {{ suspend invoke<quiet>(); }}"
            );
            let result = crate::lower_typed_trees(typed(&source));
            if accepted {
                let checked = result.expect("the fixed requirement marker remains valid for quiet");
                validate(&checked.typed).expect("selected quiet contract custody");
                let operational = validation::infer_operational_may(&checked.typed);
                let instance = checked.machine_specializations[0].instance;
                assert!(
                    operational
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == instance)
                        .expect("invoke operational summary")
                        .transitive_may_suspend
                );
            } else {
                let diagnostics = result.expect_err("missing or redundant marker must reject");
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("suspend")),
                    "{diagnostics:?}"
                );
            }
        }
    }
}

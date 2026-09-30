use checked_trees::{CheckedComposedUnitControlMachinePlan, CheckedTrees};

fn checked_context(attached_call: bool) -> CheckedTrees {
    let prefix = if attached_call {
        "self.host.mark(7);"
    } else {
        ""
    };
    let source = format!(
        "pub boundary trait Host {{ machine mark(value: u64); machine other(value: u64); }}
         data Main {{ host: Binding<Host>; spare: Binding<Host>; counter: u64; }}
         machine Main::run(&mut self) reaches Host {{
             {prefix}
             transition {{ _ -> next(&mut self) }}
             state next(context: &mut Main) {{ context.host.mark(8); }}
         }}"
    );
    checked_binding_source(&source)
}

fn checked_binding_source(source: &str) -> CheckedTrees {
    use source::{SourceMap, SourceOrigin};
    use std::path::PathBuf;

    let binding = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/binding.omg"
    ));
    let mut sources = SourceMap::default();
    let binding_source = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/binding.omg"),
            binding.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let program_source = sources
        .add(PathBuf::from("tests/main.omg"), source.to_owned())
        .source_id;
    let mut typed = crate::front_end::typed_program_from_source_map(
        sources,
        &[(binding_source, binding), (program_source, source)],
    );
    let authorizations = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .map(
            |definition| typed_trees::typed_trees::FusedServiceErasureAuthorization {
                requirement: definition.symbol,
            },
        )
        .collect();
    typed
        .bind_fused_service_erasures(authorizations)
        .expect("fixture authorization");
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("source-produced provider graph")
}

fn graph(checked: &CheckedTrees) -> &CheckedComposedUnitControlMachinePlan {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("authored entry");
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("composed entry plan")
}

#[test]
fn provider_occurrences_allow_ordinary_attached_and_fused_parameter_calls() {
    let checked = checked_binding_source(
        "pub boundary trait Host { machine mark(value: u64); }
         data Main { host: Binding<Host>; }
         machine Main::run(&mut self, context: &mut Main) reaches Host {
             self.host.mark(7);
             context.host.mark(8);
         }",
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("authored ordinary entry");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == machine.symbol)
        .expect("ordinary Unit plan");
    assert_eq!(plan.provider_attachment_requirements.len(), 1);
    let classifications = plan
        .operations
        .iter()
        .filter(|operation| operation.call().is_some())
        .map(|operation| {
            super::validate_call_source(
                &checked,
                plan.machine,
                plan.state,
                operation,
                &plan.structural_parameters,
                &plan.provider_attachment_requirements,
            )
            .expect("exact ordinary call occurrence")
        })
        .collect::<Vec<_>>();
    assert_eq!(classifications, [true, true]);
    let lowered =
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("Main::run"))
            .expect("ordinary attached and explicit-context calls lower together");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("ordinary mixed-route module independently verifies");

    let context_identity = &plan
        .structural_parameters
        .iter()
        .find(|parameter| !parameter.is_self)
        .expect("ordinary context parameter")
        .type_identity;
    for mutation in ["unresolved provider", "missing authorization"] {
        let mut changed = checked.clone();
        if mutation == "missing authorization" {
            changed
                .typed
                .bind_fused_service_erasures(Vec::new())
                .unwrap();
        } else {
            let shape = changed
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter_mut()
                .find(|shape| shape.identity == *context_identity)
                .expect("context carrier shape");
            let checked_trees::CheckedUnitStructuralTypeShape::Record { fields } = &mut shape.shape
            else {
                panic!("context record");
            };
            let field = fields
                .iter_mut()
                .find(|field| field.identity == "host")
                .unwrap();
            let checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked {
                provider_type_identity,
                ..
            } = &field.field_type
            else {
                panic!("authorized Fused field");
            };
            field.field_type = checked_trees::CheckedUnitStructuralFieldType::ProviderBacked {
                provider_type_identity: provider_type_identity.clone(),
            };
        }
        assert!(
            crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("Main::run"))
                .is_err(),
            "{mutation} cannot replace independently authorized Fused custody"
        );
    }
}

#[test]
fn provider_occurrences_retain_exact_field_roots_across_receiver_spellings() {
    for attached_call in [false, true] {
        let checked = checked_context(attached_call);
        let plan = graph(&checked);
        assert_eq!(plan.provider_attachment_requirements.len(), 1);
        let mut classifications = Vec::new();
        let mut boundaries = Vec::new();
        let mut attachment_calls = Vec::new();
        for state in &plan.states {
            for operation in &state.operations {
                let Some(call) = operation.call().filter(|call| call.is_boundary()) else {
                    continue;
                };
                let uses_attachment = super::validate_call_source(
                    &checked,
                    plan.machine,
                    state.state,
                    operation,
                    &state.structural_parameters,
                    &plan.provider_attachment_requirements,
                )
                .expect("independent occurrence replay");
                classifications.push(uses_attachment);
                boundaries.push(call.target.machine);
                if uses_attachment {
                    attachment_calls.push(call.target.machine);
                }
            }
        }
        assert_eq!(
            classifications,
            if attached_call {
                vec![true, true]
            } else {
                vec![true]
            }
        );
        assert!(
            boundaries.iter().all(|boundary| *boundary == boundaries[0]),
            "same exact boundary"
        );
        let attachment = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .find(|shape| Some(&shape.identity) == plan.attachment_type_identity.as_ref())
            .expect("retained Main attachment");
        super::validate_provider_attachment_requirements(
            &checked,
            attachment,
            &plan.provider_attachment_requirements,
            &attachment_calls,
        )
        .expect("both receiver spellings retain exact field coverage");
        assert!(
            super::validate_provider_attachment_requirements(
                &checked,
                attachment,
                &[],
                &attachment_calls,
            )
            .is_err(),
            "ordinary parameter calls cannot erase their provider roots"
        );
        let lowered =
            crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("Main::run"))
                .expect("both source receiver routes lower together");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independent Terminal verification");
        reject_substituted_self_edge(&checked);
    }
}

/// Machine-root spelling is valid only for the exact declared self source.
/// A different, valid parameter symbol cannot acquire that normalization.
fn reject_substituted_self_edge(checked: &CheckedTrees) {
    use checked_trees::expression::ExpressionNode;
    use checked_trees::statement::{StatementNode, TransitionTargetNode};
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
    };

    let plan = graph(checked);
    let entry = &plan.states[0];
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } = &entry.terminator else {
        panic!("authored self handoff");
    };
    assert!(matches!(
        successor.transfers[0].source,
        CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
    ));
    let (_, source) =
        crate::expression_preparation::source_custody::authored_state(checked, entry.state)
            .expect("entry source");
    let StatementNode::Transition(transition) = &checked
        .statement_table
        .statements(source.statement_nodes)[successor.statement_ordinal as usize]
    else {
        panic!("authored transition");
    };
    let TransitionTargetNode::Named { arguments, .. } =
        checked.statement_table.transition_target(transition.target)
    else {
        panic!("named successor");
    };
    let argument = checked.statement_table.expression_handles(*arguments)[0];
    let ExpressionNode::Borrow(borrow) = checked.expression_table.expression(argument) else {
        panic!("explicit self borrow");
    };
    let root = borrow.target;
    let ExpressionNode::Name(name) = checked.expression_table.expression(root) else {
        panic!("self root");
    };
    assert_eq!(
        (name.symbol, name.head_symbol),
        (plan.machine, plan.machine)
    );
    let (_, target) = crate::expression_preparation::source_custody::authored_state(
        checked,
        successor.target_state,
    )
    .expect("successor source");
    let foreign = checked.state_parameters(target)[0].symbol;
    assert_ne!(foreign, plan.machine);

    for mutation in [
        "leaf identity",
        "head identity",
        "both identities",
        "source index",
        "target index",
        "source position",
    ] {
        let mut changed = checked.clone();
        if matches!(
            mutation,
            "leaf identity" | "head identity" | "both identities"
        ) {
            let ExpressionNode::Name(name) = changed.typed.expression_table.expression_mut(root)
            else {
                panic!("retained source name");
            };
            if mutation != "head identity" {
                name.symbol = foreign;
            }
            if mutation != "leaf identity" {
                name.head_symbol = foreign;
            }
            assert_eq!(changed.facts, checked.facts, "only source identity changes");
        } else {
            let retained = changed
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter_mut()
                .find(|candidate| candidate.machine == plan.machine)
                .unwrap();
            let entry = &mut retained.states[0];
            if mutation == "source position" {
                entry.structural_parameters[0].position = u32::MAX;
            } else {
                let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
                    &mut entry.terminator
                else {
                    panic!("retained self edge");
                };
                if mutation == "source index" {
                    successor.transfers[0].source =
                        CheckedStructuralControlTransferSourcePlan::Parameter { index: u32::MAX };
                } else {
                    successor.transfers[0].target_parameter_index = u32::MAX;
                }
            }
        }
        assert!(
            crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("Main::run"))
                .is_err(),
            "{mutation} cannot inherit the declared self root normalization"
        );
    }
}

#[test]
fn provider_occurrence_replay_rejects_substituted_parameter_custody() {
    let checked = checked_context(true);
    let plan = graph(&checked);
    let state = plan
        .states
        .iter()
        .find(|state| {
            state
                .structural_parameters
                .iter()
                .any(|parameter| !parameter.is_self)
        })
        .expect("ordinary context state");
    let operation = state
        .operations
        .iter()
        .find(|operation| operation.call().is_some())
        .expect("parameter boundary occurrence");
    assert!(
        super::validate_call_source(
            &checked,
            plan.machine,
            state.state,
            operation,
            &state.structural_parameters,
            &plan.provider_attachment_requirements,
        )
        .expect("untampered parameter occurrence")
    );
    let (_, authored_state) =
        crate::expression_preparation::source_custody::authored_state(&checked, state.state)
            .expect("authored state");
    let statements = authored_state.statement_nodes;
    let coordinate = operation.call().unwrap().coordinate;
    for mutation in [
        "root",
        "field",
        "sibling field",
        "authorization",
        "receipt",
        "parameter",
        "coordinate",
        "requirement",
    ] {
        let mut changed = checked.clone();
        let mut changed_operation = operation.clone();
        let mut parameters = state.structural_parameters.clone();
        match mutation {
            "root" | "field" | "sibling field" => {
                let sibling = changed
                    .data_definitions()
                    .iter()
                    .flat_map(|definition| changed.data_members(definition))
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Field(field)
                            if field.name.as_str() == "spare" =>
                        {
                            Some(field.symbol)
                        }
                        _ => None,
                    })
                    .expect("valid same-boundary sibling field");
                let checked_trees::statement::StatementNode::Call(call) =
                    &mut changed.typed.statement_table.statements_mut(statements)
                        [coordinate.statement_index as usize]
                else {
                    panic!("authored parameter call");
                };
                if mutation == "root" {
                    call.receiver_root_symbol = symbols::SymbolHandle::invalid();
                } else if mutation == "sibling field" {
                    call.receiver_symbol = sibling;
                } else {
                    call.receiver_symbol = symbols::SymbolHandle::invalid();
                }
            }
            "authorization" => changed
                .typed
                .bind_fused_service_erasures(Vec::new())
                .unwrap(),
            "receipt" => {
                let field = changed
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .iter_mut()
                    .filter_map(|shape| match &mut shape.shape {
                        checked_trees::CheckedUnitStructuralTypeShape::Record { fields } => {
                            Some(fields)
                        }
                        _ => None,
                    })
                    .flatten()
                    .find(|field| {
                        matches!(
                            field.field_type,
                            checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked { .. }
                        )
                    })
                    .expect("retained Binding receipt");
                let checked_trees::CheckedUnitStructuralFieldType::FusedServiceBacked {
                    erasure,
                    ..
                } = &mut field.field_type
                else {
                    panic!("Fused field");
                };
                erasure.requirement = symbols::SymbolHandle::invalid();
            }
            "parameter" => parameters.clear(),
            "coordinate" => {
                changed_operation
                    .call_mut()
                    .unwrap()
                    .coordinate
                    .call_ordinal = u32::MAX
            }
            "requirement" => {
                let other = checked
                    .traits()
                    .iter()
                    .flat_map(|owner| checked.trait_machine_signatures(owner))
                    .find(|signature| signature.symbol != operation.call().unwrap().target.machine)
                    .expect("different requirement")
                    .symbol;
                let call = changed_operation.call_mut().unwrap();
                call.target.machine = other;
                call.target.state = other;
            }
            _ => unreachable!(),
        }
        assert!(
            super::validate_call_source(
                &changed,
                plan.machine,
                state.state,
                &changed_operation,
                &parameters,
                &plan.provider_attachment_requirements,
            )
            .is_err(),
            "{mutation} must not acquire independent Fused admission"
        );
    }
}

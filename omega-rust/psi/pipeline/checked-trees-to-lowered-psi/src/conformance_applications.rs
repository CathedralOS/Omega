use checked_trees::data::TypeParameterKind;
use checked_trees::{CheckedTrees, ClosedConformanceConstArgument};
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, TerminalModule,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

use super::LoweringError;

pub(super) fn lower_closed_conformance_applications(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let carries_applications = checked
        .machine_specializations
        .iter()
        .any(|specialization| {
            source_machines.contains(&specialization.instance)
                && !specialization.conformance_applications.is_empty()
        });
    if !carries_applications {
        module.closed_conformance_applications.clear();
        return Ok(());
    }
    if source_machines.len() != module.machines.len() {
        return Err(LoweringError::Unsupported(
            "closed conformance application ownership does not match the terminal machine closure",
        ));
    }

    let owners = source_machines
        .iter()
        .zip(&module.machines)
        .map(|(source, terminal)| (*source, terminal.id))
        .collect::<Vec<_>>();
    module.closed_conformance_applications =
        collect_closed_conformance_applications(checked, &owners, &[])?;
    Ok(())
}

/// Dynamic roots already retain their selected application. Add independently
/// selected callee applications without replacing that dynamic custody.
pub(super) fn append_closed_conformance_applications_excluding(
    checked: &CheckedTrees,
    owners: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    excluded_source: symbols::SymbolHandle,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if owners.len() != module.machines.len()
        || owners
            .iter()
            .zip(&module.machines)
            .any(|((_, owner), machine)| *owner != machine.id)
        || owners.iter().enumerate().any(|(index, (source, owner))| {
            owners[..index]
                .iter()
                .any(|(prior_source, prior_owner)| source == prior_source || owner == prior_owner)
        })
    {
        return Err(LoweringError::Unsupported(
            "closed conformance application owners do not match the exact terminal closure",
        ));
    }
    let applications =
        collect_closed_conformance_applications(checked, owners, &[excluded_source])?;
    for application in applications {
        let existing = module
            .closed_conformance_applications
            .iter()
            .find(|existing| {
                existing.owner == application.owner
                    && existing.declaration_identity == application.declaration_identity
                    && existing.report_fingerprint == application.report_fingerprint
            });
        if let Some(existing) = existing {
            if existing != &application {
                return Err(LoweringError::Unsupported(
                    "retained closed conformance application differs from its source selection",
                ));
            }
        } else {
            module.closed_conformance_applications.push(application);
        }
    }
    sort_applications(&mut module.closed_conformance_applications);
    Ok(())
}

fn collect_closed_conformance_applications(
    checked: &CheckedTrees,
    owners: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    excluded_sources: &[symbols::SymbolHandle],
) -> Result<Vec<ClosedConformanceApplication>, LoweringError> {
    let mut applications = Vec::new();
    for specialization in &checked.machine_specializations {
        if excluded_sources.contains(&specialization.instance) {
            continue;
        }
        let Some((_, owner)) = owners
            .iter()
            .find(|(source, _)| *source == specialization.instance)
        else {
            continue;
        };
        for application in &specialization.conformance_applications {
            let selected = checked
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == application.declaration)
                .ok_or(LoweringError::Unsupported(
                    "closed conformance application lost its declaration",
                ))?;
            let selected_rows =
                checked
                    .closed_conformance_rows(selected)
                    .ok_or(LoweringError::Unsupported(
                        "closed conformance application lost its normalized row map",
                    ))?;
            let mut realization_callables = checked
                .facts
                .proof
                .proof_output_calls
                .iter()
                .filter_map(|(_, invocation)| {
                    (invocation.caller_machine_symbol == specialization.instance)
                        .then_some(invocation.static_requirement_dispatch?)
                })
                .filter(|dispatch| {
                    dispatch.application_report_fingerprint == application.report_fingerprint
                        && dispatch.application_commitment == application.commitment
                })
                .map(|dispatch| {
                    let selected_row = selected_rows
                        .iter()
                        .find(|row| {
                            row.declaring_trait == dispatch.declaring_trait
                                && row.requirement == dispatch.requirement
                                && row.realization_machine == dispatch.realization_machine
                                && row.realization_state == dispatch.realization_state
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance dispatch lost its normalized row",
                        ))?;
                    let machine = owners
                        .iter()
                        .find_map(|(source, terminal)| {
                            (*source == selected_row.realization_machine).then_some(*terminal)
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization is absent from the Terminal closure",
                        ))?;
                    let realization_state = checked
                        .typed
                        .machines()
                        .iter()
                        .flat_map(|machine| checked.typed.machine_states(machine))
                        .find(|state| state.symbol == selected_row.realization_state)
                        .ok_or(LoweringError::Unsupported(
                            "static conformance realization state is absent",
                        ))?;
                    let requirements = checked
                        .typed
                        .traits()
                        .iter()
                        .find(|definition| definition.symbol == dispatch.declaring_trait)
                        .map(|definition| {
                            checked
                                .typed
                                .trait_machine_signatures(definition)
                                .iter()
                                .filter(|requirement| requirement.symbol == dispatch.requirement)
                                .collect::<Vec<_>>()
                        })
                        .ok_or(LoweringError::Unsupported(
                            "static conformance requirement is absent",
                        ))?;
                    let [requirement] = requirements.as_slice() else {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement is absent or ambiguous",
                        ));
                    };
                    let classify = |return_type: checked_trees::types::TypeReferenceHandle| {
                        if !return_type.is_valid() {
                            return Ok(ClosedConformanceCallableResult::Unit);
                        }
                        match checked.typed.primitive_type_reference(return_type) {
                            Some(super::PrimitiveType::I32) => {
                                Ok(ClosedConformanceCallableResult::I32)
                            }
                            Some(super::PrimitiveType::Bool) => {
                                Ok(ClosedConformanceCallableResult::Bool)
                            }
                            _ => Err(LoweringError::Unsupported(
                                "static conformance callable has an unsupported result class",
                            )),
                        }
                    };
                    let result = classify(realization_state.return_type)?;
                    if classify(requirement.return_type)? != result {
                        return Err(LoweringError::Unsupported(
                            "static conformance requirement and realization result classes differ",
                        ));
                    }
                    Ok(ClosedConformanceRealizationCallable {
                        source_callable_identity:
                            super::evidence_lowering::checked_evidence_machine_identity(
                                checked,
                                selected_row.realization_machine,
                            )?,
                        machine,
                        result,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            realization_callables.sort();
            realization_callables.dedup();
            let rows = application
                .rows
                .iter()
                .map(|row| {
                    let selected_row = selected_rows
                        .iter()
                        .find(|candidate| {
                            candidate.declaring_trait == row.declaring_trait
                                && candidate.requirement == row.requirement
                                && candidate.realization_state == row.realization_state
                        })
                        .ok_or(LoweringError::Unsupported(
                            "closed conformance application row no longer matches its declaration",
                        ))?;
                    let selected_by_static_dispatch = checked
                        .facts
                        .proof
                        .proof_output_calls
                        .iter()
                        .filter_map(|(_, invocation)| {
                            (invocation.caller_machine_symbol == specialization.instance)
                                .then_some(invocation.static_requirement_dispatch?)
                        })
                        .any(|dispatch| {
                            dispatch.application_report_fingerprint
                                == application.report_fingerprint
                                && dispatch.application_commitment == application.commitment
                                && dispatch.declaring_trait == selected_row.declaring_trait
                                && dispatch.requirement == selected_row.requirement
                                && dispatch.realization_machine
                                    == selected_row.realization_machine
                                && dispatch.realization_state == selected_row.realization_state
                        });
                    let realization_callable_identity = selected_by_static_dispatch
                        .then(|| {
                            let machine = owners
                                .iter()
                                .find_map(|(source, terminal)| {
                                    (*source == selected_row.realization_machine)
                                        .then_some(*terminal)
                                })
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance realization is absent from the Terminal closure",
                                ))?;
                            let identity =
                                super::evidence_lowering::checked_evidence_machine_identity(
                                    checked,
                                    selected_row.realization_machine,
                                )?;
                            realization_callables
                                .iter()
                                .any(|callable| {
                                    callable.source_callable_identity == identity
                                        && callable.machine == machine
                                })
                                .then_some(identity)
                                .ok_or(LoweringError::Unsupported(
                                    "static conformance row lost its independent callable registry entry",
                                ))
                        })
                        .transpose()?;
                    Ok(ClosedConformanceRow {
                        declaring_trait_identity: checked
                            .symbols
                            .display_path(selected_row.declaring_trait, "::"),
                        public_requirement_identity:
                            super::evidence_lowering::checked_evidence_requirement_identity(
                                checked,
                                selected_row.declaring_trait,
                                selected_row.requirement,
                            )?,
                        requirement_identity: checked
                            .symbols
                            .display_path(selected_row.requirement, "::"),
                        realization_identity: checked
                            .symbols
                            .display_path(selected_row.realization_state, "::"),
                        realization_callable_identity,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            if selected.lifetime_parameters.len() != application.lifetime_arguments.len() {
                return Err(LoweringError::Unsupported(
                    "closed conformance application has an open lifetime telescope",
                ));
            }
            let mut telescope = selected
                .lifetime_parameters
                .iter()
                .zip(&application.lifetime_arguments)
                .map(|(parameter, argument)| ClosedConformanceParameterBinding {
                    parameter: parameter.as_str().to_owned(),
                    kind: ClosedConformanceParameterKind::Lifetime,
                    argument: argument.clone(),
                })
                .collect::<Vec<_>>();
            let mut type_index = 0usize;
            let mut const_index = 0usize;
            let mut machine_index = 0usize;
            for parameter in checked.conformance_type_parameters(selected) {
                let (kind, argument) = match &parameter.kind {
                    TypeParameterKind::Type => {
                        let argument = application.type_arguments.get(type_index).cloned();
                        type_index += 1;
                        (ClosedConformanceParameterKind::Type, argument)
                    }
                    TypeParameterKind::Const { .. } => {
                        let argument = match application.const_arguments.get(const_index) {
                            Some(ClosedConformanceConstArgument::Evaluated { value, .. }) => Some(
                                language_semantics::const_value::CanonicalConstValue::new(
                                    value.type_name.clone(),
                                    value.encoding.clone(),
                                    "",
                                )
                                .atom(),
                            ),
                            Some(ClosedConformanceConstArgument::CallerBinder { .. }) => {
                                return Err(LoweringError::Unsupported(
                                    "closed conformance application reaches Terminal with an unsubstituted const binder",
                                ));
                            }
                            None => None,
                        };
                        const_index += 1;
                        (ClosedConformanceParameterKind::Const, argument)
                    }
                    TypeParameterKind::Machine { .. } => {
                        let argument = application
                            .machine_arguments
                            .get(machine_index)
                            .map(|argument| checked.symbols.display_path(*argument, "::"));
                        machine_index += 1;
                        (ClosedConformanceParameterKind::Machine, argument)
                    }
                    TypeParameterKind::Proposition { .. } => {
                        return Err(LoweringError::Unsupported(
                            "closed conformance application retains a proposition parameter",
                        ));
                    }
                };
                telescope.push(ClosedConformanceParameterBinding {
                    parameter: parameter.name.as_str().to_owned(),
                    kind,
                    argument: argument.ok_or(LoweringError::Unsupported(
                        "closed conformance application has an open static telescope",
                    ))?,
                });
            }
            if type_index != application.type_arguments.len()
                || const_index != application.const_arguments.len()
                || machine_index != application.machine_arguments.len()
            {
                return Err(LoweringError::Unsupported(
                    "closed conformance application telescope has extra arguments",
                ));
            }
            let mut lowered = ClosedConformanceApplication {
                owner: *owner,
                declaration_identity: checked.symbols.display_path(application.declaration, "::"),
                telescope,
                subject_identity: application.subject_identity.clone(),
                trait_identity: checked
                    .symbols
                    .display_path(application.trait_definition, "::"),
                trait_lifetime_arguments: application.trait_lifetime_arguments.clone(),
                trait_arguments: application.trait_arguments.clone(),
                realization_callables,
                rows,
                report_fingerprint: 0,
                commitment: Default::default(),
            };
            lowered.report_fingerprint =
                closed_conformance_application_report_fingerprint(&lowered);
            lowered.commitment = closed_conformance_application_commitment(&lowered);
            applications.push(lowered);
        }
    }
    sort_applications(&mut applications);
    Ok(applications)
}

fn sort_applications(applications: &mut [ClosedConformanceApplication]) {
    applications.sort_by(|left, right| {
        (
            left.owner,
            &left.declaration_identity,
            left.report_fingerprint,
        )
            .cmp(&(
                right.owner,
                &right.declaration_identity,
                right.report_fingerprint,
            ))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    type Owners = Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)>;

    fn dynamic_callee_fixture() -> (CheckedTrees, TerminalModule, Owners, symbols::SymbolHandle) {
        let source = r#"
            boundary trait Console {
                machine exit_process(value: i32) reaches Console;
            }
            trait Measure { machine measure(&self) -> i32; }
            data Item [copy] { value: i32; }
            Primary: Item satisfies Measure {
                machine measure(&self) -> i32 { transition { _ -> self.value } }
            }
            machine consume<Element, Order: Element satisfies Measure>(value: i32) reaches Console {
                Console::exit_process(value);
            }
            machine identity(value: i32) -> i32 { value }
            data Main { selected: Item; }
            machine Main::main(&mut self) reaches Console {
                let erased: &dyn Measure = &self.selected as &dyn Item::Primary;
                let result: i32 = erased.measure();
                transition result == 0 { true -> good() _ -> bad() }
                state good(&mut self) { consume<Item, Primary>(identity(70i32)); }
                state bad(&mut self) { consume<Item, Primary>(identity(71i32)); }
            }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokenize");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check");
        let selection = crate::machine_dispatch::select_terminal_machine(&checked, "Main::main")
            .expect("dynamic root selection");
        let root = selection.machine;
        let lowered = crate::machine_dispatch::lower_selected_machine(&checked, selection)
            .expect("dynamic root and ordinary callee lower");
        let owners = lowered.exact_sources.expect("exact dynamic source owners");
        (checked, lowered.terminal.semantic_module, owners, root)
    }

    #[test]
    fn dynamic_root_and_specialized_unit_callee_publish_distinct_applications() {
        let (checked, mut module, owners, root) = dynamic_callee_fixture();
        let [root_application] = module.closed_conformance_applications.as_slice() else {
            panic!("one retained dynamic root application");
        };
        let root_application = root_application.clone();
        let callee_source = checked
            .machine_specializations
            .iter()
            .find(|specialization| {
                specialization.instance != root
                    && !specialization.conformance_applications.is_empty()
                    && owners
                        .iter()
                        .any(|(source, _)| *source == specialization.instance)
            })
            .expect("selected generic Unit callee application")
            .instance;
        let callee_owner = owners
            .iter()
            .find_map(|(source, owner)| (*source == callee_source).then_some(*owner))
            .unwrap();
        append_closed_conformance_applications_excluding(&checked, &owners, root, &mut module)
            .expect("append callee application");
        assert_eq!(module.closed_conformance_applications.len(), 2);
        assert!(
            module
                .closed_conformance_applications
                .contains(&root_application)
        );
        let callee_application = module
            .closed_conformance_applications
            .iter()
            .find(|application| application.owner == callee_owner)
            .expect("callee application uses its exact emitted owner");
        assert_eq!(
            callee_application.declaration_identity,
            root_application.declaration_identity
        );
        assert_eq!(callee_application.subject_identity.as_deref(), Some("Item"));
        terminal_verifier::validate_module(&module).expect("root and callee applications verify");
        let once = module.clone();
        append_closed_conformance_applications_excluding(&checked, &owners, root, &mut module)
            .expect("identical retained callee application is reused");
        assert_eq!(module, once);
        let artifact = terminal_production::produce_terminal_artifact(&checked, "Main::main")
            .expect("public production retains both source-owned applications");
        assert_eq!(
            terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
            module
        );
    }

    #[test]
    fn conformance_append_rejects_conflicting_retained_callee_application() {
        let (checked, mut module, owners, root) = dynamic_callee_fixture();
        append_closed_conformance_applications_excluding(&checked, &owners, root, &mut module)
            .expect("append callee application");
        let entry = module.entry;
        let callee = module
            .closed_conformance_applications
            .iter_mut()
            .find(|application| application.owner != entry)
            .expect("callee application");
        callee.subject_identity = Some("different subject".to_owned());
        assert!(matches!(
            append_closed_conformance_applications_excluding(&checked, &owners, root, &mut module),
            Err(LoweringError::Unsupported(
                "retained closed conformance application differs from its source selection"
            ))
        ));
    }

    #[test]
    fn conformance_append_rejects_incomplete_reordered_or_duplicate_owners() {
        let (checked, module, owners, root) = dynamic_callee_fixture();
        for mutation in 0..3 {
            let mut changed = owners.clone();
            match mutation {
                0 => {
                    changed.pop();
                }
                1 => changed.swap(0, 1),
                _ => changed[1].0 = changed[0].0,
            }
            assert!(matches!(
                append_closed_conformance_applications_excluding(
                    &checked,
                    &changed,
                    root,
                    &mut module.clone()
                ),
                Err(LoweringError::Unsupported(
                    "closed conformance application owners do not match the exact terminal closure"
                ))
            ));
        }
    }
}

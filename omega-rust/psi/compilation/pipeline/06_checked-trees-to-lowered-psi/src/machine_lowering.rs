/*
Stage entrance (`lower_machine`, `select_terminal_machine`, `TerminalMachineSelection`) selecting
the checked machine, dispatching to its plan family (`machine_dispatch`), closing provider and
interrupt registrations, retaining custody/evidence, and attaching `TerminalDebugMap`.
*/

use crate::emission::operation_emission::place_declarations::reconcile_leaf_copy_place_declarations;
use crate::retention::access_lifecycles::retain;
use crate::unit::attached_unit::target_families::runtime_families;
pub(crate) mod boundary_applications {

    use checked_trees::{CheckedBoundaryOperatorApplicationUseSite, CheckedTrees};
    use lowered_psi::LoweredPsi;
    use semantic_vocabulary::OperationId;
    use terminal_psi::{
        BoundaryApplication, BoundaryApplicationArgument, BoundaryTypeIdentity,
        TerminalBoundaryApplicationDemand,
    };
    use typed_trees::type_identity::ExactOwnerTypeIdentityRequest;

    pub(crate) mod candidates {

        use checked_trees::{CheckedBoundaryOperatorApplicationDemand, CheckedTrees};
        use terminal_psi::{
            BoundaryApplication, BoundaryApplicationCandidate, BoundaryOperatorRequirement,
            CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
        };

        pub(crate) fn retain<'a>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &'a CheckedTrees,
            sources: &[(source::SourceId, [u8; 32])],
            demand: &CheckedBoundaryOperatorApplicationDemand,
            requirement: &BoundaryOperatorRequirement,
            application: &BoundaryApplication,
            output: &mut Vec<BoundaryApplicationCandidate>,
        ) -> Result<(), &'a str> {
            let nominal = |identities: &mut terminal_psi::TerminalIdentityTableBuilder, symbol| {
                checked
                    .exact_owner_nominal_identity(symbol, sources)
                    .ok_or("boundary candidate has no exact declaration owner")
                    .map(|identity| {
                        NominalDeclarationIdentity::new(
                            identities
                                .intern(&checked.type_identity_text(identity).unwrap_or_default()),
                        )
                    })
            };
            for candidate in checked
                .facts
                .operators
                .boundary_body_candidates
                .span(demand.body_candidates)
                .ok_or("boundary application has an invalid checked body span")?
            {
                let specialization = candidate
                    .specialization
                    .map(|(template, commitment)| {
                        Ok::<_, &str>(CheckedMachineSpecialization {
                            capture: super::super::declarations::capture_identity(
                                identities,
                                checked,
                                candidate.machine,
                            )?,
                            template: nominal(identities, template)?,
                            template_semantic_declaration: super::super::declarations::semantic_identity(
                                identities, checked, template,
                            )
                            .map_err(|_| "boundary specialization lost its template declaration owner")?,
                            template_callable: identities.intern(
                                &checked
                                    .machine(template)
                                    .and_then(|machine| {
                                        checked.typed.machine_callable_identity_text(machine)
                                    })
                                    .ok_or("boundary specialization lost its exact template callable")?,
                            ),
                            commitment,
                        })
                    })
                    .transpose()?;
                let machine = nominal(identities, candidate.machine)?;
                let state = nominal(identities, candidate.entry)?;
                let selection_identity = identities
                    .intern(&checked.callable_identity_text(candidate.selection_identity));
                output.push(BoundaryApplicationCandidate {
                    requirement: requirement.clone(),
                    application: application.clone(),
                    selection_identity,
                    selection_package: checked.symbols.symbol_package_identity(candidate.selection),
                    target: candidate.target.clone(),
                    body: CheckedMachineDeclaration {
                        semantic_declaration: identities.intern(
                            &checked
                                .normalized_hermetic_symbol_identity(candidate.machine)
                                .map_err(
                                    |_| "boundary candidate has no hermetic declaration identity",
                                )?,
                        ),
                        parameter_types: checked
                            .state_parameters(
                                checked
                                    .typed
                                    .state_by_symbol(candidate.entry)
                                    .ok_or("boundary candidate state is absent")?,
                            )
                            .iter()
                            .map(|parameter| {
                                identities.intern(
                                    &checked.package_qualified_type_identity_text(
                                        parameter.type_reference,
                                    ),
                                )
                            })
                            .collect(),
                        callable: identities.intern(
                            &checked
                                .typed
                                .machine_callable_identity_text(
                                    checked
                                        .machine(candidate.machine)
                                        .ok_or("boundary candidate machine is absent")?,
                                )
                                .ok_or("boundary candidate callable identity is absent")?,
                        ),
                        result_type: identities.intern(
                            &checked.typed.package_qualified_type_identity_text(
                                checked
                                    .typed
                                    .state_by_symbol(candidate.entry)
                                    .ok_or("boundary candidate state is absent")?
                                    .return_type,
                            ),
                        ),
                        machine,
                        state,
                        contract: candidate.contract,
                        published_may_suspend: checked
                            .facts
                            .operational
                            .machine_by_symbol(candidate.machine)
                            .ok_or("boundary candidate lost its published operational contract")?
                            .published_may_suspend,
                        published_may_block: checked
                            .facts
                            .operational
                            .machine_by_symbol(candidate.machine)
                            .ok_or("boundary candidate lost its published operational contract")?
                            .published_may_block,
                        specialization,
                    },
                });
            }
            Ok(())
        }
    }

    struct LoweredBoundaryApplicationOccurrence {
        application_index: usize,
        terminal_operation: OperationId,
    }

    pub(crate) fn retain<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &'a CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        lowered: &mut LoweredPsi,
    ) -> Result<(), &'a str> {
        let mut occurrences = Vec::new();
        for comparison in &lowered.selected_comparison_occurrences {
            occurrences.push(application_occurrence(
                checked,
                comparison.application_site,
                comparison.requirement_operator,
                comparison.terminal_operation,
            )?);
        }
        let mut source_calls = lowered
            .source_call_occurrences
            .iter()
            .filter_map(|source| {
                source
                    .terminal_site
                    .operation()
                    .map(|operation| (operation, source))
            })
            .collect::<Vec<_>>();
        source_calls.sort_unstable_by_key(|(operation, _)| *operation);
        for operation in lowered.semantic_module.operations() {
            let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
                continue;
            };
            let declaration = lowered
                .semantic_module
                .boundary_machine(boundary)
                .ok_or("boundary application has no Terminal declaration")?;
            if declaration.kind != terminal_psi::BoundaryRequirementKind::Operator {
                continue;
            }
            let operation_id = operation.id;
            let ordinal = source_calls.partition_point(|(id, _)| *id < operation_id);
            let Some((id, source)) = source_calls.get(ordinal) else {
                return Err("named boundary application has no source-call occurrence");
            };
            if *id != operation_id
                || source_calls
                    .get(ordinal + 1)
                    .is_some_and(|(next, _)| *next == operation_id)
            {
                return Err("named boundary application has no unique source-call occurrence");
            }
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                source.source_site
            else {
                return Err("named boundary application has no exact expression occurrence");
            };
            let mut uses = checked
                .facts
                .operators
                .named_uses
                .iter()
                .filter(|(_, usage)| {
                    usage.expression == expression
                        && usage.selected_operator_symbol == source.source_target
                        && source.matches_checked_origin(checked, usage.origin)
                });
            let (Some((_, usage)), None) = (uses.next(), uses.next()) else {
                return Err("named boundary application has no unique checked operator use");
            };
            occurrences.push(application_occurrence(
                checked,
                CheckedBoundaryOperatorApplicationUseSite::Expression {
                    expression,
                    origin: usage.origin,
                },
                source
                    .source_target
                    .declaration()
                    .ok_or("boundary operator call retained an indirect callable source")?,
                operation_id,
            )?);
        }
        occurrences.sort_unstable_by_key(|occurrence| occurrence.terminal_operation.get());
        if occurrences
            .windows(2)
            .any(|pair| pair[0].terminal_operation == pair[1].terminal_operation)
        {
            return Err("multiple boundary applications name one emitted operation");
        }
        let mut rows = Vec::with_capacity(occurrences.len());
        let mut candidates = Vec::new();
        let mut retained_applications = std::collections::BTreeSet::new();
        for occurrence in &occurrences {
            let application =
                &checked.facts.operators.boundary_applications[occurrence.application_index];
            let requirement = super::declarations::operator_requirement(
                identities,
                checked,
                exact_toolchain_sources,
                application.requirement_symbol,
                application.requirement_identity,
            )?;
            let arguments =
                project_application(identities, checked, exact_toolchain_sources, application)?;
            if retained_applications.insert((requirement.clone(), arguments.clone())) {
                candidates::retain(
                    identities,
                    checked,
                    exact_toolchain_sources,
                    application,
                    &requirement,
                    &arguments,
                    &mut candidates,
                )?;
            }
            rows.push(TerminalBoundaryApplicationDemand::new(
                occurrence.terminal_operation,
                requirement,
                arguments,
            ));
        }
        lowered.semantic_module.boundary_application_demands = rows;
        candidates.sort();
        candidates.dedup();
        lowered.semantic_module.boundary_application_candidates = candidates;
        Ok(())
    }

    fn application_occurrence(
        checked: &CheckedTrees,
        site: CheckedBoundaryOperatorApplicationUseSite,
        requirement: symbols::SymbolHandle,
        terminal_operation: OperationId,
    ) -> Result<LoweredBoundaryApplicationOccurrence, &'static str> {
        let mut applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.site == site && application.requirement_symbol == requirement
            });
        let (Some((application_index, _)), None) = (applications.next(), applications.next())
        else {
            return Err("emitted boundary operation has no unique checked application");
        };
        Ok(LoweredBoundaryApplicationOccurrence {
            application_index,
            terminal_operation,
        })
    }

    fn project_application(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
    ) -> Result<BoundaryApplication, &'static str> {
        if application.arguments.is_empty() {
            return Ok(BoundaryApplication::Empty);
        }
        let mut type_identity = |type_reference| {
            let identity = checked
                .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                    type_reference,
                    binders: &[],
                    substitutions: &[],
                    exact_toolchain_sources,
                })
                .ok_or("boundary application type has no exact package or toolchain owner")?;
            Ok(BoundaryTypeIdentity::new(identities.intern(
                &checked.type_identity_text(identity).unwrap_or_default(),
            )))
        };
        let mut arguments = Vec::with_capacity(application.arguments.len());
        for (ordinal, argument) in application.arguments.iter().enumerate() {
            let expected = u32::try_from(ordinal)
                .map_err(|_| "boundary application exceeds the supported ordinal range")?;
            arguments.push(match argument {
                checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                    binder_owner,
                    binder_ordinal,
                    type_reference,
                    ..
                } if *binder_owner == application.requirement_symbol && *binder_ordinal == expected => {
                    BoundaryApplicationArgument::type_argument(
                        *binder_ordinal,
                        type_identity(*type_reference)?,
                    )
                }
                checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                    binder_owner,
                    binder_ordinal,
                    declared_carrier,
                    value,
                    ..
                } if *binder_owner == application.requirement_symbol && *binder_ordinal == expected => {
                    BoundaryApplicationArgument::const_argument(
                        *binder_ordinal,
                        type_identity(*declared_carrier)?,
                        value.type_name.clone(),
                        value.encoding.clone(),
                    )?
                }
                _ => {
                    return Err(
                        "boundary application does not match its binder owner, category and ordinal",
                    );
                }
            });
        }
        BoundaryApplication::exact(arguments)
    }
}
pub(crate) mod build_providers {

    use crate::shared::lowering_error::LoweringError;
    use checked_trees::CheckedTrees;
    use lowered_psi::{LoweredBuildProviderRequirement, LoweredSourceCallOccurrence};

    pub(crate) fn retain(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        boundaries: &[(
            symbols::SymbolHandle,
            semantic_vocabulary::BoundaryMachineId,
        )],
        calls: &[LoweredSourceCallOccurrence],
    ) -> Result<Vec<LoweredBuildProviderRequirement>, LoweringError> {
        let mut requirements: Vec<LoweredBuildProviderRequirement> = Vec::new();
        for occurrence in calls {
            let Some(site) = occurrence.origin.statement() else {
                continue;
            };
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                occurrence.source_site
            else {
                continue;
            };
            let checked_trees::CheckedExpressionSource::ProviderReadiness {
                selection,
                requirement,
            } = checked.facts.executable_bindings.expression(expression)
            else {
                continue;
            };
            if requirements.iter().any(|requirement| {
                requirement.selection == selection && requirement.source_state == site.state
            }) {
                continue;
            }
            let owner =
                checked
                    .machine_holding_state(site.state)
                    .ok_or(LoweringError::Unsupported(
                        "Build readiness lost its source owner",
                    ))?;
            let scope = checked
                .build_provider_requirement_scope(requirement, owner.symbol)
                .map_err(LoweringError::Unsupported)?;
            let application = scope
                .portable_application_identity(checked)
                .and_then(|identity| checked.type_identity_text(identity))
                .ok_or(LoweringError::Unsupported(
                    "Build readiness requirement has no closed portable application identity",
                ))?;
            let declaration = super::declarations::nominal(
                identities,
                checked,
                sources,
                scope.declaration.symbol,
            )
            .map_err(LoweringError::Unsupported)?;
            let mut slots = Vec::new();
            for (source, boundary) in boundaries {
                let selected =
                    if let Some(application) = checked.closed_requirement_application(*source) {
                        scope
                            .declaring_scope(checked, application.declaring_trait)
                            .and_then(|declaring| declaring.application_identity(checked))
                            == Some(application.trait_application_identity)
                    } else {
                        checked
                            .traits()
                            .iter()
                            .find(|declaration| {
                                checked
                                    .trait_machine_signatures(declaration)
                                    .iter()
                                    .any(|signature| signature.symbol == *source)
                            })
                            .is_some_and(|declaration| {
                                checked.trait_type_parameters(declaration).is_empty()
                                    && declaration.lifetime_parameters.is_empty()
                                    && scope.declaring_scope(checked, declaration.symbol).is_some()
                            })
                    };
                if selected {
                    slots.push(*boundary);
                }
            }
            requirements.push(LoweredBuildProviderRequirement {
                selection,
                source_state: site.state,
                declaration,
                application: identities.intern(&application),
                boundaries: slots,
            });
        }
        Ok(requirements)
    }
}
pub(crate) mod callback_applications {

    use checked_trees::CheckedTrees;
    use checked_trees::NominalMachineUseSite;
    use lowered_psi::LoweredPsi;
    use symbols::SymbolHandle;
    use terminal_psi::TerminalCallbackApplication;

    pub(crate) fn retain(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        lowered: &mut LoweredPsi,
    ) -> Result<(), String> {
        let mut requested = checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .filter(|usage| {
                usage.callback_entry.is_some()
                    || checked
                        .typed
                        .trait_definition_by_symbol(usage.satisfaction_trait)
                        .is_some_and(|definition| definition.is_boundary)
            })
            .map(|usage| {
                (
                    source_key(usage.site, usage.registration_operation),
                    usage,
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        if requested.is_empty() {
            lowered.semantic_module.callback_applications.clear();
            return Ok(());
        }
        let mut boundary_operations = lowered
            .semantic_module
            .operations()
            .filter(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { .. }
                )
            })
            .map(|operation| operation.id)
            .collect::<Vec<_>>();
        boundary_operations.sort_unstable();
        requested.sort_unstable_by_key(|(key, _, _)| *key);
        for occurrence in &lowered.source_call_occurrences {
            let (Some(site), Some(operation)) =
                (occurrence.source_site, occurrence.terminal_site.operation())
            else {
                continue;
            };
            if boundary_operations.binary_search(&operation).is_err() {
                continue;
            }
            let source_target = occurrence
                .source_target
                .declaration()
                .ok_or("boundary registration retained an indirect callable source")?;
            let registration = authored_registration(checked, site).unwrap_or_else(|| {
                checked
                    .closed_requirement_application(source_target)
                    .map_or(source_target, |application| application.template)
            });
            let key = source_key(site, registration);
            let first = requested.partition_point(|(candidate, _, _)| *candidate < key);
            for (_, _, registrars) in requested[first..]
                .iter_mut()
                .take_while(|(candidate, _, _)| *candidate == key)
            {
                registrars.push(operation);
            }
        }
        // One checked registration can be emitted more than once: nested machines
        // that call each other in a cycle are emitted whole for every member an
        // outside caller enters, so a registering call inside the cycle becomes
        // one boundary operation per copy. Every copy runs the registration and
        // needs its own row, because a row names exactly one registrar. The first
        // copy keeps the checked occurrence number, so a program without copies
        // publishes the numbers it always did; later copies take fresh numbers
        // above every checked one, including the installed-interrupt callbacks
        // that are numbered after the ordinary uses.
        let checked_occurrences = checked.facts.nominal_machine_uses.uses.len()
            + checked
                .facts
                .installed_interrupt_registrations
                .iter()
                .map(|registration| registration.callbacks.uses.len())
                .sum::<usize>();
        let mut next_occurrence = u32::try_from(checked_occurrences)
            .map_err(|_| "callback application count exceeds u32".to_owned())?;
        requested.sort_unstable_by_key(|(_, usage, _)| usage.application_index);
        let mut applications = Vec::with_capacity(requested.len());
        for (_, usage, mut registrars) in requested {
            registrars.sort_unstable();
            registrars.dedup();
            for (copy, registrar) in registrars.into_iter().enumerate() {
                let mut application =
                    lower_application(identities, checked, sources, lowered, usage, registrar)?;
                if copy != 0 {
                    application.application_index = next_occurrence;
                    next_occurrence = next_occurrence
                        .checked_add(1)
                        .ok_or("callback application count exceeds u32")?;
                }
                applications.push(application);
            }
        }
        applications.sort_unstable_by_key(|application| application.application_index);
        lowered.semantic_module.callback_applications = applications;
        Ok(())
    }

    pub(crate) fn lower_application(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        lowered: &LoweredPsi,
        usage: &checked_trees::CheckedNominalMachineUse,
        registrar: semantic_vocabulary::OperationId,
    ) -> Result<TerminalCallbackApplication, String> {
        let mut boundaries = lowered
            .source_boundary_declarations
            .iter()
            .filter(|(source, _)| *source == usage.satisfaction_requirement);
        let boundary = match (boundaries.next(), boundaries.next()) {
            (Some((_, boundary)), None) => Some(*boundary),
            (None, None) if usage.callback_entry.is_none() => None,
            _ => {
                return Err(
                    "callback requirement has no unique exact emitted inbound declaration".into(),
                );
            }
        };
        Ok(TerminalCallbackApplication {
            boundary,
            application_index: usage.application_index,
            registrar,
            static_machine_ordinal: usage.static_machine_ordinal,
            requirement: usage.canonical_requirement_overload.clone(),
            calling_owner_application: usage.receiving_application.clone()?,
            published_contract: usage
                .published_requirement_envelope
                .contract_commitment
                .as_bytes(),
            entry_admitted: usage.callback_entry.is_some(),
            body: super::declarations::identity(
                identities,
                checked,
                sources,
                usage.selected_machine,
            )?,
        })
    }

    fn source_key(site: NominalMachineUseSite, target: SymbolHandle) -> (u8, u32, u32, u32, u32) {
        let (kind, arena_index, generation) = match site {
            NominalMachineUseSite::Statement(handle) => {
                (0, handle.arena_index(), handle.generation())
            }
            NominalMachineUseSite::Expression(handle) => {
                (1, handle.arena_index(), handle.generation())
            }
        };
        (
            kind,
            arena_index,
            generation,
            target.arena_index(),
            target.generation(),
        )
    }

    fn authored_registration(
        checked: &CheckedTrees,
        site: NominalMachineUseSite,
    ) -> Option<SymbolHandle> {
        let application = match site {
            NominalMachineUseSite::Expression(handle) => {
                checked
                    .expression_table
                    .call_node(handle)?
                    .authored_machine_application
            }
            NominalMachineUseSite::Statement(handle) => {
                checked
                    .statement_table
                    .statement(handle)
                    .as_call()?
                    .authored_machine_application
            }
        };
        application.is_valid().then(|| {
            checked
                .expression_table
                .static_machine_application(application)
                .registration_operation
        })
    }
}
pub(crate) mod debug_map {

    use checked_trees::CheckedTerminalMachineDebugPlan;
    use terminal_codec::{
        DebugFileId, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
        TerminalDebugMap, source_digest, terminal_psi_identity, validate_debug_map,
    };
    use terminal_psi::TerminalModule;

    use crate::shared::lowering_error::LoweringError;

    pub(crate) fn build_debug_map(
        plan: &CheckedTerminalMachineDebugPlan,
        module: &TerminalModule,
        live_states: Option<&[bool]>,
    ) -> Result<TerminalDebugMap, LoweringError> {
        let terminal_machine = match module.entry {
            Some(terminal_psi::TargetMachineCallable::Machine(entry)) => module.machine(entry),
            Some(terminal_psi::TargetMachineCallable::Boundary(_)) | None => None,
        }
        .ok_or(LoweringError::Unsupported(
            "the selected entry machine is absent from its terminal call closure",
        ))?;
        let result = terminal_machine
            .result
            .scalar()
            .ok_or(LoweringError::Unsupported(
                "scalar debug source entry has no scalar result",
            ))?;
        let filtered_states;
        let source_states = if let Some(live_states) = live_states {
            filtered_states = plan
                .states
                .iter()
                .zip(live_states)
                .filter_map(|(state, live)| live.then_some(state))
                .collect::<Vec<_>>();
            &filtered_states[..]
        } else {
            filtered_states = plan.states.iter().collect::<Vec<_>>();
            &filtered_states[..]
        };
        let has_source_file = |span: source::SourceSpan| {
            plan.source_files
                .iter()
                .any(|file| file.source_id == span.source_id)
        };
        let mut subjects = Vec::<(DebugSubject, source::SourceSpan)>::new();
        let mut push = |subject, span| {
            if let Some(span) = span {
                subjects.push((subject, span));
            }
        };

        push(
            DebugSubject::Machine(terminal_machine.id),
            plan.machine_span,
        );
        let contract_span = plan.contract_span;
        push(
            DebugSubject::Contract(terminal_machine.contract.id),
            contract_span,
        );
        for clause in &terminal_machine.contract.ensures {
            push(DebugSubject::Obligation(clause.obligation), contract_span);
        }

        for (index, block) in terminal_machine.blocks.iter().enumerate() {
            let source_state = source_states
                .get(index)
                .or_else(|| source_states.last())
                .expect("an accepted source machine has at least one state");
            push(DebugSubject::Block(block.id), source_state.state_span);
            for (edge_index, edge) in block.terminator.edges().enumerate() {
                let transition_span = source_state
                    .transition_spans
                    .get(edge_index)
                    .or_else(|| {
                        (source_state.transition_spans.len() == 1)
                            .then(|| &source_state.transition_spans[0])
                    })
                    .copied()
                    .filter(|span| *span != source::SourceSpan::default())
                    .filter(|span| has_source_file(*span));
                push(
                    DebugSubject::Edge(edge),
                    transition_span.or(source_state.state_span),
                );
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let source_span = source_state
                    .operation_spans
                    .get(operation_index)
                    .copied()
                    .filter(|span| *span != source::SourceSpan::default())
                    .filter(|span| has_source_file(*span));
                let source_span = source_span.or(source_state.state_span);
                push(DebugSubject::Operation(operation.id), source_span);
                if let Some(result) = operation.result.scalar() {
                    push(DebugSubject::Value(result.id), source_span);
                }
            }
            for (parameter_index, parameter) in block.parameters.iter().enumerate() {
                if let Some(source_span) = source_state
                    .parameter_spans
                    .get(parameter_index)
                    .copied()
                    .flatten()
                {
                    push(DebugSubject::Value(parameter.id), Some(source_span));
                }
            }
        }

        if let Some(entry_state) = source_states.first() {
            for (parameter_index, parameter) in terminal_machine.parameters.iter().enumerate() {
                if let Some(source_span) = entry_state
                    .parameter_spans
                    .get(parameter_index)
                    .copied()
                    .flatten()
                {
                    push(DebugSubject::Value(parameter.id), Some(source_span));
                }
            }
        }
        push(DebugSubject::Value(result.id), plan.machine_span);

        subjects.sort_by_key(|(subject, _)| *subject);
        subjects.dedup_by_key(|(subject, _)| *subject);
        let mut source_ids = subjects
            .iter()
            .map(|(_, span)| span.source_id.0)
            .collect::<Vec<_>>();
        source_ids.sort_unstable();
        source_ids.dedup();

        let mut files = Vec::with_capacity(source_ids.len());
        for (index, source_id) in source_ids.iter().copied().enumerate() {
            let source_file = plan
                .source_files
                .iter()
                .find(|file| file.source_id == source::SourceId(source_id))
                .ok_or(LoweringError::MissingDebugSourceFile(source_id))?;
            let id = DebugFileId::new(
                u32::try_from(index)
                    .map_err(|_| LoweringError::DebugSourceFileCountOverflow)?
                    .checked_add(1)
                    .ok_or(LoweringError::DebugSourceFileCountOverflow)?,
            )
            .expect("one-based debug file identity is nonzero");
            files.push(DebugSourceFile {
                id,
                origin: match source_file.origin {
                    source::SourceOrigin::User => DebugSourceOrigin::User,
                    source::SourceOrigin::Toolchain | source::SourceOrigin::ToolchainContract => {
                        DebugSourceOrigin::Toolchain
                    }
                },
                byte_len: u64::try_from(source_file.source.len())
                    .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                digest: source_digest(source_file.source.as_bytes()),
                path: source_file.path.to_string_lossy().into_owned(),
            });
        }

        let sites = subjects
            .into_iter()
            .map(|(subject, span)| {
                let file_index = source_ids
                    .binary_search(&span.source_id.0)
                    .expect("source identity was collected above");
                let file = DebugFileId::new(
                    u32::try_from(file_index)
                        .expect("validated debug file count fits u32")
                        .checked_add(1)
                        .expect("one-based debug file identity fits u32"),
                )
                .expect("one-based debug file identity is nonzero");
                Ok(DebugSite {
                    subject,
                    span: DebugSourceSpan {
                        file,
                        start: u64::try_from(span.span.start)
                            .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                        end: u64::try_from(span.span.end)
                            .map_err(|_| LoweringError::DebugSourceLengthOverflow)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let debug_map = TerminalDebugMap {
            semantic: terminal_psi_identity(module).map_err(LoweringError::DebugSemanticCodec)?,
            files,
            sites,
        };
        validate_debug_map(module, &debug_map).map_err(LoweringError::InvalidDebugMap)?;
        Ok(debug_map)
    }
}
pub(crate) mod declarations {

    use checked_trees::CheckedTrees;
    use symbols::SymbolHandle;
    use terminal_psi::{
        CheckedMachineDeclaration, CheckedMachineSpecialization, NominalDeclarationIdentity,
    };

    pub(crate) fn nominal(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        symbol: SymbolHandle,
    ) -> Result<NominalDeclarationIdentity, &'static str> {
        if let Some(identity) = invocation_local(identities, checked, symbol)? {
            return Ok(NominalDeclarationIdentity::new(identity));
        }
        let identity = checked
            .exact_owner_nominal_identity(symbol, sources)
            .ok_or("checked declaration has no exact package or toolchain owner")?;
        let text = checked
            .type_identity_text(identity)
            .ok_or("checked declaration lost its normalized identity")?;
        Ok(NominalDeclarationIdentity::new(identities.intern(&text)))
    }

    fn invocation_local(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        symbol: SymbolHandle,
    ) -> Result<Option<terminal_psi::TerminalIdentityIndex>, &'static str> {
        if !symbol.is_valid()
            || !identities.is_invocation_scoped()
            || checked.symbols.symbol_package_identity(symbol).is_some()
            || checked.symbols.symbol_source_origin(symbol) != Some(source::SourceOrigin::User)
        {
            return Ok(None);
        }
        let coordinate = format!(
            "invocation-declaration({}:{})",
            symbol.arena_index(),
            symbol.generation()
        );
        identities.intern_invocation_local(&coordinate).map(Some)
    }

    pub(crate) fn semantic_identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        symbol: SymbolHandle,
    ) -> Result<terminal_psi::TerminalIdentityIndex, String> {
        if let Some(identity) = invocation_local(identities, checked, symbol)? {
            return Ok(identity);
        }
        Ok(identities.intern(&checked.normalized_hermetic_symbol_identity(symbol)?))
    }

    pub(crate) fn identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        machine: SymbolHandle,
    ) -> Result<CheckedMachineDeclaration, String> {
        let declaration = checked
            .facts
            .declaration_descriptions
            .for_machine(machine)?;
        let parameters = checked
            .facts
            .declaration_descriptions
            .parameters
            .span(declaration.parameters)
            .ok_or("checked declaration lost its retained parameter identities")?;
        for parameter in parameters {
            type_identity(identities, checked, parameter.normalized)?;
        }
        let semantic_declaration = semantic_identity(identities, checked, machine)
            .map_err(|reason| format!("checked machine identity: {reason}"))?;
        let parameter_types = parameters
            .iter()
            .map(|parameter| type_identity(identities, checked, parameter.qualified))
            .collect::<Result<Vec<_>, _>>()?;
        let callable = identities.intern(&checked.callable_identity_text(declaration.callable));
        let result_type = type_identity(identities, checked, declaration.result)?;
        let specialization = declaration
            .specialization
            .as_ref()
            .map(|specialization| {
                Ok::<_, String>(CheckedMachineSpecialization {
                    capture: capture_identity(identities, checked, machine)?,
                    template: nominal(identities, checked, sources, specialization.template)?,
                    template_semantic_declaration: semantic_identity(
                        identities,
                        checked,
                        specialization.template,
                    )?,
                    template_callable: identities
                        .intern(&checked.callable_identity_text(specialization.callable)),
                    commitment: specialization.commitment,
                })
            })
            .transpose()?;
        Ok(CheckedMachineDeclaration {
            machine: nominal(identities, checked, sources, machine)?,
            state: nominal(identities, checked, sources, declaration.entry)?,
            semantic_declaration,
            parameter_types,
            callable,
            result_type,
            contract: declaration.contract.as_bytes(),
            published_may_suspend: checked
                .facts
                .operational
                .machine_by_symbol(machine)
                .ok_or("checked declaration lost its published operational contract")?
                .published_may_suspend,
            published_may_block: checked
                .facts
                .operational
                .machine_by_symbol(machine)
                .ok_or("checked declaration lost its published operational contract")?
                .published_may_block,
            specialization,
        })
    }

    fn type_identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        identity: typed_trees::type_identity::TypeIdentityId,
    ) -> Result<terminal_psi::TerminalIdentityIndex, &'static str> {
        let text = checked
            .type_identity_text(identity)
            .ok_or("checked declaration lost its retained type identity")?;
        Ok(identities.intern(&text))
    }

    pub(crate) fn capture_identity<'a>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &'a CheckedTrees,
        machine: SymbolHandle,
    ) -> Result<Option<terminal_psi::CheckedMachineCapture>, &'a str> {
        checked
            .facts
            .declaration_descriptions
            .for_machine(machine)?
            .specialization
            .as_ref()
            .and_then(|specialization| specialization.capture.as_ref())
            .map(|capture| {
                Ok(terminal_psi::CheckedMachineCapture {
                    template_identity: type_identity(
                        identities,
                        checked,
                        capture.template_identity,
                    )?,
                    template_commitment: capture.template_contract,
                    specialization_commitment: capture.specialization,
                })
            })
            .transpose()
    }

    pub(crate) fn boundary_identity(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        target: SymbolHandle,
    ) -> Result<Option<CheckedMachineDeclaration>, String> {
        if checked
            .facts
            .declaration_descriptions
            .description(target)
            .is_some()
        {
            return identity(identities, checked, sources, target).map(Some);
        }
        let program = &checked.typed;
        let mut requirements = program.traits().iter().flat_map(|owner| {
            program
                .executable_trait_machine_instances(owner)
                .filter_map(move |signature| {
                    (signature.symbol == target).then_some((owner, signature))
                })
        });
        let Some((owner, signature)) = requirements.next() else {
            return Ok(None);
        };
        if requirements.next().is_some() {
            return Err("boundary requirement has multiple checked declaration owners".into());
        }
        let capsule = checked
            .facts
            .contract_plans
            .crash_capsule(owner.symbol, signature.symbol)
            .ok_or("boundary requirement lost its complete checked contract capsule")?;
        let contract = capsule.target_contract_commitment();
        if contract.is_zero() {
            return Err("boundary requirement has an empty checked contract commitment".into());
        }
        let parameter_types = program
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| {
                identities
                    .intern(&program.package_qualified_type_identity_text(parameter.type_reference))
            })
            .collect();
        Ok(Some(CheckedMachineDeclaration {
            machine: nominal(identities, checked, sources, owner.symbol)?,
            state: nominal(identities, checked, sources, signature.symbol)?,
            semantic_declaration: semantic_identity(identities, checked, signature.symbol)
                .map_err(|reason| format!("boundary requirement identity: {reason}"))?,
            parameter_types,
            callable: identities
                .intern(&program.trait_requirement_callable_identity_text(owner, signature)),
            result_type: identities
                .intern(&program.package_qualified_type_identity_text(signature.return_type)),
            contract: contract.as_bytes(),
            published_may_suspend: validation::signature_operational(checked, signature)
                .may_suspend,
            published_may_block: validation::signature_operational(checked, signature).may_block,
            specialization: None,
        }))
    }

    pub(crate) fn operator_requirement(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        symbol: SymbolHandle,
        callable: typed_trees::type_identity::CallableIdentityId,
    ) -> Result<terminal_psi::BoundaryOperatorRequirement, &'static str> {
        if !callable.is_valid() {
            return Err("boundary operator has no complete callable identity");
        }
        terminal_psi::BoundaryOperatorRequirement::new(
            nominal(identities, checked, sources, symbol)?,
            checked.typed.callable_identity_text(callable),
            checked.symbols.symbol_package_identity(symbol),
        )
    }

    pub(crate) fn retain(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        owners: &[(SymbolHandle, semantic_vocabulary::MachineId)],
        module: &mut terminal_psi::TerminalModule,
    ) -> Result<(), String> {
        let mut machines = module.machines.iter_mut();
        for (source, machine) in owners {
            let declaration = identity(identities, checked, sources, *source)?;
            let emitted = machines
                .find(|row| row.id == *machine)
                .ok_or("checked declaration lost its emitted machine")?;
            emitted.declaration = Some(declaration);
        }
        Ok(())
    }
}
pub(crate) mod float_meanings {

    use checked_trees::CheckedTrees;
    use lowered_psi::LoweredPsi;
    use semantic_vocabulary::MachineId;
    use symbols::SymbolHandle;

    use crate::proofs::float_meaning_projection::{
        lower_float_meaning_equality, lower_float_meaning_projection,
        rejoin_float_semantic_applications, resolve_direct_float_source_binding,
    };
    use crate::shared::lowering_error::LoweringError;

    pub(crate) fn retain_float_meanings(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        projection_sources: &[(SymbolHandle, MachineId)],
        lowered: &mut LoweredPsi,
    ) -> Result<(), LoweringError> {
        lowered.semantic_module.float_meaning_projections = checked
            .facts
            .proof
            .float_meaning_projections
            .iter()
            .cloned()
            .map(|projection| {
                let direct_source = resolve_direct_float_source_binding(
                    identities,
                    checked,
                    projection_sources,
                    &lowered.semantic_module.machines,
                    &lowered.semantic_module.structural_types,
                    &lowered.source_call_occurrences,
                    &lowered.state_blocks,
                    projection.clone(),
                )?;
                lower_float_meaning_projection(projection, direct_source)
                    .map_err(LoweringError::InvalidFloatMeaningProjection)
            })
            .collect::<Result<Vec<_>, _>>()?;
        rejoin_float_semantic_applications(
            &checked.facts.proof.float_semantic_applications,
            &mut lowered.semantic_module.float_meaning_projections,
        )
        .map_err(LoweringError::InvalidFloatMeaningProjection)?;
        let mut transitional_sources = Vec::<u32>::new();
        for projection in &mut lowered.semantic_module.float_meaning_projections {
            if let terminal_psi::FloatMeaningSource::TransitionalInput(input) =
                &mut projection.source
            {
                let next = match transitional_sources.iter().position(|id| *id == input.id.0) {
                    Some(index) => index,
                    None => {
                        transitional_sources.push(input.id.0);
                        transitional_sources.len() - 1
                    }
                };
                input.id =
                    terminal_psi::FloatProjectionInputId(u32::try_from(next).map_err(|_| {
                        LoweringError::Unsupported(
                            "float-meaning transitional sources exceed their dense identity space",
                        )
                    })?);
            }
        }
        lowered.semantic_module.float_meaning_equalities = checked
            .facts
            .proof
            .float_meaning_equalities
            .iter()
            .copied()
            .map(lower_float_meaning_equality)
            .collect();
        Ok(())
    }
}
pub(crate) mod guarded_exits {

    use checked_trees::CheckedTrees;
    use semantic_vocabulary::MachineId;
    use symbols::SymbolHandle;
    use terminal_psi::{OperationKind, TerminalMachine, TerminalModule, Terminator};

    use crate::shared::lowering_error::{LoweringError, unsupported};

    /// Whether every structural return of a machine with outcome-specific
    /// guarantees has a decided result case: either an exact payloadless
    /// constructor established in the returning block, or the result of a
    /// structural terminal call. Under `nested-evidence-output-establishment`
    /// a terminal call's callee decides the case and Checked Psi has already
    /// discharged the caller's guarantees from the callee's, so lowering only
    /// confirms the return shape it can carry.
    pub(crate) fn returns_have_decided_cases(machine: &TerminalMachine) -> bool {
        let Some(result) = machine.result.structural() else {
            return false;
        };
        let mut returns = 0usize;
        for block in &machine.blocks {
            let Terminator::ReturnStructural { source, .. } = block.terminator else {
                continue;
            };
            returns += 1;
            let Some(operation) = block.operations.iter().find(|operation| {
                operation
                    .result
                    .structural()
                    .is_some_and(|result| result.place == source)
            }) else {
                return false;
            };
            let decided = match &operation.kind {
                OperationKind::EstablishScalarCase { fields, .. } => fields.is_empty(),
                OperationKind::CallStructural { .. }
                | OperationKind::CallStructuralWithScalarArguments { .. } => true,
                _ => false,
            };
            if !decided
                || operation
                    .result
                    .structural()
                    .is_none_or(|operation| operation.structural_type != result.structural_type)
            {
                return false;
            }
        }
        returns > 0
    }

    pub(crate) fn reject_unguarded_outcome_guarantees(
        checked: &CheckedTrees,
        source_machine_ids: &[(SymbolHandle, MachineId)],
        module: &TerminalModule,
    ) -> Result<(), LoweringError> {
        let guarded = |machine: SymbolHandle| {
            source_machine_ids
                .iter()
                .find(|(source, _)| *source == machine)
                .and_then(|(_, terminal)| module.machine(*terminal))
                .is_some_and(returns_have_decided_cases)
        };
        if checked
            .facts
            .proof
            .outcome_specific_guarantees
            .iter()
            .any(|(_, guarantee)| {
                source_machine_ids
                    .iter()
                    .any(|(source, _)| *source == guarantee.machine_symbol)
                    && !guarded(guarantee.machine_symbol)
            })
        {
            return unsupported(
                "outcome-specific guarantees require guarded exit and caller-arm lowering",
            );
        }
        Ok(())
    }
}
pub(crate) mod installed_interrupts {

    use checked_trees::CheckedTrees;
    use lowered_psi::LoweredPsi;
    use terminal_psi::{
        CheckedBoundaryExecution, OperationKind, TerminalInstalledInterruptRegistration,
    };

    pub(crate) fn retain(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
        lowered: &mut LoweredPsi,
    ) -> Result<(), String> {
        let mut registrations = Vec::new();
        for request in &checked.facts.installed_interrupt_registrations {
            let mut occurrences = lowered
                .source_call_occurrences
                .iter()
                .filter_map(|occurrence| {
                    if occurrence.source_site != Some(request.site) {
                        return None;
                    }
                    let operation = occurrence.terminal_site.operation()?;
                    lowered.semantic_module.operations().find_map(|row| {
                        if row.id != operation {
                            return None;
                        }
                        let OperationKind::BoundaryCall { boundary, .. } = row.kind else {
                            return None;
                        };
                        let declaration = lowered
                            .semantic_module
                            .boundary_machine(boundary)?
                            .declaration
                            .as_ref()?;
                        matches!(
                            declaration.execution,
                            CheckedBoundaryExecution::StaticInterruptInstall
                        )
                        .then_some((operation, boundary))
                    })
                });
            let Some((operation, installer)) = occurrences.next() else {
                continue;
            };
            if occurrences.next().is_some() {
                return Err(
                    "interrupt installation request names multiple emitted operations".into(),
                );
            }
            let mut registrar_boundaries = lowered
                .source_boundary_declarations
                .iter()
                .filter(|(source, _)| *source == request.target);
            let registrar_boundary =
                match (registrar_boundaries.next(), registrar_boundaries.next()) {
                    (Some((_, boundary)), None) => *boundary,
                    _ => {
                        return Err(
                            "interrupt registrar has no unique exact emitted boundary declaration"
                                .into(),
                        );
                    }
                };
            let closed = checked
                .closed_requirement_application(request.target)
                .ok_or("interrupt installation lost its checked registrar application")?;
            let owner = checked
                .trait_definition_by_symbol(closed.declaring_trait)
                .ok_or("interrupt installation registrar lost its declaring trait")?;
            let registrar_requirement =
                checked.trait_requirement_callable_identity_text(owner, closed.signature);
            let callbacks = request
                .callbacks
                .uses
                .iter()
                .map(|usage| {
                    super::callback_applications::lower_application(
                        identities, checked, sources, lowered, usage, operation,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            registrations.push(TerminalInstalledInterruptRegistration {
                operation,
                installer,
                service: emitted_control_service(
                    checked,
                    identities,
                    &lowered.semantic_module,
                    request.control_service,
                )?,
                registrar_boundary,
                registrar: super::declarations::nominal(
                    identities,
                    checked,
                    sources,
                    closed.template,
                )
                .map_err(str::to_owned)?,
                registrar_owner: super::declarations::nominal(
                    identities,
                    checked,
                    sources,
                    request.registrar_owner,
                )
                .map_err(str::to_owned)?,
                registrar_application: request.registrar_application_identity.clone(),
                registrar_declaring_application: request.registrar_declaring_application.clone(),
                registrar_arguments: request.registrar_arguments.clone(),
                registrar_lifetimes: request.registrar_lifetimes.clone(),
                registrar_requirement,
                slot_declaration: super::declarations::nominal(
                    identities,
                    checked,
                    sources,
                    request.slot_application.declaration,
                )
                .map_err(str::to_owned)?,
                slot_commitment: request.slot_application.commitment.as_bytes(),
                slot_encoding: request.slot_application.identity_encoding.to_vec(),
                callbacks,
            });
        }
        registrations.sort_unstable_by_key(|row| row.operation);
        if registrations
            .windows(2)
            .any(|rows| rows[0].operation == rows[1].operation)
        {
            return Err("interrupt installation operation has competing checked requests".into());
        }
        if std::env::var_os("OMEGA_TRACE_NATIVE_BOUNDARY").is_some() {
            for registration in &registrations {
                let helpers = lowered
                    .semantic_module
                    .callback_applications
                    .iter()
                    .filter(|application| application.registrar == registration.operation)
                    .map(|application| {
                        (
                            application.static_machine_ordinal,
                            application.entry_admitted,
                            application.body.callable,
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "published installed operation {:?} helpers: {helpers:?}",
                    registration.operation
                );
            }
        }
        lowered.semantic_module.installed_interrupt_registrations = registrations;
        Ok(())
    }

    fn emitted_control_service(
        checked: &CheckedTrees,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        module: &terminal_psi::TerminalModule,
        selected: language_semantics::ServiceReachId,
    ) -> Result<semantic_vocabulary::ServiceId, String> {
        let definition = checked
            .facts
            .service_reaches
            .services
            .definition(selected)
            .ok_or("interrupt installation lost its checked control service")?;
        let identity = crate::shared::terminal_identities::service_declaration_identity(
            identities,
            checked,
            definition.symbol,
        )
        .map_err(|error| format!("interrupt installer service identity: {error:?}"))?;
        let mut emitted = module
            .services
            .iter()
            .filter(|service| service.identity == identity);
        match (emitted.next(), emitted.next()) {
            (Some(service), None) => Ok(service.id),
            _ => {
                Err("interrupt installer control service has no unique emitted declaration".into())
            }
        }
    }
}
pub(crate) mod machine_dispatch {

    use crate::shared::checked_facts::flow_facts;
    use checked_trees::{
        CheckedTerminalMachineSelection, CheckedTerminalSignatureEligibility, CheckedTrees,
        CheckedUnitPlan,
    };

    use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
    use crate::shared::lowering_error::LoweringError;
    use crate::shared::producer_result::{
        DebugPublication, LoweredSelectedMachine, LoweringCompletion,
    };
    use crate::unit::attached_unit::lower_unit_effect_closure;
    use crate::unit::lower_unit_plan_machine;

    #[derive(Debug, Clone, Copy)]
    pub enum TerminalMachineSelection<'a> {
        Declarations,
        Name(&'a str),
        Symbol(symbols::SymbolHandle),
    }

    pub fn select_terminal_machine<'checked>(
        checked: &'checked CheckedTrees,
        machine: TerminalMachineSelection<'_>,
    ) -> Result<&'checked CheckedTerminalMachineSelection, LoweringError> {
        let mut machines = checked.facts.flow.terminal_machines.machines.iter();
        match machine {
            TerminalMachineSelection::Declarations => Err(LoweringError::Unsupported(
                "declaration production has no selected entry",
            )),
            TerminalMachineSelection::Name(machine_name) => {
                let mut matches = machines.filter(|machine| machine.name == machine_name);
                let selection = matches
                    .next()
                    .ok_or_else(|| LoweringError::MachineNotFound(machine_name.to_owned()))?;
                if matches.next().is_some() {
                    return Err(LoweringError::AmbiguousMachineName(machine_name.to_owned()));
                }
                Ok(selection)
            }
            TerminalMachineSelection::Symbol(machine) => machines
                .find(|selection| selection.machine == machine)
                .ok_or_else(|| {
                    LoweringError::MachineNotFound(
                        checked.typed.symbols.display_path(machine, "::"),
                    )
                }),
        }
    }

    use crate::shared::lowering_error::unsupported;

    pub(crate) fn lower_selected_machine(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        selection: &CheckedTerminalMachineSelection,
        program_roots: crate::TerminalProgramRoots<'_>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<LoweredSelectedMachine, LoweringError> {
        if checked.typed.target_machine_families.iter().any(|family| {
            family.canonical == selection.machine
                && !family
                    .members
                    .iter()
                    .any(|member| member.machine == family.canonical)
        }) {
            return Ok(LoweredSelectedMachine::source_mapped(
                crate::unit::attached_unit::lower_unit_closure(
                    identities,
                    checked,
                    &crate::unit::attached_unit::UnitClosureRequest::unit(
                        checked,
                        selection.machine,
                        &[],
                        program_roots,
                        exact_toolchain_sources,
                    ),
                )?,
                LoweringCompletion {
                    debug: DebugPublication::Omit,
                },
            ));
        }
        if let Some(plan) = select_unit_plan(checked, selection)? {
            return lower_unit_plan_machine(
                identities,
                checked,
                plan,
                program_roots,
                exact_toolchain_sources,
            );
        }
        if selection.signature == CheckedTerminalSignatureEligibility::Unsupported {
            return unsupported(
                "machine signature is outside the current terminal-Psi source slice",
            );
        }
        let scalar_entry =
            CheckedScalarCallee::find_for_unit_call(checked, selection.machine).is_ok();
        let unit_plan = flow_facts(checked)
            .terminal_unit_effects
            .for_machine(selection.machine)
            .is_some();
        if !scalar_entry
            && (unit_plan
                || selection.signature == CheckedTerminalSignatureEligibility::FreeUnitEffect)
        {
            return Ok(LoweredSelectedMachine::source_mapped(
                lower_unit_effect_closure(
                    identities,
                    checked,
                    selection.machine,
                    program_roots,
                    exact_toolchain_sources,
                )?,
                LoweringCompletion {
                    debug: DebugPublication::Omit,
                },
            ));
        }
        if !scalar_entry {
            return Err(LoweringError::InvalidUnitMachinePlan {
                machine: checked.symbols.display_path(selection.machine, "::"),
                reason: "machine has no checked executable body",
                gap: flow_facts(checked)
                    .terminal_unit_effects
                    .gap_diagnostic(selection.machine, &checked.symbols),
            });
        }
        Ok(LoweredSelectedMachine::source_mapped(
            crate::unit::attached_unit::lower_scalar_effect_closure(
                identities,
                checked,
                selection.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                ..Default::default()
            },
        ))
    }

    fn select_unit_plan<'checked>(
        checked: &'checked CheckedTrees,
        selection: &CheckedTerminalMachineSelection,
    ) -> Result<Option<CheckedUnitPlan<'checked>>, LoweringError> {
        let Some(plan) = CheckedUnitPlan::for_machine(&checked.facts.flow, selection.machine)
        else {
            return Ok(None);
        };
        if plan.is_duplicated_in(&checked.facts.flow) {
            return unsupported("Unit plan is duplicated in its roster");
        }
        if !plan.admits_signature(selection.signature) {
            return unsupported("Unit plan attachment disagrees with its selected signature");
        }
        Ok(Some(plan))
    }
}
pub(crate) mod scalar_transport {

    use lowered_psi::LoweredPsi;
    use terminal_semantics::scalar_demand::{ScalarDemandMode, complete_scalar_demand};

    pub(crate) fn complete(lowered: &mut LoweredPsi) {
        let module = &lowered.semantic_module;
        let mut retained = terminal_semantics::scalar_demand::record_named_rows(module).values;
        for operation in module.operations() {
            if module
                .boundary_application_demands
                .binary_search_by_key(&operation.id, |demand| demand.terminal_operation())
                .is_ok()
                && let Some(result) = operation.result.scalar()
            {
                retained.push(result.id);
            }
        }
        for occurrence in &lowered.source_call_occurrences {
            occurrence
                .frontier_before_call
                .visit_value_ids(|value| retained.push(value));
        }
        let mut protected = lowered
            .state_blocks
            .iter()
            .map(|row| (row.machine, row.block))
            .collect::<Vec<_>>();
        for row in &module.scalar_block_invariants {
            protected.push((row.machine, row.header));
            row.predicate.visit_value_ids(|value| retained.push(value));
        }
        let Some(qualified) =
            terminal_semantics::scalar_demand::qualification_parameter_blocks(module)
        else {
            return;
        };
        protected.extend(qualified);
        for row in &module.scalar_qualifications.coercions {
            retained.extend([row.source, row.destination]);
        }
        protected.sort_unstable();
        let Some(retained) =
            terminal_semantics::scalar_demand::retained_values_by_machine(module, retained)
        else {
            return;
        };
        for machine in &mut lowered.semantic_module.machines {
            let first = protected.partition_point(|(owner, _)| *owner < machine.id);
            let last = protected.partition_point(|(owner, _)| *owner <= machine.id);
            let blocks = protected[first..last]
                .iter()
                .map(|(_, block)| *block)
                .chain([machine.entry]);
            let values = retained
                .binary_search_by_key(&machine.id, |(owner, _)| *owner)
                .ok()
                .map_or(&[][..], |position| retained[position].1.as_slice());
            complete_scalar_demand(
                machine,
                values.iter().copied(),
                blocks,
                ScalarDemandMode::KeepAllOperations,
            );
        }
    }
}

use crate::shared::checked_facts::flow_facts;
use checked_trees::{CheckedTerminalSignatureEligibility, CheckedTrees};
use lowered_psi::LoweredPsi;

use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::float_meanings::retain_float_meanings;
use crate::machine_lowering::guarded_exits::reject_unguarded_outcome_guarantees;
use crate::machine_lowering::machine_dispatch::{
    TerminalMachineSelection, lower_selected_machine, select_terminal_machine,
};
use crate::proofs::evidence_lowering::lower_and_install_evidence_artifacts;
use crate::retention::{
    closed_reach_applications, operation_crash_contracts, reborrow_restored_call_use,
    retained_borrow_custody, suspension_call_plan,
};
use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::producer_result::{
    DebugPublication, LoweredSelectedMachine, LoweringCompletion,
};
use crate::unit::attached_unit;

pub fn lower_machine(
    checked: &CheckedTrees,
    machine: TerminalMachineSelection<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    program_roots: crate::TerminalProgramRoots<'_>,
    publication: terminal_codec::TerminalPublication,
    external_declarations: Option<&crate::PreparedExternalProviderDeclarations>,
) -> Result<LoweredPsi, LoweringError> {
    if !checked.facts.deferred_layout_applications.is_empty() {
        return unsupported(
            "physical layout applications require strict target closure before Terminal production",
        );
    }
    if flow_facts(checked)
        .terminal_unit_effects
        .boundary_machines
        .iter()
        .any(|plan| {
            !plan.pending_scalar_requires.is_empty() || !plan.pending_scalar_recipes.is_empty()
        })
    {
        return unsupported(
            "boundary scalar contract clauses require target closure before Terminal production",
        );
    }
    let mut identities = match publication {
        terminal_codec::TerminalPublication::Portable => {
            terminal_psi::TerminalIdentityTableBuilder::default()
        }
        terminal_codec::TerminalPublication::Invocation => {
            terminal_psi::TerminalIdentityTableBuilder::for_invocation()
        }
    };
    let mut roots = program_roots.machines.to_vec();
    roots.extend(
        checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .filter(|usage| usage.callback_entry.is_some())
            .map(|usage| usage.selected_machine),
    );
    roots.extend(
        checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .filter(|usage| {
                checked
                    .facts
                    .installed_interrupt_registrations
                    .iter()
                    .any(|registration| registration.site == usage.site)
            })
            .map(|usage| usage.selected_machine),
    );
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    let mut boundaries = program_roots.boundaries.to_vec();
    boundaries.extend(
        checked
            .facts
            .installed_interrupt_registrations
            .iter()
            .map(|registration| registration.target),
    );
    boundaries.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    boundaries.dedup();
    let program_roots = crate::TerminalProgramRoots {
        machines: &roots,
        boundaries: &boundaries,
    };
    let selection = match machine {
        TerminalMachineSelection::Declarations => None,
        machine => Some(select_terminal_machine(checked, machine)?),
    };
    let (mut lowered, completion) = lower_terminal_selection(
        &mut identities,
        checked,
        selection,
        exact_toolchain_sources,
        program_roots,
    )
    .map_err(|error| error.with_source_names(checked))?;
    boundary_applications::retain(
        &mut identities,
        checked,
        exact_toolchain_sources,
        &mut lowered,
    )
    .map_err(|reason| LoweringError::InvalidBoundaryApplication(reason.to_owned()))?;
    match external_declarations {
        Some(declarations) => declarations.retain(
            checked,
            exact_toolchain_sources,
            &mut identities,
            &mut lowered.semantic_module,
        )?,
        None => crate::retention::external_providers::retain(
            &mut identities,
            checked,
            exact_toolchain_sources,
            &mut lowered.semantic_module,
        )?,
    }
    callback_applications::retain(
        &mut identities,
        checked,
        exact_toolchain_sources,
        &mut lowered,
    )
    .map_err(LoweringError::InvalidDeclarationIdentity)?;
    installed_interrupts::retain(
        &mut identities,
        checked,
        exact_toolchain_sources,
        &mut lowered,
    )
    .map_err(LoweringError::InvalidDeclarationIdentity)?;
    lowered.semantic_module.task_activation_demands =
        crate::lower_task_activation_demands(checked, &mut identities)
            .map_err(LoweringError::TaskActivation)?;
    lowered.semantic_module.identities = identities.build();
    scalar_transport::complete(&mut lowered);

    if let Err(reason) =
        terminal_semantics::validate_runtime_evidence_inputs(&lowered.semantic_module)
    {
        diagnose_runtime_evidence_case_failure(&lowered.semantic_module, reason);
        return Err(LoweringError::Unsupported(reason));
    }
    lowered.debug_map = (|| -> Result<_, LoweringError> {
        if let Some(selection) = selection
            && selection.signature == CheckedTerminalSignatureEligibility::Eligible
            && completion.debug == DebugPublication::FromCheckedPlan
        {
            let live_states = flow_facts(checked)
                .terminal_unit_effects
                .composed_for_machine(selection.machine)
                .map(attached_unit::unit_graph_live_states)
                .transpose()?;
            flow_facts(checked)
                .terminal_debug
                .for_machine(selection.machine)
                .map(|plan| build_debug_map(plan, &lowered.semantic_module, live_states.as_deref()))
                .transpose()
        } else {
            Ok(None)
        }
    })()?;
    Ok(lowered)
}

pub(crate) fn nominal_cleanup_subject(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    if validation::nominal_cleanup_owner(&checked.typed, source).is_none() {
        return Ok(None);
    }
    let rendered = type_ids
        .iter()
        .map(|(identity, id)| {
            (
                checked.type_identity_text(*identity).unwrap_or_default(),
                *id,
            )
        })
        .collect::<Vec<_>>();
    nominal_cleanup_subject_from_types(
        checked,
        source,
        rendered
            .iter()
            .map(|(identity, id)| (identity.as_str(), *id)),
    )
}

fn nominal_cleanup_subject_from_types<'types>(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    mut types: impl Iterator<Item = (&'types str, semantic_vocabulary::StructuralTypeId)>,
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    let Some((owner, selected_subject)) =
        validation::nominal_cleanup_target(&checked.typed, source)
    else {
        return Ok(None);
    };
    let data = checked
        .data_definition_by_symbol(owner)
        .ok_or(LoweringError::Unsupported(
            "cleanup declaration lost its exact owner",
        ))?;
    let identity = if let Some(subject) = selected_subject {
        checked.normalized_type_identity(subject)
    } else if let Some(application) = data.generic_instance {
        checked.normalized_type_identity(application)
    } else {
        checked
            .normalized_nominal_type_identity(owner)
            .ok_or(LoweringError::Unsupported(
                "cleanup owner has no normalized nominal identity",
            ))?
    };
    let (_, subject) = types
        .find(|(candidate, _)| {
            *candidate == checked.type_identity_text(identity).unwrap_or_default()
        })
        .ok_or(LoweringError::Unsupported(
            "owner-designated cleanup lost its exact structural declaration",
        ))?;
    Ok(Some(subject))
}

fn retain_nominal_cleanup_roles(
    checked: &CheckedTrees,
    owners: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), LoweringError> {
    for (source, identity) in owners {
        let mut matches = module
            .machines
            .iter_mut()
            .filter(|machine| machine.id == *identity);
        let machine = matches.next().ok_or(LoweringError::Unsupported(
            "cleanup source mapping lost its emitted machine",
        ))?;
        if matches.next().is_some() {
            return unsupported("cleanup source mapping repeats its emitted machine");
        }
        let expected = nominal_cleanup_subject_from_types(
            checked,
            *source,
            module.structural_types.iter().map(|declaration| {
                (
                    module.identities.text(declaration.identity).unwrap_or(""),
                    declaration.id,
                )
            }),
        )?;
        machine.attachment = machine.attachment.or(expected);
        machine.nominal_cleanup_subject = expected;
    }
    Ok(())
}

fn lower_terminal_selection(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selection: Option<&checked_trees::CheckedTerminalMachineSelection>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    program_roots: crate::TerminalProgramRoots<'_>,
) -> Result<(LoweredPsi, LoweringCompletion), LoweringError> {
    let LoweredSelectedMachine {
        mut next_call_obligation,
        terminal: mut lowered,
        completion,
        source_machine_ids,
        source_boundary_ids,
    } = match selection {
        Some(selection) => lower_selected_machine(
            identities,
            checked,
            selection,
            program_roots,
            exact_toolchain_sources,
        ),
        None => Ok(LoweredSelectedMachine::source_mapped(
            attached_unit::lower_unit_closure(
                identities,
                checked,
                &attached_unit::UnitClosureRequest::unit(
                    checked,
                    symbols::SymbolHandle::invalid(),
                    &[],
                    program_roots,
                    exact_toolchain_sources,
                ),
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        )),
    }?;
    if lowered.semantic_module.target_machine_families.len() != runtime_families(checked)?.len() {
        return unsupported("entry lowering did not retain the complete target-family catalog");
    }
    let source_machines = source_machine_ids
        .iter()
        .map(|(source, _)| *source)
        .collect::<Vec<_>>();
    declarations::retain(
        identities,
        checked,
        exact_toolchain_sources,
        &source_machine_ids,
        &mut lowered.semantic_module,
    )
    .map_err(LoweringError::InvalidDeclarationIdentity)?;
    retain_nominal_cleanup_roles(checked, &source_machine_ids, &mut lowered.semantic_module)?;
    closed_reach_applications::retain_closed_reach_applications(
        identities,
        checked,
        &source_machine_ids,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    (|| -> Result<_, LoweringError> {
        for (source_machine, terminal_machine) in &source_machine_ids {
            reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
                identities,
                checked,
                *source_machine,
                *terminal_machine,
                &lowered.source_call_occurrences,
                &lowered.semantic_module.machines,
                &mut lowered.semantic_module.reborrow_restored_call_uses,
            )?;
        }
        Ok(())
    })()?;
    (|| -> Result<_, LoweringError> {
        crate::retention::access_view_plans::retain(
            identities,
            checked,
            exact_toolchain_sources,
            &mut lowered.semantic_module,
        )?;
        retain(identities, checked, exact_toolchain_sources, &mut lowered)?;
        retained_borrow_custody::retain_foreign_borrow_custodies(
            identities,
            checked,
            exact_toolchain_sources,
            &mut lowered.semantic_module,
        )?;
        Ok(())
    })()?;
    operation_crash_contracts::retain_operation_crash_contracts(
        identities,
        checked,
        &source_machines,
        &source_machine_ids,
        &mut lowered,
    )?;
    (|| -> Result<_, LoweringError> {
        reject_unguarded_outcome_guarantees(
            checked,
            &source_machine_ids,
            &lowered.semantic_module,
        )?;
        crate::retention::conformance_applications::lower_closed_conformance_applications(
            identities,
            checked,
            &source_machines,
            &source_boundary_ids,
            &mut lowered.semantic_module,
        )?;
        crate::unit::dynamic_dispatch::lower_dynamic_dispatch_catalog(
            identities,
            checked,
            &source_machines,
            &mut lowered.semantic_module,
        )?;
        suspension_call_plan::retain_suspension_call_plans(
            identities,
            checked,
            &source_machines,
            &lowered.source_call_occurrences,
            &lowered.state_blocks,
            &mut lowered.semantic_module,
        )?;
        retain_float_meanings(identities, checked, &source_machine_ids, &mut lowered)?;
        Ok(())
    })()?;
    lowered.build_provider_requirements = build_providers::retain(
        identities,
        checked,
        exact_toolchain_sources,
        &source_boundary_ids,
        &lowered.source_call_occurrences,
    )?;

    (|| -> Result<_, LoweringError> {
        {
            reconcile_leaf_copy_place_declarations(&mut lowered.semantic_module);
        };
        crate::emission::descriptor_residences::complete(&mut lowered.semantic_module)?;
        crate::retention::payload_memberships::retain(&mut lowered.semantic_module)?;
        Ok(())
    })()?;
    lower_and_install_evidence_artifacts(identities, checked, &source_machine_ids, &mut lowered)?;
    crate::callable_catalog::finalize_descriptor_evidence(
        &mut lowered.semantic_module,
        &mut next_call_obligation,
    )?;
    lowered.source_boundary_declarations = source_boundary_ids;
    lowered.source_machine_declarations = source_machine_ids;
    Ok((lowered, completion))
}

fn diagnose_runtime_evidence_case_failure(module: &terminal_psi::TerminalModule, reason: &str) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
        return;
    }
    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
    let mut analysis = terminal_semantics::descriptor_results::DescriptorAnalysis::new(
        &module.structural_types,
        &graph,
    );
    for invocation in &module.proof_output_calls {
        let Some(runtime) = invocation.runtime_call else {
            continue;
        };
        let Some(caller) = module
            .machines
            .iter()
            .find(|machine| machine.id == invocation.caller)
        else {
            continue;
        };
        let Some(machine) = graph.iter().find(|machine| machine.id == caller.id) else {
            eprintln!(
                "[runtime-evidence-case] caller={:?} consumer={:?} graph=absent reason={reason}",
                caller.id, runtime.site
            );
            continue;
        };
        let feasibility = analysis.flow_feasibility(caller.id);
        for block in &caller.blocks {
            for (index, operation) in block.operations.iter().enumerate() {
                let Some(parts) = terminal_psi::direct_call_parts(&operation.kind) else {
                    continue;
                };
                for row in parts.selected_evidence.iter().filter(|row| {
                    invocation
                        .evidence_arguments
                        .iter()
                        .any(|argument| argument.source == row.output)
                }) {
                    let terminal_psi::OperationResult::Structural(result) = &operation.result
                    else {
                        continue;
                    };
                    let predicate =
                        terminal_semantics::descriptor_results::DescriptorCasePredicate {
                            subject:
                                terminal_semantics::descriptor_lifetimes::DescriptorLocation::whole(
                                    result.place,
                                ),
                            case: row.guard.result_case,
                        };
                    eprintln!(
                        "[runtime-evidence-case] caller={:?} producer={:?} producer_cut=({:?},{index}) consumer={:?} output={:?} result={:?} type={:?} guard_case={:?} validity_result={:?} reason={reason}",
                        caller.id,
                        operation.id,
                        block.id,
                        runtime.site,
                        row.output,
                        result.place,
                        result.structural_type,
                        row.guard.result_case,
                        row.validity.result
                    );
                    for (position, step) in &machine.flow {
                        let established =
                            analysis.case_is_established(caller.id, *position, &predicate);
                        let membership =
                            analysis.case_feasibility(caller.id, *position, &predicate);
                        eprintln!(
                            "[runtime-evidence-case-step] caller={:?} cut={position:?} flow={:?} original_selected={established} original_membership={membership:?} definitions={:?} derivations={:?} producer={:?} contents_transfer={} unresolved_write={} payload_invalidations={}",
                            caller.id,
                            feasibility.get(position),
                            step.definitions,
                            step.derivations,
                            machine.payload_producers.get(position),
                            machine.contents_transfers.contains_key(position),
                            machine.unresolved_contents_writes.contains(position),
                            machine
                                .payload_invalidations
                                .get(position)
                                .map_or(0, Vec::len)
                        );
                        for edge in &step.successors {
                            eprintln!(
                                "[runtime-evidence-case-edge] caller={:?} cut={position:?} target={:?} parameters={:?} derivations={:?}",
                                caller.id,
                                edge.target,
                                edge.parameters,
                                edge.derivations().collect::<Vec<_>>()
                            );
                        }
                        for (branch, predicates) in &machine.case_successors {
                            for (observed, truth) in predicates
                                .iter()
                                .filter(|(observed, _)| observed.case == row.guard.result_case)
                            {
                                let selected =
                                    analysis.case_is_established(caller.id, *position, observed);
                                eprintln!(
                                    "[runtime-evidence-case-observation] caller={:?} cut={position:?} branch={branch:?} subject={:?} case={:?} truth={truth} selected={selected}",
                                    caller.id, observed.subject, observed.case
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

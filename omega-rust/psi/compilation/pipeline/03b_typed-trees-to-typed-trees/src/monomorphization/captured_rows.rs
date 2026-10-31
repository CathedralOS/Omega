//! Apply a named map's capture telescope before its owned method's invocation
//! telescope. Referenced realizers retain their independent declaration scope.
//! The exact closed selection supplies operands; runtime carriers and diagnostic
//! names cannot infer a missing map application.

use super::requirement_applications::selected_applications;
use super::{CallSite, Diagnostic, SymbolHandle, TypedTrees, failed};
use crate::expression_roots::append_statement_roots;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

macro_rules! err {
    ($message:literal) => {
        vec![Diagnostic::error($message)]
    };
}

#[derive(Clone, Copy)]
enum CaptureSupply {
    /// A current declaration or selected Product operand, independently
    /// reconstructed from its live source by frozen checking.
    AuthoredSelection,
    /// A lexical call carries an existing capture's source receipt; it cannot
    /// replace a missing authored selection with that same derived receipt.
    CarriedCapture,
}

pub(super) fn close(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
    let mut calls = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for handle in state.statement_nodes.handles() {
                if let Some(call) = program.statement_table.statement(handle).as_call() {
                    calls.push((
                        CallSite::Statement(handle),
                        machine.symbol,
                        call.target_symbol,
                        call.static_requirement_dispatch.clone(),
                    ));
                }
            }
            let mut expressions = Vec::new();
            append_statement_roots(program, state, &mut expressions);
            for handle in expressions {
                if let Some(call) = program.expression_table.call_node(handle) {
                    calls.push((
                        CallSite::Expression(handle),
                        machine.symbol,
                        call.target_symbol,
                        call.static_requirement_dispatch.clone(),
                    ));
                }
            }
        }
    }
    let mut changed = Vec::new();
    let mut selected_roots = program
        .trait_implementation_applications
        .iter()
        .map(|(_, selected)| (selected.supplying_application, selected.application.clone()))
        .chain(
            program
                .data_implementation_applications
                .iter()
                .map(|(_, selected)| {
                    (selected.supplying_application, selected.application.clone())
                }),
        )
        .collect::<Vec<_>>();
    // Source formation also retains applications in unexecuted helpers. Only
    // a reached Build choice demands its owned realization's capture tuple.
    // The source census independently excludes discarded preparation rows.
    selected_roots.extend(
        program
            .live_product_operand_applications()
            .into_iter()
            .filter_map(|handle| {
                let selected = program.product_operand_applications.get(handle);
                if selected.demand
                    != typed_trees::product_operand::ProductApplicationDemand::Selected
                {
                    return None;
                }
                Some((
                    selected.supplying_application,
                    selected.implementation.as_ref()?.clone(),
                ))
            }),
    );
    for (supplying, application) in selected_roots {
        let conformance = program
            .conformance_by_symbol(application.declaration)
            .ok_or_else(|| err!("selected trait implementation lost its declaration"))?
            .clone();
        for row in conformance
            .rows
            .iter()
            .filter(|row| row.source.owns_realization())
        {
            let before = program.captured_machine_applications.len();
            if let Some(captured) = realize_capture(
                program,
                &application,
                supplying,
                row.realization_machine,
                CaptureSupply::AuthoredSelection,
            )? && program.captured_machine_applications.len() != before
            {
                changed.push(captured.instance);
            }
        }
    }
    for (site, owner, target, dispatch) in calls {
        let Some((application, supplying_application, realization_machine)) =
            call_capture(program, owner, target, dispatch.as_ref())?
        else {
            continue;
        };
        let Some(captured) = realize_capture(
            program,
            &application,
            supplying_application,
            realization_machine,
            CaptureSupply::CarriedCapture,
        )?
        else {
            continue;
        };
        let selected = program
            .captured_machine_states
            .span_or_empty(captured.states)
            .iter()
            .filter(|state| state.template == target)
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            return Err(err!(
                "capture application lost its exact entry correspondence"
            ));
        };
        let selected = selected.instance;
        let name = program
            .state_by_symbol(selected)
            .ok_or_else(|| err!("capture application has no retained executable entry"))?
            .name
            .clone();
        match site {
            CallSite::Expression(handle) => {
                let super::ExpressionNode::Call(call) =
                    program.expression_table.expression_mut(handle)
                else {
                    unreachable!()
                };
                call.target_symbol = selected;
                call.target = name;
            }
            CallSite::Statement(handle) => {
                let super::StatementNode::Call(call) =
                    program.statement_table.statement_mut(handle)
                else {
                    unreachable!()
                };
                call.target_symbol = selected;
                call.target = name;
            }
        }
        changed.extend([owner, captured.instance]);
    }
    Ok(changed)
}

/// Explicit selections carry their own invocation custody. A lexical sibling
/// instead borrows the caller's exact map capture, including phantom operands;
/// it does not fabricate another authored selection occurrence.
fn call_capture(
    program: &TypedTrees,
    owner: SymbolHandle,
    target: SymbolHandle,
    dispatch: Option<&typed_trees::typed_trees::StaticRequirementDispatch>,
) -> Result<
    Option<(
        typed_trees::typed_trees::ClosedConformanceApplication,
        typed_trees::static_applications::StaticMachineApplicationHandle,
        SymbolHandle,
    )>,
    Vec<Diagnostic>,
> {
    let Some(dispatch) = dispatch else {
        let Some(selecting) =
            program.conformance_by_symbol(program.declaration_scope_parent(owner))
        else {
            return Ok(None);
        };
        let Some(capture) = program.implementation_capture(
            owner,
            program.lifetime_declaration_owner(owner),
            selecting.symbol,
        ) else {
            return Ok(None);
        };
        let mut rows = selecting
            .rows
            .iter()
            .filter(|row| row.source.owns_realization() && row.realization_state == target);
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        if rows.next().is_some() {
            return Err(vec![Diagnostic::error(
                "lexical call has ambiguous owned realization rows",
            )]);
        }
        return Ok(Some((
            capture.conformance_application.clone(),
            capture.supplying_application,
            row.realization_machine,
        )));
    };
    if target != dispatch.realization_state {
        return Ok(None);
    }
    let applications = selected_applications(program, owner, dispatch);
    let [application] = applications.as_slice() else {
        return Ok(None);
    };
    let conformance = program
        .conformance_by_symbol(application.declaration)
        .ok_or_else(|| failed("selected capture application lost its map declaration"))?;
    let owned = conformance.rows.iter().any(|row| {
        row.source.owns_realization()
            && row.realization_machine == dispatch.realization_machine
            && row.realization_state == dispatch.realization_state
            && row.requirement == dispatch.requirement
            && row.declaring_trait == dispatch.declaring_trait
    });
    if !owned
        || (program.conformance_type_parameters(conformance).is_empty()
            && conformance.lifetime_parameters.is_empty())
    {
        return Ok(None);
    }
    let supplying = if dispatch.selection_application.is_valid() {
        dispatch.selection_application
    } else {
        program
            .machine_specialization(owner)
            .map(|receipt| receipt.binding_application)
            .filter(|application| application.is_valid())
            .ok_or_else(|| failed("selected capture lost its supplying source invocation"))?
    };
    Ok(Some((
        (*application).clone(),
        supplying,
        dispatch.realization_machine,
    )))
}

/// Apply only a map's capture telescope. Runtime dispatch and declaration-owned
/// policy demands share this derivation; neither fabricates an invocation.
fn realize_capture(
    program: &mut TypedTrees,
    application: &typed_trees::typed_trees::ClosedConformanceApplication,
    supplying_application: typed_trees::static_applications::StaticMachineApplicationHandle,
    realization_machine: SymbolHandle,
    supply: CaptureSupply,
) -> Result<Option<typed_trees::typed_trees::CapturedMachineApplication>, Vec<Diagnostic>> {
    let conformance = program
        .conformance_by_symbol(application.declaration)
        .cloned()
        .ok_or_else(|| err!("selected capture lost its map declaration"))?;
    if program.conformance_type_parameters(&conformance).is_empty()
        && conformance.lifetime_parameters.is_empty()
    {
        return Ok(None);
    }
    if !conformance
        .rows
        .iter()
        .any(|row| row.source.owns_realization() && row.realization_machine == realization_machine)
        || program.declaration_scope_parent(realization_machine) != conformance.symbol
    {
        return Err(err!("selected capture has no exact owned realization"));
    }
    let supplying_owner = program
        .expression_table
        .static_machine_application(supplying_application)
        .lexical_owner;
    let retained = program
        .captured_machine_applications
        .iter()
        .filter(|(_, captured)| {
            captured.template == realization_machine
                && program.closed_conformance_applications_match(
                    &captured.conformance_application,
                    application,
                )
                && program
                    .expression_table
                    .static_machine_application(captured.supplying_application)
                    .lexical_owner
                    == supplying_owner
        })
        .map(|(handle, captured)| (handle, captured.clone()))
        .collect::<Vec<_>>();
    let captured = match retained.as_slice() {
        [(handle, captured)] => {
            let mut captured = captured.clone();
            if let CaptureSupply::AuthoredSelection = supply {
                // Bodies share by their full applied meaning and supplier,
                // not by whichever source occurrence happened to arrive first.
                // Elaboration can replace that occurrence. Retain the current
                // independent selection while preserving the original body and
                // binding tuple; frozen checking must rejoin and replay both.
                captured.supplying_application = supplying_application;
                captured.conformance_application = application.clone();
                *program.captured_machine_applications.get_mut(*handle) = captured.clone();
            }
            captured
        }
        [] => {
            let Some(index) = program.machine_position_by_symbol(realization_machine) else {
                return Err(err!("selected capture application lost its owned body"));
            };
            let template = super::candidate::from_capture(program, index, &conformance);
            let mut machines = Vec::new();
            let evidence = Vec::new();
            let mut types = Vec::new();
            let mut constants = Vec::new();
            let values = Vec::new();
            let mut executable_arguments = application.arguments.to_vec();
            let callback_count = program
                .conformance_type_parameters(&conformance)
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Machine { .. }
                    )
                })
                .count();
            let selected_callbacks = if application.callable_application.is_valid() {
                program
                    .expression_table
                    .static_machine_application_selected_machines(application.callable_application)
                    .to_vec()
            } else {
                Vec::new()
            };
            if selected_callbacks.len() != callback_count {
                return Err(vec![Diagnostic::error(
                    "capture lost its complete occurrence-owned callback selection roster",
                )]);
            }
            let mut callback_ordinal = 0;
            for (parameter, argument) in program
                .conformance_type_parameters(&conformance)
                .iter()
                .zip(&mut executable_arguments)
            {
                if matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                ) {
                    *argument = selected_callbacks[callback_ordinal].clone();
                    callback_ordinal += 1;
                }
            }
            // A closed map tuple is already bound in declaration order.
            // A normalized const carries a type-reference handle too;
            // carrier shape cannot reclassify it as a Type operand.
            let mut type_ordinal = 0;
            let mut const_ordinal = 0;
            let mut machine_ordinal = 0;
            let parameters = program.conformance_type_parameters(&conformance).to_vec();
            if parameters.len() != executable_arguments.len() {
                return Err(err!(
                    "map capture tuple differs from its declaration telescope"
                ));
            }
            for (parameter, argument) in parameters.iter().zip(&executable_arguments) {
                use typed_trees::data::TypeParameterKind;
                match &parameter.kind {
                    TypeParameterKind::Type => {
                        let reference = program
                            .static_operand_type_reference(argument)
                            .map_err(|message| vec![Diagnostic::error(message)])?;
                        types.push((0, type_ordinal, reference));
                        type_ordinal += 1;
                    }
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                        // Capture selection already retains its exact
                        // canonical value and carrier. Executable const
                        // binding transports that value, rather than a
                        // fresh expression recipe used by contract views.
                        let reference = match application.const_arguments.get(const_ordinal) {
                                Some(typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated {
                                    value, ..
                                }) => {
                                    // Use the shared static-operand namespace:
                                    // integers retain literal identity; aggregate
                                    // atoms retain the original carrier display.
                                    let canonical = language_semantics::const_value::CanonicalConstValue::new(
                                        value.type_name.clone(), value.encoding.clone(), "",
                                    );
                                    let selected_value = typed_trees::expression::StaticMachineArgument {
                                        path: vec![typed_trees::name::Identifier::generated(canonical.atom())].into_boxed_slice(),
                                        ..Default::default()
                                    };
                                    let atom = super::const_arguments::spelling(program, &selected_value)
                                        .ok_or_else(|| vec![Diagnostic::error("evaluated map capture lost its canonical value identity")])?;
                                    program.type_reference_table.insert(
                                        typed_trees::types::TypeReferenceNode::Named {
                                            symbol: SymbolHandle::invalid(),
                                            name: typed_trees::name::Identifier::generated(atom),
                                        },
                                    )
                                }
                                Some(
                                    typed_trees::typed_trees::ClosedConformanceConstArgument::CallerExpression {
                                        ..
                                    } |
                                    typed_trees::typed_trees::ClosedConformanceConstArgument::CallerBinder {
                                        ..
                                    },
                                ) => program
                                    .static_operand_type_reference(argument)
                                    .map_err(|message| vec![Diagnostic::error(message)])?,
                                None => {
                                    return Err(vec![Diagnostic::error(
                                        "map capture lost its exact const value roster",
                                    )]);
                                }
                            };
                        constants.push((0, const_ordinal, reference));
                        const_ordinal += 1;
                    }
                    TypeParameterKind::Machine { .. } => {
                        machines.push((0, machine_ordinal, argument.clone()));
                        machine_ordinal += 1;
                    }
                    _ => {
                        return Err(err!("map capture has unsupported declaration operand"));
                    }
                }
            }
            let bindings = super::selection::proposal_bindings(
                program,
                &template,
                super::selection::StaticProposals {
                    types,
                    constants,
                    machines,
                    evidence,
                    runtime_values: values
                        .into_iter()
                        .map(|(candidate, binder, argument)| {
                            (
                                candidate,
                                binder,
                                super::RuntimeValueBinding::Authored(argument),
                            )
                        })
                        .collect(),
                },
            );
            if bindings.conflicted
                || bindings.unresolved_type_parameters
                || bindings.unresolved_const_parameters
                || bindings.unresolved_machine_parameters
                || bindings.unresolved_evidence_parameters
            {
                return Ok(None);
            }
            let mut candidate = template;
            candidate.type_bindings = bindings.type_bindings;
            candidate.const_bindings = bindings.const_bindings;
            candidate.runtime_value_bindings = bindings.runtime_value_bindings;
            candidate.machine_bindings = bindings.machine_bindings;
            candidate.evidence_bindings = bindings.evidence_bindings;
            if !super::provider_tuple_closure::operands_are_closed(program, &candidate) {
                return Ok(None);
            }
            let mut bindings = super::applications::candidate_bindings(program, &candidate);
            // Admission retains the authored tuple; executable substitution
            // consumes its separately prepared selected callback entries.
            for (parameter, original) in program
                .conformance_type_parameters(&conformance)
                .iter()
                .zip(application.arguments.iter())
            {
                if matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                ) {
                    let binding = bindings
                        .iter_mut()
                        .find(|binding| binding.parameter == parameter.symbol)
                        .ok_or_else(|| err!("capture lost its declared Machine binder"))?;
                    binding.value =
                        typed_trees::static_applications::StaticMachineBindingValue::Machine(
                            original.clone(),
                        );
                }
            }
            let binding = program.expression_table.retain_static_machine_application(
                conformance.symbol,
                conformance.alias.clone().unwrap_or_default(),
                &application.arguments,
            );
            let retained = program
                .expression_table
                .static_machine_application_mut(binding);
            retained.lexical_owner = supplying_owner;
            retained.binding_declaration = conformance.symbol;
            program
                .expression_table
                .set_static_machine_application_bindings(binding, &bindings);
            let lifetimes = conformance
                .lifetime_parameters
                .iter()
                .zip(&application.lifetime_arguments)
                .map(|(parameter, argument)| {
                    typed_trees::static_applications::StaticMachineLifetimeBinding {
                        parameter: parameter.clone(),
                        argument: typed_trees::name::Identifier::generated(argument),
                    }
                })
                .collect::<Vec<_>>();
            program
                .expression_table
                .set_static_machine_application_lifetimes(binding, conformance.symbol, &lifetimes);
            program
                .expression_table
                .set_static_machine_application_selected_machines(binding, &selected_callbacks);
            candidate.binding_application = binding;
            let machine = &program.machines()[index];
            let identity = normalized_machine_identity(program, machine)
                .ok_or_else(|| err!("owned capture body has no normalized declaration identity"))?;
            let commitment = accepted_template_commitment(program, index);
            let ordinal = program
                .captured_machine_applications
                .iter()
                .filter(|(_, capture)| capture.template == realization_machine)
                .count();
            super::body_cloning::clone_single_specialized_machine(
                None,
                program,
                &candidate,
                ordinal,
                identity,
                commitment,
                Some((application, supplying_application)),
            )?;
            program
                .captured_machine_applications
                .iter()
                .find(|(_, capture)| {
                    capture.template == realization_machine
                        && program.closed_conformance_applications_match(
                            &capture.conformance_application,
                            application,
                        )
                        && program
                            .expression_table
                            .static_machine_application(capture.supplying_application)
                            .lexical_owner
                            == supplying_owner
                })
                .map(|(_, capture)| capture.clone())
                .ok_or_else(|| err!("capture cloning lost its exact substitution relation"))?
        }
        _ => {
            return Err(err!("owned map body has conflicting capture applications"));
        }
    };
    Ok(Some(captured))
}

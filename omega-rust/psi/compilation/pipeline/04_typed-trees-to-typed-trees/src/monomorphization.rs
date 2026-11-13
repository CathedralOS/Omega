/*
Compile-time specialization fixed-point coordinator (`monomorphization`),
candidate templates and tuples, const argument and static value materialization,
dynamic families and slots, and operator-candidate specialization.
*/

pub(crate) use accessor_primitives::select_calls;
pub(crate) use body_rewriting::normalize_const_arguments;
pub(crate) use result_locals::require_complete_dispatch_subjects;

pub(crate) use crate::monomorphization::body_cloning::clone_specialized_machine;
pub(crate) use crate::monomorphization::body_rewriting::rewrite_selected_call;
pub(crate) use crate::tables::{expressions_of, expressions_of_mut};
pub(crate) use arena::{Handle, HandleSpan};
pub(crate) use diagnostics::Diagnostic;
pub(crate) use selection::collect_call_selections;
pub(crate) use selection::collect_contract_call_selections;
pub(crate) use symbols::{SymbolHandle, SymbolKind};
pub(crate) use typed_trees::TypedTrees;
pub(crate) use typed_trees::data::TypeParameterKind;
pub(crate) use typed_trees::domain::ProofFact;
pub(crate) use typed_trees::domain::indexed_domain_instance_name;
pub(crate) use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
pub(crate) use typed_trees::references::DeclarationSelection;
pub(crate) use typed_trees::signature::StateSignature;
pub(crate) use typed_trees::statement::{StatementHandle, StatementNode};
pub(crate) use typed_trees::static_applications::StaticMachineApplicationHandle;
pub(crate) use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
pub(crate) use validation::contract_identities::{
    accepted_template_commitment, normalized_machine_identity,
};

pub(crate) mod accessor_primitives {
    use typed_trees::typed_trees::{AccessorPrimitive, ScalarAccessorDeclaration};

    use super::{CallSelection, Candidate, Diagnostic, SymbolHandle, TypedTrees};

    pub(crate) fn select_calls(
        program: &mut TypedTrees,
        candidates: &[Candidate],
        selections: &mut Vec<CallSelection>,
        changed_owners: &mut Vec<SymbolHandle>,
    ) -> Result<bool, Vec<Diagnostic>> {
        let mut rewrites = Vec::new();
        for selection in selections
            .iter()
            .filter(|selection| selection.is_complete())
        {
            let candidate = super::candidate_for_selection(
                program,
                &candidates[selection.candidate_index],
                selection,
            );
            let selected = select(None, program, &candidate).map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| match selection.site {
                        super::CallSite::Expression(expression) => diagnostic.with_source_span(
                            super::expressions_of(program).source_span(expression),
                        ),
                        super::CallSite::Statement(statement) => {
                            match program.statement_table.statement(statement) {
                                typed_trees::statement::StatementNode::Call(call) => {
                                    diagnostic.with_source_span(call.source_span)
                                }
                                _ => diagnostic,
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })?;
            let Some(states) = selected else {
                continue;
            };
            let Some((_, entry)) = states
                .iter()
                .find(|(original, _)| *original == selection.callee_symbol)
            else {
                return Err(vec![Diagnostic::error(
                    "placed primitive call lost its exact entry selection",
                )]);
            };
            rewrites.push((selection.site, selection.lexical_owner, *entry));
        }
        selections.retain(|selection| !rewrites.iter().any(|(site, _, _)| *site == selection.site));
        for (site, owner, entry) in &rewrites {
            super::body_rewriting::rewrite_selected_call(program, *site, *entry, &[]);
            changed_owners.push(*owner);
        }
        Ok(!rewrites.is_empty())
    }

    pub(crate) fn select(
        source: Option<&dyn validation::CheckingInput>,
        program: &TypedTrees,
        candidate: &Candidate,
    ) -> Result<Option<Vec<(SymbolHandle, SymbolHandle)>>, Vec<Diagnostic>> {
        let declarations = source.map_or(program, |source| source.typed_program());
        let Some(operation) =
            AccessorPrimitive::from_declaration(declarations, candidate.template.template_symbol)
                .map_err(|diagnostic| vec![diagnostic])?
        else {
            return Ok(None);
        };
        let [Some(accessor), Some(value)] = candidate.type_bindings.as_slice() else {
            return Err(vec![Diagnostic::error(
                "placed primitive lost its complete accessor/value application",
            )]);
        };
        let target = operation
            .select_target(declarations, *accessor, *value)
            .map_err(|diagnostic| vec![diagnostic])?;
        if source.is_some() {
            let original =
                ScalarAccessorDeclaration::authenticate(declarations, target.state_symbol)
                    .map_err(|diagnostic| vec![diagnostic])?;
            let retained = ScalarAccessorDeclaration::authenticate(program, target.state_symbol)
                .map_err(|diagnostic| vec![diagnostic])?;
            if original != retained {
                return Err(vec![Diagnostic::error(
                    "placed primitive target is not retained in the destination placement roster",
                )]);
            }
        }
        Ok(Some(vec![(
            candidate.template.state_symbols[0],
            target.state_symbol,
        )]))
    }
}
pub(crate) mod application_lifetimes {
    use super::{Diagnostic, StaticMachineArgument, SymbolHandle, TypedTrees, failed};
    use crate::tables::expressions_of;
    use typed_trees::static_applications::{
        StaticMachineApplicationHandle, StaticMachineLifetimeBinding,
    };

    pub(crate) fn bind(
        program: &TypedTrees,
        selected: SymbolHandle,
        selection: &StaticMachineArgument,
        invocation: StaticMachineApplicationHandle,
    ) -> Result<Vec<StaticMachineLifetimeBinding>, Vec<Diagnostic>> {
        let Some(explicit) = selection
            .application
            .as_ref()
            .filter(|application| !application.lifetime_arguments.is_empty())
        else {
            return Ok(Vec::new());
        };
        let machine = program
            .machines()
            .iter()
            .find(|machine| program.machine_or_state_symbol(machine, selected))
            .ok_or_else(|| failed("nested lifetime application lost its selected declaration"))?;
        if machine.lifetime_parameters.len() != explicit.lifetime_arguments.len() {
            return Err(vec![Diagnostic::error(
                "nested machine lifetime arguments do not match its complete declared telescope",
            )]);
        }
        let owner = expressions_of(program)
            .static_machine_application(invocation)
            .lexical_owner;
        let owner = program.machine_by_symbol(owner).ok_or_else(|| {
            vec![Diagnostic::error(
                "nested machine lifetime arguments have no caller declaration custody",
            )]
        })?;
        if explicit.lifetime_arguments.iter().any(|argument| {
            !owner
                .lifetime_parameters
                .iter()
                .any(|parameter| parameter == argument)
        }) {
            return Err(vec![Diagnostic::error(
                "nested machine lifetime argument is not declared in its lexical owner",
            )]);
        }
        Ok(machine
            .lifetime_parameters
            .iter()
            .zip(&explicit.lifetime_arguments)
            .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                parameter: parameter.clone(),
                argument: argument.clone(),
            })
            .collect())
    }
}
pub(crate) mod applications {
    use super::{
        CallSelection, CallSite, Candidate, ExpressionHandle, ExpressionNode, StatementNode,
        SymbolHandle, TypedTrees,
    };
    use crate::monomorphization::const_arguments;
    use crate::tables::{expressions_of, expressions_of_mut};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::static_applications::{
        StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
    };

    pub(crate) fn candidate_bindings(
        program: &dyn ::validation::CheckingInput,
        candidate: &Candidate,
    ) -> Vec<StaticMachineBinding> {
        let mut bindings = Vec::new();
        for ((parameter, _), actual) in candidate
            .template
            .type_parameters
            .iter()
            .zip(&candidate.type_bindings)
        {
            if let Some(actual) = actual {
                bindings.push(StaticMachineBinding {
                    parameter: *parameter,
                    value: StaticMachineBindingValue::Type(*actual),
                });
            }
        }
        for (ordinal, ((parameter, _, _), actual)) in candidate
            .template
            .const_parameters
            .iter()
            .zip(&candidate.const_bindings)
            .enumerate()
        {
            if let Some(actual) = actual {
                let value = if let Some(argument) = &candidate.runtime_value_bindings[ordinal] {
                    StaticMachineBindingValue::RuntimeValue {
                        type_reference: *actual,
                        subject: match argument {
                            super::RuntimeValueBinding::Authored(argument) => argument.symbol,
                            _ => SymbolHandle::invalid(),
                        },
                        expression: match argument {
                            super::RuntimeValueBinding::Authored(argument) => {
                                const_arguments::runtime_subject_expression(program, argument)
                                    .unwrap_or_else(ExpressionHandle::invalid)
                            }
                            _ => ExpressionHandle::invalid(),
                        },
                    }
                } else {
                    StaticMachineBindingValue::Const(*actual)
                };
                bindings.push(StaticMachineBinding {
                    parameter: *parameter,
                    value,
                });
            }
        }
        for ((parameter, _, _), actual) in candidate
            .template
            .machine_parameters
            .iter()
            .zip(&candidate.machine_bindings)
        {
            if let Some(actual) = actual {
                bindings.push(StaticMachineBinding {
                    parameter: *parameter,
                    value: StaticMachineBindingValue::Machine(actual.clone()),
                });
            }
        }
        for (parameter, actual) in candidate
            .template
            .evidence_parameters
            .iter()
            .zip(&candidate.evidence_bindings)
        {
            if let Some(actual) = actual {
                bindings.push(StaticMachineBinding {
                    parameter: parameter.binder,
                    value: StaticMachineBindingValue::Conformance(actual.clone()),
                });
            }
        }
        bindings
    }

    pub(crate) fn bind_recorded_tuple(
        program: &TypedTrees,
        candidate: &mut Candidate<'_>,
        tuple: StaticMachineApplicationHandle,
    ) -> bool {
        bind_typed_tuple(
            program,
            candidate,
            expressions_of(program).static_machine_application_bindings(tuple),
        )
    }

    pub(crate) fn bind_typed_tuple(
        program: &TypedTrees,
        candidate: &mut Candidate<'_>,
        bindings: &[StaticMachineBinding],
    ) -> bool {
        for binding in bindings {
            let parameter = binding.parameter;
            match &binding.value {
                StaticMachineBindingValue::Type(actual) => {
                    let Some(slot) = (candidate.template.type_parameters.iter())
                        .position(|(symbol, _)| *symbol == parameter)
                    else {
                        return false;
                    };
                    candidate.type_bindings[slot] = Some(*actual);
                }
                StaticMachineBindingValue::Const(actual) => {
                    let Some(slot) = (candidate.template.const_parameters.iter())
                        .position(|(symbol, _, _)| *symbol == parameter)
                    else {
                        return false;
                    };
                    candidate.const_bindings[slot] = Some(*actual);
                }
                StaticMachineBindingValue::Machine(argument) => {
                    let Some(slot) = (candidate.template.machine_parameters.iter())
                        .position(|(symbol, _, _)| *symbol == parameter)
                    else {
                        return false;
                    };
                    candidate.machine_bindings[slot] = Some(argument.clone());
                }
                StaticMachineBindingValue::Conformance(argument) => {
                    let Some(slot) = (candidate.template.evidence_parameters.iter())
                        .position(|bound| bound.binder == parameter)
                    else {
                        return false;
                    };
                    candidate.evidence_bindings[slot] = Some(argument.clone());
                }
                StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                    expression,
                } => {
                    if subject.is_valid()
                        || expression.is_valid()
                        || !program
                            .data_type_parameter_by_symbol(parameter)
                            .is_some_and(|parameter| {
                                matches!(parameter.kind, TypeParameterKind::Value { .. })
                            })
                    {
                        return false;
                    }
                    let Some(slot) = candidate
                        .template
                        .const_parameters
                        .iter()
                        .position(|(symbol, _, _)| *symbol == parameter)
                    else {
                        return false;
                    };
                    candidate.const_bindings[slot] = Some(*type_reference);
                    candidate.runtime_value_bindings[slot] =
                        Some(super::RuntimeValueBinding::Carrier);
                }
            }
        }
        true
    }

    pub(crate) fn retain_authored_call(
        program: &mut TypedTrees,
        site: CallSite,
        lexical_owner: SymbolHandle,
    ) -> StaticMachineApplicationHandle {
        let (application, target_symbol, target, arguments) = match site {
            CallSite::Expression(handle) => {
                let Some(call) = expressions_of(program).call_node(handle) else {
                    return StaticMachineApplicationHandle::invalid();
                };
                if let Some(query) = &call.native_code_layout {
                    (
                        call.authored_machine_application,
                        query.operand.symbol,
                        query
                            .operand
                            .path
                            .last()
                            .cloned()
                            .unwrap_or_else(|| call.target.clone()),
                        query
                            .operand
                            .application
                            .as_ref()
                            .map(|application| application.arguments.to_vec())
                            .unwrap_or_default(),
                    )
                } else {
                    (
                        call.authored_machine_application,
                        call.target_symbol,
                        call.target.clone(),
                        call.machine_arguments.to_vec(),
                    )
                }
            }
            CallSite::Statement(handle) => {
                let Some(call) = program.statement_table.statement(handle).as_call() else {
                    return StaticMachineApplicationHandle::invalid();
                };
                (
                    call.authored_machine_application,
                    call.target_symbol,
                    call.target.clone(),
                    call.machine_arguments.to_vec(),
                )
            }
        };
        if application.is_valid() {
            let retained = expressions_of_mut(program).static_machine_application_mut(application);
            if !retained.lexical_owner.is_valid() {
                retained.lexical_owner = lexical_owner;
            }
            if !retained.registration_operation.is_valid() {
                retained.registration_operation = target_symbol;
            }
            application
        } else {
            let application = expressions_of_mut(program).retain_static_machine_application(
                target_symbol,
                target,
                &arguments,
            );
            expressions_of_mut(program)
                .static_machine_application_mut(application)
                .lexical_owner = lexical_owner;
            match site {
                CallSite::Expression(handle) => {
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        unreachable!()
                    };
                    call.authored_machine_application = application;
                }
                CallSite::Statement(handle) => {
                    let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    else {
                        unreachable!()
                    };
                    call.authored_machine_application = application;
                }
            }
            application
        }
    }

    pub(crate) fn retain_call(
        program: &mut TypedTrees,
        candidate: &Candidate,
        selection: &CallSelection,
    ) {
        let application = retain_authored_call(program, selection.site, selection.lexical_owner);
        if !application.is_valid() {
            return;
        }
        let mut bindings = candidate_bindings(program, candidate);
        let parameters = program
            .machine_by_symbol(candidate.template.template_symbol)
            .map(|declaration| program.machine_type_parameters(declaration).to_vec())
            .unwrap_or_default();
        let arguments = expressions_of(program)
            .static_machine_application_arguments(application)
            .to_vec();
        for (parameter, argument) in parameters.iter().zip(&arguments) {
            if !matches!(parameter.kind, TypeParameterKind::Const { .. })
                || bindings
                    .iter()
                    .any(|binding| binding.parameter == parameter.symbol)
            {
                continue;
            }
            if let Ok(actual) =
                program.static_operand_type_reference_in_scope(argument, selection.lexical_owner)
            {
                bindings.push(StaticMachineBinding {
                    parameter: parameter.symbol,
                    value: StaticMachineBindingValue::Const(actual),
                });
            }
        }
        let runtime = super::runtime_value_subjects(&candidate.runtime_value_bindings);
        let operands = super::body_rewriting::runtime_call_operands(
            program,
            selection.site,
            selection.lexical_state,
            &runtime,
        );
        let mut operands = operands.into_iter();
        for binding in &mut bindings {
            if let StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } = &mut binding.value
            {
                let Some(operand) = operands.next() else {
                    break;
                };
                if !subject.is_valid() {
                    *expression = operand;
                }
            }
        }
        expressions_of_mut(program)
            .static_machine_application_mut(application)
            .binding_declaration = candidate.template.template_symbol;
        expressions_of_mut(program).set_static_machine_application_bindings(application, &bindings);
    }

    pub(crate) fn retain_instance(
        source: Option<&dyn validation::CheckingInput>,
        program: &mut TypedTrees,
        candidate: &Candidate,
        instance: SymbolHandle,
    ) -> Result<StaticMachineApplicationHandle, Vec<super::Diagnostic>> {
        let mut bindings = candidate_bindings(program, candidate);
        let (lexical_owner, lifetime_declaration, lifetimes) =
            if candidate.binding_application.is_valid() {
                let origin = source.unwrap_or(program);
                let source_application = expressions_of(origin)
                    .static_machine_application(candidate.binding_application);
                let mut lifetimes = expressions_of(origin)
                    .static_machine_application_lifetimes(candidate.binding_application)
                    .to_vec();
                let source_declaration = source_application.lifetime_binding_declaration;
                let target_declaration = candidate.template.template_symbol;
                if source_declaration != target_declaration
                    && origin.target_machine_families.iter().any(|family| {
                        let contains = |symbol| {
                            family.canonical == symbol
                                || family.members.iter().any(|member| member.machine == symbol)
                        };
                        contains(source_declaration) && contains(target_declaration)
                    })
                {
                    let source_machine =
                        origin
                            .machine_by_symbol(source_declaration)
                            .ok_or_else(|| {
                                vec![super::Diagnostic::error(
                                    "family lifetime tuple lost its source declaration",
                                )]
                            })?;
                    let target_machine =
                        origin
                            .machine_by_symbol(target_declaration)
                            .ok_or_else(|| {
                                vec![super::Diagnostic::error(
                                    "family lifetime tuple lost its target declaration",
                                )]
                            })?;
                    if source_machine.lifetime_parameters.len()
                        != target_machine.lifetime_parameters.len()
                    {
                        return Err(vec![super::Diagnostic::error(
                            "family lifetime telescopes have incompatible positions",
                        )]);
                    }
                    for binding in &mut lifetimes {
                        let ordinal = source_machine
                            .lifetime_parameters
                            .iter()
                            .position(|parameter| parameter == &binding.parameter)
                            .ok_or_else(|| {
                                vec![super::Diagnostic::error(
                                    "family lifetime tuple names no source binder",
                                )]
                            })?;
                        binding.parameter = target_machine.lifetime_parameters[ordinal].clone();
                    }
                } else {
                    lifetimes = origin
                        .static_application_lifetimes_for_declaration(
                            candidate.binding_application,
                            target_declaration,
                        )
                        .map_err(|reason| vec![super::Diagnostic::error(reason)])?
                        .into_owned();
                }
                (
                    source_application.lexical_owner,
                    target_declaration,
                    lifetimes,
                )
            } else {
                (instance, candidate.template.template_symbol, Vec::new())
            };
        for binding in &mut bindings {
            if let StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } = &mut binding.value
            {
                *subject = SymbolHandle::invalid();
                *expression = ExpressionHandle::invalid();
            }
        }
        if source.is_some() {
            super::body_cloning::copy_binding_type_payloads(source, program, &mut bindings, &[])?;
        }
        let application = expressions_of_mut(program).retain_static_machine_application(
            candidate.template.template_symbol,
            typed_trees::name::Identifier::generated(&candidate.template.template_name),
            &[],
        );
        expressions_of_mut(program)
            .static_machine_application_mut(application)
            .lexical_owner = lexical_owner;
        expressions_of_mut(program)
            .static_machine_application_mut(application)
            .binding_declaration = candidate.template.template_symbol;
        expressions_of_mut(program).set_static_machine_application_bindings(application, &bindings);
        expressions_of_mut(program).set_static_machine_application_lifetimes(
            application,
            lifetime_declaration,
            &lifetimes,
        );
        let contract_source = if source.is_none() {
            program
                .applied_machine_callable_origin(candidate.binding_application)
                .unwrap_or_default()
        } else {
            StaticMachineApplicationHandle::invalid()
        };
        expressions_of_mut(program)
            .static_machine_application_mut(application)
            .callable_contract_source = contract_source;
        Ok(application)
    }
}
pub(crate) mod candidate {
    use super::{CalleeState, Candidate, CandidateTemplate};
    use typed_trees::data::TypeParameterKind;

    pub(crate) fn from_machine(
        program: &dyn ::validation::CheckingInput,
        machine_index: usize,
    ) -> Candidate<'static> {
        let machine = &program.machines()[machine_index];
        from_parameters(
            program,
            machine_index,
            program.machine_type_parameters(machine),
            machine.conformance_bounds.clone(),
        )
    }

    pub(crate) fn from_capture(
        program: &dyn ::validation::CheckingInput,
        machine_index: usize,
        conformance: &typed_trees::trait_definition::Conformance,
    ) -> Candidate<'static> {
        from_parameters(
            program,
            machine_index,
            program.conformance_type_parameters(conformance),
            Vec::new(),
        )
    }

    fn from_parameters(
        program: &dyn ::validation::CheckingInput,
        machine_index: usize,
        parameters: &[typed_trees::data::TypeParameter],
        evidence_parameters: Vec<typed_trees::machine::GenericConformanceBound>,
    ) -> Candidate<'static> {
        let machine = &program.machines()[machine_index];
        let mut type_parameters = Vec::new();
        let mut const_parameters = Vec::new();
        let mut value_const_parameters = Vec::new();
        let mut machine_parameters = Vec::new();
        for parameter in parameters {
            match &parameter.kind {
                TypeParameterKind::Type => {
                    type_parameters.push((parameter.symbol, parameter.name.as_str().to_owned()));
                }
                TypeParameterKind::Const { type_reference } => const_parameters.push((
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    *type_reference,
                )),
                TypeParameterKind::Value { type_reference } => {
                    value_const_parameters.push(const_parameters.len());
                    const_parameters.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        *type_reference,
                    ));
                }
                TypeParameterKind::Machine { contract } => {
                    let signature = program
                        .machine_parameter_contract_view(contract)
                        .expect("typed machine parameter has a contract")
                        .signature();
                    machine_parameters.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        signature.clone(),
                    ));
                }
                TypeParameterKind::Implementation | TypeParameterKind::Proposition { .. } => {}
            }
        }
        Candidate {
            binding_application:
                typed_trees::static_applications::StaticMachineApplicationHandle::invalid(),
            type_bindings: vec![None; type_parameters.len()],
            const_bindings: vec![None; const_parameters.len()],
            runtime_value_bindings: vec![None; const_parameters.len()],
            machine_bindings: vec![None; machine_parameters.len()],
            evidence_bindings: vec![None; evidence_parameters.len()],
            template: std::borrow::Cow::Owned(CandidateTemplate {
                machine_index,
                template_symbol: machine.symbol,
                template_name: machine.name.as_str().to_owned(),
                state_symbols: program
                    .machine_states(machine)
                    .iter()
                    .map(|state| state.symbol)
                    .collect(),
                type_parameters,
                const_parameters,
                value_const_parameters,
                machine_parameters,
                evidence_parameters,
            }),
            conflicted: false,
        }
    }

    pub(crate) fn collect(program: &dyn ::validation::CheckingInput) -> Vec<Candidate<'static>> {
        program
            .machines()
            .iter()
            .enumerate()
            .filter(|(_, machine)| machine.has_specialization_parameters())
            .map(|(machine_index, _)| from_machine(program, machine_index))
            .collect()
    }

    pub(crate) fn callees(
        program: &dyn ::validation::CheckingInput,
        candidates: &[Candidate],
    ) -> Vec<CalleeState> {
        candidates
            .iter()
            .enumerate()
            .flat_map(|(candidate_index, candidate)| {
                program
                    .machine_states(&program.machines()[candidate.template.machine_index])
                    .iter()
                    .map(move |state| {
                        let parameters = program.state_parameters(state);
                        CalleeState {
                            symbol: state.symbol,
                            candidate_index,
                            return_type: state.return_type,
                            parameter_types: parameters
                                .iter()
                                .map(|parameter| parameter.type_reference)
                                .collect(),
                        }
                    })
            })
            .collect()
    }
}
pub(crate) mod captured_rows {
    use super::requirement_applications::selected_applications;
    use super::{CallSite, Diagnostic, SymbolHandle, TypedTrees, failed};
    use crate::expression_roots::append_statement_roots;
    use crate::tables::{expressions_of, expressions_of_mut};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::product_operand::ProductApplicationDemand;
    use typed_trees::static_applications::StaticMachineBindingValue;
    use typed_trees::static_applications::StaticMachineLifetimeBinding;
    use validation::contract_identities::{
        accepted_template_commitment, normalized_machine_identity,
    };

    macro_rules! err {
        ($message:literal) => {
            vec![Diagnostic::error($message)]
        };
    }

    #[derive(Clone, Copy)]
    enum CaptureSupply {
        AuthoredSelection,
        CarriedCapture,
    }

    pub(crate) fn close(program: &mut TypedTrees) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
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
                    if let Some(call) = expressions_of(program).call_node(handle) {
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
        selected_roots.extend(
            program
                .live_product_operand_applications()
                .into_iter()
                .filter_map(|handle| {
                    let selected = program.product_operand_applications.get(handle);
                    if selected.demand != ProductApplicationDemand::Selected {
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
                        expressions_of_mut(program).expression_mut(handle)
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
        if !conformance.rows.iter().any(|row| {
            row.source.owns_realization() && row.realization_machine == realization_machine
        }) || program.declaration_scope_parent(realization_machine) != conformance.symbol
        {
            return Err(err!("selected capture has no exact owned realization"));
        }
        let supplying_owner = expressions_of(program)
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
                    && expressions_of(program)
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
                    .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
                    .count();
                let selected_callbacks = if application.callable_application.is_valid() {
                    expressions_of(program)
                        .static_machine_application_selected_machines(
                            application.callable_application,
                        )
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
                    if matches!(parameter.kind, TypeParameterKind::Machine { .. }) {
                        *argument = selected_callbacks[callback_ordinal].clone();
                        callback_ordinal += 1;
                    }
                }
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
                    match &parameter.kind {
                        TypeParameterKind::Type => {
                            let reference = program
                                .static_operand_type_reference(argument)
                                .map_err(|message| vec![Diagnostic::error(message)])?;
                            types.push((0, type_ordinal, reference));
                            type_ordinal += 1;
                        }
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                            let reference = match application.const_arguments.get(const_ordinal) {
                                Some(typed_trees::typed_trees::ClosedConformanceConstArgument::Evaluated {
                                    value, ..
                                }) => {
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
                for (parameter, original) in program
                    .conformance_type_parameters(&conformance)
                    .iter()
                    .zip(application.arguments.iter())
                {
                    if matches!(parameter.kind, TypeParameterKind::Machine { .. }) {
                        let binding = bindings
                            .iter_mut()
                            .find(|binding| binding.parameter == parameter.symbol)
                            .ok_or_else(|| err!("capture lost its declared Machine binder"))?;
                        binding.value = StaticMachineBindingValue::Machine(original.clone());
                    }
                }
                let binding = expressions_of_mut(program).retain_static_machine_application(
                    conformance.symbol,
                    conformance.alias.clone().unwrap_or_default(),
                    &application.arguments,
                );
                let retained = expressions_of_mut(program).static_machine_application_mut(binding);
                retained.lexical_owner = supplying_owner;
                retained.binding_declaration = conformance.symbol;
                expressions_of_mut(program)
                    .set_static_machine_application_bindings(binding, &bindings);
                let lifetimes = conformance
                    .lifetime_parameters
                    .iter()
                    .zip(&application.lifetime_arguments)
                    .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                        parameter: parameter.clone(),
                        argument: typed_trees::name::Identifier::generated(argument),
                    })
                    .collect::<Vec<_>>();
                expressions_of_mut(program).set_static_machine_application_lifetimes(
                    binding,
                    conformance.symbol,
                    &lifetimes,
                );
                expressions_of_mut(program)
                    .set_static_machine_application_selected_machines(binding, &selected_callbacks);
                candidate.binding_application = binding;
                let machine = &program.machines()[index];
                let identity = normalized_machine_identity(program, machine).ok_or_else(|| {
                    err!("owned capture body has no normalized declaration identity")
                })?;
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
                            && expressions_of(program)
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
}
pub(crate) mod cleanup_applications {
    use super::{CalleeState, Diagnostic, SymbolHandle, TypedTrees, failed};
    use crate::tables::{expressions_of, expressions_of_mut};
    use validation::contract_identities::accepted_template_commitment;
    use validation::contract_identities::normalized_machine_identity;

    pub(crate) fn materialize(
        program: &mut TypedTrees,
    ) -> Result<Vec<SymbolHandle>, Vec<Diagnostic>> {
        close_data_applications(program)?;
        let requests = program.cleanup_selections().copied().collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut created = Vec::new();
        for request in requests {
            let application = expressions_of(program)
                .static_machine_application(request.application)
                .clone();
            let Some(machine_index) =
                program.machine_position_by_symbol(application.registration_operation)
            else {
                return Err(vec![Diagnostic::error(
                    "cleanup application does not select an independent machine",
                )]);
            };
            let machine = program.machines()[machine_index].clone();
            let Some(entry) = program.machine_states(&machine).first().cloned() else {
                return Err(vec![Diagnostic::error(
                    "selected cleanup machine has no entry",
                )]);
            };
            let mut candidate = super::candidate::from_machine(program, machine_index);
            candidate.binding_application = request.application;
            let arguments = expressions_of(program)
                .static_machine_application_arguments(request.application)
                .to_vec();
            let callee = CalleeState {
                symbol: entry.symbol,
                candidate_index: 0,
                return_type: entry.return_type,
                parameter_types: program
                    .state_parameters(&entry)
                    .iter()
                    .map(|parameter| parameter.type_reference)
                    .collect(),
            };
            let mut proposals = super::selection::StaticProposals::default();
            super::selection::collect_machine_proposals_for_callee(
                program,
                std::slice::from_ref(&candidate),
                &callee,
                &arguments,
                None,
                &mut proposals,
            )
            .map_err(|_| {
                vec![Diagnostic::error(
                    "cleanup application supplies arguments beyond its declared telescope",
                )]
            })?;
            let bindings = super::selection::proposal_bindings(program, &candidate, proposals);
            let open = bindings.unresolved_type_parameters
                || bindings.unresolved_const_parameters
                || bindings.unresolved_machine_parameters
                || bindings.unresolved_evidence_parameters;
            candidate.type_bindings = bindings.type_bindings;
            candidate.const_bindings = bindings.const_bindings;
            candidate.runtime_value_bindings = bindings.runtime_value_bindings;
            candidate.machine_bindings = bindings.machine_bindings;
            candidate.evidence_bindings = bindings.evidence_bindings;
            candidate.conflicted = bindings.conflicted;
            if candidate.conflicted
                || candidate.type_bindings.iter().any(Option::is_none)
                || candidate.const_bindings.iter().any(Option::is_none)
                || candidate.machine_bindings.iter().any(Option::is_none)
                || candidate.evidence_bindings.iter().any(Option::is_none)
            {
                return Err(vec![Diagnostic::error(format!(
                    "cleanup application `{}` must supply a complete consistent static tuple",
                    machine.name.as_str(),
                ))]);
            }
            if candidate.runtime_value_bindings.iter().any(Option::is_some) {
                return Err(vec![Diagnostic::error(
                    "cleanup application cannot require runtime arguments beyond its explicit value parameter",
                )]);
            }
            let retained = super::applications::candidate_bindings(program, &candidate);
            expressions_of_mut(program)
                .set_static_machine_application_bindings(request.application, &retained);
            if open {
                selected.push((request.application, SymbolHandle::invalid()));
                continue;
            }
            let executable = if program.machine_type_parameters(&machine).is_empty()
                && machine.conformance_bounds.is_empty()
            {
                machine.symbol
            } else if let Some(instance) =
                super::saved_calls::candidate_instance(program, program, &candidate)
            {
                instance.instance
            } else {
                let ordinal = program
                    .machine_specializations
                    .iter()
                    .filter(|instance| instance.template == machine.symbol)
                    .count();
                let identity = normalized_machine_identity(program, &machine).ok_or_else(|| {
                    failed("cleanup template has no normalized callable identity")
                })?;
                let commitment = accepted_template_commitment(program, machine_index);
                let states = super::body_cloning::clone_specialized_machine(
                    None, program, &candidate, ordinal, identity, commitment,
                )?;
                let entry = states
                    .iter()
                    .find_map(|(source, target)| (*source == entry.symbol).then_some(*target))
                    .ok_or_else(|| {
                        failed("cleanup specialization lost its entry correspondence")
                    })?;
                let instance = program
                    .machine_holding_state(entry)
                    .ok_or_else(|| failed("cleanup specialization entry lost its machine"))?
                    .symbol;
                created.push(instance);
                instance
            };
            selected.push((request.application, executable));
        }
        let definitions = program.roots.data_definitions;
        for data in program
            .tables
            .data_definitions
            .span_mut_or_empty(definitions)
        {
            if let Some(selection) = &mut data.cleanup_selection
                && let Some((_, executable)) = selected
                    .iter()
                    .find(|(application, _)| *application == selection.application)
            {
                selection.executable = *executable;
            }
        }
        let definitions = program.roots.domain_definitions;
        for domain in program
            .tables
            .domain_definitions
            .span_mut_or_empty(definitions)
        {
            if let Some(selection) = &mut domain.cleanup_selection
                && let Some((_, executable)) = selected
                    .iter()
                    .find(|(application, _)| *application == selection.application)
            {
                selection.executable = *executable;
            }
        }
        for application in &mut program.applied_domain_applications {
            if let Some(selection) = &mut application.theory.cleanup_selection
                && let Some((_, executable)) = selected
                    .iter()
                    .find(|(application, _)| *application == selection.application)
            {
                selection.executable = *executable;
            }
        }
        for application in &mut program.applied_data_cleanup_selections {
            if let Some((_, executable)) = selected
                .iter()
                .find(|(selected, _)| *selected == application.selection.application)
            {
                application.selection.executable = *executable;
            }
        }
        Ok(created)
    }

    fn close_data_applications(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let templates = program
            .data_definitions()
            .iter()
            .filter(|data| data.cleanup_selection.is_some() && !data.type_parameters.is_empty())
            .map(|data| data.symbol)
            .collect::<Vec<_>>();
        if templates.is_empty() {
            return Ok(());
        }
        let carriers = program
            .type_reference_table
            .type_reference_rows()
            .filter(|(_, node)| {
                matches!(node, typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
                if templates.contains(base_symbol))
            })
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        for reference in carriers {
            if program
                .closed_data_definition(reference)
                .is_some_and(|data| data.generic_instance.is_some())
            {
                continue;
            }
            program
                .prepare_closed_data_cleanup_application(reference)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
        Ok(())
    }
}
pub(crate) use crate::contextual_and_requirement_calls::contextual_calls;
pub(crate) use contextual_calls::prepare_conformance_occurrence;
pub(crate) use contextual_calls::prepare_installed_interrupts;
pub(crate) mod domain_applications {
    use super::{Diagnostic, SymbolHandle, TypedTrees};
    use typed_trees::data::TypeParameterKind;
    use typed_trees::domain::has_generic_carrier;
    use typed_trees::domain::indexed_domain_instance_name;
    use typed_trees::references::DeclarationSelection;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

    pub(crate) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        loop {
            let mut requests = Vec::new();
            for occurrence in typed_trees::references::collect_program_type_occurrences(program) {
                if let TypeReferenceNode::Constrained { constraints, .. } = program
                    .type_reference_table
                    .type_reference(occurrence.reference)
                {
                    requests.extend(
                        program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .filter_map(|constraint| match constraint {
                                TypeConstraintNode::Domain(domain) => Some((
                                    occurrence.lexical_owner,
                                    domain.symbol,
                                    domain.arguments.clone(),
                                )),
                                _ => None,
                            }),
                    );
                }
            }
            for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
                if let Some(membership) = fact.as_membership() {
                    requests.push((
                        owner,
                        membership.domain_symbol,
                        program
                            .type_reference_table
                            .type_reference_handles(membership.domain_arguments)
                            .to_vec(),
                    ));
                }
            }
            let mut changed = false;
            for (lexical_owner, symbol, arguments) in requests {
                let Some(domain) = program.domain_definition(symbol).cloned() else {
                    continue;
                };
                if domain.type_parameters.is_empty() || has_generic_carrier(program, &domain) {
                    continue;
                }
                let parameters = program.domain_type_parameters(&domain).to_vec();
                if arguments.len() != parameters.len() {
                    return Err(vec![Diagnostic::error(
                        "domain application has an incomplete explicit telescope",
                    )]);
                }
                if arguments.iter().any(|argument| {
                    !typed_trees::domain::index_argument_is_closed(program, *argument)
                }) {
                    continue;
                }
                let mut canonical = true;
                let mut closed_arguments = arguments.clone();
                for (index, (parameter, argument)) in parameters.iter().zip(&arguments).enumerate()
                {
                    if let TypeParameterKind::Const {
                        type_reference: carrier,
                    } = parameter.kind
                    {
                        match program.type_reference_table.type_reference(*argument) {
                            TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid() => {}
                            TypeReferenceNode::ConstExpression(expression)
                                if lexical_owner.is_valid()
                                    && typed_trees::domain::index_argument_is_closed(
                                        program, carrier,
                                    ) =>
                            {
                                let Some(value) = validation::check_closed_const_expression(
                                    program,
                                    lexical_owner,
                                    *expression,
                                    carrier,
                                )?
                                else {
                                    canonical = false;
                                    break;
                                };
                                let spelling = match value.decode_encoding() {
                                Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
                                Some(_) => value.atom(),
                                None => return Err(vec![Diagnostic::error("closed domain argument lost its canonical value")]),
                            };
                                closed_arguments[index] =
                                    program
                                        .type_reference_table
                                        .insert(TypeReferenceNode::Named {
                                            symbol: SymbolHandle::invalid(),
                                            name: typed_trees::name::Identifier::generated(
                                                spelling,
                                            ),
                                        });
                            }
                            _ => {
                                canonical = false;
                                break;
                            }
                        }
                    }
                }
                if !canonical {
                    continue;
                }
                let identity =
                    indexed_domain_instance_name(program, &domain, &parameters, &arguments)
                        .map_err(|error| vec![error])?;
                let identity = program.semantic_domains.intern(&identity);
                changed |= program
                    .prepare_closed_domain_application(
                        symbol,
                        &arguments,
                        &closed_arguments,
                        identity,
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?;
            }
            if !changed {
                return Ok(());
            }
            super::finalize_domain_instance_identities(program, &DeclarationSelection::all())
                .map_err(|error| vec![error])?;
        }
    }
}
pub(crate) mod explicit_arguments {
    use super::{Candidate, Diagnostic, StaticMachineArgument, SymbolHandle, TypedTrees};
    use typed_trees::state::State;

    pub(crate) fn bind(
        program: &TypedTrees,
        provider: &mut Candidate<'_>,
        state: &State,
        selected: SymbolHandle,
        arguments: &[StaticMachineArgument],
    ) -> Result<(), Vec<Diagnostic>> {
        let callee = super::CalleeState {
            symbol: selected,
            candidate_index: 0,
            return_type: state.return_type,
            parameter_types: program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect(),
        };
        let mut proposals = super::selection::StaticProposals::default();
        if super::selection::collect_machine_proposals_for_callee(
            program,
            std::slice::from_ref(provider),
            &callee,
            arguments,
            None,
            &mut proposals,
        )
        .is_err()
        {
            return Err(vec![Diagnostic::error(
                "nested machine application supplies an argument beyond its declared telescope",
            )]);
        }
        let supplied = super::selection::proposal_bindings(program, provider, proposals);
        provider.type_bindings = supplied.type_bindings;
        provider.const_bindings = supplied.const_bindings;
        provider.runtime_value_bindings = supplied.runtime_value_bindings;
        provider.machine_bindings = supplied.machine_bindings;
        provider.evidence_bindings = supplied.evidence_bindings;
        provider.conflicted = supplied.conflicted;
        Ok(())
    }
}
pub(crate) mod layout_storage_policies {
    use super::{Diagnostic, TypedTrees};
    use crate::tables::expressions_of_mut;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
    use validation::conformance_applications::close_conformance_application;

    pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let occurrences =
            typed_trees::references::collect_authored_program_type_occurrences(program);
        let mut owners = program
            .data_implementation_applications
            .iter()
            .map(|(_, selected)| selected.lexical_owner)
            .collect::<Vec<_>>();
        owners.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
        owners.dedup();
        let source_inputs = typed_trees::references::authored_type_occurrences(program, &owners)
            .into_iter()
            .map(|occurrence| (occurrence.lexical_owner, occurrence.reference))
            .collect::<std::collections::HashSet<_, symbols::BuildSymbolHasher>>();
        let retained = program
            .data_implementation_applications
            .iter()
            .filter(|(_, selected)| {
                source_inputs.contains(&(selected.lexical_owner, selected.source_application))
            })
            .map(|(_, selected)| selected.clone())
            .collect::<Vec<_>>();
        let mut applications = arena::Arena::new();
        applications.insert_many(retained);
        program.data_implementation_applications = applications;
        for occurrence in occurrences {
            let Some(storage) = program.layout_storage_arguments(occurrence.reference) else {
                continue;
            };
            if program
                .data_implementation_applications
                .iter()
                .any(|(_, selected)| {
                    selected.source_application == occurrence.reference
                        && selected.lexical_owner == occurrence.lexical_owner
                })
            {
                continue;
            }
            let operand = program
                .static_operand_from_type_reference(storage.policy)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if !operand.symbol.is_valid()
                || program.symbols.get(operand.symbol).kind
                    == symbols::SymbolKind::ConformanceParameter
            {
                continue;
            }
            let application = match close_conformance_application(program, &operand) {
                Ok(application) => application,
                Err(_) => continue,
            };
            let constructor = program
                .type_reference_table
                .type_symbol(storage.application);
            let parameters = program
                .data_definition_by_symbol(constructor)
                .map(|data| program.data_type_parameters(data).to_vec())
                .unwrap_or_default();
            let Some(parameter) = parameters.iter().find(|parameter| {
                parameter.kind == typed_trees::data::TypeParameterKind::Implementation
            }) else {
                continue;
            };
            let parameter = parameter.symbol;
            let supplying = expressions_of_mut(program).retain_static_machine_application(
                constructor,
                typed_trees::name::Identifier::generated("LayoutStorage"),
                std::slice::from_ref(&operand),
            );
            let source = expressions_of_mut(program).static_machine_application_mut(supplying);
            source.lexical_owner = occurrence.lexical_owner;
            source.binding_declaration = constructor;
            expressions_of_mut(program).set_static_machine_application_bindings(
                supplying,
                &[StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Conformance(operand),
                }],
            );
            program.data_implementation_applications.insert(
                typed_trees::typed_trees::DataImplementationApplication {
                    source_application: occurrence.reference,
                    lexical_owner: occurrence.lexical_owner,
                    parameter,
                    argument: storage.policy,
                    supplying_application: supplying,
                    application,
                },
            );
        }
        Ok(())
    }
}
pub(crate) mod native_query_lifetimes {
    use super::{CallSite, Diagnostic, ExpressionNode, TypedTrees};
    use crate::expression_roots::append_statement_roots;
    use crate::tables::{expressions_of, expressions_of_mut};

    pub(crate) fn retain(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let mut queries = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let mut expressions = Vec::new();
                append_statement_roots(program, state, &mut expressions);
                for expression in expressions {
                    if let ExpressionNode::Call(call) =
                        expressions_of(program).expression(expression)
                        && let Some(query) = &call.native_code_layout
                    {
                        queries.push((expression, machine.symbol, query.operand.clone()));
                    }
                }
            }
        }
        for (expression, owner, operand) in queries {
            let application = super::applications::retain_authored_call(
                program,
                CallSite::Expression(expression),
                owner,
            );
            let lifetimes =
                super::application_lifetimes::bind(program, operand.symbol, &operand, application)?;
            let declaration = program
                .machines()
                .iter()
                .find(|machine| program.machine_or_state_symbol(machine, operand.symbol))
                .map(|machine| machine.symbol)
                .unwrap_or(operand.symbol);
            if !expressions_of(program)
                .static_machine_application(application)
                .binding_declaration
                .is_valid()
            {
                expressions_of_mut(program)
                    .static_machine_application_mut(application)
                    .binding_declaration = declaration;
            }
            expressions_of_mut(program).set_static_machine_application_lifetimes(
                application,
                declaration,
                &lifetimes,
            );
        }
        Ok(())
    }
}
pub(crate) mod provider_tuple_closure {
    use super::{Candidate, TypedTrees};

    pub(crate) fn operands_are_open(program: &TypedTrees, provider: &Candidate<'_>) -> bool {
        provider
            .type_bindings
            .iter()
            .chain(&provider.const_bindings)
            .flatten()
            .any(|reference| super::contextual_calls::operand_is_open(program, *reference))
            || provider.machine_bindings.iter().flatten().any(|argument| {
                matches!(
                    program.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::MachineParameter
                ) || argument.application.is_some()
            })
            || provider.evidence_bindings.iter().flatten().any(|argument| {
                super::contextual_calls::argument_is_open(program, argument)
                    || validation::conformance_applications::close_conformance_application(
                        program, argument,
                    )
                    .is_err()
            })
    }

    pub(crate) fn operands_are_closed(program: &TypedTrees, provider: &Candidate<'_>) -> bool {
        !(provider.conflicted
            || provider.type_bindings.iter().any(Option::is_none)
            || provider.const_bindings.iter().any(Option::is_none)
            || provider.machine_bindings.iter().any(Option::is_none)
            || provider.evidence_bindings.iter().any(Option::is_none))
            && !operands_are_open(program, provider)
    }
}
pub(crate) use crate::selection_and_reflection::reflection;
pub(crate) mod requirement_providers {
    use super::{Diagnostic, TypedTrees, candidate, failed, saved_calls};
    use crate::tables::expressions_of;
    use typed_trees::trait_definition::AppliedTraitScope;
    use validation::contract_identities::{
        accepted_template_commitment, normalized_machine_identity,
    };

    pub(crate) fn generate_requirement_provider_specializations(
        program: &mut TypedTrees,
    ) -> Result<usize, Vec<Diagnostic>> {
        let mut demands = Vec::new();
        for (index, machine) in program.machines().iter().enumerate() {
            if !machine.supplies_checked_body() || !machine.has_specialization_parameters() {
                continue;
            }
            let mut requirements = Vec::new();
            for conformance in program.machine_trait_conformances(machine) {
                if conformance.external_binding.is_some() || conformance.via_expression.is_valid() {
                    continue;
                }
                let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                }) = typed_trees::machine::resolve_satisfied_declaration(
                    program,
                    machine,
                    conformance,
                )
                else {
                    continue;
                };
                if !definition.is_boundary {
                    continue;
                }
                if let Some(scope) =
                    AppliedTraitScope::from_machine_conformance(program, machine, conformance)
                {
                    requirements.push((requirement.symbol, scope));
                }
            }
            for conformance in program.conformances() {
                if !conformance.type_parameters.is_empty() {
                    continue;
                }
                let Some(scope) = AppliedTraitScope::from_conformance(program, conformance) else {
                    continue;
                };
                for row in &conformance.rows {
                    if row.realization_machine != machine.symbol {
                        continue;
                    }
                    let Some(declaring) = scope.declaring_scope(program, row.declaring_trait)
                    else {
                        continue;
                    };
                    if declaring.declaration.is_boundary {
                        requirements.push((row.requirement, declaring));
                    }
                }
            }
            for (requirement, scope) in requirements {
                for application in program.closed_requirement_applications() {
                    if application.template == requirement
                        && program.requirement_application_matches_scope(
                            application.signature.symbol,
                            &scope,
                        )
                    {
                        demands.push((index, application.signature.symbol));
                    }
                }
            }
        }
        let mut generated = 0;
        for (index, requirement) in demands {
            let application = program
                .closed_requirement_application(requirement)
                .ok_or_else(|| failed("provider projection lost its exact requirement instance"))?;
            let mut provider = candidate::from_machine(program, index);
            let bindings =
                projected_provider_bindings(program, &program.machines()[index], &application)?;
            if !super::applications::bind_typed_tuple(program, &mut provider, &bindings) {
                return Err(failed(
                    "provider projection does not match its complete declaration telescope",
                ));
            }
            provider.binding_application = application.application;
            if provider.conflicted
                || provider.type_bindings.iter().any(Option::is_none)
                || provider.const_bindings.iter().any(Option::is_none)
                || provider.machine_bindings.iter().any(Option::is_none)
                || provider.evidence_bindings.iter().any(Option::is_none)
            {
                continue;
            }
            if saved_calls::candidate_instance(program, program, &provider).is_some() {
                continue;
            }
            let identity = normalized_machine_identity(program, &program.machines()[index])
                .ok_or_else(|| {
                    failed("boundary provider has no normalized declaration identity")
                })?;
            let commitment = accepted_template_commitment(program, index);
            let ordinal = program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.template == provider.template.template_symbol)
                .count();
            super::body_cloning::clone_specialized_machine(
                None, program, &provider, ordinal, identity, commitment,
            )?;
            generated += 1;
        }
        Ok(generated)
    }

    fn projected_provider_bindings(
        program: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        application: &typed_trees::typed_trees::ClosedRequirementApplication<'_>,
    ) -> Result<Vec<typed_trees::static_applications::StaticMachineBinding>, Vec<Diagnostic>> {
        use typed_trees::data::TypeParameterKind;
        use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};
        use typed_trees::typed_trees::RequirementLocalArgument;
        let (_, declared) = program
            .trait_machine_signature(application.template)
            .ok_or_else(|| failed("provider projection lost its public declaration"))?;
        let public = program.state_signature_type_parameters(declared);
        let private = program.requirement_callable_parameters(
            application.signature,
            &program.machine_callable_parameters(machine),
        );
        let retained =
            expressions_of(program).static_machine_application_bindings(application.application);
        if public.len() != private.len()
            || public.len() != application.local_arguments.len()
            || retained.len() != public.len()
        {
            return Err(failed(
                "provider projection has an incomplete public or private invocation telescope",
            ));
        }
        public
            .iter()
            .zip(private)
            .zip(application.local_arguments)
            .map(|((public, private), argument)| {
                if std::mem::discriminant(&public.kind) != std::mem::discriminant(&private.kind) {
                    return Err(failed(
                        "provider projection changed an invocation binder kind",
                    ));
                }
                let mut source = retained
                    .iter()
                    .filter(|binding| binding.parameter == public.symbol);
                let binding = source
                    .next()
                    .filter(|_| source.next().is_none())
                    .ok_or_else(|| {
                        failed("provider projection has a missing or duplicate source binding")
                    })?;
                let value = match (&private.kind, argument, &binding.value) {
                    (
                        TypeParameterKind::Value { .. },
                        RequirementLocalArgument::RuntimeValue { carrier },
                        StaticMachineBindingValue::RuntimeValue { .. },
                    ) => StaticMachineBindingValue::RuntimeValue {
                        type_reference: *carrier,
                        subject: Default::default(),
                        expression: Default::default(),
                    },
                    (
                        TypeParameterKind::Type,
                        RequirementLocalArgument::Static(actual),
                        StaticMachineBindingValue::Type(_),
                    ) => StaticMachineBindingValue::Type(*actual),
                    (
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                        RequirementLocalArgument::Static(actual),
                        StaticMachineBindingValue::Const(_),
                    ) => StaticMachineBindingValue::Const(*actual),
                    (
                        TypeParameterKind::Machine { .. },
                        RequirementLocalArgument::Static(_),
                        StaticMachineBindingValue::Machine(actual),
                    ) => StaticMachineBindingValue::Machine(actual.clone()),
                    (
                        TypeParameterKind::Implementation,
                        RequirementLocalArgument::Static(_),
                        StaticMachineBindingValue::Conformance(actual),
                    ) => StaticMachineBindingValue::Conformance(actual.clone()),
                    _ => {
                        return Err(failed(
                            "provider projection changed its retained operand role",
                        ));
                    }
                };
                Ok(StaticMachineBinding {
                    parameter: private.symbol,
                    value,
                })
            })
            .collect()
    }
}
pub(crate) use crate::selection_and_reflection::selection;
pub(crate) mod trait_implementation_arguments {
    use super::{Diagnostic, TypedTrees};
    use crate::tables::expressions_of_mut;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::name::Identifier;
    use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

    pub(crate) fn prepare(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let calling_traits =
            typed_trees::calling_vocabulary::standard_calling_vocabularies(program)
                .iter()
                .map(|(calling, policy)| (calling.symbol, policy.symbol))
                .collect::<Vec<_>>();
        if calling_traits.is_empty() {
            return Ok(());
        }
        let mut pending = Vec::new();
        for root in program.applied_trait_roots() {
            root.visit(program, &mut |scope| {
                if !calling_traits
                    .iter()
                    .any(|(calling, _)| *calling == scope.declaration.symbol)
                {
                    return;
                }
                if program.require_receiving_trait_operands(scope).is_err()
                    || scope.portable_application_identity(program).is_none()
                {
                    return;
                }
                for parameter in program.trait_type_parameters(scope.declaration) {
                    if parameter.kind == TypeParameterKind::Implementation
                        && !program
                            .trait_implementation_applications
                            .iter()
                            .any(|(_, selected)| {
                                selected.parameter == parameter.symbol
                                    && selected.source_scope.scope(program).is_some_and(|known| {
                                        known.argument_owner == scope.argument_owner
                                            && known.same_application(program, scope)
                                    })
                            })
                    {
                        pending.push((scope.environment(), parameter.symbol));
                    }
                }
            });
        }
        for (environment, parameter) in pending {
            let scope = environment
                .scope(program)
                .ok_or_else(|| failure("implementation demand lost its source scope"))?;
            if program
                .trait_implementation_applications
                .iter()
                .any(|(_, selected)| {
                    selected.parameter == parameter
                        && selected.source_scope.scope(program).is_some_and(|known| {
                            known.argument_owner == scope.argument_owner
                                && known.same_application(program, &scope)
                        })
                })
            {
                continue;
            }
            let declaration = scope.declaration.symbol;
            let owner = scope.argument_owner;
            let position = program
                .trait_type_parameters(scope.declaration)
                .iter()
                .position(|binder| binder.symbol == parameter)
                .ok_or_else(|| failure("implementation demand lost its declaration slot"))?;
            let arguments = program
                .materialize_trait_application_arguments(&environment)
                .map_err(failure)?;
            let argument = *arguments.get(position).ok_or_else(|| {
                failure("implementation demand has an incomplete trait telescope")
            })?;
            if super::contextual_calls::operand_is_open(program, argument) {
                continue;
            }
            let operand = program
                .static_operand_from_type_reference(argument)
                .map_err(failure)?;
            let application = validation::conformance_applications::close_conformance_application(
                program, &operand,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            if !calling_traits.iter().any(|(calling, policy)| {
                *calling == declaration && *policy == application.trait_definition
            }) {
                continue;
            }
            let source_scope = environment.retain(program).map_err(failure)?;
            let name = Identifier::generated(program.symbols.display_path(declaration, "::"));
            let supplying = expressions_of_mut(program).retain_static_machine_application(
                declaration,
                name,
                std::slice::from_ref(&operand),
            );
            let source = expressions_of_mut(program).static_machine_application_mut(supplying);
            source.lexical_owner = owner;
            source.binding_declaration = declaration;
            expressions_of_mut(program).set_static_machine_application_bindings(
                supplying,
                &[StaticMachineBinding {
                    parameter,
                    value: StaticMachineBindingValue::Conformance(operand),
                }],
            );
            program.trait_implementation_applications.insert(
                typed_trees::typed_trees::TraitImplementationApplication {
                    source_scope,
                    parameter,
                    argument,
                    supplying_application: supplying,
                    application,
                },
            );
        }
        Ok(())
    }

    fn failure(message: &str) -> Vec<Diagnostic> {
        vec![Diagnostic::error(message)]
    }
}
pub(crate) use selection::{BindingContext, StaticProposals, infer_static_bindings};

pub(crate) use crate::body_cloning_and_rewriting::body_cloning;
pub(crate) use crate::body_cloning_and_rewriting::body_rewriting;
pub(crate) use body_cloning::copy_type_reference;
pub use body_rewriting::restore_copied_call_sources;
pub(crate) use body_rewriting::{collect_evidence_requirement_closure, insert_subject_name};
pub(crate) mod const_arguments {
    use symbols::SymbolKind;
    use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    pub(crate) use validation::expression_traversal::collect_type_expressions;
    pub(crate) use validation::static_arguments::{
        forwarded_type, is_runtime_value_expression, is_runtime_value_subject, spelling,
    };

    pub(crate) fn is_runtime_value_argument(
        program: &dyn validation::CheckingInput,
        argument: &StaticMachineArgument,
    ) -> bool {
        program.static_const_argument_identity(argument).is_none()
            && (is_runtime_value_subject(program, argument)
                || is_runtime_value_expression(program, argument)
                || argument.evidence_projection.is_some()
                || argument.symbol.is_valid()
                    && program
                        .data_type_parameter_by_symbol(argument.symbol)
                        .is_some_and(|parameter| {
                            matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Value { .. }
                            )
                        }))
    }

    pub(crate) fn runtime_subject_expression(
        program: &dyn ::validation::CheckingInput,
        argument: &StaticMachineArgument,
    ) -> Option<ExpressionHandle> {
        if !is_runtime_value_expression(program, argument) {
            return None;
        }
        let TypeReferenceNode::ConstExpression(expression) = program
            .type_reference_table
            .type_reference(argument.type_reference)
        else {
            return None;
        };
        Some(*expression)
    }

    pub(crate) fn resolve_runtime_subject(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        scope_limit: usize,
        argument: &StaticMachineArgument,
    ) -> Option<symbols::SymbolHandle> {
        if argument.type_reference.is_valid()
            || argument.application.is_some()
            || argument.evidence_projection.is_some()
            || argument.const_literal.is_some()
        {
            return None;
        }
        if argument.symbol.is_valid() {
            let kind = program.symbols.get(argument.symbol).kind;
            if kind == SymbolKind::Parameter {
                return Some(argument.symbol);
            }
            if kind == SymbolKind::TypeParameter
                && program
                    .data_type_parameter_by_symbol(argument.symbol)
                    .is_some_and(|parameter| {
                        matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Value { .. }
                        )
                    })
            {
                return Some(argument.symbol);
            }
            if kind != SymbolKind::Local {
                return None;
            }
        }
        validation::nearest_scoped_local(program, state, scope_limit, argument.path.as_ref())
    }

    pub(crate) fn has_pending_type_arguments(
        program: &dyn ::validation::CheckingInput,
        arguments: &[Option<TypeReferenceHandle>],
    ) -> bool {
        if program.pending_const_range_endpoints.is_empty() {
            return false;
        }
        let mut visited = Vec::new();
        let mut expressions = Vec::new();
        for argument in arguments.iter().flatten() {
            collect_type_expressions(program, *argument, &mut visited, &mut expressions);
        }
        expressions
            .iter()
            .any(|expression| program.pending_const_range_endpoints.contains(expression))
    }
}
pub(crate) mod dynamic_families {
    use super::body_cloning::clone_specialized_machine;
    use super::candidate;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::finite_family::{FamilyProbe, FamilyTuple};
    use typed_trees::machine::resolve_satisfied_declaration;
    use typed_trees::trait_definition::{AppliedTraitScope, Conformance};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::contract_identities::{
        accepted_template_commitment, normalized_machine_identity,
    };

    pub(crate) fn generate_dynamic_family_specializations(
        program: &mut TypedTrees,
    ) -> Result<usize, Vec<Diagnostic>> {
        let signatures = program
            .prepare_applied_callable_signatures()
            .map_err(|message| vec![Diagnostic::error(message)])?;
        super::finalize_callable_signature_identities(program, signatures)
            .map_err(|diagnostic| vec![diagnostic])?;
        let mut diagnostics = Vec::new();
        let mut demands: Vec<(SymbolHandle, Vec<FamilyTuple>)> = Vec::new();

        for realization in program.machines() {
            if !realization.supply_mode.is_checked_body()
                || !realization.body_is_present
                || program.machine_type_parameters(realization).is_empty()
            {
                continue;
            }
            for conformance in program.machine_trait_conformances(realization) {
                if conformance.external_binding.is_some() || conformance.via_expression.is_valid() {
                    continue;
                }
                let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                }) = resolve_satisfied_declaration(program, realization, conformance)
                else {
                    continue;
                };
                if !definition.is_boundary {
                    continue;
                }
                let FamilyProbe::Finite { arity, tuples } =
                    program.finite_signature_family(requirement)
                else {
                    continue;
                };
                let binders = program.machine_type_parameters(realization);
                let value_binders = binders
                    .iter()
                    .filter(|parameter| parameter.kind.is_const_or_value())
                    .count();
                if binders.len() != value_binders || value_binders != arity {
                    diagnostics.push(Diagnostic::error(format!(
                    "boundary provider candidate `{}` cannot cover the declared finite family of `{}`: a family provider must be generic over exactly the requirement's {arity} const/value binders",
                    realization.name, requirement.name
                )));
                    continue;
                }
                queue_family_demand(&mut demands, realization.symbol, tuples);
            }
        }

        for conformance in program.conformances() {
            if !conformance.type_parameters.is_empty()
                || !program
                    .trait_definition_by_symbol(conformance.trait_symbol)
                    .is_some_and(|definition| definition.is_boundary)
            {
                continue;
            }
            collect_conformance_demands(program, conformance, &mut demands, &mut diagnostics);
        }

        if let Ok(selections) = validation::collect_dynamic_implementation_selections(program)
            && !selections.is_empty()
        {
            collect_selection_demands(program, &selections, &mut demands, &mut diagnostics);
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        if demands.is_empty() {
            return Ok(0);
        }

        let mut generated = 0;
        for (template_symbol, tuples) in demands {
            let Some(machine_index) = program.machine_position_by_symbol(template_symbol) else {
                return Err(vec![Diagnostic::error(
                    "dynamic family realization lost its machine template",
                )]);
            };
            for tuple in &tuples {
                if tuple_specialization_exists(program, template_symbol, tuple) {
                    continue;
                }
                generate_tuple_specialization(program, machine_index, tuple)?;
                generated += 1;
            }
        }
        Ok(generated)
    }

    fn queue_family_demand(
        demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
        template: SymbolHandle,
        tuples: Vec<FamilyTuple>,
    ) {
        match demands
            .iter_mut()
            .find(|(existing, _)| *existing == template)
        {
            Some((_, existing)) => {
                for tuple in tuples {
                    if !existing
                        .iter()
                        .any(|candidate| candidate.identities.as_ref() == tuple.identities.as_ref())
                    {
                        existing.push(tuple);
                    }
                }
            }
            None => demands.push((template, tuples)),
        }
    }

    fn collect_selection_demands(
        program: &dyn ::validation::CheckingInput,
        selections: &[validation::DynamicImplementationSelection],
        demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut seen_conformances: Vec<SymbolHandle> = Vec::new();
        for selection in selections {
            let Some(conformance) = program.conformance_by_symbol(selection.implementation) else {
                continue;
            };
            if seen_conformances.contains(&conformance.symbol) {
                continue;
            }
            seen_conformances.push(conformance.symbol);
            collect_conformance_demands(program, conformance, demands, diagnostics);
        }
    }

    fn collect_conformance_demands(
        program: &dyn ::validation::CheckingInput,
        conformance: &Conformance,
        demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(application) =
            AppliedTraitScope::from_conformance(program.typed_program(), conformance)
        else {
            return;
        };
        for row in &conformance.rows {
            let Some(scope) =
                application.declaring_scope(program.typed_program(), row.declaring_trait)
            else {
                continue;
            };
            let Some(requirement) = program.applied_callable_signature(&scope, row.requirement)
            else {
                continue;
            };
            let FamilyProbe::Finite { arity, tuples } =
                program.finite_signature_family(requirement)
            else {
                continue;
            };
            let Some(realization) = program.machine_by_symbol(row.realization_machine) else {
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic conformance row `{}::{}` has no exact realization machine",
                    row.declaring_trait_name, row.requirement_name
                )));
                continue;
            };
            let realization_binders = program.machine_type_parameters(realization);
            let value_binders = realization_binders
                .iter()
                .filter(|parameter| parameter.kind.is_const_or_value())
                .count();
            if realization_binders.len() != value_binders || value_binders != arity {
                diagnostics.push(Diagnostic::error(format!(
                "dynamic conformance row `{}::{}` realization `{}` cannot cover the declared finite family: a family provider must be generic over exactly the requirement's {arity} const/value binders",
                row.declaring_trait_name,
                row.requirement_name,
                realization.name
            )));
                continue;
            }
            queue_family_demand(demands, realization.symbol, tuples);
        }
    }

    fn tuple_specialization_exists(
        program: &dyn ::validation::CheckingInput,
        template: SymbolHandle,
        tuple: &FamilyTuple,
    ) -> bool {
        program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.template == template
                    && specialization.const_argument_identities.as_slice()
                        == tuple.identities.as_ref()
                    && specialization.is_const_only()
            })
    }

    fn generate_tuple_specialization(
        program: &mut TypedTrees,
        machine_index: usize,
        tuple: &FamilyTuple,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut candidate = candidate::from_machine(program, machine_index);
        if !candidate.template.machine_parameters.is_empty()
            || !candidate.template.evidence_parameters.is_empty()
        {
            return Err(vec![Diagnostic::error(format!(
                "dynamic family provider `{}` carries machine or conformance binders, which a bare value tuple cannot close",
                candidate.template.template_name
            ))]);
        }
        for (binding, identity) in candidate.const_bindings.iter_mut().zip(&tuple.identities) {
            let Some(type_reference) = const_identity_type_reference(program, identity) else {
                return Err(vec![Diagnostic::error(format!(
                    "dynamic family tuple `({})` has a canonical value with no closed const carrier",
                    tuple.display.join(", ")
                ))]);
            };
            *binding = Some(type_reference);
        }

        let normalized_template_identity = normalized_machine_identity(
            program,
            &program.machines()[candidate.template.machine_index],
        )
        .expect("generic template must retain a normalized callable identity");
        let accepted_template_commitment =
            accepted_template_commitment(program, candidate.template.machine_index);
        let ordinal = program
            .machine_specializations
            .iter()
            .filter(|instance| instance.template == candidate.template.template_symbol)
            .count();
        clone_specialized_machine(
            None,
            program,
            &candidate,
            ordinal,
            normalized_template_identity,
            accepted_template_commitment,
        )?;
        Ok(())
    }

    fn const_identity_type_reference(
        program: &mut TypedTrees,
        identity: &str,
    ) -> Option<TypeReferenceHandle> {
        let spelling = const_identity_spelling(identity)?;
        let existing = program
            .type_reference_table
            .named_references()
            .find(|(_, symbol, name)| !symbol.is_valid() && *name == spelling)
            .map(|(handle, _, _)| handle);
        if let Some(handle) = existing {
            return Some(handle);
        }
        Some(
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::generated(spelling),
                }),
        )
    }

    pub(crate) fn const_identity_spelling(identity: &str) -> Option<String> {
        if let Some(value) = identity
            .strip_prefix("named(integer-const(")
            .and_then(|inner| inner.strip_suffix("))"))
        {
            return value.parse::<i128>().ok().map(|value| value.to_string());
        }
        let encoded = identity
            .strip_prefix("named(canonical-const(type(")
            .and_then(|inner| inner.strip_suffix(")))"))?;
        let (type_name, encoding) = encoded.split_once("),encoding(")?;
        let value = language_semantics::const_value::CanonicalConstValue::new(
            unescape_identity_component(type_name),
            unescape_identity_component(encoding),
            "",
        );
        match value.decode_encoding()? {
            DecodedCanonicalConstValue::Integer { value, .. } => Some(value.to_string()),
            DecodedCanonicalConstValue::Boolean(value) => {
                Some(language_semantics::const_value::CanonicalConstValue::boolean(value).atom())
            }
            _ => None,
        }
    }

    fn unescape_identity_component(component: &str) -> String {
        let mut out = String::with_capacity(component.len());
        let mut escaped = false;
        for character in component.chars() {
            if escaped {
                out.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else {
                out.push(character);
            }
        }
        out
    }
}
pub(crate) use dynamic_families::generate_dynamic_family_specializations;
pub(crate) mod dynamic_slots {
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::dynamic_traits::DynamicSlotSignature;
    use typed_trees::expression::ExpressionNode;
    use typed_trees::name::Identifier;
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::StatementNode;
    use validation::dynamic_expression_call_demand;
    use validation::expression_traversal::collect_expression_tree;

    pub(crate) fn instantiate_dynamic_slots(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<Diagnostic>> {
        let Ok(demands) = validation::collect_dynamic_slot_demands(program) else {
            return Ok(());
        };
        for demand in demands {
            let interface_identity = program.normalized_type_identity(demand.selector_type);
            if program
                .dynamic_slot(interface_identity, demand.call_requirement)
                .is_none()
            {
                let slot = instantiate_slot(program, &demand, interface_identity)?;
                program.dynamic_slots.push(slot);
            }
        }
        retarget_selector_calls(program);
        let mut diagnostics = Vec::new();
        validation::validate_prepared_inline_storage(program, &mut diagnostics);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(())
    }

    fn instantiate_slot(
        program: &mut TypedTrees,
        demand: &validation::DynamicSlotDemand,
        interface_identity: typed_trees::type_identity::TypeIdentityId,
    ) -> Result<DynamicSlotSignature, Vec<Diagnostic>> {
        let Some(definition) = program.trait_definition_by_symbol(demand.declaring_trait) else {
            return Err(vec![Diagnostic::error(
                "dynamic slot names no declaring trait",
            )]);
        };
        let Some(signature) = program
            .closed_requirement_application(demand.call_requirement)
            .map(|application| application.signature)
            .or_else(|| program.trait_machine_signature_by_symbol(definition, demand.requirement))
            .cloned()
        else {
            return Err(vec![Diagnostic::error(
                "dynamic slot names no requirement of its declaring trait",
            )]);
        };
        let binders = program
            .trait_type_parameters(definition)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let declaring_arguments = demand
            .substitution_scope_starts
            .last()
            .map_or(&[][..], |start| &demand.type_substitutions[*start..]);
        if binders.len() != declaring_arguments.len() {
            return Err(vec![Diagnostic::error(format!(
                "`dyn {}` supplies {} trait argument(s) for {} parameter(s)",
                definition.name,
                declaring_arguments.len(),
                binders.len()
            ))]);
        }
        let selector_display = program.display_type_reference(demand.selector_type);
        let instantiation_error = |reason: &'static str| {
            vec![Diagnostic::error(format!(
                "requirement `{}` cannot be instantiated for `{selector_display}`: {reason}",
                signature.name
            ))]
        };
        let formals = program.state_signature_parameters(&signature).to_vec();
        let mut parameters = Vec::with_capacity(formals.len() + 1);
        parameters.push(StateParameter {
            name: Identifier::generated_static("selector"),
            type_reference: demand.selector_type,
            ..StateParameter::default()
        });
        for formal in &formals {
            let mut parameter = formal.clone();
            parameter.type_reference =
                instantiate_requirement_type(program, formal.type_reference, demand)
                    .map_err(instantiation_error)?;
            parameters.push(parameter);
        }
        let return_type = instantiate_requirement_type(program, signature.return_type, demand)
            .map_err(instantiation_error)?;

        let symbol = program.symbols.insert_generated_root_from(
            demand.call_requirement,
            program.symbols.get(demand.call_requirement).kind,
            &format!("{selector_display}::{}", signature.name),
        );
        let names = parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let origins = std::iter::once(demand.requirement)
            .chain(formals.iter().map(|formal| formal.symbol))
            .collect::<Vec<_>>();
        let children = program.symbols.insert_generated_children_from(
            symbol,
            origins
                .iter()
                .zip(&names)
                .map(|(origin, name)| (*origin, symbols::SymbolKind::Parameter, name.as_str())),
        );
        for (offset, parameter) in (0u32..).zip(parameters.iter_mut()) {
            parameter.symbol = children.handle_at(offset);
        }
        let parameter_map = formals
            .iter()
            .zip(parameters.iter().skip(1))
            .map(|(formal, parameter)| (formal.symbol, parameter.symbol))
            .collect::<Vec<_>>();
        for parameter in &mut parameters {
            parameter.type_reference = super::body_cloning::copy_type_reference(
                None,
                program,
                parameter.type_reference,
                &parameter_map,
            )?;
        }
        let return_type =
            super::body_cloning::copy_type_reference(None, program, return_type, &parameter_map)?;
        Ok(DynamicSlotSignature {
            symbol,
            interface_identity,
            declaring_trait: demand.declaring_trait,
            requirement: demand.requirement,
            call_requirement: demand.call_requirement,
            parameters,
            return_type,
        })
    }

    fn instantiate_requirement_type(
        program: &mut TypedTrees,
        mut reference: typed_trees::types::TypeReferenceHandle,
        demand: &validation::DynamicSlotDemand,
    ) -> Result<typed_trees::types::TypeReferenceHandle, &'static str> {
        if demand.call_requirement != demand.requirement {
            return Ok(reference);
        }
        let mut end = demand.type_substitutions.len();
        for start in demand.substitution_scope_starts.iter().rev() {
            reference = program
                .type_reference_table
                .instantiate(reference, &demand.type_substitutions[*start..end])?;
            end = *start;
        }
        Ok(reference)
    }

    fn retarget_selector_calls(program: &mut TypedTrees) {
        let mut statement_targets = Vec::new();
        let mut expression_targets = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                let statements = state.statement_nodes;
                for (offset, statement) in program
                    .statement_table
                    .statements(statements)
                    .iter()
                    .enumerate()
                {
                    if let Some(call) = statement.as_call()
                        && let Some(slot) = slot_for(
                            program,
                            validation::dynamic_call_demand(
                                program,
                                machine,
                                state,
                                call.receiver_symbol,
                                call.target_symbol,
                                program.statement_table.expression_handles(call.arguments),
                            ),
                        )
                    {
                        statement_targets.push((statements.handle_at(offset as u32), slot));
                    }
                    for (_, root) in program.tables.expression_roots(statement) {
                        if !root.is_valid() {
                            continue;
                        }
                        let mut tree = Vec::new();
                        collect_expression_tree(program, root, &mut tree);
                        for expression in tree {
                            let Some(call) = expressions_of(program).call_node(expression) else {
                                continue;
                            };
                            if let Some(slot) = slot_for(
                                program,
                                dynamic_expression_call_demand(program, machine, state, call),
                            ) && !expression_targets
                                .iter()
                                .any(|(known, _)| *known == expression)
                            {
                                expression_targets.push((expression, slot));
                            }
                        }
                    }
                }
            }
        }
        for (statement, (slot, name)) in statement_targets {
            if let StatementNode::Call(call) = program.statement_table.statement_mut(statement) {
                call.target_symbol = slot;
                call.target = name;
            }
        }
        for (expression, (slot, name)) in expression_targets {
            if let ExpressionNode::Call(call) =
                expressions_of_mut(program).expression_mut(expression)
            {
                call.target_symbol = slot;
                call.target = name;
            }
        }
    }

    fn slot_for(
        program: &dyn ::validation::CheckingInput,
        demand: Option<validation::DynamicSlotDemand>,
    ) -> Option<(SymbolHandle, Identifier)> {
        let demand = demand?;
        let identity = program.normalized_type_identity(demand.selector_type);
        let slot = program.dynamic_slot(identity, demand.call_requirement)?;
        Some((
            slot.symbol,
            Identifier::generated(program.symbols.name(slot.symbol).to_owned()),
        ))
    }
}
pub(crate) use crate::contextual_and_requirement_calls::requirement_applications;
pub(crate) use dynamic_slots::instantiate_dynamic_slots;
pub(crate) use requirement_applications::instantiate_requirement_applications;
pub use requirement_applications::prepare_requirement_expression_views;
pub(crate) use requirement_providers::generate_requirement_provider_specializations;
pub(crate) use type_reference_walk::for_each_type_reference;
pub(crate) mod const_values {
    use super::Candidate;
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use language_semantics::const_value::{CanonicalConstValue, boolean_literal_spelling};
    use numerics::literals::{IntegerLanding, IntegerLiteral};
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
    use validation::expression_traversal::collect_expression_tree;

    pub(crate) fn substitute(
        program: &mut TypedTrees,
        candidate: &Candidate,
        expression_start: Option<usize>,
        runtime_expression_roots: &[ExpressionHandle],
    ) -> Result<(), Diagnostic> {
        for (index, ((binder, name, declared_type), binding)) in candidate
            .template
            .const_parameters
            .iter()
            .zip(&candidate.const_bindings)
            .enumerate()
        {
            if candidate.runtime_value_bindings[index].is_some() {
                let stray = runtime_expression_roots.iter().any(|root| {
                    let mut tree = Vec::new();
                    collect_expression_tree(program, *root, &mut tree);
                    tree.into_iter().any(|handle| {
                        matches!(
                            expressions_of(program).expression(handle),
                            ExpressionNode::Name(path)
                                if binder.is_valid()
                                    && path.symbol == *binder
                                    && path.head_symbol == *binder
                                    && expressions_of(program).name_path_members(path.members)
                                        .len()
                                        == 1
                        )
                    })
                });
                if stray {
                    return Err(Diagnostic::error(format!(
                        "value parameter `{name}` of machine `{}` is bound to a runtime argument and \
                     cannot appear in a static type, contract, or const context",
                        candidate.template.template_name,
                    )));
                }
                continue;
            }
            let occurrences = match expression_start {
                Some(start) => expressions_of(program)
                    .iter_expressions_from(start)
                    .filter_map(|(handle, _)| {
                        exact_binder_name(program, handle, *binder).then_some(handle)
                    })
                    .collect::<Vec<_>>(),
                None => expressions_of(program)
                    .iter_expressions()
                    .filter_map(|(handle, _)| {
                        exact_binder_name(program, handle, *binder).then_some(handle)
                    })
                    .collect::<Vec<_>>(),
            };
            let projections = const_projections(program, *binder, expression_start);
            if occurrences.is_empty() && projections.is_empty() {
                continue;
            }
            let replacement = binding
            .and_then(|binding| value(program, binding, *declared_type))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "machine `{}` cannot materialize const parameter `{name}` as its declared value type `{}`",
                    candidate.template.template_name,
                    program.display_type_reference(*declared_type),
                ))
            })?;
            let mut projected_receivers = Vec::new();
            for projection in &projections {
                let Some(projected) = project_const_value(program, &replacement, &projection.steps)
                else {
                    continue;
                };
                *expressions_of_mut(program).expression_mut(projection.outer) = projected;
                projected_receivers.push(projection.root);
            }
            for occurrence in occurrences {
                if projected_receivers.contains(&occurrence) {
                    continue;
                }
                *expressions_of_mut(program).expression_mut(occurrence) = replacement.clone();
            }
        }
        Ok(())
    }

    fn exact_binder_name(
        program: &TypedTrees,
        expression: ExpressionHandle,
        binder: symbols::SymbolHandle,
    ) -> bool {
        let ExpressionNode::Name(path) = expressions_of(program).expression(expression) else {
            return false;
        };
        binder.is_valid()
            && path.symbol == binder
            && path.head_symbol == binder
            && expressions_of(program)
                .name_path_members(path.members)
                .len()
                == 1
    }

    enum ProjectionStep {
        Field {
            symbol: symbols::SymbolHandle,
            name: typed_trees::name::Identifier,
            case_variant: Option<typed_trees::name::Identifier>,
        },
        Element(usize),
    }

    struct ConstProjection {
        outer: ExpressionHandle,
        root: ExpressionHandle,
        steps: Vec<ProjectionStep>,
    }

    fn projection_step(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<(ExpressionHandle, ProjectionStep)> {
        match expressions_of(program).expression(expression) {
            ExpressionNode::Member(member) => Some((
                member.receiver,
                ProjectionStep::Field {
                    symbol: member.member_symbol,
                    name: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                },
            )),
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    expressions_of(program).expression(indexed.index)
                else {
                    return None;
                };
                let index = usize::try_from(index.value_i64()?).ok()?;
                Some((indexed.collection, ProjectionStep::Element(index)))
            }
            _ => None,
        }
    }

    fn const_projections(
        program: &TypedTrees,
        binder: symbols::SymbolHandle,
        expression_start: Option<usize>,
    ) -> Vec<ConstProjection> {
        let candidates = match expression_start {
            Some(start) => expressions_of(program)
                .iter_expressions_from(start)
                .map(|(handle, _)| handle)
                .filter(|handle| projection_step(program, *handle).is_some())
                .collect::<Vec<_>>(),
            None => expressions_of(program)
                .iter_expressions()
                .map(|(handle, _)| handle)
                .filter(|handle| projection_step(program, *handle).is_some())
                .collect::<Vec<_>>(),
        };
        let receivers = candidates
            .iter()
            .filter_map(|handle| projection_step(program, *handle).map(|(receiver, _)| receiver))
            .collect::<Vec<_>>();
        let mut projections = Vec::new();
        for outer in candidates {
            if receivers.contains(&outer) {
                continue;
            }
            let mut steps = Vec::new();
            let mut current = outer;
            let root = loop {
                let Some((receiver, step)) = projection_step(program, current) else {
                    break current;
                };
                steps.push(step);
                current = receiver;
            };
            if !exact_binder_name(program, root, binder) {
                continue;
            }
            steps.reverse();
            projections.push(ConstProjection { outer, root, steps });
        }
        projections
    }

    fn project_const_value(
        program: &TypedTrees,
        value: &ExpressionNode,
        steps: &[ProjectionStep],
    ) -> Option<ExpressionNode> {
        let mut projected = value.clone();
        for step in steps {
            projected = match step {
                ProjectionStep::Field {
                    symbol,
                    name,
                    case_variant,
                } => project_nominal_field(
                    program,
                    &projected,
                    *symbol,
                    name,
                    case_variant.as_ref(),
                )?,
                ProjectionStep::Element(index) => {
                    let ExpressionNode::ArrayLiteral(elements) = &projected else {
                        return None;
                    };
                    let element = *expressions_of(program)
                        .expression_handles(*elements)
                        .get(*index)?;
                    expressions_of(program).expression(element).clone()
                }
            };
        }
        Some(projected)
    }

    fn project_nominal_field(
        program: &TypedTrees,
        value: &ExpressionNode,
        symbol: symbols::SymbolHandle,
        name: &typed_trees::name::Identifier,
        case_variant: Option<&typed_trees::name::Identifier>,
    ) -> Option<ExpressionNode> {
        let ExpressionNode::StructLiteral(literal) = value else {
            return None;
        };
        if let Some(case_variant) = case_variant {
            if literal.case_name.as_ref() != Some(case_variant) {
                return None;
            }
        } else if literal.case_name.is_some() || literal.case_symbol.is_some() {
            return None;
        }
        let selected = expressions_of(program)
            .struct_fields(literal.fields)
            .iter()
            .find(|declared| {
                if symbol.is_valid() {
                    declared.field_symbol == symbol
                } else {
                    declared.name.as_str() == name.as_str()
                }
            })?;
        Some(expressions_of(program).expression(selected.value).clone())
    }

    pub(crate) fn value(
        program: &mut TypedTrees,
        binding: TypeReferenceHandle,
        declared_type: TypeReferenceHandle,
    ) -> Option<ExpressionNode> {
        if let TypeReferenceNode::ConstExpression(expression) =
            program.type_reference_table.type_reference(binding)
        {
            let expression = *expression;
            if !expressions_of(program).expression_is_valid(expression) {
                return None;
            }
            return Some(expressions_of(program).expression(expression).clone());
        }
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(binding)
        else {
            return None;
        };
        if symbol.is_valid() {
            let declaration = program.const_declaration_by_symbol(*symbol)?;
            let encoding = declaration.evaluated_value.as_ref()?.encoding()?.to_owned();
            let value = CanonicalConstValue::new("", encoding, "").decode_encoding()?;
            return program.materialize_canonical_const_value(declared_type, &value);
        }
        if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
            return program
                .materialize_canonical_const_value(declared_type, &value.decode_encoding()?);
        }
        let primitive = program.type_reference_table.primitive_type(declared_type)?;
        if primitive == PrimitiveType::Bool {
            return boolean_literal_spelling(name.as_str()).map(ExpressionNode::Boolean);
        }
        let literal = program.static_const_literal_from_type_reference(binding)?;
        integer_value(program, declared_type, literal)
    }

    fn integer_value(
        program: &dyn ::validation::CheckingInput,
        declared_type: TypeReferenceHandle,
        literal: IntegerLiteral,
    ) -> Option<ExpressionNode> {
        let primitive = program.type_reference_table.primitive_type(declared_type)?;
        let landed_type = primitive.landed_landing()?;
        Some(ExpressionNode::Integer(
            literal.with_landing(IntegerLanding {
                landed_type,
                domain: program
                    .type_reference_table
                    .arithmetic_domain(declared_type),
            }),
        ))
    }
}
pub(crate) use crate::selection_and_reflection::result_locals;
pub(crate) use const_values::value as materialize_static_value;
pub(crate) use result_locals::refresh as refresh_inferred_result_locals;
pub(crate) mod saved_calls {
    use super::{CallSelection, CallSite, Candidate};
    use typed_trees::TypedTrees;
    use typed_trees::machine::Machine;
    use typed_trees::typed_trees::MachineSpecialization;
    use validation::conformance_applications::close_conformance_application;
    use validation::expression_traversal::collect_expression_tree;

    pub(crate) fn replay(
        source: &mut TypedTrees,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
    ) {
        let candidates = super::candidate::collect(source);
        let callees = super::candidate::callees(source, &candidates);
        let contracts = crate::monomorphization::selection::contract_expression_handles(source);
        let selections = crate::monomorphization::selection::collect_call_selections(
            source,
            &candidates,
            &callees,
            &contracts,
        );
        let mut sites = Vec::new();
        let mut expressions = Vec::new();
        for state in source.machine_states(machine) {
            for handle in crate::monomorphization::body_rewriting::statement_span_handles(
                state.statement_nodes,
            ) {
                sites.push(CallSite::Statement(handle));
                for root in super::operator_candidates::executable_statement_expression_roots(
                    source,
                    source.statement_table.statement(handle),
                ) {
                    collect_expression_tree(source, root, &mut expressions);
                }
            }
        }
        sites.extend(expressions.into_iter().map(CallSite::Expression));

        for selection in selections {
            let candidate = &candidates[selection.candidate_index];
            if !sites.contains(&selection.site)
                || !selection.is_complete()
                || candidate.template.template_symbol == machine.symbol
            {
                continue;
            }
            let concrete = super::candidate_for_selection(source, candidate, &selection);
            super::applications::retain_call(source, &concrete, &selection);
            let Some(instance) = selected_instance(source, program, candidate, &selection) else {
                continue;
            };
            let Some(state_offset) = candidate
                .template
                .state_symbols
                .iter()
                .position(|symbol| *symbol == selection.callee_symbol)
            else {
                continue;
            };
            let Some(instance_machine) = program.machine_by_symbol(instance.instance) else {
                continue;
            };
            let Some(state) = program.machine_states(instance_machine).get(state_offset) else {
                continue;
            };
            let arguments = super::runtime_value_subjects(&selection.runtime_value_bindings);
            let subjects = crate::monomorphization::body_rewriting::runtime_call_operands(
                source,
                selection.site,
                selection.lexical_state,
                &arguments,
            );
            crate::monomorphization::body_rewriting::rewrite_selected_call_with_name(
                source,
                selection.site,
                state.symbol,
                state.name.clone(),
                &subjects,
            );
        }
    }

    pub(crate) fn selected_instance<'program>(
        source: &dyn ::validation::CheckingInput,
        program: &'program dyn ::validation::CheckingInput,
        candidate: &Candidate,
        selection: &CallSelection,
    ) -> Option<&'program MachineSpecialization> {
        let concrete = super::candidate_for_selection(source, candidate, selection);
        candidate_instance(source, program, &concrete)
    }

    pub(crate) fn candidate_instance<'program>(
        source: &dyn ::validation::CheckingInput,
        program: &'program dyn ::validation::CheckingInput,
        candidate: &Candidate,
    ) -> Option<&'program MachineSpecialization> {
        let type_arguments = candidate
            .type_bindings
            .iter()
            .map(|binding| binding.map(|binding| source.normalized_type_identity_text(binding)))
            .collect::<Option<Vec<_>>>()?;
        let const_arguments = candidate
            .const_bindings
            .iter()
            .map(|binding| binding.map(|binding| source.normalized_type_identity_text(binding)))
            .collect::<Option<Vec<_>>>()?;
        let machine_arguments = candidate
            .machine_bindings
            .iter()
            .map(|binding| binding.as_ref().map(|binding| binding.symbol))
            .collect::<Option<Vec<_>>>()?;
        let evidence_arguments = candidate
            .evidence_bindings
            .iter()
            .map(|binding| binding.as_ref())
            .collect::<Option<Vec<_>>>()?;
        let applications = evidence_arguments
            .iter()
            .map(|binding| close_conformance_application(source, binding))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        program.machine_specializations.iter().find(|instance| {
            instance.template == candidate.template.template_symbol
                && instance.type_argument_identities == type_arguments
                && instance.const_argument_identities == const_arguments
                && instance.machine_arguments == machine_arguments
                && instance
                    .conformance_arguments
                    .iter()
                    .copied()
                    .eq(evidence_arguments.iter().map(|binding| binding.symbol))
                && instance.conformance_applications == applications
        })
    }
}
pub(crate) mod target_families {
    use super::{Candidate, Diagnostic, SymbolHandle, TypedTrees};
    use typed_trees::machine::TargetMachineFamilyMember;
    use validation::CheckingInput;
    use validation::contract_identities::{
        accepted_template_commitment, normalized_machine_identity,
    };

    pub(crate) fn clone_specialization(
        source: Option<&dyn CheckingInput>,
        program: &mut TypedTrees,
        candidate: &Candidate,
        ordinal: usize,
        template_identity: String,
        template_commitment: Option<String>,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Vec<Diagnostic>> {
        let input = source.unwrap_or(program);
        let template = candidate.template.template_symbol;
        let mut families = input.target_machine_families.iter().filter(|family| {
            family.canonical == template
                || family
                    .members
                    .iter()
                    .any(|member| member.machine == template)
        });
        let family = families.next().cloned();
        if families.next().is_some() {
            return Err(failure(
                "generic machine belongs to multiple target families",
            ));
        }
        let Some(mut family) = family else {
            if input.machines()[candidate.template.machine_index].target_family_interface {
                return Err(failure(
                    "generic target family interface has no retained family roster",
                ));
            }
            return super::body_cloning::clone_single_specialized_machine(
                source,
                program,
                candidate,
                ordinal,
                template_identity,
                template_commitment,
                None,
            );
        };
        if family.members.is_empty() {
            return Err(failure("generic target family has no retained members"));
        }

        let mut templates = Vec::with_capacity(family.members.len() + 1);
        templates.push(family.canonical);
        templates.extend(family.members.iter().map(|member| member.machine));
        let mut plans = Vec::with_capacity(templates.len());
        for symbol in templates {
            if plans.iter().any(|(retained, _, _, _)| *retained == symbol) {
                return Err(failure("generic target family repeats a declaration"));
            }
            let Some(machine_index) = input
                .machines()
                .iter()
                .position(|machine| machine.symbol == symbol)
            else {
                return Err(failure("generic target family declaration is absent"));
            };
            let mut concrete = super::candidate::from_machine(input, machine_index);
            if concrete.type_bindings.len() != candidate.type_bindings.len()
                || concrete.const_bindings.len() != candidate.const_bindings.len()
                || concrete.runtime_value_bindings.len() != candidate.runtime_value_bindings.len()
                || concrete.machine_bindings.len() != candidate.machine_bindings.len()
                || concrete.evidence_bindings.len() != candidate.evidence_bindings.len()
                || concrete.template.value_const_parameters
                    != candidate.template.value_const_parameters
            {
                return Err(failure(
                    "generic target family application has incompatible binder positions",
                ));
            }
            concrete.binding_application = candidate.binding_application;
            concrete.type_bindings.clone_from(&candidate.type_bindings);
            concrete
                .const_bindings
                .clone_from(&candidate.const_bindings);
            concrete
                .runtime_value_bindings
                .clone_from(&candidate.runtime_value_bindings);
            concrete
                .machine_bindings
                .clone_from(&candidate.machine_bindings);
            concrete
                .evidence_bindings
                .clone_from(&candidate.evidence_bindings);
            let machine = &input.machines()[machine_index];
            if symbol == family.canonical && !machine.target_family_interface {
                return Err(failure(
                    "generic target family has no independent interface declaration",
                ));
            }
            let identity = normalized_machine_identity(input, machine).ok_or_else(|| {
                failure("generic target family declaration has no normalized identity")
            })?;
            let commitment = accepted_template_commitment(input, machine_index);
            plans.push((symbol, concrete, identity, commitment));
        }

        let mut instances = Vec::with_capacity(plans.len());
        let mut requested_states = None;
        for (symbol, concrete, identity, commitment) in plans {
            let existing = super::saved_calls::candidate_instance(
                source.unwrap_or(program),
                program,
                &concrete,
            )
            .map(|instance| instance.instance);
            let states = if let Some(instance) = existing {
                let machine = program
                    .machine_by_symbol(instance)
                    .ok_or_else(|| failure("retained specialization has no machine declaration"))?;
                let states = program.machine_states(machine);
                if states.len() != concrete.template.state_symbols.len() {
                    return Err(failure("retained specialization changes its state roster"));
                }
                concrete
                    .template
                    .state_symbols
                    .iter()
                    .copied()
                    .zip(states.iter().map(|state| state.symbol))
                    .collect()
            } else {
                let instance_ordinal = if symbol == template {
                    ordinal
                } else {
                    program
                        .machine_specializations
                        .iter()
                        .filter(|instance| instance.template == symbol)
                        .count()
                };
                super::body_cloning::clone_single_specialized_machine(
                    source,
                    program,
                    &concrete,
                    instance_ordinal,
                    identity,
                    commitment,
                    None,
                )?
            };
            let instance = super::saved_calls::candidate_instance(
                source.unwrap_or(program),
                program,
                &concrete,
            )
            .ok_or_else(|| {
                failure("closed target family application has no retained specialization")
            })?
            .instance;
            instances.push((symbol, instance));
            if symbol == template {
                requested_states = Some(states);
            }
        }
        let closed_symbol = |symbol| {
            instances
                .iter()
                .find(|(original, _)| *original == symbol)
                .map(|(_, instance)| *instance)
                .ok_or_else(|| failure("closed target family is missing a declaration"))
        };
        family.canonical = closed_symbol(family.canonical)?;
        family.members = family
            .members
            .into_iter()
            .map(|member| {
                Ok(TargetMachineFamilyMember {
                    target: member.target,
                    machine: closed_symbol(member.machine)?,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        if let Some(existing) = program
            .target_machine_families
            .iter()
            .find(|existing| existing.canonical == family.canonical)
        {
            if existing.members != family.members {
                return Err(failure(
                    "closed target family application disagrees with its retained roster",
                ));
            }
        } else {
            program.target_machine_families.push(family);
        }
        requested_states
            .ok_or_else(|| failure("closed target family omitted the requested declaration"))
    }

    fn failure(message: &str) -> Vec<Diagnostic> {
        vec![Diagnostic::error(message)]
    }
}
pub(crate) mod type_reference_walk {
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn for_each_type_reference(
        program: &dyn ::validation::CheckingInput,
        roots: &[TypeReferenceHandle],
        mut on_node: impl FnMut(&dyn ::validation::CheckingInput, &TypeReferenceNode) -> bool,
    ) -> bool {
        let mut pending = roots.to_vec();
        let mut visited = std::collections::HashSet::new();
        while let Some(handle) = pending.pop() {
            if !handle.is_valid() || !visited.insert(handle) {
                continue;
            }
            let node = program.type_reference_table.type_reference(handle);
            if on_node(program, node) {
                return true;
            }
            match node {
                TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    pending.push(*base_type);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        match constraint {
                            TypeConstraintNode::Domain(domain) => {
                                pending.extend(domain.arguments.iter().copied());
                            }

                            TypeConstraintNode::Named(_)
                            | TypeConstraintNode::ArithmeticDomain(_) => {}
                        }
                    }
                }
                TypeReferenceNode::FixedArray { element_type, .. } => pending.push(*element_type),
                TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
                TypeReferenceNode::Generic { arguments, .. } => pending.extend(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                ),
                _ => {}
            }
        }
        false
    }
}

#[derive(Clone)]
pub(crate) struct CandidateTemplate {
    pub(crate) machine_index: usize,
    pub(crate) template_symbol: SymbolHandle,
    pub(crate) template_name: String,
    pub(crate) state_symbols: Vec<SymbolHandle>,
    pub(crate) type_parameters: Vec<(SymbolHandle, String)>,
    pub(crate) const_parameters: Vec<(SymbolHandle, String, TypeReferenceHandle)>,
    pub(crate) value_const_parameters: Vec<usize>,
    pub(crate) machine_parameters: Vec<(SymbolHandle, String, StateSignature)>,
    pub(crate) evidence_parameters: Vec<typed_trees::machine::GenericConformanceBound>,
}

#[derive(Clone)]
pub(crate) struct Candidate<'template> {
    pub(crate) template: std::borrow::Cow<'template, CandidateTemplate>,
    pub(crate) binding_application: StaticMachineApplicationHandle,
    pub(crate) type_bindings: Vec<Option<TypeReferenceHandle>>,
    pub(crate) const_bindings: Vec<Option<TypeReferenceHandle>>,
    pub(crate) runtime_value_bindings: Vec<Option<RuntimeValueBinding>>,
    pub(crate) machine_bindings: Vec<Option<StaticMachineArgument>>,
    pub(crate) evidence_bindings: Vec<Option<StaticMachineArgument>>,
    pub(crate) conflicted: bool,
}

impl Candidate<'_> {
    fn borrowed(&self) -> Candidate<'_> {
        Candidate {
            template: std::borrow::Cow::Borrowed(&self.template),
            binding_application: self.binding_application,
            type_bindings: self.type_bindings.clone(),
            const_bindings: self.const_bindings.clone(),
            runtime_value_bindings: self.runtime_value_bindings.clone(),
            machine_bindings: self.machine_bindings.clone(),
            evidence_bindings: self.evidence_bindings.clone(),
            conflicted: self.conflicted,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum RuntimeValueBinding {
    Carrier,
    Authored(StaticMachineArgument),
    CapturedArgument {
        site: CallSite,
        position: u32,
        source: ExpressionHandle,
        descriptor: SymbolHandle,
    },
}

pub(crate) struct CalleeState {
    pub(crate) symbol: SymbolHandle,
    pub(crate) candidate_index: usize,
    pub(crate) return_type: TypeReferenceHandle,
    pub(crate) parameter_types: Vec<TypeReferenceHandle>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CallSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

impl CallSite {
    pub(crate) fn authored_machine_application(
        self,
        program: &TypedTrees,
    ) -> Option<StaticMachineApplicationHandle> {
        Some(match self {
            CallSite::Expression(handle) => {
                expressions_of(program)
                    .call_node(handle)?
                    .authored_machine_application
            }
            CallSite::Statement(handle) => {
                program
                    .statement_table
                    .statement(handle)
                    .as_call()?
                    .authored_machine_application
            }
        })
    }
}

#[derive(Clone)]
pub(crate) struct CallSelection {
    pub(crate) site: CallSite,
    pub(crate) lexical_owner: SymbolHandle,
    pub(crate) lexical_state: SymbolHandle,
    pub(crate) callee_symbol: SymbolHandle,
    pub(crate) candidate_index: usize,
    pub(crate) caller_is_generic: bool,
    pub(crate) unresolved_type_parameters: bool,
    pub(crate) unresolved_machine_parameters: bool,
    pub(crate) unresolved_evidence_parameters: bool,
    pub(crate) unresolved_const_parameters: bool,
    pub(crate) type_bindings: Vec<Option<TypeReferenceHandle>>,
    pub(crate) const_bindings: Vec<Option<TypeReferenceHandle>>,
    pub(crate) runtime_value_bindings: Vec<Option<RuntimeValueBinding>>,
    pub(crate) machine_bindings: Vec<Option<StaticMachineArgument>>,
    pub(crate) evidence_bindings: Vec<Option<StaticMachineArgument>>,
    pub(crate) conflicted: bool,
    pub(crate) conflict_reason: Option<String>,
    pub(crate) explicit_argument_overflow: bool,
}

impl CallSelection {
    pub(crate) fn is_complete(&self) -> bool {
        self.has_bound_arguments()
            && !self.unresolved_type_parameters
            && !self.unresolved_machine_parameters
            && !self.unresolved_evidence_parameters
            && !self.unresolved_const_parameters
    }

    pub(crate) fn has_bound_arguments(&self) -> bool {
        !self.conflicted
            && !self.explicit_argument_overflow
            && self.type_bindings.iter().all(Option::is_some)
            && self.const_bindings.iter().all(Option::is_some)
            && self.machine_bindings.iter().all(Option::is_some)
            && self.evidence_bindings.iter().all(Option::is_some)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SpecializationKey {
    pub(crate) type_arguments: Vec<String>,
    pub(crate) const_arguments: Vec<String>,
    pub(crate) machine_arguments: Vec<SymbolHandle>,
    pub(crate) evidence_arguments: Vec<u64>,
}

pub(crate) fn monomorphize_generic_machine_value_calls(
    program: &mut TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<DeclarationSelection, Vec<Diagnostic>> {
    let mut changed_owners = Vec::new();
    loop {
        let source_owners = selection::bind_closed_contract_calls(program)?;
        let source_changed = !source_owners.is_empty();
        changed_owners.extend(source_owners);
        let named_owners = validation::resolve_named_result_overloads(program)?;
        let named_changed = !named_owners.is_empty();
        changed_owners.extend(named_owners);
        crate::conformance::explicit_requirement_calls::refresh_occurrence_selections(program)?;
        let unavailable = crate::constructed_results::retain_constructed_result_types(program)?;
        let inferred_owners = result_locals::refresh(program, &unavailable)?;
        let inferred_changed = !inferred_owners.is_empty();
        changed_owners.extend(inferred_owners);
        let subjects = result_locals::materialize_dispatch_subjects(program);
        if !subjects.is_empty() {
            changed_owners.extend(subjects);
            continue;
        }
        changed_owners.extend(result_locals::materialize_reference_observations(program)?);
        domain_applications::materialize(program)?;
        materialize_static_argument_types(program);
        changed_owners.extend(cleanup_applications::materialize(program)?);
        native_query_lifetimes::retain(program)?;
        crate::preparation::elaborate_projected_receivers(program)?;
        let contextual_owners = contextual_calls::close(program)?;
        let contextual_changed = !contextual_owners.is_empty();
        changed_owners.extend(contextual_owners);
        crate::constructed_results::retain_constructed_result_types(program)?;
        validation::static_arguments::validate_source_const_recipes(program, &mut Vec::new())?;
        body_rewriting::fold_closed_const_machine_arguments(program)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        trait_implementation_arguments::prepare(program)?;
        layout_storage_policies::prepare(program)?;
        program
            .prepare_closed_conformance_scopes()
            .map_err(|message| vec![Diagnostic::error(message)])?;
        contextual_calls::prepare_conformance_machine_demands(program)?;
        let captured_owners = captured_rows::close(program)?;
        let captures_changed = !captured_owners.is_empty();
        changed_owners.extend(captured_owners);
        let subjects = result_locals::materialize_dispatch_subjects(program);
        if !subjects.is_empty() {
            changed_owners.extend(subjects);
            continue;
        }
        let candidates = candidate::collect(program);
        let callee_states = candidate::callees(program, &candidates);
        if candidates.is_empty() {
            if source_changed
                || captures_changed
                || contextual_changed
                || named_changed
                || inferred_changed
            {
                continue;
            }
            if contextual_calls::prepare_installed_interrupts(program)? {
                continue;
            }
            return Ok(DeclarationSelection::from_symbols(changed_owners));
        }
        let (contract_expressions, logical_selections) = {
            let scopes = selection::contract_expression_scopes(program);
            let expressions = selection::contract_expression_handles_from_scopes(program, &scopes);
            let selections =
                collect_contract_call_selections(program, &scopes, &candidates, &callee_states);
            (expressions, selections)
        };
        for selection in &logical_selections {
            let candidate =
                candidate_for_selection(program, &candidates[selection.candidate_index], selection);
            applications::retain_call(program, &candidate, selection);
        }
        let mut selections =
            collect_call_selections(program, &candidates, &callee_states, &contract_expressions);
        selections.retain(|selection| !selection.explicit_argument_overflow);
        selections.retain(|selection| {
            !const_arguments::has_pending_type_arguments(program, &selection.type_bindings)
        });
        for selection in &selections {
            let candidate =
                candidate_for_selection(program, &candidates[selection.candidate_index], selection);
            applications::retain_call(program, &candidate, selection);
        }
        let generic_owners =
            result_locals::refresh_generic_call_results(program, &candidates, &selections)?;
        let generic_changed = !generic_owners.is_empty();
        changed_owners.extend(generic_owners);
        if reflection::elaborate_calls(program, &candidates, &selections, &mut changed_owners)? {
            continue;
        }
        let mut diagnostics = Vec::new();
        let mut applied_any =
            select_calls(program, &candidates, &mut selections, &mut changed_owners)?
                || source_changed
                || captures_changed
                || contextual_changed
                || named_changed
                || inferred_changed
                || generic_changed;
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            if let Some(selection) = selections.iter().find(|selection| {
                selection.candidate_index == candidate_index
                    && selection.conflicted
                    && holds_partial_static_evidence(selection)
            }) {
                diagnostics.push(incomplete_selection_diagnostic(
                    program, candidate, selection,
                ));
                continue;
            }
            match apply_call_specializations(
                program,
                candidate,
                &selections,
                candidate_index,
                &mut changed_owners,
            ) {
                Ok(changed) => applied_any |= changed,
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let observations = result_locals::materialize_reference_observations(program)?;
        if !observations.is_empty() {
            changed_owners.extend(observations);
            continue;
        }
        if !applied_any {
            for (candidate_index, candidate) in candidates.iter().enumerate() {
                if let Some(selection) = selections.iter().find(|selection| {
                    selection.candidate_index == candidate_index
                        && holds_partial_static_evidence(selection)
                }) {
                    diagnostics.push(incomplete_selection_diagnostic(
                        program, candidate, selection,
                    ));
                }
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
            if enforce_complete_concrete_selections
                && program.pending_const_range_endpoints.is_empty()
            {
                for (candidate_index, candidate) in candidates.iter().enumerate() {
                    if let Some(selection) = selections.iter().find(|selection| {
                        selection.candidate_index == candidate_index
                            && !selection.caller_is_generic
                            && !selection.is_complete()
                            && matches!(selection.site, CallSite::Statement(_))
                    }) {
                        diagnostics.push(incomplete_selection_diagnostic(
                            program, candidate, selection,
                        ));
                    }
                }
                if !diagnostics.is_empty() {
                    return Err(diagnostics);
                }
            }
            if contextual_calls::prepare_installed_interrupts(program)? {
                continue;
            }
            return Ok(DeclarationSelection::from_symbols(changed_owners));
        }
    }
}

pub(crate) fn materialize_static_argument_types(program: &mut TypedTrees) {
    fn collect(
        program: &dyn ::validation::CheckingInput,
        arguments: &[StaticMachineArgument],
        literals: &mut Vec<String>,
        types: &mut Vec<(SymbolHandle, typed_trees::name::Identifier)>,
    ) {
        for argument in arguments {
            if let Some(literal) = const_arguments::spelling(program, argument)
                && !literals.contains(&literal)
            {
                literals.push(literal);
            }
            if argument.application.is_none()
                && argument.symbol.is_valid()
                && (matches!(
                    program.symbols.get(argument.symbol).kind,
                    SymbolKind::Data | SymbolKind::BuiltinType
                ) || program.data_type_parameters.iter().any(|(_, parameter)| {
                    parameter.symbol == argument.symbol
                        && matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Type
                                | typed_trees::data::TypeParameterKind::Const { .. }
                                | typed_trees::data::TypeParameterKind::Value { .. }
                        )
                }))
                && let Some(name) = argument.path.last()
                && !types.iter().any(|(symbol, _)| *symbol == argument.symbol)
            {
                types.push((argument.symbol, name.clone()));
            }
            if let Some(application) = &argument.application {
                collect(program, &application.arguments, literals, types);
            }
        }
    }

    let mut literals = Vec::new();
    let mut types = Vec::new();
    for (_, expression) in expressions_of(program).iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            collect(program, &call.machine_arguments, &mut literals, &mut types);
            if let Some(query) = &call.native_code_layout {
                collect(
                    program,
                    std::slice::from_ref(&query.operand),
                    &mut literals,
                    &mut types,
                );
            }
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call() {
                    collect(program, &call.machine_arguments, &mut literals, &mut types);
                }
            }
        }
    }
    for (handle, _) in expressions_of(program).static_machine_applications() {
        collect(
            program,
            expressions_of(program).static_machine_application_arguments(handle),
            &mut literals,
            &mut types,
        );
    }
    literals.extend(
        program
            .type_reference_table
            .fixed_array_lengths()
            .filter_map(|(_, length)| match length {
                typed_trees::types::FixedArrayLength::Literal(value) => Some(value.to_string()),
                typed_trees::types::FixedArrayLength::ConstParameter { .. }
                | typed_trees::types::FixedArrayLength::Expression(_)
                | typed_trees::types::FixedArrayLength::ConstCall { .. } => None,
            }),
    );
    for (_, length) in program.type_reference_table.fixed_array_lengths() {
        if let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = length {
            types.push((*symbol, name.clone()));
        }
    }
    literals.sort();
    literals.dedup();
    for literal in literals {
        let exists = program
            .type_reference_table
            .named_references()
            .any(|(_, symbol, name)| !symbol.is_valid() && name == literal);
        if !exists {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::generated(literal),
                });
        }
    }
    for (symbol, name) in types {
        if program
            .type_reference_table
            .find_named_type_reference(symbol)
            .is_none()
        {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named { symbol, name });
        }
    }
    let mut atoms = Vec::new();
    for (_, expression) in expressions_of(program).iter_expressions() {
        let atom = match expression {
            ExpressionNode::Integer(literal) => literal
                .landing()
                .map(|landing| typed_trees::types::builtin_type_atom(landing.landed_type)),
            ExpressionNode::Float(literal) => literal.landing().map(|format| match format {
                numerics::literals::FloatFormat::F32 => symbols::BuiltinTypeAtom::F32,
                numerics::literals::FloatFormat::F64 => symbols::BuiltinTypeAtom::F64,
            }),
            ExpressionNode::Boolean(_) => Some(symbols::BuiltinTypeAtom::Bool),
            _ => None,
        };
        if let Some(atom) = atom
            && !atoms.contains(&atom)
        {
            atoms.push(atom);
        }
    }
    for atom in atoms {
        let Some(symbol) = program
            .symbols
            .child_handles(program.symbols.root())
            .and_then(|mut children| {
                children.find(|symbol| program.symbols.builtin_type_atom(*symbol) == Some(atom))
            })
        else {
            continue;
        };
        if program
            .type_reference_table
            .find_named_type_reference(symbol)
            .is_none()
        {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: typed_trees::name::Identifier::generated_static(atom.symbol_name()),
                });
        }
    }
}

pub(crate) mod operator_candidates {
    use super::{
        Diagnostic, ExpressionNode, StatementNode, SymbolHandle, TypeParameterKind,
        TypeReferenceHandle, TypedTrees,
    };
    use crate::monomorphization::body_cloning::{clone_specialized_machine, copy_type_reference};
    use crate::monomorphization::{
        Candidate, SpecializationKey, candidate, materialize_static_argument_types, saved_calls,
    };
    use crate::tables::expressions_of;
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use typed_trees::operator::ClosedOperatorApplicationArgument;
    use typed_trees::operator::closed_operator_application_for_operands;
    use typed_trees::operator::declaration_by_symbol;
    use typed_trees::operator::resolve_satisfied_checked_operator_for_conformance;
    use typed_trees::statement::StatementRootRole;
    use validation::collect_expression_tree;
    use validation::contract_identities::{
        accepted_template_commitment, normalized_machine_identity,
    };
    use validation::declared_place_type_raw;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct GenericOperatorCandidate {
        pub requirement_operator: SymbolHandle,
        pub realization_machine: SymbolHandle,
    }

    pub(crate) struct OperatorCandidateTemplates {
        authored: TypedTrees,
    }

    impl OperatorCandidateTemplates {
        pub(crate) fn candidate_requests(
            program: &dyn ::validation::CheckingInput,
        ) -> Vec<GenericOperatorCandidate> {
            let mut requests = Vec::new();
            for machine in program.machines() {
                if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || !machine.body_is_present
                    || program.machine_type_parameters(machine).is_empty()
                {
                    continue;
                }
                for conformance in program.machine_trait_conformances(machine) {
                    if conformance.external_binding.is_some()
                        || conformance.via_expression.is_valid()
                    {
                        continue;
                    }
                    let Some(operator) = resolve_satisfied_checked_operator_for_conformance(
                        program,
                        machine,
                        conformance,
                    ) else {
                        continue;
                    };
                    if !operator.is_boundary {
                        continue;
                    }
                    let request = GenericOperatorCandidate {
                        requirement_operator: operator.symbol,
                        realization_machine: machine.symbol,
                    };
                    if !requests.contains(&request) {
                        requests.push(request);
                    }
                }
            }
            requests
        }

        pub(crate) fn prepare(
            program: &dyn ::validation::CheckingInput,
            selected: &[GenericOperatorCandidate],
        ) -> Option<Self> {
            if selected.is_empty() {
                return None;
            }
            let mut authored = program.typed_program().clone();
            materialize_static_argument_types(&mut authored);
            Some(Self { authored })
        }
    }

    pub(crate) fn specialize_operator_candidates(
        templates: &OperatorCandidateTemplates,
        program: &mut TypedTrees,
        selected: &[GenericOperatorCandidate],
    ) -> Result<usize, Vec<Diagnostic>> {
        if selected.is_empty() {
            return Ok(0);
        }
        materialize_static_argument_types(program);
        let mut diagnostics = Vec::new();
        let mut materialized = 0_usize;
        let templates = &templates.authored;

        for request in selected {
            let Some(machine_index) =
                templates.machine_position_by_symbol(request.realization_machine)
            else {
                diagnostics.push(Diagnostic::error(
                    "generic operator candidate names no realization machine",
                ));
                continue;
            };
            let template_machine = templates.machines()[machine_index].clone();
            let Some(operator) =
                declaration_by_symbol(templates, request.requirement_operator).cloned()
            else {
                diagnostics.push(Diagnostic::error(
                    "generic operator candidate names no operator requirement",
                ));
                continue;
            };
            if !templates
                .machine_trait_conformances(&template_machine)
                .iter()
                .any(|conformance| {
                    resolve_satisfied_checked_operator_for_conformance(
                        templates,
                        &template_machine,
                        conformance,
                    )
                    .is_some_and(|resolved| resolved.symbol == operator.symbol)
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "generic operator candidate `{}` does not realize its exact requirement",
                    template_machine.name,
                )));
                continue;
            }

            let current_operator = declaration_by_symbol(program, request.requirement_operator)
                .expect("candidate requirement survives specialization");
            let applications = match selected_operator_applications(program, current_operator) {
                Ok(applications) => applications,
                Err(mut errors) => {
                    diagnostics.append(&mut errors);
                    continue;
                }
            };
            if applications.is_empty() {
                continue;
            }
            let template = candidate::from_machine(templates, machine_index);
            if applications.iter().all(|application| {
                materialized_application(
                    program,
                    templates,
                    &template_machine,
                    &template,
                    operator.symbol,
                    application,
                )
            }) {
                continue;
            }
            let mut source = templates.clone();
            saved_calls::replay(&mut source, program, &template_machine);
            let mut concrete = Vec::<(SpecializationKey, Candidate)>::new();
            for application in applications {
                let application = copy_application_types(program, &mut source, &application)?;
                let candidate = match selected_operator_candidate_for_application(
                    &source,
                    &template,
                    &template_machine,
                    &application,
                ) {
                    Ok(candidate) => candidate,
                    Err(error) => {
                        diagnostics.push(error);
                        continue;
                    }
                };
                let key = SpecializationKey {
                    type_arguments: candidate
                        .type_bindings
                        .iter()
                        .map(|binding| {
                            source.normalized_type_identity_text(
                                binding.expect("complete selected type"),
                            )
                        })
                        .collect(),
                    const_arguments: candidate
                        .const_bindings
                        .iter()
                        .map(|binding| {
                            source.normalized_type_identity_text(
                                binding.expect("complete selected const"),
                            )
                        })
                        .collect(),
                    machine_arguments: Vec::new(),
                    evidence_arguments: Vec::new(),
                };
                if has_materialized_specialization(
                    program,
                    template_machine.symbol,
                    operator.symbol,
                    &key,
                ) {
                    continue;
                }
                if !concrete.iter().any(|(retained, _)| retained == &key) {
                    concrete.push((key, candidate));
                }
            }
            if concrete.is_empty() {
                continue;
            }

            let candidates = concrete
                .into_iter()
                .map(|(_, candidate)| candidate)
                .collect::<Vec<_>>();

            let normalized_template_identity =
                normalized_machine_identity(&source, &template_machine)
                    .expect("selected generic provider has a normalized template identity");
            let accepted_template_commitment = accepted_template_commitment(&source, machine_index);

            for (ordinal, candidate) in candidates.iter().enumerate() {
                match clone_specialized_machine(
                    Some(&source),
                    program,
                    candidate,
                    ordinal,
                    normalized_template_identity.clone(),
                    accepted_template_commitment.clone(),
                ) {
                    Ok(_) => {
                        materialized += 1;
                    }
                    Err(errors) => diagnostics.extend(errors),
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(materialized)
        } else {
            Err(diagnostics)
        }
    }

    fn materialized_application(
        program: &dyn ::validation::CheckingInput,
        templates: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        template: &Candidate,
        requirement: SymbolHandle,
        application: &[ClosedOperatorApplicationArgument],
    ) -> bool {
        use typed_trees::operator::ClosedOperatorApplicationArgument;
        let parameters = templates.machine_type_parameters(machine);
        if parameters.len() != application.len()
            || !template.template.machine_parameters.is_empty()
            || !template.template.evidence_parameters.is_empty()
        {
            return false;
        }
        let mut key = SpecializationKey {
            type_arguments: Vec::new(),
            const_arguments: Vec::new(),
            machine_arguments: Vec::new(),
            evidence_arguments: Vec::new(),
        };
        for (parameter, argument) in parameters.iter().zip(application) {
            match (&parameter.kind, argument) {
                (
                    TypeParameterKind::Type,
                    ClosedOperatorApplicationArgument::Type { type_reference, .. },
                ) => key
                    .type_arguments
                    .push(program.normalized_type_identity_text(*type_reference)),
                (
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                    ClosedOperatorApplicationArgument::Const { value, .. },
                ) => {
                    let Some(reference) = const_identity_type_reference(program, value) else {
                        return false;
                    };
                    key.const_arguments
                        .push(program.normalized_type_identity_text(reference));
                }
                _ => return false,
            }
        }
        has_materialized_specialization(program, machine.symbol, requirement, &key)
    }

    fn copy_application_types(
        program: &dyn ::validation::CheckingInput,
        source: &mut TypedTrees,
        application: &[ClosedOperatorApplicationArgument],
    ) -> Result<Vec<ClosedOperatorApplicationArgument>, super::body_cloning::TypeCopyError> {
        application
            .iter()
            .map(|argument| {
                Ok(match argument {
                    ClosedOperatorApplicationArgument::Type {
                        binder_symbol,
                        type_reference,
                    } => ClosedOperatorApplicationArgument::Type {
                        binder_symbol: *binder_symbol,
                        type_reference: copy_type_reference(
                            Some(program),
                            source,
                            *type_reference,
                            &[],
                        )?,
                    },
                    ClosedOperatorApplicationArgument::Const {
                        binder_symbol,
                        declared_carrier,
                        value,
                    } => ClosedOperatorApplicationArgument::Const {
                        binder_symbol: *binder_symbol,
                        declared_carrier: copy_type_reference(
                            Some(program),
                            source,
                            *declared_carrier,
                            &[],
                        )?,
                        value: value.clone(),
                    },
                })
            })
            .collect()
    }

    fn has_materialized_specialization(
        program: &dyn ::validation::CheckingInput,
        template: SymbolHandle,
        requirement: SymbolHandle,
        key: &SpecializationKey,
    ) -> bool {
        program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.template == template
                    && specialization.type_argument_identities == key.type_arguments
                    && specialization.const_argument_identities == key.const_arguments
                    && specialization.machine_arguments.is_empty()
                    && specialization.conformance_arguments.is_empty()
                    && specialization
                        .operator_realizations
                        .iter()
                        .any(|realization| realization.requirement_symbol == requirement)
            })
    }

    fn selected_operator_candidate_for_application<'template>(
        program: &dyn ::validation::CheckingInput,
        template: &'template Candidate<'_>,
        machine: &typed_trees::machine::Machine,
        application: &[ClosedOperatorApplicationArgument],
    ) -> Result<Candidate<'template>, Diagnostic> {
        let machine_parameters = program.machine_type_parameters(machine);
        if machine_parameters.len() != application.len()
            || !template.template.machine_parameters.is_empty()
            || !template.template.evidence_parameters.is_empty()
        {
            return Err(Diagnostic::error(format!(
                "generic operator candidate `{}` has no supported exact type/const specialization tuple",
                machine.name,
            )));
        }
        let mut candidate = template.borrowed();
        for (parameter, argument) in machine_parameters.iter().zip(application) {
            match (&parameter.kind, argument) {
                (
                    TypeParameterKind::Type,
                    ClosedOperatorApplicationArgument::Type { type_reference, .. },
                ) => {
                    let index = candidate
                        .template
                        .type_parameters
                        .iter()
                        .position(|(symbol, _)| *symbol == parameter.symbol)
                        .expect("selected type parameter retained by candidate");
                    candidate.type_bindings[index] = Some(*type_reference);
                }
                (
                    TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. },
                    ClosedOperatorApplicationArgument::Const { value, .. },
                ) => {
                    let Some(binding) = const_identity_type_reference(program, value) else {
                        return Err(Diagnostic::error(format!(
                            "generic operator candidate `{}` has a const application without a supported canonical integer carrier",
                            machine.name,
                        )));
                    };
                    let index = candidate
                        .template
                        .const_parameters
                        .iter()
                        .position(|(symbol, _, _)| *symbol == parameter.symbol)
                        .expect("selected const parameter retained by candidate");
                    candidate.const_bindings[index] = Some(binding);
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "generic operator candidate `{}` changes its requirement telescope category",
                        machine.name,
                    )));
                }
            }
        }
        Ok(candidate)
    }

    fn const_identity_type_reference(
        program: &dyn ::validation::CheckingInput,
        value: &language_semantics::const_value::CanonicalConstIdentity,
    ) -> Option<TypeReferenceHandle> {
        let DecodedCanonicalConstValue::Integer { value, .. } = value.decode_encoding()? else {
            return None;
        };
        let value = value.to_string();
        program
            .type_reference_table
            .named_references()
            .find(|(_, symbol, name)| !symbol.is_valid() && *name == value)
            .map(|(handle, _, _)| handle)
    }

    fn selected_operator_applications(
        program: &dyn ::validation::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
    ) -> Result<Vec<Vec<ClosedOperatorApplicationArgument>>, Vec<Diagnostic>> {
        let mut applications = Vec::new();
        let mut diagnostics = Vec::new();
        if program.machine_by_symbol(operator.symbol).is_some() {
            for receipt in program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.template == operator.symbol)
            {
                if !receipt.binding_application.is_valid() {
                    diagnostics.push(Diagnostic::error(
                        "closed operator requirement lost its static application",
                    ));
                    continue;
                }
                let retained =
                    expressions_of(program).static_machine_application(receipt.binding_application);
                let application = (retained.binding_declaration == operator.symbol)
                    .then(|| {
                        typed_trees::operator::closed_operator_application_from_bindings(
                            program,
                            operator,
                            expressions_of(program)
                                .static_machine_application_bindings(receipt.binding_application),
                        )
                    })
                    .flatten();
                let Some(application) = application else {
                    diagnostics.push(Diagnostic::error(
                        "closed operator requirement has no complete typed binder application",
                    ));
                    continue;
                };
                match validation::validate_closed_operator_application(
                    program,
                    operator,
                    &application,
                ) {
                    Ok(()) => {
                        if !applications.contains(&application) {
                            applications.push(application);
                        }
                    }
                    Err(error) => diagnostics.push(error),
                }
            }
            return if diagnostics.is_empty() {
                Ok(applications)
            } else {
                Err(diagnostics)
            };
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_index, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    for root in executable_statement_expression_roots(program, statement) {
                        let mut expressions = Vec::new();
                        collect_expression_tree(program, root, &mut expressions);
                        for expression in expressions {
                            match selected_operator_application_at_expression(
                                program,
                                operator,
                                machine,
                                state,
                                statement_index,
                                expression,
                            ) {
                                Ok(Some(application)) if !application.is_empty() => {
                                    if !applications.contains(&application) {
                                        applications.push(application);
                                    }
                                }
                                Ok(Some(_)) | Ok(None) => {}
                                Err(error) => diagnostics.push(error),
                            }
                        }
                    }
                }
            }
        }
        if diagnostics.is_empty() {
            Ok(applications)
        } else {
            Err(diagnostics)
        }
    }

    pub(crate) fn executable_statement_expression_roots(
        program: &dyn ::validation::CheckingInput,
        statement: &StatementNode,
    ) -> Vec<typed_trees::expression::ExpressionHandle> {
        program
            .tables
            .expression_roots(statement)
            .into_iter()
            .filter(|(role, root)| *role != StatementRootRole::AssemblyFact && root.is_valid())
            .map(|(_, root)| root)
            .collect()
    }

    fn selected_operator_application_at_expression(
        program: &dyn ::validation::CheckingInput,
        operator: &typed_trees::operator::OperatorDefinition,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<Option<Vec<ClosedOperatorApplicationArgument>>, Diagnostic> {
        let (operands, explicit_static_arguments) =
            match expressions_of(program).expression(expression) {
                ExpressionNode::Call(call) => {
                    if typed_trees::operator::resolve_named_expression_call(program, call)
                        .is_none_or(|resolved| resolved.symbol != operator.symbol)
                    {
                        return Ok(None);
                    }
                    let explicit = expressions_of(program).expression_handles(call.arguments);
                    let parameters = program.operator_parameters(operator);
                    let mut operands = Vec::with_capacity(explicit.len() + 1);
                    if call.receiver.is_valid() && parameters.len() == explicit.len() + 1 {
                        operands.push(call.receiver);
                    }
                    operands.extend_from_slice(explicit);
                    (operands, Some(call.machine_arguments.as_ref()))
                }
                ExpressionNode::Binary(binary) => {
                    let Some(spelling) =
                        typed_trees::operator::binary_operator_spelling(binary.operator)
                    else {
                        return Ok(None);
                    };
                    if operator.spelling != Some(spelling) {
                        return Ok(None);
                    }
                    (vec![binary.left, binary.right], None)
                }
                ExpressionNode::Indexed(indexed)
                    if !matches!(
                        expressions_of(program).expression(indexed.index),
                        ExpressionNode::Range(_)
                    ) && operator.spelling
                        == Some(language_core::operator_spelling::OperatorSpelling::Index) =>
                {
                    (vec![indexed.collection, indexed.index], None)
                }
                _ => return Ok(None),
            };
        let origin = checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: Default::default(),
        };
        let operand_types = operands
            .iter()
            .map(|operand| {
                validation::expression_type_reference_for_origin(program, *operand, origin)
                    .or_else(|| declared_place_type_raw(program, machine, Some(state), *operand))
                    .or_else(|| {
                        validation::landed_integer_literal_type_reference(program, *operand)
                    })
            })
            .collect::<Vec<_>>();

        if let Some(explicit_static_arguments) = explicit_static_arguments {
            return match validation::validate_named_operator_application(
                program,
                operator,
                explicit_static_arguments,
                &operand_types,
            )? {
                Some(application) => Ok(Some(application)),
                None => Ok(None),
            };
        }
        let Some(spelling) = operator.spelling else {
            return Ok(None);
        };
        if !typed_trees::operator::resolve_spelling_for_operands(
            program,
            spelling,
            &operand_types,
            expressions_of(program).source_span(expression),
        )
        .iter()
        .any(|candidate| candidate.operator.symbol == operator.symbol)
        {
            return Ok(None);
        }
        let Some(application) =
            closed_operator_application_for_operands(program, operator, &operand_types)
        else {
            return Ok(None);
        };
        validation::validate_closed_operator_application(program, operator, &application)?;
        Ok(Some(application))
    }
}
pub(crate) use crate::monomorphization::selection::unique_complete_selections;
pub(crate) use operator_candidates::{OperatorCandidateTemplates, specialize_operator_candidates};

pub(crate) fn failed(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

pub(crate) fn runtime_value_subjects(
    bindings: &[Option<RuntimeValueBinding>],
) -> Vec<RuntimeValueBinding> {
    bindings.iter().flatten().cloned().collect()
}

pub(crate) fn incomplete_selection_diagnostic(
    program: &TypedTrees,
    candidate: &Candidate,
    selection: &CallSelection,
) -> Diagnostic {
    let owner = program.symbols.display_path(selection.lexical_owner, "::");
    let state = program.symbols.display_path(selection.lexical_state, "::");
    let location = if selection.lexical_state.is_valid() {
        format!(" in machine `{owner}` state `{state}`")
    } else if selection.lexical_owner.is_valid() {
        format!(" in declaration `{owner}`")
    } else {
        String::new()
    };
    let mut details = Vec::new();
    if let Some(reason) = &selection.conflict_reason {
        details.push(reason.clone());
    }
    for ((_, name), binding) in candidate
        .template
        .type_parameters
        .iter()
        .zip(&selection.type_bindings)
    {
        if binding.is_none() {
            details.push(format!("Type slot `{name}` has no proposal"));
        }
    }
    for ((_, name, _), binding) in candidate
        .template
        .const_parameters
        .iter()
        .zip(&selection.const_bindings)
    {
        if binding.is_none() {
            details.push(format!("Const slot `{name}` has no proposal"));
        }
    }
    for ((_, name, _), binding) in candidate
        .template
        .machine_parameters
        .iter()
        .zip(&selection.machine_bindings)
    {
        if binding.is_none() {
            details.push(format!("Machine slot `{name}` has no proposal"));
        }
    }
    for (ordinal, binding) in selection.evidence_bindings.iter().enumerate() {
        if binding.is_none() {
            details.push(format!(
                "evidence slot `{}` has no proposal",
                candidate.template.evidence_parameters[ordinal].binder_name
            ));
        }
    }
    for (kind, unresolved) in [
        ("Type", selection.unresolved_type_parameters),
        ("Const", selection.unresolved_const_parameters),
        ("Machine", selection.unresolved_machine_parameters),
        ("evidence", selection.unresolved_evidence_parameters),
    ] {
        if unresolved {
            details.push(format!(
                "{kind} proposals retain an open supplying parameter"
            ));
        }
    }
    let detail = if details.is_empty() {
        String::new()
    } else {
        format!(": {}", details.join("; "))
    };
    let diagnostic = Diagnostic::error(format!(
        "generic machine `{}` has a static selection, but its complete type/const/machine/conformance specialization tuple cannot be derived{location}{detail}",
        candidate.template.template_name,
    ));
    match selection.site {
        CallSite::Expression(expression) => {
            diagnostic.with_source_span(expressions_of(program).source_span(expression))
        }
        CallSite::Statement(statement) => match program.statement_table.statement(statement) {
            StatementNode::Call(call) => diagnostic.with_source_span(call.source_span),
            _ => diagnostic,
        },
    }
}

pub(crate) fn holds_partial_static_evidence(selection: &CallSelection) -> bool {
    !selection.caller_is_generic
        && !selection.is_complete()
        && (selection.const_bindings.iter().any(Option::is_some)
            || selection.machine_bindings.iter().any(Option::is_some)
            || selection.evidence_bindings.iter().any(Option::is_some))
}

pub(crate) fn apply_call_specializations(
    program: &mut TypedTrees,
    template: &Candidate,
    selections: &[CallSelection],
    candidate_index: usize,
    changed_owners: &mut Vec<SymbolHandle>,
) -> Result<bool, Vec<Diagnostic>> {
    let groups = unique_complete_selections(program, selections, candidate_index)
        .map_err(|diagnostic| vec![diagnostic])?;
    if groups.is_empty() {
        return Ok(false);
    }
    let concrete_candidates: Vec<Candidate> = groups
        .iter()
        .map(|(_, members)| candidate_for_selection(program, template, &selections[members[0]]))
        .collect();

    let normalized_template_identity = normalized_machine_identity(
        program,
        &program.machines()[template.template.machine_index],
    )
    .expect("generic template must retain a normalized callable identity");
    let accepted_template_commitment =
        accepted_template_commitment(program, template.template.machine_index);

    let mut selected_call_rewrites = Vec::new();
    for ((_, members), candidate) in groups.iter().zip(&concrete_candidates) {
        let selection = &selections[members[0]];
        let existing = saved_calls::selected_instance(program, program, template, selection)
            .map(|instance| instance.instance);
        let state_symbols = if let Some(existing) = existing {
            let Some(machine) = program.machine_by_symbol(existing) else {
                return Err(vec![Diagnostic::error(
                    "specialization receipt has no live instance",
                )]);
            };
            let states = program.machine_states(machine);
            if states.len() != template.template.state_symbols.len() {
                return Err(vec![Diagnostic::error(
                    "specialization instance lost its template state correspondence",
                )]);
            }
            template
                .template
                .state_symbols
                .iter()
                .copied()
                .zip(states.iter().map(|state| state.symbol))
                .collect()
        } else {
            let ordinal = program
                .machine_specializations
                .iter()
                .filter(|instance| instance.template == template.template.template_symbol)
                .count();
            clone_specialized_machine(
                None,
                program,
                candidate,
                ordinal,
                normalized_template_identity.clone(),
                accepted_template_commitment.clone(),
            )?
        };
        for selection_index in members {
            let selection = &selections[*selection_index];
            let Some((_, concrete_state)) = state_symbols
                .iter()
                .find(|(template_state, _)| *template_state == selection.callee_symbol)
            else {
                return Err(vec![Diagnostic::error(
                    "selected call has no exact specialization state",
                )]);
            };
            let arguments = runtime_value_subjects(&selection.runtime_value_bindings);
            let subjects = crate::monomorphization::body_rewriting::runtime_call_operands(
                program,
                selection.site,
                selection.lexical_state,
                &arguments,
            );
            selected_call_rewrites.push((
                selection.site,
                selection.lexical_owner,
                *concrete_state,
                subjects,
            ));
        }
    }
    for (site, owner, concrete_state, subjects) in selected_call_rewrites {
        rewrite_selected_call(program, site, concrete_state, &subjects);
        changed_owners.push(owner);
    }
    Ok(true)
}

pub(crate) fn candidate_for_selection<'template>(
    program: &dyn validation::CheckingInput,
    template: &'template Candidate<'_>,
    selection: &CallSelection,
) -> Candidate<'template> {
    Candidate {
        template: std::borrow::Cow::Borrowed(&template.template),
        binding_application: selection
            .site
            .authored_machine_application(program)
            .unwrap_or_else(StaticMachineApplicationHandle::invalid),
        type_bindings: selection.type_bindings.clone(),
        const_bindings: selection.const_bindings.clone(),
        runtime_value_bindings: selection.runtime_value_bindings.clone(),
        machine_bindings: selection.machine_bindings.clone(),
        evidence_bindings: selection.evidence_bindings.clone(),
        conflicted: selection.conflicted,
    }
}

pub(crate) fn candidate_conformance_fingerprint_arguments(candidate: &Candidate) -> Vec<String> {
    candidate
        .evidence_bindings
        .iter()
        .map(|binding| {
            binding
                .as_ref()
                .expect("complete specialization")
                .display_name()
        })
        .collect()
}

pub(crate) fn finalize_type_domain_instance_identities(
    program: &mut TypedTrees,
    references: impl IntoIterator<Item = TypeReferenceHandle>,
) -> Result<(), Diagnostic> {
    let mut constraint_updates = Vec::new();
    for reference in references {
        let TypeReferenceNode::Constrained { constraints, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            continue;
        };
        let constraints = *constraints;
        for (offset, constraint) in program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .enumerate()
        {
            let TypeConstraintNode::Domain(constraint) = constraint else {
                continue;
            };
            if constraint.arguments.is_empty() || !constraint.symbol.is_valid() {
                continue;
            }
            let Some(domain) = program.domain_definition(constraint.symbol).cloned() else {
                continue;
            };
            let index_parameters = typed_trees::domain::index_parameters(program, &domain);
            let identity = indexed_domain_instance_name(
                program,
                &domain,
                index_parameters,
                &constraint.arguments,
            )?;
            constraint_updates.push((constraints, offset, identity, domain.semantic_roles));
        }
    }

    for (constraints, offset, identity, roles) in constraint_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let TypeConstraintNode::Domain(domain) =
            &mut program.type_reference_table.constraints_mut(constraints)[offset]
        else {
            unreachable!("collected domain constraint changed kind")
        };
        domain.semantic_id = semantic_id;
        domain.semantic_roles = language_semantics::DomainSemanticRoles {
            denotation_dimension: roles.denotation_dimension.map(|_| semantic_id),
            arithmetic_policy: roles.arithmetic_policy.map(|_| semantic_id),
        };
    }
    Ok(())
}

pub(crate) fn finalize_callable_signature_identities(
    program: &mut TypedTrees,
    signatures: impl IntoIterator<Item = typed_trees::signature::StateSignature>,
) -> Result<(), Diagnostic> {
    for signature in signatures {
        let references = typed_trees::references::collect_signature_references(program, &signature);
        finalize_referenced_domain_instance_identities(program, references)?;
    }
    Ok(())
}

pub(crate) fn finalize_domain_instance_identities(
    program: &mut TypedTrees,
    declarations: &DeclarationSelection,
) -> Result<(), Diagnostic> {
    let references =
        typed_trees::references::collect_selected_program_references(program, declarations);
    finalize_referenced_domain_instance_identities(program, references)
}

pub(crate) fn finalize_referenced_domain_instance_identities(
    program: &mut TypedTrees,
    references: typed_trees::references::ProgramReferences,
) -> Result<(), Diagnostic> {
    // Membership facts of plain domains also receive their identity below: an
    // authored `ensures result in D` is lowered without one. So the pass may be
    // skipped only when no domain is generic and every referenced membership
    // already has its identity. A program narrowed to its validation subjects
    // can lose every generic domain and still hold such a membership.
    if program
        .domain_definitions()
        .iter()
        .all(|domain| domain.type_parameters.is_empty())
        && references.proof_facts.iter().all(|handle| {
            program
                .proof_facts
                .get(*handle)
                .as_membership()
                .is_none_or(|membership| {
                    !membership.domain_symbol.is_valid() || membership.semantic_domain.is_valid()
                })
        })
    {
        return Ok(());
    }
    finalize_type_domain_instance_identities(program, references.type_references)?;
    let mut expressions = references.expressions.into_iter().collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    let cast_sites = expressions
        .into_iter()
        .filter_map(|handle| {
            let expression = expressions_of(program).expression(handle);
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            (cast.semantic_domain_symbol.is_valid() && !cast.semantic_domain_arguments.is_empty())
                .then_some((
                    handle,
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_arguments,
                ))
        })
        .collect::<Vec<_>>();
    let mut cast_updates = Vec::new();
    for (handle, symbol, arguments) in cast_sites {
        let Some(domain) = program.domain_definition(symbol).cloned() else {
            continue;
        };
        let index_parameters = typed_trees::domain::index_parameters(program, &domain);
        let arguments = program
            .type_reference_table
            .type_reference_handles(arguments);
        let identity = indexed_domain_instance_name(program, &domain, index_parameters, arguments)?;
        cast_updates.push((handle, identity));
    }
    for (handle, identity) in cast_updates {
        let semantic_id = program.semantic_domains.intern(&identity);
        let ExpressionNode::Cast(cast) = expressions_of_mut(program).expression_mut(handle) else {
            unreachable!("collected cast changed kind")
        };
        cast.semantic_domain_id = semantic_id;
    }

    let mut membership_updates = Vec::new();
    for handle in references.proof_facts {
        let fact = program.proof_facts.get(handle);
        let Some(membership) = fact.as_membership() else {
            continue;
        };
        if !membership.domain_symbol.is_valid() {
            continue;
        }
        let domain = program
            .domain_definition(membership.domain_symbol)
            .ok_or_else(|| Diagnostic::error("membership instance has no declared domain"))?;
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments);
        let parameters = typed_trees::domain::index_parameters(program, domain);
        if arguments.len() != membership.domain_arguments.len()
            || arguments.len() != parameters.len()
            || arguments.iter().any(|argument| {
                !program
                    .type_reference_table
                    .contains_type_reference(*argument)
            })
        {
            return Err(Diagnostic::error(
                "membership instance has missing or invalid domain arguments",
            ));
        }
        let identity = indexed_domain_instance_name(program, domain, parameters, arguments)?;
        membership_updates.push((handle, identity));
    }
    for (handle, identity) in membership_updates {
        let semantic_domain = program.semantic_domains.intern(&identity);
        if let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle) {
            membership.semantic_domain = semantic_domain;
        }
    }
    Ok(())
}

pub(crate) fn remapped_symbol(
    symbol: SymbolHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> SymbolHandle {
    symbols
        .iter()
        .find_map(|(before, after)| (*before == symbol).then_some(*after))
        .unwrap_or(symbol)
}

pub(crate) fn closed_operator_realizations_for_machine(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
) -> Result<Vec<typed_trees::operator::ClosedOperatorRealizationApplication>, Diagnostic> {
    let machine = program
        .machine_by_symbol(machine_symbol)
        .expect("specialized machine must remain in the typed program");
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| {
            typed_trees::operator::declaration_by_symbol(
                program,
                conformance.requirement_symbol,
            )
            .map(|operator| (conformance, operator))
        })
        .map(|(conformance, operator)| {
            typed_trees::operator::closed_operator_realization_application(
                program, machine, operator,
            )
            .filter(|application| application.requirement_symbol == conformance.requirement_symbol)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "specialized machine `{}` does not reconstruct one exact closed application of operator requirement `{}::{}`",
                    machine.name,
                    conformance.name,
                    conformance
                        .requirement
                        .as_ref()
                        .map_or("<missing>", |requirement| requirement.as_str()),
                ))
            })
        })
        .collect()
}

pub(crate) fn specialized_attached_data(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
    machine: &typed_trees::machine::Machine,
) -> Option<(typed_trees::name::Identifier, SymbolHandle)> {
    let attached = machine.attached_data.as_ref()?;
    let parameter_index = candidate
        .template
        .type_parameters
        .iter()
        .position(|(_, name)| name == attached.as_str());
    let Some(parameter_index) = parameter_index else {
        return Some((attached.clone(), machine.attached_data_symbol));
    };
    let binding = candidate.type_bindings[parameter_index]?;
    let symbol = program.type_reference_symbol(binding);
    match program.type_reference_table.type_reference(binding) {
        TypeReferenceNode::Named { name, .. } => Some((name.clone(), symbol)),
        TypeReferenceNode::Generic { base_name, .. } => Some((base_name.clone(), symbol)),
        _ => Some((attached.clone(), machine.attached_data_symbol)),
    }
}

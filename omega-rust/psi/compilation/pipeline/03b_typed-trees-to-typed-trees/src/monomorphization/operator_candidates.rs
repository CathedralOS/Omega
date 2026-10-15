use super::{
    Diagnostic, ExpressionNode, StatementNode, SymbolHandle, TypeParameterKind,
    TypeReferenceHandle, TypedTrees,
};
use crate::monomorphization::body_cloning::{clone_specialized_machine, copy_type_reference};
use crate::monomorphization::{
    Candidate, SpecializationKey, candidate, materialize_static_argument_types, saved_calls,
};
use typed_trees::operator::ClosedOperatorApplicationArgument;
use validation::collect_expression_tree;
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

/// One exact authored conformance edge. This is internal elaboration data,
/// never a provider choice supplied by Omega or the compiler coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GenericOperatorCandidate {
    pub requirement_operator: SymbolHandle,
    pub realization_machine: SymbolHandle,
}

pub(crate) struct OperatorCandidateTemplates {
    authored: TypedTrees,
}

impl OperatorCandidateTemplates {
    /// Boundary operators publish all explicitly conforming checked candidates.
    /// Build chooses among them only after Terminal publication. Discover the
    /// authored edges before ordinary specialization can mutate their binders;
    /// the existing fixed point materializes only actual closed demands.
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
                if conformance.external_binding.is_some() || conformance.via_expression.is_valid() {
                    continue;
                }
                let Some(operator) =
                    typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
                        program,
                        machine,
                        conformance,
                    )
                else {
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
        let Some(machine_index) = templates.machine_position_by_symbol(request.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(
                "generic operator candidate names no realization machine",
            ));
            continue;
        };
        let template_machine = templates.machines()[machine_index].clone();
        let Some(operator) =
            typed_trees::operator::declaration_by_symbol(templates, request.requirement_operator)
                .cloned()
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
                typed_trees::operator::resolve_satisfied_checked_operator_for_conformance(
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

        let current_operator =
            typed_trees::operator::declaration_by_symbol(program, request.requirement_operator)
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
        // Closed application identities can be compared in the live graph.
        // Unsupported telescope categories still reach candidate validation;
        // only exact tuples with an existing receipt can avoid source copying.
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
        // Replay and type imports mutate this request's graph. Keep that
        // working copy separate from the immutable authority and other requests.
        let mut source = templates.clone();
        saved_calls::replay(&mut source, program, &template_machine);
        let mut concrete = Vec::<(SpecializationKey, Candidate)>::new();
        for application in applications {
            let application = copy_application_types(program, &mut source, &application);
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
                        source
                            .normalized_type_identity_text(binding.expect("complete selected type"))
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

        let normalized_template_identity = normalized_machine_identity(&source, &template_machine)
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
    application: &[typed_trees::operator::ClosedOperatorApplicationArgument],
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
    application: &[typed_trees::operator::ClosedOperatorApplicationArgument],
) -> Vec<typed_trees::operator::ClosedOperatorApplicationArgument> {
    application
        .iter()
        .map(|argument| match argument {
            typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                binder_symbol,
                type_reference,
            } => typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                binder_symbol: *binder_symbol,
                type_reference: copy_type_reference(Some(program), source, *type_reference, &[]),
            },
            typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                binder_symbol,
                declared_carrier,
                value,
            } => typed_trees::operator::ClosedOperatorApplicationArgument::Const {
                binder_symbol: *binder_symbol,
                declared_carrier: copy_type_reference(
                    Some(program),
                    source,
                    *declared_carrier,
                    &[],
                ),
                value: value.clone(),
            },
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
    application: &[typed_trees::operator::ClosedOperatorApplicationArgument],
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
                typed_trees::operator::ClosedOperatorApplicationArgument::Type {
                    type_reference,
                    ..
                },
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
                typed_trees::operator::ClosedOperatorApplicationArgument::Const { value, .. },
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
    let language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
        value.decode_encoding()?
    else {
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
) -> Result<Vec<Vec<typed_trees::operator::ClosedOperatorApplicationArgument>>, Vec<Diagnostic>> {
    let mut applications = Vec::new();
    let mut diagnostics = Vec::new();
    if program.machine_by_symbol(operator.symbol).is_some() {
        // Token-bearing requirements use ordinary machine specialization.
        // Its retained application is the demand roster provider coverage
        // consumes too. Re-inferring from a rewritten call loses collection
        // adaptation or static arguments already consumed by specialization.
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
            let retained = program
                .expression_table
                .static_machine_application(receipt.binding_application);
            let application = (retained.binding_declaration == operator.symbol)
                .then(|| {
                    typed_trees::operator::closed_operator_application_from_bindings(
                        program,
                        operator,
                        program
                            .expression_table
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
            match validation::validate_closed_operator_application(program, operator, &application)
            {
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

pub(super) fn executable_statement_expression_roots(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
) -> Vec<typed_trees::expression::ExpressionHandle> {
    program
        .statement_table
        .expression_roots(statement)
        .into_iter()
        .filter(|(role, root)| {
            *role != typed_trees::statement::StatementRootRole::AssemblyFact && root.is_valid()
        })
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
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                if typed_trees::operator::resolve_named_expression_call(program, call)
                    .is_none_or(|resolved| resolved.symbol != operator.symbol)
                {
                    return Ok(None);
                }
                let explicit = program.expression_table.expression_handles(call.arguments);
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
                    program.expression_table.expression(indexed.index),
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
                .or_else(|| {
                    validation::declared_place_type_raw(program, machine, Some(state), *operand)
                })
                .or_else(|| validation::landed_integer_literal_type_reference(program, *operand))
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
            // A generic template may forward one of its own binders here.
            // That is a symbolic demand, not a concrete specialization and
            // not final coverage. A concrete clone is revisited by the outer
            // fixed-point loop; validation still rejects an emitted open use.
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
        program.expression_table.source_span(expression),
    )
    .iter()
    .any(|candidate| candidate.operator.symbol == operator.symbol)
    {
        return Ok(None);
    }
    let Some(application) = typed_trees::operator::closed_operator_application_for_operands(
        program,
        operator,
        &operand_types,
    ) else {
        // Generic templates and declaration-derived expressions may retain
        // the spelling while their operands are still open. They are symbolic
        // demand, not a concrete specialization request.
        return Ok(None);
    };
    validation::validate_closed_operator_application(program, operator, &application)?;
    Ok(Some(application))
}

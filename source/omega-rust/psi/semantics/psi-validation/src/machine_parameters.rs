//! MP2b: admission of compile-time machine-symbol arguments.
//!
//! Static selections are checked at the generic call edge. The selected
//! machine must be concrete, match the authored callable shape, stay within
//! the required service and operational ceilings, and conservatively refine
//! conjunctive requires/ensures facts. This pass never invents a callback
//! contract.

mod callable_shape;
mod contract_facts;
mod nominal_admission;
mod type_refinement;

use callable_shape::validate_selected_callable_shape;
pub(crate) use callable_shape::validate_trait_callable_parameter_refinement;
use nominal_admission::validate_nominal_machine_selection;

use psi_diagnostics::Diagnostic;
use psi_symbols::{SymbolHandle, SymbolKind};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementHandle, StatementNode};

#[derive(Clone, Copy)]
struct MachineSuspensionRow {
    symbol: SymbolHandle,
    transitive_may_suspend: bool,
}

#[derive(Clone, Copy)]
struct MachineBlockingRow {
    symbol: SymbolHandle,
    transitive_may_block: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatedNominalMachineUseSite {
    Statement(StatementHandle),
    Expression(ExpressionHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedNominalMachineUse {
    pub site: ValidatedNominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
}

fn project_operational_rows(
    operational: &psi_effects::OperationalPlan,
) -> (Vec<MachineSuspensionRow>, Vec<MachineBlockingRow>) {
    let suspensions = operational
        .machines()
        .iter()
        .map(|summary| MachineSuspensionRow {
            symbol: summary.symbol,
            transitive_may_suspend: summary.transitive_may_suspend,
        })
        .collect();
    let blockings = operational
        .machines()
        .iter()
        .map(|summary| MachineBlockingRow {
            symbol: summary.symbol,
            transitive_may_block: summary.transitive_may_block,
        })
        .collect();
    (suspensions, blockings)
}

pub(crate) fn validate_static_machine_arguments(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_static_machine_arguments_with_facts(program, diagnostics, &mut Vec::new());
}

fn validate_static_machine_arguments_with_facts(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
    nominal_uses: &mut Vec<ValidatedNominalMachineUse>,
) {
    let (service_reaches, suspensions, blockings) = {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        let (suspensions, blockings) = project_operational_rows(&operational);
        (service_reaches, suspensions, blockings)
    };
    let invocations = psi_effects::infer_synchronous_invocations(program);
    for (handle, expression) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            validate_call_selection(
                program,
                &suspensions,
                &blockings,
                &service_reaches,
                &invocations,
                ValidatedNominalMachineUseSite::Expression(handle),
                call.target_symbol,
                call.target.as_str(),
                &call.machine_arguments,
                diagnostics,
                nominal_uses,
            );
        }
    }

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if let StatementNode::Call(call) = statement {
                    validate_call_selection(
                        program,
                        &suspensions,
                        &blockings,
                        &service_reaches,
                        &invocations,
                        ValidatedNominalMachineUseSite::Statement(statement_handle),
                        call.target_symbol,
                        call.target.as_str(),
                        &call.machine_arguments,
                        diagnostics,
                        nominal_uses,
                    );
                }
            }
        }
    }
}

/// Run MP2b admission as a standalone pre-specialization gate. MP4 consumes
/// the static argument syntax, so its refinement proof must happen first.
pub fn validate_static_machine_selections(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    validate_static_machine_selections_with_facts(program).map(|_| ())
}

pub fn validate_static_machine_selections_with_facts(
    program: &TypedTrees,
) -> Result<Vec<ValidatedNominalMachineUse>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut nominal_uses = Vec::new();
    validate_static_machine_arguments_with_facts(program, &mut diagnostics, &mut nominal_uses);
    if diagnostics.is_empty() {
        Ok(nominal_uses)
    } else {
        Err(diagnostics)
    }
}

fn validate_call_selection(
    program: &TypedTrees,
    suspensions: &[MachineSuspensionRow],
    blockings: &[MachineBlockingRow],
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    invocations: &psi_effects::InvocationInferencePlan,
    site: ValidatedNominalMachineUseSite,
    target_symbol: SymbolHandle,
    target_name: &str,
    arguments: &[StaticMachineArgument],
    diagnostics: &mut Vec<Diagnostic>,
    nominal_uses: &mut Vec<ValidatedNominalMachineUse>,
) {
    if matches!(target_name, "select_provider" | "select_representation") {
        return;
    }
    let (requirements, generic_types): (Vec<_>, Vec<_>) = if let Some((callee, _)) =
        machine_and_state(program, target_symbol)
    {
        (
            program
                .machine_type_parameters(callee)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .machine_type_parameters(callee)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else if let Some((declaring_machine, signature)) =
        program.machine_parameter_signature(target_symbol)
    {
        (
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .machine_type_parameters(declaring_machine)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else if let Some(signature) = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .find(|signature| signature.symbol == target_symbol)
    {
        (
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Machine { contract } => Some((parameter, contract)),
                    _ => None,
                })
                .collect(),
            program
                .state_signature_type_parameters(signature)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect(),
        )
    } else {
        if !arguments.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                    "call `{target_name}` supplies static machine arguments, but its generic callee did not resolve"
                )));
        }
        return;
    };

    if let Some(projection) = arguments
        .iter()
        .find_map(|argument| argument.evidence_projection.as_ref())
    {
        diagnostics.push(Diagnostic::error(format!(
            "proof-static evidence projection `{}.{}` cannot select an executable machine parameter; erased evidence cannot eliminate into runtime computation",
            projection.term, projection.member
        )));
        return;
    }

    let machine_arguments = arguments
        .iter()
        .filter(|argument| {
            matches!(
                program.symbols.get(argument.symbol).kind,
                SymbolKind::State | SymbolKind::MachineParameter
            )
        })
        .collect::<Vec<_>>();
    if machine_arguments.len() != requirements.len() {
        diagnostics.push(Diagnostic::error(format!(
            "generic call `{target_name}` requires {} static machine argument(s), got {}",
            requirements.len(),
            machine_arguments.len()
        )));
        return;
    }

    let mut bindings = Vec::new();

    for (static_machine_ordinal, ((parameter, requirement), selected)) in
        requirements.into_iter().zip(machine_arguments).enumerate()
    {
        let rendered = selected
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if selected.application.is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` is a nested machine application; recursive specialization identity is not yet supported",
                parameter.name
            )));
            continue;
        }
        // A recursive generic body may forward its own authored machine
        // parameter (`map<F>(tail)`). This is not a concrete selection yet,
        // but it is already governed by exactly this requirement; the
        // eventual external selection is validated at the concrete call edge.
        if selected.symbol == parameter.symbol {
            continue;
        }
        if !selected.symbol.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "static machine argument `{rendered}` for `{}` does not resolve to a concrete machine",
                parameter.name
            )));
            continue;
        }
        let Ok(nominal_selection) = validate_nominal_machine_selection(
            program,
            target_name,
            parameter,
            requirement,
            selected.symbol,
            &rendered,
            diagnostics,
        ) else {
            continue;
        };
        let requirement = machine_parameter_signature(program, requirement);
        validate_selected_callable_shape(
            program,
            suspensions,
            blockings,
            service_reaches,
            invocations,
            target_name,
            parameter,
            requirement,
            selected.symbol,
            &rendered,
            &generic_types,
            &mut bindings,
            diagnostics,
        );
        if let Some(selection) = nominal_selection {
            nominal_uses.push(ValidatedNominalMachineUse {
                site,
                registration_operation: target_symbol,
                static_machine_ordinal: u32::try_from(static_machine_ordinal)
                    .expect("static machine argument ordinal overflow"),
                selected_machine: selection.selected_machine,
                selected_entry: selection.selected_entry,
                satisfaction_trait: selection.satisfaction_trait,
                satisfaction_requirement: selection.satisfaction_requirement,
                canonical_requirement_overload: selection.canonical_requirement_overload,
            });
        }
    }
}

/// N7 data-family admission uses the same refinement judgment as a generic
/// call, but its selected symbol is carried in a generic type argument rather
/// than an expression call node. Keeping this entry point here prevents proof
/// data from growing a weaker, shape-only callback check.
pub(crate) fn validate_data_machine_selection(
    program: &TypedTrees,
    family_name: &str,
    parameter: &TypeParameter,
    requirement: &MachineParameterContract,
    selected_symbol: SymbolHandle,
    selected_name: &str,
    generic_types: &[&TypeParameter],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if validate_nominal_machine_selection(
        program,
        family_name,
        parameter,
        requirement,
        selected_symbol,
        selected_name,
        diagnostics,
    )
    .is_err()
    {
        return;
    }
    let requirement = machine_parameter_signature(program, requirement);
    let (service_reaches, suspensions, blockings) = {
        let operational = psi_effects::infer_operational_may(program);
        let service_reaches = psi_effects::infer_service_reaches(program, &operational);
        let (suspensions, blockings) = project_operational_rows(&operational);
        (service_reaches, suspensions, blockings)
    };
    let invocations = psi_effects::infer_synchronous_invocations(program);
    validate_selected_callable_shape(
        program,
        &suspensions,
        &blockings,
        &service_reaches,
        &invocations,
        family_name,
        parameter,
        requirement,
        selected_symbol,
        selected_name,
        generic_types,
        &mut Vec::new(),
        diagnostics,
    );
}

fn machine_parameter_contract(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&TypeParameter, &psi_typed_trees::signature::StateSignature)> {
    let (parameter, contract) = machine_parameter_contract_definition(program, symbol)?;
    Some((parameter, machine_parameter_signature(program, contract)))
}

fn machine_parameter_contract_definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&TypeParameter, &MachineParameterContract)> {
    for machine in program.machines() {
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.machine_type_parameters(machine),
            symbol,
        ) {
            return Some(found);
        }
    }
    for data in program.data_definitions() {
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.data_type_parameters(data),
            symbol,
        ) {
            return Some(found);
        }
    }
    None
}

fn machine_parameter_contract_definition_in<'program>(
    program: &'program TypedTrees,
    parameters: &'program [TypeParameter],
    symbol: SymbolHandle,
) -> Option<(&'program TypeParameter, &'program MachineParameterContract)> {
    for parameter in parameters {
        let TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        if parameter.symbol == symbol {
            return Some((parameter, contract));
        }
        let signature = machine_parameter_signature(program, contract);
        if let Some(found) = machine_parameter_contract_definition_in(
            program,
            program.state_signature_type_parameters(signature),
            symbol,
        ) {
            return Some(found);
        }
    }
    None
}

fn machine_parameter_signature<'program>(
    program: &'program TypedTrees,
    contract: &'program MachineParameterContract,
) -> &'program psi_typed_trees::signature::StateSignature {
    program
        .machine_parameter_contract_view(contract)
        .expect("typed machine-parameter contract must retain a valid requirement identity")
        .signature()
}

fn machine_and_state(
    program: &TypedTrees,
    selected_symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    if !selected_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find_map(|machine| {
        let states = program.machine_states(machine);
        states
            .iter()
            .find(|state| state.symbol == selected_symbol)
            .or_else(|| {
                (machine.symbol == selected_symbol)
                    .then(|| states.first())
                    .flatten()
            })
            .map(|state| (machine, state))
    })
}

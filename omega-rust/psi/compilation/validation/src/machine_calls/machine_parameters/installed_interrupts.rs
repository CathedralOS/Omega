//! Check the registrar selected by this installation, without inventing a call.
//!
//! The source occurrence supplies diagnostics and proof context. The retained
//! registrar application supplies the independently checked generic contract.
//! These rows remain separate from Scope's own Validate/Work machine binders.

use super::*;
use typed_trees::expression::{InstalledInterruptOperationRequest, StaticMachineArgument};
use typed_trees::static_applications::StaticMachineApplicationHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedInstalledInterruptRegistration {
    pub site: ValidatedNominalMachineUseSite,
    pub control_service: language_semantics::ServiceReachId,
    pub application: StaticMachineApplicationHandle,
    pub target: SymbolHandle,
    pub registrar_owner: SymbolHandle,
    pub registrar_application_identity: String,
    pub registrar_declaring_application: String,
    pub registrar_arguments: Vec<String>,
    pub registrar_lifetimes: Vec<String>,
    pub timer_slot: StaticMachineArgument,
    pub slot_application: typed_trees::typed_trees::ClosedConformanceApplication,
    pub nominal_uses: Vec<ValidatedNominalMachineUse>,
    pub specializations: Vec<ValidatedRequirementCallSpecialization>,
}

pub(super) fn form(
    program: &dyn crate::CheckingInput,
    index: &MachineStateIndex,
    site: ValidatedNominalMachineUseSite,
    request: &InstalledInterruptOperationRequest,
    diagnostics: &mut Vec<Diagnostic>,
    selections: &mut FormedStaticMachineSelections,
) {
    let Some(target) = source_call_target(program, site) else {
        diagnostics.push(Diagnostic::error(
            "interrupt installation lost its source call",
        ));
        return;
    };
    let Some(machine) = program
        .machine_holding_state(target)
        .or_else(|| program.machine_by_symbol(target))
    else {
        diagnostics.push(Diagnostic::error(
            "interrupt installation lost its source declaration",
        ));
        return;
    };
    match crate::compiler_operations::static_interrupt_declaration(program, machine) {
        Ok(Some(declaration))
            if declaration.operation
                == language_core::static_interrupts::StaticInterruptOperation::Install => {}
        Err(reason) => {
            diagnostics.push(Diagnostic::error(reason));
            return;
        }
        _ => {
            diagnostics.push(Diagnostic::error(
                "installation metadata does not belong to the core installation operation",
            ));
            return;
        }
    }
    let control_service = match machine_control_service(program, machine) {
        Ok(service) => service,
        Err(reason) => {
            diagnostics.push(Diagnostic::error(reason));
            return;
        }
    };
    let application = request.registrar_application;
    if !application.is_valid() || !request.registrar_target.is_valid() {
        // Open source bodies still carry their authored request. Their concrete
        // clones must prepare a complete registrar before executable admission.
        let owner = match site {
            ValidatedNominalMachineUseSite::Expression(handle) => program
                .expression_table
                .call_node(handle)
                .map(|call| call.authored_machine_application),
            ValidatedNominalMachineUseSite::Statement(handle) => program
                .statement_table
                .statement(handle)
                .as_call()
                .map(|call| call.authored_machine_application),
        }
        .filter(|application| application.is_valid())
        .map(|application| {
            program
                .expression_table
                .static_machine_application(application)
                .lexical_owner
        });
        if owner
            .and_then(|owner| program.machine_by_symbol(owner))
            .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
        {
            return;
        }
        diagnostics.push(Diagnostic::error(
            "interrupt installation requires its complete registrar application",
        ));
        return;
    }
    let slot_application = match crate::conformance_applications::close_conformance_application(
        program,
        &request.timer_slot,
    ) {
        Ok(application) => application,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let source = program
        .expression_table
        .static_machine_application(application);
    let identities = match registrar_identities(program, request) {
        Ok(identities) => identities,
        Err(reason) => {
            diagnostics.push(Diagnostic::error(reason));
            return;
        }
    };
    let mut registrar = FormedStaticMachineSelections::default();
    form_call_selection(
        program,
        index,
        CallSelection {
            site,
            target_symbol: source.registration_operation,
            selected_target: request.registrar_target,
            target_name: source.target.as_str(),
            arguments: program
                .expression_table
                .static_machine_application_arguments(application),
            application,
        },
        &[],
        diagnostics,
        &mut registrar,
    );
    // Operational and later crash refinement are the same obligations as for
    // an ordinary generic selection. Recording a request cannot discharge them.
    selections.pending_effects.extend(registrar.pending_effects);
    selections.rows.installed_interrupt_registrations.push(
        PreparedInstalledInterruptRegistration {
            site,
            control_service,
            application,
            target: request.registrar_target,
            registrar_owner: request.registrar_trait,
            registrar_application_identity: identities.receiving,
            registrar_declaring_application: identities.declaring,
            registrar_arguments: identities.arguments,
            registrar_lifetimes: identities.lifetimes,
            timer_slot: request.timer_slot.clone(),
            slot_application,
            nominal_uses: registrar.rows.nominal_uses,
            specializations: registrar.rows.requirement_call_specializations,
        },
    );
}

/// Select the intrinsic's service while checking still owns its source.
/// Later phases retain the selection; they do not rediscover core declarations.
fn machine_control_service(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Result<language_semantics::ServiceReachId, String> {
    let required = language_core::static_interrupts::StaticInterruptOperation::Install
        .required_service_identity()
        .ok_or("interrupt installation has no catalog service requirement")?;
    let mut selected = None;
    for service in program
        .service_reach_rows
        .services(machine.service_reach_row)
    {
        let definition = program
            .service_reaches
            .definition(*service)
            .ok_or("interrupt installer reaches an unknown service")?;
        if program.symbol_toolchain_relative_path(definition.symbol)
            != Some(std::path::Path::new("assembly.omg"))
        {
            continue;
        }
        let identity = program
            .normalized_hermetic_symbol_identity(definition.symbol)
            .map_err(|reason| format!("interrupt installer service identity: {reason}"))?;
        if identity != required {
            continue;
        }
        if selected.replace(*service).is_some() {
            return Err("interrupt installer repeats the exact core MachineControl service".into());
        }
    }
    selected.ok_or_else(|| "interrupt installer has no exact core MachineControl service".into())
}

struct RegistrarIdentities {
    receiving: String,
    declaring: String,
    arguments: Vec<String>,
    lifetimes: Vec<String>,
}

fn registrar_identities(
    program: &dyn crate::CheckingInput,
    request: &InstalledInterruptOperationRequest,
) -> Result<RegistrarIdentities, &'static str> {
    let receiving = request
        .registrar_receiving_scope
        .scope(program)
        .and_then(|scope| scope.portable_application_identity(program))
        .and_then(|identity| program.type_identity_text(identity))
        .ok_or("interrupt registrar lost its selected owner application")?;
    let closed = program
        .closed_requirement_application(request.registrar_target)
        .ok_or("interrupt registrar lost its closed requirement application")?;
    let declaring = program
        .type_identity_text(closed.portable_trait_application_identity)
        .ok_or("interrupt registrar lost its declaring owner application")?;
    if closed.local_arguments.len() != closed.portable_local_arguments.len() {
        return Err("interrupt registrar lost part of its member argument list");
    }
    let arguments = closed
        .local_arguments
        .iter()
        .zip(closed.portable_local_arguments)
        .map(|(argument, identity)| {
            program
                .requirement_argument_identity_text(*argument, *identity)
                .ok_or("interrupt registrar lost a member argument identity")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = program
        .expression_table
        .static_machine_application(request.registrar_application);
    let bindings = program
        .expression_table
        .static_machine_application_lifetimes(request.registrar_application);
    if source.lifetime_binding_declaration != closed.template {
        return Err("interrupt registrar lifetime bindings name a different member");
    }
    let parameters = typed_trees::type_identity::declaration_lifetime_parameters(
        program.typed_program(),
        closed.template,
    )
    .ok_or("interrupt registrar lost its declared lifetime parameters")?;
    if parameters.len() != bindings.len()
        || parameters
            .iter()
            .zip(bindings)
            .any(|(parameter, binding)| parameter != &binding.parameter)
    {
        return Err("interrupt registrar lifetime arguments differ from its declaration order");
    }
    let lifetimes = bindings
        .iter()
        .map(|binding| {
            typed_trees::type_identity::portable_applied_lifetime_identity(
                program.typed_program(),
                binding.argument.as_str(),
                source.lexical_owner,
                &[],
            )
            .and_then(|identity| program.type_identity_text(identity))
            .ok_or("interrupt registrar lost an applied lifetime identity")
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RegistrarIdentities {
        receiving,
        declaring,
        arguments,
        lifetimes,
    })
}

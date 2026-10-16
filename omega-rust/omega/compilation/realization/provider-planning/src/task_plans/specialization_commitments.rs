//! Exact task machine contracts and specialization commitments.

use crate::task_plans::start_selections::{TaskStartSelection, unique_task_activation_target};
use checked_trees::typed_trees::type_identity::TypeIdentityRequest;
use diagnostics::Diagnostic;
use task_plans::{TaskSpecializationCommitment, TaskStartOperation};

pub(crate) fn exact_task_machine_contract<'program>(
    check_facts: &'program checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<&'program checked_trees::MachineContractPlan, Vec<Diagnostic>> {
    let mut matches = check_facts
        .contract_plans
        .machines
        .iter()
        .filter(|plan| plan.machine == machine);
    let plan = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked machine contract"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked machine contracts"
        ))]);
    }
    Ok(plan)
}

pub(crate) fn task_specialization_commitment<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    check_facts: &checked_trees::CheckFacts,
    selection: &TaskStartSelection,
    target_machine: &checked_trees::machine::Machine,
    target_entry: &checked_trees::state::State,
    target_contract: &checked_trees::MachineContractPlan,
    requirement_identity: &str,
) -> Result<TaskSpecializationCommitment, Vec<Diagnostic>> {
    let requirement_owner = typed
        .trait_definition_by_symbol(selection.requirement_owner)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement owner",
            )]
        })?;
    let requirement = typed
        .trait_machine_signature_by_symbol(requirement_owner, selection.requirement)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement",
            )]
        })?;
    let normalized_requirement =
        typed.trait_requirement_callable_identity_text(requirement_owner, requirement);
    let application = typed
        .closed_requirement_application(selection.requirement)
        .filter(|application| application.declaring_trait == requirement_owner.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization lost its exact closed requirement application",
            )]
        })?;
    let authored_requirement = typed
        .trait_machine_signature_by_symbol(requirement_owner, application.template)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization lost its authored runtime requirement",
            )]
        })?;
    if typed.trait_requirement_callable_identity_text(requirement_owner, authored_requirement)
        != requirement_identity
    {
        return Err(vec![Diagnostic::error(
            "task specialization selected-runtime requirement differs from its exact authored requirement",
        )]);
    }

    let mut strong = ExactSpecializationEncoder::new();
    strong.string(normalized_requirement.as_str());
    strong.optional_digest(
        typed
            .symbols
            .symbol_package_identity(requirement_owner.symbol)
            .map(|identity| identity.digest()),
    );
    let mut requirement_binders = vec![(requirement_owner.symbol, "$Self".to_owned())];
    requirement_binders.extend(
        typed
            .trait_type_parameters(requirement_owner)
            .iter()
            .chain(typed.state_signature_type_parameters(requirement))
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
    );
    let requirement_parameters = typed.state_signature_parameters(requirement);
    strong.length(requirement_parameters.len());
    for parameter in requirement_parameters {
        strong.byte(u8::from(parameter.is_mutable));
        strong.byte(u8::from(parameter.is_const));
        strong.string(&typed.type_identity_request_text(TypeIdentityRequest {
            binders: &requirement_binders,
            ..TypeIdentityRequest::package_qualified(parameter.type_reference)
        }));
    }
    strong.string(&typed.type_identity_request_text(TypeIdentityRequest {
        binders: &requirement_binders,
        ..TypeIdentityRequest::package_qualified(requirement.return_type)
    }));
    strong.byte(match selection.operation {
        TaskStartOperation::Start => 1,
        TaskStartOperation::TryStart => 2,
    });
    strong.machine(typed, target_machine)?;
    strong.state(typed, target_machine, target_entry);
    strong.digest(target_contract.commitment.as_bytes());
    strong.target_machine_specialization(typed, check_facts, target_machine.symbol)?;
    Ok(strong.finish())
}

struct ExactSpecializationEncoder(crate::digest_encoder::DomainDigestEncoder);

impl ExactSpecializationEncoder {
    fn new() -> Self {
        Self(crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.task-specialization.sha256.v2\0",
        ))
    }

    fn byte(&mut self, value: u8) {
        self.0.byte(value);
    }

    fn length(&mut self, value: usize) {
        self.0.len(value);
    }

    fn string(&mut self, value: &str) {
        self.0.string(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.0.bytes(value);
    }

    fn strings(&mut self, values: &[String]) {
        self.0.strings(values);
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.raw(&value);
    }

    fn optional_digest(&mut self, value: Option<[u8; 32]>) {
        match value {
            Some(value) => {
                self.byte(1);
                self.digest(value);
            }
            None => self.byte(0),
        }
    }

    fn optional_string(&mut self, value: Option<&str>) {
        self.0.optional_string(value);
    }

    fn target_machine_specialization<'program>(
        &mut self,
        typed: &'program checked_trees::typed_trees::TypedTrees,
        check_facts: &checked_trees::CheckFacts,
        target_machine: symbols::SymbolHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut matches = typed
            .machine_specializations
            .iter()
            .filter(|specialization| specialization.instance == target_machine);
        let Some(specialization) = matches.next() else {
            self.byte(0);
            return Ok(());
        };
        if matches.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task activation target has duplicate exact checked specialization rows",
            )]);
        }
        self.byte(1);
        let template = exact_specialization_machine(typed, specialization.template, "template")?;
        self.machine(typed, template)?;
        self.digest(
            exact_task_machine_contract(check_facts, template.symbol, template.name.as_str())?
                .commitment
                .as_bytes(),
        );
        self.strings(&specialization.type_argument_identities);
        self.strings(&specialization.const_argument_identities);
        self.optional_string(specialization.accepted_template_commitment.as_deref());

        self.length(specialization.machine_arguments.len());
        for argument in &specialization.machine_arguments {
            let (owner, state) = unique_task_activation_target(
                typed,
                *argument,
            )
            .map_err(|error| {
                    vec![Diagnostic::error(format!(
                    "task specialization static machine argument has no exact semantic target: {}",
                    error.message()
                ))]
                })?;
            self.machine(typed, owner)?;
            self.state(typed, owner, state);
            self.digest(
                exact_task_machine_contract(check_facts, owner.symbol, owner.name.as_str())?
                    .commitment
                    .as_bytes(),
            );
        }

        if specialization.conformance_applications.len()
            != specialization.conformance_arguments.len()
        {
            return Err(vec![Diagnostic::error(
                "task specialization does not retain one exact closed application per selected conformance",
            )]);
        }
        self.length(specialization.conformance_arguments.len());
        self.length(specialization.conformance_applications.len());
        for application in &specialization.conformance_applications {
            if application.commitment.is_zero() {
                return Err(vec![Diagnostic::error(
                    "task specialization retained an empty closed-conformance commitment",
                )]);
            }
            self.digest(application.commitment.as_bytes());
        }
        let operator_realizations =
            checked_trees::typed_trees::operator::canonical_closed_operator_realization_bytes(
                &typed,
                specialization.instance,
                &specialization.operator_realizations,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "task specialization has invalid retained operator realizations: {error}"
                ))]
            })?;
        self.bytes(&operator_realizations);
        Ok(())
    }

    fn machine<'program>(
        &mut self,
        typed: &'program checked_trees::typed_trees::TypedTrees,
        machine: &checked_trees::machine::Machine,
    ) -> Result<(), Vec<Diagnostic>> {
        let identity = typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task specialization machine `{}` has no exact normalized overload identity",
                    machine.name.as_str()
                ))]
            })?;
        self.string(identity.as_str());
        self.optional_digest(
            typed
                .symbols
                .symbol_package_identity(machine.symbol)
                .map(|identity| identity.digest()),
        );
        Ok(())
    }

    fn state<'program>(
        &mut self,
        typed: &'program checked_trees::typed_trees::TypedTrees,
        owner: &checked_trees::machine::Machine,
        state: &checked_trees::state::State,
    ) {
        self.string(owner.name.as_str());
        self.string(state.name.as_str());
        let parameters = typed.state_parameters(state);
        self.length(parameters.len());
        for parameter in parameters {
            self.byte(u8::from(parameter.is_mutable));
            self.byte(u8::from(parameter.is_const));
            self.string(&typed.package_qualified_type_identity_text(parameter.type_reference));
        }
        self.string(&typed.package_qualified_type_identity_text(state.return_type));
    }

    fn finish(self) -> TaskSpecializationCommitment {
        TaskSpecializationCommitment::from_digest(self.0.finish().into())
    }
}

fn exact_specialization_machine<'program>(
    typed: &'program checked_trees::typed_trees::TypedTrees,
    symbol: symbols::SymbolHandle,
    role: &str,
) -> Result<&'program checked_trees::machine::Machine, Vec<Diagnostic>> {
    let mut matches = typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task specialization is missing its exact checked {role} machine"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task specialization has duplicate exact checked {role} machines"
        ))]);
    }
    Ok(machine)
}

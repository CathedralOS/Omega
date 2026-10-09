//! Exact task machine contracts and specialization commitments.

use crate::task_plans::start_selections::{TaskStartSelection, unique_task_activation_target};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use task_plans::{TaskSpecializationCommitment, TaskStartOperation};
use typed_trees::type_identity::TypeIdentityRequest;

pub(crate) fn exact_task_machine_contract<'program>(
    program: &'program CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<&'program checked_trees::MachineContractPlan, Vec<Diagnostic>> {
    let mut matches = program
        .facts
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

pub(crate) fn task_specialization_commitment(
    program: &CheckedTrees,
    selection: &TaskStartSelection,
    target_machine: &checked_trees::machine::Machine,
    target_entry: &checked_trees::state::State,
    target_contract: &checked_trees::MachineContractPlan,
    requirement_identity: &str,
) -> Result<TaskSpecializationCommitment, Vec<Diagnostic>> {
    let requirement_owner = program
        .trait_definition_by_symbol(selection.requirement_owner)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement owner",
            )]
        })?;
    let requirement = program
        .trait_machine_signature_by_symbol(requirement_owner, selection.requirement)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement",
            )]
        })?;
    let normalized_requirement =
        program.trait_requirement_callable_identity_text(requirement_owner, requirement);
    if normalized_requirement != requirement_identity {
        return Err(vec![Diagnostic::error(
            "task specialization selected-runtime requirement differs from its exact checked requirement",
        )]);
    }

    let mut strong = ExactSpecializationEncoder::new();
    strong.string(normalized_requirement.as_str());
    strong.optional_digest(
        program
            .symbols
            .symbol_package_identity(requirement_owner.symbol)
            .map(|identity| identity.digest()),
    );
    let mut requirement_binders = vec![(requirement_owner.symbol, "$Self".to_owned())];
    requirement_binders.extend(
        program
            .trait_type_parameters(requirement_owner)
            .iter()
            .chain(program.state_signature_type_parameters(requirement))
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
    );
    let requirement_parameters = program.state_signature_parameters(requirement);
    strong.length(requirement_parameters.len());
    for parameter in requirement_parameters {
        strong.byte(u8::from(parameter.is_mutable));
        strong.byte(u8::from(parameter.is_const));
        strong.string(
            program
                .type_identity(TypeIdentityRequest {
                    binders: &requirement_binders,
                    ..TypeIdentityRequest::package_qualified(parameter.type_reference)
                })
                .as_str(),
        );
    }
    strong.string(
        program
            .type_identity(TypeIdentityRequest {
                binders: &requirement_binders,
                ..TypeIdentityRequest::package_qualified(requirement.return_type)
            })
            .as_str(),
    );
    strong.byte(match selection.operation {
        TaskStartOperation::Start => 1,
        TaskStartOperation::TryStart => 2,
    });
    strong.machine(program, target_machine)?;
    strong.state(program, target_machine, target_entry);
    strong.digest(target_contract.commitment.as_bytes());
    strong.target_machine_specialization(program, target_machine.symbol)?;
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

    fn target_machine_specialization(
        &mut self,
        program: &CheckedTrees,
        target_machine: symbols::SymbolHandle,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut matches = program
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
        let template = exact_specialization_machine(program, specialization.template, "template")?;
        self.machine(program, template)?;
        self.digest(
            exact_task_machine_contract(program, template.symbol, template.name.as_str())?
                .commitment
                .as_bytes(),
        );
        self.strings(&specialization.type_argument_identities);
        self.strings(&specialization.const_argument_identities);
        self.optional_string(specialization.accepted_template_commitment.as_deref());

        self.length(specialization.machine_arguments.len());
        for argument in &specialization.machine_arguments {
            let (owner, state) =
                unique_task_activation_target(program, *argument).map_err(|error| {
                    vec![Diagnostic::error(format!(
                    "task specialization static machine argument has no exact semantic target: {}",
                    error.message()
                ))]
                })?;
            self.machine(program, owner)?;
            self.state(program, owner, state);
            self.digest(
                exact_task_machine_contract(program, owner.symbol, owner.name.as_str())?
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
        let operator_realizations = validation::canonical_closed_operator_realization_bytes(
            &program.typed,
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

    fn machine(
        &mut self,
        program: &CheckedTrees,
        machine: &checked_trees::machine::Machine,
    ) -> Result<(), Vec<Diagnostic>> {
        let identity = program
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task specialization machine `{}` has no exact normalized overload identity",
                    machine.name.as_str()
                ))]
            })?;
        self.string(identity.as_str());
        self.optional_digest(
            program
                .symbols
                .symbol_package_identity(machine.symbol)
                .map(|identity| identity.digest()),
        );
        Ok(())
    }

    fn state(
        &mut self,
        program: &CheckedTrees,
        owner: &checked_trees::machine::Machine,
        state: &checked_trees::state::State,
    ) {
        self.string(owner.name.as_str());
        self.string(state.name.as_str());
        let parameters = program.state_parameters(state);
        self.length(parameters.len());
        for parameter in parameters {
            self.byte(u8::from(parameter.is_mutable));
            self.byte(u8::from(parameter.is_const));
            self.string(
                program
                    .package_qualified_type_identity(parameter.type_reference)
                    .as_str(),
            );
        }
        self.string(
            program
                .package_qualified_type_identity(state.return_type)
                .as_str(),
        );
    }

    fn finish(self) -> TaskSpecializationCommitment {
        TaskSpecializationCommitment::from_digest(self.0.finish().into())
    }
}

fn exact_specialization_machine<'program>(
    program: &'program CheckedTrees,
    symbol: symbols::SymbolHandle,
    role: &str,
) -> Result<&'program checked_trees::machine::Machine, Vec<Diagnostic>> {
    let mut matches = program
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

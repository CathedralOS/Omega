//! Exact task machine contracts and specialization commitments.

use crate::task_plans::start_selections::{TaskStartSelection, unique_task_activation_target};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use sha2::Digest;
use sha2::Sha256;
use task_plans::{TaskSpecializationCommitment, TaskStartOperation};

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
        .traits()
        .iter()
        .find(|definition| definition.symbol == selection.requirement_owner)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement owner",
            )]
        })?;
    let requirement = program
        .trait_machine_signatures(requirement_owner)
        .iter()
        .find(|signature| signature.symbol == selection.requirement)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task specialization is missing its exact TaskRuntime requirement",
            )]
        })?;
    let normalized_requirement =
        program.normalized_trait_requirement_overload_identity(requirement_owner, requirement);
    if normalized_requirement.identity() != requirement_identity {
        return Err(vec![Diagnostic::error(
            "task specialization selected-runtime requirement differs from its exact checked requirement",
        )]);
    }

    let mut strong = ExactSpecializationEncoder::new();
    strong.string(normalized_requirement.identity().as_str());
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
        strong.byte(u8::from(parameter.is_self));
        strong.byte(u8::from(parameter.is_mutable));
        strong.byte(u8::from(parameter.is_const));
        strong.string(
            program
                .package_qualified_type_identity_with_binders(
                    parameter.type_reference,
                    &requirement_binders,
                )
                .as_str(),
        );
    }
    strong.string(
        program
            .package_qualified_type_identity_with_binders(
                requirement.return_type,
                &requirement_binders,
            )
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

struct ExactSpecializationEncoder(Sha256);

impl ExactSpecializationEncoder {
    fn new() -> Self {
        let mut digest = Sha256::new();
        digest.update(b"omega.task-specialization.sha256.v2\0");
        Self(digest)
    }

    fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    fn length(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    fn string(&mut self, value: &str) {
        self.length(value.len());
        self.0.update(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.length(value.len());
        self.0.update(value);
    }

    fn strings(&mut self, values: &[String]) {
        self.length(values.len());
        for value in values {
            self.string(value);
        }
    }

    fn digest(&mut self, value: [u8; 32]) {
        self.0.update(value);
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
        match value {
            Some(value) => {
                self.byte(1);
                self.string(value);
            }
            None => self.byte(0),
        }
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

        let selected_conformance_count = specialization.conformance_arguments.len()
            + specialization.inferred_conformance_arguments.len();
        if specialization.conformance_applications.len() != selected_conformance_count {
            return Err(vec![Diagnostic::error(
                "task specialization does not retain one exact closed application per selected conformance",
            )]);
        }
        self.length(specialization.conformance_arguments.len());
        self.length(specialization.inferred_conformance_arguments.len());
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
            .normalized_machine_overload_identity(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task specialization machine `{}` has no exact normalized overload identity",
                    machine.name.as_str()
                ))]
            })?;
        self.string(identity.identity().as_str());
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
            self.byte(u8::from(parameter.is_self));
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
        TaskSpecializationCommitment::from_digest(self.0.finalize().into())
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

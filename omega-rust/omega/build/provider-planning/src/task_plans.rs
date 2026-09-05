use checked_trees::{CheckedTrees, SuspensionCrossingStorage};
use diagnostics::Diagnostic;
use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use layout::TypeLayout;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use target::{Architecture, NativeTarget, ObjectFormat};
use task_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CallingPlanId,
    CanonicalSuspensionCrossing, MachineContractId, MachineEntryId,
    SelectedTaskRuntimeProviderFact, StackPlan, StackRepresentationId, TaskActivationPlanFact,
    TaskActivationPlanSet, TaskRuntimeId, TaskSpecializationCommitment, TaskStartOperation,
    ValueLayoutId, validate_activation_plan,
};

/// Elaborate every concrete `TaskRuntime::{start,try_start}<M>` specialization
/// into a provider-independent activation demand. The source classifier is
/// deliberately nominal and closed: an unrelated method named `start` does
/// not become a task operation.
pub fn elaborate_task_activation_plans(
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    let selections = task_start_selections(program)?;
    if selections.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let layouts = layout::build_layout_plan(program, target, opaque_representation_selections)
        .map_err(|error| vec![error])?;
    let mut activations = Vec::new();

    for selection in selections {
        let selected_runtime =
            selected_task_runtime_provider(program, selected_provider_plans, &selection)?;
        let (target_machine, entry) = exact_task_activation_target(
            program,
            selection.target_machine,
            selection.target_entry,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "TaskRuntime start specialization has an invalid retained target coordinate: {}",
                error.message()
            ))]
        })?;
        if !program.machine_type_parameters(target_machine).is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "task activation target `{}` is still generic after specialization",
                target_machine.name
            ))]);
        }

        let contract = exact_task_machine_contract(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let specialization_commitment = task_specialization_commitment(
            program,
            &selection,
            target_machine,
            entry,
            contract,
            selected_runtime.requirement_identity.as_str(),
        )?;
        let suspension = exact_task_machine_suspension(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let blocking = exact_task_machine_blocking(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let machine_contract = normalized_id(
            contract.report_fingerprint,
            MachineContractId::from_normalized_identity,
        )?;
        let entry_report_fingerprint = entry_report_fingerprint(program, target_machine, entry);
        let entry_id = normalized_id(
            entry_report_fingerprint,
            MachineEntryId::from_normalized_identity,
        )?;
        let argument_layout_report_fingerprint = signature_layout_report_fingerprint(
            program,
            target,
            opaque_representation_selections,
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .map(|parameter| parameter.type_reference),
        )?;
        let argument_layout = normalized_id(
            argument_layout_report_fingerprint,
            ValueLayoutId::from_normalized_identity,
        )?;
        let outcome_layout_report_fingerprint = signature_layout_report_fingerprint(
            program,
            target,
            opaque_representation_selections,
            std::iter::once(entry.return_type),
        )?;
        let terminal_outcome_layout = normalized_id(
            outcome_layout_report_fingerprint,
            ValueLayoutId::from_normalized_identity,
        )?;
        let calling_plan_report_fingerprint = calling_plan_report_fingerprint(
            target,
            entry_report_fingerprint,
            argument_layout_report_fingerprint,
            outcome_layout_report_fingerprint,
        );
        let calling_plan = normalized_id(
            calling_plan_report_fingerprint,
            CallingPlanId::from_normalized_identity,
        )?;

        let may_suspend = suspension.checked_may_suspend;
        let may_block = blocking.checked_may_block;
        let crossings = activation_carry_crossings(program, target_machine.symbol)?;
        let canonical_suspension_crossings = crossings
            .subtree
            .iter()
            .map(|crossing| canonical_suspension_crossing(program, crossing))
            .collect::<Result<Vec<_>, _>>()?;
        let activation_wide_carry = exact_activation_wide_carry(
            program,
            target_machine.symbol,
            target_machine.name.as_str(),
        )?;
        let carry_obligations = carry_obligations(activation_wide_carry.effective);
        let (stack_bytes, stack_alignment) = fixed_stack_layout(
            program,
            target,
            opaque_representation_selections,
            &layouts,
            target_machine,
            &crossings.root,
        )?;
        let stack_representation = normalized_id(
            stack_representation_report_fingerprint(target),
            StackRepresentationId::from_normalized_identity,
        )?;

        let plan = validate_activation_plan(ActivationPlanCandidate {
            machine_contract,
            entry: entry_id,
            argument_layout,
            terminal_outcome_layout,
            calling_plan,
            stack_plan: StackPlan {
                bytes: stack_bytes,
                alignment: stack_alignment,
                representation: stack_representation,
            },
            may_suspend,
            may_block,
            // Missing or locally unsafe crossings remain visible so the plan
            // validator rejects them fail-closed.
            canonical_suspension_crossings,
            carry_obligations,
            // `Task<T>` always carries cancellation-request authority. A
            // selected provider must establish that operation later.
            cancellation_required: true,
        })
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;

        activations.push(TaskActivationPlanFact {
            start_requirement: selection.requirement,
            target_machine: target_machine.symbol,
            target_entry: entry.symbol,
            specialization_report_fingerprint: selection.report_fingerprint,
            specialization_commitment,
            operation: selection.operation,
            selected_runtime,
            plan,
        });
    }

    Ok(TaskActivationPlanSet { activations })
}

/// Derive and commit the complete compiler-owned task-activation sidecar.
/// The checked program and selected providers remain shared read-only inputs;
/// a failed derivation leaves the previously retained sidecar unchanged.
pub fn settle_task_activation_plans(
    retained: &mut Arc<TaskActivationPlanSet>,
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<(), Vec<Diagnostic>> {
    let task_activations = elaborate_task_activation_plans(
        program,
        selected_provider_plans,
        target,
        opaque_representation_selections,
    )?;
    *retained = Arc::new(task_activations);
    Ok(())
}

fn exact_task_machine_contract<'program>(
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

fn task_specialization_commitment(
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

fn exact_task_machine_suspension(
    program: &CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::SuspensionPlan, Vec<Diagnostic>> {
    let mut matches = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches.next().map(|fact| fact.plan).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked suspension plan"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked suspension plans"
        ))]);
    }
    Ok(plan)
}

fn exact_task_machine_blocking(
    program: &CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<language_semantics::BlockingPlan, Vec<Diagnostic>> {
    let mut matches = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches.next().map(|fact| fact.plan).ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no checked blocking plan"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact checked blocking plans"
        ))]);
    }
    Ok(plan)
}

fn selected_task_runtime_provider(
    program: &CheckedTrees,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    selection: &TaskStartSelection,
) -> Result<SelectedTaskRuntimeProviderFact, Vec<Diagnostic>> {
    let (requirement_owner, _, authored_requirement_identity) =
        exact_task_runtime_requirement(program, selection)?;
    let matches = selected_provider_plans
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema.trait_name == requirement_owner.name.as_str()
                && plan.schema.methods.iter().any(|method| {
                    method.requirement_owner == requirement_owner.name.as_str()
                        && method.requirement_identity == authored_requirement_identity
                })
        })
        .collect::<Vec<_>>();
    let [plan] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(match matches.len() {
            0 => "TaskRuntime boundary slot has no retained selected provider plan".to_owned(),
            count => format!(
                "TaskRuntime boundary slot matches {count} retained selected provider plans"
            ),
        })]);
    };
    let requirement_name = match selection.operation {
        TaskStartOperation::Start => "start",
        TaskStartOperation::TryStart => "try_start",
    };
    let requirements = plan
        .schema
        .methods
        .iter()
        .filter(|method| {
            method.requirement_owner == requirement_owner.name.as_str()
                && method.requirement_identity == authored_requirement_identity
        })
        .collect::<Vec<_>>();
    let [method] = requirements.as_slice() else {
        return Err(vec![Diagnostic::error(match requirements.len() {
            0 => format!(
                "selected TaskRuntime provider plan `{}` has no `{requirement_name}` requirement",
                plan.name
            ),
            count => format!(
                "selected TaskRuntime provider plan `{}` has {count} `{requirement_name}` requirements; operation binding must be exact",
                plan.name
            ),
        })]);
    };
    if method.requirement_identity != authored_requirement_identity {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` as `{}`, but the activation call requires `{authored_requirement_identity}`",
            plan.name, method.requirement_identity
        ))]);
    }
    let covering_rows = plan
        .rows
        .iter()
        .filter(|row| plan.schema.row_binds_method(row, method))
        .count();
    if covering_rows != 1 {
        return Err(vec![Diagnostic::error(format!(
            "selected TaskRuntime provider plan `{}` binds `{requirement_name}` {covering_rows} times; exactly one operation realization is required",
            plan.name
        ))]);
    }
    let runtime = TaskRuntimeId::from_normalized_identity(plan.report_fingerprint())
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    Ok(SelectedTaskRuntimeProviderFact {
        runtime,
        provider_plan_name: plan.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
    })
}

fn exact_task_runtime_requirement<'program>(
    program: &'program CheckedTrees,
    selection: &TaskStartSelection,
) -> Result<
    (
        &'program checked_trees::trait_definition::TraitDefinition,
        &'program checked_trees::signature::StateSignature,
        String,
    ),
    Vec<Diagnostic>,
> {
    let mut owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == selection.requirement_owner);
    let owner = owners.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "TaskRuntime activation requirement must name one exact retained trait owner",
        )]
    })?;
    if owners.next().is_some() {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement owner must resolve uniquely",
        )]);
    }
    if !owner.is_boundary {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement owner must be an exact boundary trait",
        )]);
    }
    let mut signatures = program
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.symbol == selection.requirement);
    let signature = signatures.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "TaskRuntime activation requirement must belong to its exact retained owner",
        )]
    })?;
    if signatures.next().is_some() {
        return Err(vec![Diagnostic::error(
            "TaskRuntime activation requirement must resolve uniquely within its exact owner",
        )]);
    }
    let identity = program
        .normalized_trait_requirement_overload_identity(owner, signature)
        .identity();
    Ok((owner, signature, identity))
}

struct ActivationCarryCrossings<'program> {
    root: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
    subtree: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
}

fn exact_activation_wide_carry<'program>(
    program: &'program CheckedTrees,
    machine: symbols::SymbolHandle,
    machine_name: &str,
) -> Result<&'program checked_trees::MachineActivationCarryFact, Vec<Diagnostic>> {
    let mut matches = program
        .facts
        .carry
        .activation_wide_carry
        .iter()
        .filter(|fact| fact.machine == machine);
    let fact = matches.next().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has no exact activation-wide CPU/thread carry envelope"
        ))]
    })?;
    if matches.next().is_some() {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has duplicate exact activation-wide CPU/thread carry envelopes"
        ))]);
    }
    if !fact.analysis_complete {
        return Err(vec![Diagnostic::error(format!(
            "task activation target `{machine_name}` has incomplete activation-wide CPU/thread carry analysis"
        ))]);
    }
    Ok(fact)
}

fn activation_carry_crossings(
    program: &CheckedTrees,
    root: symbols::SymbolHandle,
) -> Result<ActivationCarryCrossings<'_>, Vec<Diagnostic>> {
    let subtree_machines = exact_activation_carry_subtree(program, root)?;
    let subtree = program
        .facts
        .carry
        .suspension_crossings
        .iter()
        .filter(|crossing| subtree_machines.contains(&crossing.machine))
        .collect::<Vec<_>>();
    let mut coordinates = Vec::new();
    for crossing in &subtree {
        validate_activation_carry_crossing(program, crossing)?;
        let coordinate = (
            crossing.machine,
            crossing.state,
            crossing.statement_index,
            crossing.call_ordinal,
        );
        if coordinates.contains(&coordinate) {
            return Err(vec![Diagnostic::error(
                "task activation carry crossings must retain one row per exact call coordinate",
            )]);
        }
        coordinates.push(coordinate);
    }
    let root = subtree
        .iter()
        .copied()
        .filter(|crossing| crossing.machine == root)
        .collect();
    Ok(ActivationCarryCrossings { root, subtree })
}

fn exact_activation_carry_subtree(
    program: &CheckedTrees,
    root: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, Vec<Diagnostic>> {
    let mut machines = vec![root];
    let mut cursor = 0;
    while cursor < machines.len() {
        let machine_symbol = machines[cursor];
        cursor += 1;

        let mut typed_machines = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == machine_symbol);
        typed_machines.next().ok_or_else(|| {
            vec![Diagnostic::error(
                "task activation carry topology must name an exact typed machine",
            )]
        })?;
        if typed_machines.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task activation carry topology machine must resolve uniquely",
            )]);
        }

        let mut topologies = program
            .facts
            .carry
            .machine_topologies
            .iter()
            .filter(|(_, topology)| topology.machine == machine_symbol);
        let topology = topologies
            .next()
            .map(|(_, topology)| topology)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task activation carry topology must retain one exact row per reached machine",
                )]
            })?;
        if topologies.next().is_some() {
            return Err(vec![Diagnostic::error(
                "task activation carry topology must retain exactly one row per reached machine",
            )]);
        }
        let fields = program
            .facts
            .carry
            .contained_fields
            .span(topology.fields)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "task activation carry topology must retain an exact valid field span",
                )]
            })?;
        let mut field_symbols = Vec::new();
        for field in fields {
            if !field.field.is_valid() || !field.data.is_valid() || !field.type_reference.is_valid()
            {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology fields must retain nonempty exact coordinates",
                )]);
            }
            if field_symbols.contains(&field.field) {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology fields must be unique within their machine",
                )]);
            }
            field_symbols.push(field.field);
            let targets = program
                .facts
                .carry
                .contained_targets
                .span(field.targets)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task activation carry topology field must retain an exact valid target span",
                    )]
                })?;
            if targets.is_empty() {
                return Err(vec![Diagnostic::error(
                    "task activation carry topology field must retain at least one exact target",
                )]);
            }
            let mut field_targets = Vec::new();
            for target in targets {
                if field_targets.contains(&target.machine) {
                    return Err(vec![Diagnostic::error(
                        "task activation carry topology field targets must be unique",
                    )]);
                }
                field_targets.push(target.machine);
                let mut typed_targets = program
                    .machines()
                    .iter()
                    .filter(|machine| machine.symbol == target.machine);
                typed_targets.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task activation carry topology target must name an exact typed machine",
                    )]
                })?;
                if typed_targets.next().is_some() {
                    return Err(vec![Diagnostic::error(
                        "task activation carry topology target must resolve uniquely",
                    )]);
                }
                if !machines.contains(&target.machine) {
                    machines.push(target.machine);
                }
            }
        }
    }
    Ok(machines)
}

fn validate_activation_carry_crossing(
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<(), Vec<Diagnostic>> {
    let mut machines = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == crossing.machine);
    let machine = machines.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name an exact typed machine",
        )]
    })?;
    if machines.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing machine must resolve uniquely",
        )]);
    }
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == crossing.state);
    let state = states.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing state must belong to its exact typed machine",
        )]
    })?;
    if states.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing state must resolve uniquely within its machine",
        )]);
    }
    if program
        .statement_table
        .statements(state.statement_nodes)
        .get(crossing.statement_index)
        .is_none()
    {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing statement must belong to its exact typed state",
        )]);
    }

    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| {
            flow.machine_symbol == crossing.machine && flow.state_symbol == crossing.state
        });
    let flow_state = flow_states.next().map(|(_, flow)| flow).ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name one exact checked flow state",
        )]
    })?;
    if flow_states.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must name exactly one checked flow state",
        )]);
    }
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task activation carry crossing flow state must retain an exact valid call span",
            )]
        })?;
    let mut calls = calls.iter().filter(|call| {
        call.statement_index == crossing.statement_index
            && call.call_ordinal == crossing.call_ordinal
    });
    let call = calls.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing must name one exact checked flow call",
        )]
    })?;
    if calls.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must name exactly one checked flow call",
        )]);
    }
    if call.target_symbol != crossing.target {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must retain its exact checked call target",
        )]);
    }
    let mut targets = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == crossing.target)
    });
    targets.next().ok_or_else(|| {
        vec![Diagnostic::error(
            "task activation carry crossing target must name an exact typed state",
        )]
    })?;
    if targets.next().is_some() {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing target must resolve to exactly one typed state",
        )]);
    }
    if !call.suspension.direct_may_suspend && !call.suspension.transitive_may_suspend {
        return Err(vec![Diagnostic::error(
            "task activation carry crossing must retain a may-suspend checked call",
        )]);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskStartSelection {
    requirement_owner: symbols::SymbolHandle,
    requirement: symbols::SymbolHandle,
    target_machine: symbols::SymbolHandle,
    target_entry: symbols::SymbolHandle,
    report_fingerprint: u64,
    operation: TaskStartOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskActivationTargetError {
    MissingEntry,
    AmbiguousEntry,
    AmbiguousMachine,
    MachineMismatch,
}

impl TaskActivationTargetError {
    fn message(self) -> &'static str {
        match self {
            Self::MissingEntry => "target entry must name one exact typed state",
            Self::AmbiguousEntry => "target entry must resolve to exactly one typed state",
            Self::AmbiguousMachine => "target owner must resolve to exactly one typed machine",
            Self::MachineMismatch => "target entry must belong to its retained exact machine",
        }
    }
}

fn unique_task_activation_target(
    program: &CheckedTrees,
    target_entry: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    TaskActivationTargetError,
> {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(move |state| state.symbol == target_entry)
            .map(move |state| (machine, state))
    });
    let (machine, entry) = matches
        .next()
        .ok_or(TaskActivationTargetError::MissingEntry)?;
    if matches.next().is_some() {
        return Err(TaskActivationTargetError::AmbiguousEntry);
    }
    let mut owners = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine.symbol);
    owners
        .next()
        .ok_or(TaskActivationTargetError::MissingEntry)?;
    if owners.next().is_some() {
        return Err(TaskActivationTargetError::AmbiguousMachine);
    }
    Ok((machine, entry))
}

fn exact_task_activation_target(
    program: &CheckedTrees,
    target_machine: symbols::SymbolHandle,
    target_entry: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    TaskActivationTargetError,
> {
    let (machine, entry) = unique_task_activation_target(program, target_entry)?;
    if machine.symbol != target_machine {
        return Err(TaskActivationTargetError::MachineMismatch);
    }
    Ok((machine, entry))
}

fn task_start_selections(
    program: &CheckedTrees,
) -> Result<Vec<TaskStartSelection>, Vec<Diagnostic>> {
    let mut selections = Vec::new();
    let mut diagnostics = Vec::new();
    for (_, expression) in program.expression_table.iter_expressions() {
        let checked_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        append_task_start_selection(
            program,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
            &mut selections,
            &mut diagnostics,
        );
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let checked_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                append_task_start_selection(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    &call.machine_arguments,
                    &mut selections,
                    &mut diagnostics,
                );
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    selections.sort_by_key(|selection| selection.report_fingerprint);
    selections.dedup();
    Ok(selections)
}

fn append_task_start_selection(
    program: &CheckedTrees,
    requirement: symbols::SymbolHandle,
    target_name: &str,
    machine_arguments: &[checked_trees::expression::StaticMachineArgument],
    selections: &mut Vec<TaskStartSelection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((definition, signature, operation)) = program.traits().iter().find_map(|definition| {
        if !definition.is_boundary
            || definition.name.as_str().rsplit("::").next() != Some("TaskRuntime")
        {
            return None;
        }
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == requirement)
            .and_then(|signature| {
                let result = program.display_type_reference(signature.return_type);
                match (signature.name.as_str(), result.as_str()) {
                    ("start", result) if result.starts_with("Task<") => {
                        Some((definition, signature, TaskStartOperation::Start))
                    }
                    ("try_start", result) if result.starts_with("StartOutcome<") => {
                        Some((definition, signature, TaskStartOperation::TryStart))
                    }
                    _ => None,
                }
            })
    }) else {
        return;
    };
    let [target] = machine_arguments else {
        diagnostics.push(Diagnostic::error(format!(
            "TaskRuntime requirement `{target_name}` must select exactly one static target machine, got {}",
            machine_arguments.len()
        )));
        return;
    };
    let (target_machine, target_entry) = match unique_task_activation_target(program, target.symbol)
    {
        Ok(target) => target,
        Err(TaskActivationTargetError::MissingEntry)
            if target.symbol.is_valid()
                && program
                    .machines()
                    .iter()
                    .flat_map(|machine| program.machine_type_parameters(machine))
                    .any(|parameter| parameter.symbol == target.symbol) =>
        {
            // A generic wrapper may forward its own machine parameter. Its
            // cloned concrete specialization contributes the eventual row.
            return;
        }
        Err(TaskActivationTargetError::MissingEntry) => {
            diagnostics.push(Diagnostic::error(format!(
                "TaskRuntime requirement `{target_name}` selects an unresolved static target `{}`",
                target
                    .path
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::")
            )));
            return;
        }
        Err(error) => {
            diagnostics.push(Diagnostic::error(format!(
                "TaskRuntime requirement `{target_name}` selects an invalid static target coordinate: {}",
                error.message()
            )));
            return;
        }
    };
    let mut hash = StableHash::new();
    hash.string("task-runtime-requirement-specialization-v1");
    hash.string(
        program
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity()
            .as_str(),
    );
    hash.u64(entry_report_fingerprint(
        program,
        target_machine,
        target_entry,
    ));
    let selection = TaskStartSelection {
        requirement_owner: definition.symbol,
        requirement,
        target_machine: target_machine.symbol,
        target_entry: target_entry.symbol,
        report_fingerprint: hash.finish(),
        operation,
    };
    if !selections.contains(&selection) {
        selections.push(selection);
    }
}

/// Current fixed-stack layout bridge: persistent machine storage plus the
/// largest canonical park frontier and entry resume overhead. Whole-call-graph
/// WCSU composition will replace this local sizing pass before provider stack
/// reservation is enabled.
fn fixed_stack_layout(
    program: &CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    layouts: &layout::LayoutPlan,
    machine: &checked_trees::machine::Machine,
    crossings: &[&checked_trees::SuspensionCrossingCarryFact],
) -> Result<(u64, u64), Vec<Diagnostic>> {
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find_map(|(_, layout)| (layout.symbol == machine.symbol).then_some(layout.layout))
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "task activation target `{}` has no concrete machine layout",
                machine.name
            ))]
        })?;
    // Every activation needs an explicit resume-state word even when the
    // selected machine has no stored fields and never parks.
    let state_word = TypeLayout {
        size: target.pointer_size,
        alignment: target.pointer_alignment,
    };
    let mut base_size = 0usize;
    let mut base_alignment = 1usize;
    append_layout(&mut base_size, &mut base_alignment, state_word)?;
    append_layout(&mut base_size, &mut base_alignment, machine_layout)?;
    base_size = align_to(base_size, base_alignment)?;

    let mut maximum_size = base_size;
    let mut maximum_alignment = base_alignment;
    for crossing in crossings {
        let mut size = base_size;
        let mut alignment = base_alignment;
        for live in &crossing.live_values {
            if live.storage == SuspensionCrossingStorage::Persistent {
                continue;
            }
            let layout = layout::layout_type_reference(
                program,
                target,
                opaque_representation_selections,
                live.type_reference,
            )
            .map_err(|error| vec![error])?;
            append_layout(&mut size, &mut alignment, layout)?;
        }
        size = align_to(size, alignment)?;
        maximum_size = maximum_size.max(size);
        maximum_alignment = maximum_alignment.max(alignment);
    }
    maximum_size = align_to(maximum_size, maximum_alignment)?;
    Ok((
        u64::try_from(maximum_size)
            .map_err(|_| vec![Diagnostic::error("task continuation size exceeds u64")])?,
        u64::try_from(maximum_alignment)
            .map_err(|_| vec![Diagnostic::error("task continuation alignment exceeds u64")])?,
    ))
}

fn append_layout(
    size: &mut usize,
    alignment: &mut usize,
    layout: TypeLayout,
) -> Result<(), Vec<Diagnostic>> {
    let field_alignment = layout.alignment.max(1);
    *size = align_to(*size, field_alignment)?;
    *size = size
        .checked_add(layout.size)
        .ok_or_else(|| vec![Diagnostic::error("task continuation layout size overflow")])?;
    *alignment = (*alignment).max(field_alignment);
    Ok(())
}

fn align_to(value: usize, alignment: usize) -> Result<usize, Vec<Diagnostic>> {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|rounded| rounded / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task continuation layout alignment overflow",
            )]
        })
}

fn carry_obligations(policy: CarryPolicy) -> ActivationCarryObligations {
    ActivationCarryObligations {
        preserve_cpu: policy.cpu == CarryCpu::Origin,
        preserve_host_thread: policy.host_thread == CarryHostThread::Origin,
    }
}

fn canonical_suspension_crossing(
    program: &CheckedTrees,
    crossing: &checked_trees::SuspensionCrossingCarryFact,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    Ok(CanonicalSuspensionCrossing {
        identity: checked_trees::canonical_suspension_crossing_id(program, crossing).ok_or_else(
            || {
                vec![Diagnostic::error(
                    "task activation carry crossing source identity must resolve exactly",
                )]
            },
        )?,
        suspension_allowed: crossing.effective.suspension == CarrySuspension::Allowed,
        preserve_cpu: crossing.effective.cpu == CarryCpu::Origin,
        preserve_host_thread: crossing.effective.host_thread == CarryHostThread::Origin,
    })
}

fn stack_representation_report_fingerprint(target: NativeTarget) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x53);
    hash.string("fixed-nonmoving-stack-v1");
    hash.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    hash.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    hash.usize(target.pointer_size);
    hash.usize(target.pointer_alignment);
    hash.finish()
}

fn signature_layout_report_fingerprint(
    program: &CheckedTrees,
    target: NativeTarget,
    opaque_representation_selections: &[representation_planning::OpaqueRepresentationSelection],
    types: impl IntoIterator<Item = checked_trees::types::TypeReferenceHandle>,
) -> Result<u64, Vec<Diagnostic>> {
    let mut hash = StableHash::new();
    hash.byte(0x51);
    for type_reference in types {
        let layout = layout::layout_type_reference(
            program,
            target,
            opaque_representation_selections,
            type_reference,
        )
        .map_err(|error| vec![error])?;
        hash.string(program.normalized_type_identity(type_reference).as_str());
        hash.usize(layout.size);
        hash.usize(layout.alignment);
        hash.byte(0xff);
    }
    Ok(hash.finish())
}

fn entry_report_fingerprint(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    entry: &checked_trees::state::State,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(0x45);
    hash.string(machine.name.as_str());
    hash.string(entry.name.as_str());
    for parameter in program.state_parameters(entry) {
        hash.byte(u8::from(parameter.is_self));
        hash.byte(u8::from(parameter.is_mutable));
        hash.string(
            program
                .normalized_type_identity(parameter.type_reference)
                .as_str(),
        );
    }
    hash.byte(0xfe);
    hash.string(program.normalized_type_identity(entry.return_type).as_str());
    hash.finish()
}

fn calling_plan_report_fingerprint(
    target: NativeTarget,
    entry_report_fingerprint: u64,
    arguments_report_fingerprint: u64,
    outcome_report_fingerprint: u64,
) -> u64 {
    let mut hash = StableHash::new();
    hash.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    hash.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    hash.usize(target.pointer_size);
    hash.usize(target.pointer_alignment);
    hash.u64(entry_report_fingerprint);
    hash.u64(arguments_report_fingerprint);
    hash.u64(outcome_report_fingerprint);
    hash.finish()
}

fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, task_plans::TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity.max(1)).map_err(|error| vec![Diagnostic::error(error.to_string())])
}

struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
        self.byte(0);
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preservation_mapping_keeps_only_cpu_and_thread_obligations() {
        assert_eq!(
            carry_obligations(CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Origin,
                address: language_semantics::CarryAddress::Stable,
            }),
            ActivationCarryObligations {
                preserve_cpu: true,
                preserve_host_thread: true,
            }
        );
    }

    fn activation_operational_fixture()
    -> (CheckedTrees, symbols::SymbolHandle, symbols::SymbolHandle) {
        let target = symbols::SymbolHandle::from_arena_index(1);
        let unrelated = symbols::SymbolHandle::from_arena_index(2);
        let mut program = CheckedTrees::default();
        program.facts.contract_plans.machines = vec![
            checked_trees::MachineContractPlan {
                machine: target,
                closed_scalar_values: Default::default(),
                crash: Default::default(),
                report_fingerprint: 0x1111,
                commitment: checked_trees::MachineContractCommitment::from_digest([1; 32]),
            },
            checked_trees::MachineContractPlan {
                machine: unrelated,
                closed_scalar_values: Default::default(),
                crash: Default::default(),
                report_fingerprint: 0x2222,
                commitment: checked_trees::MachineContractCommitment::from_digest([2; 32]),
            },
        ];
        program.facts.suspensions.machines = vec![
            checked_trees::MachineSuspensionFact {
                machine: target,
                plan: language_semantics::SuspensionPlan {
                    interface: language_semantics::SuspensionInterface::InternalInferred,
                    checked_may_suspend: true,
                },
            },
            checked_trees::MachineSuspensionFact {
                machine: unrelated,
                plan: language_semantics::SuspensionPlan {
                    interface: language_semantics::SuspensionInterface::PublishedMaySuspend(false),
                    checked_may_suspend: false,
                },
            },
        ];
        program.facts.blocking.machines = vec![
            checked_trees::MachineBlockingFact {
                machine: target,
                plan: language_semantics::BlockingPlan {
                    interface: language_semantics::BlockingInterface::PublishedMayBlock(false),
                    checked_may_block: false,
                },
            },
            checked_trees::MachineBlockingFact {
                machine: unrelated,
                plan: language_semantics::BlockingPlan {
                    interface: language_semantics::BlockingInterface::InternalInferred,
                    checked_may_block: true,
                },
            },
        ];
        (program, target, unrelated)
    }

    fn operational_error<T>(result: Result<T, Vec<Diagnostic>>) -> String {
        match result {
            Ok(_) => panic!("invalid exact operational rows must fail closed"),
            Err(diagnostics) => diagnostics
                .first()
                .expect("operational diagnostic")
                .message
                .clone(),
        }
    }

    #[test]
    fn activation_operational_rows_preserve_independent_exact_axes() {
        let (program, target, _) = activation_operational_fixture();

        assert_eq!(
            exact_task_machine_contract(&program, target, "Target")
                .expect("exact contract")
                .report_fingerprint,
            0x1111
        );
        assert!(
            exact_task_machine_suspension(&program, target, "Target")
                .expect("exact suspension")
                .checked_may_suspend
        );
        assert!(
            !exact_task_machine_blocking(&program, target, "Target")
                .expect("exact blocking")
                .checked_may_block
        );
    }

    #[test]
    fn activation_operational_rejects_missing_contract() {
        let (mut program, target, _) = activation_operational_fixture();
        program
            .facts
            .contract_plans
            .machines
            .retain(|plan| plan.machine != target);

        assert!(
            operational_error(exact_task_machine_contract(&program, target, "Target"))
                .contains("no checked machine contract")
        );
    }

    #[test]
    fn activation_operational_rejects_missing_suspension() {
        let (mut program, target, _) = activation_operational_fixture();
        program
            .facts
            .suspensions
            .machines
            .retain(|fact| fact.machine != target);

        assert!(
            operational_error(exact_task_machine_suspension(&program, target, "Target"))
                .contains("no checked suspension plan")
        );
    }

    #[test]
    fn activation_operational_rejects_missing_blocking() {
        let (mut program, target, _) = activation_operational_fixture();
        program
            .facts
            .blocking
            .machines
            .retain(|fact| fact.machine != target);

        assert!(
            operational_error(exact_task_machine_blocking(&program, target, "Target"))
                .contains("no checked blocking plan")
        );
    }

    #[test]
    fn activation_operational_rejects_duplicate_contract() {
        let (mut program, target, _) = activation_operational_fixture();
        let mut duplicate = program.facts.contract_plans.machines[0].clone();
        duplicate.report_fingerprint = 0x3333;
        program.facts.contract_plans.machines.push(duplicate);

        assert!(
            operational_error(exact_task_machine_contract(&program, target, "Target"))
                .contains("duplicate exact checked machine contracts")
        );
    }

    #[test]
    fn activation_operational_rejects_duplicate_suspension() {
        let (mut program, target, _) = activation_operational_fixture();
        let mut duplicate = program.facts.suspensions.machines[0];
        duplicate.plan.checked_may_suspend = false;
        program.facts.suspensions.machines.push(duplicate);

        assert!(
            operational_error(exact_task_machine_suspension(&program, target, "Target"))
                .contains("duplicate exact checked suspension plans")
        );
    }

    #[test]
    fn activation_operational_rejects_duplicate_blocking() {
        let (mut program, target, _) = activation_operational_fixture();
        let mut duplicate = program.facts.blocking.machines[0];
        duplicate.plan.checked_may_block = true;
        program.facts.blocking.machines.push(duplicate);

        assert!(
            operational_error(exact_task_machine_blocking(&program, target, "Target"))
                .contains("duplicate exact checked blocking plans")
        );
    }

    #[test]
    fn activation_operational_ignores_unrelated_duplicate_rows() {
        let (mut program, target, _) = activation_operational_fixture();
        let unrelated_contract = program.facts.contract_plans.machines[1].clone();
        let unrelated_suspension = program.facts.suspensions.machines[1];
        let unrelated_blocking = program.facts.blocking.machines[1];
        program
            .facts
            .contract_plans
            .machines
            .push(unrelated_contract);
        program
            .facts
            .suspensions
            .machines
            .push(unrelated_suspension);
        program.facts.blocking.machines.push(unrelated_blocking);

        assert!(exact_task_machine_contract(&program, target, "Target").is_ok());
        assert!(exact_task_machine_suspension(&program, target, "Target").is_ok());
        assert!(exact_task_machine_blocking(&program, target, "Target").is_ok());
    }

    struct ActivationRequirementFixture {
        program: CheckedTrees,
        selection: TaskStartSelection,
        other_owner: symbols::SymbolHandle,
        other_requirement: symbols::SymbolHandle,
        private_owner: symbols::SymbolHandle,
        private_requirement: symbols::SymbolHandle,
    }

    fn activation_requirement_fixture(duplicate_requirement: bool) -> ActivationRequirementFixture {
        let owner = symbols::SymbolHandle::from_arena_index(1);
        let requirement = symbols::SymbolHandle::from_arena_index(2);
        let other_owner = symbols::SymbolHandle::from_arena_index(3);
        let other_requirement = symbols::SymbolHandle::from_arena_index(4);
        let private_owner = symbols::SymbolHandle::from_arena_index(5);
        let private_requirement = symbols::SymbolHandle::from_arena_index(6);
        let mut program = CheckedTrees::default();

        let mut task_runtime = checked_trees::trait_definition::TraitDefinition {
            symbol: owner,
            is_boundary: true,
            name: checked_trees::name::Identifier::generated("core::TaskRuntime"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut task_runtime,
            checked_trees::signature::StateSignature {
                symbol: requirement,
                name: checked_trees::name::Identifier::generated("start"),
                ..Default::default()
            },
        );
        if duplicate_requirement {
            program.typed.push_trait_machine_signature(
                &mut task_runtime,
                checked_trees::signature::StateSignature {
                    symbol: requirement,
                    name: checked_trees::name::Identifier::generated("duplicate"),
                    ..Default::default()
                },
            );
        }
        program.typed.push_trait_definition(task_runtime);

        let mut other = checked_trees::trait_definition::TraitDefinition {
            symbol: other_owner,
            is_boundary: true,
            name: checked_trees::name::Identifier::generated("other::TaskRuntime"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut other,
            checked_trees::signature::StateSignature {
                symbol: other_requirement,
                name: checked_trees::name::Identifier::generated("start"),
                ..Default::default()
            },
        );
        program.typed.push_trait_definition(other);

        let mut private = checked_trees::trait_definition::TraitDefinition {
            symbol: private_owner,
            is_boundary: false,
            name: checked_trees::name::Identifier::generated("PrivateRuntime"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut private,
            checked_trees::signature::StateSignature {
                symbol: private_requirement,
                name: checked_trees::name::Identifier::generated("start"),
                ..Default::default()
            },
        );
        program.typed.push_trait_definition(private);

        ActivationRequirementFixture {
            program,
            selection: TaskStartSelection {
                requirement_owner: owner,
                requirement,
                target_machine: symbols::SymbolHandle::from_arena_index(7),
                target_entry: symbols::SymbolHandle::from_arena_index(8),
                report_fingerprint: 1,
                operation: TaskStartOperation::Start,
            },
            other_owner,
            other_requirement,
            private_owner,
            private_requirement,
        }
    }

    fn requirement_error(program: &CheckedTrees, selection: &TaskStartSelection) -> String {
        match exact_task_runtime_requirement(program, selection) {
            Ok(_) => panic!("invalid exact requirement custody must fail closed"),
            Err(diagnostics) => diagnostics
                .first()
                .expect("requirement diagnostic")
                .message
                .clone(),
        }
    }

    #[test]
    fn activation_requirement_retains_exact_boundary_owner_and_signature() {
        let fixture = activation_requirement_fixture(false);
        let (owner, signature, identity) =
            exact_task_runtime_requirement(&fixture.program, &fixture.selection)
                .expect("exact requirement");

        assert_eq!(owner.symbol, fixture.selection.requirement_owner);
        assert_eq!(signature.symbol, fixture.selection.requirement);
        assert!(identity.contains("core::TaskRuntime::start"));
    }

    #[test]
    fn activation_requirement_rejects_missing_owner() {
        let mut fixture = activation_requirement_fixture(false);
        fixture.selection.requirement_owner = symbols::SymbolHandle::invalid();

        assert!(requirement_error(&fixture.program, &fixture.selection).contains("one exact"));
    }

    #[test]
    fn activation_requirement_rejects_duplicate_owner() {
        let mut fixture = activation_requirement_fixture(false);
        fixture.program.typed.push_trait_definition(
            checked_trees::trait_definition::TraitDefinition {
                symbol: fixture.selection.requirement_owner,
                is_boundary: true,
                name: checked_trees::name::Identifier::generated("duplicate::TaskRuntime"),
                ..Default::default()
            },
        );

        assert!(requirement_error(&fixture.program, &fixture.selection).contains("uniquely"));
    }

    #[test]
    fn activation_requirement_rejects_non_boundary_owner() {
        let mut fixture = activation_requirement_fixture(false);
        fixture.selection.requirement_owner = fixture.private_owner;
        fixture.selection.requirement = fixture.private_requirement;

        assert!(requirement_error(&fixture.program, &fixture.selection).contains("boundary"));
    }

    #[test]
    fn activation_requirement_rejects_missing_owned_signature() {
        let mut fixture = activation_requirement_fixture(false);
        fixture.selection.requirement = symbols::SymbolHandle::invalid();

        assert!(
            requirement_error(&fixture.program, &fixture.selection)
                .contains("belong to its exact retained owner")
        );
    }

    #[test]
    fn activation_requirement_rejects_duplicate_owned_signature() {
        let fixture = activation_requirement_fixture(true);

        assert!(
            requirement_error(&fixture.program, &fixture.selection)
                .contains("resolve uniquely within its exact owner")
        );
    }

    #[test]
    fn activation_requirement_rejects_cross_owner_signature_drift() {
        let mut fixture = activation_requirement_fixture(false);
        fixture.selection.requirement_owner = fixture.other_owner;

        assert!(
            requirement_error(&fixture.program, &fixture.selection)
                .contains("belong to its exact retained owner")
        );
    }

    #[test]
    fn activation_requirement_ignores_unrelated_trait_and_signature() {
        let fixture = activation_requirement_fixture(false);
        assert_ne!(fixture.other_owner, fixture.selection.requirement_owner);
        assert_ne!(fixture.other_requirement, fixture.selection.requirement);

        let (owner, signature, _) =
            exact_task_runtime_requirement(&fixture.program, &fixture.selection)
                .expect("unrelated retained trait does not perturb exact owner");
        assert_eq!(owner.symbol, fixture.selection.requirement_owner);
        assert_eq!(signature.symbol, fixture.selection.requirement);
    }

    fn activation_target_fixture() -> (
        CheckedTrees,
        symbols::SymbolHandle,
        symbols::SymbolHandle,
        symbols::SymbolHandle,
        symbols::SymbolHandle,
    ) {
        let first_machine = symbols::SymbolHandle::from_arena_index(1);
        let first_entry = symbols::SymbolHandle::from_arena_index(2);
        let second_machine = symbols::SymbolHandle::from_arena_index(3);
        let second_entry = symbols::SymbolHandle::from_arena_index(4);
        let mut program = CheckedTrees::default();

        let mut first = checked_trees::machine::Machine {
            symbol: first_machine,
            name: checked_trees::name::Identifier::generated("First::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut first,
            checked_trees::state::State {
                symbol: first_entry,
                name: checked_trees::name::Identifier::generated("run"),
                ..Default::default()
            },
        );
        program.typed.push_machine(first);

        let mut second = checked_trees::machine::Machine {
            symbol: second_machine,
            name: checked_trees::name::Identifier::generated("Second::run"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut second,
            checked_trees::state::State {
                symbol: second_entry,
                name: checked_trees::name::Identifier::generated("run"),
                ..Default::default()
            },
        );
        program.typed.push_machine(second);

        (
            program,
            first_machine,
            first_entry,
            second_machine,
            second_entry,
        )
    }

    #[test]
    fn activation_target_retains_exact_machine_and_entry() {
        let (program, first_machine, first_entry, _, _) = activation_target_fixture();
        let (machine, entry) = exact_task_activation_target(&program, first_machine, first_entry)
            .expect("exact task target");

        assert_eq!(machine.symbol, first_machine);
        assert_eq!(entry.symbol, first_entry);
    }

    #[test]
    fn activation_target_rejects_missing_entry() {
        let (program, first_machine, _, _, _) = activation_target_fixture();
        assert_eq!(
            exact_task_activation_target(
                &program,
                first_machine,
                symbols::SymbolHandle::invalid(),
            )
            .expect_err("missing entry must fail closed"),
            TaskActivationTargetError::MissingEntry
        );
    }

    #[test]
    fn activation_target_rejects_duplicate_entry_within_owner() {
        let (_, first_machine, first_entry, _, _) = activation_target_fixture();
        let mut program = CheckedTrees::default();
        let mut machine = checked_trees::machine::Machine {
            symbol: first_machine,
            name: checked_trees::name::Identifier::generated("First::run"),
            ..Default::default()
        };
        for name in ["run", "duplicate"] {
            program.typed.push_machine_state(
                &mut machine,
                checked_trees::state::State {
                    symbol: first_entry,
                    name: checked_trees::name::Identifier::generated(name),
                    ..Default::default()
                },
            );
        }
        program.typed.push_machine(machine);

        assert_eq!(
            exact_task_activation_target(&program, first_machine, first_entry)
                .expect_err("duplicate entry must fail closed"),
            TaskActivationTargetError::AmbiguousEntry
        );
    }

    #[test]
    fn activation_target_rejects_duplicate_entry_across_owners() {
        let (mut program, first_machine, first_entry, second_machine, _) =
            activation_target_fixture();
        let second = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == second_machine)
            .expect("second machine")
            .clone();
        program.typed.machine_states_mut(&second)[0].symbol = first_entry;

        assert_eq!(
            exact_task_activation_target(&program, first_machine, first_entry)
                .expect_err("cross-owner duplicate entry must fail closed"),
            TaskActivationTargetError::AmbiguousEntry
        );
    }

    #[test]
    fn activation_target_rejects_duplicate_owner_machine_identity() {
        let (mut program, first_machine, first_entry, second_machine, _) =
            activation_target_fixture();
        program.typed.machines.for_each_mut(|_, machine| {
            if machine.symbol == second_machine {
                machine.symbol = first_machine;
            }
        });

        assert_eq!(
            exact_task_activation_target(&program, first_machine, first_entry)
                .expect_err("duplicate owner machine identity must fail closed"),
            TaskActivationTargetError::AmbiguousMachine
        );
    }

    #[test]
    fn activation_target_rejects_stored_machine_entry_drift() {
        let (program, first_machine, _, _, second_entry) = activation_target_fixture();
        assert_eq!(
            exact_task_activation_target(&program, first_machine, second_entry)
                .expect_err("stored machine/entry drift must fail closed"),
            TaskActivationTargetError::MachineMismatch
        );
    }

    fn activation_crossing_validation_fixture() -> (
        CheckedTrees,
        symbols::SymbolHandle,
        symbols::SymbolHandle,
        symbols::SymbolHandle,
    ) {
        let root = symbols::SymbolHandle::from_arena_index(1);
        let root_state = symbols::SymbolHandle::from_arena_index(2);
        let child = symbols::SymbolHandle::from_arena_index(3);
        let child_state = symbols::SymbolHandle::from_arena_index(4);
        let field = symbols::SymbolHandle::from_arena_index(5);
        let data = symbols::SymbolHandle::from_arena_index(6);
        let mut program = CheckedTrees::default();
        let contained_type = program.typed.type_reference_table.insert(
            checked_trees::types::TypeReferenceNode::Named {
                symbol: data,
                name: checked_trees::name::Identifier::generated("ChildData"),
            },
        );

        let mut root_machine = checked_trees::machine::Machine {
            symbol: root,
            name: checked_trees::name::Identifier::generated("Root::run"),
            ..Default::default()
        };
        let mut root_state_definition = checked_trees::state::State {
            symbol: root_state,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        };
        program.typed.statement_table.push_statement(
            &mut root_state_definition.statement_nodes,
            Default::default(),
        );
        program
            .typed
            .push_machine_state(&mut root_machine, root_state_definition);
        program.typed.push_machine(root_machine);

        let mut child_machine = checked_trees::machine::Machine {
            symbol: child,
            name: checked_trees::name::Identifier::generated("Child::run"),
            ..Default::default()
        };
        let mut child_state_definition = checked_trees::state::State {
            symbol: child_state,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        };
        program.typed.statement_table.push_statement(
            &mut child_state_definition.statement_nodes,
            Default::default(),
        );
        program
            .typed
            .push_machine_state(&mut child_machine, child_state_definition);
        program.typed.push_machine(child_machine);

        let mut calls = arena::HandleSpan::empty();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: root_state,
                suspension: language_semantics::SuspensionSummary {
                    direct_may_suspend: true,
                    transitive_may_suspend: false,
                },
                ..Default::default()
            },
        );
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 1,
                target_symbol: child_state,
                suspension: language_semantics::SuspensionSummary {
                    direct_may_suspend: false,
                    transitive_may_suspend: true,
                },
                ..Default::default()
            },
        );
        program
            .facts
            .flow
            .control
            .states
            .append(checked_trees::FlowStateFact {
                machine_symbol: child,
                state_symbol: child_state,
                calls,
                ..Default::default()
            });

        let targets = program
            .facts
            .carry
            .contained_targets
            .insert_many([checked_trees::ContainedMachineTargetFact { machine: child }]);
        let fields = program.facts.carry.contained_fields.insert_many([
            checked_trees::ContainedMachineFieldFact {
                field,
                data,
                type_reference: contained_type,
                targets,
            },
        ]);
        program
            .facts
            .carry
            .machine_topologies
            .insert(checked_trees::MachineCarryTopologyFact {
                machine: root,
                fields,
            });
        program
            .facts
            .carry
            .machine_topologies
            .insert(checked_trees::MachineCarryTopologyFact {
                machine: child,
                fields: arena::HandleSpan::empty(),
            });
        program
            .facts
            .carry
            .suspension_crossings
            .push(checked_trees::SuspensionCrossingCarryFact {
                machine: child,
                state: child_state,
                statement_index: 0,
                call_ordinal: 0,
                target: root_state,
                receiver: None,
                effective: CarryPolicy {
                    suspension: CarrySuspension::Allowed,
                    cpu: CarryCpu::Origin,
                    host_thread: CarryHostThread::Any,
                    address: language_semantics::CarryAddress::Movable,
                },
                live_values: Vec::new(),
            });
        program
            .facts
            .carry
            .suspension_crossings
            .push(checked_trees::SuspensionCrossingCarryFact {
                machine: child,
                state: child_state,
                statement_index: 0,
                call_ordinal: 1,
                target: child_state,
                receiver: None,
                effective: CarryPolicy::PERMISSIVE,
                live_values: Vec::new(),
            });
        (program, root, root_state, child_state)
    }

    fn crossing_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
        match activation_carry_crossings(program, root) {
            Ok(_) => panic!("invalid crossing must fail closed"),
            Err(diagnostics) => diagnostics
                .first()
                .expect("crossing diagnostic")
                .message
                .clone(),
        }
    }

    #[test]
    fn activation_crossings_include_contained_machine_crossings() {
        let (program, root, _, _) = activation_crossing_validation_fixture();
        let child_policy = program.facts.carry.suspension_crossings[0].effective;
        let crossings = activation_carry_crossings(&program, root).expect("exact crossing custody");

        assert!(crossings.root.is_empty());
        assert_eq!(crossings.subtree.len(), 2);
        assert_eq!(crossings.subtree[0].call_ordinal, 0);
        assert_eq!(crossings.subtree[1].call_ordinal, 1);
        assert!(
            crossings
                .subtree
                .iter()
                .all(|crossing| crossing.effective.suspension == CarrySuspension::Allowed)
        );
        assert_eq!(crossings.subtree[0].effective, child_policy);
    }

    fn topology_error(program: &CheckedTrees, root: symbols::SymbolHandle) -> String {
        match exact_activation_carry_subtree(program, root) {
            Ok(_) => panic!("invalid topology must fail closed"),
            Err(diagnostics) => diagnostics
                .first()
                .expect("topology diagnostic")
                .message
                .clone(),
        }
    }

    #[test]
    fn activation_topology_preserves_target_order_and_allows_cycles() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let child = program.facts.carry.suspension_crossings[0].machine;
        let sibling = symbols::SymbolHandle::from_arena_index(20);
        program.typed.push_machine(checked_trees::machine::Machine {
            symbol: sibling,
            name: checked_trees::name::Identifier::generated("Sibling::run"),
            ..Default::default()
        });
        program
            .facts
            .carry
            .machine_topologies
            .insert(checked_trees::MachineCarryTopologyFact {
                machine: sibling,
                fields: arena::HandleSpan::empty(),
            });
        let existing_target = *program
            .facts
            .carry
            .contained_targets
            .iter()
            .next()
            .expect("child target")
            .1;
        let ordered_targets = program.facts.carry.contained_targets.insert_many([
            existing_target,
            checked_trees::ContainedMachineTargetFact { machine: sibling },
        ]);
        program
            .facts
            .carry
            .contained_fields
            .for_each_mut(|_, field| field.targets = ordered_targets);

        let type_reference = program
            .facts
            .carry
            .contained_fields
            .iter()
            .next()
            .expect("contained field")
            .1
            .type_reference;
        let cycle_targets = program
            .facts
            .carry
            .contained_targets
            .insert_many([checked_trees::ContainedMachineTargetFact { machine: root }]);
        let child_fields = program.facts.carry.contained_fields.insert_many([
            checked_trees::ContainedMachineFieldFact {
                field: symbols::SymbolHandle::from_arena_index(21),
                data: symbols::SymbolHandle::from_arena_index(22),
                type_reference,
                targets: cycle_targets,
            },
        ]);
        program
            .facts
            .carry
            .machine_topologies
            .for_each_mut(|_, topology| {
                if topology.machine == child {
                    topology.fields = child_fields;
                }
            });

        assert_eq!(
            exact_activation_carry_subtree(&program, root).expect("exact ordered cyclic topology"),
            vec![root, child, sibling],
        );
    }

    #[test]
    fn activation_topology_rejects_missing_reached_row() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let child = program.facts.carry.suspension_crossings[0].machine;
        program
            .facts
            .carry
            .machine_topologies
            .for_each_mut(|_, topology| {
                if topology.machine == child {
                    topology.machine = symbols::SymbolHandle::invalid();
                }
            });
        assert!(topology_error(&program, root).contains("one exact row per reached machine"));
    }

    #[test]
    fn activation_topology_rejects_duplicate_reached_row() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let duplicate = program
            .facts
            .carry
            .machine_topologies
            .iter()
            .find(|(_, topology)| topology.machine == root)
            .expect("root topology")
            .1
            .clone();
        program.facts.carry.machine_topologies.append(duplicate);
        assert!(topology_error(&program, root).contains("exactly one row per reached machine"));
    }

    #[test]
    fn activation_topology_rejects_invalid_field_span() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .carry
            .machine_topologies
            .for_each_mut(|_, topology| {
                if topology.machine == root {
                    topology.fields = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
                }
            });
        assert!(topology_error(&program, root).contains("exact valid field span"));
    }

    #[test]
    fn activation_topology_rejects_empty_field_coordinate() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .carry
            .contained_fields
            .for_each_mut(|_, field| field.field = symbols::SymbolHandle::invalid());
        assert!(topology_error(&program, root).contains("nonempty exact coordinates"));
    }

    #[test]
    fn activation_topology_rejects_duplicate_field_coordinate() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let field = program
            .facts
            .carry
            .contained_fields
            .iter()
            .next()
            .expect("contained field")
            .1
            .clone();
        let fields = program
            .facts
            .carry
            .contained_fields
            .insert_many([field.clone(), field]);
        program
            .facts
            .carry
            .machine_topologies
            .for_each_mut(|_, topology| {
                if topology.machine == root {
                    topology.fields = fields;
                }
            });
        assert!(topology_error(&program, root).contains("unique within their machine"));
    }

    #[test]
    fn activation_topology_rejects_invalid_target_span() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .carry
            .contained_fields
            .for_each_mut(|_, field| {
                field.targets = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
            });
        assert!(topology_error(&program, root).contains("exact valid target span"));
    }

    #[test]
    fn activation_topology_rejects_empty_target_span() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .carry
            .contained_fields
            .for_each_mut(|_, field| field.targets = arena::HandleSpan::empty());
        assert!(topology_error(&program, root).contains("at least one exact target"));
    }

    #[test]
    fn activation_topology_rejects_duplicate_target() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let target = *program
            .facts
            .carry
            .contained_targets
            .iter()
            .next()
            .expect("contained target")
            .1;
        let targets = program
            .facts
            .carry
            .contained_targets
            .insert_many([target, target]);
        program
            .facts
            .carry
            .contained_fields
            .for_each_mut(|_, field| field.targets = targets);
        assert!(topology_error(&program, root).contains("field targets must be unique"));
    }

    #[test]
    fn activation_topology_rejects_missing_typed_target() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .carry
            .contained_targets
            .for_each_mut(|_, target| target.machine = symbols::SymbolHandle::invalid());
        assert!(topology_error(&program, root).contains("target must name an exact typed machine"));
    }

    #[test]
    fn activation_crossings_reject_missing_machine() {
        let (program, _, _, _) = activation_crossing_validation_fixture();
        let mut crossing = program.facts.carry.suspension_crossings[0].clone();
        crossing.machine = symbols::SymbolHandle::invalid();
        let diagnostics = validate_activation_carry_crossing(&program, &crossing)
            .expect_err("missing crossing machine must fail closed");
        assert!(diagnostics[0].message.contains("exact typed machine"));
    }

    #[test]
    fn activation_crossings_reject_cross_machine_state() {
        let (mut program, root, root_state, _) = activation_crossing_validation_fixture();
        program.facts.carry.suspension_crossings[0].state = root_state;
        assert!(crossing_error(&program, root).contains("belong to its exact typed machine"));
    }

    #[test]
    fn activation_crossings_reject_out_of_range_statement() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program.facts.carry.suspension_crossings[0].statement_index = 1;
        assert!(crossing_error(&program, root).contains("statement must belong"));
    }

    #[test]
    fn activation_crossings_reject_missing_flow_state() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program.facts.flow.control.states = Default::default();
        assert!(crossing_error(&program, root).contains("one exact checked flow state"));
    }

    #[test]
    fn activation_crossings_reject_ambiguous_flow_state() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let duplicate = program
            .facts
            .flow
            .control
            .states
            .iter()
            .next()
            .expect("flow state")
            .1
            .clone();
        program.facts.flow.control.states.append(duplicate);
        assert!(crossing_error(&program, root).contains("exactly one checked flow state"));
    }

    #[test]
    fn activation_crossings_reject_invalid_call_span() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program.facts.flow.control.states.for_each_mut(|_, state| {
            state.calls = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
        });
        assert!(crossing_error(&program, root).contains("exact valid call span"));
    }

    #[test]
    fn activation_crossings_reject_missing_call_coordinate() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program.facts.carry.suspension_crossings[0].call_ordinal = 2;
        assert!(crossing_error(&program, root).contains("one exact checked flow call"));
    }

    #[test]
    fn activation_crossings_reject_ambiguous_call_coordinate() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let call = program
            .facts
            .flow
            .control
            .calls
            .iter()
            .next()
            .expect("flow call")
            .1
            .clone();
        let calls = program
            .facts
            .flow
            .control
            .calls
            .insert_many([call.clone(), call]);
        program
            .facts
            .flow
            .control
            .states
            .for_each_mut(|_, state| state.calls = calls);
        assert!(crossing_error(&program, root).contains("exactly one checked flow call"));
    }

    #[test]
    fn activation_crossings_reject_target_drift() {
        let (mut program, root, _, child_state) = activation_crossing_validation_fixture();
        program.facts.carry.suspension_crossings[0].target = child_state;
        assert!(crossing_error(&program, root).contains("exact checked call target"));
    }

    #[test]
    fn activation_crossings_reject_missing_typed_target() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program.facts.carry.suspension_crossings[0].target = symbols::SymbolHandle::invalid();
        program
            .facts
            .flow
            .control
            .calls
            .for_each_mut(|_, call| call.target_symbol = symbols::SymbolHandle::invalid());
        assert!(crossing_error(&program, root).contains("target must name an exact typed state"));
    }

    #[test]
    fn activation_crossings_reject_non_suspending_call() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        program
            .facts
            .flow
            .control
            .calls
            .for_each_mut(|_, call| call.suspension = Default::default());
        assert!(crossing_error(&program, root).contains("may-suspend checked call"));
    }

    #[test]
    fn activation_crossings_reject_duplicate_coordinate() {
        let (mut program, root, _, _) = activation_crossing_validation_fixture();
        let duplicate = program.facts.carry.suspension_crossings[0].clone();
        program.facts.carry.suspension_crossings.push(duplicate);
        assert!(crossing_error(&program, root).contains("one row per exact call coordinate"));
    }

    fn concrete_task_start_fixture() -> (
        CheckedTrees,
        effects::SelectedProviderPlanFacts,
        Vec<effects::provider_plan::ProviderPlan>,
    ) {
        let source = r#"
            data Task<T> [linear] { provider: u64; activation: u64; }
            machine Task::settle<T>(self) {}
            data StartRejection { code: i32; }
            data StartOutcome<T, Arguments> {
                case Started(task: Task<T>);
                case Rejected(arguments: Arguments, reason: StartRejection);
            }
            boundary trait TaskRuntime {
                machine start<T, Arguments, machine Target>(
                    &self,
                    arguments: Arguments
                ) -> Task<T>
                where machine Target(arguments: Arguments) -> T suspends; blocks;
                ensures true;
                machine try_start<T, Arguments, machine Target>(
                    &self,
                    arguments: Arguments
                ) -> StartOutcome<T, Arguments>
                where machine Target(arguments: Arguments) -> T suspends; blocks;
                ensures true;
            }

            data LocalTaskRuntime { }
            LocalTaskRuntimeTaskRuntime: LocalTaskRuntime satisfies TaskRuntime;
            machine LocalTaskRuntime::start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> Task<T>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            satisfies TaskRuntime::start
            via Binding::CompilerIntrinsic;
            machine LocalTaskRuntime::try_start<T, Arguments, machine Target>(
                &self,
                arguments: Arguments
            ) -> StartOutcome<T, Arguments>
            where machine Target(arguments: Arguments) -> T suspends; blocks;
            satisfies TaskRuntime::try_start
            via Binding::CompilerIntrinsic;

            pub boundary data Sleeper;
            boundary machine Sleeper::park(token: i32) suspends;
            data Job { value: i32; }
            data Worker {}
            machine Worker::run(job: Job) -> i32 suspends; {
                let value: i32 = job.value;
                suspend Sleeper::park(value);
                value
            }
            data Main { runtime: &TaskRuntime; }
            machine Main::run(&mut self) {
                let job: Job = Job { value: 7 };
                let task: Task<i32> = self.runtime.start<Worker::run>(job);
                Task::settle(task);
                let retry: Job = Job { value: 9 };
                let outcome: StartOutcome<i32, Job> =
                    self.runtime.try_start<Worker::run>(retry);
                let retry_task: Task<i32> = outcome.task;
                Task::settle(retry_task);
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
        let provider_plans = crate::plans::derive_satisfies_plans(&typed, None);
        assert_eq!(provider_plans.len(), 1);
        assert!(
            crate::plans::validate_provider_plan_candidates(&typed, &provider_plans,).is_empty()
        );
        let selected = effects::SelectedProviderPlanFacts::from_selection(
            &provider_plans,
            &[provider_plans[0].name.clone()],
        )
        .expect("select complete TaskRuntime provider");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check and specialize task start");

        (checked, selected, provider_plans)
    }

    #[test]
    fn compiler_task_activation_settlement_borrows_shared_program_and_commits_two_rows() {
        let (checked, selected, _) = concrete_task_start_fixture();
        let original_sidecar = Arc::new(TaskActivationPlanSet::default());
        let mut retained = Arc::clone(&original_sidecar);

        settle_task_activation_plans(
            &mut retained,
            &checked,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect("shared checked custody should produce the complete activation sidecar");

        assert!(!Arc::ptr_eq(&original_sidecar, &retained));
        assert_eq!(retained.as_slice().len(), 2);
        assert!(
            retained
                .as_slice()
                .iter()
                .any(|activation| { activation.operation == TaskStartOperation::Start })
        );
        assert!(
            retained
                .as_slice()
                .iter()
                .any(|activation| { activation.operation == TaskStartOperation::TryStart })
        );
    }

    #[test]
    fn compact_equal_specialization_substitution_changes_authoritative_commitment() {
        let (mut checked, selected, _) = concrete_task_start_fixture();
        let baseline_start = task_start_selections(&checked)
            .expect("baseline task selections")
            .into_iter()
            .find(|selection| selection.operation == TaskStartOperation::Start)
            .expect("baseline start selection");
        let target_specialization_index = checked.typed.machine_specializations.len();
        checked.typed.machine_specializations.push(
            typed_trees::typed_trees::MachineSpecialization {
                template: baseline_start.target_machine,
                instance: baseline_start.target_machine,
                type_argument_identities: vec!["exact::Baseline".to_owned()],
                report_fingerprint: 0x4455,
                ..Default::default()
            },
        );
        let baseline_runtime = selected_task_runtime_provider(&checked, &selected, &baseline_start)
            .expect("baseline selected task runtime");
        let (baseline_machine, baseline_entry) = exact_task_activation_target(
            &checked,
            baseline_start.target_machine,
            baseline_start.target_entry,
        )
        .expect("baseline exact task target");
        let baseline_contract = exact_task_machine_contract(
            &checked,
            baseline_machine.symbol,
            baseline_machine.name.as_str(),
        )
        .expect("baseline exact task contract");
        let baseline_commitment = task_specialization_commitment(
            &checked,
            &baseline_start,
            baseline_machine,
            baseline_entry,
            baseline_contract,
            baseline_runtime.requirement_identity.as_str(),
        )
        .expect("baseline task specialization commitment");

        let mut substituted = checked.clone();
        substituted.typed.machine_specializations[target_specialization_index]
            .type_argument_identities[0] = "exact::Substituted".to_owned();
        assert_eq!(
            substituted.machine_specializations[target_specialization_index].report_fingerprint,
            checked.machine_specializations[target_specialization_index].report_fingerprint,
        );
        let (changed_machine, changed_entry) = exact_task_activation_target(
            &substituted,
            baseline_start.target_machine,
            baseline_start.target_entry,
        )
        .expect("changed exact task target");
        let changed_contract = exact_task_machine_contract(
            &substituted,
            changed_machine.symbol,
            changed_machine.name.as_str(),
        )
        .expect("changed exact task contract");
        let changed_commitment = task_specialization_commitment(
            &substituted,
            &baseline_start,
            changed_machine,
            changed_entry,
            changed_contract,
            baseline_runtime.requirement_identity.as_str(),
        )
        .expect("changed task specialization commitment");

        assert_ne!(baseline_commitment, changed_commitment);
    }

    #[test]
    fn compiler_task_activation_rejection_preserves_prior_sidecar_identity() {
        let (mut checked, selected, _) = concrete_task_start_fixture();
        let retained =
            elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
                .expect("fixture should produce a retained activation sidecar");
        let target = retained
            .as_slice()
            .iter()
            .find(|activation| activation.operation == TaskStartOperation::Start)
            .expect("fixture start activation")
            .target_machine;
        checked
            .facts
            .suspensions
            .machines
            .retain(|fact| fact.machine != target);
        let original_sidecar = Arc::new(retained);
        let mut retained = Arc::clone(&original_sidecar);

        let diagnostics = settle_task_activation_plans(
            &mut retained,
            &checked,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("missing exact suspension evidence must reject settlement");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has no checked suspension plan"
        );
        assert!(Arc::ptr_eq(&original_sidecar, &retained));
    }

    #[test]
    fn compiler_task_activation_settlement_commits_canonical_empty_sidecar() {
        let original_sidecar = Arc::new(TaskActivationPlanSet::default());
        let checked = CheckedTrees::default();
        let selected = effects::SelectedProviderPlanFacts::default();
        let mut retained = Arc::clone(&original_sidecar);

        settle_task_activation_plans(
            &mut retained,
            &checked,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect("an empty checked program should produce an empty activation sidecar");

        assert!(!Arc::ptr_eq(&original_sidecar, &retained));
        assert_eq!(retained.as_ref(), &TaskActivationPlanSet::default());
    }

    #[test]
    fn concrete_task_start_specialization_elaborates_a_validated_plan() {
        let (checked, selected, provider_plans) = concrete_task_start_fixture();

        let mut foreign_leaf_plan = provider_plans[0].clone();
        foreign_leaf_plan.schema.trait_name = "other::TaskRuntime".to_owned();
        let foreign_leaf_selected = effects::SelectedProviderPlanFacts::from_selection(
            &[foreign_leaf_plan.clone()],
            &[foreign_leaf_plan.name.clone()],
        )
        .expect("same-leaf foreign schema remains a structurally complete plan");
        let diagnostics = elaborate_task_activation_plans(
            &checked,
            &foreign_leaf_selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("same-leaf foreign schema must not satisfy exact TaskRuntime owner");
        assert!(
            diagnostics[0]
                .message
                .contains("no retained selected provider plan")
        );

        let mut wrong_method_owner_plan = provider_plans[0].clone();
        for method in &mut wrong_method_owner_plan.schema.methods {
            method.requirement_owner = "other::TaskRuntime".to_owned();
        }
        let wrong_method_owner_selected = effects::SelectedProviderPlanFacts::from_selection(
            &[wrong_method_owner_plan.clone()],
            &[wrong_method_owner_plan.name.clone()],
        )
        .expect("method-owner drift remains structurally complete");
        let diagnostics = elaborate_task_activation_plans(
            &checked,
            &wrong_method_owner_selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("method owner drift must not satisfy exact TaskRuntime requirement");
        assert!(
            diagnostics[0]
                .message
                .contains("no retained selected provider plan")
        );

        let task_activations =
            elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
                .expect("elaborate activation plan");
        let activations = task_activations.as_slice();
        assert_eq!(activations.len(), 2);
        let activation = activations
            .iter()
            .find(|activation| activation.operation == TaskStartOperation::Start)
            .expect("start activation plan");
        let target_machine_symbol = activation.target_machine;
        assert!(
            activations
                .iter()
                .any(|activation| activation.operation == TaskStartOperation::TryStart)
        );
        let target = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == activation.target_machine)
            .expect("target machine");
        assert_eq!(target.name.as_str(), "Worker::run");
        let plan = activation.plan.candidate();
        assert_eq!(plan.stack_plan.bytes, 16);
        assert_eq!(plan.stack_plan.alignment, 8);
        assert!(plan.may_suspend);
        assert!(!plan.may_block);
        assert_eq!(plan.canonical_suspension_crossings.len(), 1);
        assert_eq!(plan.carry_obligations, ActivationCarryObligations::none());
        assert!(plan.cancellation_required);
        assert_ne!(plan.machine_contract.normalized_identity(), 0);
        assert_ne!(plan.argument_layout.normalized_identity(), 0);
        assert_ne!(plan.terminal_outcome_layout.normalized_identity(), 0);
        assert_ne!(
            activation.plan.normalized_identity().normalized_identity(),
            0
        );
        assert_eq!(
            activation.selected_runtime.provider_plan_name,
            "LocalTaskRuntime::satisfies::TaskRuntime"
        );
        assert_eq!(
            activation.selected_runtime.runtime.normalized_identity(),
            selected
                .plans()
                .first()
                .expect("selected runtime plan")
                .report_fingerprint()
        );
        assert!(
            activation
                .selected_runtime
                .requirement_identity
                .contains("TaskRuntime")
        );

        let manifest = visualizations::task_activation_manifest_json(&checked, &task_activations);
        assert!(manifest.contains("\"operation\": \"start\""));
        assert!(manifest.contains("\"operation\": \"try_start\""));
        assert!(manifest.contains("\"target_machine\": \"Worker::run\""));
        assert!(manifest.contains("\"selected_runtime\": {"));
        assert!(
            manifest.contains("\"provider_plan\": \"LocalTaskRuntime::satisfies::TaskRuntime\"")
        );
        assert!(manifest.contains("\"stack_plan\": {\"bytes\": 16, \"alignment\": 8"));
        assert!(manifest.contains("\"canonical_suspension_crossings\": ["));
        assert!(manifest.contains(
            "\"cpu_thread_preservation\": {\"preserve_cpu\": false, \"preserve_host_thread\": false}"
        ));
        assert!(manifest.contains("\"cancellation_required\": true"));
        assert!(manifest.contains("\"activation_plan_id\": \"0x"));
        assert!(!manifest.contains("\"runtime_admission\""));

        let mut missing_suspension = checked.clone();
        missing_suspension
            .facts
            .suspensions
            .machines
            .retain(|fact| fact.machine != activation.target_machine);
        let diagnostics = elaborate_task_activation_plans(
            &missing_suspension,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("missing exact target suspension facts must fail closed");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has no checked suspension plan"
        );

        let mut missing_blocking = checked.clone();
        missing_blocking
            .facts
            .blocking
            .machines
            .retain(|fact| fact.machine != activation.target_machine);
        let diagnostics = elaborate_task_activation_plans(
            &missing_blocking,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("missing exact target blocking facts must fail closed");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has no checked blocking plan"
        );

        let unrelated_machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol != target_machine_symbol)
            .expect("unrelated checked machine")
            .symbol;

        let mut missing_carry = checked.clone();
        missing_carry
            .facts
            .carry
            .activation_wide_carry
            .retain(|fact| fact.machine != target_machine_symbol);
        let diagnostics = elaborate_task_activation_plans(
            &missing_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("missing exact target carry envelope must fail closed");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
        );

        let mut duplicate_carry = checked.clone();
        let duplicate = duplicate_carry
            .facts
            .carry
            .activation_wide_carry
            .iter()
            .find(|fact| fact.machine == target_machine_symbol)
            .expect("target carry envelope")
            .clone();
        duplicate_carry
            .facts
            .carry
            .activation_wide_carry
            .push(duplicate);
        let diagnostics = elaborate_task_activation_plans(
            &duplicate_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("duplicate exact target carry envelope must fail closed");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has duplicate exact activation-wide CPU/thread carry envelopes"
        );

        let mut incomplete_carry = checked.clone();
        incomplete_carry
            .facts
            .carry
            .activation_wide_carry
            .iter_mut()
            .find(|fact| fact.machine == target_machine_symbol)
            .expect("target carry envelope")
            .analysis_complete = false;
        let diagnostics = elaborate_task_activation_plans(
            &incomplete_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("incomplete exact target carry envelope must fail closed");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has incomplete activation-wide CPU/thread carry analysis"
        );

        let mut authoritative_carry = checked.clone();
        let target_carry = authoritative_carry
            .facts
            .carry
            .activation_wide_carry
            .iter_mut()
            .find(|fact| fact.machine == target_machine_symbol)
            .expect("target carry envelope");
        target_carry.effective.cpu = CarryCpu::Origin;
        target_carry.effective.host_thread = CarryHostThread::Origin;
        let authoritative = elaborate_task_activation_plans(
            &authoritative_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect("exact checked target envelope remains the sole carry authority");
        let authoritative_plan = authoritative
            .as_slice()
            .iter()
            .find(|activation| activation.operation == TaskStartOperation::Start)
            .expect("authoritative start activation")
            .plan
            .candidate();
        assert_eq!(
            authoritative_plan.carry_obligations,
            ActivationCarryObligations {
                preserve_cpu: true,
                preserve_host_thread: true,
            }
        );
        assert_eq!(authoritative_plan.stack_plan, plan.stack_plan);
        assert_eq!(
            authoritative_plan.canonical_suspension_crossings,
            plan.canonical_suspension_crossings,
        );

        let mut unrelated_carry = checked.clone();
        let mut unrelated = unrelated_carry
            .facts
            .carry
            .activation_wide_carry
            .iter()
            .find(|fact| fact.machine == target_machine_symbol)
            .expect("target carry envelope")
            .clone();
        unrelated.machine = unrelated_machine;
        unrelated_carry
            .facts
            .carry
            .activation_wide_carry
            .push(unrelated);
        assert_eq!(
            elaborate_task_activation_plans(
                &unrelated_carry,
                &selected,
                NativeTarget::macos_arm64(),
                &[],
            )
            .expect("unrelated carry envelope must not perturb exact target selection"),
            task_activations,
        );

        let mut unrelated_only = checked.clone();
        unrelated_only
            .facts
            .carry
            .activation_wide_carry
            .iter_mut()
            .find(|fact| fact.machine == target_machine_symbol)
            .expect("target carry envelope")
            .machine = unrelated_machine;
        let diagnostics = elaborate_task_activation_plans(
            &unrelated_only,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect_err("unrelated carry envelope must not satisfy exact target selection");
        assert_eq!(
            diagnostics[0].message,
            "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
        );
    }
}

//! Source dependencies of the selected supplier, without a second body walk.
//!
//! Calls, static selections, cleanup and operator choices consume checking's
//! retained topology. Physical provider leaves retain their original selected
//! row. This inventory does not certify a package policy, an opaque native
//! dependency closure, or the truth of any semantic assumption.

use super::{AssumedBindingRegistration, ProductBoundaryRequirementReference, supplier_links};
use crate::SelectedProviderReviewProvenance;
use arena::{Arena, Handle};
use checked_trees::{CheckedTrees, CheckedValueOrigin, ContractProofFact};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use symbols::SymbolHandle;

/// Coordinates in PreparedAssumedBindingRelationship's original selected owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingSupplierSelection {
    pub provider_ordinal: usize,
    pub row_ordinal: usize,
}

/// One original accepted judgment used in the reached supplier body.
/// The handle resolves in the prepared relationship's original checked owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssumedBindingProofJudgmentUse {
    judgment: Handle<checked_trees::CheckedProofJudgment>,
    mathematical_content: Option<super::AssumedBindingMathematicalJudgmentContent>,
}

impl AssumedBindingProofJudgmentUse {
    /// Exact source judgment in the prepared relationship's checked owner.
    /// Zero denotes only the arena's absent slot.
    pub fn judgment(&self) -> Handle<checked_trees::CheckedProofJudgment> {
        self.judgment
    }

    /// Canonical content of this original accepted mathematical certificate.
    /// For a live use, None means its accepted route has no mathematical
    /// certificate. The absent arena slot also has no content; neither case
    /// denotes an empty certificate or complete semantic dependency commitment.
    pub fn mathematical_content(&self) -> Option<super::AssumedBindingMathematicalJudgmentContent> {
        self.mathematical_content
    }
}

/// One reached source application under the original checked signature.
/// Formation provenance is retained separately from accepted proof judgments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssumedBindingMathematicalApplicationUse {
    application: Handle<checked_trees::CheckedMathematicalApplication>,
}

impl AssumedBindingMathematicalApplicationUse {
    pub fn application(&self) -> Handle<checked_trees::CheckedMathematicalApplication> {
        self.application
    }
}

/// Exact checked dependency evidence for the code selected at this crossing.
/// Arena handles belong to the prepared row's original declaration owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingSupplierDependencies {
    machines: Vec<SymbolHandle>,
    declarations: Vec<SymbolHandle>,
    selected_rows: Vec<AssumedBindingSupplierSelection>,
    assumed_requirements: Vec<ProductBoundaryRequirementReference>,
    semantic_dependencies: Vec<checked_trees::CheckedSemanticDependency>,
    contract_facts: Vec<Handle<ContractProofFact>>,
    accepted_judgments: Arena<AssumedBindingProofJudgmentUse>,
    source_contract_entailments: Arena<Handle<checked_trees::CheckedSourceContractEntailment>>,
    mathematical_applications: Arena<AssumedBindingMathematicalApplicationUse>,
    proof_calls: Vec<Handle<checked_trees::ProofOutputCallFact>>,
    qualification_facts: Vec<facts::FactHandle>,
    value_qualifications: Vec<Handle<checked_trees::CheckedValueQualification>>,
    mathematical: super::mathematical_dependencies::AssumedBindingMathematicalDependencies,
    qualification_correspondences: Vec<facts::QualificationCorrespondenceHandle>,
}

impl AssumedBindingSupplierDependencies {
    pub fn mathematical(
        &self,
    ) -> &super::mathematical_dependencies::AssumedBindingMathematicalDependencies {
        &self.mathematical
    }
    pub fn machines(&self) -> &[SymbolHandle] {
        &self.machines
    }
    pub fn declarations(&self) -> &[SymbolHandle] {
        &self.declarations
    }
    pub fn selected_rows(&self) -> &[AssumedBindingSupplierSelection] {
        &self.selected_rows
    }
    /// Other assumed links remain separate trust premises, including a link
    /// encountered inside the supplier's own implementation.
    pub fn assumed_requirements(&self) -> &[ProductBoundaryRequirementReference] {
        &self.assumed_requirements
    }
    pub fn semantic_dependencies(&self) -> &[checked_trees::CheckedSemanticDependency] {
        &self.semantic_dependencies
    }
    pub fn contract_facts(&self) -> &[Handle<ContractProofFact>] {
        &self.contract_facts
    }
    /// Certificate-local assumptions remain attached to each original
    /// accepted judgment, separate from authored mathematical roots.
    pub fn accepted_judgments(&self) -> &Arena<AssumedBindingProofJudgmentUse> {
        &self.accepted_judgments
    }
    /// Original accepted source contract batches for reached supplier machines.
    /// Their observed application reads resolve in the same original ProofFacts
    /// ledger. These handles are separate from bounded kernel accepted facts;
    /// absence is not a complete no-dependency receipt.
    pub fn source_contract_entailments(
        &self,
    ) -> &Arena<Handle<checked_trees::CheckedSourceContractEntailment>> {
        &self.source_contract_entailments
    }
    /// Original application formation rows owned by reached supplier machines,
    /// together with exact reads retained by their accepted source judgments.
    /// This does not certify that every proof judgment exported its use roots.
    pub fn mathematical_applications(&self) -> &Arena<AssumedBindingMathematicalApplicationUse> {
        &self.mathematical_applications
    }
    pub fn proof_calls(&self) -> &[Handle<checked_trees::ProofOutputCallFact>] {
        &self.proof_calls
    }
    pub fn qualification_facts(&self) -> &[facts::FactHandle] {
        &self.qualification_facts
    }
    pub fn value_qualifications(&self) -> &[Handle<checked_trees::CheckedValueQualification>] {
        &self.value_qualifications
    }
    pub fn qualification_correspondences(&self) -> &[facts::QualificationCorrespondenceHandle] {
        &self.qualification_correspondences
    }
}

pub(super) fn capture(
    program: &CheckedTrees,
    registrations: &[AssumedBindingRegistration],
    selected: &[SelectedProviderReviewProvenance],
    first: AssumedBindingSupplierSelection,
    first_application: Option<&crate::selected_applications::SelectedCheckedRequirementApplication>,
) -> Result<AssumedBindingSupplierDependencies, Diagnostic> {
    let mut inventory = AssumedBindingSupplierDependencies::default();
    select_row(program, selected, first, first_application, &mut inventory)?;
    let mut cursor = 0;
    while cursor < inventory.machines.len() {
        let symbol = inventory.machines[cursor];
        cursor += 1;
        let plan = &program.facts.operational;
        let index = plan.machine_index_by_symbol(symbol).ok_or_else(|| {
            Diagnostic::error(
                "assumed binding supplier dependency has no retained checked call topology",
            )
        })?;
        let machine = &plan.machines()[index];
        for state in plan.states.span_or_empty(machine.states) {
            for call in plan.calls.span_or_empty(state.calls) {
                let target = if call.target_machine_symbol.is_valid() {
                    call.target_machine_symbol
                } else if call.target_operator_symbol.is_valid() {
                    call.target_operator_symbol
                } else {
                    call.target_state_symbol
                };
                follow(program, registrations, selected, target, &mut inventory)?;
                follow_static_bindings(
                    program,
                    registrations,
                    selected,
                    call.static_machine_bindings,
                    &mut inventory,
                )?;
            }
        }
        for (_, cleanup) in plan.cleanup_dependencies.iter() {
            if cleanup.caller_machine == symbol {
                follow(
                    program,
                    registrations,
                    selected,
                    cleanup.target_machine,
                    &mut inventory,
                )?;
            }
        }
        // Spelled operators are value decisions rather than source call rows.
        for (_, operation) in program.facts.operators.uses.iter() {
            if origin_has_machine(program, operation.origin, symbol) {
                follow(
                    program,
                    registrations,
                    selected,
                    operation.selected_operator_symbol,
                    &mut inventory,
                )?;
            }
        }
        for (_, operation) in program.facts.operators.named_uses.iter() {
            if origin_has_machine(program, operation.origin, symbol) {
                follow(
                    program,
                    registrations,
                    selected,
                    operation.selected_operator_symbol,
                    &mut inventory,
                )?;
            }
        }
        // Erased proof use remains a semantic dependency even when no runtime
        // operation survived. Retain both its public target and exact dispatch.
        for (handle, call) in program.facts.proof.proof_output_calls.iter() {
            if call.caller_machine_symbol == symbol {
                inventory.proof_calls.push(handle);
                follow(
                    program,
                    registrations,
                    selected,
                    call_target(call.target_machine_symbol, call.target_state_symbol),
                    &mut inventory,
                )?;
                if let Some(dispatch) = &call.static_requirement_dispatch {
                    append_unique(&mut inventory.declarations, dispatch.declaring_trait);
                    append_unique(&mut inventory.declarations, dispatch.requirement);
                    follow(
                        program,
                        registrations,
                        selected,
                        dispatch.realization_machine,
                        &mut inventory,
                    )?;
                }
            }
        }
        for call in &program.facts.proof.contract_expression_evidence_calls {
            if contract_machine(call.owner) == Some(symbol) {
                follow(
                    program,
                    registrations,
                    selected,
                    call_target(call.target_machine_symbol, call.target_state_symbol),
                    &mut inventory,
                )?;
            }
        }
        for (_, call) in program.facts.proof.contract_calls.iter() {
            if call.caller_machine_symbol == symbol {
                follow(
                    program,
                    registrations,
                    selected,
                    call_target(call.target_machine_symbol, call.target_state_symbol),
                    &mut inventory,
                )?;
            }
        }
        for dependency in program.facts.flow.semantic_dependencies.iter() {
            if dependency.consumer_machine == symbol {
                append_unique(&mut inventory.declarations, dependency.dependency);
                inventory.semantic_dependencies.push(*dependency);
            }
        }
        for (handle, contract) in program.facts.proof.contract_facts.iter() {
            let owner = contract_machine(contract.owner);
            if owner == Some(symbol) {
                inventory.contract_facts.push(handle);
            }
        }
        for (handle, application) in program.facts.proof.mathematical_applications.iter() {
            if application.source_owner == symbol {
                append_mathematical_application(&mut inventory, handle);
            }
        }
        let source_entailments = &program.facts.proof.source_contract_entailments;
        for (handle, entailment) in source_entailments.entailments().iter() {
            if entailment.machine == symbol {
                inventory.source_contract_entailments.append(handle);
                for application in source_entailments.mathematical_reads(entailment) {
                    append_mathematical_application(&mut inventory, *application);
                }
            }
        }
        for (handle, judgment) in program.facts.proof.accepted_judgments.iter() {
            let obligation = program.facts.proof.obligations.get(judgment.obligation);
            if obligation.machine_symbol == symbol {
                inventory
                    .accepted_judgments
                    .append(AssumedBindingProofJudgmentUse {
                        judgment: handle,
                        mathematical_content: super::proof_content::capture(judgment)?,
                    });
            }
        }
        for (handle, fact) in program.facts.semantic.facts.iter() {
            if point_machine(fact.point) == Some(symbol)
                && facts::QualificationPayloadIdentity::from_fact_payload(fact.payload).is_some()
            {
                inventory.qualification_facts.push(handle);
                if let Some((_, _, domain_symbol, _)) = fact.payload.domain_membership()
                    && domain_symbol.is_valid()
                {
                    append_unique(&mut inventory.declarations, domain_symbol);
                }
                if fact.evidence.source_symbol.is_valid() {
                    append_unique(&mut inventory.declarations, fact.evidence.source_symbol);
                }
            }
        }
        for (handle, formation) in program.facts.qualifications.value_formations.iter() {
            if formation.machine == symbol {
                inventory.value_qualifications.push(handle);
            }
        }
        for (handle, correspondence) in program.facts.semantic.qualification_correspondences.iter()
        {
            if point_machine(correspondence.formation) == Some(symbol) {
                inventory.qualification_correspondences.push(handle);
            }
        }
    }
    inventory.mathematical = super::mathematical_dependencies::capture(
        program,
        &inventory.declarations,
        &inventory.mathematical_applications,
    )?;
    for root in inventory.mathematical.roots() {
        append_unique(&mut inventory.declarations, root.symbol);
    }
    Ok(inventory)
}

fn append_mathematical_application(
    inventory: &mut AssumedBindingSupplierDependencies,
    application: Handle<checked_trees::CheckedMathematicalApplication>,
) {
    if !inventory
        .mathematical_applications
        .iter()
        .any(|(_, existing)| existing.application == application)
    {
        inventory
            .mathematical_applications
            .append(AssumedBindingMathematicalApplicationUse { application });
    }
}

fn follow(
    program: &CheckedTrees,
    registrations: &[AssumedBindingRegistration],
    selected: &[SelectedProviderReviewProvenance],
    target: SymbolHandle,
    inventory: &mut AssumedBindingSupplierDependencies,
) -> Result<(), Diagnostic> {
    if !target.is_valid() {
        return Ok(());
    }
    let target = program
        .machine_holding_state(target)
        .map_or(target, |machine| machine.symbol);
    append_unique(&mut inventory.declarations, target);
    let mut matching = Vec::new();
    for registration in registrations {
        let instance =
            checked_trees::typed_trees::product_requirements::prepared_product_requirement_symbol(
                program,
                &registration.required.application_demand(),
            )?;
        if instance == target {
            matching.push(registration);
        }
    }
    let endpoint = match matching.as_slice() {
        [registration] => {
            let links = supplier_links(registrations, &registration.required)?;
            for link in links.iter().take(links.len().saturating_sub(1)) {
                append_unique(&mut inventory.assumed_requirements, link.clone());
            }
            let last = links
                .last()
                .expect("supplier links include their original requirement");
            checked_trees::typed_trees::product_requirements::prepared_product_requirement_symbol(
                program,
                &last.application_demand(),
            )?
        }
        [] => target,
        _ => {
            return Err(Diagnostic::error(
                "assumed supplier dependency has competing exact requirement applications",
            ));
        }
    };
    let endpoint_template =
        checked_trees::typed_trees::product_requirements::original_prepared_requirement_symbol(
            program, endpoint,
        )?;
    let mut matches = selected
        .iter()
        .enumerate()
        .flat_map(|(provider_ordinal, supplier)| {
            supplier
                .provider
                .row_requirements
                .iter()
                .enumerate()
                .filter_map(move |(row_ordinal, requirement)| {
                    (*requirement == endpoint_template).then_some(AssumedBindingSupplierSelection {
                        provider_ordinal,
                        row_ordinal,
                    })
                })
        });
    if let Some(selection) = matches.next() {
        if matches.next().is_some() {
            return Err(Diagnostic::error(
                "assumed binding dependency has competing selected supplier rows",
            ));
        }
        let supplier = &selected[selection.provider_ordinal];
        let application = if endpoint != endpoint_template
            && matches!(
                supplier.plan.rows[selection.row_ordinal].binding,
                ProviderBinding::CheckedAdapter { .. }
            ) {
            Some(
                crate::selected_applications::selected_checked_requirement_application(
                    program,
                    supplier,
                    selection.row_ordinal,
                    endpoint,
                )?,
            )
        } else {
            if endpoint != endpoint_template {
                return Err(Diagnostic::error(
                    "assumed dependency physical supplier needs its exact applied payload",
                ));
            }
            None
        };
        return select_row(
            program,
            selected,
            selection,
            application.as_ref(),
            inventory,
        );
    }
    if let Some(machine) = program.machine(endpoint) {
        if machine.supply_mode.is_boundary_declaration() {
            return Err(Diagnostic::error(
                "assumed binding dependency has no selected boundary supplier",
            ));
        }
        append_unique(&mut inventory.machines, machine.symbol);
    } else if program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == endpoint)
    }) {
        return Err(Diagnostic::error(
            "assumed binding dependency has no selected boundary supplier",
        ));
    }
    Ok(())
}

fn select_row(
    program: &CheckedTrees,
    selected: &[SelectedProviderReviewProvenance],
    selection: AssumedBindingSupplierSelection,
    application: Option<&crate::selected_applications::SelectedCheckedRequirementApplication>,
    inventory: &mut AssumedBindingSupplierDependencies,
) -> Result<(), Diagnostic> {
    let supplier = selected.get(selection.provider_ordinal).ok_or_else(|| {
        Diagnostic::error("assumed binding dependency lost its selected provider owner")
    })?;
    let row = supplier
        .plan
        .rows
        .get(selection.row_ordinal)
        .ok_or_else(|| {
            Diagnostic::error("assumed binding dependency lost its selected provider row")
        })?;
    let realization = *supplier
        .provider
        .row_realizations
        .get(selection.row_ordinal)
        .ok_or_else(|| {
            Diagnostic::error("assumed binding dependency lost its row-aligned realization")
        })?;
    let realization = if let Some(application) = application {
        if application.realization_template() != realization
            || supplier
                .provider
                .row_requirements
                .get(selection.row_ordinal)
                != Some(&application.requirement_template())
        {
            return Err(Diagnostic::error(
                "assumed dependency application lost its original selected row correspondence",
            ));
        }
        application.realization_instance()
    } else {
        if program
            .machine(realization)
            .is_some_and(|machine| machine.has_specialization_parameters())
        {
            return Err(Diagnostic::error(
                "assumed supplier dependency retains an unapplied body",
            ));
        }
        realization
    };
    append_unique(&mut inventory.selected_rows, selection);
    if realization.is_valid() {
        append_unique(&mut inventory.declarations, realization);
        if program.machine(realization).is_some() {
            append_unique(&mut inventory.machines, realization);
        }
    }
    if matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) && !realization.is_valid() {
        return Err(Diagnostic::error(
            "assumed binding checked dependency has no exact realizing machine",
        ));
    }
    Ok(())
}

fn follow_static_bindings(
    program: &CheckedTrees,
    registrations: &[AssumedBindingRegistration],
    selected: &[SelectedProviderReviewProvenance],
    bindings: arena::HandleSpan<flow_effects::StaticMachineCallBinding>,
    inventory: &mut AssumedBindingSupplierDependencies,
) -> Result<(), Diagnostic> {
    for binding in program
        .facts
        .operational
        .static_machine_bindings
        .span_or_empty(bindings)
    {
        follow(
            program,
            registrations,
            selected,
            binding.selected,
            inventory,
        )?;
        follow_static_bindings(
            program,
            registrations,
            selected,
            binding.arguments,
            inventory,
        )?;
    }
    Ok(())
}

fn origin_has_machine(
    program: &CheckedTrees,
    origin: CheckedValueOrigin,
    machine: SymbolHandle,
) -> bool {
    let mut pending = vec![origin];
    let mut visited = Vec::new();
    while let Some(origin) = pending.pop() {
        if let Some(owner) = origin.machine_symbol() {
            if owner == machine {
                return true;
            }
        } else if let CheckedValueOrigin::NestedExpression { parent } = origin
            && parent.is_valid()
            && !visited.contains(&parent)
        {
            visited.push(parent);
            pending.extend(
                program
                    .facts
                    .values
                    .expression_values(parent)
                    .map(|(_, value)| value.origin),
            );
        }
    }
    false
}

fn point_machine(point: facts::ProgramPoint) -> Option<SymbolHandle> {
    match point {
        facts::ProgramPoint::Global | facts::ProgramPoint::Definition { .. } => None,
        facts::ProgramPoint::Machine { machine_symbol }
        | facts::ProgramPoint::State { machine_symbol, .. }
        | facts::ProgramPoint::Statement { machine_symbol, .. }
        | facts::ProgramPoint::Call { machine_symbol, .. }
        | facts::ProgramPoint::CallRequires { machine_symbol, .. }
        | facts::ProgramPoint::CallEnsures { machine_symbol, .. }
        | facts::ProgramPoint::TransitionArm { machine_symbol, .. }
        | facts::ProgramPoint::Exit { machine_symbol, .. } => Some(machine_symbol),
    }
}

fn append_unique<T: PartialEq>(rows: &mut Vec<T>, value: T) {
    if !rows.contains(&value) {
        rows.push(value);
    }
}

fn contract_machine(owner: checked_trees::ContractProofFactOwner) -> Option<SymbolHandle> {
    match owner {
        checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } => {
            Some(machine_symbol)
        }
        _ => None,
    }
}

fn call_target(machine: SymbolHandle, state: SymbolHandle) -> SymbolHandle {
    if machine.is_valid() { machine } else { state }
}

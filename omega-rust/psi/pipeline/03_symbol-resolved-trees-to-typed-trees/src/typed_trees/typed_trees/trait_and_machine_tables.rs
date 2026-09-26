//! The trait, conformance, boundary calling plan, machine and state tables.

use crate::typed_trees::typed_trees::{BoundaryCallingPlanIdentity, TypedTrees};
use crate::typed_trees::{data, machine, signature, trait_definition};

impl TypedTrees {
    pub fn push_trait_definition(&mut self, trait_definition: trait_definition::TraitDefinition) {
        self.tables
            .traits
            .append_to_span(&mut self.roots.traits, trait_definition);
    }

    pub fn traits(&self) -> &[trait_definition::TraitDefinition] {
        self.tables.traits.span_or_empty(self.roots.traits)
    }

    pub fn push_conformance(&mut self, conformance: trait_definition::Conformance) {
        self.tables
            .conformances
            .append_to_span(&mut self.roots.conformances, conformance);
    }

    pub fn conformances(&self) -> &[trait_definition::Conformance] {
        self.tables
            .conformances
            .span_or_empty(self.roots.conformances)
    }

    pub fn conformance_type_parameters(
        &self,
        conformance: &trait_definition::Conformance,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(conformance.type_parameters)
    }

    pub fn closed_conformance_rows<'conformance>(
        &self,
        conformance: &'conformance trait_definition::Conformance,
    ) -> Option<&'conformance [trait_definition::ConformanceRow]> {
        match &conformance.implementation {
            trait_definition::ConformanceImplementation::AttachedRequirementMachines => None,
            trait_definition::ConformanceImplementation::Closed { rows } => Some(rows),
        }
    }

    pub fn push_trait_type_parameter(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut trait_definition.type_parameters, type_parameter);
    }

    pub fn trait_type_parameters(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(trait_definition.type_parameters)
    }

    pub fn push_trait_requirement(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        requirement: trait_definition::TraitRequirement,
    ) {
        self.trait_requirements
            .append_to_span(&mut trait_definition.requires, requirement);
    }

    pub fn trait_requirements(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[trait_definition::TraitRequirement] {
        self.trait_requirements
            .span_or_empty(trait_definition.requires)
    }

    pub fn trait_composition_kind(
        &self,
        requirement: &trait_definition::TraitRequirement,
    ) -> Option<trait_definition::TraitCompositionKind> {
        self.traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol)
            .map(|candidate| {
                if candidate.is_boundary {
                    trait_definition::TraitCompositionKind::ServiceReach
                } else {
                    trait_definition::TraitCompositionKind::Policy
                }
            })
    }

    pub fn record_boundary_calling_plan(&mut self, identity: BoundaryCallingPlanIdentity) {
        if let Some(existing) = self.boundary_calling_plans.iter_mut().find(|candidate| {
            candidate.boundary_trait == identity.boundary_trait
                && candidate.boundary_arguments == identity.boundary_arguments
                && candidate.requirement_machine == identity.requirement_machine
        }) {
            *existing = identity;
        } else {
            self.boundary_calling_plans.push(identity);
        }
    }

    pub fn boundary_calling_plan_report_fingerprint(
        &self,
        boundary_trait: symbols::SymbolHandle,
        requirement_machine: symbols::SymbolHandle,
    ) -> Option<u64> {
        self.boundary_calling_plan_report_fingerprint_for_arguments(
            boundary_trait,
            &[],
            requirement_machine,
        )
    }

    pub fn boundary_calling_plan_report_fingerprint_for_arguments(
        &self,
        boundary_trait: symbols::SymbolHandle,
        boundary_arguments: &[crate::typed_trees::types::TypeReferenceHandle],
        requirement_machine: symbols::SymbolHandle,
    ) -> Option<u64> {
        self.boundary_calling_plans
            .iter()
            .find(|identity| {
                identity.boundary_trait == boundary_trait
                    && identity.boundary_arguments == boundary_arguments
                    && identity.requirement_machine == requirement_machine
            })
            .map(|identity| identity.report_fingerprint)
    }

    pub fn boundary_calling_plan_identity_for_arguments(
        &self,
        boundary_trait: symbols::SymbolHandle,
        boundary_arguments: &[crate::typed_trees::types::TypeReferenceHandle],
        requirement_machine: symbols::SymbolHandle,
    ) -> Option<&BoundaryCallingPlanIdentity> {
        self.boundary_calling_plans.iter().find(|identity| {
            identity.boundary_trait == boundary_trait
                && identity.boundary_arguments == boundary_arguments
                && identity.requirement_machine == requirement_machine
        })
    }

    pub fn boundary_calling_plan_identity(
        &self,
        boundary_trait: symbols::SymbolHandle,
        requirement_machine: symbols::SymbolHandle,
    ) -> Option<&BoundaryCallingPlanIdentity> {
        self.boundary_calling_plan_identity_for_arguments(boundary_trait, &[], requirement_machine)
    }

    pub fn push_trait_machine_signature(
        &mut self,
        trait_definition: &mut trait_definition::TraitDefinition,
        signature: signature::StateSignature,
    ) {
        self.trait_machine_signatures
            .append_to_span(&mut trait_definition.machines, signature);
    }

    pub fn trait_machine_signatures(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
    ) -> &[signature::StateSignature] {
        self.trait_machine_signatures
            .span_or_empty(trait_definition.machines)
    }

    pub fn machine_parameter_contract_view<'program>(
        &'program self,
        contract: &'program data::MachineParameterContract,
    ) -> Option<data::MachineParameterContractView<'program>> {
        match contract {
            data::MachineParameterContract::RequirementIdentity => None,
            data::MachineParameterContract::Structural(signature) => {
                Some(data::MachineParameterContractView::Structural(signature))
            }
            data::MachineParameterContract::Nominal {
                trait_definition,
                requirement,
            } => {
                let trait_definition = self
                    .traits()
                    .iter()
                    .find(|candidate| candidate.symbol == *trait_definition)?;
                let requirement = self
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|candidate| candidate.symbol == *requirement)?;
                Some(data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement,
                })
            }
        }
    }

    pub fn push_machine(&mut self, machine: machine::Machine) {
        self.tables
            .machines
            .append_to_span(&mut self.roots.machines, machine);
    }

    /// The realized declaration named `name`: never a target sibling, which
    /// shares its family's authored name but belongs to another target and
    /// is pruned once checking has committed. Evaluators and settlement
    /// select through this; checking iterates `machines()` for every body.
    pub fn realized_machine_named(&self, name: &str) -> Option<&machine::Machine> {
        self.machines()
            .iter()
            .find(|machine| machine.target.is_none() && machine.name.as_str() == name)
    }

    #[inline]
    pub fn machines(&self) -> &[machine::Machine] {
        self.tables.machines.span_or_empty(self.roots.machines)
    }

    #[inline]
    pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
        self.tables.machines.span_mut_or_empty(self.roots.machines)
    }

    pub fn push_machine_type_parameter(
        &mut self,
        machine: &mut machine::Machine,
        type_parameter: data::TypeParameter,
    ) {
        self.data_type_parameters
            .append_to_span(&mut machine.type_parameters, type_parameter);
    }

    #[inline]
    pub fn machine_type_parameters(&self, machine: &machine::Machine) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(machine.type_parameters)
    }

    /// The authored callable contract of a compile-time machine parameter in
    /// `machine`. The parameter symbol is also the contract's call-target
    /// identity until specialization replaces it with a concrete state.
    pub fn machine_parameter_signature_in(
        &self,
        machine: &machine::Machine,
        symbol: symbols::SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        self.machine_type_parameters(machine)
            .iter()
            .find_map(|parameter| match &parameter.kind {
                data::TypeParameterKind::Machine { contract } if parameter.symbol == symbol => self
                    .machine_parameter_contract_view(contract)
                    .map(data::MachineParameterContractView::signature),
                _ => None,
            })
    }

    /// Rejoin the original binder contract after specialization removed its
    /// live generic parameter span. This is a contract lookup, not authority:
    /// callers must also validate the specialization's binder/selection join.
    pub fn retained_static_machine_contract(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<data::MachineParameterContractView<'_>> {
        if !symbol.is_valid() {
            return None;
        }
        self.data_type_parameters.iter().find_map(|(_, parameter)| {
            let data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                return None;
            };
            (parameter.symbol == symbol)
                .then(|| self.machine_parameter_contract_view(contract))
                .flatten()
        })
    }

    /// Find a machine-parameter contract and its declaring machine by its
    /// normalized symbol. Used by service-reach/proof consumers that see only a call
    /// target, not the lexical generic scope.
    pub fn machine_parameter_signature(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<(&machine::Machine, &signature::StateSignature)> {
        self.machines().iter().find_map(|machine| {
            self.machine_parameter_signature_in(machine, symbol)
                .map(|signature| (machine, signature))
        })
    }

    pub fn push_machine_owned_data(
        &mut self,
        machine: &mut machine::Machine,
        owned_data: machine::OwnedData,
    ) {
        self.machine_owned_data
            .append_to_span(&mut machine.owned_data, owned_data);
    }

    pub fn machine_owned_data(&self, machine: &machine::Machine) -> &[machine::OwnedData] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn push_machine_trait_conformance(
        &mut self,
        machine: &mut machine::Machine,
        conformance: machine::TraitConformance,
    ) {
        self.machine_trait_conformances
            .append_to_span(&mut machine.satisfies, conformance);
    }

    /// The conformances a machine contributes to this realization. A target
    /// sibling contributes none: its own conformances enter only when
    /// per-target selection promotes it to the family's realized body.
    pub fn machine_trait_conformances(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::TraitConformance] {
        if machine.target.is_some() {
            return &[];
        }
        self.declared_machine_trait_conformances(machine)
    }

    /// Every conformance a machine declares, including a target sibling's.
    pub fn declared_machine_trait_conformances(
        &self,
        machine: &machine::Machine,
    ) -> &[machine::TraitConformance] {
        self.machine_trait_conformances
            .span_or_empty(machine.satisfies)
    }

    pub fn push_machine_invoke(
        &mut self,
        machine: &mut machine::Machine,
        invocation: signature::AuthoredInvocation,
    ) {
        self.signature_invokes
            .append_to_span(&mut machine.invokes, invocation);
    }

    pub fn machine_invokes(&self, machine: &machine::Machine) -> &[signature::AuthoredInvocation] {
        self.signature_invokes.span_or_empty(machine.invokes)
    }

    pub fn push_machine_contract(
        &mut self,
        machine: &mut machine::Machine,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut machine.contracts, contract);
    }

    pub fn machine_contracts(&self, machine: &machine::Machine) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(machine.contracts)
    }

    pub fn push_machine_state(
        &mut self,
        machine: &mut machine::Machine,
        state: crate::typed_trees::state::State,
    ) {
        self.machine_states
            .append_to_span(&mut machine.states, state);
    }

    #[inline]
    pub fn machine_states(
        &self,
        machine: &machine::Machine,
    ) -> &[crate::typed_trees::state::State] {
        self.machine_states.span_or_empty(machine.states)
    }

    #[inline]
    pub fn machine_states_mut(
        &mut self,
        machine: &machine::Machine,
    ) -> &mut [crate::typed_trees::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    /// The machine whose state list stores `state_symbol`. A state's
    /// retained parent names its owning machine, so the common case is one
    /// symbol read, one machines-row scan, and one membership check. The
    /// whole-program scan still runs when the retained parent disagrees with
    /// storage, but only for a symbol that `may_name_stored_state`: call
    /// targets naming a machine head, builtin, parameter or trait
    /// requirement skip it. State spans are disjoint append-only ranges, so
    /// at most one machine can contain a given state symbol.
    pub fn machine_holding_state(
        &self,
        state_symbol: symbols::SymbolHandle,
    ) -> Option<&machine::Machine> {
        if !state_symbol.is_valid() {
            return None;
        }
        let symbol = self.symbols.get(state_symbol);
        let parent = symbol.parent;
        if let Some(machine) = self
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == parent)
            && self
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == state_symbol)
        {
            return Some(machine);
        }
        if !self.may_name_stored_state(symbol) {
            return None;
        }
        self.machines()
            .iter()
            .filter(|candidate| candidate.symbol != parent)
            .find(|machine| {
                self.machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == state_symbol)
            })
    }

    /// Whether a symbol can name a stored state row: a state symbol, or one
    /// the table cannot resolve, and not a state under a trait, which is a
    /// requirement signature no machine stores.
    pub fn may_name_stored_state(&self, symbol: &symbols::Symbol) -> bool {
        matches!(
            symbol.kind,
            symbols::SymbolKind::State | symbols::SymbolKind::Unknown
        ) && self.symbols.get(symbol.parent).kind != symbols::SymbolKind::Trait
    }

    /// The state `state_symbol` names, resolved through its owning
    /// machine's span rather than a whole-program state scan.
    pub fn state_by_symbol(
        &self,
        state_symbol: symbols::SymbolHandle,
    ) -> Option<&crate::typed_trees::state::State> {
        self.machine_holding_state(state_symbol)
            .and_then(|machine| {
                self.machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == state_symbol)
            })
    }

    pub fn push_state_parameter(
        &mut self,
        state: &mut crate::typed_trees::state::State,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut state.parameters, parameter);
    }

    #[inline]
    pub fn state_parameters(
        &self,
        state: &crate::typed_trees::state::State,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn push_state_contract(
        &mut self,
        state: &mut crate::typed_trees::state::State,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut state.contracts, contract);
    }

    #[inline]
    pub fn state_contracts(
        &self,
        state: &crate::typed_trees::state::State,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(state.contracts)
    }

    pub fn push_state_signature_parameter(
        &mut self,
        signature: &mut signature::StateSignature,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut signature.parameters, parameter);
    }

    pub fn state_signature_parameters(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(signature.parameters)
    }

    pub fn state_signature_type_parameters(
        &self,
        signature: &signature::StateSignature,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(signature.type_parameters)
    }

    pub fn push_state_signature_invoke(
        &mut self,
        signature: &mut signature::StateSignature,
        invocation: signature::AuthoredInvocation,
    ) {
        self.signature_invokes
            .append_to_span(&mut signature.invokes, invocation);
    }

    pub fn state_signature_invokes(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::AuthoredInvocation] {
        self.signature_invokes.span_or_empty(signature.invokes)
    }

    pub fn push_state_signature_contract(
        &mut self,
        signature: &mut signature::StateSignature,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut signature.contracts, contract);
    }

    pub fn state_signature_contracts(
        &self,
        signature: &signature::StateSignature,
    ) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(signature.contracts)
    }
}

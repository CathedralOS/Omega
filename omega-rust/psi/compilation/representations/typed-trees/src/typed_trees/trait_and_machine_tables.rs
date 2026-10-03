//! The trait, conformance, boundary calling plan, machine and state tables.

use crate::typed_trees::{BoundaryCallingPlanIdentity, TypedTrees};
use crate::{data, machine, signature, trait_definition};

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
        boundary_arguments: &[crate::types::TypeReferenceHandle],
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
        boundary_arguments: &[crate::types::TypeReferenceHandle],
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
        state: crate::state::State,
    ) {
        self.machine_states
            .append_to_span(&mut machine.states, state);
    }

    #[inline]
    pub fn machine_states(&self, machine: &machine::Machine) -> &[crate::state::State] {
        self.machine_states.span_or_empty(machine.states)
    }

    #[inline]
    pub fn machine_states_mut(&mut self, machine: &machine::Machine) -> &mut [crate::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    /// Whether some state of `machine` declares an authored `self`
    /// parameter. Only such a receiver requests attachment storage while
    /// receiver-based source is migrated: under the static-machine contract a
    /// carrier qualification (`Counter::clear`) names the semantic owner and
    /// contributes no argument or storage, so `machine.attached_data` alone
    /// never implies an attachment.
    pub fn machine_has_receiver(&self, machine: &machine::Machine) -> bool {
        self.machine_states(machine).iter().any(|state| {
            self.state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self)
        })
    }

    /// The parameter of `parameters` that carries the machine's data record.
    ///
    /// Under the receiver spelling this is the authored `self`, which cannot be
    /// shadowed, so a declaration that takes one reaches its data through
    /// nothing else and the receiver wins outright. The same declaration
    /// written statically names the data directly (`main: &mut Main`), and the
    /// attached data's symbol is what identifies it. Both spellings answer one
    /// question -- which binding holds the record this body's field places are
    /// rooted at -- so a rendered place is data-rooted when its head names
    /// this parameter, whatever the parameter is called.
    ///
    /// `None` when the declaration reaches no single such record:
    ///
    /// - a machine qualified by a carrier it takes no parameter of
    ///   (`Main::set_at(cell: &mut Cell)`), because the qualification
    ///   contributes no argument or storage;
    /// - two parameters declared as the attached data, because they are
    ///   distinct storage and choosing one would analyze the wrong place.
    pub fn data_binding_parameter<'parameters>(
        &self,
        machine: &machine::Machine,
        parameters: &'parameters [signature::StateParameter],
    ) -> Option<&'parameters signature::StateParameter> {
        self.data_binding_position(machine.attached_data_symbol, parameters)
            .map(|index| &parameters[index])
    }

    /// The name of `state`'s data binding, for callers comparing rendered
    /// place text.
    pub fn state_data_binding(
        &self,
        machine: &machine::Machine,
        state: &crate::state::State,
    ) -> Option<&crate::name::Identifier> {
        self.data_binding_parameter(machine, self.state_parameters(state))
            .map(|parameter| &parameter.name)
    }

    /// The data binding of `machine`'s entry state, for the callers that hold
    /// a machine without one of its states.
    pub fn machine_data_binding(
        &self,
        machine: &machine::Machine,
    ) -> Option<&crate::name::Identifier> {
        let entry = self.machine_states(machine).first()?;
        self.state_data_binding(machine, entry)
    }

    /// Whether `parameter` is declared as this machine's exact attached data,
    /// looking through the reference and constraint shells a `&mut Main`
    /// binding carries.
    ///
    /// This answers "could this position be the subject", which is what a
    /// caller holding a position already knows it wants: a trait requirement
    /// names the subject's position, so its satisfier's parameter there needs
    /// only to declare the conforming data. `data_binding_parameter` is the
    /// stronger question -- WHICH parameter is the subject -- and refuses when
    /// two could be. `equals(point: &Point, other: &Point)` is exactly that
    /// difference: no single data binding, yet position 0 is the subject
    /// because the requirement says so.
    pub fn parameter_declares_attached_data(
        &self,
        machine: &machine::Machine,
        parameter: &signature::StateParameter,
    ) -> bool {
        parameter.is_self
            || self.type_reference_names_attached_data(
                parameter.type_reference,
                machine.attached_data_symbol,
            )
    }

    /// The position of the one parameter carrying `attached_data_symbol`'s
    /// record, by the same rule as `data_binding_parameter`: a receiver wins
    /// outright, otherwise the single parameter declaring that data, and
    /// `None` when two could be.
    ///
    /// Callers that hold the attachment without the machine record ask here.
    pub fn data_binding_position(
        &self,
        attached_data_symbol: symbols::SymbolHandle,
        parameters: &[signature::StateParameter],
    ) -> Option<usize> {
        if let Some(index) = parameters.iter().position(|parameter| parameter.is_self) {
            return Some(index);
        }
        if !attached_data_symbol.is_valid() {
            return None;
        }
        let mut declared = parameters.iter().enumerate().filter(|(_, parameter)| {
            self.type_reference_names_attached_data(parameter.type_reference, attached_data_symbol)
        });
        let (index, _) = declared.next()?;
        declared.next().is_none().then_some(index)
    }

    /// Whether `type_reference` names `attached_data_symbol` once references
    /// and constraints are stripped.
    ///
    /// A data binding is spelled `&mut Main`, `&Main` or `Main`; the
    /// ownership mode decides borrow access, not which record the binding
    /// names, so every mode answers this the same way.
    fn type_reference_names_attached_data(
        &self,
        type_reference: crate::types::TypeReferenceHandle,
        attached_data_symbol: symbols::SymbolHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self.type_reference_table.type_reference(type_reference) {
            crate::types::TypeReferenceNode::Reference { referee, .. } => {
                self.type_reference_names_attached_data(*referee, attached_data_symbol)
            }
            crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.type_reference_names_attached_data(*base_type, attached_data_symbol)
            }
            crate::types::TypeReferenceNode::Named { symbol, name } => {
                *symbol == attached_data_symbol
                    || (name.as_str() == "Self" && attached_data_symbol.is_valid())
            }
            _ => false,
        }
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
    ) -> Option<&crate::state::State> {
        self.machine_holding_state(state_symbol)
            .and_then(|machine| {
                self.machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == state_symbol)
            })
    }

    pub fn push_state_parameter(
        &mut self,
        state: &mut crate::state::State,
        parameter: signature::StateParameter,
    ) {
        self.state_parameters
            .append_to_span(&mut state.parameters, parameter);
    }

    #[inline]
    pub fn state_parameters(&self, state: &crate::state::State) -> &[signature::StateParameter] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn push_state_contract(
        &mut self,
        state: &mut crate::state::State,
        contract: signature::SignatureContract,
    ) {
        self.signature_contracts
            .append_to_span(&mut state.contracts, contract);
    }

    #[inline]
    pub fn state_contracts(&self, state: &crate::state::State) -> &[signature::SignatureContract] {
        self.signature_contracts.span_or_empty(state.contracts)
    }

    /// The top-level statement of `state` that declares local `symbol`: its
    /// ordinal in `state.statement_nodes` and its `LocalData` node. Nested
    /// blocks and state parameters are not searched.
    ///
    /// Resolution gives every top-level `LocalData` its own child symbol, so
    /// a local symbol is declared once. Two declarations of one symbol are
    /// malformed statements; the query answers None for them rather than
    /// choose one, because callers build proof facts on the declaration it
    /// returns. A caller that asks "declared before statement N" compares the
    /// returned ordinal with N.
    pub fn state_local_declaration(
        &self,
        state: &crate::state::State,
        symbol: symbols::SymbolHandle,
    ) -> Option<(usize, &crate::statement::TableLocalData)> {
        let mut declarations = self
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                crate::statement::StatementNode::LocalData(local) if local.symbol == symbol => {
                    Some((ordinal, local))
                }
                _ => None,
            });
        let declaration = declarations.next()?;
        declarations.next().is_none().then_some(declaration)
    }

    /// The authored ordinal of the scalar local a `let` at `statement_index`
    /// declares: how many immutable primitive locals the state declares
    /// before it. Source roles (`LocalInitializer`, `CallArgument`) key a
    /// `let` by this ordinal. It differs from the dense binding ordinal a
    /// plan assigns, which also counts the call results of preceding field
    /// stores; a consumer holding a plan's binding ordinal converts through
    /// this count before it looks up a source role.
    pub fn immutable_scalar_locals_before(
        &self,
        state: &crate::state::State,
        statement_index: usize,
    ) -> usize {
        self.statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(statement_index)
            .filter(|statement| {
                matches!(statement, crate::statement::StatementNode::LocalData(local)
                    if !local.is_mutable
                        && local.initial_value.is_valid()
                        && self.primitive_type_reference(local.type_reference).is_some())
            })
            .count()
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

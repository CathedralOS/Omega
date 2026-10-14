//! The trait, conformance, boundary calling plan, machine and state tables.

use crate::typed_trees::TypedTrees;
use crate::{data, machine, signature, trait_definition};
use symbols::SymbolHandle;

impl TypedTrees {
    super::typed_tree_table! {
        traits => traits(trait_definition::TraitDefinition) [push_trait_definition];
        conformances => conformances(trait_definition::Conformance) [push_conformance];
        machines => machines(machine::Machine) [push_machine], inline;
    }

    /// The trait definition `symbol` declares, if any.
    pub fn trait_definition_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&trait_definition::TraitDefinition> {
        self.traits()
            .iter()
            .find(|definition| definition.symbol == symbol)
    }

    /// The conformance `symbol` declares, if any.
    pub fn conformance_by_symbol(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<&trait_definition::Conformance> {
        self.conformances()
            .iter()
            .find(|conformance| conformance.symbol == symbol)
    }

    pub fn conformance_type_parameters(
        &self,
        conformance: &trait_definition::Conformance,
    ) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(conformance.type_parameters)
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

    /// Executable coordinates of the complete authored requirement roster.
    /// A closed family replaces its open template here, not in the declaration
    /// table used to check conformance rows. Uncalled tuples remain coordinates.
    pub fn trait_machine_instances<'program>(
        &'program self,
        trait_definition: &'program trait_definition::TraitDefinition,
    ) -> impl Iterator<Item = &'program signature::StateSignature> {
        self.trait_machine_signatures(trait_definition)
            .iter()
            .flat_map(|signature| self.requirement_machine_instances(signature))
    }

    /// Expand one authored coordinate without turning provider visibility into
    /// application evidence. An already closed coordinate remains itself.
    pub fn requirement_machine_instances<'program>(
        &'program self,
        signature: &'program signature::StateSignature,
    ) -> impl Iterator<Item = &'program signature::StateSignature> {
        std::iter::once(signature)
            .filter(|signature| {
                !self
                    .closed_requirement_applications()
                    .any(|application| application.template == signature.symbol)
            })
            .chain(
                self.closed_requirement_applications()
                    .filter(move |application| application.template == signature.symbol)
                    .map(|application| application.signature),
            )
    }

    /// The machine signature keyed `symbol` within `trait_definition`, when declared.
    pub fn trait_machine_signature_by_symbol(
        &self,
        trait_definition: &trait_definition::TraitDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        self.trait_machine_signatures(trait_definition)
            .iter()
            .find(|candidate| candidate.symbol == symbol)
            .or_else(|| {
                self.closed_requirement_application(symbol)
                    .filter(|application| application.declaring_trait == trait_definition.symbol)
                    .map(|application| application.signature)
            })
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
                instantiated,
                ..
            } => {
                let trait_definition = self.trait_definition_by_symbol(*trait_definition)?;
                let declared =
                    self.trait_machine_signature_by_symbol(trait_definition, *requirement)?;
                Some(data::MachineParameterContractView::Nominal {
                    trait_definition,
                    requirement: instantiated.as_deref().unwrap_or(declared),
                })
            }
        }
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

    /// The `(machine, state)` pairs whose state symbol is `symbol` —
    /// normally at most one; callers decide how duplicates behave.
    pub fn machine_states_named(
        &self,
        symbol: SymbolHandle,
    ) -> impl Iterator<Item = (&machine::Machine, &crate::state::State)> + '_ {
        self.machines().iter().flat_map(move |machine| {
            self.machine_states(machine)
                .iter()
                .filter(move |state| state.symbol == symbol)
                .map(move |state| (machine, state))
        })
    }

    /// Whether `symbol` is `machine` itself or one of its states.
    pub fn machine_or_state_symbol(
        &self,
        machine: &machine::Machine,
        symbol: SymbolHandle,
    ) -> bool {
        machine.symbol == symbol
            || self
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    }

    /// The machine signature `symbol` declared by any trait, with its
    /// declaring trait.
    pub fn trait_machine_signature(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(
        &trait_definition::TraitDefinition,
        &signature::StateSignature,
    )> {
        self.traits().iter().find_map(|trait_definition| {
            self.trait_machine_signature_by_symbol(trait_definition, symbol)
                .map(|signature| (trait_definition, signature))
        })
    }

    /// The machine `symbol` declares, if any.
    pub fn machine(&self, symbol: symbols::SymbolHandle) -> Option<&machine::Machine> {
        // Both lookup spellings share the owner-revision index. A roster
        // scan here made structural proof normalization repeat whole-program
        // searches despite the declaration owner already retaining this join.
        self.machine_by_symbol(symbol)
    }

    /// The machine spelled `name`, if any.
    pub fn machine_by_name_str(&self, name: &str) -> Option<&machine::Machine> {
        self.machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
    }

    /// The state `state_symbol` of machine `machine_symbol`, or `None` when
    /// either symbol names no declared machine or state.
    pub fn machine_state(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<(&machine::Machine, &crate::state::State)> {
        let machine = self.machine(machine_symbol)?;
        let state = crate::state::state_by_symbol(self.machine_states(machine), state_symbol)?;
        Some((machine, state))
    }

    #[inline]
    pub fn machines_mut(&mut self) -> &mut [machine::Machine] {
        self.tables.machines.span_mut_or_empty(self.roots.machines)
    }

    #[inline]
    pub fn machine_type_parameters(&self, machine: &machine::Machine) -> &[data::TypeParameter] {
        self.data_type_parameters
            .span_or_empty(machine.type_parameters)
    }

    /// The `Machine`-kind entries within `machine`'s type parameters — the
    /// positional trait binders that select implementations, never stored
    /// types.
    pub fn machine_kind_type_parameters<'a>(
        &'a self,
        machine: &'a machine::Machine,
    ) -> impl Iterator<Item = &'a data::TypeParameter> + 'a {
        self.machine_type_parameters(machine)
            .iter()
            .filter(|parameter| matches!(parameter.kind, data::TypeParameterKind::Machine { .. }))
    }

    /// The declaration scope visible to types in a machine, including source
    /// regions captured in a specialization's actual type operands. Captures
    /// belong to the retained application's lexical owner, not new binders on
    /// the private clone. This query admits their spelling during formation;
    /// applied identity still resolves regions through that retained owner.
    pub fn machine_type_reference_scope(
        &self,
        machine: &machine::Machine,
        same_type_parameter: impl Fn(&data::TypeParameter, &data::TypeParameter) -> bool,
    ) -> (Vec<data::TypeParameter>, Vec<crate::name::Identifier>) {
        let mut type_parameters = self.machine_type_parameters(machine).to_vec();
        let mut lifetime_parameters = machine.lifetime_parameters.clone();
        // A named map's owned row captures its public regions. They are
        // visible during formation, not fresh binders of each invocation.
        let mut enclosing = self.declaration_scope_parent(machine.symbol);
        while enclosing.is_valid() {
            if let Some(conformance) = self.conformance_by_symbol(enclosing) {
                for parameter in self.conformance_type_parameters(conformance) {
                    if !type_parameters
                        .iter()
                        .any(|existing| same_type_parameter(existing, parameter))
                    {
                        type_parameters.push(parameter.clone());
                    }
                }
                for parameter in &conformance.lifetime_parameters {
                    if !lifetime_parameters.contains(parameter) {
                        lifetime_parameters.push(parameter.clone());
                    }
                }
            }
            enclosing = self.declaration_scope_parent(enclosing);
        }
        if let Some(attached_data) = &machine.attached_data
            && let Some(definition) = self.data_definition_by_name(attached_data)
        {
            for parameter in self.data_type_parameters(definition) {
                if !type_parameters
                    .iter()
                    .any(|existing| same_type_parameter(existing, parameter))
                {
                    type_parameters.push(parameter.clone());
                }
            }
            for parameter in &definition.lifetime_parameters {
                if !lifetime_parameters
                    .iter()
                    .any(|existing| existing == parameter)
                {
                    lifetime_parameters.push(parameter.clone());
                }
            }
        }
        // A type operand can carry regions arbitrarily deep in its arguments.
        // The nominal head's lifetime roster is not the operand's scope. Follow
        // retained source ownership rather than collecting tags from type text
        // (which would admit undeclared regions). The roster bounds traversal
        // even when a recursive specialization refers to its own application.
        let mut owner = machine.symbol;
        for _ in 0..self.machine_specializations.len() {
            let Some(specialization) = self
                .machine_specializations
                .iter()
                .find(|specialization| specialization.instance == owner)
            else {
                break;
            };
            if !specialization.binding_application.is_valid() {
                break;
            }
            let lexical_owner = self
                .expression_table
                .static_machine_application(specialization.binding_application)
                .lexical_owner;
            if lexical_owner == owner {
                break;
            }
            let captured = self
                .machine_by_symbol(lexical_owner)
                .map(|machine| &machine.lifetime_parameters)
                .or_else(|| {
                    self.data_definition_by_symbol(lexical_owner)
                        .map(|definition| &definition.lifetime_parameters)
                })
                .or_else(|| {
                    self.conformance_by_symbol(lexical_owner)
                        .map(|conformance| &conformance.lifetime_parameters)
                });
            if let Some(captured) = captured {
                for parameter in captured {
                    if !lifetime_parameters.contains(parameter) {
                        lifetime_parameters.push(parameter.clone());
                    }
                }
            }
            owner = lexical_owner;
        }
        (type_parameters, lifetime_parameters)
    }

    /// The authored callable contract of a compile-time machine parameter in
    /// `machine`. The parameter symbol is also the contract's call-target
    /// identity until specialization replaces it with a concrete state.
    pub fn machine_parameter_signature_in(
        &self,
        machine: &machine::Machine,
        symbol: SymbolHandle,
    ) -> Option<&signature::StateSignature> {
        let own = self.machine_type_parameters(machine);
        let mut parameters = own;
        let mut enclosing = machine.symbol;
        let mut visited = Vec::new();
        loop {
            if let Some(parameter) = parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return match &parameter.kind {
                    data::TypeParameterKind::Machine { contract } => self
                        .machine_parameter_contract_view(contract)
                        .map(data::MachineParameterContractView::signature),
                    _ => None,
                };
            }
            enclosing = self.declaration_scope_parent(enclosing);
            if !enclosing.is_valid() || visited.contains(&enclosing) {
                return None;
            }
            visited.push(enclosing);
            if let Some(map) = self.conformance_by_symbol(enclosing) {
                parameters = self.conformance_type_parameters(map);
                if parameters.iter().any(|parameter| {
                    parameter.symbol == symbol
                        && own.iter().any(|local| local.name == parameter.name)
                }) {
                    return None;
                }
            } else {
                parameters = &[];
            }
        }
    }

    /// Rejoin the original binder contract after specialization removed its
    /// live generic parameter span. This is a contract lookup, not authority:
    /// callers must also validate the specialization's binder/selection join.
    pub fn retained_static_machine_contract(
        &self,
        symbol: SymbolHandle,
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
        symbol: SymbolHandle,
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

    /// The owned-data row keyed `symbol` within `machine`, when declared.
    pub fn machine_owned_data_by_symbol(
        &self,
        machine: &machine::Machine,
        symbol: symbols::SymbolHandle,
    ) -> Option<&machine::OwnedData> {
        self.machine_owned_data(machine)
            .iter()
            .find(|candidate| candidate.symbol == symbol)
    }

    pub fn push_machine_trait_conformance(
        &mut self,
        machine: &mut machine::Machine,
        conformance: machine::TraitConformance,
    ) {
        self.machine_trait_conformances
            .append_to_span(&mut machine.satisfies, conformance);
    }

    /// Every conformance this declaration owes, including an unrealized body.
    ///
    /// Declaration queries must not observe realization selection: checking,
    /// inherited proof facts, cloning and lowering all need the same authored
    /// obligations. A caller choosing implementations owns its execution scope;
    /// a target marker never erases semantic evidence from its member.
    pub fn machine_trait_conformances(
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

    /// Visits each (machine, state) contract frame in program order: the
    /// state's own contracts, extended by the machine's contracts at state 0,
    /// and the diagnostic label (the bare machine name at state 0,
    /// `machine::state` otherwise).
    pub fn for_each_machine_state_contract_frame(
        &self,
        mut visit: impl FnMut(
            &machine::Machine,
            &crate::state::State,
            Vec<&signature::SignatureContract>,
            String,
        ),
    ) {
        for machine in self.machines() {
            for (state_index, state) in self.machine_states(machine).iter().enumerate() {
                let mut contracts = self.state_contracts(state).iter().collect::<Vec<_>>();
                if state_index == 0 {
                    contracts.extend(self.machine_contracts(machine));
                }
                let label = if state_index == 0 {
                    machine.name.to_string()
                } else {
                    format!("{}::{}", machine.name, state.name)
                };
                visit(machine, state, contracts, label);
            }
        }
    }

    /// The proof facts of every `kind` contract in `contracts`, in
    /// declaration order.
    pub fn contract_facts<'a>(
        &'a self,
        contracts: impl Iterator<Item = &'a signature::SignatureContract> + 'a,
        kind: signature::SignatureContractKind,
    ) -> impl Iterator<Item = &'a crate::domain::ProofFact> + 'a {
        contracts
            .filter(move |contract| contract.kind == kind)
            .flat_map(|contract| self.proof_facts.span_or_empty(contract.facts))
    }

    /// The `ProofFact::Expression` bodies of every `kind` contract in
    /// `contracts` — the premise vocabulary a contract tier carries.
    pub fn contract_expressions<'a>(
        &'a self,
        contracts: impl Iterator<Item = &'a signature::SignatureContract> + 'a,
        kind: signature::SignatureContractKind,
    ) -> impl Iterator<Item = crate::expression::ExpressionHandle> + 'a {
        self.contract_facts(contracts, kind)
            .filter_map(|fact| match fact {
                crate::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
    }

    /// Whether the machine's crash interface is its published ceiling rather
    /// than an inferred body summary. A public or non-checked-body supply
    /// publishes one even with no `crashes` clause (omitting a cause forbids
    /// it); a private checked body publishes one exactly when it authors a
    /// `crashes` clause. Body crash sites must then fit those routes.
    pub fn machine_publishes_crash_ceiling(&self, machine: &machine::Machine) -> bool {
        machine.is_public
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || self.machine_contracts(machine).iter().any(|contract| {
                matches!(
                    contract.kind,
                    signature::SignatureContractKind::Crashes { .. }
                )
            })
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

    /// The state `symbol` of `machine`, if the machine declares one.
    /// The machine and state a flow fact's machine and state symbols name,
    /// or `None` for a dangling pair.
    pub fn state_flow_site(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<(&machine::Machine, &crate::state::State)> {
        let machine = self.machine_by_symbol(machine_symbol)?;
        let state = self.state_in_machine(machine, state_symbol)?;
        Some((machine, state))
    }

    pub fn state_in_machine(
        &self,
        machine: &machine::Machine,
        symbol: SymbolHandle,
    ) -> Option<&crate::state::State> {
        crate::state::state_by_symbol(self.machine_states(machine), symbol)
    }

    #[inline]
    pub fn machine_states_mut(&mut self, machine: &machine::Machine) -> &mut [crate::state::State] {
        self.machine_states.span_mut_or_empty(machine.states)
    }

    /// The parameter of `parameters` that carries the machine's data record.
    ///
    /// An explicit parameter names the data directly (`main: &mut Main`). The
    /// attached data's symbol identifies the carrier, never an implicit argument.
    /// A rendered place is data-rooted when its head names this parameter.
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
        self.type_reference_names_attached_data(
            parameter.type_reference,
            machine.attached_data_symbol,
        )
    }

    /// The position of the one parameter carrying `attached_data_symbol`'s
    /// record, by the same rule as `data_binding_parameter`: the single
    /// parameter declaring that data, and `None` when two could be.
    ///
    /// Callers that hold the attachment without the machine record ask here.
    pub fn data_binding_position(
        &self,
        attached_data_symbol: SymbolHandle,
        parameters: &[signature::StateParameter],
    ) -> Option<usize> {
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
    /// names, so every mode answers this the same way. An application names
    /// its resolved owner too; its arguments and regions are checked separately
    /// and do not turn an explicit parameter into an implicit receiver.
    pub(crate) fn type_reference_names_attached_data(
        &self,
        type_reference: crate::types::TypeReferenceHandle,
        attached_data_symbol: SymbolHandle,
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
            crate::types::TypeReferenceNode::Named { symbol, .. }
            | crate::types::TypeReferenceNode::Generic {
                base_symbol: symbol,
                ..
            } => *symbol == attached_data_symbol,
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
    pub fn machine_holding_state(&self, state_symbol: SymbolHandle) -> Option<&machine::Machine> {
        if !state_symbol.is_valid() {
            return None;
        }
        let symbol = self.symbols.get(state_symbol);
        let parent = symbol.parent;
        if let Some(machine) = self.machine(parent)
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
    pub fn state_by_symbol(&self, state_symbol: SymbolHandle) -> Option<&crate::state::State> {
        self.machine_holding_state(state_symbol)
            .and_then(|machine| {
                crate::state::state_by_symbol(self.machine_states(machine), state_symbol)
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

    /// `state`'s parameter carrying `symbol`, if any.
    pub fn state_parameter_by_symbol(
        &self,
        state: &crate::state::State,
        symbol: SymbolHandle,
    ) -> Option<&signature::StateParameter> {
        self.state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
    }

    /// `state`'s runtime-visible parameters — the non-`const` entries a
    /// caller supplies positionally.
    pub fn public_state_parameters<'a>(
        &'a self,
        state: &'a crate::state::State,
    ) -> impl Iterator<Item = &'a signature::StateParameter> + 'a {
        self.state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_const)
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

    /// Whether `machine` itself or any of its states declares a crash
    /// contract: the whole signature roster where an abrupt exit is
    /// admissible for this machine.
    pub fn machine_declares_crash(&self, machine: &machine::Machine) -> bool {
        self.machine_contracts(machine)
            .iter()
            .chain(
                self.machine_states(machine)
                    .iter()
                    .flat_map(|state| self.state_contracts(state)),
            )
            .any(|contract| {
                matches!(
                    contract.kind,
                    signature::SignatureContractKind::Crashes { .. }
                )
            })
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
        symbol: SymbolHandle,
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
    /// before it, including declarations established by zero initialization.
    /// Source roles (`LocalInitializer`, `CallArgument`) key a
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

#[cfg(test)]
mod tests {
    use crate::TypedTrees;
    use crate::machine::Machine;
    use symbols::SymbolHandle;

    #[test]
    fn machine_lookup_retains_first_match_and_refreshes_after_mutation() {
        let original = SymbolHandle::from_parts(1, 1);
        let replacement = SymbolHandle::from_parts(1, 2);
        let mut program = TypedTrees::default();
        program.push_machine(Machine {
            symbol: original,
            ..Machine::default()
        });
        assert_eq!(
            program.machine(original).map(|machine| machine.is_public),
            Some(false)
        );
        assert!(program.machine(replacement).is_none());
        assert!(program.machine(SymbolHandle::invalid()).is_none());

        program.push_machine(Machine {
            symbol: original,
            is_public: true,
            ..Machine::default()
        });
        assert_eq!(
            program.machine(original).map(|machine| machine.is_public),
            Some(false)
        );

        let snapshot = program.clone();
        program.machines_mut()[0].symbol = replacement;
        assert_eq!(
            program.machine(original).map(|machine| machine.is_public),
            Some(true)
        );
        assert_eq!(
            program
                .machine(replacement)
                .map(|machine| machine.is_public),
            Some(false)
        );
        assert_eq!(
            snapshot.machine(original).map(|machine| machine.is_public),
            Some(false)
        );
        assert!(snapshot.machine(replacement).is_none());
    }
}

//! Discover only demanded or selected native promises, retaining their exact
//! source owners. Binding intent is not a checked conformance or an invocation.

use language_core::sealed_intrinsics::SealedIntrinsic;
use language_semantics::{
    ExternalBindingIdentity, MachineSupplyMode, ServiceReachRowId, ServiceReachRowTable,
    TerminationGuarantee,
};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;

use super::{Program, services};
use crate::external_providers::{ExternalProviderSource, ProviderOwnerSource};

impl Program<'_> {
    pub(super) fn select_native_provider(
        &mut self,
        context: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<bool> {
        let Some(machine) = self.typed.machine_by_symbol(symbol) else {
            return Ok(false);
        };
        if machine.body_is_present
            || !matches!(
                machine.supply_mode,
                MachineSupplyMode::Boundary | MachineSupplyMode::ExternalRealization { .. }
            )
        {
            return Ok(false);
        }
        if self.typed.machine_trait_conformances(machine).is_empty() {
            return Ok(false);
        }
        if !self.typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(services::unavailable(
                context,
                "native selection has a stale source declaration",
            ));
        }
        if !self.external_providers.selected.contains(&symbol) {
            self.external_providers.selected.push(symbol);
        }
        self.admitted_declarations.include(symbol);
        Ok(true)
    }

    pub(crate) fn prepare_external_providers(&mut self, context: MachineId) -> Checked<()> {
        if std::mem::replace(&mut self.external_providers.prepared, true) {
            return Err(services::unavailable(
                context,
                "native declaration headers were already prepared",
            ));
        }
        let typed = self.typed;
        let mut demanded = self
            .boundaries
            .rows
            .values()
            .map(|row| row.source)
            .collect::<Vec<_>>();
        demanded.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        demanded.dedup();
        let mut selected = self.external_providers.selected.clone();
        selected.sort_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        selected.dedup();
        let contains = |symbols: &[SymbolHandle], symbol: SymbolHandle| {
            symbols
                .binary_search_by_key(&(symbol.arena_index(), symbol.generation()), |candidate| {
                    (candidate.arena_index(), candidate.generation())
                })
                .is_ok()
        };
        for machine in typed.machines() {
            let chosen = contains(&selected, machine.symbol);
            if machine.body_is_present
                || !matches!(
                    machine.supply_mode,
                    MachineSupplyMode::Boundary | MachineSupplyMode::ExternalRealization { .. }
                )
            {
                continue;
            }
            for edge in typed.machine_trait_conformances(machine) {
                if !chosen && !contains(&demanded, edge.requirement_symbol) {
                    continue;
                }
                let satisfaction = typed
                    .machine_trait_conformances
                    .handle_of(edge)
                    .ok_or_else(|| {
                        services::unavailable(
                            context,
                            "native conformance lost its exact declaration occurrence",
                        )
                    })?;
                if self
                    .external_providers
                    .rows
                    .iter()
                    .any(|row| row.source == machine.symbol && row.satisfaction == satisfaction)
                {
                    continue;
                }
                let mut source = self.native_provider_source(context, machine, edge)?;
                source.boundary = self.boundaries.rows.iter().find_map(|(id, boundary)| {
                    (boundary.native && boundary.source == machine.symbol).then_some(id)
                });
                self.external_providers.rows.push(source);
            }
        }
        for symbol in selected {
            if !self
                .external_providers
                .rows
                .iter()
                .any(|row| row.source == symbol)
            {
                return Err(services::unavailable(
                    context,
                    "selected native declaration supplied no exact checked conformance question",
                ));
            }
        }
        Ok(())
    }
    fn native_provider_source(
        &mut self,
        context: MachineId,
        machine: &Machine,
        edge: &typed_trees::machine::TraitConformance,
    ) -> Checked<ExternalProviderSource> {
        let typed = self.typed;
        if !edge.requirement_symbol.is_valid() || edge.via_expression.is_valid() {
            return Err(services::unavailable(
                context,
                "native conformance has no exact retained requirement or binding",
            ));
        }
        let Some((owner, required)) = typed.trait_machine_signature(edge.requirement_symbol) else {
            return Err(services::unavailable(
                context,
                "native conformance needs its exact supported boundary requirement header",
            ));
        };
        if !owner.is_boundary
            || edge.symbol != owner.symbol
            || !edge.trait_lifetime_arguments.is_empty()
            || !edge.arguments.is_empty()
        {
            return Err(services::unavailable(
                context,
                "native conformance has an unresolved declaring trait application",
            ));
        }
        let (binding, sealed_leaf) = native_binding(
            typed,
            context,
            machine,
            owner.symbol,
            required,
            edge.external_binding,
        )?;
        let Some((requirement, _)) = self.boundary_call(context, required.symbol)? else {
            return Err(services::unavailable(
                context,
                "native conformance requirement lost its boundary declaration",
            ));
        };
        if !machine.type_parameters.is_empty()
            || !machine.lifetime_parameters.is_empty()
            || !machine.conformance_bounds.is_empty()
            || !machine.owned_data.is_empty()
            || !machine.declared_type_equations.is_empty()
            || machine.structural_type_equations_pending
            || machine.lexical_region_origin.is_some()
            || machine.body_status != typed_trees::machine::MachineBodyStatus::Complete
            || machine.purpose != typed_trees::machine::MachinePurpose::Runtime
            || machine.generic_data_template.is_valid()
            || machine.attached_data_application.is_valid()
        {
            return Err(services::unavailable(
                context,
                "native provider needs a complete closed declaration header",
            ));
        }
        let [state] = typed.machine_states(machine) else {
            return Err(services::unavailable(
                context,
                "native provider has no unique exact entry signature",
            ));
        };
        if !state.statement_nodes.is_empty() || !state.contracts.is_empty() {
            return Err(services::unavailable(
                context,
                "native provider header contains a body or separate state contract",
            ));
        }
        let declared = StateSignature {
            symbol: state.symbol,
            name: machine.name.clone(),
            spelling: machine.spelling,
            lifetime_parameters: machine.lifetime_parameters.clone(),
            type_parameters: machine.type_parameters,
            conformance_bounds: machine.conformance_bounds.clone(),
            is_default: false,
            parameters: state.parameters,
            native_callback_parameters: Vec::new(),
            return_type: state.return_type,
            invokes: machine.invokes,
            service_reach_row: machine.service_reach_row,
            service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
            suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts: machine.contracts,
            termination_guarantee: machine
                .termination_plan
                .interface
                .published()
                .cloned()
                .unwrap_or(TerminationGuarantee::NoGuarantee),
            declares_no_crash_route: machine.declares_no_crash_route,
        };
        let mut operational = declared.clone();
        if operational.service_reach_row == ServiceReachRowId::NULL {
            if !sealed_leaf {
                return Err(services::unavailable(
                    context,
                    "native provider has an unresolved service contract",
                ));
            }
            // The authenticated sealed physical leaf has no outgoing
            // Omega service calls. This is its catalog-owned inventory,
            // not an inference about an unknown foreign implementation.
            operational.service_reach_row = ServiceReachRowTable::EMPTY_ROW;
        }
        let mut envelope = self.signature_envelope(context, &operational)?;
        // Every exact native satisfaction contributes its service use to the
        // leaf's one public header. Choosing another conformance cannot change
        // the meaning of a direct call of this source declaration.
        for satisfaction in typed.machine_trait_conformances(machine) {
            let (declaring, requirement) = typed
                .trait_machine_signature(satisfaction.requirement_symbol)
                .ok_or_else(|| {
                    services::unavailable(context, "native service use lost its exact requirement")
                })?;
            native_binding(
                typed,
                context,
                machine,
                declaring.symbol,
                requirement,
                satisfaction.external_binding,
            )?;
            if !crate::reach::has_authored_ceiling(typed, context, requirement.symbol)? {
                return Err(services::unavailable(
                    context,
                    "native conformance requirement has an open service ceiling",
                ));
            }
            let required = self.signature_envelope(context, requirement)?;
            envelope.service_reach.extend(required.service_reach);
        }
        envelope.service_reach.sort_by_key(|service| service.0);
        envelope.service_reach.dedup();
        let (mut signature, _) = self.native_provider_signature(context, &declared)?;
        signature.termination = match &envelope.termination {
            terminal_psi::TerminalCallableTermination::NotGuaranteed => {
                crate::body::Termination::Silent
            }
            terminal_psi::TerminalCallableTermination::Terminates { .. } => {
                crate::body::Termination::Promised
            }
        };
        let provider_symbol =
            typed_trees::provider_declarations::satisfaction_provider_declaration(typed, machine);
        let provider = provider_symbol
            .map(|symbol| -> Checked<ProviderOwnerSource> {
                Ok(ProviderOwnerSource {
                    nominal: symbol,
                    identity: typed
                        .normalized_hermetic_symbol_identity(symbol)
                        .map_err(|_| {
                            services::unavailable(
                                context,
                                "native provider owner has no exact semantic identity",
                            )
                        })?,
                    package: typed.symbols.symbol_package_identity(symbol),
                })
            })
            .transpose()?;
        let callable = typed
            .machine_callable_identity_text(machine)
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                services::unavailable(context, "native provider has no exact callable identity")
            })?;
        let declaration = crate::declare::boundary_source(
            typed,
            machine.symbol,
            provider_symbol.unwrap_or(machine.symbol),
            &declared,
            callable,
        )
        .map_err(|reason| services::unavailable(context, reason))?;
        self.admitted_declarations.include(machine.symbol);
        self.admitted_declarations.include(owner.symbol);
        self.admitted_declarations.include(required.symbol);
        Ok(ExternalProviderSource {
            source: machine.symbol,
            satisfaction: typed
                .machine_trait_conformances
                .handle_of(edge)
                .ok_or_else(|| {
                    services::unavailable(
                        context,
                        "native conformance lost its exact declaration occurrence",
                    )
                })?,
            state: state.symbol,
            requirement,
            signature,
            envelope,
            declaration,
            provider,
            package: typed.symbols.symbol_package_identity(machine.symbol),
            requirement_package: typed.symbols.symbol_package_identity(required.symbol),
            target: typed.machine_authored_target(machine).map(str::to_owned),
            binding,
            boundary: None,
            completion: typed_trees::boundary_completions::boundary_requirement_completion(
                typed,
                machine.symbol,
            ),
        })
    }

    pub(super) fn native_boundary_call(
        &mut self,
        context: MachineId,
        target: SymbolHandle,
    ) -> Checked<Option<(semantic_vocabulary::BoundaryMachineId, crate::Signature)>> {
        let typed = self.typed;
        let Some(machine) = typed.machine_by_own_or_state_symbol(target) else {
            return Ok(None);
        };
        if machine.body_is_present
            || !matches!(
                machine.supply_mode,
                MachineSupplyMode::Boundary | MachineSupplyMode::ExternalRealization { .. }
            )
        {
            return Ok(None);
        }
        self.visible_to(context, machine.symbol)?;
        let target = machine.symbol;
        let Some(edge) = typed.machine_trait_conformances(machine).first() else {
            return Ok(None);
        };
        let source = self.native_provider_source(context, machine, edge)?;
        let signature = source.signature.clone();
        let owner =
            typed_trees::provider_declarations::satisfaction_provider_declaration(typed, machine)
                .unwrap_or(target);
        let boundary = self
            .boundaries
            .rows
            .push(crate::boundaries::BoundarySource {
                source: target,
                owner,
                signature: signature.clone(),
                envelope: source.envelope.clone(),
                completion: source.completion,
                owning_service: None,
                declaration: source.declaration.clone(),
                kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
                native: true,
            });
        let mut source = source;
        source.boundary = Some(boundary);
        if let Some(existing) = self
            .external_providers
            .rows
            .iter_mut()
            .find(|row| row.source == target && row.satisfaction == source.satisfaction)
        {
            existing.boundary = Some(boundary);
        } else {
            self.external_providers.rows.push(source);
        }
        if !self.external_providers.selected.contains(&target) {
            self.external_providers.selected.push(target);
        }
        Ok(Some((boundary, signature)))
    }
}

fn native_binding(
    typed: &typed_trees::TypedTrees,
    context: MachineId,
    machine: &Machine,
    owner: SymbolHandle,
    required: &StateSignature,
    retained: Option<language_semantics::ExternalBindingId>,
) -> Checked<(ExternalBindingIdentity, bool)> {
    if let MachineSupplyMode::ExternalRealization {
        binding: Some(binding),
        mechanism: Some(mechanism),
    } = machine.supply_mode
    {
        let identity = typed
            .external_bindings
            .identity(binding)
            .filter(|identity| identity.mechanism() == mechanism)
            .ok_or_else(|| {
                services::unavailable(
                    context,
                    "native provider lost its exact retained binding identity",
                )
            })?;
        if retained != Some(binding) {
            return Err(services::unavailable(
                context,
                "native provider and conformance name different bindings",
            ));
        }
        return Ok((identity.clone(), false));
    }
    let realization = typed
        .symbols
        .sealed_declaration(machine.symbol, machine.name.as_str());
    let requirement = typed
        .symbols
        .sealed_member_intrinsic(owner, required.name.as_str());
    let exact = match (realization, requirement) {
        (
            Some(SealedIntrinsic::HostedRealization {
                operation: actual,
                target,
            }),
            Some(SealedIntrinsic::HostedRequirement(expected)),
        ) => actual == expected && typed.machine_authored_target(machine) == Some(target),
        (
            Some(SealedIntrinsic::FixedImageRealization(actual)),
            Some(SealedIntrinsic::FixedImageRequirement(expected)),
        ) => actual == expected,
        _ => false,
    };
    if !exact || retained.is_some() {
        return Err(services::unavailable(
            context,
            "native source declaration has no exact retained binding owner",
        ));
    }
    Ok((ExternalBindingIdentity::CompilerIntrinsic, true))
}

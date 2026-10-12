//! Exact source application of an authentic selected checked-adapter row.
//!
//! Selection owns the original plan, declaration and binding. Source checking
//! owns the concrete requirement-to-provider conformance coordinates. This
//! query joins those owners without electing another body by its signature or
//! changing the plan's public schema. Named family receipts keep their portable
//! source head; Omega alone projects the closed head into the selected member.
//! Physical leaves use their original
//! selected payload and do not gain a source application through this query.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

use crate::{ProviderPlanningContext, SelectedProviderReviewProvenance};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedCheckedRequirementApplication {
    requirement_template: SymbolHandle,
    requirement_instance: SymbolHandle,
    realization_template: SymbolHandle,
    realization_instance: SymbolHandle,
    source_conformance: arena::Handle<checked_trees::typed_trees::machine::TraitConformance>,
    instance_conformance: arena::Handle<checked_trees::typed_trees::machine::TraitConformance>,
    named_conformance: arena::Handle<checked_trees::typed_trees::trait_definition::Conformance>,
    realization_state: SymbolHandle,
}

impl SelectedCheckedRequirementApplication {
    pub const fn requirement_template(&self) -> SymbolHandle {
        self.requirement_template
    }
    pub const fn requirement_instance(&self) -> SymbolHandle {
        self.requirement_instance
    }
    pub const fn realization_template(&self) -> SymbolHandle {
        self.realization_template
    }
    pub const fn realization_instance(&self) -> SymbolHandle {
        self.realization_instance
    }
    /// Original-to-instance source coordinates exist for produced applications.
    /// Direct authored rows retain their separate selected plan provenance.
    pub const fn source_conformance(
        &self,
    ) -> arena::Handle<checked_trees::typed_trees::machine::TraitConformance> {
        self.source_conformance
    }
    pub const fn instance_conformance(
        &self,
    ) -> arena::Handle<checked_trees::typed_trees::machine::TraitConformance> {
        self.instance_conformance
    }
    pub const fn named_conformance(
        &self,
    ) -> arena::Handle<checked_trees::typed_trees::trait_definition::Conformance> {
        self.named_conformance
    }
    pub const fn realization_state(&self) -> SymbolHandle {
        self.realization_state
    }
}

/// One selected source row and its original checking owner. Consumers borrow
/// this view for both direct and produced applications; the selected plan is
/// never reconstructed from an instantiated body or runtime signature.
pub struct SelectedCheckedRequirementRow<'checked> {
    program: &'checked checked_trees::typed_trees::TypedTrees,
    check_facts: &'checked checked_trees::CheckFacts,
    plan: &'checked effects::provider_plan::ProviderPlan,
    requirement_template: SymbolHandle,
    adapter: &'checked checked_trees::typed_trees::machine::Machine,
    source_template: SymbolHandle,
    named_conformance: arena::Handle<checked_trees::typed_trees::trait_definition::Conformance>,
    realization_state: SymbolHandle,
}

pub fn selected_checked_requirement_row<'checked>(
    program: &'checked CheckedTrees,
    selected: &'checked SelectedProviderReviewProvenance,
    row_ordinal: usize,
) -> Result<SelectedCheckedRequirementRow<'checked>, Diagnostic> {
    let requirement_template = *selected
        .provider
        .row_requirements
        .get(row_ordinal)
        .ok_or_else(|| {
            Diagnostic::error("selected checked application lost its required declaration")
        })?;
    let realization_template = *selected
        .provider
        .row_realizations
        .get(row_ordinal)
        .ok_or_else(|| {
            Diagnostic::error("selected checked application lost its realizing declaration")
        })?;
    let source = selected_checked_requirement_row_for_plan(
        program,
        &program.facts,
        &selected.plan,
        row_ordinal,
        requirement_template,
    )?;
    if source.adapter.symbol != realization_template {
        return Err(Diagnostic::error(
            "selected checked application disagrees with its authentic adapter provenance",
        ));
    }
    Ok(source)
}

/// Join the same original plan row for source dispatch and specialization
/// provenance. The supplied requirement is an original declaration coordinate;
/// the selected plan and choosing declaration must independently name it.
pub(crate) fn selected_checked_requirement_row_for_plan<'checked>(
    program: &'checked checked_trees::typed_trees::TypedTrees,
    check_facts: &'checked checked_trees::CheckFacts,
    plan: &'checked effects::provider_plan::ProviderPlan,
    row_ordinal: usize,
    requirement_template: SymbolHandle,
) -> Result<SelectedCheckedRequirementRow<'checked>, Diagnostic> {
    let row = plan.rows.get(row_ordinal).ok_or_else(|| {
        Diagnostic::error("selected checked application lost its original provider plan row")
    })?;
    let context = ProviderPlanningContext::new(program, source::DependencyScope::Product);
    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(
            "selected checked application lost its nominal provider identity",
        ));
    }
    let adapter = crate::exact_checked_adapter(context, plan, row)?;
    if let Some(requirement) = program.machine(requirement_template) {
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || crate::service_schema::from_typed_boundary_requirement(context, requirement).as_ref()
                != Some(&plan.schema)
            || program
                .machine_callable_identity_text(requirement)
                .as_deref()
                != Some(plan.identity_text(row.requirement_identity))
        {
            return Err(Diagnostic::error(
                "selected checked application lost its original machine requirement association",
            ));
        }
    } else if program
        .trait_machine_signature(requirement_template)
        .is_none()
    {
        return Err(Diagnostic::error(
            "selected checked application lost its original requirement",
        ));
    }
    if let Some((definition, requirement)) = program.trait_machine_signature(requirement_template)
        && !crate::provider_trait_requirement_matches(
            context,
            plan,
            row,
            adapter,
            definition.symbol,
            requirement.symbol,
        )
    {
        return Err(Diagnostic::error(
            "selected checked application lost its original trait requirement association",
        ));
    }
    let mut named_conformance = arena::Handle::invalid();
    let mut realization_state = SymbolHandle::invalid();
    let mut source_template = adapter.symbol;
    if let Some(declaration) = crate::provider_identity::named_provider(context, plan) {
        let original = crate::provider_identity::named_provider_row(context, plan, row)
            .ok_or_else(|| {
                Diagnostic::error("selected checked application lost its original named row")
            })?;
        if original.requirement != requirement_template
            || crate::target_machines::named_row_realization(context, original, Some(&plan.target))
                .map_or(true, |realization| realization.symbol != adapter.symbol)
        {
            return Err(Diagnostic::error(
                "selected checked application changed its original named row",
            ));
        }
        named_conformance = program
            .conformances
            .iter()
            .find(|(_, candidate)| candidate.symbol == declaration.symbol)
            .map(|(handle, _)| handle)
            .ok_or_else(|| {
                Diagnostic::error("selected checked application lost its named declaration handle")
            })?;
        realization_state = original.realization_state;
        source_template = original.realization_machine;
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(
            "selected checked application has no original checked source body",
        ));
    }
    Ok(SelectedCheckedRequirementRow {
        program,
        check_facts,
        plan,
        requirement_template,
        adapter,
        source_template,
        named_conformance,
        realization_state,
    })
}

impl SelectedCheckedRequirementRow<'_> {
    pub const fn requirement_template(&self) -> SymbolHandle {
        self.requirement_template
    }

    /// Authored source family retained by the selected row, separate from the
    /// actual target member adapter and its produced executable instance.
    pub const fn source_template(&self) -> SymbolHandle {
        self.source_template
    }

    pub const fn adapter(&self) -> &checked_trees::typed_trees::machine::Machine {
        self.adapter
    }

    /// Original checked receipts own the static tuple, source edge and copied
    /// body. A direct closed declaration needs no manufactured application.
    /// Finite families borrow the independent checked closed-requirement and
    /// realization-tuple receipts. An open template without either producer
    /// contributes no executable application.
    pub fn applications(&self) -> Result<Vec<SelectedCheckedRequirementApplication>, Diagnostic> {
        let mut instances = Vec::new();
        for (_, application) in self.program.instantiated_requirement_realizations.iter() {
            if application.requirement_template == self.requirement_template
                && application.provider_template == self.source_template
                && application.named_conformance == self.named_conformance
                && !instances.contains(&application.requirement_instance)
            {
                instances.push(application.requirement_instance);
            }
        }
        for application in self.program.closed_requirement_applications() {
            if application.template == self.requirement_template
                && self
                    .program
                    .trait_definition_by_symbol(application.declaring_trait)
                    .and_then(|owner| {
                        crate::service_schema::from_typed_instance(
                            ProviderPlanningContext::new(
                                self.program,
                                source::DependencyScope::Product,
                            ),
                            &[],
                            owner,
                            application.trait_arguments,
                        )
                    })
                    .is_some_and(|demand| self.plan.schema.supplies_receiver_schema(&demand))
                && !instances.contains(&application.signature.symbol)
            {
                instances.push(application.signature.symbol);
            }
        }
        // Ordinary top-level machine specialization has its own checked
        // association owner. It does not publish a trait application header.
        // Join only the selected source lineage; another provider's receipt
        // cannot add demand or supply to this plan row.
        if self.program.machine(self.requirement_template).is_some() {
            for row in self.check_facts.provider_closed_requirements.iter() {
                if row.template == self.requirement_template
                    && row.selecting_declaration == row.provider
                    && self.is_source_machine_instance(row.provider)
                    && !instances.contains(&row.closed_requirement)
                {
                    instances.push(row.closed_requirement);
                }
            }
        }
        if std::env::var_os("OMEGA_SOURCE_CONSTANT_INPUT_TRACE").is_some()
            && self.program.machine(self.requirement_template).is_some()
        {
            eprintln!(
                "TRACE_SELECTED_MACHINE_APPLICATION template={:?} source={:?} instances={:?}",
                self.requirement_template, self.source_template, instances
            );
            for row in self
                .check_facts
                .provider_closed_requirements
                .iter()
                .filter(|row| row.template == self.requirement_template)
            {
                eprintln!(
                    "TRACE_SELECTED_MACHINE_APPLICATION_ROW provider={:?} selecting={:?} closed={:?} source_lineage={}",
                    row.provider,
                    row.selecting_declaration,
                    row.closed_requirement,
                    self.is_source_machine_instance(row.provider)
                );
            }
        }
        if self.directly_closed() && !self.adapter.has_specialization_parameters() {
            instances.insert(0, self.requirement_template);
        }
        instances
            .into_iter()
            .map(|instance| self.application(instance))
            .collect()
    }

    fn directly_closed(&self) -> bool {
        if let Some(machine) = self.program.machine(self.requirement_template) {
            !machine.has_specialization_parameters()
        } else if let Some((definition, signature)) = self
            .program
            .trait_machine_signature(self.requirement_template)
        {
            self.program.trait_type_parameters(definition).is_empty()
                && definition.conformance_bounds.is_empty()
                && definition.lifetime_parameters.is_empty()
                && self
                    .program
                    .state_signature_type_parameters(signature)
                    .is_empty()
                && signature.lifetime_parameters.is_empty()
        } else {
            false
        }
    }

    pub fn schema(
        &self,
        application: &SelectedCheckedRequirementApplication,
    ) -> Option<effects::provider_plan::ServiceSchema> {
        if application.requirement_template != self.requirement_template
            || application.realization_template != self.adapter.symbol
        {
            return None;
        }
        crate::service_schema::from_typed_requirement(
            ProviderPlanningContext::new(self.program, source::DependencyScope::Product),
            application.requirement_instance,
        )
    }

    pub fn application(
        &self,
        requirement_instance: SymbolHandle,
    ) -> Result<SelectedCheckedRequirementApplication, Diagnostic> {
        if self
            .program
            .closed_requirement_application(requirement_instance)
            .is_some()
        {
            return self.finite_application(requirement_instance);
        }
        if requirement_instance != self.requirement_template
            && self.program.machine(self.requirement_template).is_some()
        {
            return self.checked_machine_application(requirement_instance);
        }
        let mut source_conformance = arena::Handle::invalid();
        let mut instance_conformance = arena::Handle::invalid();
        let mut realization_state = self.realization_state;
        let realization_instance = if requirement_instance == self.requirement_template {
            if !self.directly_closed() || self.adapter.has_specialization_parameters() {
                return Err(Diagnostic::error(
                    "selected checked application retains an unapplied telescope",
                ));
            }
            self.adapter.symbol
        } else {
            let mut applications = self
                .program
                .instantiated_requirement_realizations
                .iter()
                .filter(|(_, application)| {
                    application.requirement_template == self.requirement_template
                        && application.requirement_instance == requirement_instance
                        && application.provider_template == self.source_template
                        && application.named_conformance == self.named_conformance
                });
            let (_, application) = applications.next().ok_or_else(|| {
                Diagnostic::error(
                    "selected checked application has no retained exact source realization",
                )
            })?;
            if applications.next().is_some() {
                return Err(Diagnostic::error(
                    "selected checked application has competing source realizations",
                ));
            }
            if self.program.machine(self.requirement_template).is_none() {
                let mut headers = self
                    .program
                    .instantiated_requirement_signatures
                    .iter()
                    .filter(|(_, header)| {
                        header.requirement_template == self.requirement_template
                            && self
                                .program
                                .trait_machine_signatures
                                .get(header.signature)
                                .symbol
                                == requirement_instance
                    });
                let (_, header) = headers.next().ok_or_else(|| {
                    Diagnostic::error(
                        "selected checked application lost its owned public requirement header",
                    )
                })?;
                let Some((definition, _)) = self
                    .program
                    .trait_machine_signature(self.requirement_template)
                else {
                    return Err(Diagnostic::error(
                        "selected checked application lost its original requirement owner",
                    ));
                };
                if headers.next().is_some()
                    || !header.binding_application.is_valid()
                    || header.declaring_trait != definition.symbol
                    || !self
                        .program
                        .state_signature_type_parameters(
                            self.program.trait_machine_signatures.get(header.signature),
                        )
                        .is_empty()
                {
                    return Err(Diagnostic::error(
                        "selected checked application has no unique owned requirement tuple",
                    ));
                }
            }
            let implementation = self
                .program
                .machine(application.provider_instance)
                .ok_or_else(|| {
                    Diagnostic::error("selected checked application lost its concrete source body")
                })?;
            if self.named_conformance.is_valid() {
                self.authenticate_named_source(application, implementation)?;
                realization_state = application.realization_state;
            } else {
                self.authenticate_machine_source(application, implementation)?;
                source_conformance = application.conformance;
                instance_conformance = application.instance_conformance;
            }
            // The receipt authenticates the portable source application. Only
            // Omega projects that closed family into an executable member; no
            // selected body contributes facts to Psi's public head contract.
            let implementation = crate::target_machines::realized_machine(
                self.program,
                implementation,
                if self.plan.target.is_empty() {
                    target::TargetProfile::host()
                } else {
                    target::TargetProfile::from_canonical_target_name(&self.plan.target).map_err(
                        |_| Diagnostic::error("selected checked application has no target"),
                    )?
                },
            )
            .ok_or_else(|| {
                Diagnostic::error("selected checked application has no target-family member")
            })?;
            if implementation.has_specialization_parameters()
                || !implementation.supplies_checked_body()
                || self
                    .check_facts
                    .contract_plans
                    .for_machine(implementation.symbol)
                    .is_none()
            {
                return Err(Diagnostic::error(
                    "selected checked application has no complete checked body contract",
                ));
            }
            implementation.symbol
        };
        Ok(SelectedCheckedRequirementApplication {
            requirement_template: self.requirement_template,
            requirement_instance,
            realization_template: self.adapter.symbol,
            realization_instance,
            source_conformance,
            instance_conformance,
            named_conformance: self.named_conformance,
            realization_state,
        })
    }

    fn is_source_machine_instance(&self, instance: SymbolHandle) -> bool {
        let mut receipts = self
            .program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == instance);
        receipts.next().is_some_and(|receipt| {
            receipt.template == self.source_template && receipt.binding_application.is_valid()
        }) && receipts.next().is_none()
    }

    /// Consume checking's exact closed machine/refinement association. This
    /// is the ordinary machine receipt class beside public trait applications;
    /// its actual tuple was already judged by the checking producer.
    fn checked_machine_application(
        &self,
        requirement_instance: SymbolHandle,
    ) -> Result<SelectedCheckedRequirementApplication, Diagnostic> {
        let failure = || {
            Diagnostic::error(
                "selected checked machine application lost its exact source association",
            )
        };
        if self.named_conformance.is_valid() {
            return Err(failure());
        }
        let mut rows = self
            .check_facts
            .provider_closed_requirements
            .iter()
            .filter(|row| {
                row.template == self.requirement_template
                    && row.closed_requirement == requirement_instance
                    && row.selecting_declaration == row.provider
                    && self.is_source_machine_instance(row.provider)
            });
        let row = rows.next().ok_or_else(failure)?;
        if rows.next().is_some()
            || row.receiving_application.is_valid()
            || self
                .check_facts
                .provider_closed_requirements
                .receiving_regions(row)
                .is_none_or(|regions| !regions.is_empty())
        {
            return Err(failure());
        }
        let mut requirements = self
            .program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == requirement_instance);
        if !requirements.next().is_some_and(|receipt| {
            receipt.template == self.requirement_template && receipt.binding_application.is_valid()
        }) || requirements.next().is_some()
        {
            return Err(failure());
        }
        let requirement = self
            .program
            .machine(requirement_instance)
            .ok_or_else(failure)?;
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || requirement.has_specialization_parameters()
        {
            return Err(failure());
        }
        let source = self
            .program
            .machine(self.source_template)
            .ok_or_else(failure)?;
        let implementation = self.program.machine(row.provider).ok_or_else(failure)?;
        let source_clause = |machine: &checked_trees::typed_trees::machine::Machine| {
            let mut clauses = self
                .program
                .machine_trait_conformances(machine)
                .iter()
                .enumerate()
                .filter(|(_, clause)| {
                    clause.symbol == self.requirement_template
                        && clause.requirement_symbol == self.requirement_template
                        && clause.external_binding.is_none()
                });
            let (ordinal, _) = clauses.next()?;
            if clauses.next().is_some() {
                return None;
            }
            Some(machine.satisfies.handle_at(u32::try_from(ordinal).ok()?))
        };
        let source_conformance = source_clause(source).ok_or_else(failure)?;
        let instance_conformance = source_clause(implementation).ok_or_else(failure)?;
        let implementation = crate::target_machines::realized_machine(
            self.program,
            implementation,
            if self.plan.target.is_empty() {
                target::TargetProfile::host()
            } else {
                target::TargetProfile::from_canonical_target_name(&self.plan.target)
                    .map_err(|_| failure())?
            },
        )
        .ok_or_else(failure)?;
        if implementation.has_specialization_parameters()
            || !implementation.supplies_checked_body()
            || self
                .check_facts
                .contract_plans
                .for_machine(implementation.symbol)
                .is_none()
        {
            return Err(failure());
        }
        Ok(SelectedCheckedRequirementApplication {
            requirement_template: self.requirement_template,
            requirement_instance,
            realization_template: self.adapter.symbol,
            realization_instance: implementation.symbol,
            source_conformance,
            instance_conformance,
            named_conformance: Default::default(),
            realization_state: Default::default(),
        })
    }

    /// A finite family has declaration-owned tuple custody, not a demanded
    /// original-to-instance satisfaction row. Reuse its checked provider fact
    /// and exact realization specialization; no synthetic edge is published.
    fn finite_application(
        &self,
        requirement_instance: SymbolHandle,
    ) -> Result<SelectedCheckedRequirementApplication, Diagnostic> {
        let application = self
            .program
            .closed_requirement_application(requirement_instance)
            .ok_or_else(|| {
                Diagnostic::error("selected finite application lost its public owner")
            })?;
        if application.template != self.requirement_template {
            return Err(Diagnostic::error(
                "selected finite application names another requirement",
            ));
        }
        let selecting_declaration = if self.named_conformance.is_valid() {
            Some(self.program.conformances.get(self.named_conformance).symbol)
        } else {
            None
        };
        let mut facts = self
            .check_facts
            .provider_closed_requirements
            .iter()
            .filter(|fact| {
                fact.template == self.requirement_template
                    && fact.closed_requirement == requirement_instance
                    && fact.receiving_application == application.trait_application_identity
                    && fact.selecting_declaration == selecting_declaration.unwrap_or(fact.provider)
                    && self.program.requirement_realization_matches(
                        requirement_instance,
                        self.adapter.symbol,
                        fact.provider,
                    )
            });
        let fact = facts.next().ok_or_else(|| {
            Diagnostic::error("selected finite application has no exact checked realization")
        })?;
        if facts.next().is_some() {
            return Err(Diagnostic::error(
                "selected finite application has competing realizations",
            ));
        }
        let implementation = self.program.machine(fact.provider).ok_or_else(|| {
            Diagnostic::error("selected finite application lost its concrete source body")
        })?;
        if implementation.has_specialization_parameters()
            || !implementation.supplies_checked_body()
            || self
                .check_facts
                .contract_plans
                .for_machine(implementation.symbol)
                .is_none()
        {
            return Err(Diagnostic::error(
                "selected finite application has no complete checked body contract",
            ));
        }
        Ok(SelectedCheckedRequirementApplication {
            requirement_template: self.requirement_template,
            requirement_instance,
            realization_template: self.adapter.symbol,
            realization_instance: implementation.symbol,
            source_conformance: Default::default(),
            instance_conformance: Default::default(),
            named_conformance: self.named_conformance,
            realization_state: self.realization_state,
        })
    }

    fn authenticate_machine_source(
        &self,
        application: &checked_trees::typed_trees::static_applications::InstantiatedRequirementRealization,
        implementation: &checked_trees::typed_trees::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let original_edge = self
            .program
            .machine_trait_conformances(self.adapter)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| {
                self.adapter.satisfies.handle_at(*ordinal as u32) == application.conformance
            })
            .map(|(_, edge)| edge);
        let copied_edge = self
            .program
            .machine_trait_conformances(implementation)
            .iter()
            .enumerate()
            .find(|(ordinal, _)| {
                implementation.satisfies.handle_at(*ordinal as u32)
                    == application.instance_conformance
            })
            .map(|(_, edge)| edge);
        if !application.conformance.is_valid()
            || !application.instance_conformance.is_valid()
            || application.named_conformance.is_valid()
            || application.realization_state.is_valid()
            || original_edge.is_none()
            || original_edge != copied_edge
            || original_edge.is_none_or(|edge| edge.requirement_symbol != self.requirement_template)
        {
            return Err(Diagnostic::error(
                "selected checked application lost its original-to-instance conformance coordinate",
            ));
        }
        Ok(())
    }

    fn authenticate_named_source(
        &self,
        application: &checked_trees::typed_trees::static_applications::InstantiatedRequirementRealization,
        implementation: &checked_trees::typed_trees::machine::Machine,
    ) -> Result<(), Diagnostic> {
        let failure = || {
            Diagnostic::error(
                "selected checked application lost its original-to-instance named coordinate",
            )
        };
        if application.conformance.is_valid() || application.instance_conformance.is_valid() {
            return Err(failure());
        }
        let declaration = self
            .program
            .conformances
            .iter()
            .find(|(handle, _)| *handle == self.named_conformance)
            .map(|(_, declaration)| declaration)
            .ok_or_else(failure)?;
        let (owner, _) = self
            .program
            .trait_machine_signature(self.requirement_template)
            .ok_or_else(failure)?;
        let mut rows = declaration.rows.iter().filter(|row| {
            row.declaring_trait == owner.symbol && row.requirement == self.requirement_template
        });
        if !rows.next().is_some_and(|row| {
            crate::target_machines::named_row_realization(
                ProviderPlanningContext::new(self.program, source::DependencyScope::Product),
                row,
                Some(&self.plan.target),
            )
            .is_ok_and(|realization| realization.symbol == self.adapter.symbol)
                && row.realization_machine == application.provider_template
                && row.realization_state == application.realization_state
        }) || rows.next().is_some()
        {
            return Err(failure());
        }
        let source = self
            .program
            .machine(self.source_template)
            .ok_or_else(failure)?;
        if source.symbol != implementation.symbol {
            let mut receipts = self
                .program
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == implementation.symbol);
            if !receipts.next().is_some_and(|receipt| {
                receipt.template == source.symbol && receipt.binding_application.is_valid()
            }) || receipts.next().is_some()
                || self.program.machine_states(source).len()
                    != self.program.machine_states(implementation).len()
            {
                return Err(failure());
            }
        }
        Ok(())
    }
}

pub fn selected_checked_requirement_application(
    program: &CheckedTrees,
    selected: &SelectedProviderReviewProvenance,
    row_ordinal: usize,
    requirement_instance: SymbolHandle,
) -> Result<SelectedCheckedRequirementApplication, Diagnostic> {
    selected_checked_requirement_row(program, selected, row_ordinal)?
        .application(requirement_instance)
}

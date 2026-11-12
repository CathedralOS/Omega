/*
Companion projections produced alongside lowered machines: `TerminalProgramRoots`, external
provider declarations (`project_external_provider_declarations`), lowered task-activation demands
(`lower_task_activation_demands`), and diagnostic machine provenance (`lowered_module_provenance`).
*/

pub(crate) mod external_provider_declarations {

    use checked_trees::CheckedTrees;
    use std::sync::Arc;
    use symbols::SymbolHandle;
    use terminal_psi::{
        ExternalProviderConformance, NominalDeclarationIdentity, TerminalIdentityTable,
    };

    pub(crate) mod boundary_review {

        use checked_trees::{
            CheckFacts, CheckedBoundaryOperatorApplicationUseSite, CheckedProviderRequirementKind,
        };
        use language_semantics::CheckedBodyCandidate;
        use language_semantics::boundary_provider_review::{
            BoundaryApplicationEvidenceError, BoundaryOperatorApplication,
            BoundaryOperatorDeclaration, BoundaryOperatorUse, BoundaryProviderReview,
            BoundaryRequirementReview, BoundaryRequirementUse,
        };

        pub(crate) fn prepare<'facts>(
            facts: &'facts CheckFacts,
            external: &'facts super::ExternalProviderDeclarations,
        ) -> BoundaryProviderReview<'facts> {
            let requirements = facts
                .operators
                .provider_requirements
                .iter()
                .map(|(_, requirement)| {
                    (
                        requirement,
                        BoundaryRequirementReview {
                            schema_identity: &requirement.schema_identity,
                            package_identity: requirement.package_identity,
                            numeric_requirement: numeric_requirement(external, requirement.symbol),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let declarations = requirements
                .iter()
                .filter_map(|(source, requirement)| {
                    if !matches!(
                        source.kind,
                        CheckedProviderRequirementKind::Operator {
                            is_product_boundary: true
                        }
                    ) {
                        return None;
                    }
                    let bodies = facts
                        .operators
                        .boundary_declarations
                        .iter()
                        .filter(|(_, declaration)| declaration.requirement == source.symbol)
                        .map(|(_, declaration)| CheckedBodyCandidate {
                            body: (),
                            declaration: &declaration.selection,
                            target: declaration.selection.target.as_deref(),
                            is_product: declaration.selection.is_product,
                        })
                        .collect();
                    Some(BoundaryOperatorDeclaration {
                        requirement: *requirement,
                        bodies,
                    })
                })
                .collect();

            let spelled = facts.operators.uses.iter().map(|(_, usage)| {
                (
                    usage.selected_operator_symbol,
                    usage.application_site(),
                    usage.is_generic_template,
                )
            });
            let named = facts.operators.named_uses.iter().map(|(_, usage)| {
                (
                    usage.selected_operator_symbol,
                    CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression: usage.expression,
                        origin: usage.origin,
                    },
                    usage.is_generic_template,
                )
            });
            let operator_uses = spelled
                .chain(named)
                .filter_map(|(symbol, site, template)| {
                    let (_, requirement) = requirements.iter().find(|(source, _)| {
                        source.symbol == symbol
                            && matches!(
                                source.kind,
                                CheckedProviderRequirementKind::Operator { .. }
                            )
                    })?;
                    let application = if template {
                        BoundaryOperatorApplication::GenericTemplate
                    } else {
                        BoundaryOperatorApplication::Closed(application_bodies(facts, symbol, site))
                    };
                    Some(BoundaryOperatorUse {
                        requirement: *requirement,
                        application,
                    })
                })
                .collect();

            let requirement_uses = facts
                .operators
                .named_requirement_uses
                .iter()
                .filter_map(|(_, usage)| {
                    let (source, requirement) = requirements.iter().find(|(source, _)| {
                        source.symbol == usage.requirement_symbol
                            && matches!(
                                source.kind,
                                CheckedProviderRequirementKind::DirectCall { .. }
                            )
                    })?;
                    let CheckedProviderRequirementKind::DirectCall { diagnostic_name } =
                        &source.kind
                    else {
                        return None;
                    };
                    Some(BoundaryRequirementUse {
                        requirement: *requirement,
                        diagnostic_name,
                    })
                })
                .collect();
            BoundaryProviderReview {
                declarations,
                operator_uses,
                requirement_uses,
            }
        }

        fn numeric_requirement(
            external: &super::ExternalProviderDeclarations,
            symbol: symbols::SymbolHandle,
        ) -> Result<&str, &'static str> {
            let mut declarations = external
                .correspondence()
                .iter()
                .filter(|(source, _)| *source == symbol);
            let (_, declaration) = declarations
                .next()
                .ok_or("numeric requirement has no checked nominal declaration")?;
            if declarations.next().is_some() {
                return Err("numeric requirement has competing nominal declarations");
            }
            let declaration = declaration.as_ref().map_err(|reason| *reason)?;
            external
                .identities
                .text(declaration.identity())
                .ok_or("numeric requirement identity is absent from its checked catalog")
        }

        fn application_bodies(
            facts: &CheckFacts,
            symbol: symbols::SymbolHandle,
            site: CheckedBoundaryOperatorApplicationUseSite,
        ) -> Result<Vec<CheckedBodyCandidate<'_, ()>>, BoundaryApplicationEvidenceError> {
            let operators = &facts.operators;
            let mut applications = operators
                .boundary_applications
                .iter()
                .filter(|application| {
                    application.requirement_symbol == symbol && application.site == site
                });
            let first = applications.next();
            let count = usize::from(first.is_some()) + applications.count();
            if count != 1 {
                return Err(BoundaryApplicationEvidenceError::ApplicationCount(count));
            }
            let Some(application) = first else {
                return Err(BoundaryApplicationEvidenceError::ApplicationCount(0));
            };
            operators.project_body_candidates(application, |_| ())
        }
    }
    pub(crate) mod callbacks {
        use crate::machine_lowering::declarations;
        use checked_trees::CheckedTrees;
        use symbols::SymbolHandle;
        pub use terminal_psi::PreparedCallbackDeclarations;
        use terminal_psi::{
            CallbackContractEnvelope, CallbackContractRefinement, CallbackDeclarationRoster,
            CallbackEntryResources, CallbackRequirementDeclaration, CallbackRequirementId,
            CallbackUseDeclaration, TerminalIdentityTableBuilder,
        };
        pub(super) fn prepare(
            checked: &CheckedTrees,
            sources: &[(source::SourceId, [u8; 32])],
            identities: &mut TerminalIdentityTableBuilder,
        ) -> PreparedCallbackDeclarations {
            let mut declarations = CallbackDeclarationRoster::default();
            let mut requirements: Vec<(
                Result<String, String>,
                SymbolHandle,
                SymbolHandle,
                CallbackRequirementId,
            )> = Vec::new();
            let mut occurrences = Vec::new();
            let mut installed_uses = Vec::new();
            let ordinary = checked
                .facts
                .nominal_machine_uses
                .uses
                .iter()
                .map(|usage| (usage, false));
            let installed = checked
                .facts
                .installed_interrupt_registrations
                .iter()
                .flat_map(|registration| {
                    registration
                        .callbacks
                        .uses
                        .iter()
                        .map(|usage| (usage, true))
                });
            for (usage, installed) in ordinary.chain(installed) {
                let classification = checked.callback_declaration(usage);
                let requirement = match requirements.iter().find(|(receiving, owner, member, _)| {
                    *owner == usage.satisfaction_trait
                        && *member == usage.satisfaction_requirement
                        && *receiving == usage.receiving_application
                }) {
                    Some((_, _, _, declaration)) => *declaration,
                    None => {
                        let typed = &checked.typed;
                        let callable = typed
                            .trait_machine_signature(usage.satisfaction_requirement)
                            .map(|(owner, signature)| {
                                typed.trait_requirement_callable_identity_text(owner, signature)
                            })
                            .ok_or_else(|| {
                                "callback requirement has no exact trait callable declaration"
                                    .to_owned()
                            });
                        let declaration =
                            declarations
                                .requirements
                                .insert(CallbackRequirementDeclaration {
                                    owner: declarations::nominal(
                                        identities,
                                        checked,
                                        sources,
                                        usage.satisfaction_trait,
                                    )
                                    .map_err(str::to_owned),
                                    member: declarations::nominal(
                                        identities,
                                        checked,
                                        sources,
                                        usage.satisfaction_requirement,
                                    )
                                    .map_err(str::to_owned),
                                    callable,
                                    is_boundary: classification.satisfaction_is_boundary,
                                });
                        requirements.push((
                            usage.receiving_application.clone(),
                            usage.satisfaction_trait,
                            usage.satisfaction_requirement,
                            declaration,
                        ));
                        declaration
                    }
                };
                let selected_declaration =
                    declarations::identity(identities, checked, sources, usage.selected_machine);
                let declaration = declarations.uses.insert(CallbackUseDeclaration {
                    application_index: usage.application_index,
                    static_machine_ordinal: usage.static_machine_ordinal,
                    registration: classification.registration,
                    registration_declaration: declarations::nominal(
                        identities,
                        checked,
                        sources,
                        usage.registration_operation,
                    )
                    .map_err(str::to_owned),
                    requirement,
                    requirement_overload: usage.canonical_requirement_overload.clone(),
                    published: envelope(usage.published_requirement_envelope),
                    actual: envelope(usage.selected_actual_envelope),
                    refinement: CallbackContractRefinement {
                        published: CallbackContractEnvelope {
                            report_fingerprint: usage
                                .refinement
                                .published_requirement_report_fingerprint,
                            commitment: usage
                                .refinement
                                .published_requirement_commitment
                                .as_bytes(),
                        },
                        actual: CallbackContractEnvelope {
                            report_fingerprint: usage.refinement.selected_actual_report_fingerprint,
                            commitment: usage.refinement.selected_actual_commitment.as_bytes(),
                        },
                    },
                    entry: usage.callback_entry.map(|entry| CallbackEntryResources {
                        machine: declarations::nominal(
                            identities,
                            checked,
                            sources,
                            entry.machine(),
                        )
                        .map_err(str::to_owned),
                        entry: declarations::nominal(identities, checked, sources, entry.entry())
                            .map_err(str::to_owned),
                        contract: CallbackContractEnvelope {
                            report_fingerprint: entry.contract_report_fingerprint(),
                            commitment: entry.contract_commitment().as_bytes(),
                        },
                        stack_report_fingerprint: entry.stack_report_fingerprint(),
                        logical_work_report_fingerprint: entry
                            .logical_structural_work_report_fingerprint(),
                        machine_state_report_fingerprint: entry.machine_state_report_fingerprint(),
                        envelope_report_fingerprint: entry.envelope_report_fingerprint(),
                    }),
                    selected_declaration,
                    selected_entry: declarations::nominal(
                        identities,
                        checked,
                        sources,
                        usage.selected_entry,
                    )
                    .map_err(str::to_owned),
                });
                if installed {
                    installed_uses.push(declaration);
                }
                occurrences.push((
                    declaration,
                    checked
                        .typed
                        .machine(usage.selected_machine)
                        .map(|machine| machine.name.as_str().to_owned()),
                    checked.symbols.symbol_source_span(usage.selected_machine),
                ));
            }
            PreparedCallbackDeclarations::from_projection(
                declarations,
                requirements,
                occurrences,
                installed_uses,
            )
        }

        fn envelope(
            envelope: checked_trees::CheckedMachineContractEnvelopeIdentity,
        ) -> CallbackContractEnvelope {
            CallbackContractEnvelope {
                report_fingerprint: envelope.contract_report_fingerprint,
                commitment: envelope.contract_commitment.as_bytes(),
            }
        }
    }
    pub use callbacks::PreparedCallbackDeclarations;
    pub(crate) mod prepared {

        use super::{ExternalProviderDeclarations, project_external_provider_declarations};
        use checked_trees::CheckedTrees;
        use std::sync::{Arc, Weak};
        use terminal_psi::{
            ExternalProviderConformance, NominalDeclarationIdentity, TerminalIdentityIndex,
            TerminalIdentityTableBuilder, TerminalModule,
        };

        pub struct PreparedExternalProviderDeclarations {
            checked: Weak<CheckedTrees>,
            sources: Arc<[(source::SourceId, [u8; 32])]>,
            declarations: ExternalProviderDeclarations,
            review: language_semantics::boundary_provider_review::RetainedBoundaryProviderReview,
        }

        impl std::fmt::Debug for PreparedExternalProviderDeclarations {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("PreparedExternalProviderDeclarations")
                    .field("sources", &self.sources)
                    .field("declarations", &self.declarations)
                    .finish_non_exhaustive()
            }
        }

        impl PartialEq for PreparedExternalProviderDeclarations {
            fn eq(&self, other: &Self) -> bool {
                Weak::ptr_eq(&self.checked, &other.checked)
                    && self.sources == other.sources
                    && self.declarations == other.declarations
                    && self.review == other.review
            }
        }

        impl Eq for PreparedExternalProviderDeclarations {}

        impl std::ops::Deref for PreparedExternalProviderDeclarations {
            type Target = ExternalProviderDeclarations;
            fn deref(&self) -> &Self::Target {
                &self.declarations
            }
        }

        impl PreparedExternalProviderDeclarations {
            pub fn selected_callback_roots(
                &self,
                selected: impl IntoIterator<Item = terminal_psi::CallbackUseId>,
            ) -> crate::PreparedTerminalRoots {
                self.declarations
                    .callbacks
                    .selected_roots(Arc::clone(&self.declarations.identities), selected)
            }

            pub fn boundary_provider_review(
                &self,
            ) -> language_semantics::boundary_provider_review::BoundaryProviderReview<'_>
            {
                self.review.borrow()
            }

            pub fn prepare(
                checked: Arc<CheckedTrees>,
                sources: &[(source::SourceId, [u8; 32])],
            ) -> Result<Self, crate::LoweringError> {
                let declarations = project_external_provider_declarations(&checked, sources)?;
                let review =
                    super::boundary_review::prepare(&checked.facts, &declarations).retain();
                Ok(Self {
                    checked: Arc::downgrade(&checked),
                    sources: sources.into(),
                    declarations,
                    review,
                })
            }

            pub fn require_owner(
                &self,
                checked: &CheckedTrees,
                sources: &[(source::SourceId, [u8; 32])],
            ) -> Result<(), crate::LoweringError> {
                self.require_owner_address(checked, sources)
            }

            pub fn require_prepared_owner(
                &self,
                checked: &Weak<CheckedTrees>,
                sources: &[(source::SourceId, [u8; 32])],
            ) -> Result<(), crate::LoweringError> {
                self.require_owner_address(checked.as_ptr(), sources)
            }

            fn require_owner_address(
                &self,
                checked: *const CheckedTrees,
                sources: &[(source::SourceId, [u8; 32])],
            ) -> Result<(), crate::LoweringError> {
                if self.checked.as_ptr() != checked || self.sources.as_ref() != sources {
                    return Err(crate::LoweringError::Unsupported(
                        "external declarations belong to another checked program or source custody",
                    ));
                }
                Ok(())
            }

            pub(crate) fn retain(
                &self,
                checked: &CheckedTrees,
                sources: &[(source::SourceId, [u8; 32])],
                identities: &mut TerminalIdentityTableBuilder,
                module: &mut TerminalModule,
            ) -> Result<(), crate::LoweringError> {
                self.require_owner(checked, sources)?;
                let mut retained = Vec::new();
                for candidate in &self.declarations.conformances {
                    let callable = self
                        .declarations
                        .identities
                        .text(candidate.requirement_identity)
                        .ok_or(crate::LoweringError::Unsupported(
                            "external declaration lost its requirement identity",
                        ))?;
                    if !module
                        .boundary_machines
                        .iter()
                        .any(|boundary| identities.text(boundary.identity) == Some(callable))
                        && !module
                            .boundary_application_demands
                            .iter()
                            .any(|demand| demand.requirement().overload() == callable)
                    {
                        continue;
                    }
                    let mut candidate = candidate.clone();
                    remap(&mut candidate, |identity| {
                        let text = self
                            .declarations
                            .identities
                            .text(identity)
                            .ok_or("external declaration identity is absent from its owner")?;
                        if self.declarations.identities.is_invocation_local(identity) {
                            identities.intern_invocation_local(text)
                        } else {
                            Ok(identities.intern(text))
                        }
                    })
                    .map_err(crate::LoweringError::Unsupported)?;
                    retained.push(candidate);
                }
                retained.sort();
                retained.dedup();
                module.external_providers = retained;
                Ok(())
            }
        }

        fn remap(
            candidate: &mut ExternalProviderConformance,
            mut map: impl FnMut(TerminalIdentityIndex) -> Result<TerminalIdentityIndex, &'static str>,
        ) -> Result<(), &'static str> {
            candidate.requirement =
                NominalDeclarationIdentity::new(map(candidate.requirement.identity())?);
            candidate.requirement_identity = map(candidate.requirement_identity)?;
            if let Some(schema) = &mut candidate.requirement_schema {
                schema.identity = map(schema.identity)?;
                schema.method = map(schema.method)?;
                schema.owner = map(schema.owner)?;
            }
            if let Some(provider) = &mut candidate.provider {
                provider.declaration =
                    NominalDeclarationIdentity::new(map(provider.declaration.identity())?);
                provider.identity = map(provider.identity)?;
            }
            let declaration = &mut candidate.realization;
            declaration.machine =
                NominalDeclarationIdentity::new(map(declaration.machine.identity())?);
            declaration.state = NominalDeclarationIdentity::new(map(declaration.state.identity())?);
            declaration.semantic_declaration = map(declaration.semantic_declaration)?;
            declaration.callable = map(declaration.callable)?;
            declaration.result_type = map(declaration.result_type)?;
            for parameter in &mut declaration.parameter_types {
                *parameter = map(*parameter)?;
            }
            if let Some(specialization) = &mut declaration.specialization {
                specialization.template =
                    NominalDeclarationIdentity::new(map(specialization.template.identity())?);
                specialization.template_callable = map(specialization.template_callable)?;
                specialization.template_semantic_declaration =
                    map(specialization.template_semantic_declaration)?;
                if let Some(capture) = &mut specialization.capture {
                    capture.template_identity = map(capture.template_identity)?;
                }
            }
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn policy_view_outlives_the_checked_producer() {
                let checked = Arc::new(CheckedTrees::default());
                let lifetime = Arc::downgrade(&checked);
                let prepared =
                    PreparedExternalProviderDeclarations::prepare(Arc::clone(&checked), &[])
                        .expect("empty declaration projection");
                prepared.require_owner(&checked, &[]).unwrap();
                assert!(
                    prepared
                        .require_owner(&CheckedTrees::default(), &[])
                        .is_err()
                );
                drop(checked);
                assert!(
                    lifetime.upgrade().is_none(),
                    "policy data retains checked source"
                );
                let review = prepared.boundary_provider_review();
                assert!(review.declarations.is_empty());
                assert!(review.operator_uses.is_empty());
                assert!(review.requirement_uses.is_empty());
            }

            #[test]
            fn retained_facts_do_not_transfer_their_source_namespace() {
                let mut checked = Arc::new(CheckedTrees::default());
                let sources = [(source::SourceId(7), [19; 32])];
                let prepared =
                    PreparedExternalProviderDeclarations::prepare(Arc::clone(&checked), &sources)
                        .expect("empty declaration projection");
                assert!(Arc::get_mut(&mut checked).is_none());
                let clone = checked.as_ref().clone();
                assert_eq!(&clone, checked.as_ref());
                assert!(prepared.require_owner(&clone, &sources).is_err());
                assert!(prepared.require_owner(&checked, &[]).is_err());
                assert!(
                    prepared
                        .require_owner(&checked, &[(source::SourceId(7), [20; 32])])
                        .is_err()
                );
                prepared.require_owner(&checked, &sources).unwrap();
            }
        }
    }
    pub use prepared::PreparedExternalProviderDeclarations;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExternalProviderDeclarations {
        pub identities: Arc<TerminalIdentityTable>,
        pub conformances: Vec<ExternalProviderConformance>,
        pub callbacks: PreparedCallbackDeclarations,
        declarations: Vec<(
            SymbolHandle,
            Result<NominalDeclarationIdentity, &'static str>,
        )>,
    }

    impl ExternalProviderDeclarations {
        pub fn correspondence(
            &self,
        ) -> &[(
            SymbolHandle,
            Result<NominalDeclarationIdentity, &'static str>,
        )] {
            &self.declarations
        }

        pub fn has_requirement_schema(&self, symbol: SymbolHandle) -> bool {
            let Some(nominal) = self.nominal(symbol) else {
                return false;
            };
            self.conformances.iter().any(|candidate| {
                self.identities.text(candidate.requirement.identity()) == Some(nominal)
                    && candidate.requirement_schema.is_some()
            })
        }
        pub fn nominal(&self, symbol: SymbolHandle) -> Option<&str> {
            let (_, declaration) = self
                .declarations
                .iter()
                .find(|(source, _)| *source == symbol)?;
            self.identities.text(declaration.as_ref().ok()?.identity())
        }
    }

    pub fn project_external_provider_declarations(
        checked: &CheckedTrees,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<ExternalProviderDeclarations, crate::LoweringError> {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
        let mut declarations = Vec::new();
        let conformances = crate::retention::external_providers::project(
            &mut identities,
            checked,
            sources,
            |_, realization, requirement, _| {
                [realization, requirement].into_iter().all(|symbol| {
                    checked
                        .symbols
                        .symbol_is_in_dependency_scope(symbol, source::DependencyScope::Product)
                })
            },
            |symbol, identity| declarations.push((symbol, Ok(identity))),
        )?;
        let mut represented = declarations
            .iter()
            .map(|(symbol, _)| (symbol.arena_index(), symbol.generation()))
            .collect::<Vec<_>>();
        represented.sort_unstable();
        represented.dedup();
        let requirements = arena::unique_handles_in_order(
            checked
                .facts
                .operators
                .provider_requirements
                .iter()
                .map(|(_, requirement)| requirement.symbol),
        );
        for requirement in requirements {
            if represented
                .binary_search(&(requirement.arena_index(), requirement.generation()))
                .is_ok()
            {
                continue;
            }
            declarations.push((
                requirement,
                crate::machine_lowering::declarations::nominal(
                    &mut identities,
                    checked,
                    sources,
                    requirement,
                ),
            ));
        }
        declarations.sort_by_key(|(symbol, declaration)| {
            (
                symbol.arena_index(),
                symbol.generation(),
                declaration
                    .as_ref()
                    .map(NominalDeclarationIdentity::identity)
                    .map_err(|reason| *reason),
            )
        });
        declarations.dedup();
        let callbacks = callbacks::prepare(checked, sources, &mut identities);
        Ok(ExternalProviderDeclarations {
            identities: Arc::new(identities.build()),
            conformances,
            callbacks,
            declarations,
        })
    }
}
pub(crate) mod lowered_provenance {

    use crate::shared::direct_calls::direct_call_parts;
    use checked_trees::CheckedTrees;
    use semantic_vocabulary::MachineId;
    use std::collections::BTreeMap;
    use terminal_psi::TerminalMachineProvenance;

    pub fn lowered_module_provenance(
        checked: &CheckedTrees,
        lowered: &lowered_psi::LoweredPsi,
    ) -> TerminalMachineProvenance {
        let module = &lowered.semantic_module;
        let mut callee_by_operation = BTreeMap::new();
        for machine in &module.machines {
            for block in &machine.blocks {
                for operation in &block.operations {
                    if let Some(call) = direct_call_parts(&operation.kind) {
                        callee_by_operation.insert(operation.id, call.callee);
                    }
                }
            }
        }
        let mut provenance = TerminalMachineProvenance::default();
        for occurrence in &lowered.source_call_occurrences {
            if let Some(operation) = occurrence.terminal_site.operation()
                && let Some(callee) = callee_by_operation.get(&operation)
                && let Ok(target) =
                    crate::shared::call_sites::selected_module_target(checked, occurrence)
            {
                name(&mut provenance, checked, *callee, target.machine.symbol);
            }
        }
        if !module.provider_candidates.is_empty() {
            let identities: BTreeMap<String, symbols::SymbolHandle> = checked
                .typed
                .machines()
                .iter()
                .filter_map(|machine| {
                    checked
                        .typed
                        .machine_callable_identity_text(machine)
                        .map(|identity| (identity, machine.symbol))
                })
                .collect();
            for candidate in &module.provider_candidates {
                if let Some(symbol) = identities.get(
                    module
                        .identities
                        .text(candidate.candidate_identity)
                        .unwrap_or_default(),
                ) {
                    name(&mut provenance, checked, candidate.candidate, *symbol);
                }
            }
        }
        provenance
    }

    fn name(
        provenance: &mut TerminalMachineProvenance,
        checked: &CheckedTrees,
        machine: MachineId,
        symbol: symbols::SymbolHandle,
    ) {
        if let Some(declaration) = checked.typed.machine(symbol) {
            provenance.name(
                machine,
                declaration.name.as_str().to_owned(),
                checked.typed.symbols.symbol_source_span(symbol),
            );
        }
    }
}
pub(crate) mod program_roots {

    #[derive(Clone, Copy, Default)]
    pub struct TerminalProgramRoots<'a> {
        pub machines: &'a [symbols::SymbolHandle],
        pub boundaries: &'a [symbols::SymbolHandle],
    }
}
pub(crate) mod task_activation {
    pub(crate) mod carry_crossings {

        use diagnostics::Diagnostic;

        macro_rules! cross {
            ($detail:literal) => {
                vec![Diagnostic::error(concat!(
                    "task activation carry crossing ",
                    $detail
                ))]
            };
        }

        use language_semantics::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
        use terminal_psi::{
            TerminalTaskCarryObligations as ActivationCarryObligations,
            TerminalTaskCarryStorage as LiveCarryStorage, TerminalTaskLiveCarry as LiveCarryDemand,
            TerminalTaskSuspensionCrossing as CanonicalSuspensionCrossing,
        };

        pub(crate) fn exact_activation_wide_carry<'program>(
            check_facts: &'program checked_trees::CheckFacts,
            machine: symbols::SymbolHandle,
            machine_name: &str,
        ) -> Result<&'program checked_trees::MachineActivationCarryFact, Vec<Diagnostic>> {
            let mut matches = check_facts
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

        pub(crate) fn validate_activation_carry_crossing(
            typed: &checked_trees::typed_trees::TypedTrees,
            check_facts: &checked_trees::CheckFacts,
            crossing: &checked_trees::SuspensionCrossingCarryFact,
        ) -> Result<(), Vec<Diagnostic>> {
            let mut machines = typed
                .machines()
                .iter()
                .filter(|machine| machine.symbol == crossing.machine);
            let machine = machines
                .next()
                .ok_or_else(|| cross!("must name an exact typed machine"))?;
            if machines.next().is_some() {
                return Err(cross!("machine must resolve uniquely"));
            }
            let mut states = typed
                .machine_states(machine)
                .iter()
                .filter(|state| state.symbol == crossing.state);
            states
                .next()
                .ok_or_else(|| cross!("state must belong to its exact typed machine"))?;
            if states.next().is_some() {
                return Err(cross!("state must resolve uniquely within its machine"));
            }

            let mut flow_states = check_facts
                .flow
                .control
                .machine_states(crossing.machine)
                .into_iter()
                .filter(|flow| flow.state_symbol == crossing.state)
                .map(|state| ((), state));
            let flow_state = flow_states
                .next()
                .map(|(_, flow)| flow)
                .ok_or_else(|| cross!("must name one exact checked flow state"))?;
            if flow_states.next().is_some() {
                return Err(cross!("must name exactly one checked flow state"));
            }
            let calls = check_facts
                .flow
                .control
                .calls
                .span(flow_state.calls)
                .ok_or_else(|| cross!("flow state must retain an exact valid call span"))?;
            let mut calls = calls.iter().filter(|call| {
                call.statement_index == crossing.statement_index
                    && call.call_ordinal == crossing.call_ordinal
            });
            let call = calls
                .next()
                .ok_or_else(|| cross!("must name one exact checked flow call"))?;
            if calls.next().is_some() {
                return Err(cross!("must name exactly one checked flow call"));
            }
            if call.target_symbol != crossing.target {
                return Err(cross!("must retain its exact checked call target"));
            }
            let mut targets = typed.machines().iter().flat_map(|machine| {
                typed
                    .machine_states(machine)
                    .iter()
                    .filter(|state| state.symbol == crossing.target)
            });
            targets
                .next()
                .ok_or_else(|| cross!("target must name an exact typed state"))?;
            if targets.next().is_some() {
                return Err(cross!("target must resolve to exactly one typed state"));
            }
            if !call.suspension.direct_may_suspend && !call.suspension.transitive_may_suspend {
                return Err(cross!("must retain a may-suspend checked call"));
            }
            Ok(())
        }

        pub(crate) fn carry_obligations(policy: CarryPolicy) -> ActivationCarryObligations {
            ActivationCarryObligations {
                preserve_cpu: policy.cpu == CarryCpu::Origin,
                preserve_host_thread: policy.host_thread == CarryHostThread::Origin,
            }
        }

        pub(crate) fn canonical_suspension_crossing(
            typed: &checked_trees::typed_trees::TypedTrees,
            crossing: &checked_trees::SuspensionCrossingCarryFact,
        ) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
            let mut live_carry = Vec::with_capacity(crossing.live_values.len());
            for live in &crossing.live_values {
                live_carry.push(live_carry_demand(typed, crossing, live)?);
            }
            Ok(CanonicalSuspensionCrossing {
                identity: checked_trees::canonical_suspension_crossing_id(typed, crossing)
                    .ok_or_else(|| cross!("source identity must resolve exactly"))?
                    .get(),
                suspension_allowed: crossing.effective.suspension == CarrySuspension::Allowed,
                preserve_cpu: crossing.effective.cpu == CarryCpu::Origin,
                preserve_host_thread: crossing.effective.host_thread == CarryHostThread::Origin,
                live_carry,
            })
        }

        fn live_carry_demand(
            typed: &checked_trees::typed_trees::TypedTrees,
            crossing: &checked_trees::SuspensionCrossingCarryFact,
            live: &checked_trees::SuspensionCrossingLiveValueFact,
        ) -> Result<LiveCarryDemand, Vec<Diagnostic>> {
            let mut claims = live
                .claims
                .iter()
                .map(|claim| live_claim_identity(typed, claim))
                .collect::<Result<Vec<_>, _>>()?;
            claims.sort_unstable();
            if claims.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(vec![Diagnostic::error(
                    "task carry frontier repeats a live claim",
                )]);
            }
            Ok(LiveCarryDemand {
                place: live_place_identity(typed, crossing, live),
                value_type: {
                    let mut hash = semantic_vocabulary::Fnv1a::new();
                    hash.byte(0x54);
                    hash.string_terminated(
                        &typed.normalized_type_identity_text(live.type_reference),
                    );
                    hash.finish_or(1)
                },
                storage: match live.storage {
                    checked_trees::SuspensionCrossingStorage::Parameter => {
                        LiveCarryStorage::Parameter
                    }
                    checked_trees::SuspensionCrossingStorage::Local => LiveCarryStorage::Local,
                    checked_trees::SuspensionCrossingStorage::CallArgument => {
                        LiveCarryStorage::CallArgument
                    }
                },
                claims,
                effective: live.effective,
            })
        }

        fn live_place_identity(
            typed: &checked_trees::typed_trees::TypedTrees,
            crossing: &checked_trees::SuspensionCrossingCarryFact,
            live: &checked_trees::SuspensionCrossingLiveValueFact,
        ) -> u64 {
            let mut hash = semantic_vocabulary::Fnv1a::new();
            hash.byte(0x4c);
            hash.string_terminated(&typed.symbols.display_path(crossing.machine, "::"));
            hash.string_terminated(&typed.symbols.display_path(crossing.state, "::"));
            hash.usize(crossing.statement_index);
            hash.usize(crossing.call_ordinal);
            match live.origin {
                checked_trees::SuspensionCrossingValueOrigin::Parameter { symbol, position } => {
                    hash.byte(2);
                    hash.string_terminated(&typed.symbols.display_path(symbol, "::"));
                    hash.usize(position);
                }
                checked_trees::SuspensionCrossingValueOrigin::Local {
                    symbol,
                    statement_index,
                    environment_position,
                } => {
                    hash.byte(3);
                    hash.string_terminated(&typed.symbols.display_path(symbol, "::"));
                    hash.usize(statement_index);
                    hash.usize(environment_position);
                }
                checked_trees::SuspensionCrossingValueOrigin::CallArgument { position } => {
                    hash.byte(4);
                    hash.usize(position);
                }
            }
            hash.finish_or(1)
        }

        fn live_claim_identity(
            typed: &checked_trees::typed_trees::TypedTrees,
            claim: &language_semantics::PermissionClaimIdentity,
        ) -> Result<u64, Vec<Diagnostic>> {
            let language_semantics::PermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } = *claim
            else {
                return Err(cross!("retains a live claim whose identity never resolved"));
            };
            let mut hash = semantic_vocabulary::Fnv1a::new();
            hash.byte(0x43);
            hash.string_terminated(&typed.symbols.display_path(machine_symbol, "::"));
            hash.string_terminated(&typed.symbols.display_path(state_symbol, "::"));
            match source {
                language_semantics::PermissionEventSource::StateEntry => hash.byte(1),
                language_semantics::PermissionEventSource::Statement { statement_index } => {
                    hash.byte(2);
                    hash.usize(statement_index);
                }
                language_semantics::PermissionEventSource::Call {
                    statement_index,
                    call_ordinal,
                    target_symbol,
                } => {
                    hash.byte(3);
                    hash.usize(statement_index);
                    hash.usize(call_ordinal);
                    hash.string_terminated(&typed.symbols.display_path(target_symbol, "::"));
                }
                language_semantics::PermissionEventSource::StateExit => hash.byte(4),
                language_semantics::PermissionEventSource::InstructionExitDisposition {
                    statement_index,
                } => {
                    hash.byte(6);
                    hash.usize(statement_index);
                }
                language_semantics::PermissionEventSource::InstructionDisposition {
                    statement_index,
                } => {
                    hash.byte(5);
                    hash.usize(statement_index);
                }
            }
            hash.u64(u64::from(ordinal));
            Ok(hash.finish_or(1))
        }
    }
    pub(crate) mod runtime_requirements {

        use crate::projections::task_activation::start_selections::TaskStartSelection;
        use diagnostics::Diagnostic;

        fn exact_task_machine_plan<P>(
            matches: impl Iterator<Item = (symbols::SymbolHandle, P)>,
            machine: symbols::SymbolHandle,
            machine_name: &str,
            plan_domain: &'static str,
        ) -> Result<P, Vec<Diagnostic>> {
            let mut matches = matches.filter(|(fact_machine, _)| *fact_machine == machine);
            let (_, plan) = matches.next().ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "task activation target `{machine_name}` has no checked {plan_domain} plan"
                ))]
            })?;
            if matches.next().is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "task activation target `{machine_name}` has duplicate exact checked {plan_domain} plans"
                ))]);
            }
            Ok(plan)
        }

        pub(crate) fn exact_task_machine_suspension(
            check_facts: &checked_trees::CheckFacts,
            machine: symbols::SymbolHandle,
            machine_name: &str,
        ) -> Result<language_semantics::SuspensionPlan, Vec<Diagnostic>> {
            exact_task_machine_plan(
                check_facts
                    .suspensions
                    .machines
                    .iter()
                    .map(|fact| (fact.machine, fact.plan)),
                machine,
                machine_name,
                "suspension",
            )
        }

        pub(crate) fn exact_task_machine_blocking(
            check_facts: &checked_trees::CheckFacts,
            machine: symbols::SymbolHandle,
            machine_name: &str,
        ) -> Result<language_semantics::BlockingPlan, Vec<Diagnostic>> {
            exact_task_machine_plan(
                check_facts
                    .blocking
                    .machines
                    .iter()
                    .map(|fact| (fact.machine, fact.plan)),
                machine,
                machine_name,
                "blocking",
            )
        }

        pub(crate) fn exact_task_runtime_requirement<'program>(
            typed: &'program checked_trees::typed_trees::TypedTrees,
            selection: &TaskStartSelection,
        ) -> Result<
            (
                &'program checked_trees::trait_definition::TraitDefinition,
                &'program checked_trees::signature::StateSignature,
                checked_trees::typed_trees::type_identity::CallableIdentityId,
            ),
            Vec<Diagnostic>,
        > {
            let owner = typed
                .trait_definition_by_symbol(selection.requirement_owner)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "TaskRuntime activation requirement must name one exact retained trait owner",
                    )]
                })?;
            if !owner.is_boundary {
                return Err(vec![Diagnostic::error(
                    "TaskRuntime activation requirement owner must be an exact boundary trait",
                )]);
            }
            let application = typed
                .closed_requirement_application(selection.requirement)
                .filter(|application| application.declaring_trait == owner.symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "TaskRuntime activation requirement must retain its exact closed application",
                    )]
                })?;
            let signature = typed
                .trait_machine_signature_by_symbol(owner, application.template)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "TaskRuntime activation requirement must belong to its exact retained owner",
                    )]
                })?;
            let identity = typed.trait_requirement_callable_identity(owner, signature);
            Ok((owner, signature, identity))
        }
    }
    pub(crate) mod specialization_commitments {

        use crate::projections::task_activation::start_selections::{
            TaskStartSelection, unique_task_activation_target,
        };
        use checked_trees::typed_trees::type_identity::TypeIdentityRequest;
        use diagnostics::Diagnostic;
        use terminal_psi::TerminalTaskStartOperation as TaskStartOperation;

        pub(crate) fn exact_task_machine_contract<'program>(
            check_facts: &'program checked_trees::CheckFacts,
            machine: symbols::SymbolHandle,
            machine_name: &str,
        ) -> Result<&'program checked_trees::MachineContractPlan, Vec<Diagnostic>> {
            let mut matches = check_facts
                .contract_plans
                .machines()
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
            typed: &checked_trees::typed_trees::TypedTrees,
            check_facts: &checked_trees::CheckFacts,
            selection: &TaskStartSelection,
            target_machine: &checked_trees::machine::Machine,
            target_entry: &checked_trees::state::State,
            target_contract: &checked_trees::MachineContractPlan,
        ) -> Result<[u8; 32], Vec<Diagnostic>> {
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
                self.0.bytes(value.as_bytes());
            }

            fn bytes(&mut self, value: &[u8]) {
                self.0.bytes(value);
            }

            fn strings(&mut self, values: &[String]) {
                self.length(values.len());
                for value in values {
                    self.string(value);
                }
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
                typed: &checked_trees::typed_trees::TypedTrees,
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
                let template =
                    exact_specialization_machine(typed, specialization.template, "template")?;
                self.machine(typed, template)?;
                self.digest(
                    exact_task_machine_contract(
                        check_facts,
                        template.symbol,
                        template.name.as_str(),
                    )?
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
                        exact_task_machine_contract(
                            check_facts,
                            owner.symbol,
                            owner.name.as_str(),
                        )?
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
                        typed,
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
                typed: &checked_trees::typed_trees::TypedTrees,
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

            fn state(
                &mut self,
                typed: &checked_trees::typed_trees::TypedTrees,
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
                    self.string(
                        &typed.package_qualified_type_identity_text(parameter.type_reference),
                    );
                }
                self.string(&typed.package_qualified_type_identity_text(state.return_type));
            }

            fn finish(self) -> [u8; 32] {
                self.0.finish()
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
    }
    pub(crate) mod stack_graphs {

        use super::carry_crossings::{
            exact_activation_wide_carry, validate_activation_carry_crossing,
        };
        use checked_trees::SuspensionCrossingStorage;
        use diagnostics::Diagnostic;
        use language_semantics::{CarryPolicy, MachineSupplyMode};
        use symbols::SymbolHandle;
        use terminal_psi::{
            TerminalIdentityTableBuilder, TerminalTaskFrameCall, TerminalTaskUnresolvedCall,
            TerminalTaskUnresolvedCallKind as UnresolvedCallKind,
        };
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) struct TaskFrameState {
            pub(crate) state: terminal_psi::TerminalIdentityIndex,
            pub(crate) resident: Vec<TypeReferenceHandle>,
            pub(crate) crossing_arguments: Vec<Vec<TypeReferenceHandle>>,
        }
        pub(crate) struct TaskFrame {
            pub(crate) identity: u64,
            pub(crate) machine_callable_identity: terminal_psi::TerminalIdentityIndex,
            pub(crate) entry: terminal_psi::TerminalIdentityIndex,
            pub(crate) states: Vec<TaskFrameState>,
            pub(crate) calls: Vec<TerminalTaskFrameCall>,
            pub(crate) unresolved_calls: Vec<TerminalTaskUnresolvedCall>,
        }

        pub(crate) struct TaskCallGraph<'program> {
            pub(crate) root: u64,
            pub(crate) frames: Vec<TaskFrame>,
            pub(crate) crossings: Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
            pub(crate) carry: CarryPolicy,
        }

        pub(crate) fn task_call_graph<'program>(
            typed: &'program checked_trees::typed_trees::TypedTrees,
            check_facts: &'program checked_trees::CheckFacts,
            identities: &mut TerminalIdentityTableBuilder,
            root_machine: &'program checked_trees::machine::Machine,
            root_entry: &'program checked_trees::state::State,
        ) -> Result<TaskCallGraph<'program>, Vec<Diagnostic>> {
            let root = frame_identity(typed, root_machine, root_entry)?;
            let mut pending = vec![root_entry.symbol];
            let mut visited = Vec::new();
            let mut frames = Vec::new();
            let mut crossings = Vec::new();
            let mut carry = CarryPolicy::PERMISSIVE;
            while let Some(entry_symbol) = pending.pop() {
                if visited.contains(&entry_symbol) {
                    continue;
                }
                visited.push(entry_symbol);
                let (machine, entry) = exact_frame_target(typed, entry_symbol)?;
                carry = carry.intersect(
                    exact_activation_wide_carry(
                        check_facts,
                        machine.symbol,
                        machine.name.as_str(),
                    )?
                    .effective,
                );
                let summary = task_frame_summary(
                    typed,
                    check_facts,
                    identities,
                    machine,
                    entry,
                    &mut pending,
                    &mut crossings,
                )?;
                frames.push(summary);
            }
            Ok(TaskCallGraph {
                root,
                frames,
                crossings,
                carry,
            })
        }

        pub(crate) fn exact_frame_target(
            typed: &checked_trees::typed_trees::TypedTrees,
            entry: SymbolHandle,
        ) -> Result<
            (
                &checked_trees::machine::Machine,
                &checked_trees::state::State,
            ),
            Vec<Diagnostic>,
        > {
            let mut matches = typed.machine_states_named(entry);
            let (machine, state) = matches.next().ok_or_else(|| {
                vec![Diagnostic::error(
                    "task WCSU graph call target must name an exact typed state",
                )]
            })?;
            if matches.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "task WCSU graph call target must resolve to exactly one typed state",
                )]);
            }
            Ok((machine, state))
        }

        fn task_frame_summary<'program>(
            typed: &'program checked_trees::typed_trees::TypedTrees,
            check_facts: &'program checked_trees::CheckFacts,
            identities: &mut TerminalIdentityTableBuilder,
            machine: &'program checked_trees::machine::Machine,
            entry: &'program checked_trees::state::State,
            pending: &mut Vec<SymbolHandle>,
            reachable_crossings: &mut Vec<&'program checked_trees::SuspensionCrossingCarryFact>,
        ) -> Result<TaskFrame, Vec<Diagnostic>> {
            let frame = frame_identity(typed, machine, entry)?;
            let states = frame_state_closure(typed, check_facts, machine, entry)?;

            let mut resident_states = Vec::new();
            for state in &states {
                let mut resident = typed
                    .state_parameters(state)
                    .iter()
                    .filter(|parameter| !parameter.relevance.is_erased())
                    .map(|parameter| parameter.type_reference)
                    .collect::<Vec<_>>();
                resident.extend(
                    check_facts
                        .state_locals
                        .locals(state.symbol)
                        .iter()
                        .filter(|local| !local.is_erased)
                        .map(|local| local.declared_type),
                );
                let mut crossing_coordinates = Vec::new();
                let mut crossing_arguments = Vec::new();
                for crossing in check_facts
                    .carry
                    .suspension_crossings
                    .iter()
                    .filter(|crossing| {
                        crossing.machine == machine.symbol && crossing.state == state.symbol
                    })
                {
                    validate_activation_carry_crossing(typed, check_facts, crossing)?;
                    let coordinate = (crossing.statement_index, crossing.call_ordinal);
                    if crossing_coordinates.contains(&coordinate) {
                        return Err(vec![Diagnostic::error(
                            "task activation carry crossings must retain one row per exact call coordinate",
                        )]);
                    }
                    crossing_coordinates.push(coordinate);
                    reachable_crossings.push(crossing);
                    crossing_arguments.push(
                        crossing
                            .live_values
                            .iter()
                            .filter(|live| live.storage == SuspensionCrossingStorage::CallArgument)
                            .map(|live| live.type_reference)
                            .collect(),
                    );
                }
                resident_states.push(TaskFrameState {
                    state: identities.intern(state.name.as_str()),
                    resident,
                    crossing_arguments,
                });
            }

            let mut calls = Vec::new();
            let mut unresolved_calls = Vec::new();
            for state in &states {
                let mut flow_states = check_facts
                    .flow
                    .control
                    .machine_states(machine.symbol)
                    .into_iter()
                    .filter(|flow| flow.state_symbol == state.symbol)
                    .map(|state| ((), state));
                let Some((_, flow_state)) = flow_states.next() else {
                    continue;
                };
                if flow_states.next().is_some() {
                    return Err(vec![Diagnostic::error(
                        "task WCSU frame must name exactly one checked flow state",
                    )]);
                }
                let flow_calls = check_facts
                    .flow
                    .control
                    .calls
                    .span(flow_state.calls)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "task WCSU frame flow state must retain an exact valid call span",
                        )]
                    })?;
                for call in flow_calls {
                    if check_facts.flow.control.is_retired(state.symbol, call) {
                        continue;
                    }
                    let may_suspend = call.suspension.direct_may_suspend
                        || call.suspension.transitive_may_suspend;
                    if may_suspend
                        && !check_facts
                            .carry
                            .suspension_crossings
                            .iter()
                            .any(|crossing| {
                                crossing.machine == machine.symbol
                                    && crossing.state == state.symbol
                                    && crossing.statement_index == call.statement_index
                                    && crossing.call_ordinal == call.call_ordinal
                            })
                    {
                        return Err(vec![Diagnostic::error(format!(
                            "task WCSU frame `{}` state `{}` has a possibly-suspending call at statement {} call {} with no canonical suspension crossing",
                            machine.name, state.name, call.statement_index, call.call_ordinal,
                        ))]);
                    }
                    let Some((callee_machine, callee_entry)) =
                        checked_call_target(typed, call.target_symbol)?
                    else {
                        unresolved_calls.push(TerminalTaskUnresolvedCall {
                            state: identities.intern(state.name.as_str()),
                            statement_index: u32::try_from(call.statement_index).map_err(|_| {
                                vec![Diagnostic::error(
                                    "task call statement coordinate exceeds u32",
                                )]
                            })?,
                            call_ordinal: u32::try_from(call.call_ordinal).map_err(|_| {
                                vec![Diagnostic::error("task call ordinal exceeds u32")]
                            })?,
                            kind: UnresolvedCallKind::UnresolvedTarget,
                        });
                        continue;
                    };
                    if callee_machine.supply_mode != MachineSupplyMode::CheckedBody {
                        unresolved_calls.push(TerminalTaskUnresolvedCall {
                            state: identities.intern(state.name.as_str()),
                            statement_index: u32::try_from(call.statement_index).map_err(|_| {
                                vec![Diagnostic::error(
                                    "task call statement coordinate exceeds u32",
                                )]
                            })?,
                            call_ordinal: u32::try_from(call.call_ordinal).map_err(|_| {
                                vec![Diagnostic::error("task call ordinal exceeds u32")]
                            })?,
                            kind: UnresolvedCallKind::NonCheckedSupply,
                        });
                        continue;
                    }
                    let callee = frame_identity(typed, callee_machine, callee_entry)?;
                    calls.push(TerminalTaskFrameCall { callee });
                    pending.push(callee_entry.symbol);
                }
            }

            Ok(TaskFrame {
                identity: frame,
                machine_callable_identity: identities.intern(
                    &typed
                        .machine_callable_identity_text(machine)
                        .ok_or_else(|| {
                            vec![Diagnostic::error("task frame lacks its callable identity")]
                        })?,
                ),
                entry: identities.intern(entry.name.as_str()),
                states: resident_states,
                calls,
                unresolved_calls,
            })
        }

        fn frame_state_closure<'program>(
            typed: &'program checked_trees::typed_trees::TypedTrees,
            check_facts: &checked_trees::CheckFacts,
            machine: &'program checked_trees::machine::Machine,
            entry: &'program checked_trees::state::State,
        ) -> Result<Vec<&'program checked_trees::state::State>, Vec<Diagnostic>> {
            let mut ordered = Vec::new();
            let mut seen = Vec::new();
            let mut pending = vec![entry.symbol];
            while let Some(symbol) = pending.pop() {
                if seen.contains(&symbol) {
                    continue;
                }
                seen.push(symbol);
                let state = typed.state_in_machine(machine, symbol).ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "task WCSU frame `{}` transition reaches a state outside its machine",
                        machine.name
                    ))]
                })?;
                for transition in check_facts
                    .executable_bindings
                    .transitions
                    .iter()
                    .filter(|transition| transition.state == state.symbol)
                {
                    for target in &transition.named_targets {
                        if typed.state_in_machine(machine, *target).is_some() {
                            pending.push(*target);
                        }
                    }
                }
                ordered.push(state);
            }
            Ok(ordered)
        }

        fn checked_call_target(
            typed: &checked_trees::typed_trees::TypedTrees,
            target: SymbolHandle,
        ) -> Result<
            Option<(
                &checked_trees::machine::Machine,
                &checked_trees::state::State,
            )>,
            Vec<Diagnostic>,
        > {
            let mut matches = typed.machine_states_named(target);
            let Some(target) = matches.next() else {
                return Ok(None);
            };
            if matches.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "task WCSU graph call target must resolve to exactly one typed state",
                )]);
            }
            Ok(Some(target))
        }

        fn frame_identity<'program>(
            typed: &'program checked_trees::typed_trees::TypedTrees,
            machine: &'program checked_trees::machine::Machine,
            entry: &'program checked_trees::state::State,
        ) -> Result<u64, Vec<Diagnostic>> {
            let callable = typed
                .machine_callable_identity_text(machine)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "task WCSU frame `{}` has no normalized machine overload identity",
                        machine.name
                    ))]
                })?;
            let mut hash = semantic_vocabulary::Fnv1a::new();
            hash.byte(0x57);
            hash.string_terminated(callable.as_str());
            hash.string_terminated(entry.name.as_str());
            Ok(hash.finish_or(1))
        }
    }
    pub(crate) mod start_selections {

        use super::entry_report_fingerprint;
        use diagnostics::Diagnostic;
        use semantic_vocabulary::Fnv1a;
        use terminal_psi::TerminalTaskStartOperation as TaskStartOperation;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct TaskStartSelection {
            pub(crate) requirement_owner: symbols::SymbolHandle,
            pub(crate) requirement: symbols::SymbolHandle,
            pub(crate) target_machine: symbols::SymbolHandle,
            pub(crate) target_entry: symbols::SymbolHandle,
            pub(crate) report_fingerprint: u64,
            pub(crate) operation: TaskStartOperation,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum TaskActivationTargetError {
            MissingEntry,
            AmbiguousEntry,
            AmbiguousMachine,
            MachineMismatch,
        }

        impl TaskActivationTargetError {
            pub(crate) fn message(self) -> &'static str {
                match self {
                    Self::MissingEntry => "target entry must name one exact typed state",
                    Self::AmbiguousEntry => "target entry must resolve to exactly one typed state",
                    Self::AmbiguousMachine => {
                        "target owner must resolve to exactly one typed machine"
                    }
                    Self::MachineMismatch => {
                        "target entry must belong to its retained exact machine"
                    }
                }
            }
        }

        pub(crate) fn unique_task_activation_target(
            typed: &checked_trees::typed_trees::TypedTrees,
            target_entry: symbols::SymbolHandle,
        ) -> Result<
            (
                &checked_trees::machine::Machine,
                &checked_trees::state::State,
            ),
            TaskActivationTargetError,
        > {
            let mut matches = typed.machine_states_named(target_entry);
            let (machine, entry) = matches
                .next()
                .ok_or(TaskActivationTargetError::MissingEntry)?;
            if matches.next().is_some() {
                return Err(TaskActivationTargetError::AmbiguousEntry);
            }
            let mut owners = typed
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

        pub(crate) fn exact_task_activation_target(
            typed: &checked_trees::typed_trees::TypedTrees,
            target_machine: symbols::SymbolHandle,
            target_entry: symbols::SymbolHandle,
        ) -> Result<
            (
                &checked_trees::machine::Machine,
                &checked_trees::state::State,
            ),
            TaskActivationTargetError,
        > {
            let (machine, entry) = unique_task_activation_target(typed, target_entry)?;
            if machine.symbol != target_machine {
                return Err(TaskActivationTargetError::MachineMismatch);
            }
            Ok((machine, entry))
        }

        pub(crate) fn task_start_selections(
            checked: &checked_trees::CheckedTrees,
        ) -> Result<Vec<TaskStartSelection>, Vec<Diagnostic>> {
            let mut selections = Vec::new();
            let mut diagnostics = Vec::new();
            for call in &checked.facts.executable_bindings.calls {
                append_task_start_selection(
                    checked,
                    call.target,
                    &call.diagnostic_name,
                    &mut selections,
                    &mut diagnostics,
                );
            }
            if !diagnostics.is_empty() {
                return Err(diagnostics);
            }
            selections.sort_by_key(|selection| selection.report_fingerprint);
            selections.dedup();
            Ok(selections)
        }

        fn append_task_start_selection(
            checked: &checked_trees::CheckedTrees,
            requirement: symbols::SymbolHandle,
            target_name: &str,
            selections: &mut Vec<TaskStartSelection>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let typed = &checked.typed;
            /*
                A call may name a specialized copy of the requirement, which
                has no authored declaration of its own; ask the catalog about
                the authored trait and the member name instead.
            */
            use language_core::sealed_intrinsics::SealedIntrinsic;
            let Some((definition, signature, operation)) =
                typed.traits().iter().find_map(|definition| {
                    if !definition.is_boundary {
                        return None;
                    }
                    let signature =
                        typed.trait_machine_signature_by_symbol(definition, requirement)?;
                    let operation = match typed
                        .symbols
                        .sealed_member_intrinsic(definition.symbol, signature.name.as_str())
                    {
                        Some(SealedIntrinsic::TaskRuntimeStart) => TaskStartOperation::Start,
                        Some(SealedIntrinsic::TaskRuntimeTryStart) => TaskStartOperation::TryStart,
                        _ => return None,
                    };
                    Some((definition, signature, operation))
                })
            else {
                return;
            };
            let Some(application) = checked
                .facts
                .executable_bindings
                .requirements
                .iter()
                .find(|application| application.requirement == requirement)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "TaskRuntime requirement `{target_name}` has no retained checked static application"
                )));
                return;
            };
            let targets = &application.machines;
            let [target] = targets.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "TaskRuntime requirement `{target_name}` must select exactly one static target machine, got {}",
                    targets.len()
                )));
                return;
            };
            let (target_machine, target_entry) = match unique_task_activation_target(
                typed,
                target.symbol,
            ) {
                Ok(target) => target,
                Err(TaskActivationTargetError::MissingEntry)
                    if target.symbol.is_valid()
                        && typed
                            .machines()
                            .iter()
                            .flat_map(|machine| typed.machine_type_parameters(machine))
                            .any(|parameter| parameter.symbol == target.symbol) =>
                {
                    return;
                }
                Err(TaskActivationTargetError::MissingEntry) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "TaskRuntime requirement `{target_name}` selects an unresolved static target `{}`",
                        target.diagnostic_path
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
            let mut hash = Fnv1a::new();
            hash.string_terminated("task-runtime-requirement-specialization-v1");
            hash.string_terminated(
                typed
                    .trait_requirement_callable_identity_text(definition, signature)
                    .as_str(),
            );
            hash.u64(entry_report_fingerprint(
                typed,
                target_machine,
                target_entry,
            ));
            let selection = TaskStartSelection {
                requirement_owner: definition.symbol,
                requirement,
                target_machine: target_machine.symbol,
                target_entry: target_entry.symbol,
                report_fingerprint: hash.finish_or(1),
                operation,
            };
            if !selections.contains(&selection) {
                selections.push(selection);
            }
        }
    }
    pub(crate) mod target_layouts {
        use crate::unit::attached_unit::{bodies::UnitPlans, catalog::selected_declaration};
        use checked_trees::{
            CheckedTrees, CheckedUnitRecordStorage, CheckedUnitStructuralTypeShape,
        };
        use diagnostics::Diagnostic;
        use terminal_psi::*;

        pub(crate) fn project(
            checked: &CheckedTrees,
            plans: UnitPlans<'_>,
            types: &[StructuralTypeDeclaration],
            type_ids: &[(
                typed_trees::type_identity::TypeIdentityId,
                semantic_vocabulary::StructuralTypeId,
            )],
        ) -> Result<Vec<TerminalTaskTargetLayout>, Vec<Diagnostic>> {
            let failure = |message: &'static str| vec![Diagnostic::error(message)];
            let mut layouts = Vec::new();
            for record in types {
                let StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::TargetLayout(id),
                    ..
                } = &record.shape
                else {
                    continue;
                };
                let identity = type_ids
                    .iter()
                    .find_map(|(identity, candidate)| {
                        (*candidate == record.id).then_some(*identity)
                    })
                    .ok_or_else(|| failure("task layout lost its exact checked type"))?;
                let source =
                    selected_declaration(plans, identity).map_err(super::lowering_diagnostics)?;
                let CheckedUnitStructuralTypeShape::Record {
                    storage: CheckedUnitRecordStorage::TargetLayout { application, .. },
                    ..
                } = &source.shape
                else {
                    return Err(failure("task layout lost its exact checked application"));
                };
                let plan = checked
                    .typed
                    .plan_laid_layout_for_type(*application)
                    .ok_or_else(|| failure("task layout lost its evaluated policy"))?;
                let typed_trees::PlanLaidRealizations::TargetInputs(span) = plan.realizations
                else {
                    return Err(failure("task target layout names an invariant policy"));
                };
                let alternatives = checked
                    .typed
                    .plan_laid_target_layouts
                    .span(span)
                    .ok_or_else(|| failure("task layout lost its target alternatives"))?;
                if alternatives.is_empty() && !plan.target_inputs.is_empty() {
                    return Err(failure("task layout has no complete target input roster"));
                }
                let mut realizations = Vec::new();
                for alternative in alternatives {
                    let geometry = &alternative.geometry;
                    let byte_size = geometry
                        .validated_layout
                        .size
                        .ok_or_else(|| failure("task layout alternative has no fixed extent"))?;
                    realizations.push(TerminalTaskLayoutRealization {
                        target: alternative.target.as_str().to_owned(),
                        storage: semantic_vocabulary::AuthoredRecordStorage {
                            byte_size,
                            byte_alignment: geometry.validated_layout.align,
                        },
                    });
                }
                realizations.sort_by(|left, right| left.target.cmp(&right.target));
                layouts.push(TerminalTaskTargetLayout {
                    id: *id,
                    record: record.id,
                    realizations,
                });
            }
            layouts.sort_by_key(|layout| layout.id);
            Ok(layouts)
        }
    }

    use crate::unit::attached_unit::{
        bodies::UnitPlans, catalog::lower_unit_structural_type_roots,
    };
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use terminal_psi::*;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub fn lower_task_activation_demands(
        checked: &CheckedTrees,
        identities: &mut TerminalIdentityTableBuilder,
    ) -> Result<TerminalTaskActivationDemands, Vec<Diagnostic>> {
        let typed = &checked.typed;
        let selections = start_selections::task_start_selections(checked)?;
        if selections.is_empty() {
            return Ok(TerminalTaskActivationDemands::default());
        }
        let mut sources = Vec::new();
        let mut roots = Vec::new();
        for selection in selections {
            let (machine, entry) = start_selections::exact_task_activation_target(
                typed,
                selection.target_machine,
                selection.target_entry,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "TaskRuntime start specialization has an invalid retained target coordinate: {}",
                    error.message()
                ))]
            })?;
            if !typed.machine_type_parameters(machine).is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "task activation target `{}` is still generic after specialization",
                    machine.name
                ))]);
            }
            let graph =
                stack_graphs::task_call_graph(typed, &checked.facts, identities, machine, entry)?;
            for reference in typed
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .chain(std::iter::once(entry.return_type))
                .chain(
                    graph
                        .frames
                        .iter()
                        .flat_map(|frame| &frame.states)
                        .flat_map(|state| {
                            state
                                .resident
                                .iter()
                                .chain(state.crossing_arguments.iter().flatten())
                        })
                        .copied(),
                )
            {
                if scalar_or_unit(checked, reference)?.is_none() {
                    roots.push(validation::stored_structural_type_identity(
                        typed,
                        reference,
                        &[],
                        &[],
                    ));
                }
            }
            sources.push((selection, machine, entry, graph));
        }
        roots.sort();
        roots.dedup();
        let plans = UnitPlans::published(&checked.facts.flow);
        let (structural_types, type_ids, structural_referents, _native_records) =
            lower_unit_structural_type_roots(
                identities,
                checked,
                &crate::unit::attached_unit::scalar_applications::SourceScalarApplications::EMPTY,
                plans,
                &roots,
            )
            .map_err(lowering_diagnostics)?;
        let target_layout_applications =
            target_layouts::project(checked, plans, &structural_types, &type_ids)?;
        let mut demands = Vec::new();
        for (selection, machine, entry, graph) in sources {
            let (owner, _, authored) =
                runtime_requirements::exact_task_runtime_requirement(typed, &selection)?;
            let requirement = typed
                .trait_machine_signature_by_symbol(owner, selection.requirement)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task requirement lost its closed signature",
                    )]
                })?;
            let contract = specialization_commitments::exact_task_machine_contract(
                &checked.facts,
                machine.symbol,
                machine.name.as_str(),
            )?;
            let specialization_commitment =
                specialization_commitments::task_specialization_commitment(
                    typed,
                    &checked.facts,
                    &selection,
                    machine,
                    entry,
                    contract,
                )?;
            let mut crossings = graph
                .crossings
                .iter()
                .map(|crossing| carry_crossings::canonical_suspension_crossing(typed, crossing))
                .collect::<Result<Vec<_>, _>>()?;
            crossings.sort_by_key(|crossing| crossing.identity);
            crossings.dedup();
            let mut frames = Vec::new();
            for frame in graph.frames {
                let mut states = Vec::new();
                for state in frame.states {
                    let resident = state
                        .resident
                        .into_iter()
                        .map(|reference| resident_value(checked, identities, &type_ids, reference))
                        .collect::<Result<Vec<_>, _>>()?;
                    let crossing_arguments = state
                        .crossing_arguments
                        .into_iter()
                        .map(|arguments| {
                            arguments
                                .into_iter()
                                .map(|reference| {
                                    resident_value(checked, identities, &type_ids, reference)
                                })
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    states.push(TerminalTaskFrameState {
                        state: state.state,
                        resident,
                        crossing_arguments,
                    });
                }
                frames.push(TerminalTaskFrame {
                    identity: frame.identity,
                    machine_callable_identity: frame.machine_callable_identity,
                    entry: frame.entry,
                    states,
                    calls: frame.calls,
                    unresolved_calls: frame.unresolved_calls,
                });
            }
            frames.sort_by_key(|frame| frame.identity);
            let signature_arguments = typed
                .state_parameters(entry)
                .iter()
                .map(|parameter| {
                    let value =
                        resident_value(checked, identities, &type_ids, parameter.type_reference)?;
                    Ok(TerminalTaskSignatureValue {
                        value_type: value.value_type,
                        normalized_identity: value.normalized_identity,
                        is_mutable: parameter.is_mutable,
                    })
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            let outcome = resident_value(checked, identities, &type_ids, entry.return_type)?;
            demands.push(TerminalTaskActivationDemand {
                operation: selection.operation,
                report_fingerprint: selection.report_fingerprint,
                requirement_callable_identity: identities
                    .intern(&typed.trait_requirement_callable_identity_text(owner, requirement)),
                requirement_owner_path: identities.intern(&typed.trait_declaration_path(owner)),
                requirement_owner_package: typed.symbols.symbol_package_identity(owner.symbol),
                requirement_in_scope: typed
                    .symbols
                    .symbol_provenance_source_span(owner.symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product),
                authored_requirement_identity: identities
                    .intern(&typed.callable_identity_text(authored)),
                machine_callable_identity: identities.intern(
                    &typed
                        .machine_callable_identity_text(machine)
                        .ok_or_else(|| {
                            vec![Diagnostic::error("task target lacks its callable identity")]
                        })?,
                ),
                entry: identities.intern(entry.name.as_str()),
                entry_report_fingerprint: entry_report_fingerprint(typed, machine, entry),
                contract_report_fingerprint: contract.report_fingerprint,
                specialization_commitment,
                specialization_report_fingerprint: selection.report_fingerprint,
                may_suspend: runtime_requirements::exact_task_machine_suspension(
                    &checked.facts,
                    machine.symbol,
                    machine.name.as_str(),
                )?
                .checked_may_suspend,
                may_block: runtime_requirements::exact_task_machine_blocking(
                    &checked.facts,
                    machine.symbol,
                    machine.name.as_str(),
                )?
                .checked_may_block,
                cancellation_required: true,
                signature_arguments,
                signature_outcome: TerminalTaskSignatureValue {
                    value_type: outcome.value_type,
                    normalized_identity: outcome.normalized_identity,
                    is_mutable: false,
                },
                root_frame: graph.root,
                frames,
                crossings,
                carry_obligations: carry_crossings::carry_obligations(graph.carry),
            });
        }
        demands.sort_by_key(|demand| {
            (
                demand.report_fingerprint,
                demand.machine_callable_identity,
                demand.entry,
            )
        });
        Ok(TerminalTaskActivationDemands {
            demands,
            structural_types,
            structural_referents,
            target_layout_applications,
        })
    }

    fn scalar_or_unit(
        checked: &CheckedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<Option<TerminalTaskValueType>, Vec<Diagnostic>> {
        if matches!(
            checked.typed.type_reference_table.type_reference(reference),
            TypeReferenceNode::Unit
        ) {
            return Ok(Some(TerminalTaskValueType::Unit));
        }
        checked
            .typed
            .type_reference_table
            .primitive_type(reference)
            .map(|primitive| {
                crate::emission::scalar_types::terminal_scalar_type(primitive)
                    .map(TerminalTaskValueType::Scalar)
                    .map_err(lowering_diagnostics)
            })
            .transpose()
    }

    fn resident_value(
        checked: &CheckedTrees,
        identities: &mut TerminalIdentityTableBuilder,
        types: &[(
            typed_trees::type_identity::TypeIdentityId,
            semantic_vocabulary::StructuralTypeId,
        )],
        reference: TypeReferenceHandle,
    ) -> Result<TerminalTaskResidentValue, Vec<Diagnostic>> {
        let value_type = match scalar_or_unit(checked, reference)? {
            Some(value_type) => value_type,
            None => TerminalTaskValueType::Structural(
                crate::shared::terminal_identities::lookup_type_id(
                    types,
                    &validation::stored_structural_type_identity(
                        &checked.typed,
                        reference,
                        &[],
                        &[],
                    ),
                )
                .map_err(lowering_diagnostics)?,
            ),
        };
        Ok(TerminalTaskResidentValue {
            value_type,
            normalized_identity: identities
                .intern(&checked.typed.normalized_type_identity_text(reference)),
        })
    }

    fn lowering_diagnostics(error: crate::LoweringError) -> Vec<Diagnostic> {
        vec![Diagnostic::error(format!(
            "task activation type projection failed: {error:?}"
        ))]
    }

    fn entry_report_fingerprint(
        typed: &checked_trees::typed_trees::TypedTrees,
        machine: &checked_trees::machine::Machine,
        entry: &checked_trees::state::State,
    ) -> u64 {
        let mut hash = semantic_vocabulary::Fnv1a::new();
        hash.byte(0x45);
        hash.string_terminated(machine.name.as_str());
        hash.string_terminated(entry.name.as_str());
        for parameter in typed.state_parameters(entry) {
            hash.byte(u8::from(parameter.is_mutable));
            hash.string_terminated(&typed.normalized_type_identity_text(parameter.type_reference));
        }
        hash.byte(0xfe);
        hash.string_terminated(&typed.normalized_type_identity_text(entry.return_type));
        hash.finish_or(1)
    }
}

// Finish provider demand after the complete execution scope is known. Keep
// authored choices separate from the direct and target-family rows added here.
use crate::native_proposal::TerminalNativeRealizationProposal;
use diagnostics::Diagnostic;
use std::collections::BTreeSet;

pub struct PreparedNativeExecution {
    evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    scope: crate::NativeExecutionScope,
    selection: effects::SelectedProviderPlanFacts,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    demands: BTreeSet<String>,
    compiler_builtins: Vec<provider_admission::provider_admission::TerminalCompilerBuiltinProposal>,
    local_adapters: Vec<provider_admission::SelectedProviderAdapter>,
    boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

impl TerminalNativeRealizationProposal {
    pub fn execution_scope(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        components: &[&crate::AdmittedNativeComponentCall<'_>],
    ) -> Result<crate::NativeExecutionScope, Vec<Diagnostic>> {
        let adapters = self.providers().checked_adapters.as_slice();
        if components.iter().any(|component| {
            component.selection().receiver()
                != &terminal_codec::TerminalExecutionIdentity::portable(artifact.view().identity())
        }) {
            return Err(vec![Diagnostic::error(
                "component scope selection names another receiving artifact",
            )]);
        }
        for (position, component) in components.iter().enumerate() {
            if components[..position].iter().any(|earlier| {
                earlier.selection().receiving_boundary()
                    == component.selection().receiving_boundary()
            }) || adapters
                .iter()
                .filter(|adapter| component.replaces_local_adapter(adapter))
                .count()
                != 1
            {
                return Err(vec![Diagnostic::error(
                    "component scope selection has competing or missing exact local adapter coordinates",
                )]);
            }
        }
        let local = adapters
            .iter()
            .filter(|adapter| {
                !components
                    .iter()
                    .any(|component| component.replaces_local_adapter(adapter))
            })
            .cloned()
            .collect::<Vec<_>>();
        let entry = self.program_entry();
        let root = entry.physical_adapter().map_or(
            terminal_psi::TargetMachineCallable::Machine(entry.checked_entry().terminal_entry()),
            |physical| terminal_psi::TargetMachineCallable::Machine(physical.machine()),
        );
        let semantic_call = crate::entry_settlement::bind_selected_program_entry_call(
            artifact,
            entry,
            &self.providers().selection,
            &self.providers().intrinsic_executions,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        self.derive_scope(artifact, root, semantic_call.as_ref(), &local)
    }

    pub fn callable_execution_scope(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TargetMachineCallable,
        previous: Option<&PreparedNativeExecution>,
    ) -> Result<crate::NativeExecutionScope, Vec<Diagnostic>> {
        let adapters = self.providers().checked_adapters.as_slice();
        let local = if let Some(previous) = previous {
            previous
                .scope
                .require_proposal_context(artifact.view(), self)?;
            if previous.scope.target_selections().terminal_psi() != artifact.view().identity()
                || previous.scope.target_selections().target()
                    != self.target_profile().target_name()
            {
                return Err(vec![Diagnostic::error(
                    "native callable preparation belongs to another artifact or target",
                )]);
            }
            previous.local_adapters.as_slice()
        } else {
            adapters
        };
        let mut scope = self.derive_scope(artifact, entry, None, local)?;
        if let Some(previous) = previous {
            scope.bind_completed_provider_selection(previous.selection());
        }
        Ok(scope)
    }

    fn derive_scope(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        root: terminal_psi::TargetMachineCallable,
        entry_call: Option<&crate::SelectedProgramEntryCall>,
        adapters: &[provider_admission::SelectedProviderAdapter],
    ) -> Result<crate::NativeExecutionScope, Vec<Diagnostic>> {
        if self.program_entry().checked_entry().terminal_psi_identity()
            != artifact.view().identity()
        {
            return Err(vec![Diagnostic::error(
                "native proposal belongs to another canonical artifact",
            )]);
        }
        let targets = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            self.target_profile().target_name(),
        );
        let entry = targets
            .entry(artifact.view().identity(), artifact.semantic_module(), root)
            .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        let assumptions = self
            .providers()
            .selection
            .assumed_binding_coordinates(artifact.semantic_module())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut scope = crate::NativeExecutionScope::derive(
            artifact.view(),
            entry,
            &targets,
            entry_call,
            (adapters, &self.providers().selection),
            &assumptions,
            &self.occurrences().callbacks,
        )
        .map_err(|reason| vec![Diagnostic::error(reason.to_string())])?;
        scope.bind_checked_source_entry(self.program_entry().checked_entry());
        Ok(scope)
    }

    pub fn selected_intrinsic_provenance<'a>(
        &'a self,
        selection: &'a effects::SelectedProviderPlanFacts,
    ) -> Result<
        Vec<provider_admission::provider_admission::CompilerIntrinsicPlanProvenance<'a>>,
        Vec<Diagnostic>,
    > {
        static NON_INTRINSIC_EXECUTION: [Option<effects::CompilerIntrinsicExecutionIdentity>; 1] =
            [None];
        static NON_AUTHORED_REALIZATION: [Option<String>; 1] = [None];
        selection
            .plans()
            .iter()
            .map(|plan| -> Result<_, Vec<Diagnostic>> {
                if let Some(position) = self
                    .providers()
                    .selection
                    .plans()
                    .iter()
                    .position(|original| original == plan)
                {
                    let row_executions = self
                        .providers()
                        .intrinsic_executions
                        .get(position)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native execution lost original intrinsic row custody",
                            )]
                        })?;
                    let row_numeric_realizations = self
                        .providers()
                        .numeric_realizations
                        .get(position)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "native execution lost selected external declaration custody",
                            )]
                        })?;
                    return Ok(
                        provider_admission::provider_admission::CompilerIntrinsicPlanProvenance {
                            plan,
                            semantic_schema: &plan.schema,
                            row_executions,
                            row_numeric_realizations,
                        },
                    );
                }
                let leaf = self
                    .providers()
                    .direct_boundary_leaves
                    .iter()
                    .find(|leaf| leaf.plan == *plan)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "native execution has an unowned direct plan",
                        )]
                    })?;
                let row_executions: &[Option<effects::CompilerIntrinsicExecutionIdentity>] =
                    match &leaf.execution {
                        backend_plan::DirectBoundaryExecution::Intrinsic(identity) => {
                            std::slice::from_ref(identity)
                        }
                        backend_plan::DirectBoundaryExecution::Foreign(_) => {
                            &NON_INTRINSIC_EXECUTION
                        }
                    };
                Ok(
                    provider_admission::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan,
                        semantic_schema: &leaf.satisfied_schema,
                        row_executions,
                        row_numeric_realizations: &NON_AUTHORED_REALIZATION,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()
    }
    pub fn settle_scoped_boundary_applications(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        scope: &crate::NativeExecutionScope,
        selection: &effects::SelectedProviderPlanFacts,
        provenance: &[provider_admission::provider_admission::CompilerIntrinsicPlanProvenance<
            '_,
        >],
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let coverage = crate::settlement::boundary_application_settlement::settle(
            artifact,
            selection,
            provenance,
            &self.boundary_applications().calling,
            scope,
        )?;
        crate::settlement::comparison_settlement::settle_comparison_occurrences(
            artifact.semantic_module(),
            selection,
            &coverage,
            scope.machines(),
        )?;
        Ok(coverage)
    }
    pub fn prepare_native_provider_demand(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        mut scope: crate::NativeExecutionScope,
    ) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
        let module = artifact.semantic_module();
        scope.require_proposal_context(artifact.view(), self)?;
        let local_adapters = scope.local_adapters().to_vec();
        let identity = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
            module
                .identities
                .text(boundary.identity)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native execution boundary has no exact identity",
                    )]
                })
        };
        let mut demands = BTreeSet::new();
        let mut direct_demands = BTreeSet::new();
        let mut family_members = Vec::new();
        for boundary in module
            .boundary_machines
            .iter()
            .filter(|boundary| scope.contains_boundary(boundary.id))
        {
            let name = identity(boundary)?;
            demands.insert(name.to_owned());
            if self
                .providers()
                .selection
                .selected_requirement_row(name)
                .is_some()
            {
                continue;
            }
            match terminal_installation::admitted_boundary_member(
                module,
                Some(scope.target_selections()),
                boundary.id,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            {
                Some(terminal_psi::TargetMachineCallable::Boundary(member)) => {
                    let member = module.boundary_machine(member).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "foreign family lost its exact target member",
                        )]
                    })?;
                    direct_demands.insert(identity(member)?.to_owned());
                    family_members.push((boundary, member));
                }
                _ => {
                    direct_demands.insert(name.to_owned());
                }
            }
        }
        let mut direct = Vec::new();
        let mut external_bindings = self.providers().external_bindings.clone();
        for leaf in &self.providers().direct_boundary_leaves {
            if leaf.plan.rows.iter().any(|row| {
                direct_demands.contains(leaf.plan.identities.text(row.requirement_identity))
            }) {
                leaf.validate_bound_calling_custody(module)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                direct.push(leaf.plan.clone());
                if let backend_plan::DirectBoundaryExecution::Foreign(binding) = &leaf.execution {
                    external_bindings.push(binding.clone()?);
                }
            }
        }
        let mut selection = self
            .providers()
            .selection
            .clone()
            .with_direct_boundary_plans(direct)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut family_rows = selection.family_realization_rows().to_vec();
        let mut identities = selection.identities().clone().into_builder();
        for (head, member) in family_members {
            let Some((plan, row)) = selection.selected_requirement_row(identity(member)?) else {
                continue;
            };
            family_rows.push(effects::SelectedTargetFamilyRealization {
                terminal: artifact.manifest().semantic(),
                target: self.target_profile().target_name().to_owned(),
                head: head.id,
                member: member.id,
                head_identity: identities.intern(identity(head)?),
                member_identity: identities.intern(identity(member)?),
                selected_requirement_identity: identities
                    .intern(plan.identities.text(row.requirement_identity)),
                provider_plan_report_identity: plan.report_fingerprint(),
                provider_plan_digest: plan.identity_digest(),
            });
        }
        selection = selection
            .with_family_realization_rows(family_rows, &identities.build())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let intrinsic_provenance = self.selected_intrinsic_provenance(&selection)?;
        let boundary_application_coverage = self.settle_scoped_boundary_applications(
            artifact,
            &scope,
            &selection,
            &intrinsic_provenance,
        )?;
        let compiler_builtins =
            provider_admission::provider_admission::derive_selected_intrinsic_settlement_proposals(
                &selection,
                intrinsic_provenance.into_iter(),
                &demands,
                &module.boundary_machines,
                &module.external_providers,
                &module.identities,
            )?;
        let demands = demands
            .into_iter()
            .map(|identity| {
                selection
                    .selected_requirement_identity(&identity)
                    .to_owned()
            })
            .collect();
        if std::env::var_os("OMEGA_TRACE_NATIVE_BOUNDARY").is_some() {
            // A missing settlement can originate before native admission. Show
            // the retained choices without changing which boundaries are used.
            let retained_boundary_count = module
                .boundary_machines
                .iter()
                .filter(|boundary| scope.contains_boundary(boundary.id))
                .count();
            let mut text = format!(
                "prepared native boundaries: entry={:?} total={} in_scope={} direct_leaves={} builtins={}\n",
                scope.entry(),
                module.boundary_machines.len(),
                retained_boundary_count,
                self.providers().direct_boundary_leaves.len(),
                compiler_builtins.len(),
            );
            for boundary in module.boundary_machines.iter().take(16) {
                let retained = scope.contains_boundary(boundary.id);
                let name = module.identities.text(boundary.identity);
                let selected =
                    name.is_some_and(|name| selection.selected_requirement_row(name).is_some());
                let leaves = self
                    .providers()
                    .direct_boundary_leaves
                    .iter()
                    .filter(|leaf| {
                        leaf.plan.rows.iter().any(|row| {
                            name == Some(leaf.plan.identities.text(row.requirement_identity))
                        })
                    })
                    .count();
                let builtin = compiler_builtins
                    .iter()
                    .any(|row| row.boundary() == boundary.id);
                text.push_str(&format!(
                    "  id={:?} in_scope={retained} direct_leaves={leaves} selected={selected} builtin={builtin}\n",
                    boundary.id,
                ));
            }
            text.push_str(&format!(
                "prepared native boundaries omitted={}\n",
                module.boundary_machines.len().saturating_sub(16),
            ));
            // Every field is a bounded numeric identity, count or boolean; also
            // cap the assembled record if an internal Debug spelling changes.
            let mut end = text.len().min(4096);
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            eprint!("{}", &text[..end]);
        }
        scope.bind_completed_provider_selection(&selection);
        Ok(PreparedNativeExecution {
            evidence: None,
            scope,
            selection,
            external_bindings,
            demands,
            compiler_builtins,
            local_adapters,
            boundary_application_coverage,
        })
    }
}

impl PreparedNativeExecution {
    pub fn with_operation_evidence(
        mut self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if let Some(evidence) = &evidence {
            crate::native_proposal::validate_operation_evidence(artifact, evidence)?;
        }
        self.evidence = evidence;
        Ok(self)
    }

    pub const fn evidence(&self) -> Option<&std::sync::Arc<terminal_codec::EvidenceSeals>> {
        self.evidence.as_ref()
    }

    pub fn selection(&self) -> &effects::SelectedProviderPlanFacts {
        &self.selection
    }
    pub fn source_selection(&self) -> &effects::SelectedProviderPlanFacts {
        self.scope.source_provider_selection()
    }
    pub const fn scope(&self) -> &crate::NativeExecutionScope {
        &self.scope
    }
    pub fn external_bindings(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.external_bindings
    }
    pub fn demands(&self) -> &BTreeSet<String> {
        &self.demands
    }
    pub fn compiler_builtins(
        &self,
    ) -> &[provider_admission::provider_admission::TerminalCompilerBuiltinProposal] {
        &self.compiler_builtins
    }
    pub fn local_adapters(&self) -> &[provider_admission::SelectedProviderAdapter] {
        &self.local_adapters
    }
    pub const fn boundary_application_coverage(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationCoverage {
        &self.boundary_application_coverage
    }
}

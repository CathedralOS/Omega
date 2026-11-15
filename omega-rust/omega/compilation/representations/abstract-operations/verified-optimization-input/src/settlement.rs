/*
  Native evidence settlements against produced Terminal artifacts
  (`boundary_application_settlement`, `comparison_settlement`,
  `callback_settlement`, `entry_settlement`).

  Selects boundary realizations from canonical Terminal demands and Build rows,
  binds integer and IEEE float comparison occurrences to settled executions, and
  delegates callback and program-entry sidecar validation.
*/
pub mod boundary_application_settlement {
    use diagnostics::Diagnostic;

    pub struct BoundaryApplicationSettlement<'input> {
        pub selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
        pub selected_provider_provenance:
            Vec<provider_admission::provider_admission::CompilerIntrinsicPlanProvenance<'input>>,
        pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    }

    impl BoundaryApplicationSettlement<'_> {
        pub fn settle(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            scope: &crate::NativeExecutionScope,
        ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>>
        {
            settle(
                artifact,
                self.selected_provider_plans,
                &self.selected_provider_provenance,
                &self.calling_applications,
                &provider_admission::RetainedNativeBindings::default(),
                None,
                scope,
            )
        }
    }

    pub fn settle(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        selected: &effects::SelectedProviderPlanFacts,
        provenance: &[provider_admission::provider_admission::CompilerIntrinsicPlanProvenance<
            '_,
        >],
        calling: &[calling_conventions::BoundaryCallingApplication],
        native_bindings: &provider_admission::RetainedNativeBindings,
        profile: Option<target::TargetProfile>,
        scope: &crate::NativeExecutionScope,
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let demands = boundary_applications::TerminalBoundaryApplicationDemands::new(
            artifact.manifest().semantic(),
            scope
                .boundary_application_demands(artifact.semantic_module())
                .map_err(|message| vec![Diagnostic::error(message)])?,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let provenance = provenance
            .iter()
            .map(|row| provider_selection::CompilerIntrinsicPlanProvenance {
                plan: row.plan,
                semantic_schema: row.semantic_schema,
                row_executions: row.row_executions,
            })
            .collect::<Vec<_>>();
        let module = artifact.semantic_module();
        let mut provider_demands = Vec::new();
        let mut native_realizations = Vec::new();
        for demand in demands.rows() {
            let mut candidates = native_bindings.rows.iter().filter(|row| {
                module
                    .boundary_machine(row.boundary)
                    .and_then(|boundary| boundary.declaration.as_ref())
                    .is_some_and(|declaration| {
                        declaration.source.state.same_identity_as(
                            &module.identities,
                            demand.requirement().declaration(),
                            &module.identities,
                        )
                    })
            });
            let first = candidates.next();
            if candidates.next().is_some() {
                return Err(vec![Diagnostic::error(
                    "native boundary application has competing receipts",
                )]);
            }
            let Some(row) = first else {
                provider_demands.push(demand.clone());
                continue;
            };
            if !matches!(
                demand.application(),
                boundary_applications::BoundaryApplication::Empty
            ) {
                return Err(vec![Diagnostic::error(
                    "native binding application needs its complete applied requirement",
                )]);
            }
            let abi = module
                .calling_abi_identity(row.boundary)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let mut applications = calling.iter().filter(|application| application.abi == abi);
            let application = applications
                .next()
                .filter(|_| applications.next().is_none())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native boundary application has no unique calling application",
                    )]
                })?;
            let profile = profile.ok_or_else(|| {
                vec![Diagnostic::error(
                    "native boundary application has no target",
                )]
            })?;
            let execution = provider_admission::validate_native_binding_execution(
                row,
                &native_bindings.identities,
                module,
                profile,
                application,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            native_realizations.push(
                boundary_applications::BoundaryApplicationRealizationCompanion::native_binding(
                    demand.terminal_operation(),
                    execution.receipt(),
                    execution.contract(),
                    execution.calling(),
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?,
            );
        }
        let provider_demands = boundary_applications::TerminalBoundaryApplicationDemands::new(
            demands.terminal_psi(),
            provider_demands,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let provider_realizations = provider_selection::select_boundary_applications(
            &provenance,
            selected,
            module,
            &provider_demands,
        )?;
        native_realizations.extend_from_slice(provider_realizations.rows());
        let realizations = boundary_applications::TerminalBoundaryApplicationRealizations::new(
            &demands,
            native_realizations,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let opaque_applications = project_opaque_applications(calling, artifact.semantic_module())
            .map_err(|message| vec![Diagnostic::error(message)])?;
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .map_err(|message| vec![Diagnostic::error(message)])
            .map(|coverage| coverage.with_opaque_applications(opaque_applications))
    }

    fn project_opaque_applications(
        calling_applications: &[calling_conventions::BoundaryCallingApplication],
        module: &terminal_psi::TerminalModule,
    ) -> Result<boundary_applications::BoundaryOpaqueRepresentationApplications, &'static str> {
        let rows = calling_applications
        .iter()
        .flat_map(|application| {
            application.opaque_applications.iter().map(|use_| {
                module
                    .identities
                    .index_of(&application.requirement_identity)
                    .ok_or(
                        "boundary opaque application names a requirement absent from the module identity table",
                    )
                    .map(|requirement_identity| {
                        boundary_applications::BoundaryOpaqueRepresentationApplication {
                            abi: application.abi.clone(),
                            requirement_identity,
                            shape_root: use_.shape_root,
                            application_report_fingerprint: use_.application_report_fingerprint,
                            selected_application_commitment: use_
                                .selected_application_commitment,
                        }
                    })
            })
        })
        .collect::<Result<_, _>>()?;
        boundary_applications::BoundaryOpaqueRepresentationApplications::new(rows)
    }
}
pub use crate::callback_settlement;
pub mod comparison_settlement {
    use boundary_applications::{
        BoundaryApplicationRealization, TerminalBoundaryApplicationCoverage,
        TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
    };
    use diagnostics::Diagnostic;
    use effects::{CompilerIntrinsicExecutionIdentity, CompilerPrimitiveFloatBinaryOperation};
    use semantic_vocabulary::{IeeeFloatComparisonOperation, IeeeFloatFormat};

    pub struct SelectedComparisonOccurrences {
        pub ieee_float: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
        pub integer: Vec<TerminalIntegerComparisonOccurrenceProposal>,
    }

    pub fn settle_comparison_occurrences(
        module: &terminal_psi::TerminalModule,
        selected: &effects::SelectedProviderPlanFacts,
        coverage: &TerminalBoundaryApplicationCoverage,
        execution_machines: &[semantic_vocabulary::MachineId],
    ) -> Result<SelectedComparisonOccurrences, Vec<Diagnostic>> {
        let plans = selected.plans();
        let mut plan_indices = plans
            .iter()
            .enumerate()
            .map(|(index, plan)| (*plan.identity_digest().as_bytes(), index))
            .collect::<Vec<_>>();
        plan_indices.sort_unstable_by_key(|(identity, _)| *identity);
        let realizations = coverage.realizations().rows();
        let mut occurrences = SelectedComparisonOccurrences {
            ieee_float: Vec::new(),
            integer: Vec::new(),
        };
        for machine in module
            .machines
            .iter()
            .filter(|machine| execution_machines.binary_search(&machine.id).is_ok())
        {
            for operation in machine.operations() {
                use terminal_psi::OperationKind;
                if !matches!(
                    operation.kind,
                    OperationKind::IeeeFloatCompare { .. }
                        | OperationKind::IntegerEqual { .. }
                        | terminal_psi::integer_less_or_equal_pair_pattern!(..)
                ) {
                    continue;
                }
                let fail = |reason: &str| {
                    vec![Diagnostic::error(format!(
                        "Terminal comparison {}: {reason}",
                        operation.id.get()
                    ))]
                };
                let Ok(row_index) = realizations.binary_search_by_key(&operation.id.get(), |row| {
                    row.terminal_operation().get()
                }) else {
                    if matches!(operation.kind, OperationKind::IeeeFloatCompare { .. }) {
                        return Err(fail("has no selected boundary execution"));
                    }
                    continue;
                };
                let row = &realizations[row_index];
                let identity = row.selected_plan_digest().ok_or_else(|| {
                    fail("comparison requires a checked or intrinsic provider plan")
                })?;
                let position = plan_indices.partition_point(|(digest, _)| digest < identity);
                let Some((digest, provider_plan_index)) = plan_indices.get(position) else {
                    return Err(fail("selected execution names an absent provider plan"));
                };
                if digest != identity
                    || plan_indices
                        .get(position + 1)
                        .is_some_and(|(next, _)| next == identity)
                {
                    return Err(fail(
                        "selected execution does not name one exact provider plan",
                    ));
                }
                let provider_plan_index = *provider_plan_index;
                let provider_plan_commitment = plans[provider_plan_index].identity_digest();
                let BoundaryApplicationRealization::ExactCompilerIntrinsic { execution } =
                    row.realization()
                else {
                    return Err(fail("requires an exact comparison execution"));
                };
                match (&operation.kind, execution) {
                    (
                        OperationKind::IeeeFloatCompare { comparison, .. },
                        CompilerIntrinsicExecutionIdentity::PrimitiveFloatBinary {
                            operation: selected_operation,
                            format,
                        },
                    ) => {
                        use CompilerPrimitiveFloatBinaryOperation as Selected;
                        let selected_comparison = match selected_operation {
                            Selected::Equal => IeeeFloatComparisonOperation::Equal,
                            Selected::NotEqual => IeeeFloatComparisonOperation::NotEqual,
                            Selected::Less => IeeeFloatComparisonOperation::Less,
                            Selected::LessOrEqual => IeeeFloatComparisonOperation::LessOrEqual,
                            Selected::Greater => IeeeFloatComparisonOperation::Greater,
                            Selected::GreaterOrEqual => {
                                IeeeFloatComparisonOperation::GreaterOrEqual
                            }
                            _ => return Err(fail("selected execution is not a float comparison")),
                        };
                        if *comparison != selected_comparison {
                            return Err(fail(
                                "selected float execution changes comparison meaning",
                            ));
                        }
                        occurrences.ieee_float.push(
                            TerminalIeeeFloatComparisonOccurrenceProposal {
                                terminal_machine: machine.id,
                                terminal_operation: operation.id,
                                provider_plan_index,
                                provider_plan_commitment,
                                comparison: *comparison,
                                format: match format {
                                    numerics::literals::FloatFormat::F32 => {
                                        IeeeFloatFormat::Binary32
                                    }
                                    numerics::literals::FloatFormat::F64 => {
                                        IeeeFloatFormat::Binary64
                                    }
                                },
                            },
                        );
                    }
                    (
                        OperationKind::IntegerEqual { .. }
                        | terminal_psi::integer_less_or_equal_pair_pattern!(..),
                        CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                            operation: comparison,
                            integer_type,
                        },
                    ) => {
                        occurrences
                            .integer
                            .push(TerminalIntegerComparisonOccurrenceProposal {
                                terminal_machine: machine.id,
                                terminal_operation: operation.id,
                                provider_plan_index,
                                provider_plan_commitment,
                                comparison: *comparison,
                                integer_type: *integer_type,
                            });
                    }
                    _ => return Err(fail("selected execution has the wrong comparison family")),
                }
            }
        }
        TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster(
            module,
            plans,
            &occurrences.ieee_float,
            execution_machines,
        )
        .and_then(|()| {
            TerminalIntegerComparisonOccurrenceProposal::validate_roster(
                module,
                plans,
                &occurrences.integer,
                execution_machines,
            )
        })
        .map_err(|message| vec![Diagnostic::error(message)])?;
        Ok(occurrences)
    }
}
pub use crate::entry_settlement;

//! Execute a source-authenticated call using the common Build interpreter and its actual sponsor.
use super::target::{InvocationTarget, complete_invocation_scalar_applications};
use super::{PreparedBuildMachineEntry, source_selection};
use crate::compile_time_invocations::{BuildTimeValue, SourceCompletionAccounting};
use checked_program_to_psi as production;
use source_completion_accounting::RecordedSourceInvocation;
use std::sync::Arc;

impl PreparedBuildMachineEntry {
    pub fn invoke_recorded(
        &self,
        target: InvocationTarget<'_>,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<BuildTimeValue, String> {
        let produced = Arc::new(self.produce_invocation()?);
        self.invoke_produced_recorded(produced, target, accounting)
    }

    fn invoke_produced_recorded(
        &self,
        produced: Arc<production::ProducedTerminalProgram>,
        target: InvocationTarget<'_>,
        accounting: &mut SourceCompletionAccounting,
    ) -> Result<BuildTimeValue, String> {
        self.invoke_with_observations(produced, target, accounting, &[], |value, _| Ok(value))
    }

    /// Record both execution and source-specific interpretation of any requested observations.
    pub(crate) fn invoke_with_observations<T>(
        &self,
        produced: Arc<production::ProducedTerminalProgram>,
        target: InvocationTarget<'_>,
        accounting: &mut SourceCompletionAccounting,
        observed_calls: &[terminal_interpreter::TerminalCallObservation],
        complete: impl FnOnce(
            BuildTimeValue,
            Vec<terminal_interpreter::ObservedTerminalCall>,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        let (admitted, requirements) = self.admit_invocation(&produced, target)?;
        let sponsor = pure_terminal_invocation_account()?;
        let mut meter = build_model::work::sponsored_meter(sponsor, None)
            .map_err(|error| format!("source invocation meter: {error:?}"))?;
        let run = self.execute_invocation(
            &produced,
            &admitted,
            requirements,
            target,
            &mut meter,
            observed_calls,
        );
        RecordedSourceInvocation::record(
            self.source_invocation_inputs(),
            &produced,
            admitted,
            &meter,
            run,
        )?
        .finish(accounting, |(value, events)| complete(value, events))
    }

    pub fn invoke_recorded_in_targets(
        &self,
        targets: &[(&str, Option<&layout_plans::LayoutTargetFacts>)],
        accounting: &mut crate::compile_time_invocations::SourceCompletionAccounting,
    ) -> Result<Vec<crate::compile_time_invocations::BuildTimeValue>, String> {
        let produced = Arc::new(self.produce_invocation()?);
        let module = produced.view().module();
        if !Self::uses_target_layout(module) {
            let target = InvocationTarget {
                name: None,
                layout_target: None,
                carrier_layout: None,
                address_model: None,
            };
            return Ok(vec![
                self.invoke_produced_recorded(produced, target, accounting)?,
            ]);
        }
        if targets.is_empty() {
            return Err(
                "the computation stores through an authored layout, and no deployment \
                 that reaches it selects that layout"
                    .into(),
            );
        }
        let mut values = Vec::with_capacity(targets.len());
        for &(name, facts) in targets {
            let facts = facts.ok_or_else(|| format!("deployment `{name}` has no complete layout facts for the layout it stores through"))?;
            let target = InvocationTarget {
                name: Some(name),
                layout_target: Some(facts),
                carrier_layout: None,
                address_model: None,
            };
            values.push(self.invoke_produced_recorded(
                Arc::clone(&produced),
                target,
                accounting,
            )?);
        }
        Ok(values)
    }

    fn admit_invocation(
        &self,
        produced: &production::ProducedTerminalProgram,
        target: InvocationTarget<'_>,
    ) -> Result<
        (
            build_model::admission::AdmittedCompilerEvaluation,
            terminal_interpreter::TerminalExecutionRequirements,
        ),
        String,
    > {
        if target.name.is_none()
            && (target.layout_target.is_some()
                || target.carrier_layout.is_some()
                || target.address_model.is_some())
        {
            return Err("target-independent evaluation cannot carry target observations".into());
        }
        for value in self.input_values.iter() {
            if let crate::compile_time_invocations::BuildTimeValue::LayoutTarget(facts) = value
                && (target.name != Some(facts.profile.as_str())
                    || target.layout_target != Some(facts))
            {
                return Err(
                    "issued layout context differs from the actual invocation target".into(),
                );
            }
        }
        let mut authorities = psi_authorization::selection_questions::SelectionAuthorities {
            build: None,
            product: None,
        };
        match self.purpose {
            source_provenance::DependencyScope::Build => {
                authorities.build = self.authority.as_deref().map(|scope| scope as _)
            }
            source_provenance::DependencyScope::Product => {
                authorities.product = self.authority.as_deref().map(|scope| scope as _)
            }
        }
        produced
            .source_selections()
            .ok_or("source invocation lost its declaration selections")?
            .admit(produced.view(), &authorities)?;
        use terminal_interpreter::TerminalProgram;
        let program = TerminalProgram::borrow(produced.view());
        let unselected =
            psi_provider_selections::admit_provider_execution_installation(produced.view(), &[])
                .map_err(|error| format!("source invocation provider admission: {error:?}"))?;
        let layouts = Self::layout_supplies(produced, &unselected, target)?;
        let requirements = program
            .execution_requirements(layouts.as_ref())
            .map_err(|error| format!("source invocation demands: {error:?}"))?;
        if !requirements.imports.is_empty()
            || !requirements.missing_providers.is_empty()
            || !requirements.external_completions.is_empty()
            || (target.layout_target.is_none()
                && (!requirements.scalar_applications.is_empty()
                    || !requirements.scalar_entry_conditions.is_empty()))
        {
            return Err("source invocation requires selected target, scalar or external authority that is not supplied".into());
        }
        let view = produced.view();
        let selected = view
            .module()
            .entry
            .ok_or("source invocation lost its emitted root")?;
        let scope = &view.module().scalars.scalar_applications.proof_scope;
        let root = scope
            .callable_declaration(selected)
            .ok_or("source invocation has no exact proof declaration")?;
        let mut headers = scope
            .callable_headers
            .iter()
            .filter_map(|(_, header)| (header.declaration == root).then_some(header));
        let header = headers
            .next()
            .ok_or("source invocation lost its accepted root header")?;
        if headers.next().is_some() || !header.supplies_checked_body {
            return Err("source invocation has no unique accepted caller body".into());
        }
        let requester = self.source.symbols.declaration_custody(self.requester);
        let source_selection = source_selection::InvocationSelection::new(self, view);
        let admitted = build_model::admission::admit_compiler_evaluation_with_source_consent(
            view,
            target.name,
            root,
            unselected,
            requester,
            self.authority.as_deref().map(|scope| scope as _),
            source_selection.as_ref().map(|selection| {
                selection as &dyn build_model::admission::InvocationSourceSelection
            }),
        )?;
        admitted.require_invocation(view, target.name, selected)?;
        Ok((admitted, requirements))
    }

    /// A storage application with an authored layout has bytes only under one
    /// deployment's admitted plan. Install that target's checked layouts so
    /// the invocation runs with the geometry its target closure selected;
    /// without layout facts the program keeps no layout supply and refuses
    /// at its first layout demand.
    fn layout_supplies(
        produced: &production::ProducedTerminalProgram,
        providers: &psi_provider_selections::AdmittedProviderSelections,
        target: InvocationTarget<'_>,
    ) -> Result<Option<terminal_interpreter::TerminalRuntimeSupplies>, String> {
        let view = produced.view();
        let (Some(name), Some(facts)) = (target.name, target.layout_target) else {
            return Ok(None);
        };
        if view.module().layouts.target_layout_applications.is_empty() {
            return Ok(None);
        }
        let selected = psi_target_layouts::terminal_target_context(view, name);
        let layouts = psi_target_layouts::admit_target_layouts(view, &selected, facts)
            .map_err(|error| format!("source invocation target layouts: {error:?}"))?;
        Self::runtime_supplies(view, providers, Some(&selected), Some(&layouts)).map(Some)
    }

    fn runtime_supplies(
        view: terminal_codec::TerminalExecutionView<'_>,
        providers: &psi_provider_selections::AdmittedProviderSelections,
        selected: Option<&psi_target_layouts::TerminalTargetContext>,
        layouts: Option<&psi_target_layouts::AdmittedTargetLayouts>,
    ) -> Result<terminal_interpreter::TerminalRuntimeSupplies, String> {
        terminal_interpreter::TerminalProgram::borrow(view)
            .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
                native_code_layouts: None,
                x86_flags: None,
                memory_ordering: None,
                entry: None,
                installation: Some(providers),
                host_imports: &[],
                external_completions: &[],
                target_machines: selected,
                target_layouts: layouts,
            })
            .map_err(|error| format!("source invocation target supplies: {error:?}"))
    }

    fn execute_invocation(
        &self,
        produced: &production::ProducedTerminalProgram,
        admitted: &build_model::admission::AdmittedCompilerEvaluation,
        requirements: terminal_interpreter::TerminalExecutionRequirements,
        target: InvocationTarget<'_>,
        meter: &mut build_model::work::EvaluationWorkMeter,
        observed_calls: &[terminal_interpreter::TerminalCallObservation],
    ) -> Result<build_time_evaluation::invocations::InvocationOutcome, String> {
        let selected = produced
            .view()
            .module()
            .entry
            .ok_or("source invocation lost its emitted root")?;
        admitted.require_invocation(produced.view(), admitted.target(), selected)?;
        let view = produced.view();
        let providers = admitted.providers();
        let mut requested = requirements.scalar_applications;
        requested.extend(requirements.scalar_entry_conditions);
        requested.sort_unstable();
        requested.dedup();
        let scalars = if requested.is_empty() {
            None
        } else {
            let name = target
                .name
                .ok_or("source scalar application has no deployment")?;
            let targets = psi_target_layouts::terminal_target_context(view, name);
            let declaration = view
                .module()
                .scalars
                .scalar_applications
                .proof_scope
                .callable_declaration(selected)
                .ok_or("source scalar application has no exact invocation root")?;
            Some(complete_invocation_scalar_applications(
                view,
                target,
                &targets,
                providers,
                declaration,
                &requested,
                meter,
            )?)
        };
        let installation = if let Some(scalars) = &scalars {
            let scope = scalars.primary();
            Self::runtime_supplies(view, scope.providers(), scope.targets(), scope.layouts())?
        } else {
            Self::layout_supplies(produced, providers, target)?
                .unwrap_or_else(|| terminal_interpreter::TerminalRuntimeSupplies::from(providers))
        };
        Ok(
            build_time_evaluation::invocations::ArtifactInvocation::bind(
                build_time_evaluation::invocations::InvocationInput {
                    code: view,
                    admission: admitted,
                    target: target.name,
                    layout_target: target.layout_target,
                    arguments: &self.arguments,
                    inputs: &self.input_values,
                    supplies: &installation,
                    scalars: scalars.as_ref(),
                    observed_calls,
                },
                meter,
            )?
            .execute(),
        )
    }
}
pub(crate) fn pure_terminal_invocation_account()
-> Result<build_model::accounting::EvaluationInvocationAccount, String> {
    let limits = build_model::contracts::BuildEvaluationSponsorLimits::new(
        build_time_evaluation::scalars::PURE_PSI_WORK_BUDGET,
        1,
        1,
        1,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    )
    .ok_or("pure invocation limits are invalid")?;
    let measurement = build_model::contracts::EvaluationMeasurementIdentity::TerminalPsi {
        work_schedule: terminal_work::WorkSchedule::CURRENT.identity(),
        storage_schedule: build_model::contracts::TerminalStorageScheduleIdentity::CURRENT,
    };
    build_model::accounting::BuildEvaluationSponsor::new_with_measurement(limits, measurement)
        .map_err(|error| format!("pure invocation sponsor: {error:?}"))?
        .begin_invocation(
            measurement,
            build_time_evaluation::scalars::PURE_PSI_WORK_BUDGET,
        )
        .map_err(|error| format!("pure invocation account: {error:?}"))
}

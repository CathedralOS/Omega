//! The retained Terminal product and the target-constrained proposal that has to
//! match it occurrence for occurrence - every boundary demand, callback and
//! nearest-FMA site - before native realization is allowed to begin.

pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};

/// One compiler-owned builtin proposed beside a target-neutral Terminal
/// artifact. The local lowerer still has to rejoin this row to the exact
/// selected plan, Terminal demand, and its own target catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompilerBuiltinProposal {
    requirement_identity: String,
    provider_plan_index: usize,
    execution: target_operations::CompilerBuiltinExecution,
}

impl TerminalCompilerBuiltinProposal {
    pub fn new(
        requirement_identity: String,
        provider_plan_index: usize,
        execution: target_operations::CompilerBuiltinExecution,
    ) -> Result<Self, &'static str> {
        if requirement_identity.is_empty() {
            return Err("Terminal compiler-builtin proposal has an empty requirement identity");
        }
        Ok(Self {
            requirement_identity,
            provider_plan_index,
            execution,
        })
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_index(&self) -> usize {
        self.provider_plan_index
    }

    pub const fn execution(&self) -> target_operations::CompilerBuiltinExecution {
        self.execution
    }
}

pub use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};

/// Exact target-constrained proposal retained beside a target-neutral
/// Terminal artifact.
///
/// This owns full selected plans and external-binding rows rather than a
/// compact report fingerprint. It grants no provider execution, installation,
/// proof admission, optimization, or publication authority. It does retain the
/// build's exact pending physical-optimization proposal; a later consumer must
/// accept that proposal exactly and independently replay it, or reject native
/// realization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalNativeRealizationProposal {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    target_profile: target::TargetProfile,
    native_target: target::NativeTarget,
    subsystem: u16,
    application_intent: Option<build_evaluation::HostedApplicationIntent>,
    application_identifier: Option<build_evaluation::ApplicationIdentifier>,
    /// The validated authored `builder.application` name, retained beside the
    /// intent and identifier. It supplies the publication `.app` basename and
    /// inner executable leaf; `None` means the build declared no application.
    application_name: Option<String>,
    post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    program_entry: build_evaluation::SelectedCompilerProgramEntry,
    /// The checked-source ProgramEntry-to-Terminal join receipt produced with
    /// this artifact. Native re-entry attaches it to the entry settlement as
    /// independently admitted authority; `validate_for_artifact` binds it to
    /// the retained source signature and canonical semantic identity so a
    /// drifted receipt cannot enter custody beside a valid artifact.
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    selected_provider_plans: effects::SelectedProviderPlanFacts,
    external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
    callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
    ieee_float_comparison_occurrences: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
    integer_comparison_occurrences: Vec<TerminalIntegerComparisonOccurrenceProposal>,
    boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    checked_boundary_operator_scope:
        lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    /// Canonical union of the authored `builder.exclude_*` selections the
    /// producing build required. It is retained beside the artifact so a
    /// source-free consumer replays the identical absence policy against the
    /// committed semantic subject; it grants no exclusion authority itself.
    behavior_exclusions: effects::BehaviorExclusions,
}

/// Everything a Terminal native realization proposal is built from besides
/// the artifact itself: the selected target, entry, plans, rows and occurrences
/// the proposal validates against the artifact's closure.
#[derive(Debug)]
pub struct TerminalNativeRealizationInputs {
    pub target_profile: target::TargetProfile,
    pub native_target: target::NativeTarget,
    pub subsystem: u16,
    pub application_intent: Option<build_evaluation::HostedApplicationIntent>,
    pub application_identifier: Option<build_evaluation::ApplicationIdentifier>,
    pub application_name: Option<String>,
    pub post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelectionProjection,
    pub program_entry: build_evaluation::SelectedCompilerProgramEntry,
    pub checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    pub selected_provider_plans: effects::SelectedProviderPlanFacts,
    pub external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    /// Exact selected opaque applications used by value across the retained
    /// signatures, canonicalized by the producing checked compilation.
    pub boundary_opaque_applications:
        boundary_applications::BoundaryOpaqueRepresentationApplications,
    pub package_terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub compiler_builtins: Vec<TerminalCompilerBuiltinProposal>,
    pub callback_occurrences: Vec<TerminalCallbackOccurrenceProposal>,
    pub ieee_float_comparison_occurrences: Vec<TerminalIeeeFloatComparisonOccurrenceProposal>,
    pub integer_comparison_occurrences: Vec<TerminalIntegerComparisonOccurrenceProposal>,
    pub boundary_application_demands: boundary_applications::TerminalBoundaryApplicationDemands,
    pub boundary_application_realizations:
        boundary_applications::TerminalBoundaryApplicationRealizations,
    pub checked_boundary_operator_scope:
        lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    pub behavior_exclusions: effects::BehaviorExclusions,
}

impl TerminalNativeRealizationProposal {
    pub fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        inputs: TerminalNativeRealizationInputs,
    ) -> Result<Self, &'static str> {
        let TerminalNativeRealizationInputs {
            target_profile,
            native_target,
            subsystem,
            application_intent,
            application_identifier,
            application_name,
            post_terminal_optimizations,
            program_entry,
            checked_program_entry,
            selected_provider_plans,
            external_binding_rows,
            boundary_opaque_applications,
            mut package_terminal_authority_permissions,
            compiler_builtins,
            callback_occurrences,
            ieee_float_comparison_occurrences,
            integer_comparison_occurrences,
            boundary_application_demands,
            boundary_application_realizations,
            checked_boundary_operator_scope,
            behavior_exclusions,
        } = inputs;
        let boundary_application_coverage =
            boundary_applications::TerminalBoundaryApplicationCoverage::new(
                boundary_application_demands,
                boundary_application_realizations,
            )?
            // Opaque custody rejoins coverage so every artifact, replay, and
            // install record built from this coverage carries the identical
            // selected-application set.
            .with_opaque_applications(boundary_opaque_applications);
        package_terminal_authority_permissions.sort_by(|left, right| {
            left.service_schema()
                .as_bytes()
                .cmp(right.service_schema().as_bytes())
                .then_with(|| {
                    left.requirement_identity()
                        .cmp(right.requirement_identity())
                })
        });
        let proposal = Self {
            terminal_artifact_identity: artifact.manifest().identity(),
            target_profile,
            native_target,
            subsystem,
            application_intent,
            application_identifier,
            application_name,
            post_terminal_optimizations,
            program_entry,
            checked_program_entry,
            selected_provider_plans,
            external_binding_rows,
            package_terminal_authority_permissions,
            compiler_builtins,
            callback_occurrences,
            ieee_float_comparison_occurrences,
            integer_comparison_occurrences,
            boundary_application_coverage,
            checked_boundary_operator_scope,
            behavior_exclusions,
        };
        proposal.validate_for_artifact(artifact)?;
        Ok(proposal)
    }

    pub fn validate_for_artifact(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        artifact
            .validate()
            .map_err(|_| "Terminal native proposal is paired with an invalid canonical artifact")?;
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("Terminal native proposal belongs to a different canonical artifact");
        }
        let complete_selection = optimization_core::OptimizationSelections::new(
            artifact
                .optimization()
                .selections()
                .as_slice()
                .iter()
                .copied()
                .map(optimization_core::Optimization::from)
                .chain(
                    self.post_terminal_optimizations
                        .selections()
                        .as_slice()
                        .iter()
                        .copied(),
                ),
        )
        .map_err(|_| "Terminal native proposal repeats an optimization across phase custody")?;
        if complete_selection.identity() != self.post_terminal_optimizations.complete_selection() {
            return Err(
                "Terminal native proposal optimization phases do not reconstruct the complete build selection",
            );
        }
        terminal_psi_to_abstract_operations::validate_terminal_boundary_application_coverage(
            artifact,
            &self.checked_boundary_operator_scope,
            &self.boundary_application_coverage,
        )?;
        if self.target_profile.native_target() != self.native_target
            || self.program_entry.source_signature().target_slot().owner != self.target_profile
        {
            return Err("Terminal native proposal target, profile, and ProgramEntry disagree");
        }
        if self.checked_program_entry.source_signature_identity()
            != self.program_entry.source_signature().identity().bytes()
            || self.checked_program_entry.source_machine_symbol()
                != self.program_entry.source_signature().machine_symbol()
        {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt drifts from its selected source entry",
            );
        }
        if self.checked_program_entry.terminal_psi_identity() != artifact.manifest().semantic() {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt drifts from the canonical Terminal identity",
            );
        }
        abstract_operations_to_target_operations::provider_admission::validate_evaluated_import_rows(
            self.target_profile,
            self.selected_provider_plans.plans(),
            &self.external_binding_rows,
        )?;
        self.validate_package_terminal_authority_permissions()?;
        let mut requirements = std::collections::BTreeSet::new();
        for builtin in &self.compiler_builtins {
            if !requirements.insert(builtin.requirement_identity()) {
                return Err("Terminal native proposal repeats a compiler-builtin requirement");
            }
            let Some(plan) = self
                .selected_provider_plans
                .plans()
                .get(builtin.provider_plan_index())
            else {
                return Err("Terminal native proposal names an absent selected provider plan");
            };
            let matching_rows = plan
                .rows
                .iter()
                .filter(|row| {
                    row.requirement_identity == builtin.requirement_identity
                        && matches!(
                            row.binding,
                            effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                        )
                })
                .count();
            if matching_rows != 1 {
                return Err(
                    "Terminal native proposal compiler builtin does not rejoin one exact selected row",
                );
            }
        }
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
            |_| "Terminal callback occurrence replay could not decode canonical semantics",
        )?;
        if module.entry != self.checked_program_entry.terminal_entry() {
            return Err(
                "Terminal native proposal checked ProgramEntry receipt names a different Terminal entry",
            );
        }
        program_entry_plan::validate_terminal_fused_service_establishments(
            &module,
            self.program_entry.source_signature(),
            self.program_entry.fused_service_establishments(),
            self.selected_provider_plans.plans(),
        )?;
        // The retained exclusion union is a source-free admission policy over
        // this artifact's committed semantics: a consumer replays the same
        // Terminal-closure absence check the producing build enforced, so a
        // proposal whose retained policy the artifact cannot satisfy never
        // reaches realization. Boundary-to-service ownership is reconstructed
        // from the canonical requirement identities inside the artifact, not
        // trusted from producer rows. The producing admission gate
        // additionally checked the unoptimized closure committed as this
        // artifact's optimization input; this replay re-verifies the
        // published subject. Physical-authority-class rows produce no site in
        // this walk — mechanisms exist only at realization — but still demand
        // the bounded entry/call closure the walk enforces; their
        // adjudication waits for the mechanism-closure review at native
        // realization, which consumes this same retained union.
        match effects::establish_behavior_exclusions(
            &module,
            &[module.entry],
            &self.behavior_exclusions,
            &self.selected_provider_plans,
        )
        .verdict()
        {
            effects::BehaviorExclusionVerdict::Satisfied => {}
            effects::BehaviorExclusionVerdict::Prohibited => {
                return Err(
                    "Terminal native proposal retains a possible behavior its authored exclusions prohibit",
                );
            }
            effects::BehaviorExclusionVerdict::InsufficientEvidence => {
                return Err(
                    "Terminal native proposal cannot establish the absence its authored exclusions require",
                );
            }
        }
        for demand in self.boundary_application_coverage.demands().rows() {
            let matching_operations = module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == demand.terminal_operation())
                .count();
            if matching_operations != 1 {
                return Err(
                    "Terminal native proposal boundary demand does not name one canonical operation",
                );
            }
        }
        backend_plan::validate_callback_occurrences_for_module(
            &module,
            &self.callback_occurrences,
        )?;
        terminal_psi_to_abstract_operations::validate_terminal_comparison_coverage(
            &module,
            self.selected_provider_plans.plans(),
            &self.ieee_float_comparison_occurrences,
            &self.integer_comparison_occurrences,
            self.boundary_application_coverage.realizations(),
            &self.checked_boundary_operator_scope,
        )?;
        Ok(())
    }

    fn validate_package_terminal_authority_permissions(&self) -> Result<(), &'static str> {
        let mut previous = None;
        for permission in &self.package_terminal_authority_permissions {
            if permission.requirement_identity().is_empty()
                || permission
                    .requirement_identity()
                    .chars()
                    .any(char::is_control)
            {
                return Err(
                    "Terminal native proposal contains an invalid package terminal-authority permission requirement",
                );
            }
            let coordinate = (
                permission.service_schema(),
                permission.requirement_identity(),
            );
            if previous == Some(coordinate) {
                return Err(
                    "Terminal native proposal repeats a package terminal-authority permission",
                );
            }
            previous = Some(coordinate);
        }
        Ok(())
    }

    pub const fn terminal_artifact_identity(&self) -> terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub const fn target_profile(&self) -> target::TargetProfile {
        self.target_profile
    }

    pub const fn native_target(&self) -> target::NativeTarget {
        self.native_target
    }

    pub const fn subsystem(&self) -> u16 {
        self.subsystem
    }

    /// The authored hosted application intent retained from the checked build,
    /// kept separate from the PE loader word so a raw `Unspecified` value can
    /// never impersonate `Gui` at native realization.
    pub const fn application_intent(&self) -> Option<build_evaluation::HostedApplicationIntent> {
        self.application_intent
    }

    /// The build-validated authored application identifier retained for
    /// native realization. Signed macOS GUI emission requires it; console
    /// output may fall back to the executable leaf as its ad-hoc label.
    pub const fn application_identifier(&self) -> Option<&build_evaluation::ApplicationIdentifier> {
        self.application_identifier.as_ref()
    }

    /// The validated authored `builder.application` name retained for
    /// publication. It supplies the `.app` basename and inner executable leaf
    /// for a retained proposal realized on a later machine; `None` means the
    /// build declared a non-application role.
    pub fn application_name(&self) -> Option<&str> {
        self.application_name.as_deref()
    }

    pub const fn post_terminal_optimizations(
        &self,
    ) -> &optimization_core::PostTerminalOptimizationSelectionProjection {
        &self.post_terminal_optimizations
    }

    pub const fn program_entry(&self) -> &build_evaluation::SelectedCompilerProgramEntry {
        &self.program_entry
    }

    /// The checked-source ProgramEntry-to-Terminal receipt retained for native
    /// re-entry. Validation binds it to this proposal's selected source
    /// signature and canonical Terminal identity; settlement independently
    /// re-derives the complete join before trusting it.
    pub const fn checked_program_entry(&self) -> &terminal_psi::CheckedProgramEntryTerminalReceipt {
        &self.checked_program_entry
    }

    pub const fn selected_provider_plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    pub fn external_binding_rows(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.external_binding_rows
    }

    /// Consumer-supplied package permission rows retained before the checked
    /// frontend was destroyed. These rows grant nothing by themselves; native
    /// re-entry must first match them to independently accepted package
    /// evidence and then rejoin that accepted set to the receiving policy.
    pub fn package_terminal_authority_permissions(
        &self,
    ) -> &[effects::ServiceTerminalAuthorityPermission] {
        &self.package_terminal_authority_permissions
    }

    pub fn compiler_builtins(&self) -> &[TerminalCompilerBuiltinProposal] {
        &self.compiler_builtins
    }

    pub fn callback_occurrences(&self) -> &[TerminalCallbackOccurrenceProposal] {
        &self.callback_occurrences
    }

    pub fn ieee_float_comparison_occurrences(
        &self,
    ) -> &[TerminalIeeeFloatComparisonOccurrenceProposal] {
        &self.ieee_float_comparison_occurrences
    }

    pub fn integer_comparison_occurrences(&self) -> &[TerminalIntegerComparisonOccurrenceProposal] {
        &self.integer_comparison_occurrences
    }

    /// Canonical union of the authored behavior exclusions this artifact's
    /// build required. The producing admission gate enforced it over the
    /// unoptimized selected-entry closure; consumers replay it against the
    /// retained artifact semantics through `validate_for_artifact`.
    /// Physical-authority-class rows are retained for realization: native
    /// realization adjudicates them against the mechanism-closure review
    /// even when the request supplies no receiving permission policy.
    pub const fn behavior_exclusions(&self) -> &effects::BehaviorExclusions {
        &self.behavior_exclusions
    }

    /// Non-caller-authored checked D29 scope retained before the checked
    /// frontend was destroyed.
    pub const fn checked_boundary_operator_scope(
        &self,
    ) -> &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope {
        &self.checked_boundary_operator_scope
    }

    pub const fn boundary_application_demands(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationDemands {
        self.boundary_application_coverage.demands()
    }

    pub const fn boundary_application_realizations(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationRealizations {
        self.boundary_application_coverage.realizations()
    }

    pub const fn boundary_application_coverage(
        &self,
    ) -> &boundary_applications::TerminalBoundaryApplicationCoverage {
        &self.boundary_application_coverage
    }

    /// Strong selected-application custody for every by-value opaque boundary
    /// edge this artifact's signatures carry. Native realization binds the
    /// identical set into the native artifact; a consumer artifact must carry
    /// the same commitments at the same edges for an exchange to agree.
    pub const fn boundary_opaque_applications(
        &self,
    ) -> &boundary_applications::BoundaryOpaqueRepresentationApplications {
        self.boundary_application_coverage.opaque_applications()
    }
}

/// Canonical Terminal artifact coupled to every exact checked callback
/// placement and target-constrained native proposal that crossed its
/// production boundary.
///
/// Callback rows remain target-owned compiler evidence rather than Terminal-
/// Psi vocabulary. Keeping them in the same retained product prevents an
/// artifact-only consuming escape from silently discarding the sidecar. This
/// carrier grants no registration, invocation, address, or lifetime authority.
#[derive(Debug)]
pub struct RetainedTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    native_realization_proposal: Option<TerminalNativeRealizationProposal>,
}

impl RetainedTerminalArtifact {
    /// Couples structurally valid rows to an artifact without reconstructing
    /// their checked-compilation provenance. The compiler's private product
    /// route supplies that provenance and preserves its checked row order.
    pub fn new(
        artifact: terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: None,
        };
        retained.validate()?;
        Ok(retained)
    }

    pub fn new_with_native_realization_proposal(
        artifact: terminal_codec::CanonicalTerminalArtifact,
        callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
        native_realization_proposal: TerminalNativeRealizationProposal,
    ) -> Result<Self, &'static str> {
        let retained = Self {
            artifact,
            callback_placements,
            native_realization_proposal: Some(native_realization_proposal),
        };
        retained.validate()?;
        Ok(retained)
    }

    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    pub fn callback_placements(&self) -> &[backend_plan::BoundNominalCallbackPlacement] {
        &self.callback_placements
    }

    pub const fn native_realization_proposal(&self) -> Option<&TerminalNativeRealizationProposal> {
        self.native_realization_proposal.as_ref()
    }

    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        Vec<backend_plan::BoundNominalCallbackPlacement>,
        Option<TerminalNativeRealizationProposal>,
    ) {
        (
            self.artifact,
            self.callback_placements,
            self.native_realization_proposal,
        )
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.artifact
            .validate()
            .map_err(|_| "retained Terminal product contains an invalid canonical artifact")?;
        for placement in &self.callback_placements {
            backend_plan::validate_bound_nominal_callback_placement(placement)
                .map_err(|_| "retained Terminal product contains an invalid callback placement")?;
        }
        if let Some(proposal) = &self.native_realization_proposal {
            proposal.validate_for_artifact(&self.artifact)?;
            backend_plan::validate_callback_occurrence_placements(
                &self.callback_placements,
                &proposal.callback_occurrences,
            )?;
        }
        Ok(())
    }
}

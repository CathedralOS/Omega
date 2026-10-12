//! Native authority is established by ordinary verification, never by promotion
//! of optimizer-only input.

use super::ArtifactLoweringError;
use super::retention::{AdmittedOptimizationArtifact, retain_optimization_context};
use crate::lowering::lower_decoded_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;
use terminal_installation::TerminalPlacedViewEstablishment;

/// Native-admitted input and the provider establishments bound to its
/// direct-entry placed-view roster rows. Construction is private:
/// `AdmittedNativeArtifact::try_into_native_input` joins one supply per
/// declared row, so an empty roster takes an empty supply.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInput {
    optimization_input: VerifiedPsiOptimizationInput,
    placed_view_establishments: Vec<TerminalPlacedViewEstablishment>,
}

impl VerifiedNativeArtifactInput {
    /// Resolve the portable entry for this exact deployment, lower its
    /// executable projection, and bind supplied direct-entry placed-view loans.
    /// Canonical semantics remain unchanged and no executable plan exists with
    /// an unresolved entry. Selection supplies no provider or backing authority.
    pub fn prepare(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TargetMachineCallable,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Result<Self, ArtifactLoweringError> {
        let module = artifact.semantic_module();
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            profile.target_name(),
        );
        let entry = selections
            .entry(artifact.view().identity(), module, root)
            .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
        lower_decoded_native_module(module, entry, Some(selections))?
            .try_into_native_input(establishments)
    }

    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    /// Select one deployment's executable closure before physical work.
    /// Entry artifacts and private callback artifacts use this same projection;
    /// canonical storage and actual callable IDs never change. Selection grants
    /// no provider installation authority. A self-contained callback supplies
    /// an empty adapter roster and still requires ordinary target lowering.
    pub fn select_target_execution(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        selected: &[crate::SelectedProviderAdapter],
        receiving_provider_roots: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Self, crate::NativeExecutionScopeError> {
        if artifact.view().identity() != self.plan().psi {
            return Err(crate::NativeExecutionScopeError::TerminalIdentityMismatch);
        }
        let mut input = self.select_target_families(profile);
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            profile.target_name(),
        );
        selections
            .entry(
                artifact.view().identity(),
                artifact.semantic_module(),
                terminal_psi::TargetMachineCallable::Machine(input.plan().entry),
            )
            .map_err(|error| crate::NativeExecutionScopeError::TargetFamily(error.to_string()))?;
        let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selections)
            .map_err(|error| crate::NativeExecutionScopeError::TargetLayout(error.to_string()))?;
        if !layouts.resolved_records().is_empty() {
            let declarations = input.optimization_input.plan.structural_types.make_mut();
            for resolved in layouts.resolved_records() {
                let declaration = declarations
                    .iter_mut()
                    .find(|row| row.id == resolved.id)
                    .ok_or_else(|| {
                        crate::NativeExecutionScopeError::TargetLayout(
                            "selected layout lost its executable structural declaration".to_owned(),
                        )
                    })?;
                *declaration = resolved.clone();
            }
        }
        input.optimization_input.target_machine_selections = Some(selections);
        input.optimization_input.target_layout_selections = Some(layouts);
        let scope = crate::native_execution_scope::NativeExecutionScope::derive(
            &input.optimization_input,
            selected,
            receiving_provider_roots,
        )?;
        input.retain_execution_scope(&scope);
        Ok(input)
    }

    /// Admit selected row aliases only with the exact artifact member receipt.
    pub(crate) fn bind_selected_family_realizations(
        mut self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<Self, String> {
        let selections = self
            .optimization_input
            .target_machine_selections
            .as_ref()
            .ok_or_else(|| "selected family providers require target selection".to_owned())?;
        for row in selected.family_realization_rows() {
            let member = selections
                .member(terminal_psi::TargetMachineCallable::Boundary(row.head))
                .map_err(|error| error.to_string())?;
            let module = artifact.semantic_module();
            if row.terminal != artifact.manifest().semantic()
                || row.terminal != selections.terminal_psi()
                || row.target != selections.target()
                || member.callable() != terminal_psi::TargetMachineCallable::Boundary(row.member)
                || terminal_psi::find_boundary_machine(&module.boundary_machines, row.head)
                    .is_none_or(|head| {
                        module.identities.text(head.identity)
                            != selected.identities().text(row.head_identity)
                    })
                || terminal_psi::find_boundary_machine(&module.boundary_machines, row.member)
                    .is_none_or(|member| {
                        module.identities.text(member.identity)
                            != selected.identities().text(row.member_identity)
                    })
            {
                return Err(
                    "selected family provider row disagrees with the exact artifact member"
                        .to_owned(),
                );
            }
        }
        self.optimization_input.target_family_realizations =
            selected.family_realization_rows().to_vec();
        self.optimization_input.target_family_realization_identities =
            selected.identities().clone();
        Ok(self)
    }

    /// Restrict physical work to this deployment's retained implementations.
    /// Canonical Terminal storage and actual member identities remain intact;
    /// this does not redirect calls or substitute a body at a family head.
    /// A surviving call to an excluded member still fails ordinary closure
    /// admission. Portable head calls keep their original occurrence and bind
    /// to the exact selected member through separate internal call custody.
    fn select_target_families(mut self, profile: target::TargetProfile) -> Self {
        let members = self
            .optimization_input
            .context
            .module()
            .target_machine_families
            .iter()
            .flat_map(|family| &family.members)
            .filter_map(|member| match member.callable {
                terminal_psi::TargetMachineCallable::Machine(machine) => {
                    Some((machine, member.target.as_str() == profile.target_name()))
                }
                terminal_psi::TargetMachineCallable::Boundary(_) => None,
            })
            .collect::<Vec<_>>();
        self.optimization_input.plan.functions.retain(|function| {
            members
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .is_none_or(|(_, selected)| *selected)
        });
        self
    }

    /// Narrow only the invocation's executable projection. The immutable
    /// canonical module remains available to every contract/evidence consumer.
    fn retain_execution_scope(
        &mut self,
        scope: &crate::native_execution_scope::NativeExecutionScope,
    ) {
        self.optimization_input
            .plan
            .functions
            .retain(|function| scope.contains_machine(function.machine));
        self.optimization_input
            .plan
            .boundary_machines
            .retain(|boundary| scope.contains_boundary(boundary.id));
        let mut candidate_position = 0;
        self.optimization_input
            .plan
            .provider_candidates
            .retain(|_| {
                let retained = scope.contains_candidate_position(candidate_position);
                candidate_position += 1;
                retained
            });
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub const fn optimization_input(&self) -> &VerifiedPsiOptimizationInput {
        &self.optimization_input
    }

    /// The provider establishments bound to this input's direct-entry roster
    /// rows, in roster order. Empty for a program that declared none.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        &self.placed_view_establishments
    }

    /// Establishment bindings are invocation-scoped executable evidence and
    /// do not transfer into optimizer authority; the provider keeps custody
    /// of its supply.
    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.optimization_input
    }
}

/// One canonical native-admitted plan with its exact retained verifier context.
/// Placed-view rows are semantic custody, not backing or access-event authority.
#[derive(Debug, Clone)]
pub struct AdmittedNativeArtifact {
    optimization_input: VerifiedPsiOptimizationInput,
}

impl AdmittedNativeArtifact {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.context().module().placed_view_inputs
    }

    /// Downgrade authority while preserving the complete semantic roster.
    pub fn into_optimization_artifact(self) -> AdmittedOptimizationArtifact {
        AdmittedOptimizationArtifact {
            input: self.optimization_input,
        }
    }

    /// The lowered plan alone. The plan carries no placed-view rows or
    /// verifier context; consumers that need either keep the admission and
    /// read `placed_view_inputs` or `context` instead.
    pub fn into_plan(self) -> AbstractOperationPlan {
        self.optimization_input.plan
    }

    /// The executable-image realization input: join one provider
    /// establishment to each placed-view roster row the direct entry
    /// declares. A program that declared none takes an empty supply. The
    /// bound supplies ride inside the native input so the realized entry
    /// boundary can lend the exact qualified backing for the invocation's
    /// duration; a supply that answers no declared row, answers one twice,
    /// carries noncanonical qualifications, or overlaps another exclusive
    /// referent rejects here rather than at access, and a row left
    /// unanswered still fails custody — a roster or a pointer is not the
    /// authority that lends the referent. Native execution of the derived
    /// placed-entry ABI with a lent host referent is exercised by the
    /// compiler control `direct_placed_view_input_survives_codec_and_native_replay`.
    pub(crate) fn try_into_native_input(
        self,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        let placed_view_establishments = super::establishment::establish_native_placed_view_inputs(
            self.context().module(),
            self.plan().entry,
            establishments,
        )?;
        Ok(VerifiedNativeArtifactInput {
            optimization_input: self.optimization_input,
            placed_view_establishments,
        })
    }
}

pub(super) fn lower_decoded_native_module(
    module: &terminal_psi::TerminalModule,
    entry: semantic_vocabulary::MachineId,
    selections: Option<terminal_installation::AdmittedTargetMachineSelections>,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let plan = lower_decoded_module(module, entry).map_err(ArtifactLoweringError::Lowering)?;
    let context = retain_optimization_context(module);
    Ok(AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput {
            plan,
            context,
            target_machine_selections: selections,
            target_layout_selections: None,
            target_family_realizations: Vec::new(),
            target_family_realization_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
        },
    })
}

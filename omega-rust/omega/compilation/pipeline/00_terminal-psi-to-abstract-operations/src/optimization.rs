/*
  Verified Psi optimization-unit construction (`build_verified_psi_optimization_unit`,
  `VerifiedPsiOptimizationUnit`, `VerifiedPsiOptimizationInput`).

  Reconstructs and validates the mutable control-flow, value, ownership, and
  fact representation (`PsiOptimizationUnit`) from an admitted Terminal module
  before Omega 01 abstract rewrites run.
*/
mod error {
    use semantic_vocabulary::{MachineId, ObligationId, OperationId};
    use terminal_codec::CodecError;
    #[derive(Debug)]
    pub enum VerifiedPsiOptimizationUnitBuildError {
        Unit(optimization_unit::OptimizationUnitBuildError),
        MissingReconstructedObligation {
            machine: MachineId,
            operation: OperationId,
            obligation: ObligationId,
        },
        MissingAcceptedObligation {
            machine: MachineId,
            operation: OperationId,
            obligation: ObligationId,
        },
        PropositionCodec(CodecError),
        FactIndex(optimization_unit::AcceptedObligationFactIndexError),
        ProofQuestionIndex(optimization_unit::ProofQuestionIndexError),
        MissingStructuralCatalogMachine(MachineId),
    }

    impl std::fmt::Display for VerifiedPsiOptimizationUnitBuildError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "cannot construct verified Psi optimization unit: {self:?}"
            )
        }
    }

    impl std::error::Error for VerifiedPsiOptimizationUnitBuildError {}

    impl From<optimization_unit::OptimizationUnitBuildError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: optimization_unit::OptimizationUnitBuildError) -> Self {
            Self::Unit(error)
        }
    }

    impl From<CodecError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: CodecError) -> Self {
            Self::PropositionCodec(error)
        }
    }

    impl From<optimization_unit::AcceptedObligationFactIndexError>
        for VerifiedPsiOptimizationUnitBuildError
    {
        fn from(error: optimization_unit::AcceptedObligationFactIndexError) -> Self {
            Self::FactIndex(error)
        }
    }

    impl From<optimization_unit::ProofQuestionIndexError> for VerifiedPsiOptimizationUnitBuildError {
        fn from(error: optimization_unit::ProofQuestionIndexError) -> Self {
            Self::ProofQuestionIndex(error)
        }
    }
}

pub use error::VerifiedPsiOptimizationUnitBuildError;

use abstract_operations::AbstractOperationPlan;
use installation_evidence::InstalledAssumedBindingEvidence;
use optimization_unit::{AcceptedObligationFact, PsiOptimizationUnit};
use std::collections::BTreeSet;
use terminal_installation::{
    AdmittedScalarApplications, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
};

pub fn build_verified_psi_optimization_unit(
    input: VerifiedPsiOptimizationInput,
    work_schedule: semantic_vocabulary::WorkScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), work_schedule)?;
    let context = input.context();
    optimization_unit::attach_verified_module_context(&mut seed, context.module())
        .map_err(VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine)?;
    let facts = source_checked_obligation_facts(&seed)?;
    if std::env::var_os("OMEGA_PROOF_MEASUREMENTS").is_some() {
        eprint!("{}", product_obligation_census(context.module(), &facts));
    }
    let unit = optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

/// The census that `wiki/drafts/designs/check_on_lowered_form.md` asks for
/// before route one: how many obligations the product accepts, and how many of
/// those carry evidence. It prints one line per kind of owning Terminal
/// operation and reads only what this stage already built, so verdicts and
/// artifacts are unchanged. "Evidence" means a nonzero proof-bundle
/// fingerprint on the accepted fact; a reconstructed goal is counted apart
/// because it states the obligation without proving it. A call's `requires`
/// obligations travel on the call and never become accepted facts here, so
/// they are counted as carried, with the callee's source-checked `requires`.
fn product_obligation_census(
    module: &terminal_psi::TerminalModule,
    facts: &[AcceptedObligationFact],
) -> String {
    use semantic_vocabulary::{MachineId, OperationId};
    use std::collections::BTreeMap;
    let mut owners: Vec<((MachineId, OperationId), &'static str)> = Vec::new();
    let mut carried_requires = 0usize;
    let mut source_checked_requires = 0usize;
    for machine in &module.machines {
        source_checked_requires += machine.contract.source_checked_requires.len();
        for block in &machine.blocks {
            carried_requires += block
                .boundary_invocations()
                .map(|call| call.requirement_obligations.len())
                .sum::<usize>();
            for operation in &block.operations {
                owners.push((
                    (machine.id, operation.id),
                    terminal_semantics::OperationSemanticTag::for_operation(&operation.kind).name(),
                ));
                carried_requires += match &operation.kind {
                    terminal_psi::OperationKind::CallDynamic {
                        requirement_obligations,
                        ..
                    } => requirement_obligations.len(),
                    kind => kind
                        .module_invocation()
                        .map_or(0, |call| call.requirement_obligations.len()),
                };
            }
        }
    }
    owners.sort_unstable();
    // ponytail: string-keyed tally, measurement output only; never feeds a decision.
    let mut kinds: BTreeMap<&'static str, [usize; 3]> = BTreeMap::new();
    for fact in facts {
        let kind = match fact.owner {
            optimization_unit::PsiProvenance::Operation(operation) => owners
                .binary_search_by_key(&(fact.machine, operation), |(key, _)| *key)
                .map_or("unknown_operation", |found| owners[found].1),
            optimization_unit::PsiProvenance::Edge(_) => "EdgeRuntimeIndex",
        };
        let row = kinds.entry(kind).or_default();
        row[0] += 1;
        row[1] += usize::from(fact.proof_bundle_fingerprint != [0; 32]);
        row[2] += usize::from(!fact.proposition.is_empty());
    }
    let mut census = String::new();
    for (kind, [accepted, evidence, goal]) in kinds {
        census.push_str(&format!(
            "product_obligation_census kind={kind} accepted={accepted} \
             with_evidence={evidence} reconstructed_goal={goal}\n"
        ));
    }
    census.push_str(&format!(
        "product_obligation_census kind=CallRequires carried={carried_requires} \
         accepted=0 with_evidence=0 callee_source_checked_requires={source_checked_requires}\n"
    ));
    census
}

fn source_checked_obligation_facts(
    unit: &PsiOptimizationUnit,
) -> Result<Vec<AcceptedObligationFact>, VerifiedPsiOptimizationUnitBuildError> {
    let mut coordinates = BTreeSet::new();
    let mut facts = Vec::new();
    for function in &unit.functions {
        for fact in &function.facts {
            let optimization_unit::OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } = fact
            else {
                continue;
            };
            let owner = optimization_unit::PsiProvenance::Operation(*support);
            if coordinates.insert((function.machine, owner, *obligation)) {
                let representation =
                    function.block_nodes().find_map(|node| {
                        let abstract_operations::AbstractOperation::EstablishElementView {
                            psi_operation,
                            source,
                            element,
                            region:
                                region @ terminal_psi::ElementViewRegion::WholeRepresentation {
                                    source_length,
                                    obligation: count_obligation,
                                },
                            ..
                        } = &node.operation
                        else {
                            return None;
                        };
                        (*psi_operation == *support && *count_obligation == *obligation)
                            .then_some((source, element, region, source_length))
                    });
                let proposition = if let Some((source, element, region, source_length)) =
                    representation
                {
                    let missing =
                        || VerifiedPsiOptimizationUnitBuildError::MissingReconstructedObligation {
                            machine: function.machine,
                            operation: *support,
                            obligation: *obligation,
                        };
                    let root = optimization_unit::field_evidence(function, source.place)
                        .and_then(|row| row.root_type)
                        .ok_or_else(missing)?;
                    let collection = terminal_semantics::runtime_structural_path_tip(
                        unit.structural_types.iter(),
                        root,
                        &source.path,
                    )
                    .ok_or_else(missing)?;
                    let Some(terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                        source: source_layout,
                        target: target_layout,
                        ..
                    }) = terminal_semantics::element_view_shape_region_geometry(
                        unit.structural_types.iter(),
                        &collection.shape,
                        *element,
                        *region,
                        source.access,
                    )
                    else {
                        return Err(missing());
                    };
                    let factor = source_layout
                        .size
                        .checked_div(target_layout.size)
                        .ok_or_else(missing)?;
                    let goal =
                        terminal_semantics::whole_representation_count_goal(*source_length, factor)
                            .ok_or_else(missing)?;
                    terminal_codec::canonical_proposition_order_key(&goal)?
                } else {
                    Vec::new()
                };
                let fact = AcceptedObligationFact::new(
                    unit.psi,
                    [0; 32],
                    function.machine,
                    owner,
                    *obligation,
                    proposition,
                );
                let receiving = function.block_nodes().find_map(|node| {
                    let (operation, retained_obligation, value, restriction) = match &node.operation
                    {
                        abstract_operations::AbstractOperation::WriteOnlyPrimitiveStore {
                            psi_operation,
                            destination,
                            path,
                            value,
                            range_obligation,
                        } => (
                            *psi_operation,
                            *range_obligation,
                            value.value,
                            {
                                let (whole, projected) = match destination {
                                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                                    | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter { parameter, .. } => (&parameter.qualifications, &parameter.projected_qualifications),
                                    abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => (&result.qualifications, &result.projected_qualifications),
                                };
                                terminal_semantics::primitive_store_restriction(unit.structural_types.iter(), unit.structural_domains.iter(), destination.structural_type(), whole, projected, destination.additional_input_qualifications(), path)??
                            },
                        ),
                        abstract_operations::AbstractOperation::EstablishPrimitiveLocal {
                            psi_operation,
                            result,
                            value,
                            range_obligation,
                        } => (
                            *psi_operation,
                            *range_obligation,
                            value.value,
                            terminal_semantics::primitive_projection_restriction(
                                unit.structural_types.iter(),
                                result.structural_type,
                                &[],
                            )?.clone(),
                        ),
                        abstract_operations::AbstractOperation::AtomicEvent { psi_operation, event, resident_obligation, .. } => {
                            let location = event.location()?;
                            let root = optimization_unit::field_evidence(function, location.root)?.root_type?;
                            let cell = terminal_semantics::atomic_cell_type(unit.structural_types.iter(), root, &location.path)?;
                            let value = event.replacement().or(match event {
                                abstract_operations::AbstractAtomicEvent::ReadModifyWrite { operand, .. } => Some(*operand),
                                _ => None,
                            })?;
                            (*psi_operation, *resident_obligation, value, terminal_psi::StructuralFieldType::from(cell))
                        },
                        _ => return None,
                    };
                    if operation != *support || retained_obligation != Some(*obligation) {
                        return None;
                    }
                    Some(optimization_unit::SourceCheckedScalarReceiving {
                        value,
                        restriction: restriction.clone(),
                    })
                });
                facts.push(match receiving {
                    Some(receiving) => fact.with_scalar_receiving(receiving),
                    None => fact,
                });
            }
        }
        for node in function.block_nodes() {
            let paths = node
                .successors
                .iter()
                .flat_map(|edge| {
                    edge.structural_bindings
                        .iter()
                        .map(move |binding| (edge.psi_edge, binding.argument.path.as_slice()))
                })
                .chain(
                    node.operation
                        .cleanup_rosters()
                        .flat_map(|(edge, actions)| {
                            actions.iter().map(move |action| (edge, action.path()))
                        }),
                );
            for (edge, path) in paths {
                for (_, obligation) in path
                    .iter()
                    .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
                {
                    let owner = optimization_unit::PsiProvenance::Edge(edge);
                    if coordinates.insert((function.machine, owner, obligation)) {
                        facts.push(AcceptedObligationFact::new(
                            unit.psi,
                            [0; 32],
                            function.machine,
                            owner,
                            obligation,
                            Vec::new(),
                        ));
                    }
                }
            }
        }
    }
    Ok(facts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
    pub(crate) scalar_applications: Option<AdmittedScalarApplications>,
    pub(crate) selected_program_entry_call: Option<crate::SelectedProgramEntryCall>,
    pub(crate) plan: AbstractOperationPlan,
    pub(crate) context: VerifiedPsiOptimizationContext,
    pub(crate) admitted_assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    pub(crate) admitted_native_callbacks: provider_admission::AdmittedNativeCallbackCohort,
    pub(crate) admitted_installed_interrupts: provider_admission::AdmittedInstalledInterruptCohort,
    pub(crate) target_machine_selections: Option<AdmittedTargetMachineSelections>,
    pub(crate) target_layout_selections: Option<AdmittedTargetLayouts>,
    pub(crate) target_family_realizations: Vec<effects::SelectedTargetFamilyRealization>,
    pub(crate) external_requirement_applications:
        Vec<effects::SelectedExternalRequirementApplication>,
    pub(crate) selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable,
}

impl VerifiedPsiOptimizationInput {
    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.scalar_applications.as_ref()
    }

    pub fn selected_program_entry_call(&self) -> Option<&crate::SelectedProgramEntryCall> {
        self.selected_program_entry_call.as_ref()
    }
    pub fn admitted_assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.admitted_assumed_bindings
    }

    pub fn admitted_native_callbacks(&self) -> &[crate::AdmittedNativeCallbackArgument] {
        self.admitted_native_callbacks.callbacks()
    }

    pub const fn admitted_installed_interrupts(
        &self,
    ) -> &provider_admission::AdmittedInstalledInterruptCohort {
        &self.admitted_installed_interrupts
    }

    pub fn into_lowering_facts(self) -> provider_admission::AdmittedLoweringFacts {
        provider_admission::AdmittedLoweringFacts {
            program_entry: self.selected_program_entry_call.as_ref().map(|call| {
                provider_admission::LoweringEntryCall {
                    boundary: call.boundary(),
                    entry: call.entry(),
                    source_signature: call.source_signature(),
                }
            }),
            module: self.context.module,
            assumed_bindings: self.admitted_assumed_bindings,
            target_machine_selections: self.target_machine_selections,
            native_callbacks: self.admitted_native_callbacks,
            installed_interrupts: self.admitted_installed_interrupts,
        }
    }

    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
    }

    pub fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    pub fn target_family_realizations(&self) -> &[effects::SelectedTargetFamilyRealization] {
        &self.target_family_realizations
    }

    pub fn selected_provider_requirement_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.selected_provider_requirement_identities
    }

    pub fn target_layout_selections(&self) -> Option<&AdmittedTargetLayouts> {
        self.target_layout_selections.as_ref()
    }

    pub fn selected_calling_abi_identity(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let declaration = selected_calling_boundary(
            self.context.module(),
            self.target_machine_selections(),
            boundary,
        )?;
        self.context.module().calling_abi_identity(declaration)
    }

    pub fn target_family_member(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<Option<terminal_psi::TargetMachineCallable>, String> {
        target_family_member(
            self.context.module(),
            self.target_machine_selections(),
            boundary,
        )
    }
}

pub(crate) use terminal_installation::admitted_boundary_member as target_family_member;

pub(crate) use ::provider_admission::selected_calling_boundary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    pub(crate) module: terminal_psi::TerminalModule,
}

impl VerifiedPsiOptimizationContext {
    pub const fn module(&self) -> &terminal_psi::TerminalModule {
        &self.module
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationUnit {
    pub(crate) input: VerifiedPsiOptimizationInput,
    pub(crate) unit: PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationUnit {
    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedPsiOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

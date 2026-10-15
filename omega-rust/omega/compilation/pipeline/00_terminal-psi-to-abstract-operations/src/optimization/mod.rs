//! Optimizer module role: executable entrance. Optimizer-input entrance:
//! reconstruct the complete unit seed, bind admitted proof facts and questions,
//! then seal the unit beside the verifier-owned input context. Stage 01
//! reconstructs current ownership from this unit before running any pass.

mod error;

pub use error::VerifiedPsiOptimizationUnitBuildError;

use abstract_operations::AbstractOperationPlan;
use std::collections::BTreeSet;

/// The only optimizer-facing unit constructor. Consuming the verified carrier
/// prevents callers from pairing a plan with evidence admitted for a different
/// Terminal-Psi artifact.
pub fn build_verified_psi_optimization_unit(
    input: VerifiedPsiOptimizationInput,
    fuel_schedule: semantic_vocabulary::FuelScheduleIdentity,
) -> Result<VerifiedPsiOptimizationUnit, VerifiedPsiOptimizationUnitBuildError> {
    let mut seed =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), fuel_schedule)?;
    let context = input.context();
    optimization_unit::attach_verified_module_context(&mut seed, context.module())
        .map_err(VerifiedPsiOptimizationUnitBuildError::MissingStructuralCatalogMachine)?;
    let facts = source_checked_obligation_facts(&seed);
    let unit = optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

fn source_checked_obligation_facts(
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Vec<optimization_unit::AcceptedObligationFact> {
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
                let fact = optimization_unit::AcceptedObligationFact::new(
                    unit.psi,
                    [0; 32],
                    function.machine,
                    owner,
                    *obligation,
                    Vec::new(),
                );
                // This original source-cut correspondence is immutable. A
                // later rewrite cannot reuse the obligation for a different
                // RHS or a changed receiving predicate. No proof replay occurs.
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
                            terminal_semantics::primitive_projection_restriction(
                                unit.structural_types.iter(),
                                destination.structural_type(),
                                path,
                            )?,
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
                            )?,
                        ),
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
        // Successor projections and captured cleanup places carry bounds at
        // their own edge, not at a fabricated instruction or a later store.
        // Retain every already-checked path occurrence under that exact owner;
        // sharing an obligation number never borrows another edge's judgment.
        for node in function.block_nodes() {
            let paths =
                node.successors
                    .iter()
                    .flat_map(|edge| {
                        edge.structural_bindings
                            .iter()
                            .map(move |binding| (edge.psi_edge, binding.argument.path.as_slice()))
                    })
                    .chain(node.operation.cleanup_rosters().into_iter().flat_map(
                        |(edge, actions)| actions.iter().map(move |action| (edge, action.path())),
                    ));
            for (edge, path) in paths {
                for (_, obligation) in path
                    .iter()
                    .filter_map(terminal_psi::StructuralPathSegment::runtime_index)
                {
                    let owner = optimization_unit::PsiProvenance::Edge(edge);
                    if coordinates.insert((function.machine, owner, obligation)) {
                        facts.push(optimization_unit::AcceptedObligationFact::new(
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
    facts
}

/// Required optimizer input produced only after canonical artifact decoding,
/// Terminal-Psi validation, proof reconstruction, and evidence admission.
///
/// Optimizer entry points require this carrier so proof, ownership, and
/// path-sensitive semantic context cannot become an optional side channel.
/// Native artifact admission retains it inside its distinct authority carrier;
/// optimizer-only input cannot grant native admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInput {
    pub(crate) plan: AbstractOperationPlan,
    pub(crate) context: VerifiedPsiOptimizationContext,
    pub(crate) target_machine_selections:
        Option<terminal_installation::AdmittedTargetMachineSelections>,
    pub(crate) target_layout_selections: Option<terminal_installation::AdmittedTargetLayouts>,
    pub(crate) target_family_realizations: Vec<effects::SelectedTargetFamilyRealization>,
    /// The selected facts table the family-realization row identities
    /// resolve against. Rows are stored without their owning facts, so the
    /// table is retained beside them.
    pub(crate) target_family_realization_identities: terminal_psi::TerminalIdentityTable,
}

impl VerifiedPsiOptimizationInput {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
    }

    /// Invocation selection is catalog membership, never provider authority.
    /// Optimizer-only preparation retains no execution selection.
    pub fn target_machine_selections(
        &self,
    ) -> Option<&terminal_installation::AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    /// Exact provider coordinates already closed against this artifact and its
    /// invocation selection. The original boundary remains the call contract.
    pub fn target_family_realizations(&self) -> &[effects::SelectedTargetFamilyRealization] {
        &self.target_family_realizations
    }

    /// Identity table the family-realization row indices resolve against.
    pub fn target_family_realization_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.target_family_realization_identities
    }

    /// Exact geometry closure retained separately from immutable canonical Psi.
    /// The executable plan uses only this receipt's selected declarations.
    pub fn target_layout_selections(
        &self,
    ) -> Option<&terminal_installation::AdmittedTargetLayouts> {
        self.target_layout_selections.as_ref()
    }

    /// Resolve a portable family head to its exact execution coordinate. A missing
    /// selected member refuses only when reached; retained, unused families
    /// impose no deployment requirement. Opaque members still require their
    /// own realization and cannot be promoted to executable checked bodies.
    pub fn target_family_member(
        &self,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<Option<terminal_psi::TargetMachineCallable>, String> {
        let head = terminal_psi::TargetMachineCallable::Boundary(boundary);
        let families = &self.context.module.target_machine_families;
        let Ok(position) = families.binary_search_by_key(&head, |family| family.canonical) else {
            return Ok(None);
        };
        if families[position]
            .members
            .iter()
            .any(|member| member.callable == head)
        {
            return Ok(None);
        }
        let selections = self.target_machine_selections().ok_or_else(|| {
            format!("target-family head {boundary:?} has no invocation deployment selection")
        })?;
        let member = selections.member(head).map_err(|error| error.to_string())?;
        match (member.callable(), member.supply()) {
            (
                terminal_psi::TargetMachineCallable::Machine(machine),
                terminal_psi::TargetMachineSupply::CheckedBody
                | terminal_psi::TargetMachineSupply::Boundary,
            ) => Ok(Some(terminal_psi::TargetMachineCallable::Machine(machine))),
            (terminal_psi::TargetMachineCallable::Boundary(member), supply)
                if supply != terminal_psi::TargetMachineSupply::CheckedBody =>
            {
                Ok(Some(terminal_psi::TargetMachineCallable::Boundary(member)))
            }
            _ => Err(format!(
                "target-family head {boundary:?} selected incompatible callable {:?} for `{}`",
                member.callable(),
                selections.target(),
            )),
        }
    }
}

/// Verifier-owned proof context retained beside the reconstructible Omega plan.
/// The complete immutable Terminal module remains for proof-question and cycle
/// admission until those independent replay paths are deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationContext {
    pub(crate) module: terminal_psi::TerminalModule,
}

impl VerifiedPsiOptimizationContext {
    pub const fn module(&self) -> &terminal_psi::TerminalModule {
        &self.module
    }
}

/// A reconstructible optimizer unit that cannot detach from the exact
/// verifier context which authorized its proof- and borrow-sensitive facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationUnit {
    pub(crate) input: VerifiedPsiOptimizationInput,
    pub(crate) unit: optimization_unit::PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationUnit {
    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &optimization_unit::PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(
        self,
    ) -> (
        VerifiedPsiOptimizationInput,
        optimization_unit::PsiOptimizationUnit,
    ) {
        (self.input, self.unit)
    }
}

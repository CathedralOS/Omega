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
    let facts = source_checked_operation_facts(&seed);
    let unit = optimization_unit::attach_accepted_obligation_facts(seed, facts)?;
    Ok(VerifiedPsiOptimizationUnit { input, unit })
}

fn source_checked_operation_facts(
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
            if coordinates.insert((function.machine, *support, *obligation)) {
                facts.push(optimization_unit::AcceptedObligationFact::new(
                    unit.psi,
                    [0; 32],
                    function.machine,
                    *support,
                    *obligation,
                    Vec::new(),
                ));
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
}

impl VerifiedPsiOptimizationInput {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        &self.plan
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        &self.context
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

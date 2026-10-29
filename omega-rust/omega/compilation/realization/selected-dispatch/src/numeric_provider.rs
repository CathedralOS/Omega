//! Numeric execution consumes Psi declarations; only selected-plan association
//! remains here. Review supplies its candidate separately from checked custody.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};
use psi_terminal_production::NumericRealizationDeclarations;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub struct NumericProviderSelection<'a> {
    pub candidate: &'a NumericRealizationDeclarations,
    pub custody: &'a NumericRealizationDeclarations,
    pub provider: Option<SymbolHandle>,
}

impl<'a> NumericProviderSelection<'a> {
    pub(crate) fn execution(
        self,
        plan: &ProviderPlan,
        row: &ProviderPlanRow,
        requirement: SymbolHandle,
        realization: SymbolHandle,
    ) -> Result<effects::CompilerIntrinsicExecutionIdentity, Diagnostic> {
        let join = || -> Result<_, &'static str> {
            let nominal = self
                .custody
                .nominal(requirement)
                .ok_or("numeric requirement has no checked declaration")?;
            if self.candidate.nominal(requirement) != Some(nominal) {
                return Err("numeric requirement differs from checked declaration custody");
            }
            let realization = self
                .custody
                .nominal(realization)
                .ok_or("numeric realization has no checked declaration")?;
            let select = |declarations: &'a NumericRealizationDeclarations| {
                effects::numeric_realizations::selected_numeric_realization(
                    &declarations.conformances,
                    &declarations.identities,
                    effects::numeric_realizations::NumericInvocation::Requirement {
                        declaration: nominal,
                        realization: Some(realization),
                    },
                    plan,
                    row,
                )
                .and_then(|candidate| {
                    candidate.ok_or("numeric selection has no checked external satisfaction")
                })
                .map(|candidate| {
                    (
                        candidate.numeric_intrinsic,
                        declarations
                            .identities
                            .text(candidate.realization.machine.identity()),
                        candidate.provider.as_ref().and_then(|owner| {
                            declarations.identities.text(owner.declaration.identity())
                        }),
                    )
                })
            };
            let candidate = select(self.candidate)?;
            let custody = select(self.custody)?;
            let provider = self
                .provider
                .map(|symbol| {
                    self.custody
                        .nominal(symbol)
                        .ok_or("numeric provider has no checked declaring owner")
                })
                .transpose()?;
            if candidate != custody || candidate.1 != Some(realization) || candidate.2 != provider {
                return Err(
                    "numeric meaning, realization or owner differs from checked selection custody",
                );
            }
            Ok(candidate.0.into())
        };
        join().map_err(|reason| {
            Diagnostic::error(format!("selected ProviderPlan `{}`: {reason}", plan.name))
        })
    }
}

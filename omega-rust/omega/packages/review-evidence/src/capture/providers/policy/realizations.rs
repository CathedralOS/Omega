//! Keep the selected executable association while describing its stable source
//! application. Private specialization names must not change package consent.

use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::PackageReviewNominalIdentity;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(in crate::capture::providers) struct ReviewedProviderRealization {
    pub(in crate::capture::providers) symbol: SymbolHandle,
    pub(in crate::capture::providers) declaration: PackageReviewNominalIdentity,
    pub(in crate::capture::providers) executable_callable: String,
    pub(in crate::capture::providers) portable_callable: String,
}

impl ReviewedProviderRealization {
    pub(in crate::capture::providers) fn project(
        compilation: &PackageReviewInput<'_>,
        symbol: SymbolHandle,
    ) -> Result<Self, Vec<Diagnostic>> {
        let machine = compilation
            .exact_machine(symbol)
            .ok_or_else(|| rejected("provider realization has no exact checked machine"))?;
        let executable_callable = compilation
            .machine_callable_identity_text(machine)
            .ok_or_else(|| rejected("provider realization has no exact callable identity"))?;
        let mut declaration = nominal_identity(compilation, symbol)?;
        let applied = typed_trees::type_identity::portable_machine_application_identity(
            &compilation.typed,
            symbol,
            compilation.exact_toolchain_sources,
        )
        .map_err(|reason| rejected(&reason))?;
        let portable_callable = if let Some(identity) = applied {
            let receipt = compilation
                .machine_instantiation(symbol)
                .map_err(rejected)?
                .ok_or_else(|| {
                    rejected("provider realization lost its application after projection")
                })?;
            let template = nominal_identity(compilation, receipt.template())?;
            if declaration.owner != template.owner {
                return Err(rejected(
                    "provider application changed its original declaration owner",
                ));
            }
            declaration.path = identity.clone();
            identity
        } else {
            executable_callable.clone()
        };
        Ok(Self {
            symbol,
            declaration,
            executable_callable,
            portable_callable,
        })
    }

    pub(in crate::capture::providers) fn policy_callable(
        &self,
        compilation: &PackageReviewInput<'_>,
    ) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
        if compilation
            .machine_instantiation(self.symbol)
            .map_err(rejected)?
            .is_some()
        {
            Ok(self.declaration.clone())
        } else {
            crate::capture::semantics::conformances::policy_callable_identity(
                compilation,
                self.symbol,
            )
        }
    }
}

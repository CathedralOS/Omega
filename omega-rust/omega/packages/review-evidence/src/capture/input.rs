/*
Review projection inputs retain their original query namespace. A frozen typed
source and the route's verdict cannot stand in for the retired checker's
original contract custody, so public construction now refuses explicitly.
Fallible conversion preserves that error through every projection entrance.
Legacy private readers never reinterpret a typed source as a CheckingInput.
*/

use checked_trees::CheckedTrees;
use checked_trees::typed_trees::service_declarations::PreparedServiceCatalog;
use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelection;
use provider_planning::ProviderPlanningContext;
use provider_planning::service_declarations::ServiceDeclarations;
use psi_terminal_production::ExternalProviderDeclarations;
use typed_trees::machine::Machine;

#[derive(Clone, Copy)]
pub struct PackageReviewInput<'a> {
    pub(crate) source: SourceDeclarationInput<'a>,
    pub(crate) custody: &'a CheckedCompilation,
    pub(crate) service_declarations: Option<&'a ServiceDeclarations>,
    pub(crate) external_providers: Option<&'a ExternalProviderDeclarations>,
}

#[derive(Clone, Copy)]
pub struct SourceDeclarationInput<'a> {
    pub(crate) program: &'a CheckedTrees,
    pub(crate) checking_input: &'a dyn validation::CheckingInput, // Supplied candidate query state.
    pub(crate) original_checking_input: &'a dyn validation::CheckingInput, // Immutable custody query state.
    pub(crate) mathematical_calls:
        Option<&'a super::contracts::mathematical_calls::MathematicalCallQueries<'a>>,
    pub(crate) original_mathematical_calls:
        Option<&'a super::contracts::mathematical_calls::MathematicalCallQueries<'a>>,
    pub(crate) original_program: &'a CheckedTrees,
    pub(crate) package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
}

impl<'a> SourceDeclarationInput<'a> {
    pub(crate) fn original(self) -> Self {
        Self {
            program: self.original_program,
            checking_input: self.original_checking_input,
            mathematical_calls: self.original_mathematical_calls,
            ..self
        }
    }
}

impl<'a> PackageReviewInput<'a> {
    /// Typed source and a route verdict do not supply the retired checker's
    /// original checking namespace. Review refuses until the route publishes
    /// the contract custody this projection consumes.
    pub fn supplied(
        _program: &'a typed_trees::TypedTrees,
        _custody: &'a CheckedCompilation,
    ) -> Result<Self, Vec<Diagnostic>> {
        Err(vec![Diagnostic::error(
            "no product route yet: package review requires route-owned original contract custody",
        )])
    }

    pub(super) fn project_external_providers(
        &self,
    ) -> Result<ExternalProviderDeclarations, Vec<diagnostics::Diagnostic>> {
        psi_terminal_production::project_external_provider_declarations(
            self.program,
            self.custody.sources().exact_toolchain_sources(),
        )
        .map_err(|error| {
            vec![diagnostics::Diagnostic::error(format!(
                "package external provider declarations: {error:?}"
            ))]
        })
    }

    pub(super) fn project_service_declarations(
        &self,
    ) -> Result<ServiceDeclarations, Vec<diagnostics::Diagnostic>> {
        let context =
            ProviderPlanningContext::new(self.checking_input, source::DependencyScope::Product);
        let catalog = std::sync::Arc::new(PreparedServiceCatalog::prepare(
            context,
            self.custody.sources().exact_toolchain_sources(),
        ));
        Ok(ServiceDeclarations::prepare(
            catalog
                .bind(context)
                .map_err(|error| vec![error])?
                .declared_traits(),
            &self.custody.providers().boundary_calling_plan_realizations,
        ))
    }
}

impl<'a> SourceDeclarationInput<'a> {
    pub(crate) fn derive_checked_nominal_call_target(
        &self,
        _expression: typed_trees::_expression::ExpressionHandle,
    ) -> Result<Option<symbols::SymbolHandle>, Vec<diagnostics::Diagnostic>> {
        Err(vec![diagnostics::Diagnostic::error(
            "no product route yet: package-review nominal call target evidence",
        )])
    }

    pub(crate) fn exact_machine(&self, symbol: symbols::SymbolHandle) -> Option<&Machine> {
        let mut machines = self
            .machines()
            .iter()
            .filter(|machine| machine.symbol == symbol);
        let only = machines.next()?;
        machines.next().is_none().then_some(only)
    }

    pub(super) fn authored_machine_target(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<Option<&str>, Vec<diagnostics::Diagnostic>> {
        let rejected = |reason: &str| {
            vec![diagnostics::Diagnostic::error(format!(
                "package target context rejects {reason}"
            ))]
        };
        let target = self
            .program
            .typed
            .authored_machine_target(symbol)
            .map_err(rejected)?;
        let original = self
            .original_program
            .typed
            .authored_machine_target(symbol)
            .map_err(rejected)?;
        if target != original {
            return Err(rejected(
                "authored target context differs from checked source custody",
            ));
        }
        Ok(target)
    }
}

impl<'a> SourceDeclarationInput<'a> {
    pub(crate) fn authored_selections(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> impl Iterator<Item = &AuthoredDeclarationSelection> + '_ {
        self.authored_selection_occurrences(expression)
            .filter_map(|occurrence| self.authored_declaration_selections().get(occurrence))
    }
}

/// Converts review requests without substituting typed source for checked
/// contract custody. Conversion failures remain the projection's full error.
pub trait IntoPackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>>;
}

impl<'a> IntoPackageReviewInput<'a> for &'a CheckedCompilation {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        PackageReviewInput::supplied(self, self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for PackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        Ok(self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for &PackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        Ok(*self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        self
    }
}

impl<'a> std::ops::Deref for PackageReviewInput<'a> {
    type Target = SourceDeclarationInput<'a>;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl std::ops::Deref for SourceDeclarationInput<'_> {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

#[cfg(test)]
mod route_review_input_tests {
    use super::*;

    #[test]
    fn failed_review_input_keeps_the_complete_projection_error() {
        let input: Result<PackageReviewInput<'static>, Vec<Diagnostic>> =
            Err(vec![Diagnostic::error(
                "route contract custody is unavailable",
            )]);
        let errors = match input.into_review_input() {
            Ok(_) => panic!("an unavailable input cannot produce a review namespace"),
            Err(errors) => errors,
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "route contract custody is unavailable");
    }
}

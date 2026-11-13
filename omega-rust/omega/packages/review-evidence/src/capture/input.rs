//! Retained package inputs for projections that already have portable evidence.
//!
//! Nominal target proof reconstruction belonged to the deleted source checker;
//! callers needing that evidence refuse until the lowered-form product supplies it.

use checked_trees::CheckedTrees;
use checked_trees::typed_trees::service_declarations::PreparedServiceCatalog;
use compilation_products::CheckedCompilation;
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
    pub(crate) fn checked(
        program: &'a CheckedTrees,
        sources: &'a package_compilation::CheckedSourceCustody,
    ) -> Self {
        Self {
            program,
            original_program: program,
            checking_input: program,
            original_checking_input: program,
            mathematical_calls: None,
            original_mathematical_calls: None,
            package: sources.package_identity(),
            exact_toolchain_sources: sources.exact_toolchain_sources(),
        }
    }

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
    pub fn supplied(program: &'a CheckedTrees, custody: &'a CheckedCompilation) -> Self {
        Self {
            source: SourceDeclarationInput {
                program,
                original_program: custody,
                checking_input: program,
                original_checking_input: &**custody,
                mathematical_calls: None,
                original_mathematical_calls: None,
                package: custody.sources().package_identity(),
                exact_toolchain_sources: custody.sources().exact_toolchain_sources(),
            },
            custody,
            external_providers: None,
            service_declarations: None,
        }
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
            "no product route yet: package review requires lowered-form evidence",
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

impl<'a> From<&'a CheckedCompilation> for PackageReviewInput<'a> {
    fn from(compilation: &'a CheckedCompilation) -> Self {
        Self::supplied(compilation, compilation)
    }
}

impl<'a> From<&PackageReviewInput<'a>> for PackageReviewInput<'a> {
    fn from(input: &PackageReviewInput<'a>) -> Self {
        *input
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

/*
Legacy declaration projection inputs keep candidate and original checked data
separate. Product review instead enters through `PublishedPackageReviewInput`,
which retains the accepted Terminal artifact and immutable typed owner. There
is no conversion from a new Product into these legacy checker queries.
*/

use checked_trees::CheckedTrees;
use checked_trees::typed_trees::service_declarations::PreparedServiceCatalog;
use compilation_products::CheckedCompilation;
use language_semantics::declaration_selection::AuthoredDeclarationSelection;
use provider_planning::ExternalProviderDeclarations;
use provider_planning::ProviderPlanningContext;
use provider_planning::service_declarations::ServiceDeclarations;
use typed_trees::machine::Machine;
use validation::expression_owners::OwnerEnvironmentIndex;

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
    pub(crate) contract_owner_queries: Option<&'a ContractOwnerQueries<'a>>, // Batch-local owner index scratch.
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
            contract_owner_queries: None,
            mathematical_calls: self.original_mathematical_calls,
            ..self
        }
    }
}

impl<'a> PackageReviewInput<'a> {
    pub(super) fn project_external_providers(
        &self,
    ) -> Result<ExternalProviderDeclarations, Vec<diagnostics::Diagnostic>> {
        Err(vec![diagnostics::Diagnostic::error(
            "legacy source review has no independently published native-provider contract receipts",
        )])
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
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<symbols::SymbolHandle> {
        match self.contract_owner_queries {
            Some(queries) => queries.derive_checked_nominal_call_target(expression),
            None => typed_trees_to_checked_trees::derive_checked_nominal_call_target(
                self.checking_input,
                &self.facts,
                expression,
                None,
            ),
        }
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

pub(crate) struct ContractOwnerQueries<'program> {
    program: &'program dyn validation::CheckingInput,
    facts: &'program checked_trees::CheckFacts,
    owners: std::cell::OnceCell<OwnerEnvironmentIndex>,
}

impl<'program> ContractOwnerQueries<'program> {
    pub(crate) fn new(
        program: &'program dyn validation::CheckingInput,
        facts: &'program checked_trees::CheckFacts,
    ) -> Self {
        Self {
            program,
            facts,
            owners: std::cell::OnceCell::new(),
        }
    }

    fn derive_checked_nominal_call_target(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<symbols::SymbolHandle> {
        self.program.expression_table.call_node(expression)?;
        let owners = self.owners.get_or_init(|| {
            OwnerEnvironmentIndex::new(self.program, &self.facts.proof.contract_facts)
        });
        typed_trees_to_checked_trees::derive_checked_nominal_call_target(
            self.program,
            self.facts,
            expression,
            Some(owners),
        )
    }
}

#[cfg(test)]
mod owner_query_tests;

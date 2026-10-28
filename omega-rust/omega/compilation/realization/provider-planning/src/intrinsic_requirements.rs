//! Omega plan adapters over Psi's source-owned requirement view.
//! Source declaration classification lives in typed trees; provider schema and
//! plan association remain here until their callers consume retained facts.

use checked_trees::typed_trees;
use checked_trees::typed_trees::TypedTrees;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema};
use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirement as SourceRequirement;
pub use typed_trees::typed_trees::declarations::intrinsic_requirements::IntrinsicRequirementKind;
pub use typed_trees::typed_trees::declarations::numeric_intrinsics::primitive_numeric_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicRequirement<'typed>(SourceRequirement<'typed>);

impl<'typed> std::ops::Deref for IntrinsicRequirement<'typed> {
    type Target = SourceRequirement<'typed>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'typed> IntrinsicRequirement<'typed> {
    /// Return the source-owned view without copying its semantic identity.
    pub fn into_source(self) -> SourceRequirement<'typed> {
        self.0
    }

    /// Review's token-only float conformance lane is deliberately narrower
    /// than the complete numeric requirement catalog.
    pub fn closed_float_execution(
        &self,
        typed: &TypedTrees,
    ) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        let meaning = typed.numeric_intrinsic_execution(self.symbol)?;
        matches!(
            meaning,
            language_semantics::numeric_intrinsics::NumericIntrinsic::FloatBinary { .. }
        )
        .then(|| meaning.into())
    }

    /// Classify the existing numeric catalog against the complete signature.
    /// This supplies meaning only; exact provider/overload/source joins remain
    /// independent obligations at each admission boundary.
    pub fn execution_identity(
        &self,
        typed: &TypedTrees,
    ) -> Option<effects::CompilerIntrinsicExecutionIdentity> {
        typed
            .numeric_intrinsic_execution(self.symbol)
            .map(Into::into)
    }

    pub fn from_operator(
        typed: &'typed TypedTrees,
        operator: &'typed typed_trees::operator::OperatorDefinition,
    ) -> Option<Self> {
        SourceRequirement::from_operator(typed, operator).map(Self)
    }

    pub fn by_symbol(typed: &'typed TypedTrees, symbol: symbols::SymbolHandle) -> Option<Self> {
        SourceRequirement::by_symbol(typed, symbol).map(Self)
    }

    /// Whether a provider-plan schema is exactly this requirement's slot.
    pub(crate) fn schema_binds(
        &self,
        context: crate::ProviderPlanningContext<'_>,
        schema: &ServiceSchema,
    ) -> bool {
        let typed = context.typed;
        match self.kind {
            IntrinsicRequirementKind::Operator => self.as_operator().is_some_and(|operator| {
                crate::service_schema::schema_binds_exact_boundary_operator(
                    context, schema, operator,
                )
            }),
            IntrinsicRequirementKind::TopLevelRequirement => {
                typed.machine(self.symbol).is_some_and(|requirement| {
                    crate::service_schema::schema_binds_exact_boundary_requirement(
                        context,
                        schema,
                        requirement,
                    )
                })
            }
        }
    }

    /// Whether a selected plan's single schema method and row bind exactly
    /// this requirement: the operator schema is the overload identity with
    /// its one `realize` method; the requirement schema is the requirement
    /// path with its leaf-named method and normalized overload identity. A
    /// token machine uses that overload identity for its family coordinate's
    /// schema slot as well.
    pub fn plan_row_binds(
        &self,
        plan: &ProviderPlan,
        method: &ServiceMethod,
        row: &ProviderPlanRow,
    ) -> bool {
        effects::external_providers::RequirementPlanIdentity {
            overload: &self.requirement_identity,
            package: self.package_identity,
            schema: &self.schema_identity(),
            method: self.method_name(),
            owner: self.method_owner(),
        }
    }

    /// Whether the machine named by a plan row's intrinsic binding is an
    /// external leaf whose `satisfies` edge rejoins exactly this requirement.
    pub fn intrinsic_realization_matches(
        &self,
        typed: &TypedTrees,
        realization_machine_identity: &str,
    ) -> bool {
        // The conformance tests read fields; the overload identity normalizes
        // the whole signature, so it runs only for a machine that could bind
        // an external realization at all.
        typed.machines().iter().any(|machine| {
            typed.sealed_numeric_realization(machine.symbol, self.symbol).is_some()
                && typed
                    .machine_callable_identity_text(machine)
                    .is_some_and(|identity| identity == realization_machine_identity)
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| match self.kind {
                        IntrinsicRequirementKind::Operator => {
                            checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                                typed, machine, conformance,
                            )
                            .is_some_and(|resolved| resolved.symbol == self.symbol)
                        }
                        IntrinsicRequirementKind::TopLevelRequirement => matches!(
                            checked_trees::typed_trees::machine::resolve_satisfied_declaration(
                                typed, machine, conformance,
                            ),
                            Some(checked_trees::typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                requirement,
                            )) if requirement.symbol == self.symbol
                        ),
                    })
        })
    }
}

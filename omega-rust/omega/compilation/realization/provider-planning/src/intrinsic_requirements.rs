//! One requirement view over intrinsic-realizable declarations and token views.
//!
//! A compiler intrinsic realizes either a named boundary operator
//! (`boundary operator F32::negate(value: f32) -> f32`) or a
//! receiver-free top-level boundary requirement (`boundary requirement
//! F32::negate(value: f32) -> f32`). The sealed realization catalog, the
//! plan-schema joins and selected execution need
//! the same facts of the requirement -- its exact symbol, `Owner::name` path,
//! entry parameters, result type and package -- so this view carries them
//! once and each consumer keys on it instead of on one declaration species.
//! A token-bearing boundary machine retains its ordinary requirement identity
//! and exposes its token signature for closed primitive-meaning classification.
//!
//! `execution_identity` classifies one validated execution child for both
//! planning and dispatch. Token semantics and named numeric signatures have
//! distinct catalog rules, but no consumer decodes a diagnostic label.

use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::signature::StateParameter;
use checked_trees::typed_trees::types::TypeReferenceHandle;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema};

pub use checked_trees::typed_trees::typed_trees::declarations::numeric_intrinsics::primitive_numeric_type;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicRequirementKind {
    /// A named `boundary operator` declaration.
    Operator,
    /// A receiver-free top-level `boundary requirement` machine.
    TopLevelRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicRequirement<'typed> {
    pub kind: IntrinsicRequirementKind,
    /// The declaration symbol: the operator or the requirement machine.
    pub symbol: symbols::SymbolHandle,
    /// The symbol a direct call retains as its target: the operator itself,
    /// or the requirement's entry state.
    pub call_target: symbols::SymbolHandle,
    /// `Owner::name`, split.
    pub namespace: String,
    pub name: String,
    pub parameters: &'typed [StateParameter],
    pub return_type: TypeReferenceHandle,
    pub package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// The identity a provider-plan row binds: the canonical operator
    /// overload identity, or the requirement's normalized machine overload.
    pub requirement_identity: String,
    operator: Option<&'typed checked_trees::typed_trees::operator::OperatorDefinition>,
}

impl<'typed> IntrinsicRequirement<'typed> {
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
        operator: &'typed checked_trees::typed_trees::operator::OperatorDefinition,
    ) -> Option<Self> {
        if !operator.is_boundary {
            return None;
        }
        // A token view is the machine's signature, not a second requirement.
        // Its provider identity and satisfaction edges belong to that machine.
        if let Some(requirement) = typed.machine(operator.symbol) {
            return Self::from_requirement(typed, requirement);
        }
        let [namespace, name] = typed.operator_path_members(operator.name) else {
            return None;
        };
        let requirement_identity =
            checked_trees::typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            );
        if requirement_identity.is_empty() {
            return None;
        }
        Some(Self {
            kind: IntrinsicRequirementKind::Operator,
            symbol: operator.symbol,
            call_target: operator.symbol,
            namespace: namespace.as_str().to_owned(),
            name: name.as_str().to_owned(),
            parameters: typed.operator_parameters(operator),
            return_type: operator.return_type,
            package_identity: typed.symbols.symbol_package_identity(operator.symbol),
            requirement_identity,
            operator: Some(operator),
        })
    }

    pub(crate) fn from_requirement(
        typed: &'typed TypedTrees,
        requirement: &'typed checked_trees::typed_trees::machine::Machine,
    ) -> Option<Self> {
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || requirement.body_is_present
            || !typed.machine_type_parameters(requirement).is_empty()
        {
            return None;
        }
        let [entry] = typed.machine_states(requirement) else {
            return None;
        };
        let parameters = typed.state_parameters(entry);
        let (namespace, name) = requirement.name.as_str().rsplit_once("::")?;
        if namespace.is_empty() || name.is_empty() || namespace.contains("::") {
            return None;
        }
        let requirement_identity = typed.machine_callable_identity_text(requirement)?;
        if requirement_identity.is_empty() {
            return None;
        }
        Some(Self {
            kind: IntrinsicRequirementKind::TopLevelRequirement,
            symbol: requirement.symbol,
            call_target: entry.symbol,
            namespace: namespace.to_owned(),
            name: name.to_owned(),
            parameters,
            return_type: entry.return_type,
            package_identity: typed.symbols.symbol_package_identity(requirement.symbol),
            requirement_identity,
            operator: typed.machine_token_binding_by_symbol(requirement.symbol),
        })
    }

    /// The declaration behind `symbol`: an operator symbol or a requirement
    /// machine symbol. A requirement's entry-state symbol also resolves, so
    /// a retained call target selects the same view.
    pub fn by_symbol(typed: &'typed TypedTrees, symbol: symbols::SymbolHandle) -> Option<Self> {
        if !symbol.is_valid() {
            return None;
        }
        if let Some(operator) = typed.operator_definition_by_symbol(symbol) {
            return Self::from_operator(typed, operator);
        }
        typed
            .machines()
            .iter()
            .find(|machine| {
                machine.symbol == symbol
                    || typed
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == symbol)
            })
            .and_then(|machine| Self::from_requirement(typed, machine))
    }

    pub fn display(&self) -> String {
        format!("{}::{}", self.namespace, self.name)
    }

    /// The operator signature, including a boundary machine's same-symbol
    /// token view. Token meaning does not change the machine's provider slot.
    pub fn as_operator(
        &self,
    ) -> Option<&'typed checked_trees::typed_trees::operator::OperatorDefinition> {
        self.operator
    }

    /// Whether a provider-plan schema is exactly this requirement's slot.
    pub(crate) fn schema_binds(
        &self,
        context: crate::ProviderPlanningContext<'_>,
        schema: &ServiceSchema,
    ) -> bool {
        let typed = context.typed;
        match self.kind {
            IntrinsicRequirementKind::Operator => self.operator.is_some_and(|operator| {
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
        let owner_matches = plan.schema.trait_package_identity == self.package_identity
            && method.requirement_owner_package_identity == self.package_identity
            && plan.schema.identity_text(method.requirement_identity) == self.requirement_identity
            && plan.identities.text(row.requirement_identity) == self.requirement_identity
            && plan.schema.row_binds_method(row, method, &plan.identities);
        match self.kind {
            IntrinsicRequirementKind::Operator => {
                owner_matches
                    && plan.schema.identity_text(plan.schema.trait_name)
                        == self.requirement_identity
                    && method.name == "realize"
                    && method.requirement_owner == self.requirement_identity
            }
            IntrinsicRequirementKind::TopLevelRequirement => {
                owner_matches
                    && if self.operator.is_some() {
                        plan.schema.identity_text(plan.schema.trait_name)
                            == self.requirement_identity
                    } else {
                        plan.schema.identity_text(plan.schema.trait_name) == self.display()
                    }
                    && method.name == self.name
                    && method.requirement_owner == self.namespace
            }
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

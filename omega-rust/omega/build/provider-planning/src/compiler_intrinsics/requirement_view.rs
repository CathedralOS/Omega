//! One requirement view over both intrinsic-realizable declaration species.
//!
//! A compiler intrinsic realizes either a named boundary operator
//! (`boundary operator F32::negate(value: f32) -> f32`) or a public
//! receiver-free top-level boundary requirement (`boundary requirement
//! F32::negate(value: f32) -> f32`). The sealed realization catalog, the
//! diagnostic labels, the plan-schema joins and the execution rewrites need
//! the same facts of the requirement -- its exact symbol, `Owner::name` path,
//! entry parameters, result type and package -- so this view carries them
//! once and each consumer keys on it instead of on one declaration species.

use effects::provider_plan::{ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema};
use typed_trees::TypedTrees;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicRequirementKind {
    /// A named `boundary operator` declaration.
    Operator,
    /// A public receiver-free top-level `boundary requirement` machine.
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
    operator: Option<&'typed typed_trees::operator::OperatorDefinition>,
}

impl<'typed> IntrinsicRequirement<'typed> {
    pub fn from_operator(
        typed: &'typed TypedTrees,
        operator: &'typed typed_trees::operator::OperatorDefinition,
    ) -> Option<Self> {
        if !operator.is_boundary {
            return None;
        }
        let [namespace, name] = typed.operator_path_members(operator.name) else {
            return None;
        };
        let requirement_identity =
            typed_trees::operator::boundary_operator_requirement_identity(typed, operator);
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

    pub fn from_requirement(
        typed: &'typed TypedTrees,
        requirement: &'typed typed_trees::machine::Machine,
    ) -> Option<Self> {
        if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
            || !requirement.is_public
            || requirement.body_is_present
            || !typed.machine_type_parameters(requirement).is_empty()
        {
            return None;
        }
        let [entry] = typed.machine_states(requirement) else {
            return None;
        };
        let parameters = typed.state_parameters(entry);
        if parameters.iter().any(|parameter| parameter.is_self) {
            return None;
        }
        let (namespace, name) = requirement.name.as_str().rsplit_once("::")?;
        if namespace.is_empty() || name.is_empty() || namespace.contains("::") {
            return None;
        }
        let requirement_identity = typed
            .normalized_machine_overload_identity(requirement)?
            .identity();
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
            operator: None,
        })
    }

    /// The declaration behind `symbol`: an operator symbol or a requirement
    /// machine symbol. A requirement's entry-state symbol also resolves, so
    /// a retained call target selects the same view.
    pub fn by_symbol(typed: &'typed TypedTrees, symbol: symbols::SymbolHandle) -> Option<Self> {
        if !symbol.is_valid() {
            return None;
        }
        if let Some(operator) = typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == symbol)
        {
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

    /// The operator declaration when this view is the operator species.
    pub fn as_operator(&self) -> Option<&'typed typed_trees::operator::OperatorDefinition> {
        self.operator
    }

    /// Whether a provider-plan schema is exactly this requirement's slot.
    pub fn schema_binds(&self, typed: &TypedTrees, schema: &ServiceSchema) -> bool {
        match self.kind {
            IntrinsicRequirementKind::Operator => self.operator.is_some_and(|operator| {
                crate::service_schema::schema_binds_exact_boundary_operator(typed, schema, operator)
            }),
            IntrinsicRequirementKind::TopLevelRequirement => typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == self.symbol)
                .is_some_and(|requirement| {
                    crate::service_schema::schema_binds_exact_boundary_requirement(
                        typed,
                        schema,
                        requirement,
                    )
                }),
        }
    }

    /// Whether a selected plan's single schema method and row bind exactly
    /// this requirement: the operator schema is the overload identity with
    /// its one `realize` method; the requirement schema is the requirement
    /// path with its leaf-named method and normalized overload identity.
    pub fn plan_row_binds(
        &self,
        plan: &ProviderPlan,
        method: &ServiceMethod,
        row: &ProviderPlanRow,
    ) -> bool {
        let owner_matches = plan.schema.trait_package_identity == self.package_identity
            && method.requirement_owner_package_identity == self.package_identity
            && method.requirement_identity == self.requirement_identity
            && row.requirement_identity == self.requirement_identity
            && plan.schema.row_binds_method(row, method);
        match self.kind {
            IntrinsicRequirementKind::Operator => {
                owner_matches
                    && plan.schema.trait_name == self.requirement_identity
                    && method.name == "realize"
                    && method.requirement_owner == self.requirement_identity
            }
            IntrinsicRequirementKind::TopLevelRequirement => {
                owner_matches
                    && plan.schema.trait_name == self.display()
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
        typed.machines().iter().any(|machine| {
            typed
                .normalized_machine_overload_identity(machine)
                .is_some_and(|identity| identity.identity() == realization_machine_identity)
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| conformance.external_binding.is_some())
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| match self.kind {
                        IntrinsicRequirementKind::Operator => {
                            typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                                typed, machine, conformance,
                            )
                            .is_some_and(|resolved| resolved.symbol == self.symbol)
                        }
                        IntrinsicRequirementKind::TopLevelRequirement => matches!(
                            typed_trees::machine::resolve_satisfied_declaration(
                                typed, machine, conformance,
                            ),
                            Some(typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                requirement,
                            )) if requirement.symbol == self.symbol
                        ),
                    })
        })
    }
}

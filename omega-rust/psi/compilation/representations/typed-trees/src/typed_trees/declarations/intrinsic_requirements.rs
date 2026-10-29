//! Source-owned view of numeric-catalog requirement declarations.
//! Schema and method coordinates are declaration facts, independent of provider
//! selection, target plans, or execution authority.

use crate::TypedTrees;
use crate::signature::StateParameter;
use crate::types::TypeReferenceHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicRequirementKind {
    /// A named `boundary operator` declaration.
    Operator,
    /// A receiver-free top-level `boundary requirement` machine.
    TopLevelRequirement,
    /// Exact closed-catalogue boundary-trait requirement.
    Trait(symbols::SymbolHandle),
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
    operator: Option<&'typed crate::operator::OperatorDefinition>,
}

impl<'typed> IntrinsicRequirement<'typed> {
    pub fn from_operator(
        typed: &'typed TypedTrees,
        operator: &'typed crate::operator::OperatorDefinition,
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
            crate::operator::boundary_operator_requirement_identity(typed, operator);
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
        requirement: &'typed crate::machine::Machine,
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
        if let Some((owner, requirement)) = typed.trait_machine_signature(symbol) {
            typed.sealed_numeric_requirement(owner, requirement)?;
            return Some(Self {
                kind: IntrinsicRequirementKind::Trait(owner.symbol),
                symbol: requirement.symbol,
                call_target: requirement.symbol,
                namespace: typed.trait_declaration_path(owner),
                name: requirement.name.as_str().to_owned(),
                parameters: typed.state_signature_parameters(requirement),
                return_type: requirement.return_type,
                package_identity: typed.symbols.symbol_package_identity(requirement.symbol),
                requirement_identity: typed
                    .trait_requirement_callable_identity_text(owner, requirement),
                operator: None,
            });
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
    pub fn as_operator(&self) -> Option<&'typed crate::operator::OperatorDefinition> {
        self.operator
    }

    /// Expected schema coordinate, retained before source is discarded.
    pub fn schema_identity(&self) -> std::borrow::Cow<'_, str> {
        if matches!(self.kind, IntrinsicRequirementKind::Trait(_)) {
            return std::borrow::Cow::Borrowed(&self.namespace);
        }
        if self.kind == IntrinsicRequirementKind::Operator || self.operator.is_some() {
            std::borrow::Cow::Borrowed(&self.requirement_identity)
        } else {
            std::borrow::Cow::Owned(self.display())
        }
    }

    pub fn method_name(&self) -> &str {
        match self.kind {
            IntrinsicRequirementKind::Operator => "realize",
            IntrinsicRequirementKind::TopLevelRequirement | IntrinsicRequirementKind::Trait(_) => {
                &self.name
            }
        }
    }

    pub fn method_owner(&self) -> &str {
        match self.kind {
            IntrinsicRequirementKind::Operator => &self.requirement_identity,
            IntrinsicRequirementKind::TopLevelRequirement | IntrinsicRequirementKind::Trait(_) => {
                &self.namespace
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
            typed
                .sealed_numeric_realization(machine.symbol, self.symbol)
                .is_some()
                && typed
                    .machine_callable_identity_text(machine)
                    .is_some_and(|identity| identity == realization_machine_identity)
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| match self.kind {
                        IntrinsicRequirementKind::Operator => {
                            crate::operator::resolve_satisfied_boundary_operator_for_conformance(
                                typed,
                                machine,
                                conformance,
                            )
                            .is_some_and(|resolved| resolved.symbol == self.symbol)
                        }
                        IntrinsicRequirementKind::Trait(owner) => matches!(
                            crate::machine::resolve_satisfied_declaration(typed, machine, conformance),
                            Some(crate::machine::SatisfiedDeclaration::Trait { definition, requirement })
                                if definition.symbol == owner && requirement.symbol == self.symbol
                        ),
                        IntrinsicRequirementKind::TopLevelRequirement => matches!(
                            crate::machine::resolve_satisfied_declaration(
                                typed, machine, conformance,
                            ),
                            Some(crate::machine::SatisfiedDeclaration::TopLevelRequirement(
                                requirement,
                            )) if requirement.symbol == self.symbol
                        ),
                    })
        })
    }
}

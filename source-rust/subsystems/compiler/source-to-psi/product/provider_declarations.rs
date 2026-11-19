//! Rejoin the provider declarations selected by Build to their exact source and closed operands.

#[path = "provider_declarations/requests.rs"]
mod requests;
#[path = "provider_declarations/retained.rs"]
mod retained;
pub(crate) use retained::rejoin_provider_defaults;
#[cfg(test)]
#[path = "provider_declarations/tests.rs"]
mod tests;

pub use super::retained_application::RetainedProductApplication;
use crate::declaration_selection::BuildSelectionOrigin;
use crate::provider_choice_inputs::application_id;
pub use crate::provider_choice_inputs::{
    ProviderApplicationId, ProviderOperatorFamilyCoordinate, ProviderOperatorFamilySelection,
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject, TargetProviderDefaults,
    source_application,
};
pub use crate::source_state::provider_declarations::ProviderPlanningContext;
use diagnostics::Diagnostic;
pub use requests::collect_product_provider_selections;
pub use semantic_vocabulary::source_selections::CompositionMode;
use semantic_vocabulary::source_selections::ProductTarget;
use source_admission::RetainedDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProviderSelection {
    pub subject: RetainedProviderSubject,
    pub provider: RetainedDeclaration,
    pub provider_identity: String,
    pub provider_application: Option<RetainedProductApplication>,
    pub target: ProductTarget,
    pub composition_mode: CompositionMode,
    pub origin: BuildSelectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainedProviderSubject {
    Declaration {
        declaration: RetainedDeclaration,
        identity: String,
        application: Option<RetainedProductApplication>,
    },
    OperatorFamily(RetainedOperatorFamily),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedOperatorCoordinate {
    pub declaration: RetainedDeclaration,
    pub requirement_identity: String,
    pub static_parameter_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedOperatorFamily {
    pub canonical_path: String,
    coordinates: Vec<RetainedOperatorCoordinate>,
}

impl RetainedOperatorFamily {
    pub fn new(
        canonical_path: String,
        coordinates: Vec<RetainedOperatorCoordinate>,
    ) -> Result<Self, &'static str> {
        if canonical_path.is_empty()
            || coordinates.is_empty()
            || coordinates.iter().any(|member| {
                member.requirement_identity.is_empty()
                    || !matches!(
                        member.declaration.kind(),
                        symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
                    )
            })
            || coordinates
                .windows(2)
                .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
        {
            return Err("retained operator family needs its complete canonical coordinate roster");
        }
        Ok(Self {
            canonical_path,
            coordinates,
        })
    }

    pub fn coordinates(&self) -> &[RetainedOperatorCoordinate] {
        &self.coordinates
    }
}

impl RetainedProviderSubject {
    /// All files containing original declarations belong to the selected family.
    pub fn declarations(&self) -> impl Iterator<Item = &RetainedDeclaration> {
        let (single, family): (_, &[RetainedOperatorCoordinate]) = match self {
            Self::Declaration { declaration, .. } => (Some(declaration), &[]),
            Self::OperatorFamily(family) => (None, family.coordinates()),
        };
        single
            .into_iter()
            .chain(family.iter().map(|member| &member.declaration))
    }

    pub fn application(&self) -> Option<&RetainedProductApplication> {
        match self {
            Self::Declaration { application, .. } => application.as_ref(),
            Self::OperatorFamily(_) => None,
        }
    }
}

pub(crate) fn identity(
    context: ProviderPlanningContext<'_>,
    symbol: symbols::SymbolHandle,
    authored_path: &str,
) -> ProviderSelectionIdentity {
    let symbols = &context.typed.symbols;
    ProviderSelectionIdentity {
        symbol,
        application: Default::default(),
        applied: false,
        application_identity: None,
        package: symbols.symbol_package_identity(symbol),
        canonical_path: symbols.display_path(symbol, "::"),
        authored_path: authored_path.to_owned(),
    }
}

/// Classifies the slot and checks the supported static declaration form.
/// Defaults, explicit Build choices and retained dependency choices share
/// this validation; a default's namespace grants no provider value.
pub(crate) fn form(
    context: ProviderPlanningContext<'_>,
    slot: ProviderSelectionIdentity,
    provider: ProviderSelectionIdentity,
    origin: BuildSelectionOrigin,
    target: Option<ProductTarget>,
    composition_mode: CompositionMode,
) -> Result<ProviderSelection, Diagnostic> {
    use symbols::SymbolKind;
    let typed = context.typed;
    if !slot.symbol.is_valid() || !provider.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "provider selection `{}` for `{}` does not resolve to declarations",
            provider.authored_path, slot.authored_path
        )));
    }
    if !context.contains(slot.symbol) || !context.contains(provider.symbol) {
        return Err(Diagnostic::error(format!(
            "provider selection subject or provider is outside the {:?} planning context",
            context.scope
        )));
    }
    let slot_kind = typed.symbols.get(slot.symbol).kind;
    // A slot or provider with parameters is selected only as one
    // closed application of them, which its identity then states.
    if slot.application_identity.is_none()
        && typed
            .trait_definition_by_symbol(slot.symbol)
            .is_some_and(|definition| !definition.type_parameters.is_empty())
    {
        return Err(Diagnostic::error(format!(
            "no product route yet: provider slot `{}` requires its complete static application",
            slot.authored_path,
        )));
    }
    let subject = if slot_kind == SymbolKind::Trait
        && typed
            .traits()
            .iter()
            .any(|definition| definition.symbol == slot.symbol && definition.is_boundary)
    {
        ProviderSelectionSubject::BoundaryTrait(slot)
    } else if slot_kind == SymbolKind::Machine
        && typed.machines().iter().any(|requirement| {
            requirement.symbol == slot.symbol
                && requirement.spelling.is_none()
                && requirement.supply_mode
                    == source_declarations::MachineSupplyMode::TopLevelRequirement
        })
    {
        ProviderSelectionSubject::BoundaryRequirement(slot)
    } else if slot_kind == SymbolKind::Operator
        || typed
            .machine_token_bindings()
            .iter()
            .any(|binding| binding.is_boundary && binding.symbol == slot.symbol)
    {
        ProviderSelectionSubject::BoundaryOperatorFamily(operator_family(context, slot)?)
    } else {
        return Err(Diagnostic::error(format!(
            "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
            slot.authored_path
        )));
    };
    let implementation = typed
        .conformances()
        .iter()
        .find(|definition| definition.symbol == provider.symbol);
    if implementation.is_none() {
        return Err(Diagnostic::error(format!(
            "provider selection `{}` is not a static operation record from a named implementation; a data type is not a provider value",
            provider.authored_path
        )));
    }
    if provider.application_identity.is_none()
        && implementation.is_some_and(|implementation| !implementation.type_parameters.is_empty())
    {
        return Err(Diagnostic::error(format!(
            "no product route yet: provider `{}` requires its complete static application",
            provider.authored_path,
        )));
    }
    // A named implementation with lifetime parameters provides a
    // requirement its parent trait declares only when the parent's
    // lifetimes are spelled by the implementation's own.
    if let Some(implementation) =
        implementation.filter(|definition| definition.type_parameters.is_empty())
    {
        let root = typed.trait_definition_by_symbol(implementation.trait_symbol);
        let arguments = implementation.trait_lifetime_arguments.as_slice();
        let own = implementation.lifetime_parameters.len();
        let spelled = root.is_some_and(|root| {
            arguments.len() == root.lifetime_parameters.len()
                && arguments
                    .iter()
                    .all(|ordinal| usize::try_from(*ordinal).is_ok_and(|ordinal| ordinal < own))
        });
        for row in &implementation.rows {
            let inherited_without_lifetimes = typed
                .trait_definition_by_symbol(row.declaring_trait)
                .is_some_and(|declaring| declaring.lifetime_parameters.is_empty());
            let direct = root.is_some_and(|root| root.symbol == row.declaring_trait);
            if !spelled || !(direct || inherited_without_lifetimes) {
                return Err(Diagnostic::error(format!(
                    "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                    provider.authored_path, row.requirement_name,
                )));
            }
        }
    }
    Ok(ProviderSelection {
        target,
        subject,
        provider_type: provider,
        composition_mode,
        origin,
    })
}

/// One visible representative selects the complete family in that package and
/// Product instance. A particular choice cannot omit an overload.
fn operator_family(
    context: ProviderPlanningContext<'_>,
    slot: ProviderSelectionIdentity,
) -> Result<ProviderOperatorFamilySelection, Diagnostic> {
    let typed = context.typed;
    let symbols = &typed.symbols;
    let representative =
        crate::source_state::operator::declaration_by_symbol(typed, slot.symbol)
            .ok_or_else(|| Diagnostic::error("selected operator has no authored declaration"))?;
    let path = typed.operator_path_string(representative.name);
    let mut coordinates = Vec::new();
    for operator in typed.all_operators().chain(typed.machine_token_bindings()) {
        let member = operator.symbol;
        if !operator.is_boundary
            || !context.contains(member)
            || symbols.symbol_package_identity(member) != slot.package
            || !symbols.same_symbol_source_package(member, slot.symbol)
            || typed.operator_path_string(operator.name) != path
        {
            continue;
        }
        if typed.machine(member).is_some_and(|machine| {
            machine.supply_mode != source_declarations::MachineSupplyMode::TopLevelRequirement
        }) {
            return Err(Diagnostic::error(format!(
                "boundary-operator family `{path}` contains a token machine without requirement supply",
            )));
        }
        let coordinate = typed.boundary_operator_requirement_callable_identity(operator);
        if !coordinate.is_valid() {
            return Err(Diagnostic::error(format!(
                "boundary-operator family `{path}` has a coordinate without a complete callable identity",
            )));
        }
        coordinates.push(ProviderOperatorFamilyCoordinate {
            symbol: member,
            requirement_identity: typed.callable_identity_text(coordinate),
            static_parameter_count: operator.lifetime_parameters.len()
                + typed.operator_type_parameters(operator).len(),
        });
    }
    ProviderOperatorFamilySelection::new(slot.package, path, slot.authored_path, coordinates)
        .map_err(Diagnostic::error)
}

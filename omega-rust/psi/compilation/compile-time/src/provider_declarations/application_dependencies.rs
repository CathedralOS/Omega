//! Declaration-only provider edges for checking an authored deployment scope.
//!
//! The retained choice describes a complete prepared implementation. This
//! projection grants neither candidate selection nor installed supply. Only an
//! actual requirement call activates its matching member in the shared graph;
//! retaining a provider description is not an invocation of all of its bodies.

use super::{
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject, TargetProviderDefaults,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::trait_definition::AppliedTraitScope;

pub(crate) fn source_application_dependencies(
    typed: &TypedTrees,
    target: &str,
    explicit: &[ProviderSelection],
    defaults: &[TargetProviderDefaults],
    candidates: &[(SymbolHandle, SymbolHandle)],
) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
    let applicable = |selection: &&ProviderSelection| {
        selection
            .target
            .is_none_or(|scope| scope.canonical_name() == target)
    };
    let overrides = explicit.iter().filter(applicable).collect::<Vec<_>>();
    let mut dependencies = Vec::new();
    for selection in overrides.iter().copied().chain(
        defaults
            .iter()
            .filter(|row| row.target.as_deref().is_none_or(|scope| scope == target))
            .flat_map(|row| &row.selections)
            .filter(applicable)
            // Exact whole-subject replacement is the declaration owner's
            // correspondence, not candidate availability or native inference.
            // Unresolved/conflicting choices remain obligations of ordinary
            // provider settlement; this dependency projection admits neither.
            .filter(|fallback| {
                !overrides
                    .iter()
                    .any(|selected| selected.subject.same_declaration_as(&fallback.subject))
            }),
    ) {
        let members = selection
            .provider_type
            .prepared_implementation_members(typed)?;
        let subject = Subject::new(typed, &selection.subject)?;
        for member in members {
            let template = typed
                .captured_machine_applications
                .iter()
                .find_map(|(_, capture)| (capture.instance == member).then_some(capture.template))
                .unwrap_or(member);
            for &(requirement, realization) in candidates {
                let dependency = (requirement, member);
                if realization == template
                    && subject.contains(typed, requirement)
                    && member_matches_subject(typed, selection, &subject, dependency)?
                    && !dependencies.contains(&dependency)
                {
                    dependencies.push(dependency);
                }
            }
        }
    }
    Ok(dependencies)
}

/// The subject's complete inherited declaration roster, independently of any
/// selected implementation. Applied scopes retain every Type/Const/region
/// argument; a provider's other satisfaction rows are not another subject.
enum Subject<'program> {
    Trait(AppliedTraitScope<'program>),
    Exact(Vec<SymbolHandle>),
}

impl<'program> Subject<'program> {
    fn new(
        typed: &'program TypedTrees,
        selected: &ProviderSelectionSubject,
    ) -> Result<Self, &'static str> {
        match selected {
            ProviderSelectionSubject::BoundaryTrait(identity) => {
                let scope = if identity.application.is_valid() {
                    let prepared = prepared_application(typed, identity)?;
                    if prepared.role != Some(language_semantics::declaration_selection::ProductOperandRole::BoundarySubject) {
                        return Err("layout dependency subject lost its boundary application role");
                    }
                    prepared.scope.scope(typed)
                } else {
                    typed
                        .trait_definition_by_symbol(identity.symbol)
                        .map(AppliedTraitScope::abstract_scope)
                };
                scope
                    .map(Self::Trait)
                    .ok_or("layout dependency subject lost its exact trait scope")
            }
            ProviderSelectionSubject::BoundaryRequirement(identity) => {
                Ok(Self::Exact(vec![identity.symbol]))
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                if !family
                    .replay_against_typed(super::ProviderPlanningContext::new(
                        typed,
                        source::DependencyScope::Product,
                    ))
                    .is_empty()
                {
                    return Err("layout dependency subject lost its complete operator family");
                }
                Ok(Self::Exact(
                    family.coordinates().iter().map(|row| row.symbol).collect(),
                ))
            }
        }
    }

    fn contains(&self, typed: &'program TypedTrees, requirement: SymbolHandle) -> bool {
        match self {
            Self::Exact(symbols) => symbols.contains(&requirement),
            Self::Trait(root) => {
                let mut found = false;
                root.visit(typed, &mut |scope| {
                    found |= typed
                        .trait_machine_signatures(scope.declaration)
                        .iter()
                        .any(|row| row.symbol == requirement);
                });
                found
            }
        }
    }
}

fn prepared_application<'program>(
    typed: &'program TypedTrees,
    identity: &ProviderSelectionIdentity,
) -> Result<&'program typed_trees::product_operand::ProductOperandApplication, &'static str> {
    if !typed
        .product_operand_applications
        .is_valid(identity.application)
    {
        return Err("layout dependency lost its prepared source application");
    }
    let prepared = typed.product_operand_applications.get(identity.application);
    let actual = typed
        .product_operand_application_identity(identity.application)?
        .and_then(|identity| typed.type_identity_text(identity));
    if prepared.declaration != identity.symbol || actual != identity.application_identity {
        return Err("layout dependency changed its complete source application");
    }
    Ok(prepared)
}

fn member_matches_subject<'program>(
    typed: &'program TypedTrees,
    selected: &ProviderSelection,
    subject: &Subject<'program>,
    (requirement, member): (SymbolHandle, SymbolHandle),
) -> Result<bool, &'static str> {
    let Subject::Trait(expected) = subject else {
        return Ok(true);
    };
    let matches = |actual: AppliedTraitScope<'program>| {
        actual
            .declaring_scope(typed, expected.declaration.symbol)
            .is_some_and(|actual| actual.same_application(typed, expected))
    };
    if let Some(code) = typed.conformance_by_symbol(selected.provider_type.symbol) {
        let scope = if selected.provider_type.application.is_valid() {
            let prepared = prepared_application(typed, &selected.provider_type)?;
            if prepared.role
                != Some(language_semantics::declaration_selection::ProductOperandRole::Provider)
            {
                return Err("layout dependency implementation lost its provider role");
            }
            prepared
                .implementation
                .as_ref()
                .and_then(|application| application.trait_scope.scope(typed))
        } else {
            AppliedTraitScope::from_conformance(typed, code)
        };
        return scope
            .map(matches)
            .ok_or("layout dependency implementation lost its exact trait scope");
    }
    let Some(machine) = typed.machine_by_symbol(member) else {
        return Err("layout dependency member lost its exact machine");
    };
    Ok(typed
        .machine_trait_conformances(machine)
        .iter()
        .filter(|edge| edge.requirement_symbol == requirement)
        .filter_map(|edge| AppliedTraitScope::from_machine_conformance(typed, machine, edge))
        .any(matches))
}

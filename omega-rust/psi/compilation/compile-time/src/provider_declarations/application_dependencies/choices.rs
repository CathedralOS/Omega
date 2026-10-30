//! Authored choice descriptions preserve tiers and complete static applications.
use super::super::{ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject};
use super::schemas::Key;
use language_semantics::declaration_selection::ProductOperandRole;
use language_semantics::declaration_selection::provider_choices::Declaration;
use typed_trees::TypedTrees;

pub(super) struct Choice<'source> {
    pub source: &'source ProviderSelection,
    provider: Key,
}
impl<'source> Choice<'source> {
    pub fn is_family(&self) -> bool {
        matches!(
            self.source.subject,
            ProviderSelectionSubject::BoundaryOperatorFamily(_)
        )
    }
    pub fn new(program: &TypedTrees, source: &'source ProviderSelection) -> Result<Self, String> {
        validate_identity(program, &source.provider_type, ProductOperandRole::Provider)?;
        match &source.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                validate_identity(program, identity, ProductOperandRole::BoundarySubject)?
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                let errors =
                    family.replay_against_typed(super::super::ProviderPlanningContext::new(
                        program,
                        source::DependencyScope::Product,
                    ));
                if !errors.is_empty() {
                    return Err(errors.join("\n"));
                }
            }
        }
        Ok(Self {
            source,
            provider: (
                source.provider_type.package,
                source.provider_type.canonical_path.clone(),
                source.provider_type.application_identity.clone(),
            ),
        })
    }
}
impl Declaration<Key, Key> for Choice<'_> {
    fn target(&self) -> Option<&str> {
        self.source.target.map(|target| target.canonical_name())
    }
    fn slots(&self, available: &[Key]) -> Vec<Key> {
        match &self.source.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                language_semantics::declaration_selection::provider_choices::selected_application_slots(
                    &(identity.package, identity.canonical_path.clone(), identity.application_identity.clone()),
                    available,
                )
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => family
                .coordinates()
                .iter()
                .map(|coordinate| {
                    (
                        family.package,
                        coordinate.requirement_identity.clone(),
                        None,
                    )
                })
                .collect(),
        }
    }
    fn provider(&self) -> &Key {
        &self.provider
    }
}

impl Declaration<Key, Key> for &Choice<'_> {
    fn target(&self) -> Option<&str> {
        Declaration::target(*self)
    }
    fn slots(&self, available: &[Key]) -> Vec<Key> {
        Declaration::slots(*self, available)
    }
    fn provider(&self) -> &Key {
        &self.provider
    }
}

fn validate_identity(
    program: &TypedTrees,
    identity: &ProviderSelectionIdentity,
    role: ProductOperandRole,
) -> Result<(), String> {
    if !identity.symbol.is_valid()
        || program.symbols.symbol_package_identity(identity.symbol) != identity.package
        || program.symbols.display_path(identity.symbol, "::") != identity.canonical_path
    {
        return Err("source provider description changed its exact nominal owner".into());
    }
    if identity.application.is_valid() || identity.application_identity.is_some() {
        if !program
            .product_operand_applications
            .is_valid(identity.application)
        {
            return Err("source provider description lost its prepared application".into());
        }
        let prepared = program
            .product_operand_applications
            .get(identity.application);
        let actual = program
            .product_operand_application_identity(identity.application)?
            .and_then(|identity| program.type_identity_text(identity));
        if prepared.declaration != identity.symbol
            || prepared.role != Some(role)
            || actual != identity.application_identity
        {
            return Err(
                "source provider description changed its complete prepared application".into(),
            );
        }
    }
    Ok(())
}

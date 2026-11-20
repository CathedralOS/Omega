use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{StaticValue, ValueType};
use std::borrow::Cow;

pub(crate) trait StaticValueCarrier: Sized {
    fn matches_carrier(
        &self,
        carrier: &ValueType,
        catalog: &DeclarationCatalog<'_>,
    ) -> bool;
}

impl StaticValueCarrier for StaticValue {
    fn matches_carrier(
        &self,
        carrier: &ValueType,
        catalog: &DeclarationCatalog<'_>,
    ) -> bool {
        self.children_at(carrier.carrier(), Cow::Borrowed, |declaration, arguments, case| {
            let shape = match (catalog.cases.get(declaration), case) {
                (None, None) => declaration,
                (Some(cases), Some(case)) if cases.contains(&case) => case,
                _ => return None,
            };
            // A supplied snapshot may use an application that has never been
            // checked against this declaration's enclosing and local binders.
            let arity: usize = catalog.types.scopes.get(declaration)?.iter().map(|scope| catalog.types.arity(scope)).sum();
            if arguments.len() != arity {
                return None;
            }
            Some(catalog.fields.get(shape)?.ordered.iter().map(move |field| {
                Cow::Owned(catalog.types.apply_type_arguments(&field.ty, declaration, arguments))
            }))
        })
        .is_some_and(|mut children| children.all(|(value, ty)| value.matches_carrier(&ty, catalog)))
    }
}

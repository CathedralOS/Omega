use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::{StaticValue, ValueType};

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
        match (self, carrier.carrier()) {
            (Self::Integer(value), ty) => ty.contains_integer(value),
            (Self::Bool(_), ValueType::Bool) => true,
            (Self::Array(values), ValueType::Array(element, length)) => {
                u64::try_from(values.len()) == Ok(*length)
                    && values
                        .iter()
                        .all(|value| value.matches_carrier(element, catalog))
            }
            (value, ty) => {
                let Some((declaration, arguments)) = ty.nominal_parts() else {
                    return false;
                };
                let (shape, values) = match value {
                    Self::Record { owner, fields } if owner == declaration && !catalog.cases.contains_key(owner) => (owner, fields),
                    Self::Sum { owner, case, fields }
                        if owner == declaration
                            && catalog
                                .cases
                                .get(owner)
                                .is_some_and(|cases| cases.contains(&case)) =>
                    {
                        (case, fields)
                    }
                    _ => return false,
                };
                let Some(fields) = catalog.fields.get(shape) else {
                    return false;
                };
                // A supplied snapshot can contain a nominal application whose
                // argument count has never been checked against this package.
                let Some(scopes) = catalog.types.scopes.get(declaration) else { return false };
                let arity: usize = scopes.iter().map(|scope| catalog.types.arity(scope)).sum();
                if arguments.len() != arity {
                    return false;
                }
                let fields = &fields.ordered;
                fields.len() == values.len()
                    && fields.iter().zip(values.iter()).all(|(field, value)| {
                        let ty = catalog
                            .types
                            .apply_type_arguments(&field.ty, declaration, arguments);
                        value.matches_carrier(&ty, catalog)
                    })
            }
        }
    }
}

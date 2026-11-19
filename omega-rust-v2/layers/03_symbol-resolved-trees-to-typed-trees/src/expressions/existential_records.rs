use crate::expressions::body_typer::BodyTyper;
use crate::type_system::substitution::field_components;
use crate::{Opening, Site, ValueType};
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    /// A machine supplies its signature before conversion to a stored callable.
    /// Other field expressions can supply a witness through their own type.
    pub(super) fn package_field_hint(
        &self,
        expr: usize,
        field: &ValueType,
        owner: &DeclarationId,
        witnesses: &mut [Option<ValueType>],
    ) -> Result<Option<ValueType>, crate::TypeError> {
        if let ValueType::Callable(callable) = field
            && let Some(Symbol::Declaration(id)) = self.unit.symbol(self.unit.file.ungroup(expr))
            && let Some(signature) = self.catalog.signatures.get(id)
        {
            for (formal, actual) in callable
                .parameters
                .iter()
                .zip(signature.parameters.iter().map(|(_, ty)| ty))
                .chain(std::iter::once((&callable.result, &signature.result)))
            {
                infer_witnesses(formal, actual, owner, witnesses).ok_or_else(|| crate::TypeError::at(self.unit.file, expr, "inconsistent package witness"))?;
            }
        }
        let bound = replace_witnesses(field, owner, &|i| witnesses[i].clone());
        let unresolved = bound
            .components()
            .any(|ty| matches!(ty,ValueType::Witness { owner: binder, opening: None, .. } if binder.as_ref()==owner));
        Ok((!unresolved).then_some(bound))
    }

    pub(super) fn opening(
        &self,
        node: usize,
    ) -> Opening {
        let source = self.unit.file;
        let (root, steps) = self.output.place_path(source.node_id(source.ungroup(node)));
        let mut subject = Site::at(source, root.node);
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(root.node) {
            subject.span = *span;
        }
        Opening {
            subject,
            fields: steps.into_iter().filter_map(|s| s.projection.clone()).collect(),
            version: self.version.clone(),
        }
    }
}
pub(super) fn replace_witnesses(
    ty: &ValueType,
    owner: &DeclarationId,
    resolve: &impl Fn(usize) -> Option<ValueType>,
) -> ValueType {
    ty.rewrite(0, &|part, _| match part {
        ValueType::Witness {
            owner: binder,
            index,
            opening: None,
        } if binder.as_ref() == owner => resolve(*index),
        _ => None,
    })
}
impl crate::type_system::type_catalog::TypeCatalog<'_> {
    pub(super) fn opened_field(
        &self,
        field: &ValueType,
        owner: &DeclarationId,
        arguments: &[ValueType],
        opening: impl FnOnce() -> Opening,
    ) -> ValueType {
        let field = self.apply_type_arguments(field, owner, arguments);
        if self.witness_count(owner) == 0 {
            return field;
        }
        let opening = opening();
        replace_witnesses(&field, owner, &|index| {
            Some(ValueType::Witness {
                owner: Box::new(owner.clone()),
                index,
                opening: Some(Box::new(opening.clone())),
            })
        })
    }
}
pub(super) fn infer_witnesses(
    ty: &ValueType,
    actual: &ValueType,
    owner: &DeclarationId,
    bindings: &mut [Option<ValueType>],
) -> Option<()> {
    use ValueType::*;
    for (formal, value) in field_components(ty, actual) {
        if let Witness {
            owner: binder,
            index,
            opening: None,
        } = formal
            && binder.as_ref() == owner
        {
            if matches!(value, Exact { .. } | NoResult) || bindings[*index].as_ref().is_some_and(|bound| bound != value) {
                return None;
            }
            bindings[*index] = Some(value.clone());
        }
    }
    Some(())
}

//! Local data initializers infer omitted lifetime arguments from the value and its fields.
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_system::substitution::field_components;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::site_at;

impl ResolvedSourceFile<'_> {
    pub(crate) fn initialized_annotation(
        &self,
        node: usize,
        types: &TypeCatalog<'_>,
        value: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        if self.file.nodes[node].children.is_empty()
            && let Some(Symbol::Declaration(id)) = self.name_in_role(node, true)
            && id.kind == NodeKind::Data
        {
            if let Some(ty) = self.inferred_lifetime_application(node, id, types)? {
                return Ok(if let Some(value) = value { FieldLifetimes::from_value(ty, value) } else { ty });
            }
            // An expected application of this very declaration determines its
            // omitted arguments. Each initializer still checks against its
            // substituted field type, including any data establishment goals.
            if let Some(applied @ ValueType::Applied(owner, _)) = value
                && owner.as_ref() == id
            {
                return Ok(applied.clone());
            }
        }
        self.annotation(node, types)
    }

    /// Apply `data` at `node`, which wrote no arguments, with one inferred
    /// region for each lifetime argument. A record head and a sum case both
    /// name their carrier this way. `None` means the data takes no lifetime, or
    /// also needs an argument supplied explicitly or by the expected application.
    pub(crate) fn inferred_lifetime_application(
        &self,
        node: usize,
        data: &DeclarationId,
        types: &TypeCatalog<'_>,
    ) -> Result<Option<ValueType>, TypeError> {
        let mut args = Vec::new();
        for &owner in &types.scopes[data] {
            for index in 0..types.arity(owner) {
                if !types.lifetime_slot(owner, index) {
                    return Ok(None);
                }
                args.push(ValueType::InferredLifetime {
                    site: Box::new(site_at(self.file, node)),
                    index: args.len(),
                });
            }
        }
        if args.is_empty() {
            return Ok(None);
        }
        let applied = ValueType::Applied(Box::new(data.clone()), args);
        types
            .normalize_at(applied, self, node)
            .map(Some)
            .map_err(|m| TypeError::at(self.file, node, m))
    }
}
#[derive(Default)]
pub(crate) struct FieldLifetimes(std::collections::HashMap<ValueType, Option<ValueType>>);
impl FieldLifetimes {
    pub(crate) fn from_value(
        ty: ValueType,
        value: &ValueType,
    ) -> ValueType {
        if !ty
            .components()
            .any(|t| matches!(t, ValueType::InferredLifetime { .. }))
        {
            return ty;
        }
        let mut regions = Self::default();
        regions.observe(&ty, value);
        regions.finish(ty)
    }
    pub(crate) fn observe(
        &mut self,
        field: &ValueType,
        value: &ValueType,
    ) {
        for (formal, actual) in field_components(field, value) {
            let (region, supplied) = match (formal, actual) {
                (ValueType::AtLifetime(_, l), ValueType::AtLifetime(_, r)) => (l.as_ref(), Some(r.as_ref())),
                (ValueType::AtLifetime(_, l), ValueType::Reference(..)) => (l.as_ref(), None),
                (ValueType::InferredLifetime { .. }, r) => (formal, Some(r)),
                _ => continue,
            };
            if matches!(region, ValueType::InferredLifetime { .. }) {
                self.0
                    .entry(region.clone())
                    .and_modify(|observed| {
                        if observed.as_ref() != supplied {
                            *observed = None;
                        }
                    })
                    .or_insert_with(|| supplied.cloned());
            }
        }
    }
    pub(crate) fn finish(
        &self,
        ty: ValueType,
    ) -> ValueType {
        if self.0.is_empty() {
            ty
        } else {
            ty.rewrite(0, &|t, _| self.0.get(t).cloned().flatten())
        }
    }
}

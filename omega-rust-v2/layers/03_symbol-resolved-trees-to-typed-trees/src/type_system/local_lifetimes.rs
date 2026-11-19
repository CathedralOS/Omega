//! Local data initializers infer omitted lifetime arguments from the value and its fields.
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_system::substitution::field_components;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

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
            let mut args = Vec::new();
            for &owner in &types.scopes[id] {
                for index in 0..types.arity(owner) {
                    if !types.lifetime_slot(owner, index) {
                        return self.annotation(node, types);
                    }
                    args.push(ValueType::InferredLifetime {
                        site: Box::new(Site::at(self.file, node)),
                        index: args.len(),
                    });
                }
            }
            if !args.is_empty() {
                let applied = ValueType::Applied(Box::new(id.clone()), args);
                let ty = types
                    .normalize_at(applied, self, node)
                    .map_err(|m| TypeError::at(self.file, node, m))?;
                return Ok(if let Some(value) = value { FieldLifetimes::from_value(ty, value) } else { ty });
            }
        }
        self.annotation(node, types)
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
        self,
        ty: ValueType,
    ) -> ValueType {
        if self.0.is_empty() {
            ty
        } else {
            ty.rewrite(0, &|t, _| self.0.get(t).cloned().flatten())
        }
    }
}

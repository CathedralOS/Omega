use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_resolved::Symbol;
use omega_syntax::NodeKind;

impl ResolvedSourceFile<'_> {
    pub(crate) fn open_index(
        &self,
        types: &TypeCatalog<'_>,
        n: usize,
        carrier: &ValueType,
    ) -> Result<Option<ValueType>, TypeError> {
        let f = self.file;
        if !matches!(carrier, ValueType::Integer { .. })
            || !matches!(
                f.nodes[n].kind,
                NodeKind::Binary | NodeKind::Paren | NodeKind::Call | NodeKind::Cast | NodeKind::Unary | NodeKind::FieldAccess | NodeKind::Match
            )
        {
            return Ok(None);
        }
        let Some(owner) = self.specialization_owner(n) else {
            return Ok(None);
        };
        let has_static = (n..f.nodes[n].end).any(|i| {
            matches!(
                self.name_in_role(i, false),
                Some(Symbol::Binding {
                    kind: NodeKind::ConstParam | NodeKind::KindedParam | NodeKind::TypeParam,
                    ..
                })
            )
        });
        let args = types.specialization_formals(owner);
        let may_select_evidence = args.iter().any(|arg| {
            let Some(ValueType::Evidence(slot)) = types.parameter_carrier(arg) else {
                return false;
            };
            types
                .trait_operator_tokens
                .get(&slot.declaration)
                .is_some_and(|tokens| (n..f.nodes[n].end).any(|i| f.operator(i).is_some_and(|op| tokens.contains(op.token))))
        });
        if !has_static && !may_select_evidence {
            return Ok(None);
        }
        if (n..f.nodes[n].end).any(|i| {
            matches!(
                self.name_in_role(i, false),
                Some(Symbol::Binding {
                    kind: NodeKind::Param | NodeKind::BindingPattern,
                    ..
                })
            )
        }) {
            return Err(TypeError::at(f, n, "const index needs a static argument"));
        }
        let prepared = if may_select_evidence {
            None
        } else {
            self.primitive_index(n, types, carrier)?
        };
        use super::check_static_indices::IndexEvaluation;
        let evaluation = if prepared.is_some() {
            IndexEvaluation::Primitive
        } else {
            IndexEvaluation::Specialized(args.len())
        };
        let term = if let Some(term) = prepared {
            term
        } else {
            let term = types
                .indices
                .specializations
                .template(self, n, owner, carrier)
                .apply_integer(args);
            types
                .formations
                .index(self, n, term.clone().typed(carrier.clone()));
            term
        };
        types
            .indices
            .uses
            .borrow_mut()
            .insert(crate::StaticValueRequest::at(f, n, carrier), evaluation);
        Ok(Some(term.typed(carrier.clone())))
    }
}

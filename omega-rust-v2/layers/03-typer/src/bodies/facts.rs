use super::{Body, NodeKind, Site, SourceSpan, Symbol, TypeError, TypedExpression, ValueType};
use omega_resolver::DeclarationId;
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum Subject {
    SelfValue,
    Binding(SourceSpan),
}

impl Body<'_, '_> {
    pub(super) fn fact_subject(
        &self,
        mut n: usize,
    ) -> Option<Subject> {
        while self.unit.file.nodes[n].kind == NodeKind::Paren {
            n = self.unit.single_child(n).ok()?;
        }
        match self.unit.symbol(n)? {
            Symbol::Builtin(s) if s == "self" => Some(Subject::SelfValue),
            Symbol::Binding { span, .. } => Some(Subject::Binding(*span)),
            _ => None,
        }
    }
    pub(super) fn case_guard(
        &self,
        mut n: usize,
    ) -> Option<(Subject, DeclarationId)> {
        let f = &self.unit.file;
        while f.nodes[n].kind == NodeKind::Paren {
            n = self.unit.single_child(n).ok()?;
        }
        let [subject, target] = *f.nodes[n].children.as_slice() else {
            return None;
        };
        if f.nodes[n].kind != NodeKind::Binary || !f.nodes[n].leaves.iter().any(|w| w.text == "in") {
            return None;
        }
        let Some(Symbol::Declaration(id)) = self.unit.symbol(target) else {
            return None;
        };
        if id.kind != NodeKind::Case {
            return None;
        }
        Some((self.fact_subject(subject)?, id.clone()))
    }
    pub(super) fn type_term(
        &self,
        n: usize,
    ) -> bool {
        match self.unit.symbol(n) {
            Some(Symbol::Builtin(name)) => matches!(name.as_str(), "bool" | "f32" | "f64") || ValueType::integer(name).is_some(),
            Some(Symbol::Declaration(id)) => matches!(id.kind, NodeKind::Data | NodeKind::Domain),
            Some(Symbol::Binding { kind: NodeKind::TypeParam, .. }) => true,
            _ => false,
        }
    }
    pub(super) fn type_equality(
        &mut self,
        n: usize,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let [left, right] = *f.nodes[n].children.as_slice() else {
            return Err(TypeError::unsupported(f, n));
        };
        let a = self.unit.annotation(left, &self.catalog.types)?;
        let b = self.unit.annotation(right, &self.catalog.types)?;
        self.record_operator(n, "type ==", vec![a, b], &ValueType::Bool);
        self.output.expressions.push(TypedExpression {
            site: Site::at(f, n),
            ty: ValueType::Bool,
        });
        Ok(ValueType::Bool)
    }
    pub(super) fn membership(
        &mut self,
        n: usize,
        lhs: &ValueType,
        right: usize,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let Some(Symbol::Declaration(id)) = self.unit.symbol(right) else {
            return Err(TypeError::at(f, right, "membership requires a case or domain"));
        };
        match id.kind {
            NodeKind::Case
                if lhs
                    .carrier()
                    .nominal_parts()
                    .is_some_and(|(o, _)| self.catalog.case_owner[id] == o) => {}
            NodeKind::Domain if self.catalog.types.domains.admits(id, lhs.carrier()) => {}
            _ => return Err(TypeError::at(f, right, "membership subject has the wrong carrier")),
        }
        self.output.memberships.push(crate::TypedMembership {
            site: Site::at(f, n),
            subject: lhs.clone(),
            domain: if id.kind == NodeKind::Domain {
                self.unit.domain_application(right, &self.catalog.types)?
            } else {
                crate::DomainApplication {
                    declaration: id.clone(),
                    arguments: Vec::new(),
                }
            },
        });
        Ok(ValueType::Bool)
    }
}

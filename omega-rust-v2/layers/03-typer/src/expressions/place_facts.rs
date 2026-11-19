use crate::expressions::body_typer::BodyTyper;
use crate::{Site, TypeError, TypedExpression, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum Subject {
    SelfValue,
    Binding(SourceSpan),
}

impl BodyTyper<'_, '_> {
    pub(super) fn fact_subject(
        &self,
        mut node_index: usize,
    ) -> Option<Subject> {
        while self.unit.file.nodes[node_index].kind == NodeKind::Paren {
            node_index = self.unit.single_child(node_index).ok()?;
        }
        match self.unit.symbol(node_index)? {
            Symbol::Builtin(s) if s == "self" => Some(Subject::SelfValue),
            Symbol::Binding { span, .. } => Some(Subject::Binding(*span)),
            _ => None,
        }
    }
    pub(super) fn case_guard(
        &self,
        mut node_index: usize,
    ) -> Option<(Subject, DeclarationId)> {
        let source = &self.unit.file;
        while source.nodes[node_index].kind == NodeKind::Paren {
            node_index = self.unit.single_child(node_index).ok()?;
        }
        let [subject, target] = *source.nodes[node_index].children.as_slice() else {
            return None;
        };
        if source.nodes[node_index].kind != NodeKind::Binary || !source.nodes[node_index].leaves.iter().any(|w| w.text == "in") {
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
        node_index: usize,
    ) -> bool {
        match self.unit.symbol(node_index) {
            Some(Symbol::Builtin(name)) => matches!(name.as_str(), "bool" | "f32" | "f64") || ValueType::integer(name).is_some(),
            Some(Symbol::Declaration(id)) => matches!(id.kind, NodeKind::Data | NodeKind::Domain),
            Some(Symbol::Binding { kind: NodeKind::TypeParam, .. }) => true,
            _ => false,
        }
    }
    pub(super) fn type_equality(
        &mut self,
        node_index: usize,
    ) -> Result<ValueType, TypeError> {
        let source = &self.unit.file;
        let [left, right] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        let a = self.unit.annotation(left, &self.catalog.types)?;
        let b = self.unit.annotation(right, &self.catalog.types)?;
        self.record_operator(node_index, "type ==", vec![a, b], &ValueType::Bool);
        self.output.expressions.push(TypedExpression {
            site: Site::at(source, node_index),
            ty: ValueType::Bool,
        });
        Ok(ValueType::Bool)
    }
    pub(super) fn membership(
        &mut self,
        node_index: usize,
        left_type: &ValueType,
        right: usize,
    ) -> Result<ValueType, TypeError> {
        let source = &self.unit.file;
        let Some(Symbol::Declaration(id)) = self.unit.symbol(right) else {
            return Err(TypeError::at(source, right, "membership requires a case or domain"));
        };
        match id.kind {
            NodeKind::Case
                if left_type
                    .carrier()
                    .nominal_parts()
                    .is_some_and(|(o, _)| self.catalog.case_owner[id] == o) => {}
            NodeKind::Domain if self.catalog.types.domains.admits(id, left_type.carrier()) => {}
            _ => return Err(TypeError::at(source, right, "membership subject has the wrong carrier")),
        }
        self.output.memberships.push(crate::TypedMembership {
            site: Site::at(source, node_index),
            subject: left_type.clone(),
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

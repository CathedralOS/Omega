use crate::expressions::body_typer::BodyTyper;
use crate::{Site, TypeError, ValueType};
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
        node_index: usize,
    ) -> Option<Subject> {
        let node_index = self.unit.file.ungroup(node_index);
        match self.unit.symbol(node_index)? {
            Symbol::Builtin(s) if s == "self" => Some(Subject::SelfValue),
            Symbol::Binding { span, .. } => Some(Subject::Binding(*span)),
            _ => None,
        }
    }
    pub(super) fn case_guard(
        &self,
        node_index: usize,
    ) -> Option<(Subject, DeclarationId)> {
        let source = self.unit.file;
        let node_index = source.ungroup(node_index);
        let [subject, target] = *source.nodes[node_index].children.as_slice() else {
            return None;
        };
        if source.nodes[node_index].kind != NodeKind::Binary
            || self
                .unit
                .file
                .operator(node_index)
                .is_none_or(|op| op.token != "in")
        {
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
    pub(super) fn type_equality(
        &mut self,
        node_index: usize,
        left: usize,
        right: usize,
    ) -> Result<ValueType, TypeError> {
        let a = self.unit.annotation(left, &self.catalog.types)?;
        let b = self.unit.annotation(right, &self.catalog.types)?;
        self.record_operator(node_index, "type ==", vec![a, b], &ValueType::Bool);
        self.record_expression(node_index, ValueType::Bool);
        Ok(ValueType::Bool)
    }
    pub(super) fn membership(
        &mut self,
        node_index: usize,
        left_type: &ValueType,
        right: usize,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let carrier = left_type.carrier();
        let domain = match self.unit.symbol(right) {
            Some(name) if name.names_domain() => {
                let app = self
                    .unit
                    .domain_application(right, carrier, &self.catalog.types)?;
                if !self.catalog.types.domains.admits(&app.declaration, carrier) {
                    return Err(TypeError::at(source, right, "membership subject has the wrong carrier"));
                }
                self.catalog
                    .types
                    .expanded_domain_at(&app, carrier, true, self.unit, right)
                    .map_err(|message| TypeError::at(source, right, message))?;
                app
            }
            Some(Symbol::Declaration(id)) if id.kind == NodeKind::Case => {
                if !carrier
                    .nominal_parts()
                    .is_some_and(|(owner, _)| self.catalog.case_owner[id] == owner)
                {
                    return Err(TypeError::at(source, right, "membership subject has the wrong carrier"));
                }
                crate::DomainApplication {
                    declaration: id.clone(),
                    arguments: Vec::new(),
                }
            }
            _ => return Err(TypeError::at(source, right, "membership requires a case or domain")),
        };
        self.output.memberships.push(crate::TypedMembership {
            site: Site::at(source, node_index),
            subject: left_type.clone(),
            domain,
        });
        Ok(ValueType::Bool)
    }
}

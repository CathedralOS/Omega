use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;

impl BodyTyper<'_, '_> {
    pub(super) fn check_fact(
        &mut self,
        expr: usize,
    ) -> Result<(), TypeError> {
        let value = self.type_expression_with_expected_type(expr, Some(&ValueType::Bool))?;
        if matches!(value, ValueType::Universe { strict: true, .. }) {
            self.record_obligation(expr, ObligationKind::MathematicalFact { sort: value });
            Ok(())
        } else {
            self.check_destination(expr, &value, &ValueType::Bool)
        }
    }
    pub(super) fn check_hypothesis(
        &mut self,
        node_index: usize,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
        let requirement = &self.catalog.hypotheses[&Site::at(source, node_index)];
        let parent_result = self.subject.replace(requirement.result.clone());
        self.locals.extend(requirement.parameters.iter().cloned());
        for &part in &source.nodes[node_index].children {
            match source.nodes[part].kind {
                NodeKind::Requires | NodeKind::Ensures => {
                    let expr = self.unit.single_child(part)?;
                    self.check_fact(expr)?;
                }
                NodeKind::Hypothesis => self.check_hypothesis(part)?,
                _ => {}
            }
        }
        for (span, _) in &requirement.parameters {
            self.locals.remove(span);
        }
        self.subject = parent_result;
        Ok(())
    }
}

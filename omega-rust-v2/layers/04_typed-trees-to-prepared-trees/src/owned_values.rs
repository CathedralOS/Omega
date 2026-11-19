use crate::constant_evaluation::ConstantEvaluation;
use crate::integer_arithmetic::IntegerRange;
use crate::invocation::{EvaluationError, UNSUPPORTED_BODY};
use omega_resolver::DeclarationId;
use omega_typer::{Multiplicity, NodeSemantics, ObligationKind, TypedConstruction, TypedTree, ValueType};
use std::collections::HashSet;

pub use omega_typer::StaticValue as PreparedValue;

impl ConstantEvaluation<'_, '_> {
    pub(super) fn construction<'a>(
        &'a self,
        tree: &'a TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        owner: &DeclarationId,
    ) -> Result<(&'a TypedConstruction, &'a [ValueType]), EvaluationError> {
        let construction = tree
            .construction(node)
            .ok_or("the selected nominal value has no supported initializer")?;
        if &construction.owner != owner || &construction.carrier != carrier {
            return Err("the selected constructor has a different carrier".into());
        }
        let shape = self
            .trees
            .static_data_shapes
            .get(carrier)
            .ok_or(UNSUPPORTED_BODY)?;
        let fields = match &construction.case {
            Some(case) => shape
                .cases
                .iter()
                .find(|(declared, _)| declared == case)
                .map(|(_, payload)| payload),
            None if shape.cases.is_empty() => Some(&shape.fields),
            None => None,
        }
        .ok_or("the selected case does not belong to the carrier")?;
        if tree
            .semantics_at(node)
            .any(|record| !matches!(record, NodeSemantics::Expression(_)))
        {
            return Err("the selected constructor requires proof outside the prepared value fragment".into());
        }
        if fields.len() != construction.initializers.len() {
            return Err("the selected constructor must explicitly initialize each field".into());
        }
        Ok((construction, fields))
    }

    pub(super) fn check_owned_type(
        &self,
        carrier: &ValueType,
        array: bool,
    ) -> Result<(), EvaluationError> {
        enum Step<'a> {
            Check(&'a ValueType),
            Complete(&'a ValueType),
        }
        let mut steps = vec![Step::Check(carrier)];
        let mut active = HashSet::new();
        let mut checked = HashSet::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Complete(carrier) => {
                    active.remove(carrier);
                    checked.insert(carrier);
                }
                Step::Check(ValueType::Array(element, _)) => steps.push(Step::Check(element)),
                Step::Check(carrier @ (ValueType::Nominal(owner) | ValueType::Applied(owner, _))) => {
                    if checked.contains(carrier) {
                        continue;
                    }
                    if !active.insert(carrier) {
                        return Err("a recursive by-value type has no supported frozen value".into());
                    }
                    if !self
                        .trees
                        .data_properties
                        .iter()
                        .any(|(id, properties)| id == owner.as_ref() && properties.multiplicity == Multiplicity::Copy)
                    {
                        return Err(if array {
                            "the complete array element type must permit copying"
                        } else {
                            "the complete constant type must permit copying"
                        }
                        .into());
                    }
                    if self
                        .trees
                        .cleanups
                        .iter()
                        .chain(
                            self.trees
                                .library
                                .into_iter()
                                .flat_map(|library| &library.cleanups),
                        )
                        .any(|selection| &selection.owner == owner.as_ref())
                    {
                        return Err("the complete constant type has a cleanup obligation".into());
                    }
                    let shape = self
                        .trees
                        .static_data_shapes
                        .get(carrier)
                        .ok_or("the complete constant type has no finite typed field shape")?;
                    steps.push(Step::Complete(carrier));
                    steps.extend(
                        shape
                            .cases
                            .iter()
                            .rev()
                            .flat_map(|(_, fields)| fields.iter().rev())
                            .map(Step::Check),
                    );
                    steps.extend(shape.fields.iter().rev().map(Step::Check));
                }
                Step::Check(value) if *value == ValueType::Bool || IntegerRange::for_type(value).is_some() => {}
                Step::Check(_) => {
                    let reason = if array {
                        "the complete array element type is outside the prepared value fragment"
                    } else {
                        "the complete constant type is outside the prepared value fragment"
                    };
                    return Err(reason.into());
                }
            }
        }
        Ok(())
    }

    pub(super) fn constant_reference(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
    ) -> Result<Option<&PreparedValue>, EvaluationError> {
        let mut selected = None;
        for record in tree.semantics_at(node) {
            if let NodeSemantics::Constant(id) = record
                && selected.replace(id).is_some()
            {
                return Err("ambiguous constant reference".into());
            }
        }
        let Some(id) = selected else { return Ok(None) };
        if tree.semantics_at(node).any(|record| match record {
            NodeSemantics::Constant(_) | NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => false,
            NodeSemantics::Obligation(o) if matches!(o.kind, ObligationKind::TotalCrashGuard) => false,
            _ => true,
        }) {
            return Err(UNSUPPORTED_BODY.into());
        }
        let definition = self
            .definitions
            .get(id)
            .ok_or("the selected constant has no local typed initializer")?;
        if &definition.initializer.carrier != carrier {
            return Err("the selected constant has a different carrier".into());
        }
        self.values
            .get(id)
            .map(Some)
            .ok_or_else(|| EvaluationError::NeedsConstant(id.clone()))
    }
}

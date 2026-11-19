use crate::constant_evaluation::ConstantEvaluation;
use crate::integer_arithmetic::IntegerRange;
use crate::integer_invocation::{EvaluationError, Scope, UNSUPPORTED_BODY};
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_typer::{NodeSemantics, ObligationKind, StaticIntegerRequest, TypedTree, ValueType};

/// A closed semantic value. Array elements are in index order; layout and
/// target-specific bytes are decisions for materialization.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PreparedValue {
    Integer(BigInt),
    Bool(bool),
    Array(std::sync::Arc<[PreparedValue]>),
}

impl ConstantEvaluation<'_, '_> {
    pub(super) fn owned_value(
        &self,
        request: &StaticIntegerRequest,
    ) -> Result<PreparedValue, EvaluationError> {
        // Visit literal elements in source order. Sized arrays can be nested
        // without consuming a Rust call frame for each dimension.
        enum Step<'a> {
            Visit(usize, &'a ValueType),
            Array(usize),
        }
        if let ValueType::Array(element, _) = &request.carrier {
            let mut leaf = element.as_ref();
            while let ValueType::Array(child, _) = leaf {
                leaf = child;
            }
            if leaf != &ValueType::Bool && IntegerRange::for_type(leaf).is_none() {
                return Err("the complete array element type is outside the prepared value fragment".into());
            }
        }
        let tree = self
            .trees
            .trees()
            .nth(request.node.file)
            .ok_or(UNSUPPORTED_BODY)?;
        let file = tree.resolved().file;
        let scope = Scope::Initializer {
            root: request.node.node,
            owner_arguments: None,
        };
        let mut steps = vec![Step::Visit(request.node.node, &request.carrier)];
        let mut values = Vec::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Visit(node, carrier) => {
                    let node = file.ungroup(node);
                    let value = match carrier {
                        ValueType::Bool => PreparedValue::Bool(self.boolean_value(&tree, node, &scope)?),
                        ValueType::Array(element, length) => {
                            if let Some(value) = self.constant_reference(&tree, node, carrier)? {
                                value.clone()
                            } else {
                                let literal = &file.nodes[node];
                                if literal.kind != NodeKind::ArrayLiteral || literal.children.len() as u64 != *length {
                                    return Err("the selected array value has no supported initializer".into());
                                }
                                steps.push(Step::Array(literal.children.len()));
                                steps.extend(
                                    literal
                                        .children
                                        .iter()
                                        .rev()
                                        .map(|child| Step::Visit(*child, element)),
                                );
                                continue;
                            }
                        }
                        ValueType::Integer { .. } => PreparedValue::Integer(self.expression(&tree, node, carrier, &scope)?),
                        _ => return Err("the initializer type has no supported frozen value".into()),
                    };
                    values.push(value);
                }
                Step::Array(count) => {
                    let elements = values.split_off(values.len() - count);
                    values.push(PreparedValue::Array(elements.into()));
                }
            }
        }
        Ok(values.pop().expect("visited initializer"))
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

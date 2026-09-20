//! Decompose declared type constructors without observing runtime contents.
//!
//! Element positions bind types and length positions bind integer constants.
//! Arrays and nominal applications share recursion; `applications` selects the
//! exact declaration and parameter kinds of each nominal head. Closed leaves
//! use the existing identity owner. This is not an arithmetic solver or a new
//! source of array-length evaluation.

use super::{Binding, Solver, collect_expression_binder_mentions};
use crate::preparation::generic_data::closed_argument_identity;
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::TypeParameterKind;
use syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod applications;

pub(super) fn collect_binder_mentions(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    parameters: &[String],
    mentions: &mut Vec<usize>,
) {
    let mut name_mention = |name: &str| {
        if let Some(position) = parameters.iter().position(|parameter| parameter == name) {
            mentions.push(position);
        }
    };
    match syntax.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => name_mention(name.as_str()),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            if let FixedArrayLength::ConstParameter(name) = length {
                name_mention(name.as_str());
            }
            collect_binder_mentions(syntax, *element_type, parameters, mentions);
        }
        TypeReferenceNode::Reference { referee, .. } => {
            collect_binder_mentions(syntax, *referee, parameters, mentions);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_binder_mentions(syntax, *element_type, parameters, mentions);
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            name_mention(base_name.as_str());
            for argument in syntax.type_references.type_reference_handles(*arguments) {
                collect_binder_mentions(syntax, *argument, parameters, mentions);
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_binder_mentions(syntax, *base_type, parameters, mentions);
            for constraint in syntax.type_references.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        collect_expression_binder_mentions(syntax, *minimum, parameters, mentions);
                        collect_expression_binder_mentions(syntax, *maximum, parameters, mentions);
                    }
                    TypeConstraintNode::Domain(domain) => {
                        for argument in syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                        {
                            collect_binder_mentions(syntax, *argument, parameters, mentions);
                        }
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            collect_expression_binder_mentions(syntax, *expression, parameters, mentions);
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => {}
    }
}

impl Solver<'_, '_> {
    fn parameter_position(&self, name: &str) -> Option<(usize, bool)> {
        self.syntax
            .items
            .type_parameters(self.base_info.parameters)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.name.as_str() == name)
            .map(|(position, parameter)| {
                (position, matches!(parameter.kind, TypeParameterKind::Type))
            })
    }

    fn type_structure_error(&self, reason: &str, span: SourceSpan) -> Diagnostic {
        self.reject(format!("where type equation {reason}"))
            .with_source_span(span)
    }

    pub(super) fn match_type_structure(
        &mut self,
        pattern: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        match self.syntax.type_references.type_reference(pattern).clone() {
            TypeReferenceNode::Named(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str()) {
                    if !is_type {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an element position",
                            span,
                        ));
                    }
                    let expected = match self.bindings[position].clone() {
                        None => {
                            if closed_argument_identity(self.syntax, self.selection, actual, false)
                                .is_none()
                            {
                                return Err(self
                                    .type_structure_error("requires a closed element type", span));
                            }
                            self.bindings[position] = Some(Binding::Type(actual));
                            return Ok(());
                        }
                        Some(Binding::Type(reference)) => reference,
                        Some(Binding::NamedType(name)) => self
                            .syntax
                            .type_references
                            .insert(TypeReferenceNode::Named(name)),
                        Some(Binding::Integer(_) | Binding::Opaque) => {
                            return Err(self.type_structure_error(
                                "mixes type and value kinds in an element position",
                                span,
                            ));
                        }
                    };
                    return self.require_equal_type_structure(expected, actual, span);
                }
                self.require_equal_type_structure(pattern, actual, span)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let TypeReferenceNode::FixedArray {
                    element_type: actual_element,
                    length: FixedArrayLength::Literal(actual_length),
                } = self.syntax.type_references.type_reference(actual).clone()
                else {
                    return Err(self.type_structure_error(
                        "requires the same fixed-array constructor with a normalized length",
                        span,
                    ));
                };
                self.match_type_structure(element_type, actual_element, span)?;
                if let FixedArrayLength::ConstParameter(name) = &length
                    && let Some((position, is_type)) = self.parameter_position(name.as_str())
                {
                    if is_type || self.base_info.const_parameter_types[position].is_none() {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an array length",
                            span,
                        ));
                    }
                    return self.bind_integer(
                        position,
                        BigInt::from_u64(actual_length as u64),
                        span,
                    );
                }
                let expected = self.closed_array_length(&length, span)?.ok_or_else(|| {
                    self.type_structure_error("requires an explicit array length", span)
                })?;
                if actual_length != expected {
                    return Err(
                        self.type_structure_error("has conflicting fixed-array lengths", span)
                    );
                }
                Ok(())
            }
            TypeReferenceNode::Generic { .. } => self.match_application(pattern, actual, span),
            _ => {
                self.require_closed_pattern(pattern, span)?;
                self.require_equal_type_structure(pattern, actual, span)
            }
        }
    }

    /// A global declaration with a matching spelling cannot close a reference
    /// to this template's binder. Only the constructors decomposed above may
    /// recover binders; unsupported open leaves must not fall through to the
    /// canonical identity owner as if they were already closed arguments.
    fn require_closed_pattern(
        &self,
        pattern: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let mut mentions = Vec::new();
        collect_binder_mentions(
            self.syntax,
            pattern,
            self.base_info.parameter_names,
            &mut mentions,
        );
        if !mentions.is_empty() {
            return Err(self.type_structure_error(
                "cannot decide an open or unsupported element type; supply explicit arguments",
                span,
            ));
        }
        Ok(())
    }

    fn require_equal_type_structure(
        &self,
        expected: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        match (
            closed_argument_identity(self.syntax, self.selection, expected, false),
            closed_argument_identity(self.syntax, self.selection, actual, false),
        ) {
            (Some(expected), Some(actual)) if expected == actual => Ok(()),
            (Some(_), Some(_)) => {
                Err(self.type_structure_error("has conflicting element types", span))
            }
            _ => Err(self.type_structure_error(
                "cannot decide an open or unsupported element type; supply explicit arguments",
                span,
            )),
        }
    }

    pub(super) fn construct_type_structure(
        &mut self,
        pattern: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
        match self.syntax.type_references.type_reference(pattern).clone() {
            TypeReferenceNode::Named(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str()) {
                    if !is_type {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an element position",
                            span,
                        ));
                    }
                    return match self.bindings[position].clone() {
                        None => Ok(None),
                        Some(Binding::Type(reference)) => Ok(Some(reference)),
                        Some(Binding::NamedType(name)) => Ok(Some(
                            self.syntax
                                .type_references
                                .insert(TypeReferenceNode::Named(name)),
                        )),
                        Some(Binding::Integer(_) | Binding::Opaque) => Err(self
                            .type_structure_error(
                                "mixes type and value kinds in an element position",
                                span,
                            )),
                    };
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let Some(element_type) = self.construct_type_structure(element_type, span)? else {
                    return Ok(None);
                };
                let Some(length) = self.closed_array_length(&length, span)? else {
                    return Ok(None);
                };
                return Ok(Some(self.syntax.type_references.insert(
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    },
                )));
            }
            TypeReferenceNode::Generic { .. } => return self.construct_application(pattern, span),
            _ => {}
        }
        self.require_closed_pattern(pattern, span)?;
        if closed_argument_identity(self.syntax, self.selection, pattern, false).is_none() {
            return Err(self.type_structure_error(
                "cannot construct an open or unsupported element type; supply explicit arguments",
                span,
            ));
        }
        Ok(Some(pattern))
    }

    fn closed_array_length(
        &self,
        length: &FixedArrayLength,
        span: SourceSpan,
    ) -> Result<Option<usize>, Diagnostic> {
        let integer = match length {
            FixedArrayLength::Literal(length) => return Ok(Some(*length)),
            FixedArrayLength::ConstCall(_) => {
                return Err(self.type_structure_error(
                    "requires an array length normalized by constant evaluation",
                    span,
                ));
            }
            FixedArrayLength::ConstParameter(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str()) {
                    if is_type || self.base_info.const_parameter_types[position].is_none() {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an array length",
                            span,
                        ));
                    }
                    match &self.bindings[position] {
                        None => return Ok(None),
                        Some(Binding::Integer(value)) => value.clone(),
                        Some(_) => {
                            return Err(self.type_structure_error(
                                "requires an integer const array length",
                                span,
                            ));
                        }
                    }
                } else {
                    crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                        self.syntax, name.as_str(), name.source_span(),
                    ).map_err(|reason| self.type_structure_error(&reason, span))?;
                    let value = self.const_values.get(name.as_str()).ok_or_else(|| {
                        self.type_structure_error("requires a closed array length", span)
                    })?;
                    BigInt::from_i128(*value)
                }
            }
        };
        integer
            .to_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| {
                self.type_structure_error("has an array length outside the supported extent", span)
            })
    }
}

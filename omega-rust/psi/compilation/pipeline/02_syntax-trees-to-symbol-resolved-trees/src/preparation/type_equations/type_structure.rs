//! Decompose declared type constructors without observing runtime contents.
//!
//! Element positions bind types and length positions bind integer constants.
//! Nominal const positions additionally preserve Boolean identity and kind.
//! Arrays, slices, references and nominal applications share recursion;
//! references compare exact access, not borrow compatibility. A selected
//! callable may introduce fresh invocation regions; its declared telescope
//! gives those tags binder role. Captured actual regions still require lexical
//! custody, never matching names or region-erased type identity.
//! `applications` selects the
//! exact declaration and parameter kinds of each nominal head. Closed leaves
//! use the existing identity owner. This is not an arithmetic solver or a new
//! source of array-length evaluation.
//! Actual array geometry uses the same closed-extent reader as instance
//! identity and diagnostic names. Actual names belong to their authored scope,
//! not the template's binder roster. Keep those authored nodes intact: later
//! resolution and typing retain their selection and constant-origin custody.

use super::{Binding, Solver, collect_expression_binder_mentions};
use crate::preparation::generic_data::{
    canonicalize_selected_declared_const_definition, closed_argument_identity,
    closed_fixed_array_length,
};
use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use numerics::bignum::BigInt;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
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
        TypeReferenceNode::Named(name) => name_mention(name.as_str(syntax)),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            match length {
                FixedArrayLength::ConstParameter(name) | FixedArrayLength::ConstCall(name) => {
                    name_mention(name.as_str(syntax));
                }
                FixedArrayLength::Expression(reference) => {
                    collect_binder_mentions(syntax, *reference, parameters, mentions);
                }
                FixedArrayLength::Literal(_) => {}
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
            name_mention(base_name.as_str(syntax));
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
                        if let Some(position) = parameters
                            .iter()
                            .position(|name| name == domain.name.as_str(syntax))
                        {
                            mentions.push(position);
                        }
                        for argument in syntax
                            .type_references
                            .type_reference_handles(domain.arguments)
                        {
                            collect_binder_mentions(syntax, *argument, parameters, mentions);
                        }
                    }
                    TypeConstraintNode::Named(name) => {
                        if let Some(position) = parameters
                            .iter()
                            .position(|parameter| parameter == name.as_str(syntax))
                        {
                            mentions.push(position);
                        }
                    }
                    TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            collect_expression_binder_mentions(syntax, *expression, parameters, mentions);
        }
        TypeReferenceNode::DynamicTrait {
            trait_name,
            trait_arguments,
        } => {
            name_mention(trait_name.as_str(syntax));
            for argument in syntax
                .type_references
                .type_reference_handles(*trait_arguments)
            {
                collect_binder_mentions(syntax, *argument, parameters, mentions);
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

impl Solver<'_, '_> {
    // Only the selected callable's own telescope introduces invocation variables.
    // Data applications supply their regions explicitly; captured tags and named
    // actuals remain subject to lexical identity rather than spelling equality.
    fn require_invocation_lifetime(
        &self,
        lifetime: &syntax_trees::identifier::Identifier,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        if self
            .base_info
            .invocation_lifetimes
            .iter()
            .any(|parameter| parameter.as_str(self.syntax) == lifetime.as_str(self.syntax))
        {
            Ok(())
        } else {
            Err(self.type_structure_error(
                "reference region is not a declared invocation lifetime",
                span,
            ))
        }
    }

    /// Materialize only the type grammar retained in an ambiguous expression
    /// operand. Classification already established the opposite type binder;
    /// every resulting node rejoins the same constructor matcher below.
    pub(super) fn type_operand_reference(
        &mut self,
        expression: syntax_trees::expression::ExpressionHandle,
        span: SourceSpan,
    ) -> Result<TypeReferenceHandle, Diagnostic> {
        use syntax_trees::expression::ExpressionNode;
        use syntax_trees::identifier::Identifier;
        match self.syntax.expressions.expression(expression).clone() {
            ExpressionNode::TypeExpression(reference) => Ok(reference),
            ExpressionNode::Name(path) => {
                let parts = self.syntax.expressions.identifier_path_members(path);
                let Some(first) = parts.first() else {
                    return Err(self.type_structure_error("lost its element type name", span));
                };
                let mut name_span = first.source_span();
                if let Some(last) = parts.last() {
                    name_span.span.end = last.source_span().span.end;
                }
                let name = Identifier::new(parts.joined_path_in(self.syntax), name_span);
                Ok(self
                    .syntax
                    .type_references
                    .insert(TypeReferenceNode::Named(name)))
            }
            ExpressionNode::Borrow(borrow) => {
                let referee = self.type_operand_reference(borrow.target, span)?;
                Ok(self
                    .syntax
                    .type_references
                    .insert_reference(referee, borrow.access))
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let [element] = self.syntax.expressions.expression_handles(elements) else {
                    return Err(
                        self.type_structure_error("requires exactly one slice element type", span)
                    );
                };
                let element_type = self.type_operand_reference(*element, span)?;
                Ok(self
                    .syntax
                    .type_references
                    .insert(TypeReferenceNode::Slice { element_type }))
            }
            _ => Err(self.type_structure_error(
                "cannot use a value as a reference or slice element type",
                span,
            )),
        }
    }

    fn parameter_position(&self, name: &str) -> Option<(usize, bool)> {
        self.syntax
            .items
            .type_parameters(self.base_info.parameters)
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.name.as_str(self.syntax) == name)
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
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                if let Some(lifetime) = lifetime {
                    self.require_invocation_lifetime(&lifetime, span)?;
                }
                let TypeReferenceNode::Reference {
                    referee: actual_referee,
                    access: actual_access,
                    lifetime: None,
                } = self.syntax.type_references.type_reference(actual).clone()
                else {
                    return Err(self.type_structure_error(
                        "requires the same reference constructor with an elided actual region",
                        span,
                    ));
                };
                if access != actual_access {
                    return Err(self.type_structure_error("has conflicting reference access", span));
                }
                self.match_type_structure(referee, actual_referee, span)
            }
            TypeReferenceNode::Slice { element_type } => {
                let TypeReferenceNode::Slice {
                    element_type: actual_element,
                } = self.syntax.type_references.type_reference(actual).clone()
                else {
                    return Err(
                        self.type_structure_error("requires the same slice constructor", span)
                    );
                };
                self.match_type_structure(element_type, actual_element, span)
            }
            TypeReferenceNode::Named(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
                {
                    if !is_type {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an element position",
                            span,
                        ));
                    }
                    let expected = match self.bindings[position].clone() {
                        None => {
                            self.require_lifetime_free_argument(actual, span)?;
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
                        Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => {
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
                    length: actual_length,
                } = self.syntax.type_references.type_reference(actual).clone()
                else {
                    return Err(self
                        .type_structure_error("requires the same fixed-array constructor", span));
                };
                // An actual extent never refers to this template's const
                // binder by spelling. Read closed geometry without a template
                // substitution, before binding either its element or length.
                let actual_length = closed_fixed_array_length(
                    self.syntax,
                    &actual_length,
                    &crate::preparation::declaration_selection::GenericApplicationSubstitution::new(
                    ),
                    self.selection,
                )
                .map_err(|reason| self.type_structure_error(&reason, span))?;
                self.match_type_structure(element_type, actual_element, span)?;
                if let FixedArrayLength::ConstParameter(name) = &length
                    && let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
            TypeReferenceNode::Constrained { constraints, .. }
                if matches!(
                    self.syntax.type_references.constraints(constraints),
                    [TypeConstraintNode::Domain(_)]
                ) =>
            {
                self.match_application(pattern, actual, span)
            }
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

    pub(super) fn require_equal_type_structure(
        &self,
        expected: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        self.require_lifetime_free_argument(expected, span)?;
        self.require_lifetime_free_argument(actual, span)?;
        match (
            closed_argument_identity(self.syntax, self.selection, expected, false),
            closed_argument_identity(self.syntax, self.selection, actual, false),
        ) {
            (Some(expected), Some(actual)) if expected == actual => Ok(()),
            (Some(_), Some(_)) => {
                Err(self.type_structure_error("has conflicting element types", span))
            }
            _ if self.application_shape(expected) || self.application_shape(actual) => {
                self.require_equal_application_structure(expected, actual, span)
            }
            _ => Err(self.type_structure_error(
                "cannot decide an open or unsupported element type; supply explicit arguments",
                span,
            )),
        }
    }

    /// The shapes the constructor matcher can decompose without a closed
    /// identity: a generic application, or a carrier carrying one declared
    /// domain constraint.
    fn application_shape(&self, reference: TypeReferenceHandle) -> bool {
        match self.syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Generic { .. } => true,
            TypeReferenceNode::Constrained { constraints, .. } => matches!(
                self.syntax.type_references.constraints(*constraints),
                [TypeConstraintNode::Domain(_)]
            ),
            _ => false,
        }
    }

    pub(super) fn construct_type_structure(
        &mut self,
        pattern: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
        match self.syntax.type_references.type_reference(pattern).clone() {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                if let Some(lifetime) = &lifetime {
                    self.require_invocation_lifetime(lifetime, span)?;
                }
                let Some(referee) = self.construct_type_structure(referee, span)? else {
                    return Ok(None);
                };
                return Ok(Some(self.syntax.type_references.insert(
                    TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    },
                )));
            }
            TypeReferenceNode::Slice { element_type } => {
                let Some(element_type) = self.construct_type_structure(element_type, span)? else {
                    return Ok(None);
                };
                return Ok(Some(
                    self.syntax
                        .type_references
                        .insert(TypeReferenceNode::Slice { element_type }),
                ));
            }
            TypeReferenceNode::Named(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
                {
                    if !is_type {
                        return Err(self.type_structure_error(
                            "mixes type and value kinds in an element position",
                            span,
                        ));
                    }
                    return match self.bindings[position].clone() {
                        None => Ok(None),
                        Some(Binding::Type(reference)) => {
                            self.require_lifetime_free_argument(reference, span)?;
                            Ok(Some(reference))
                        }
                        Some(Binding::NamedType(name)) => Ok(Some(
                            self.syntax
                                .type_references
                                .insert(TypeReferenceNode::Named(name)),
                        )),
                        Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => {
                            Err(self.type_structure_error(
                                "mixes type and value kinds in an element position",
                                span,
                            ))
                        }
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
            TypeReferenceNode::Constrained { constraints, .. }
                if matches!(
                    self.syntax.type_references.constraints(constraints),
                    [TypeConstraintNode::Domain(_)]
                ) =>
            {
                return self.construct_application(pattern, span);
            }
            _ => {}
        }
        self.require_closed_pattern(pattern, span)?;
        self.require_lifetime_free_argument(pattern, span)?;
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
            FixedArrayLength::ConstCall(_) | FixedArrayLength::Expression(_) => {
                return Err(self.type_structure_error(
                    "requires an array length normalized by constant evaluation",
                    span,
                ));
            }
            FixedArrayLength::ConstParameter(name) => {
                if let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
                {
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
                } else if let Some(selection) = self.selection {
                    // The equation belongs to the template's source scope,
                    // not its caller. Reuse exact declaration selection and
                    // canonical constant validation before observing a length;
                    // failed lookup must never fall back to the root-name map.
                    let definition = selection.select(self.syntax, name)?.ok_or_else(|| {
                        self.type_structure_error(
                            "requires an unambiguous selected array length constant",
                            name.source_span(),
                        )
                    })?;
                    let value = canonicalize_selected_declared_const_definition(
                        self.syntax,
                        &definition,
                        Some(selection),
                    )
                    .map_err(|reason| self.type_structure_error(&reason, name.source_span()))?;
                    let Some(DecodedCanonicalConstValue::Integer { value, .. }) =
                        value.decode_encoding()
                    else {
                        return Err(self.type_structure_error(
                            "requires an integer array length constant",
                            name.source_span(),
                        ));
                    };
                    BigInt::from_i128(value)
                } else {
                    crate::preparation::generic_data::module_constants::reject_module_constant_selection(
                        self.syntax, name.as_str(self.syntax), name.source_span(),
                    ).map_err(|reason| self.type_structure_error(&reason, span))?;
                    let value =
                        self.const_values
                            .get(name.as_str(self.syntax))
                            .ok_or_else(|| {
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

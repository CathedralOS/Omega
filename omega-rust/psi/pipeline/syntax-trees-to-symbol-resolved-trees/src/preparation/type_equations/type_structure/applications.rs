//! Nominal constructors decompose by declaration identity and parameter kind.
//! Original application trees survive synthesis; generated display names never
//! recover their arguments. This is structural matching, not const evaluation.

use super::super::{Binding, Solver, const_binder_envelope, normalized_boolean_argument};
use crate::preparation::generic_data::{ClosedArgumentIdentity, closed_name_identity};
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use numerics::bignum::BigInt;
use source::SourceSpan;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, ItemHandle, TypeParameter, TypeParameterKind};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct Application {
    name: Identifier,
    declaration: ItemHandle,
    arguments: Vec<TypeReferenceHandle>,
    parameters: Vec<TypeParameter>,
}

impl Solver<'_, '_> {
    fn application(
        &self,
        reference: TypeReferenceHandle,
        pattern: bool,
        span: SourceSpan,
    ) -> Result<Application, Diagnostic> {
        let origin = self
            .syntax
            .type_references
            .generic_application_origin(reference);
        let reference = if origin.is_valid() { origin } else { reference };
        let TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } = self.syntax.type_references.type_reference(reference)
        else {
            return Err(self.type_structure_error("requires an exact declared application", span));
        };
        if pattern && self.parameter_position(base_name.as_str()).is_some() {
            return Err(
                self.type_structure_error("cannot use a binder as a nominal constructor", span)
            );
        }
        let Some(ClosedArgumentIdentity::Nominal(declaration)) =
            closed_name_identity(self.syntax, self.selection, base_name)
        else {
            return Err(
                self.type_structure_error("requires a selected application declaration", span)
            );
        };
        let Item::Data(definition) = self.syntax.root_item(declaration) else {
            return Err(self.type_structure_error("requires a data constructor", span));
        };
        if !lifetime_arguments.is_empty()
            || !definition.lifetime_parameters.is_empty()
            || definition.generic_instance.is_some()
        {
            return Err(self.type_structure_error(
                "requires original lifetime-free application structure",
                span,
            ));
        }
        let parameters = self
            .syntax
            .items
            .type_parameters(definition.type_parameters);
        let arguments = self
            .syntax
            .type_references
            .type_reference_handles(*arguments);
        if parameters.len() != arguments.len() {
            return Err(
                self.type_structure_error("requires a complete constructor argument tuple", span)
            );
        }
        if parameters.iter().any(|parameter| {
            !matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Const { .. }
            )
        }) {
            return Err(self.type_structure_error(
                "requires type and scalar const constructor parameters",
                span,
            ));
        }
        Ok(Application {
            name: base_name.clone(),
            declaration,
            arguments: arguments.to_vec(),
            parameters: parameters.to_vec(),
        })
    }

    pub(super) fn match_application(
        &mut self,
        pattern: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let pattern = self.application(pattern, true, span)?;
        let actual = self.application(actual, false, span)?;
        if pattern.declaration != actual.declaration {
            return Err(self.type_structure_error("has conflicting nominal constructors", span));
        }
        for ((pattern, actual), parameter) in pattern
            .arguments
            .into_iter()
            .zip(actual.arguments)
            .zip(pattern.parameters)
        {
            match parameter.kind {
                TypeParameterKind::Type => {
                    self.require_lifetime_free_argument(actual, span)?;
                    self.match_type_structure(pattern, actual, span)?;
                }
                TypeParameterKind::Const { type_reference } => {
                    if self.boolean_constructor_parameter(type_reference) {
                        self.match_application_boolean(pattern, actual, span)?;
                        continue;
                    }
                    let value =
                        self.application_integer(actual, false, span)?
                            .ok_or_else(|| {
                                self.type_structure_error(
                                    "requires a normalized integer constructor argument",
                                    span,
                                )
                            })?;
                    self.validate_application_integer(type_reference, &value, span)?;
                    if let TypeReferenceNode::Named(name) =
                        self.syntax.type_references.type_reference(pattern)
                        && let Some((position, is_type)) = self.parameter_position(name.as_str())
                    {
                        if is_type || self.base_info.const_parameter_types[position].is_none() {
                            return Err(self.type_structure_error(
                                "mixes type and value kinds in a constructor argument",
                                span,
                            ));
                        }
                        self.bind_integer(position, value, span)?;
                    } else if self.application_integer(pattern, true, span)? != Some(value) {
                        return Err(self
                            .type_structure_error("has conflicting constructor constants", span));
                    }
                }
                _ => {
                    return Err(
                        self.type_structure_error("has an unsupported constructor parameter", span)
                    );
                }
            }
        }
        Ok(())
    }

    pub(super) fn construct_application(
        &mut self,
        pattern: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<Option<TypeReferenceHandle>, Diagnostic> {
        let application = self.application(pattern, true, span)?;
        let mut arguments = Vec::new();
        for (argument, parameter) in application
            .arguments
            .into_iter()
            .zip(application.parameters)
        {
            let reference = match parameter.kind {
                TypeParameterKind::Type => {
                    let Some(reference) = self.construct_type_structure(argument, span)? else {
                        return Ok(None);
                    };
                    self.require_lifetime_free_argument(reference, span)?;
                    reference
                }
                TypeParameterKind::Const { type_reference } => {
                    if self.boolean_constructor_parameter(type_reference) {
                        let Some(value) = self.application_boolean(argument, true, span)? else {
                            return Ok(None);
                        };
                        arguments.push(self.syntax.type_references.insert(
                            TypeReferenceNode::Named(Identifier::generated(
                                CanonicalConstValue::boolean(value).atom(),
                            )),
                        ));
                        continue;
                    }
                    let Some(value) = self.application_integer(argument, true, span)? else {
                        return Ok(None);
                    };
                    self.validate_application_integer(type_reference, &value, span)?;
                    self.syntax.type_references.insert(TypeReferenceNode::Named(
                        Identifier::generated(value.to_string()),
                    ))
                }
                _ => {
                    return Err(
                        self.type_structure_error("has an unsupported constructor parameter", span)
                    );
                }
            };
            arguments.push(reference);
        }
        let arguments = self
            .syntax
            .type_references
            .insert_type_reference_handles(arguments);
        Ok(Some(self.syntax.type_references.insert(
            TypeReferenceNode::Generic {
                base_name: application.name,
                lifetime_arguments: Default::default(),
                arguments,
            },
        )))
    }

    fn application_integer(
        &self,
        reference: TypeReferenceHandle,
        binders: bool,
        span: SourceSpan,
    ) -> Result<Option<BigInt>, Diagnostic> {
        let TypeReferenceNode::Named(name) = self.syntax.type_references.type_reference(reference)
        else {
            return Err(self
                .type_structure_error("requires a normalized integer constructor argument", span));
        };
        if binders && let Some((position, is_type)) = self.parameter_position(name.as_str()) {
            if is_type || self.base_info.const_parameter_types[position].is_none() {
                return Err(self.type_structure_error(
                    "mixes type and value kinds in a constructor argument",
                    span,
                ));
            }
            return match &self.bindings[position] {
                None => Ok(None),
                Some(Binding::Integer(value)) => Ok(Some(value.clone())),
                _ => Err(self.type_structure_error("requires an integer const binding", span)),
            };
        }
        name.as_str()
            .parse::<i128>()
            .map(BigInt::from_i128)
            .map(Some)
            .map_err(|_| {
                self.type_structure_error(
                    "requires a normalized integer constructor argument",
                    span,
                )
            })
    }

    fn boolean_constructor_parameter(&self, reference: TypeReferenceHandle) -> bool {
        let TypeReferenceNode::Named(name) = self.syntax.type_references.type_reference(reference)
        else {
            return false;
        };
        closed_name_identity(self.syntax, self.selection, name)
            == Some(ClosedArgumentIdentity::Builtin(
                symbols::BuiltinTypeAtom::Bool,
            ))
    }

    fn application_boolean(
        &self,
        reference: TypeReferenceHandle,
        binders: bool,
        span: SourceSpan,
    ) -> Result<Option<bool>, Diagnostic> {
        let TypeReferenceNode::Named(name) = self.syntax.type_references.type_reference(reference)
        else {
            return Err(self
                .type_structure_error("requires a normalized Boolean constructor argument", span));
        };
        if binders && let Some((position, is_type)) = self.parameter_position(name.as_str()) {
            if is_type
                || self.base_info.const_parameter_types[position]
                    .is_none_or(|reference| !self.boolean_constructor_parameter(reference))
            {
                return Err(self.type_structure_error(
                    "requires a Boolean const binder in a Boolean constructor position",
                    span,
                ));
            }
            return match self.bindings[position] {
                None => Ok(None),
                Some(Binding::Boolean(value)) => Ok(Some(value)),
                _ => Err(self.type_structure_error("requires a Boolean const binding", span)),
            };
        }
        normalized_boolean_argument(name.as_str())
            .map(Some)
            .ok_or_else(|| {
                self.type_structure_error(
                    "requires a normalized Boolean constructor argument",
                    span,
                )
            })
    }

    fn match_application_boolean(
        &mut self,
        pattern: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let value = self
            .application_boolean(actual, false, span)?
            .ok_or_else(|| {
                self.type_structure_error("requires a closed Boolean constructor argument", span)
            })?;
        // Check the declared binder carrier even when it already has an explicit
        // value: equal encodings cannot justify a Boolean/integer kind mixture.
        let expected = self.application_boolean(pattern, true, span)?;
        if let Some(expected) = expected {
            if expected != value {
                return Err(
                    self.type_structure_error("has conflicting constructor constants", span)
                );
            }
        } else if let TypeReferenceNode::Named(name) =
            self.syntax.type_references.type_reference(pattern)
            && let Some((position, false)) = self.parameter_position(name.as_str())
        {
            self.bindings[position] = Some(Binding::Boolean(value));
        } else {
            return Err(self.type_structure_error("lost its Boolean constructor binder", span));
        }
        Ok(())
    }

    // ClosedArgumentIdentity currently omits application lifetimes. Do not
    // let a whole recovered element conceal one from repeated-equality checks.
    pub(in crate::preparation::type_equations) fn require_lifetime_free_argument(
        &self,
        reference: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let origin = self
            .syntax
            .type_references
            .generic_application_origin(reference);
        let reference = if origin.is_valid() { origin } else { reference };
        match self.syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Reference {
                lifetime: Some(_), ..
            } => {
                return Err(self.type_structure_error("requires anonymous reference lifetimes until lexical lifetime identity is retained", span));
            }
            TypeReferenceNode::Generic { .. } => {
                let application = self.application(reference, false, span)?;
                for (argument, parameter) in application
                    .arguments
                    .into_iter()
                    .zip(application.parameters)
                {
                    if matches!(parameter.kind, TypeParameterKind::Type) {
                        self.require_lifetime_free_argument(argument, span)?;
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::Reference {
                referee: element_type,
                ..
            } => {
                self.require_lifetime_free_argument(*element_type, span)?;
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.require_lifetime_free_argument(*base_type, span)?;
            }
            TypeReferenceNode::Named(name) => {
                if let Some(ClosedArgumentIdentity::Nominal(declaration)) =
                    closed_name_identity(self.syntax, self.selection, name)
                    && let Item::Data(definition) = self.syntax.root_item(declaration)
                    && (!definition.lifetime_parameters.is_empty()
                        || definition.generic_instance.is_some())
                {
                    return Err(self.type_structure_error(
                        "requires original lifetime-free element structure",
                        span,
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_application_integer(
        &self,
        reference: TypeReferenceHandle,
        value: &BigInt,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let TypeReferenceNode::Named(name) = self.syntax.type_references.type_reference(reference)
        else {
            return Err(
                self.type_structure_error("requires a builtin integer constructor parameter", span)
            );
        };
        let Some((minimum, maximum)) = const_binder_envelope(name.as_str()) else {
            return Err(
                self.type_structure_error("requires a builtin integer constructor parameter", span)
            );
        };
        if *value < minimum || *value > maximum {
            return Err(self.type_structure_error(
                "has a constructor constant outside its declared carrier",
                span,
            ));
        }
        Ok(())
    }
}

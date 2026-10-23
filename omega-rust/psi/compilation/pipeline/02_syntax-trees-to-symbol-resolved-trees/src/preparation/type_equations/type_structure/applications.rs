//! Nominal constructors decompose by declaration identity and parameter kind.
//! Original application trees survive synthesis; generated display names never
//! recover their arguments. This is structural matching, not const evaluation.
//! Declared domains join the same tuple discipline: a carrier-qualified
//! `u64::AtMost<N>` head and the constrained `u64 in AtMost<N>` spelling name
//! the same application, indices bind by the family's declared telescope, and
//! distinct indices gain no implicit variance — a predicate is never searched
//! for a maximum.
//! Forwarded const binders retain their declared carrier until the selected
//! constructor telescope has checked it. Scalar bindings alone cannot carry
//! this authority: matching and reconstruction normalize them to values.
//! Closed named constructor indices instead select in the equation's authored
//! source scope. Their declarations pass the ordinary const-index canonicalizer
//! against the constructor's carrier before name erasure. Matching, construction
//! and repeated equality share that selection; equal scalar payloads cannot
//! justify a different carrier, and unresolved names never use a lexical map.

use super::super::{Binding, Solver, const_binder_envelope, normalized_boolean_argument};
use crate::preparation::generic_data::{
    ClosedArgumentIdentity, DomainCarrier, DomainHead, canonicalize_selected_const_definition,
    closed_argument_identity, closed_name_identity, domain_application_head,
    domain_index_telescope, selected_domain_family,
};
use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use numerics::bignum::BigInt;
use source::SourceSpan;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ItemHandle, TypeParameter, TypeParameterKind};
use syntax_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(super) struct Application {
    name: Identifier,
    declaration: ItemHandle,
    arguments: Vec<TypeReferenceHandle>,
    parameters: Vec<TypeParameter>,
    /// `Some` when the head is a declared domain: construction emits the
    /// constrained runtime spelling rather than a nominal generic node.
    domain: Option<DomainApplication>,
}

struct DomainApplication {
    /// The declaration's qualified name; it materializes the constraint name
    /// verbatim so module-law selection decides reachability downstream.
    declared_name: Identifier,
    /// The matched carrier: the declared target for a `Carrier::Name<..>`
    /// head, the authored base for a `base in Name<..>` constraint.
    carrier: DomainCarrier,
}

impl Solver<'_, '_> {
    pub(super) fn application(
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
        let application = match self.syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if pattern
                    && self
                        .parameter_position(base_name.as_str(self.syntax))
                        .is_some()
                {
                    return Err(self.type_structure_error(
                        "cannot use a binder as a nominal constructor",
                        span,
                    ));
                }
                let arguments = self
                    .syntax
                    .type_references
                    .type_reference_handles(*arguments)
                    .to_vec();
                let (name, declaration, parameters, domain) =
                    match closed_name_identity(self.syntax, self.selection, base_name) {
                        Some(ClosedArgumentIdentity::Nominal(declaration)) => {
                            let Some(definition) = self.syntax.root_item(declaration).as_data()
                            else {
                                return Err(
                                    self.type_structure_error("requires a data constructor", span)
                                );
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
                            (
                                base_name.clone(),
                                declaration,
                                self.syntax
                                    .items
                                    .type_parameters(definition.type_parameters)
                                    .to_vec(),
                                None,
                            )
                        }
                        _ => {
                            let Some(domain) = self.domain_head(base_name) else {
                                return Err(self.type_structure_error(
                                    "requires a selected application declaration",
                                    span,
                                ));
                            };
                            if !lifetime_arguments.is_empty() {
                                return Err(self.type_structure_error(
                                    "requires original lifetime-free application structure",
                                    span,
                                ));
                            }
                            (
                                base_name.clone(),
                                domain.declaration,
                                domain.index_parameters,
                                Some(DomainApplication {
                                    declared_name: domain.declared_name,
                                    carrier: domain.carrier,
                                }),
                            )
                        }
                    };
                self.complete_application(name, declaration, parameters, arguments, domain, span)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let constraints = self.syntax.type_references.constraints(*constraints);
                let [TypeConstraintNode::Domain(constraint)] = constraints else {
                    return Err(
                        self.type_structure_error("requires an exact declared application", span)
                    );
                };
                self.domain_constraint_application(*base_type, constraint, span)
            }
            _ => Err(self.type_structure_error("requires an exact declared application", span)),
        }?;
        if pattern {
            self.require_constructor_const_binder_carriers(&application, span)?;
        }
        Ok(application)
    }

    fn require_constructor_const_binder_carriers(
        &self,
        application: &Application,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        for (argument, parameter) in application.arguments.iter().zip(&application.parameters) {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                continue;
            };
            let Some(name) = self
                .syntax
                .type_references
                .type_reference(*argument)
                .as_named()
            else {
                continue;
            };
            let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
            else {
                continue;
            };
            let binder_carrier = self.base_info.const_parameter_types[position];
            if is_type || binder_carrier.is_none() {
                return Err(self.type_structure_error(
                    "mixes type and value kinds in a constructor argument",
                    span,
                ));
            }
            // Compare declaration identities before either direction erases
            // the binder into an integer/Boolean atom. Equal values or overlapping
            // numeric envelopes do not authorize forwarding a different carrier.
            let selected_carrier =
                closed_argument_identity(self.syntax, self.selection, type_reference, false);
            let binder_carrier = binder_carrier.and_then(|carrier| {
                closed_argument_identity(self.syntax, self.selection, carrier, false)
            });
            if selected_carrier.is_none() || selected_carrier != binder_carrier {
                return Err(self.type_structure_error(
                    "constructor const binder has a different declared carrier",
                    span,
                ));
            }
        }
        Ok(())
    }

    /// The tuple discipline shared by nominal and domain applications: arity
    /// against the head's declared telescope, then only type and canonical const
    /// indices.
    fn complete_application(
        &self,
        name: Identifier,
        declaration: ItemHandle,
        parameters: Vec<TypeParameter>,
        arguments: Vec<TypeReferenceHandle>,
        domain: Option<DomainApplication>,
        span: SourceSpan,
    ) -> Result<Application, Diagnostic> {
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
            name,
            declaration,
            arguments,
            parameters,
            domain,
        })
    }

    /// A carrier-qualified head selects the in-forest domain by leaf name and
    /// carrier: `u64::AtMost` reaches `domain<const C: u64> u64::AtMost<C>`,
    /// whose declared name retains only the domain segments. Generic families
    /// retain the spelled carrier for ordinary structural type binding, apart
    /// from the index telescope. The selection law is shared with argument
    /// identity in `generic_data::arguments`.
    fn domain_head(&self, name: &Identifier) -> Option<DomainHead> {
        domain_application_head(self.syntax, self.selection, name)
    }

    /// The `base in Name<index, ..>` spelling: the constraint name reaches a
    /// declaration's leaf or its full qualified name, and the declared carrier
    /// must be the base's closed identity. Competing carriers or an ambiguous
    /// leaf decline rather than match on spelling.
    fn domain_constraint_application(
        &self,
        base_type: TypeReferenceHandle,
        constraint: &DomainConstraint,
        span: SourceSpan,
    ) -> Result<Application, Diagnostic> {
        let decline = || {
            self.type_structure_error(
                "requires a selected domain application declaration",
                constraint.name.source_span(),
            )
        };
        let declaration =
            selected_domain_family(self.syntax, self.selection, &constraint.name, None)
                .ok_or_else(decline)?;
        let definition = self
            .syntax
            .root_item(declaration)
            .as_domain()
            .ok_or_else(decline)?;
        let (index_parameters, generic_carrier) = domain_index_telescope(self.syntax, definition);
        if !generic_carrier {
            self.require_equal_type_structure(definition.target_type, base_type, span)?;
        }
        let arguments = self
            .syntax
            .type_references
            .type_reference_handles(constraint.arguments)
            .to_vec();
        self.complete_application(
            constraint.name.clone(),
            declaration,
            index_parameters,
            arguments,
            Some(DomainApplication {
                declared_name: definition.name.clone(),
                carrier: DomainCarrier::Type(base_type),
            }),
            span,
        )
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
        if let (Some(pattern_domain), Some(actual_domain)) = (&pattern.domain, &actual.domain) {
            let pattern_carrier = pattern_domain.carrier.clone().materialize(self.syntax);
            let actual_carrier = actual_domain.carrier.clone().materialize(self.syntax);
            self.match_type_structure(pattern_carrier, actual_carrier, span)?;
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
                        self.match_application_boolean(pattern, actual, type_reference, span)?;
                        continue;
                    }
                    if !self.integer_constructor_parameter(type_reference) {
                        self.match_application_constant(pattern, actual, type_reference, span)?;
                        continue;
                    }
                    let value = self
                        .application_integer(actual, type_reference, false, span)?
                        .ok_or_else(|| {
                            self.type_structure_error(
                                "requires a normalized integer constructor argument",
                                span,
                            )
                        })?;
                    self.validate_application_integer(type_reference, &value, span)?;
                    if let Some(name) = self
                        .syntax
                        .type_references
                        .type_reference(pattern)
                        .as_named()
                        && let Some((position, is_type)) =
                            self.parameter_position(name.as_str(self.syntax))
                    {
                        if is_type || self.base_info.const_parameter_types[position].is_none() {
                            return Err(self.type_structure_error(
                                "mixes type and value kinds in a constructor argument",
                                span,
                            ));
                        }
                        self.bind_integer(position, value, span)?;
                    } else if self.application_integer(pattern, type_reference, true, span)?
                        != Some(value)
                    {
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
                        let Some(value) =
                            self.application_boolean(argument, type_reference, true, span)?
                        else {
                            return Ok(None);
                        };
                        arguments.push(self.syntax.type_references.insert(
                            TypeReferenceNode::Named(Identifier::generated(
                                CanonicalConstValue::boolean(value).atom(),
                            )),
                        ));
                        continue;
                    }
                    if !self.integer_constructor_parameter(type_reference) {
                        let Some((reference, _)) =
                            self.application_constant(argument, type_reference, true, span)?
                        else {
                            return Ok(None);
                        };
                        arguments.push(reference);
                        continue;
                    }
                    let Some(value) =
                        self.application_integer(argument, type_reference, true, span)?
                    else {
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
        if let Some(domain) = application.domain {
            let carrier = domain.carrier.materialize(self.syntax);
            let Some(base_type) = self.construct_type_structure(carrier, span)? else {
                return Ok(None);
            };
            let constraints =
                self.syntax
                    .type_references
                    .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                        name: domain.declared_name,
                        arguments,
                    })]);
            return Ok(Some(self.syntax.type_references.insert(
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                },
            )));
        }
        Ok(Some(self.syntax.type_references.insert(
            TypeReferenceNode::Generic {
                base_name: application.name,
                lifetime_arguments: Default::default(),
                arguments,
            },
        )))
    }

    pub(super) fn application_integer(
        &self,
        reference: TypeReferenceHandle,
        parameter_type: TypeReferenceHandle,
        binders: bool,
        span: SourceSpan,
    ) -> Result<Option<BigInt>, Diagnostic> {
        let Some(name) = self
            .syntax
            .type_references
            .type_reference(reference)
            .as_named()
        else {
            return Err(self
                .type_structure_error("requires a normalized integer constructor argument", span));
        };
        if binders
            && let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
        {
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
        if let Ok(value) = name.as_str(self.syntax).parse::<i128>() {
            return Ok(Some(BigInt::from_i128(value)));
        }
        match self
            .selected_constructor_constant(name, parameter_type)?
            .decode_encoding()
        {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => {
                Ok(Some(BigInt::from_i128(value)))
            }
            _ => Err(self.type_structure_error("requires an integer constructor constant", span)),
        }
    }

    /// Selection and index validation precede scalar observation. In particular,
    /// a named u32 with value four is not an argument for a u64 constructor.
    fn selected_constructor_constant(
        &self,
        name: &Identifier,
        parameter_type: TypeReferenceHandle,
    ) -> Result<CanonicalConstValue, Diagnostic> {
        let span = name.source_span();
        let selection = self.selection.ok_or_else(|| {
            self.type_structure_error(
                "requires source-scoped constructor constant selection",
                span,
            )
        })?;
        let definition = selection.select(self.syntax, name)?.ok_or_else(|| {
            self.type_structure_error(
                "requires an unambiguous selected constructor constant",
                span,
            )
        })?;
        canonicalize_selected_const_definition(
            self.syntax,
            &definition,
            parameter_type,
            Some(selection),
        )
        .map_err(|reason| self.type_structure_error(&reason, span))
    }

    fn integer_constructor_parameter(&self, reference: TypeReferenceHandle) -> bool {
        self.syntax
            .type_references
            .type_reference(reference)
            .as_named()
            .is_some_and(|name| const_binder_envelope(name.as_str(self.syntax)).is_some())
    }

    /// Canonical identity compares values, not the atom's diagnostic rendering.
    /// The selected constructor telescope and its forwarded-binder carrier check
    /// supply nominal authority. Retaining the actual reference also retains its
    /// authored constant/expression custody for reconstruction and machine calls.
    fn application_constant(
        &self,
        reference: TypeReferenceHandle,
        parameter_type: TypeReferenceHandle,
        binders: bool,
        span: SourceSpan,
    ) -> Result<Option<(TypeReferenceHandle, CanonicalConstValue)>, Diagnostic> {
        let Some(name) = self
            .syntax
            .type_references
            .type_reference(reference)
            .as_named()
        else {
            return Err(self.type_structure_error(
                "requires a normalized structural constructor constant",
                span,
            ));
        };
        if binders
            && let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
        {
            if is_type || self.base_info.const_parameter_types[position].is_none() {
                return Err(self.type_structure_error(
                    "mixes type and value kinds in a constructor argument",
                    span,
                ));
            }
            return match self.bindings[position] {
                None => Ok(None),
                Some(Binding::Constant(reference)) => {
                    self.application_constant(reference, parameter_type, false, span)
                }
                _ => Err(self.type_structure_error("requires a structural const binding", span)),
            };
        }
        let value = if let Some(value) = CanonicalConstValue::from_atom(name.as_str(self.syntax)) {
            if value.decode_encoding().is_none() {
                return Err(self.type_structure_error(
                    "requires a canonical structural constructor constant",
                    span,
                ));
            }
            value
        } else {
            self.selected_constructor_constant(name, parameter_type)?
        };
        Ok(Some((reference, value)))
    }

    fn match_application_constant(
        &mut self,
        pattern: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        parameter_type: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let (reference, value) = self
            .application_constant(actual, parameter_type, false, span)?
            .ok_or_else(|| {
                self.type_structure_error("requires a closed structural constructor constant", span)
            })?;
        if let Some((_, expected)) =
            self.application_constant(pattern, parameter_type, true, span)?
        {
            if expected.identity() != value.identity() {
                return Err(
                    self.type_structure_error("has conflicting constructor constants", span)
                );
            }
        } else if let Some(name) = self
            .syntax
            .type_references
            .type_reference(pattern)
            .as_named()
            && let Some((position, false)) = self.parameter_position(name.as_str(self.syntax))
        {
            self.bindings[position] = Some(Binding::Constant(reference));
        } else {
            return Err(self.type_structure_error("lost its structural constructor binder", span));
        }
        Ok(())
    }

    fn boolean_constructor_parameter(&self, reference: TypeReferenceHandle) -> bool {
        let Some(name) = self
            .syntax
            .type_references
            .type_reference(reference)
            .as_named()
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
        parameter_type: TypeReferenceHandle,
        binders: bool,
        span: SourceSpan,
    ) -> Result<Option<bool>, Diagnostic> {
        let Some(name) = self
            .syntax
            .type_references
            .type_reference(reference)
            .as_named()
        else {
            return Err(self
                .type_structure_error("requires a normalized Boolean constructor argument", span));
        };
        if binders
            && let Some((position, is_type)) = self.parameter_position(name.as_str(self.syntax))
        {
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
        if let Some(value) = normalized_boolean_argument(name.as_str(self.syntax)) {
            return Ok(Some(value));
        }
        match self
            .selected_constructor_constant(name, parameter_type)?
            .decode_encoding()
        {
            Some(DecodedCanonicalConstValue::Boolean(value)) => Ok(Some(value)),
            _ => Err(self.type_structure_error("requires a Boolean constructor constant", span)),
        }
    }

    fn match_application_boolean(
        &mut self,
        pattern: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        parameter_type: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let value = self
            .application_boolean(actual, parameter_type, false, span)?
            .ok_or_else(|| {
                self.type_structure_error("requires a closed Boolean constructor argument", span)
            })?;
        // Check the declared binder carrier even when it already has an explicit
        // value: equal encodings cannot justify a Boolean/integer kind mixture.
        let expected = self.application_boolean(pattern, parameter_type, true, span)?;
        if let Some(expected) = expected {
            if expected != value {
                return Err(
                    self.type_structure_error("has conflicting constructor constants", span)
                );
            }
        } else if let Some(name) = self
            .syntax
            .type_references
            .type_reference(pattern)
            .as_named()
            && let Some((position, false)) = self.parameter_position(name.as_str(self.syntax))
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
                    && let Some(definition) = self.syntax.root_item(declaration).as_data()
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
        let Some(name) = self
            .syntax
            .type_references
            .type_reference(reference)
            .as_named()
        else {
            return Err(
                self.type_structure_error("requires a builtin integer constructor parameter", span)
            );
        };
        let Some((minimum, maximum)) = const_binder_envelope(name.as_str(self.syntax)) else {
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

    /// Domain applications have no closed argument identity yet; compare two
    /// closed occurrences by selected declaration and each index argument.
    /// This is equality, not matching: binders never bind here.
    pub(super) fn require_equal_application_structure(
        &self,
        expected: TypeReferenceHandle,
        actual: TypeReferenceHandle,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let undecidable = || {
            self.type_structure_error(
                "cannot decide an open or unsupported element type; supply explicit arguments",
                span,
            )
        };
        let (Ok(expected), Ok(actual)) = (
            self.application(expected, false, span),
            self.application(actual, false, span),
        ) else {
            return Err(undecidable());
        };
        if expected.declaration != actual.declaration {
            return Err(self.type_structure_error("has conflicting element types", span));
        }
        if let (Some(expected_domain), Some(actual_domain)) = (&expected.domain, &actual.domain) {
            let expected_carrier = expected_domain
                .carrier
                .identity(self.syntax, self.selection)
                .ok_or_else(undecidable)?;
            let actual_carrier = actual_domain
                .carrier
                .identity(self.syntax, self.selection)
                .ok_or_else(undecidable)?;
            if expected_carrier != actual_carrier {
                return Err(self.type_structure_error("has conflicting domain carriers", span));
            }
        }
        for ((expected, actual), parameter) in expected
            .arguments
            .into_iter()
            .zip(actual.arguments)
            .zip(expected.parameters)
        {
            match parameter.kind {
                TypeParameterKind::Type => {
                    self.require_equal_type_structure(expected, actual, span)?;
                }
                TypeParameterKind::Const { type_reference } => {
                    if self.boolean_constructor_parameter(type_reference) {
                        let expected =
                            self.application_boolean(expected, type_reference, false, span)?;
                        let actual =
                            self.application_boolean(actual, type_reference, false, span)?;
                        if expected != actual || expected.is_none() {
                            return Err(undecidable());
                        }
                        continue;
                    }
                    if !self.integer_constructor_parameter(type_reference) {
                        let expected =
                            self.application_constant(expected, type_reference, false, span)?;
                        let actual =
                            self.application_constant(actual, type_reference, false, span)?;
                        if expected.as_ref().map(|(_, value)| value.identity())
                            != actual.as_ref().map(|(_, value)| value.identity())
                            || expected.is_none()
                        {
                            return Err(undecidable());
                        }
                        continue;
                    }
                    let expected =
                        self.application_integer(expected, type_reference, false, span)?;
                    let actual = self.application_integer(actual, type_reference, false, span)?;
                    if expected != actual || expected.is_none() {
                        return Err(undecidable());
                    }
                }
                _ => return Err(undecidable()),
            }
        }
        Ok(())
    }
}

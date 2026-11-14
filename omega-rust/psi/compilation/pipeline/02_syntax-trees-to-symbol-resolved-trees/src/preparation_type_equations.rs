/*
    Recover static arguments from declared structural type equations.

    A `requires` equation such as `Backing == [Element; Count]` relates exact
    type structure. A partial argument tuple can recover omitted binders from
    that structure; every complete tuple still proves the entire equation.
    Predicates on runtime scalar values are ordinary contract obligations and
    do not define structural type arguments.

    Fixed arrays, slices, anonymous references and declared generic applications
    retain ordinary type trees.
    Their type and canonical const positions recursively recover binders, or build
    an omitted type once those binders are known. `type_structure` owns this
    traversal: nominal heads join by selected declaration, never layout or leaf
    spelling. Declared domains join the same traversal by declaration and
    carrier — the `u64::AtMost<N>` head and the `u64 in AtMost<N>` constraint
    name one application whose indices are the family's declared binders.
    Closed leaves still use `closed_argument_identity`.
    Name operands retain their complete path and authored selection span. Only
    bare names may capture template binders; qualification neither turns a type
    into a value nor grants type authority to a constant. Closed-name recovery
    checks the same selected type as matching before publishing a named binding.
    Borrow and singleton-array operands gain type role only opposite a declared
    type binder. They materialize into that same type tree; ordinary value-borrow
    equalities remain value facts, never parser guesses about identifier spelling.
    Runtime contents and compatible storage sizes never supply an argument.
    Data synthesis and machine-call preparation supply their exact selected
    declaration telescopes; this solver owns neither call selection nor
    publication. Machine preparation combines closed declared ordinary types
    with fixed explicit arguments and sends completed type roots through data normalization;
    an equation does not discharge the selected constructor's own obligations.
*/

use crate::preparation::declaration_selection;
use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::closed_argument_identity;
use crate::preparation::generic_data::closed_name_identity;

use crate::preparation::generic_data::selected_data_item;
use diagnostics::Diagnostic;
use language_semantics::const_value::{
    CanonicalConstValue, DecodedCanonicalConstValue, boolean_literal_spelling,
};
use numerics::bignum::BigInt;
use numerics::literals::LandedIntegerType;
use source::SourceSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::item::{ProofFact, TypeParameter, TypeParameterKind};
use syntax_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use type_structure::collect_binder_mentions;

mod type_structure {
    /*
        Decompose declared type constructors without observing runtime contents.

        Element positions bind types and length positions bind integer constants.
        Nominal const positions additionally preserve Boolean identity and kind.
        Arrays, slices, references and nominal applications share recursion;
        references compare exact access, not borrow compatibility. A selected
        callable may introduce fresh invocation regions; its declared telescope
        gives those tags binder role. Captured actual regions still require lexical
        custody, never matching names or region-erased type identity.
        `applications` selects the
        exact declaration and parameter kinds of each nominal head. Closed leaves
        use the existing identity owner. This is not an arithmetic solver or a new
        source of array-length evaluation.
        Actual array geometry uses the same closed-extent reader as instance
        identity and diagnostic names. Actual names belong to their authored scope,
        not the template's binder roster. Keep those authored nodes intact: later
        resolution and typing retain their selection and constant-origin custody.
    */

    use super::{Binding, Solver, collect_expression_binder_mentions};
    use crate::preparation::generic_data::{
        canonicalize_selected_declared_const_definition, closed_argument_identity,
        closed_fixed_array_length,
    };
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
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

    mod applications {
        /*
            Nominal constructors decompose by declaration identity and parameter kind.
            Original application trees survive synthesis; generated display names never
            recover their arguments. This is structural matching, not const evaluation.
            Declared domains join the same tuple discipline: a carrier-qualified
            `u64::AtMost<N>` head and the constrained `u64 in AtMost<N>` spelling name
            the same application, indices bind by the family's declared telescope, and
            distinct indices gain no implicit variance — a predicate is never searched
            for a maximum.
            Forwarded const binders retain their declared carrier until the selected
            constructor telescope has checked it. Scalar bindings alone cannot carry
            this authority: matching and reconstruction normalize them to values.
            Closed named constructor indices instead select in the equation's authored
            source scope. Their declarations pass the ordinary const-index canonicalizer
            against the constructor's carrier before name erasure. Matching, construction
            and repeated equality share that selection; equal scalar payloads cannot
            justify a different carrier, and unresolved names never use a lexical map.
        */

        use super::super::{Binding, Solver, const_binder_envelope, normalized_boolean_argument};
        use crate::preparation::generic_data::{
            ClosedArgumentIdentity, DomainCarrier, DomainHead,
            canonicalize_selected_const_definition, closed_argument_identity, closed_name_identity,
            domain_application_head, domain_index_telescope, selected_domain_family,
        };
        use crate::tables::syntax_items;
        use crate::tables::syntax_type_references;
        use diagnostics::Diagnostic;
        use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
        use numerics::bignum::BigInt;
        use source::SourceSpan;
        use symbols::BuiltinTypeAtom;
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
            /*
                `Some` when the head is a declared domain: construction emits the
                constrained runtime spelling rather than a nominal generic node.
            */
            domain: Option<DomainApplication>,
        }

        struct DomainApplication {
            /*
                The declaration's qualified name; it materializes the constraint name
                verbatim so module-law selection decides reachability downstream.
            */
            declared_name: Identifier,
            /*
                The matched carrier: the declared target for a `Carrier::Name<..>`
                head, the authored base for a `base in Name<..>` constraint.
            */
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
                                    let Some(definition) =
                                        self.syntax.root_item(declaration).as_data()
                                    else {
                                        return Err(self.type_structure_error(
                                            "requires a data constructor",
                                            span,
                                        ));
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
                                        syntax_items(self.syntax)
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
                        self.complete_application(
                            name,
                            declaration,
                            parameters,
                            arguments,
                            domain,
                            span,
                        )
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        let constraints = self.syntax.type_references.constraints(*constraints);
                        let [TypeConstraintNode::Domain(constraint)] = constraints else {
                            return Err(self.type_structure_error(
                                "requires an exact declared application",
                                span,
                            ));
                        };
                        self.domain_constraint_application(*base_type, constraint, span)
                    }
                    _ => {
                        Err(self
                            .type_structure_error("requires an exact declared application", span))
                    }
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
                for (argument, parameter) in
                    application.arguments.iter().zip(&application.parameters)
                {
                    let TypeParameterKind::Const { type_reference } = parameter.kind else {
                        continue;
                    };
                    let Some(name) = self.type_reference_name(*argument) else {
                        continue;
                    };
                    let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                    let selected_carrier = closed_argument_identity(
                        self.syntax,
                        self.selection,
                        type_reference,
                        false,
                    );
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

            /*
                The tuple discipline shared by nominal and domain applications: arity
                against the head's declared telescope, then only type and canonical const
                indices.
            */
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
                    return Err(self.type_structure_error(
                        "requires a complete constructor argument tuple",
                        span,
                    ));
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

            /*
                A carrier-qualified head selects the in-forest domain by leaf name and
                carrier: `u64::AtMost` reaches `domain<const C: u64> u64::AtMost<C>`,
                whose declared name retains only the domain segments. Generic families
                retain the spelled carrier for ordinary structural type binding, apart
                from the index telescope. The selection law is shared with argument
                identity in `generic_data::arguments`.
            */
            fn domain_head(&self, name: &Identifier) -> Option<DomainHead> {
                domain_application_head(self.syntax, self.selection, name)
            }

            /*
                The `base in Name<index, ..>` spelling: the constraint name reaches a
                declaration's leaf or its full qualified name, and the declared carrier
                must be the base's closed identity. Competing carriers or an ambiguous
                leaf decline rather than match on spelling.
            */
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
                let (index_parameters, generic_carrier) =
                    domain_index_telescope(self.syntax, definition);
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
                    return Err(
                        self.type_structure_error("has conflicting nominal constructors", span)
                    );
                }
                if let (Some(pattern_domain), Some(actual_domain)) =
                    (&pattern.domain, &actual.domain)
                {
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
                                self.match_application_boolean(
                                    pattern,
                                    actual,
                                    type_reference,
                                    span,
                                )?;
                                continue;
                            }
                            if !self.integer_constructor_parameter(type_reference) {
                                self.match_application_constant(
                                    pattern,
                                    actual,
                                    type_reference,
                                    span,
                                )?;
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
                            if let Some(name) = self.type_reference_name(pattern)
                                && let Some((position, is_type)) =
                                    self.parameter_position(name.as_str(self.syntax))
                            {
                                if is_type
                                    || self.base_info.const_parameter_types[position].is_none()
                                {
                                    return Err(self.type_structure_error(
                                        "mixes type and value kinds in a constructor argument",
                                        span,
                                    ));
                                }
                                self.bind_integer(position, value, span)?;
                            } else if self.application_integer(
                                pattern,
                                type_reference,
                                true,
                                span,
                            )? != Some(value)
                            {
                                return Err(self.type_structure_error(
                                    "has conflicting constructor constants",
                                    span,
                                ));
                            }
                        }
                        _ => {
                            return Err(self.type_structure_error(
                                "has an unsupported constructor parameter",
                                span,
                            ));
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
                            let Some(reference) = self.construct_type_structure(argument, span)?
                            else {
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
                                let Some((reference, _)) = self.application_constant(
                                    argument,
                                    type_reference,
                                    true,
                                    span,
                                )?
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
                            return Err(self.type_structure_error(
                                "has an unsupported constructor parameter",
                                span,
                            ));
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
                    let constraints = self.syntax.type_references.insert_constraints([
                        TypeConstraintNode::Domain(DomainConstraint {
                            name: domain.declared_name,
                            arguments,
                        }),
                    ]);
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
                let Some(name) = self.type_reference_name(reference) else {
                    return Err(self.type_structure_error(
                        "requires a normalized integer constructor argument",
                        span,
                    ));
                };
                if binders
                    && let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                        _ => {
                            Err(self
                                .type_structure_error("requires an integer const binding", span))
                        }
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
                    _ => {
                        Err(self
                            .type_structure_error("requires an integer constructor constant", span))
                    }
                }
            }

            /*
                Selection and index validation precede scalar observation. In particular,
                a named u32 with value four is not an argument for a u64 constructor.
            */
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
                syntax_type_references(self.syntax)
                    .type_reference(reference)
                    .as_named()
                    .is_some_and(|name| const_binder_envelope(name.as_str(self.syntax)).is_some())
            }

            /*
                Canonical identity compares values, not the atom's diagnostic rendering.
                The selected constructor telescope and its forwarded-binder carrier check
                supply nominal authority. Retaining the actual reference also retains its
                authored constant/expression custody for reconstruction and machine calls.
            */
            fn application_constant(
                &self,
                reference: TypeReferenceHandle,
                parameter_type: TypeReferenceHandle,
                binders: bool,
                span: SourceSpan,
            ) -> Result<Option<(TypeReferenceHandle, CanonicalConstValue)>, Diagnostic>
            {
                let Some(name) = self.type_reference_name(reference) else {
                    return Err(self.type_structure_error(
                        "requires a normalized structural constructor constant",
                        span,
                    ));
                };
                if binders
                    && let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                        _ => {
                            Err(self
                                .type_structure_error("requires a structural const binding", span))
                        }
                    };
                }
                let value =
                    if let Some(value) = CanonicalConstValue::from_atom(name.as_str(self.syntax)) {
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
                        self.type_structure_error(
                            "requires a closed structural constructor constant",
                            span,
                        )
                    })?;
                if let Some((_, expected)) =
                    self.application_constant(pattern, parameter_type, true, span)?
                {
                    if expected.identity() != value.identity() {
                        return Err(self
                            .type_structure_error("has conflicting constructor constants", span));
                    }
                } else if let Some(name) = self.type_reference_name(pattern)
                    && let Some((position, false)) =
                        self.parameter_position(name.as_str(self.syntax))
                {
                    self.bindings[position] = Some(Binding::Constant(reference));
                } else {
                    return Err(
                        self.type_structure_error("lost its structural constructor binder", span)
                    );
                }
                Ok(())
            }

            /* The identifier a reference names, absent a structural node. */
            fn type_reference_name(&self, reference: TypeReferenceHandle) -> Option<&Identifier> {
                syntax_type_references(self.syntax)
                    .type_reference(reference)
                    .as_named()
            }

            fn boolean_constructor_parameter(&self, reference: TypeReferenceHandle) -> bool {
                let Some(name) = self.type_reference_name(reference) else {
                    return false;
                };
                closed_name_identity(self.syntax, self.selection, name)
                    == Some(ClosedArgumentIdentity::Builtin(BuiltinTypeAtom::Bool))
            }

            fn application_boolean(
                &self,
                reference: TypeReferenceHandle,
                parameter_type: TypeReferenceHandle,
                binders: bool,
                span: SourceSpan,
            ) -> Result<Option<bool>, Diagnostic> {
                let Some(name) = self.type_reference_name(reference) else {
                    return Err(self.type_structure_error(
                        "requires a normalized Boolean constructor argument",
                        span,
                    ));
                };
                if binders
                    && let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                        _ => {
                            Err(self.type_structure_error("requires a Boolean const binding", span))
                        }
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
                    _ => {
                        Err(self
                            .type_structure_error("requires a Boolean constructor constant", span))
                    }
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
                        self.type_structure_error(
                            "requires a closed Boolean constructor argument",
                            span,
                        )
                    })?;
                let expected = self.application_boolean(pattern, parameter_type, true, span)?;
                if let Some(expected) = expected {
                    if expected != value {
                        return Err(self
                            .type_structure_error("has conflicting constructor constants", span));
                    }
                } else if let Some(name) = self.type_reference_name(pattern)
                    && let Some((position, false)) =
                        self.parameter_position(name.as_str(self.syntax))
                {
                    self.bindings[position] = Some(Binding::Boolean(value));
                } else {
                    return Err(
                        self.type_structure_error("lost its Boolean constructor binder", span)
                    );
                }
                Ok(())
            }

            /*
                ClosedArgumentIdentity currently omits application lifetimes. Do not
                let a whole recovered element conceal one from repeated-equality checks.
            */
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
                let Some(name) = self.type_reference_name(reference) else {
                    return Err(self.type_structure_error(
                        "requires a builtin integer constructor parameter",
                        span,
                    ));
                };
                let Some((minimum, maximum)) = const_binder_envelope(name.as_str(self.syntax))
                else {
                    return Err(self.type_structure_error(
                        "requires a builtin integer constructor parameter",
                        span,
                    ));
                };
                if *value < minimum || *value > maximum {
                    return Err(self.type_structure_error(
                        "has a constructor constant outside its declared carrier",
                        span,
                    ));
                }
                Ok(())
            }

            /*
                Domain applications have no closed argument identity yet; compare two
                closed occurrences by selected declaration and each index argument.
                This is equality, not matching: binders never bind here.
            */
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
                if let (Some(expected_domain), Some(actual_domain)) =
                    (&expected.domain, &actual.domain)
                {
                    let expected_carrier = expected_domain
                        .carrier
                        .identity(self.syntax, self.selection)
                        .ok_or_else(undecidable)?;
                    let actual_carrier = actual_domain
                        .carrier
                        .identity(self.syntax, self.selection)
                        .ok_or_else(undecidable)?;
                    if expected_carrier != actual_carrier {
                        return Err(
                            self.type_structure_error("has conflicting domain carriers", span)
                        );
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
                                let expected = self.application_boolean(
                                    expected,
                                    type_reference,
                                    false,
                                    span,
                                )?;
                                let actual =
                                    self.application_boolean(actual, type_reference, false, span)?;
                                if expected != actual || expected.is_none() {
                                    return Err(undecidable());
                                }
                                continue;
                            }
                            if !self.integer_constructor_parameter(type_reference) {
                                let expected = self.application_constant(
                                    expected,
                                    type_reference,
                                    false,
                                    span,
                                )?;
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
                            let actual =
                                self.application_integer(actual, type_reference, false, span)?;
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
    }

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
            TypeReferenceNode::Reference {
                referee, lifetime, ..
            } => {
                if let Some(lifetime) = lifetime {
                    name_mention(lifetime.as_str(syntax));
                }
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
                        TypeConstraintNode::Domain(domain) => {
                            if let Some(position) = parameters
                                .iter()
                                .position(|name| name == domain.name.as_str(syntax))
                            {
                                mentions.push(position);
                            }
                            for argument in syntax_type_references(syntax)
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
                for argument in
                    syntax_type_references(syntax).type_reference_handles(*trait_arguments)
                {
                    collect_binder_mentions(syntax, *argument, parameters, mentions);
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                super::collect_static_argument_binder_mentions(
                    syntax, selection, parameters, mentions,
                );
            }
            TypeReferenceNode::Unit => {}
        }
    }

    impl Solver<'_, '_> {
        /*
            Only the selected callable's own telescope introduces invocation variables.
            Data applications supply their regions explicitly; captured tags and named
            actuals remain subject to lexical identity rather than spelling equality.
        */
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

        /*
            Materialize only the type grammar retained in an ambiguous expression
            operand. Classification already established the opposite type binder;
            every resulting node rejoins the same constructor matcher below.
        */
        pub(super) fn type_operand_reference(
            &mut self,
            expression: syntax_trees::expression::ExpressionHandle,
            span: SourceSpan,
        ) -> Result<TypeReferenceHandle, Diagnostic> {
            use syntax_trees::expression::ExpressionNode;
            use syntax_trees::identifier::Identifier;
            match self.syntax.expressions.expression(expression).clone() {
                ExpressionNode::TypeExpression(reference) => Ok(reference),
                ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } if path_application.is_none() => {
                    let parts = self
                        .syntax
                        .expressions
                        .identifier_path_members(path.to_owned());
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
                        return Err(self.type_structure_error(
                            "requires exactly one slice element type",
                            span,
                        ));
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
            syntax_items(self.syntax)
                .type_parameters(self.base_info.parameters)
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.name.as_str(self.syntax) == name)
                .map(|(position, parameter)| {
                    (position, matches!(parameter.kind, TypeParameterKind::Type))
                })
        }

        fn type_structure_error(&self, reason: &str, span: SourceSpan) -> Diagnostic {
            self.reject(format!("requires type equation {reason}"))
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
                        return Err(
                            self.type_structure_error("has conflicting reference access", span)
                        );
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
                    if let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                                if closed_argument_identity(
                                    self.syntax,
                                    self.selection,
                                    actual,
                                    false,
                                )
                                .is_none()
                                {
                                    return Err(self.type_structure_error(
                                        "requires a closed element type",
                                        span,
                                    ));
                                }
                                self.bindings[position] = Some(Binding::Type(actual));
                                return Ok(());
                            }
                            Some(Binding::Type(reference)) => reference,
                            Some(Binding::NamedType(name)) => self
                                .syntax
                                .type_references
                                .insert(TypeReferenceNode::Named(name)),
                            Some(
                                Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_),
                            ) => {
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
                        return Err(self.type_structure_error(
                            "requires the same fixed-array constructor",
                            span,
                        ));
                    };
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

        /*
            A global declaration with a matching spelling cannot close a reference
            to this template's binder. Only the constructors decomposed above may
            recover binders; unsupported open leaves must not fall through to the
            canonical identity owner as if they were already closed arguments.
        */
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

        /*
            The shapes the constructor matcher can decompose without a closed
            identity: a generic application, or a carrier carrying one declared
            domain constraint.
        */
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
                    let Some(element_type) = self.construct_type_structure(element_type, span)?
                    else {
                        return Ok(None);
                    };
                    return Ok(Some(
                        self.syntax
                            .type_references
                            .insert(TypeReferenceNode::Slice { element_type }),
                    ));
                }
                TypeReferenceNode::Named(name) => {
                    if let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                            Some(
                                Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_),
                            ) => Err(self.type_structure_error(
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
                    let Some(element_type) = self.construct_type_structure(element_type, span)?
                    else {
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
                TypeReferenceNode::Generic { .. } => {
                    return self.construct_application(pattern, span);
                }
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
                    if let Some((position, is_type)) =
                        self.parameter_position(name.as_str(self.syntax))
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
                        return Err(
                            self.type_structure_error("requires a closed array length", span)
                        );
                    }
                }
            };
            integer
                .to_u64()
                .and_then(|value| usize::try_from(value).ok())
                .map(Some)
                .ok_or_else(|| {
                    self.type_structure_error(
                        "has an array length outside the supported extent",
                        span,
                    )
                })
        }
    }
}

pub(crate) fn type_reference_parameter_mentions(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    names: &[String],
) -> Vec<usize> {
    let mut mentions = Vec::new();
    if reference.is_valid() {
        collect_binder_mentions(syntax, reference, names, &mut mentions);
    }
    mentions
}

/*
    One declaration-level `requires` fact that states a type equation, by its offset in
    the template's fact span. Instances never carry it: applying the template
    decides it against the complete argument tuple.
*/
#[derive(Clone)]
pub(crate) struct TypeEquation {
    pub(crate) fact_offset: usize,
    definition: ExpressionHandle,
    shape: EquationShape,
}

impl TypeEquation {
    /*
        Conjunctions can recover the bindings of their structural leaves.
        Alternatives and negation only constrain an already selected tuple;
        their semantic predicates do not require syntax-based argument recovery.
    */
    pub(crate) fn requires_argument_recovery(&self) -> bool {
        match &self.shape {
            EquationShape::Structural { .. } => true,
            EquationShape::Boolean {
                operator: TypeBooleanOperator::And,
                operands,
                ..
            } => operands.iter().any(Self::requires_argument_recovery),
            EquationShape::Boolean { .. } | EquationShape::KindMismatch { .. } => false,
        }
    }

    /*
        The classifier, rather than an expression's spelling, gives this
        operand type role. Retain the equation in that declaration's scope.
    */
    pub(crate) fn structural_definition(&self) -> Option<(usize, ExpressionHandle)> {
        match self.shape {
            EquationShape::Structural { binder, .. } => Some((binder, self.definition)),
            EquationShape::Boolean { .. } | EquationShape::KindMismatch { .. } => None,
        }
    }

    /*
        A machine `requires` fact joins structural recovery only when its
        definition names another of the declaration's binders (`binders` lists
        its type parameters, then its lifetimes): only then can a call recover
        that binder from the defined one. A closed definition pins the binder
        exactly as an entry type predicate does, so it stays one. A mixed-kind
        equation stays here so it is rejected.
    */
    pub(crate) fn defines_structure(&self, syntax: &SyntaxTrees, binders: &[String]) -> bool {
        match &self.shape {
            EquationShape::Structural {
                binder, structure, ..
            } => structure
                .binder_mentions(syntax, binders)
                .iter()
                .any(|mention| mention != binder),
            EquationShape::KindMismatch { .. } => true,
            EquationShape::Boolean {
                operator: TypeBooleanOperator::And,
                operands,
                ..
            } => operands
                .iter()
                .any(|operand| operand.defines_structure(syntax, binders)),
            EquationShape::Boolean { .. } => false,
        }
    }

    /*
        Keep non-equation telescope slots out of structural matching without
        letting their names be reselected as closed declarations.
    */
    pub(crate) fn mentions_parameter(
        &self,
        syntax: &SyntaxTrees,
        names: &[String],
        index: usize,
    ) -> bool {
        match &self.shape {
            EquationShape::Structural {
                binder, structure, ..
            } => *binder == index || structure.binder_mentions(syntax, names).contains(&index),
            EquationShape::Boolean { operands, .. } => operands
                .iter()
                .any(|operand| operand.mentions_parameter(syntax, names, index)),
            EquationShape::KindMismatch { .. } => false,
        }
    }

    pub(crate) fn validate_kind(&self, declaration: &str) -> Result<(), Diagnostic> {
        if let EquationShape::Boolean { operands, .. } = &self.shape {
            for operand in operands {
                operand.validate_kind(declaration)?;
            }
        }
        if let EquationShape::KindMismatch { span, description } = &self.shape {
            return Err(Diagnostic::error(format!(
                "{declaration} requires equation mixes type and value kinds: {description}"
            ))
            .with_source_span(*span));
        }
        Ok(())
    }

    fn validate_names(
        &self,
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
    ) -> Result<(), Diagnostic> {
        self.validate_kind("declaration")?;
        match &self.shape {
            EquationShape::Structural {
                structure: Structure::Name { name, binder: None },
                span,
                ..
            } => validate_closed_name_type(syntax, selection, name)
                .map_err(|reason| Diagnostic::error(reason).with_source_span(*span)),
            EquationShape::Boolean { operands, .. } => {
                for operand in operands {
                    operand.validate_names(syntax, selection)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /*
        Closed names in a machine `requires` equation are entry predicates, not
        argument-recovery structure. Reject a const or missing operand here,
        before operand retention reads the same equality as a mixed type/value
        comparison. A selected data type stays for the later predicate check.
    */
    pub(crate) fn reject_unclosed_names(
        &self,
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
    ) -> Result<(), Diagnostic> {
        match &self.shape {
            EquationShape::Structural {
                structure: Structure::Name { name, binder: None },
                span,
                ..
            } => validate_closed_name_type(syntax, selection, name)
                .map_err(|reason| Diagnostic::error(reason).with_source_span(*span)),
            EquationShape::Boolean { operands, .. } => {
                for operand in operands {
                    operand.reject_unclosed_names(syntax, selection)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/*
    Static type equations remain specialization obligations, rather than value
    operators on a template's type binders. Named operands require the same
    selected, complete type identity that synthesis consumes. Range expressions
    still retain provisional resolution for their value-position endpoints.
    Closed instances must already have discharged equations and take no exemption.
*/
pub(crate) fn template_type_equation_offsets(
    syntax: &SyntaxTrees,
    definition: &syntax_trees::item::DataDefinition,
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Result<Vec<usize>, Diagnostic> {
    if definition.type_parameters.is_empty() {
        return Ok(Vec::new());
    }
    let mut offsets = Vec::new();
    for equation in classify_type_equations(
        syntax,
        syntax.items.type_parameters(definition.type_parameters),
        syntax.items.proof_facts(definition.requires_facts),
    ) {
        match equation.shape {
            EquationShape::Boolean { ref operands, .. } => {
                for operand in operands {
                    operand.validate_names(syntax, selection)?;
                }
                offsets.push(equation.fact_offset);
            }
            EquationShape::Structural {
                structure: Structure::TypeReference(_) | Structure::TypeOperand(_),
                ..
            } => offsets.push(equation.fact_offset),
            EquationShape::Structural {
                structure: Structure::Name { name, binder },
                span,
                ..
            } => {
                if binder.is_none() {
                    validate_closed_name_type(syntax, selection, &name).map_err(|reason| {
                        Diagnostic::error(format!(
                            "generic data `{}` {reason}",
                            definition.name.as_str(syntax),
                        ))
                        .with_source_span(span)
                    })?;
                }
                offsets.push(equation.fact_offset);
            }

            EquationShape::KindMismatch { span, description } => {
                return Err(Diagnostic::error(format!(
                    "generic data `{}` requires equation mixes type and value kinds: {description}",
                    definition.name.as_str(syntax),
                ))
                .with_source_span(span));
            }
        }
    }
    Ok(offsets)
}

/*
    Both template admission and specialization use the selected declaration's
    complete type role. A known generic head still owes its argument tuple.
*/
fn validate_closed_name_type(
    syntax: &SyntaxTrees,
    selection: Option<&declaration_selection::ConstantSelection>,
    name: &Identifier,
) -> Result<(), String> {
    let Some(identity) = closed_name_identity(syntax, selection, name) else {
        return Err(format!(
            "requires type equation operand `{}` does not name a closed type",
            name.as_str(syntax),
        ));
    };
    let complete = match identity {
        ClosedArgumentIdentity::Builtin(_) => true,
        ClosedArgumentIdentity::Nominal(declaration) => {
            syntax.root_item(declaration).as_data().is_some_and(|data| {
                data.type_parameters.is_empty() && data.lifetime_parameters.is_empty()
            })
        }
        ClosedArgumentIdentity::RetainedNominal(selected) => selection
            .and_then(|selection| selection.retained_data_header(selected))
            .is_some_and(|data| {
                data.type_parameters.is_empty() && data.lifetime_parameters.is_empty()
            }),
        _ => false,
    };
    if !complete {
        return Err(
            "requires type equation requires a complete constructor argument tuple".to_owned(),
        );
    }
    Ok(())
}

/*
    Synthesis may defer containers it cannot specialize (for example, an
    independently generic attached method). Such a surviving application has
    not discharged the type-reference equation and cannot use the runtime-fact
    exemption. Closed instances lower as Named references; only their retained
    application metadata may still describe the original generic tuple.
*/
pub(crate) fn validate_materialized_type_equations(
    syntax: &SyntaxTrees,
    reference: TypeReferenceHandle,
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Result<(), Diagnostic> {
    let TypeReferenceNode::Generic { base_name, .. } =
        syntax.type_references.type_reference(reference)
    else {
        return Ok(());
    };
    let Some(declaration) = selected_data_item(syntax, selection, base_name) else {
        return Ok(());
    };
    let Some(definition) = syntax.root_item(declaration).as_data() else {
        return Ok(());
    };
    if !template_type_equation_offsets(syntax, definition, selection)?.is_empty() {
        return Err(Diagnostic::error(format!(
            "generic data `{}` retains unsolved type equations; its application requires closed instance specialization",
            definition.name.as_str(syntax),
        )).with_source_span(base_name.source_span()));
    }
    Ok(())
}

#[derive(Clone)]
enum EquationShape {
    /*
        A Boolean condition over exact type equations. Alternatives constrain an
        already determined tuple; they never select one possible type for an
        omitted argument or supply implicit conformance evidence.
    */
    Boolean {
        operator: TypeBooleanOperator,
        operands: Vec<TypeEquation>,
        span: SourceSpan,
    },
    /* `binder == structure` with a `type` binder on the binder side. */
    Structural {
        binder: usize,
        structure: Structure,
        span: SourceSpan,
    },
    /*
        One side names a type constructor or binder, but the other side is
        a value, a const binder, or a value binder: the sides cannot be
        equated as types, and nothing converts a type to a value.
    */
    KindMismatch {
        span: SourceSpan,
        description: String,
    },
}

#[derive(Clone, Copy)]
enum TypeBooleanOperator {
    And,
    Or,
    Not,
}

#[derive(Clone)]
enum Structure {
    /*
        Borrow/singleton-array syntax whose type role follows from the opposite
        declared type binder, not from parser guesses about identifier spelling.
    */
    TypeOperand(ExpressionHandle),
    /*
        Type-role syntax retains recursive constructors instead of rendering them
        as names or treating their operands as runtime array elements.
    */
    TypeReference(TypeReferenceHandle),
    /* `carrier[minimum..maximum]` or `carrier[minimum..=maximum]`. */

    /* A single name: a closed type, or another type binder. */
    Name {
        name: Identifier,
        binder: Option<usize>,
    },
}

impl Structure {
    fn binder_mentions(&self, syntax: &SyntaxTrees, parameters: &[String]) -> Vec<usize> {
        let mut mentions = Vec::new();
        match self {
            Structure::TypeOperand(expression) => {
                collect_expression_binder_mentions(syntax, *expression, parameters, &mut mentions);
            }
            Structure::TypeReference(reference) => {
                collect_binder_mentions(syntax, *reference, parameters, &mut mentions);
            }

            Structure::Name { name, binder } => mentions.extend(binder.or_else(|| {
                parameters
                    .iter()
                    .position(|parameter| parameter == name.as_str(syntax))
            })),
        }
        mentions
    }
}

enum Side {
    AmbiguousTypeOperand(ExpressionHandle),
    TypeStructure(TypeReferenceHandle),
    TypeBinder(usize),
    ValueBinder(usize),
    ClosedName(Identifier),

    Other(&'static str),
}

fn binder_index<'parameters>(
    syntax_trees: &SyntaxTrees,
    parameters: &'parameters [TypeParameter],
    name: &str,
) -> Option<(usize, &'parameters TypeParameterKind)> {
    parameters
        .iter()
        .position(|parameter| parameter.name.as_str(syntax_trees) == name)
        .map(|index| (index, &parameters[index].kind))
}

/*
    Classify every declaration-level `requires` fact that equates a type binder or a
    type constructor. Facts over const binders and values alone (`Capacity > 0`)
    stay ordinary instantiation obligations for the const evaluator.
    A machine's structural equations among its `requires` facts; see
    [`TypeEquation::defines_structure`].
*/
pub(crate) fn classify_machine_equations(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    lifetimes: &[Identifier],
    requirements: &[ProofFact],
) -> Vec<TypeEquation> {
    let binders = parameters
        .iter()
        .map(|parameter| &parameter.name)
        .chain(lifetimes)
        .map(|name| name.as_str(syntax).to_owned())
        .collect::<Vec<_>>();
    classify_type_equations(syntax, parameters, requirements)
        .into_iter()
        .filter(|equation| equation.defines_structure(syntax, &binders))
        .collect()
}

pub(crate) fn classify_type_equations(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    requires_facts: &[ProofFact],
) -> Vec<TypeEquation> {
    requires_facts
        .iter()
        .enumerate()
        .filter_map(|(fact_offset, fact)| {
            let ProofFact::Expression(expression) = fact else {
                return None;
            };
            let nested = |expression| {
                classify_type_equations(syntax, parameters, &[ProofFact::Expression(expression)])
                    .into_iter()
                    .next()
            };
            let span = syntax.expressions.source_span(*expression);
            let boolean = match syntax.expressions.expression(*expression) {
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
                {
                    Some((
                        if binary.operator == BinaryOperator::And {
                            TypeBooleanOperator::And
                        } else {
                            TypeBooleanOperator::Or
                        },
                        vec![nested(binary.left)?, nested(binary.right)?],
                    ))
                }
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    Some((TypeBooleanOperator::Not, vec![nested(unary.operand)?]))
                }
                _ => None,
            };
            if let Some((operator, operands)) = boolean {
                return Some(TypeEquation {
                    fact_offset,
                    definition: *expression,
                    shape: EquationShape::Boolean {
                        operator,
                        operands,
                        span,
                    },
                });
            }
            let ExpressionNode::Binary(binary) = syntax.expressions.expression(*expression) else {
                return None;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                return None;
            }
            let left = classify_side(syntax, parameters, binary.left);
            let right = classify_side(syntax, parameters, binary.right);
            let definition = if matches!(left, Side::TypeBinder(_)) {
                binary.right
            } else {
                binary.left
            };
            let shape = match (left, right) {
                (Side::TypeBinder(binder), other) | (other, Side::TypeBinder(binder)) => {
                    equation_shape(syntax, parameters, binder, other, span)
                }

                (Side::TypeStructure(_), other) | (other, Side::TypeStructure(_)) => {
                    EquationShape::KindMismatch {
                        span,
                        description: format!(
                            "{} cannot equal a type structure",
                            describe_side(syntax, parameters, &other)
                        ),
                    }
                }
                _ => return None,
            };
            let equation = TypeEquation {
                fact_offset,
                definition,
                shape,
            };
            Some(if binary.operator == BinaryOperator::NotEqual {
                TypeEquation {
                    fact_offset,
                    definition: *expression,
                    shape: EquationShape::Boolean {
                        operator: TypeBooleanOperator::Not,
                        operands: vec![equation],
                        span,
                    },
                }
            } else {
                equation
            })
        })
        .collect()
}

fn classify_side(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> Side {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Borrow(_) | ExpressionNode::ArrayLiteral(_) => {
            Side::AmbiguousTypeOperand(expression)
        }
        ExpressionNode::TypeExpression(reference) => Side::TypeStructure(*reference),
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            let members = syntax.expressions.identifier_path_members(path.to_owned());
            if let [member] = members
                && let Some((index, kind)) = binder_index(syntax, parameters, member.as_str(syntax))
            {
                return if matches!(kind, TypeParameterKind::Type) {
                    Side::TypeBinder(index)
                } else {
                    Side::ValueBinder(index)
                };
            }
            let Some(first) = members.first() else {
                return Side::Other("an absent name");
            };
            let mut span = first.source_span();
            span.span.end = members
                .last()
                .expect("name has a first member")
                .source_span()
                .span
                .end;
            Side::ClosedName(Identifier::new(members.joined_path_in(syntax), span))
        }
        _ => Side::Other("a value expression"),
    }
}

fn describe_side(syntax_trees: &SyntaxTrees, parameters: &[TypeParameter], side: &Side) -> String {
    match side {
        Side::AmbiguousTypeOperand(_) => "a reference or slice operand".to_owned(),
        Side::TypeStructure(_) => "a type structure".to_owned(),
        Side::TypeBinder(index) => format!(
            "type binder `{}`",
            parameters[*index].name.as_str(syntax_trees)
        ),
        Side::ValueBinder(index) => {
            format!(
                "value binder `{}`",
                parameters[*index].name.as_str(syntax_trees)
            )
        }
        Side::ClosedName(name) => format!("`{}`", name.as_str(syntax_trees)),

        Side::Other(what) => (*what).to_owned(),
    }
}

fn equation_shape(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    binder: usize,
    other: Side,
    span: SourceSpan,
) -> EquationShape {
    let binder_name = parameters[binder].name.as_str(syntax);
    let mismatch = |description: String| EquationShape::KindMismatch { span, description };
    let structure = match other {
        Side::AmbiguousTypeOperand(expression) => {
            if let Some(reason) = type_operand_kind_error(syntax, parameters, expression) {
                return mismatch(reason.to_owned());
            }
            Structure::TypeOperand(expression)
        }
        Side::TypeStructure(reference) => Structure::TypeReference(reference),
        Side::TypeBinder(index) => Structure::Name {
            name: parameters[index].name.clone(),
            binder: Some(index),
        },
        Side::ClosedName(name) => Structure::Name { name, binder: None },
        Side::ValueBinder(index) => {
            return mismatch(format!(
                "type binder `{binder_name}` cannot equal value binder `{}`",
                parameters[index].name.as_str(syntax)
            ));
        }
        Side::Other(what) => {
            return mismatch(format!("type binder `{binder_name}` cannot equal {what}"));
        }
    };
    EquationShape::Structural {
        binder,
        structure,
        span,
    }
}

/*
    Resolve the operand's role only after the opposite side selected a declared
    type binder. Ordinary borrow/array equalities remain value propositions.
*/
fn type_operand_kind_error(
    syntax: &SyntaxTrees,
    parameters: &[TypeParameter],
    expression: ExpressionHandle,
) -> Option<&'static str> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } if path_application.is_none() => {
            if let [name] = syntax.expressions.identifier_path_members(path.to_owned())
                && binder_index(syntax, parameters, name.as_str(syntax))
                    .is_some_and(|(_, kind)| !matches!(kind, TypeParameterKind::Type))
            {
                return Some("a value binder cannot supply a reference or slice element type");
            }
            None
        }
        ExpressionNode::Borrow(borrow) => {
            type_operand_kind_error(syntax, parameters, borrow.target)
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let [element] = syntax.expressions.expression_handles(*elements) else {
                return Some("a slice type requires exactly one element type");
            };
            type_operand_kind_error(syntax, parameters, *element)
        }
        ExpressionNode::TypeExpression(_) => None,
        _ => Some("a value expression cannot supply a reference or slice element type"),
    }
}

fn collect_expression_binder_mentions(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    parameters: &[String],
    mentions: &mut Vec<usize>,
) {
    if !expression.is_valid() {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            collect_expression_binder_mentions(syntax, borrow.target, parameters, mentions);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in syntax.expressions.expression_handles(*elements) {
                collect_expression_binder_mentions(syntax, *element, parameters, mentions);
            }
        }
        ExpressionNode::TypeExpression(reference) => {
            collect_binder_mentions(syntax, *reference, parameters, mentions);
        }
        ExpressionNode::Name {
            members: path,
            application: path_application,
        } => {
            if let Some(application) = &path_application {
                for argument in &application.arguments {
                    collect_static_argument_binder_mentions(syntax, argument, parameters, mentions);
                }
                if let Some(member) = syntax.expressions.static_member(application.member) {
                    collect_static_argument_binder_mentions(syntax, member, parameters, mentions);
                }
            }
            if let [member] = syntax.expressions.identifier_path_members(path.to_owned())
                && let Some(index) = parameters
                    .iter()
                    .position(|parameter| parameter == member.as_str(syntax))
            {
                mentions.push(index);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_binder_mentions(syntax, binary.left, parameters, mentions);
            collect_expression_binder_mentions(syntax, binary.right, parameters, mentions);
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_binder_mentions(syntax, unary.operand, parameters, mentions);
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_binder_mentions(syntax, indexed.collection, parameters, mentions);
            collect_expression_binder_mentions(syntax, indexed.index, parameters, mentions);
        }
        ExpressionNode::Range(range) => {
            collect_expression_binder_mentions(syntax, range.start, parameters, mentions);
            collect_expression_binder_mentions(syntax, range.end, parameters, mentions);
        }
        ExpressionNode::Member(member) => {
            collect_expression_binder_mentions(syntax, member.receiver, parameters, mentions);
        }
        ExpressionNode::Call(call) => {
            collect_expression_binder_mentions(syntax, call.receiver, parameters, mentions);
            if let Some(index) = parameters
                .iter()
                .position(|parameter| parameter == call.target.as_str(syntax))
            {
                mentions.push(index);
            }
            for argument in &call.machine_arguments {
                collect_static_argument_binder_mentions(syntax, argument, parameters, mentions);
            }
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_expression_binder_mentions(syntax, *argument, parameters, mentions);
            }
        }
        _ => {}
    }
}

fn collect_static_argument_binder_mentions(
    syntax: &SyntaxTrees,
    argument: &syntax_trees::expression::StaticMachineArgument,
    parameters: &[String],
    mentions: &mut Vec<usize>,
) {
    if argument.type_reference.is_valid() {
        collect_binder_mentions(syntax, argument.type_reference, parameters, mentions);
    }
    let name = match (&*argument.path, &argument.evidence_projection) {
        ([name], _) => Some(name),
        (_, Some(projection)) => Some(&projection.term),
        _ => None,
    };
    if let Some(name) = name
        && let Some(index) = parameters
            .iter()
            .position(|parameter| parameter == name.as_str(syntax))
    {
        mentions.push(index);
    }
    if let Some(application) = &argument.application {
        for argument in &application.arguments {
            collect_static_argument_binder_mentions(syntax, argument, parameters, mentions);
        }
        if let Some(member) = syntax.expressions.static_member(application.member) {
            collect_static_argument_binder_mentions(syntax, member, parameters, mentions);
        }
    }
}

/*
    Declaration-neutral inputs for structural equation completion. Data and
    machine applications retain their own selection and publication owners.
*/
#[derive(Clone, Copy)]
pub(crate) struct EquationTemplate<'template> {
    pub(crate) kind: &'static str,
    pub(crate) name: &'template str,
    pub(crate) parameters: arena::HandleSpan<TypeParameter>,
    /* Fresh invocation regions, separate from runtime generic arguments. */
    pub(crate) invocation_lifetimes: &'template [Identifier],
    pub(crate) parameter_names: &'template [String],
    pub(crate) const_parameter_types: &'template [Option<TypeReferenceHandle>],
    pub(crate) type_equations: &'template [TypeEquation],
}

pub(crate) fn complete_equation_arguments(
    syntax: &mut SyntaxTrees,
    base_info: EquationTemplate<'_>,
    base_name: &Identifier,
    supplied: &[TypeReferenceHandle],
    selection: Option<&declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
    complete_equation_arguments_with_types(
        syntax,
        base_info,
        base_name,
        supplied,
        &[],
        selection,
        warnings,
    )
}

/*
    Ordinary call types and declaration equations share one substitution. The
    exact formal/actual observations come from selected declarations, never
    runtime elements or compatible value bounds. Explicit arguments are fixed
    before compatibility; only patterns mentioning an omitted binder contribute
    inference, and every such occurrence is checked even after it is bound.
    Canonical generics.md (e3d20c0c1f44d36ac6221811e46446975bea57e8):
    "All applicable occurrences must be consistent, not just the first one visited."
    "Inference selects arguments before checking ordinary compatibility."
*/
pub(crate) fn complete_equation_arguments_with_types(
    syntax: &mut SyntaxTrees,
    base_info: EquationTemplate<'_>,
    base_name: &Identifier,
    supplied: &[TypeReferenceHandle],
    ordinary_types: &[(TypeReferenceHandle, TypeReferenceHandle)],
    selection: Option<&declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
    let bindings = {
        let mut solver = Solver::new(&mut *syntax, base_info, base_name, supplied, selection);
        for &(formal, actual) in ordinary_types {
            let mut mentions = Vec::new();
            collect_binder_mentions(
                solver.syntax,
                formal,
                base_info.parameter_names,
                &mut mentions,
            );
            if mentions.iter().any(|index| *index >= supplied.len()) {
                solver.match_type_structure(formal, actual, base_name.source_span())?;
            }
        }
        solver.solve(warnings)?;
        solver.bindings
    };
    let mut tuple = supplied.to_vec();
    for binding in bindings.into_iter().skip(supplied.len()) {
        let node = match binding {
            Some(Binding::Type(handle)) => {
                tuple.push(handle);
                continue;
            }
            Some(Binding::NamedType(name)) => {
                TypeReferenceNode::Named(Identifier::new(name.as_str(syntax), name.source_span()))
            }
            Some(Binding::Integer(value)) => {
                TypeReferenceNode::Named(Identifier::generated(value.to_string()))
            }
            Some(Binding::Boolean(value)) => TypeReferenceNode::Named(Identifier::generated(
                CanonicalConstValue::boolean(value).atom(),
            )),
            Some(Binding::Constant(reference)) => {
                tuple.push(reference);
                continue;
            }
            None => {
                unreachable!("solving binds every omitted binder or rejects")
            }
        };
        tuple.push(syntax.tables.type_references.insert(node));
    }
    Ok(tuple)
}

/*
    Instantiate a selected signature from its already completed argument tuple.
    This is the same constructor traversal used to synthesize an equation's
    omitted type, so composing a generic call cannot erase index/carrier identity
    or reconstruct a type from the callee's implementation or runtime contents.
*/
pub(crate) fn materialize_completed_type(
    syntax: &mut SyntaxTrees,
    base_info: EquationTemplate<'_>,
    base_name: &Identifier,
    arguments: &[TypeReferenceHandle],
    result: TypeReferenceHandle,
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Result<TypeReferenceHandle, Diagnostic> {
    let mut solver = Solver::new(syntax, base_info, base_name, arguments, selection);
    solver
        .construct_type_structure(result, base_name.source_span())?
        .ok_or_else(|| {
            solver.reject("retains an undetermined result type after argument inference".to_owned())
        })
}

#[derive(Clone)]
enum Binding {
    /* A supplied or forwarded type argument. */
    Type(TypeReferenceHandle),
    /*
        A type binder recovered from a closed name equation; materialized as a
        fresh `Named` leaf once solving completes.
    */
    NamedType(Identifier),
    Integer(BigInt),
    /* Boolean index identity never participates in integer endpoint arithmetic. */
    Boolean(bool),
    /*
        A canonical non-scalar index retains its original reference, including
        declaration/expression custody. Its identity never enters endpoint arithmetic.
    */
    Constant(TypeReferenceHandle),
}

enum Outcome {
    Settled,
    Deferred,
}

struct Solver<'a, 's> {
    /* Constructing an omitted type argument appends its ordinary type tree. */
    syntax: &'a mut SyntaxTrees,
    base_info: EquationTemplate<'a>,
    base_name: &'a Identifier,
    selection: Option<&'a declaration_selection::ConstantSelection<'s>>,
    supplied_count: usize,
    bindings: Vec<Option<Binding>>,
    settled: Vec<bool>,
}

impl<'a, 's> Solver<'a, 's> {
    fn new(
        syntax: &'a mut SyntaxTrees,
        base_info: EquationTemplate<'a>,
        base_name: &'a Identifier,
        supplied: &[TypeReferenceHandle],
        selection: Option<&'a declaration_selection::ConstantSelection<'s>>,
    ) -> Self {
        let mut bindings = vec![None; base_info.parameter_names.len()];
        for (index, argument) in supplied.iter().enumerate() {
            bindings[index] = Some(if base_info.const_parameter_types[index].is_some() {
                match syntax.tables.type_references.type_reference(*argument) {
                    TypeReferenceNode::Named(value) => {
                        if let Some(value) = normalized_boolean_argument(value.as_str(syntax)) {
                            Binding::Boolean(value)
                        } else {
                            value
                                .as_str(syntax)
                                .parse::<i128>()
                                .map_or(Binding::Constant(*argument), |value| {
                                    Binding::Integer(BigInt::from_i128(value))
                                })
                        }
                    }
                    _ => Binding::Constant(*argument),
                }
            } else {
                Binding::Type(*argument)
            });
        }
        Self {
            syntax,
            base_info,
            base_name,
            selection,
            supplied_count: supplied.len(),
            bindings,
            settled: vec![false; base_info.type_equations.len()],
        }
    }

    fn reject(&self, message: String) -> Diagnostic {
        Diagnostic::error(format!(
            "generic {} `{}` {message}",
            self.base_info.kind, self.base_info.name
        ))
        .with_source_span(self.base_name.source_span())
    }

    fn binder_name(&self, index: usize) -> &str {
        self.base_info.parameter_names[index].as_str()
    }

    fn solve(&mut self, warnings: &mut Vec<Diagnostic>) -> Result<(), Diagnostic> {
        loop {
            let mut progress = false;
            for index in 0..self.base_info.type_equations.len() {
                if self.settled[index] {
                    continue;
                }
                let equation = self.base_info.type_equations[index].clone();
                if let Outcome::Settled = self.apply(&equation, warnings)? {
                    self.settled[index] = true;
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        if let Some(index) = self.bindings.iter().position(Option::is_none) {
            let binder = self.binder_name(index);
            if self.defined_through_cycle(index) {
                return Err(self.reject(format!(
                    "requires equations define `{binder}` through itself; supply the argument explicitly"
                )));
            }
            return Err(self.reject(format!(
                "cannot determine `{binder}` from its requires equations; supply the argument explicitly"
            )));
        }
        if let Some(index) = self.settled.iter().position(|settled| !settled) {
            if let EquationShape::Boolean { span, .. } = &self.base_info.type_equations[index].shape
            {
                return Err(self
                    .reject(
                        "requires type predicate cannot be decided from the supplied arguments"
                            .to_owned(),
                    )
                    .with_source_span(*span));
            }
            let EquationShape::Structural { binder, span, .. } =
                &self.base_info.type_equations[index].shape
            else {
                unreachable!("kind mismatches reject when applied");
            };
            return Err(self
                .reject(format!(
                    "requires equation on `{}` cannot be decided from the supplied arguments",
                    self.binder_name(*binder)
                ))
                .with_source_span(*span));
        }
        Ok(())
    }

    fn equations_defining(&self, binder: usize) -> impl Iterator<Item = &Structure> {
        self.base_info
            .type_equations
            .iter()
            .filter_map(move |equation| match &equation.shape {
                EquationShape::Structural {
                    binder: defined,
                    structure,
                    ..
                } if *defined == binder => Some(structure),
                _ => None,
            })
    }

    /*
        Whether following `binder == structure` edges from `start` returns to
        `start`: the binder is defined through itself.
    */
    fn defined_through_cycle(&self, start: usize) -> bool {
        let mut stack = vec![start];
        let mut visited = Vec::new();
        while let Some(node) = stack.pop() {
            for structure in self.equations_defining(node) {
                for mention in
                    structure.binder_mentions(self.syntax, self.base_info.parameter_names)
                {
                    if mention == start {
                        return true;
                    }
                    if !visited.contains(&mention) {
                        visited.push(mention);
                        stack.push(mention);
                    }
                }
            }
        }
        false
    }

    fn apply(
        &mut self,
        equation: &TypeEquation,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        match &equation.shape {
            EquationShape::Boolean {
                operator,
                operands,
                span,
            } => {
                equation.validate_names(self.syntax, self.selection)?;
                if matches!(operator, TypeBooleanOperator::And) {
                    let mut settled = true;
                    for operand in operands {
                        settled &= matches!(self.apply(operand, warnings)?, Outcome::Settled);
                    }
                    return Ok(if settled {
                        Outcome::Settled
                    } else {
                        Outcome::Deferred
                    });
                }
                match self.predicate_value(equation, warnings)? {
                    Some(true) => Ok(Outcome::Settled),
                    Some(false) => Err(self
                        .reject(
                            "requires type predicate does not hold for the supplied arguments"
                                .to_owned(),
                        )
                        .with_source_span(*span)),
                    None => Ok(Outcome::Deferred),
                }
            }
            EquationShape::KindMismatch { span, description } => Err(self
                .reject(format!(
                    "requires equation mixes type and value kinds: {description}"
                ))
                .with_source_span(*span)),
            EquationShape::Structural {
                binder,
                structure,
                span,
            } => {
                if let Structure::Name { name, binder: None } = structure {
                    validate_closed_name_type(self.syntax, self.selection, name)
                        .map_err(|reason| self.reject(reason).with_source_span(*span))?;
                }
                if structure
                    .binder_mentions(self.syntax, self.base_info.parameter_names)
                    .contains(binder)
                {
                    return Err(self
                        .reject(format!(
                            "requires equations define `{}` through itself; supply the argument explicitly",
                            self.binder_name(*binder)
                        ))
                        .with_source_span(*span));
                }
                match self.bindings[*binder].clone() {
                    Some(Binding::Type(handle)) => {
                        self.match_structure(*binder, handle, structure, *span, warnings)
                    }
                    Some(Binding::NamedType(name)) => {
                        let Structure::Name {
                            name: other,
                            binder: other_binder,
                        } = structure
                        else {
                            return Err(self
                                .reject(format!(
                                    "cannot construct type binder `{}` from its requires equation; supply the argument explicitly",
                                    self.binder_name(*binder)
                                ))
                                .with_source_span(*span));
                        };
                        let expected = closed_name_identity(self.syntax, self.selection, &name);
                        self.match_name(*binder, expected, other, *other_binder, *span)
                    }
                    Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => Err(self
                        .reject(format!(
                            "requires equation mixes type and value kinds: type binder `{}` received a value argument",
                            self.binder_name(*binder)
                        ))
                        .with_source_span(*span)),
                    None => self.recover_binder(*binder, structure, *span, warnings),
                }
            }
        }
    }

    /*
        Decide a condition without using an alternative as inference. Reuse the
        ordinary structural constructor, restoring the original binder before
        observing either outcome. Every leaf is formed, including on an arm
        whose Boolean value is already determined by its sibling.
    */
    fn predicate_value(
        &mut self,
        equation: &TypeEquation,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Option<bool>, Diagnostic> {
        match &equation.shape {
            EquationShape::Boolean {
                operator, operands, ..
            } => {
                let values = operands
                    .iter()
                    .map(|operand| self.predicate_value(operand, warnings))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(match (*operator, values.as_slice()) {
                    (TypeBooleanOperator::Not, [Some(value)]) => Some(!value),
                    (TypeBooleanOperator::And, [Some(left), Some(right)]) => Some(*left && *right),
                    (TypeBooleanOperator::Or, [Some(left), Some(right)]) => Some(*left || *right),
                    _ => None,
                })
            }
            EquationShape::Structural {
                binder,
                structure,
                span,
            } => {
                let actual = self.bindings[*binder].clone();
                let formed = self.recover_binder(*binder, structure, *span, warnings);
                let expected = self.bindings[*binder].clone();
                self.bindings[*binder] = actual.clone();
                if matches!(formed?, Outcome::Deferred) {
                    return Ok(None);
                }
                let Some(actual) = self.predicate_binding_identity(actual.as_ref(), *span)? else {
                    return Ok(None);
                };
                let Some(expected) = self.predicate_binding_identity(expected.as_ref(), *span)?
                else {
                    return Ok(None);
                };
                Ok(Some(actual == expected))
            }
            EquationShape::KindMismatch { .. } => {
                equation.validate_kind("declaration")?;
                unreachable!("kind mismatch must refuse")
            }
        }
    }

    fn predicate_binding_identity(
        &self,
        binding: Option<&Binding>,
        span: SourceSpan,
    ) -> Result<Option<ClosedArgumentIdentity>, Diagnostic> {
        let identity = match binding {
            None => return Ok(None),
            Some(Binding::Type(reference)) => {
                closed_argument_identity(self.syntax, self.selection, *reference, false)
            }
            Some(Binding::NamedType(name)) => {
                validate_closed_name_type(self.syntax, self.selection, name)
                    .map_err(|reason| self.reject(reason).with_source_span(span))?;
                closed_name_identity(self.syntax, self.selection, name)
            }
            Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => {
                return Err(self
                    .reject("requires type predicate received a value operand".to_owned())
                    .with_source_span(span));
            }
        };
        identity.map(Some).ok_or_else(|| {
            self.reject(
                "requires type predicate operand has no complete exact type identity".to_owned(),
            )
            .with_source_span(span)
        })
    }

    /*
        The binder side is unbound: a closed name, an already bound type
        binder, or the equation's own type structure defines it.
    */
    fn recover_binder(
        &mut self,
        binder: usize,
        structure: &Structure,
        span: SourceSpan,
        _warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        let (name, other) = match structure {
            Structure::TypeOperand(expression) => {
                let pattern = self.type_operand_reference(*expression, span)?;
                let Some(reference) = self.construct_type_structure(pattern, span)? else {
                    return Ok(Outcome::Deferred);
                };
                self.bindings[binder] = Some(Binding::Type(reference));
                return Ok(Outcome::Settled);
            }
            Structure::TypeReference(reference) => {
                let Some(reference) = self.construct_type_structure(*reference, span)? else {
                    return Ok(Outcome::Deferred);
                };
                self.bindings[binder] = Some(Binding::Type(reference));
                return Ok(Outcome::Settled);
            }
            Structure::Name {
                name,
                binder: other,
            } => (name, other),
        };
        let binding = match other {
            None => Binding::NamedType(name.clone()),
            Some(other) => match self.bindings[*other].clone() {
                Some(binding @ (Binding::Type(_) | Binding::NamedType(_))) => binding,
                Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => {
                    unreachable!("a type binder holds a type binding")
                }
                None => return Ok(Outcome::Deferred),
            },
        };
        self.bindings[binder] = Some(binding);
        Ok(Outcome::Settled)
    }

    fn match_structure(
        &mut self,
        binder: usize,
        handle: TypeReferenceHandle,
        structure: &Structure,
        span: SourceSpan,
        _warnings: &mut Vec<Diagnostic>,
    ) -> Result<Outcome, Diagnostic> {
        match structure {
            Structure::TypeOperand(expression) => {
                let pattern = self.type_operand_reference(*expression, span)?;
                self.match_type_structure(pattern, handle, span)?;
                Ok(Outcome::Settled)
            }
            Structure::TypeReference(reference) => {
                self.match_type_structure(*reference, handle, span)?;
                Ok(Outcome::Settled)
            }
            Structure::Name {
                name,
                binder: other,
            } => {
                self.require_lifetime_free_argument(handle, span)?;
                let actual = closed_argument_identity(self.syntax, self.selection, handle, false);
                self.match_name(binder, actual, name, *other, span)
            }
        }
    }

    fn match_name(
        &mut self,
        binder: usize,
        actual: Option<ClosedArgumentIdentity>,
        name: &Identifier,
        other: Option<usize>,
        span: SourceSpan,
    ) -> Result<Outcome, Diagnostic> {
        let binder_name = self.binder_name(binder).to_owned();
        let expected = match other {
            None => closed_name_identity(self.syntax, self.selection, name),
            Some(index) => match self.bindings[index].clone() {
                None => {
                    self.bindings[index] = Some(match self.bindings[binder].clone() {
                        Some(binding) => binding,
                        None => unreachable!("the binder side is bound before matching"),
                    });
                    return Ok(Outcome::Settled);
                }
                Some(Binding::Type(bound)) => {
                    self.require_lifetime_free_argument(bound, span)?;
                    closed_argument_identity(self.syntax, self.selection, bound, false)
                }
                Some(Binding::NamedType(bound)) => {
                    closed_name_identity(self.syntax, self.selection, &bound)
                }
                Some(Binding::Integer(_) | Binding::Boolean(_) | Binding::Constant(_)) => {
                    unreachable!("a type binder holds a type binding")
                }
            },
        };
        match (expected, actual) {
            (Some(expected), Some(actual)) if expected == actual => Ok(Outcome::Settled),
            (Some(_), Some(_)) => Err(self
                .reject(format!(
                    "requires equation on `{binder_name}` does not hold: the supplied `{binder_name}` argument is not `{}`",
                    name.as_str(self.syntax)
                ))
                .with_source_span(span)),
            _ => Err(self
                .reject(format!(
                    "requires equation on `{binder_name}` cannot be decided: `{}` or the supplied `{binder_name}` argument does not name a closed type",
                    name.as_str(self.syntax)
                ))
                .with_source_span(span)),
        }
    }
    fn bind_integer(
        &mut self,
        index: usize,
        value: BigInt,
        span: SourceSpan,
    ) -> Result<(), Diagnostic> {
        let name = self.binder_name(index).to_owned();
        match &self.bindings[index] {
            Some(Binding::Integer(existing)) if *existing == value => Ok(()),
            Some(Binding::Integer(existing)) => Err(self
                .reject(if index < self.supplied_count {
                    format!(
                        "requires equation binds `{name}` to {value} but its explicit argument is {existing}"
                    )
                } else {
                    format!("requires equations bind `{name}` to both {existing} and {value}")
                })
                .with_source_span(span)),
            Some(Binding::Boolean(_) | Binding::Constant(_)) => Err(self
                .reject(format!(
                    "requires equation mixes type and value kinds: const binder `{name}` holds a non-integer argument"
                ))
                .with_source_span(span)),
            Some(Binding::Type(_) | Binding::NamedType(_)) => {
                unreachable!("classification admits only value binders as endpoints")
            }
            None => {
                let Some(parameter_type) = self.base_info.const_parameter_types[index] else {
                    return Err(self
                        .reject(format!(
                            "requires equation cannot recover `{name}` statically: it is a runtime-capable value binder, not a `const` binder"
                        ))
                        .with_source_span(span));
                };
                let Some(type_name) = self
                    .syntax
                    .tables
                    .type_references
                    .type_reference(parameter_type).as_named() else {
                    return Err(self
                        .reject(format!(
                            "requires equation mixes type and value kinds: const binder `{name}` is not a builtin integer"
                        ))
                        .with_source_span(span));
                };
                let Some((minimum, maximum)) = const_binder_envelope(type_name.as_str(self.syntax)) else {
                    return Err(self
                        .reject(format!(
                            "requires equation mixes type and value kinds: const binder `{name}` of type `{}` cannot hold a range endpoint",
                            type_name.as_str(self.syntax)
                        ))
                        .with_source_span(span));
                };
                if value < minimum || value > maximum {
                    return Err(self
                        .reject(format!(
                            "requires equation binds `{name}` to {value}, outside its declared `{}`",
                            type_name.as_str(self.syntax)
                        ))
                        .with_source_span(span));
                }
                self.bindings[index] = Some(Binding::Integer(value));
                Ok(())
            }
        }
    }
}

/*
    Only already closed Boolean spelling or the shared canonical value atom.
    Constructor matching does not evaluate expressions or reinterpret integers.
*/
pub(super) fn normalized_boolean_argument(text: &str) -> Option<bool> {
    if let Some(value) = boolean_literal_spelling(text) {
        return Some(value);
    }
    let value = CanonicalConstValue::from_atom(text)?;
    if value.type_name != "bool" {
        return None;
    }
    match value.decode_encoding()? {
        DecodedCanonicalConstValue::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn const_binder_envelope(type_name: &str) -> Option<(BigInt, BigInt)> {
    let (minimum, maximum) = LandedIntegerType::from_name(type_name)?.bounds()?;
    Some((BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
}

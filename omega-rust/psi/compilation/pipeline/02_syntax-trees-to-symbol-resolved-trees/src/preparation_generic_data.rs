/*
    Pre-resolution synthesis of closed generic data inside the name-resolution owner.

    These routines consume syntax and return syntax. Discovered templates,
    substitutions and pending instances are private working state. Durable
    instance declarations remain in SyntaxTrees, not a second program schema.
    Exact declaration lookup comes from the shared
    `preparation::declaration_selection` owner.

    Closed declaration specialization is demanded by authored applications
    and inline storage. Reference identity alone does not demand a recursively
    growing catalog, even for const-bearing or constrained declarations. Closed
    argument preparation remains independent of declaration copying. Data facts
    define default domains; impossible domains are legal to name, not to inhabit
    without establishing their predicates. Typed preparation elaborates field
    views without reopening the frozen program during checking.
*/

use crate::preparation::declaration_selection;
#[cfg(test)]
#[path = "preparation_const_operands_tests.rs"]
mod const_operands_tests;
use crate::preparation::generic_data::uses::concrete_machine_expression_handles;
use crate::tables::syntax_expressions;
use crate::tables::syntax_type_references;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::const_value::CanonicalConstValue;
use numerics::literals::IntegerLiteral;
use std::collections::{HashMap, HashSet};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::item::{
    ConstDefinition, DataDefinition, DataMember, Item, ProofFact, TypeParameter, TypeParameterKind,
};
use syntax_trees::statement::StatementNode;
use syntax_trees::types::{
    FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

mod arguments {
    /*
        Closed argument identities and lifetime-bearing type substitution.
        Closed array extents share the declared-constant reader with structural
        matching. Observe geometry for identity and display without rewriting the
        authored type: receiving resolution still owns its selected extent origin.
    */
    use crate::preparation::declaration_selection;
    use crate::preparation::declaration_selection::GenericApplicationSubstitution;
    use crate::preparation::generic_data::ClosedArgumentIdentity;
    use crate::preparation::generic_data::ClosedConstraintIdentity;
    use crate::preparation::generic_data::GenericData;
    use crate::preparation::generic_data::selected_data_item;
    use crate::preparation::type_equations::{EquationTemplate, complete_equation_arguments};
    use crate::symbols::builtin_type_atom_by_symbol_name;
    use crate::tables::syntax_type_references;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::DomainDefinition;
    use syntax_trees::item::Item;
    use syntax_trees::item::ItemHandle;
    use syntax_trees::item::TypeParameter;
    use syntax_trees::item::TypeParameterKind;
    use syntax_trees::types::DomainConstraint;
    use syntax_trees::types::FixedArrayLength;
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;

    /*
        Recover every omitted trailing binder of `base_info` from its type
        equations and verify the equations against the supplied prefix. The
        returned tuple is complete; recovered const binders are the canonical
        decimal `Named` leaves literal const arguments already use.
    */
    pub(super) fn complete_argument_tuple(
        syntax: &mut SyntaxTrees,
        base_info: &GenericData,
        base_name: &Identifier,
        supplied: &[TypeReferenceHandle],
        selection: Option<&declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
        let Some(definition) = syntax.root_item(base_info.declaration).as_data() else {
            return Err(Diagnostic::error(
                "structural data equation lost its declaration",
            ));
        };
        let template = EquationTemplate {
            kind: "data",
            name: &base_info.name,
            parameters: definition.type_parameters,
            invocation_lifetimes: &[],
            parameter_names: &base_info.parameter_names,
            const_parameter_types: &base_info.const_parameter_types,
            type_equations: &base_info.type_equations,
        };
        complete_equation_arguments(syntax, template, base_name, supplied, selection, warnings)
    }

    /*
        Diagnostic names for admitted closed arguments, not application identity.
        Range shells require structured observations from typed numeric evaluation;
        this syntax owner never evaluates their bounds. Substitution retains the
        original argument type and its constraints. Instance sharing uses the exact
        declaration/argument identities below, not matching rendered names.
    */
    pub(crate) fn monomorphizable_argument_slugs(
        syntax: &SyntaxTrees,
        argument_handles: &[TypeReferenceHandle],
        selection: Option<&declaration_selection::ConstantSelection>,
    ) -> Option<Vec<String>> {
        argument_handles
            .iter()
            .map(|&argument| type_reference_slug(syntax, argument, selection))
            .collect()
    }

    /*
        Rebind erased lifetimes carried by an already-synthesized local instance
        from one concrete outer use to the outer template's own binder roster.

        This first exact cohort is deliberately positional: the nested instance
        must forward the complete outer lifetime application in the same order.
        That preserves one stable synthesized definition across differently named
        use-site lifetimes without inventing binders or choosing an alias/routing
        policy. Broader permutations remain on the unnormalized path.
    */
    pub(super) fn canonicalize_monomorphizable_argument_handles(
        syntax: &mut SyntaxTrees,
        base_info: &GenericData,
        outer_lifetime_arguments: &[Identifier],
        argument_handles: &[TypeReferenceHandle],
    ) -> Option<Vec<TypeReferenceHandle>> {
        base_info
            .const_parameter_types
            .iter()
            .zip(argument_handles)
            .map(|(const_parameter_type, argument)| {
                if const_parameter_type.is_some() {
                    Some(*argument)
                } else {
                    canonicalize_lifetime_bearing_type_argument(
                        syntax,
                        *argument,
                        &base_info.lifetime_parameters,
                        outer_lifetime_arguments,
                    )
                }
            })
            .collect()
    }

    pub(crate) fn canonicalize_lifetime_bearing_type_argument(
        syntax: &mut SyntaxTrees,
        type_reference: TypeReferenceHandle,
        outer_lifetime_parameters: &[Identifier],
        outer_lifetime_arguments: &[Identifier],
    ) -> Option<TypeReferenceHandle> {
        let node = syntax_type_references(syntax)
            .type_reference(type_reference)
            .clone();
        match node {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            } => {
                let referee = canonicalize_lifetime_bearing_type_argument(
                    syntax,
                    referee,
                    outer_lifetime_parameters,
                    outer_lifetime_arguments,
                )?;
                Some(syntax.type_references.insert_reference(referee, access))
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = canonicalize_lifetime_bearing_type_argument(
                    syntax,
                    element_type,
                    outer_lifetime_parameters,
                    outer_lifetime_arguments,
                )?;
                Some(
                    syntax
                        .type_references
                        .insert(TypeReferenceNode::Slice { element_type }),
                )
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } if !lifetime_arguments.is_empty()
                && syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .is_empty()
                && exact_synthesized_lifetime_instance(
                    syntax,
                    base_name.as_str(syntax),
                    lifetime_arguments.len(),
                ) =>
            {
                if outer_lifetime_parameters.len() != outer_lifetime_arguments.len()
                    || lifetime_arguments.len() != outer_lifetime_arguments.len()
                    || !lifetime_arguments
                        .iter()
                        .zip(outer_lifetime_arguments)
                        .all(|(nested, outer)| nested.as_str(syntax) == outer.as_str(syntax))
                {
                    return None;
                }
                Some(
                    syntax
                        .tables
                        .type_references
                        .insert(TypeReferenceNode::Generic {
                            base_name,
                            lifetime_arguments: outer_lifetime_parameters.to_vec(),
                            arguments: HandleSpan::empty(),
                        }),
                )
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let element_type = canonicalize_lifetime_bearing_type_argument(
                    syntax,
                    element_type,
                    outer_lifetime_parameters,
                    outer_lifetime_arguments,
                )?;
                Some(
                    syntax
                        .tables
                        .type_references
                        .insert(TypeReferenceNode::FixedArray {
                            element_type,
                            length: FixedArrayLength::Literal(length),
                        }),
                )
            }
            _ => Some(type_reference),
        }
    }

    pub(crate) fn exact_synthesized_lifetime_instance(
        syntax: &SyntaxTrees,
        name: &str,
        lifetime_arity: usize,
    ) -> bool {
        lifetime_arity > 0
            && syntax.root_items().any(|item| {
                matches!(
                    item,
                    Item::Data(definition)
                        if definition.name.as_str(syntax) == name
                            && definition.generic_instance.is_some()
                            && definition.type_parameters.is_empty()
                            && definition.lifetime_parameters.len() == lifetime_arity
                )
            })
    }

    /*
        Naming metadata for closed types, arrays, qualifications and observed ranges.
        Unsupported or open shapes remain on the ordinary generic path.
    */
    pub(crate) fn type_reference_slug(
        syntax: &SyntaxTrees,
        handle: TypeReferenceHandle,
        selection: Option<&declaration_selection::ConstantSelection>,
    ) -> Option<String> {
        match syntax.tables.type_references.type_reference(handle) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            } => {
                let access = access.borrow_prefix();
                Some(format!(
                    "{access}{}",
                    type_reference_slug(syntax, *referee, selection)?
                ))
            }
            TypeReferenceNode::Slice { element_type } => Some(format!(
                "[{}]",
                type_reference_slug(syntax, *element_type, selection)?
            )),
            TypeReferenceNode::Named(name) => Some(name.as_str(syntax).to_string()),
            TypeReferenceNode::ConstExpression(expression) => {
                match syntax.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => {
                        super::integer_literal_value(literal).map(|value| value.to_string())
                    }
                    ExpressionNode::Name {
                        members: path,
                        application: path_application,
                    } if path_application.is_none() => {
                        let [member] = syntax.expressions.identifier_path_members(path.to_owned())
                        else {
                            return None;
                        };
                        Some(member.as_str(syntax).to_owned())
                    }
                    _ => None,
                }
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } if syntax_type_references(syntax)
                .type_reference_handles(*arguments)
                .is_empty()
                && exact_synthesized_lifetime_instance(
                    syntax,
                    base_name.as_str(syntax),
                    lifetime_arguments.len(),
                ) =>
            {
                Some(base_name.as_str(syntax).to_owned())
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let length = super::closed_fixed_array_length(
                    syntax,
                    length,
                    &GenericApplicationSubstitution::new(),
                    selection,
                )
                .ok()?;
                if length == 0 {
                    return None;
                }
                Some(format!(
                    "[{}; {length}]",
                    type_reference_slug(syntax, *element_type, selection)?
                ))
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base = type_reference_slug(syntax, *base_type, selection)?;
                let mut rendered = Vec::new();
                for constraint in syntax_type_references(syntax).constraints(*constraints) {
                    rendered.push(constraint_slug(syntax, constraint, selection)?);
                }
                if rendered.is_empty() {
                    return Some(base);
                }
                Some(format!("{base} in {}", rendered.join(" + ")))
            }
            _ => None,
        }
    }

    /*
        Nameable behavior/domain tags. Ranges need their exact owner observation,
        handled by the caller, rather than rendering an arbitrary expression here.
        An indexed declared domain renders its name with the index tuple.
    */
    pub(crate) fn constraint_slug(
        syntax: &SyntaxTrees,
        constraint: &TypeConstraintNode,
        selection: Option<&declaration_selection::ConstantSelection>,
    ) -> Option<String> {
        match constraint {
            TypeConstraintNode::Named(name) => Some(name.as_str(syntax).to_string()),
            TypeConstraintNode::Domain(domain) => {
                let arguments =
                    syntax_type_references(syntax).type_reference_handles(domain.arguments);
                if arguments.is_empty() {
                    return Some(domain.name.as_str(syntax).to_string());
                }
                let indices = arguments
                    .iter()
                    .map(|argument| type_reference_slug(syntax, *argument, selection))
                    .collect::<Option<Vec<_>>>()?;
                Some(format!(
                    "{}<{}>",
                    domain.name.as_str(syntax),
                    indices.join(",")
                ))
            }
            TypeConstraintNode::ArithmeticDomain(domain) => Some(domain.name().to_string()),
        }
    }

    /*
        Classify only arguments the existing closed-shape gate has admitted. Open
        binders remain on the ordinary generic path; no unresolved name is an atom.
        The shared parameter-argument zip: each argument's closed identity —
        `Const`-kinded binders marking it const — or `None` when lengths disagree
        or any argument stays open.
    */
    fn closed_argument_identities(
        syntax: &SyntaxTrees,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        parameters: &[TypeParameter],
        arguments: &[TypeReferenceHandle],
    ) -> Option<Vec<ClosedArgumentIdentity>> {
        if parameters.len() != arguments.len() {
            return None;
        }
        parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                closed_parameter_argument_identity(syntax, selection, *argument, &parameter.kind)
            })
            .collect::<Option<Vec<_>>>()
    }

    /*
        Close an argument under its receiving declaration's actual parameter kind.
        Implementations do not enter the data namespace; unsupported static operand
        categories stay on the ordinary resolver/checker path.
    */
    pub(crate) fn closed_parameter_argument_identity(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        handle: TypeReferenceHandle,
        kind: &TypeParameterKind,
    ) -> Option<ClosedArgumentIdentity> {
        match kind {
            TypeParameterKind::Type => closed_argument_identity(syntax, selection, handle, false),
            TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                closed_argument_identity(syntax, selection, handle, true)
            }
            TypeParameterKind::Implementation => {
                let origin = syntax.type_references.generic_application_origin(handle);
                let handle = if origin.is_valid() { origin } else { handle };
                let (name, arguments) = match syntax.type_references.type_reference(handle) {
                    TypeReferenceNode::Named(name) => (name, &[][..]),
                    TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments,
                    } if lifetime_arguments.is_empty() => (
                        base_name,
                        syntax.type_references.type_reference_handles(*arguments),
                    ),
                    _ => return None,
                };
                let declaration = selection?.implementation_item(syntax, name).ok()?;
                let Item::Conformance(implementation) = syntax.root_item(declaration) else {
                    return None;
                };
                if !implementation.lifetime_parameters.is_empty() {
                    return None;
                }
                let parameters = syntax.items.type_parameters(implementation.type_parameters);
                Some(ClosedArgumentIdentity::Implementation(
                    declaration,
                    closed_argument_identities(syntax, selection, parameters, arguments)?,
                ))
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => None,
        }
    }

    pub(crate) fn closed_argument_identity(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        handle: TypeReferenceHandle,
        is_constant: bool,
    ) -> Option<ClosedArgumentIdentity> {
        let original = syntax.type_references.generic_application_origin(handle);
        if original.is_valid() {
            let TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } = syntax.type_references.type_reference(original)
            else {
                return None;
            };
            let declaration = selected_data_item(syntax, selection, base_name)?;
            let data = syntax.root_item(declaration).as_data()?;
            let parameters = syntax.items.type_parameters(data.type_parameters);
            let arguments = syntax.type_references.type_reference_handles(*arguments);
            let identities = closed_argument_identities(syntax, selection, parameters, arguments)?;
            return Some(ClosedArgumentIdentity::Instance(declaration, identities));
        }
        match syntax.type_references.type_reference(handle) {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime: None,
            } => Some(ClosedArgumentIdentity::Reference(
                *access,
                Box::new(closed_argument_identity(
                    syntax, selection, *referee, false,
                )?),
            )),
            TypeReferenceNode::Slice { element_type } => {
                Some(ClosedArgumentIdentity::Slice(Box::new(
                    closed_argument_identity(syntax, selection, *element_type, false)?,
                )))
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if arguments.is_empty() {
                    let mut candidates = syntax.root_items().filter_map(|item| match item {
                        Item::Data(data)
                            if data.name.as_str(syntax) == base_name.as_str(syntax)
                                && data.lifetime_parameters.len() == lifetime_arguments.len()
                                && data.generic_instance.is_some() =>
                        {
                            Some(data)
                        }
                        _ => None,
                    });
                    let instance = candidates.next()?;
                    if candidates.next().is_some() {
                        return None;
                    }
                    return closed_argument_identity(
                        syntax,
                        selection,
                        instance.generic_instance?,
                        false,
                    );
                }
                let declaration = selected_data_item(syntax, selection, base_name)?;
                let data = syntax.root_item(declaration).as_data()?;
                let parameters = syntax.items.type_parameters(data.type_parameters);
                let arguments = syntax.type_references.type_reference_handles(*arguments);
                Some(ClosedArgumentIdentity::Instance(
                    declaration,
                    closed_argument_identities(syntax, selection, parameters, arguments)?,
                ))
            }
            TypeReferenceNode::Named(name) if is_constant => Some(
                ClosedArgumentIdentity::Constant(name.as_str(syntax).to_owned()),
            ),
            TypeReferenceNode::ConstExpression(expression) if is_constant => {
                match syntax.expressions.expression(*expression) {
                    ExpressionNode::Integer(literal) => super::integer_literal_value(literal)
                        .map(|value| ClosedArgumentIdentity::Constant(value.to_string())),
                    ExpressionNode::Name {
                        members: path,
                        application: path_application,
                    } if path_application.is_none() => {
                        let [member] = syntax.expressions.identifier_path_members(path.to_owned())
                        else {
                            return None;
                        };
                        Some(ClosedArgumentIdentity::Constant(
                            member.as_str(syntax).to_owned(),
                        ))
                    }
                    _ => None,
                }
            }
            TypeReferenceNode::Named(name) => {
                if let Some(atom) = builtin_type_atom_by_symbol_name(name.as_str(syntax)) {
                    return Some(ClosedArgumentIdentity::Builtin(atom));
                }
                if let Some(declaration) = selected_data_item(syntax, selection, name) {
                    Some(ClosedArgumentIdentity::Nominal(declaration))
                } else {
                    Some(ClosedArgumentIdentity::RetainedNominal(
                        selection?.retained_identity(syntax, name, symbols::SymbolKind::Data)?,
                    ))
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => Some(ClosedArgumentIdentity::Array(
                Box::new(closed_argument_identity(
                    syntax,
                    selection,
                    *element_type,
                    false,
                )?),
                super::closed_fixed_array_length(
                    syntax,
                    length,
                    &GenericApplicationSubstitution::new(),
                    selection,
                )
                .ok()?,
            )),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base = closed_argument_identity(syntax, selection, *base_type, false)?;
                let mut identities = Vec::new();
                for constraint in syntax_type_references(syntax)
                    .constraints(*constraints)
                    .iter()
                {
                    identities.push(match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        ClosedConstraintIdentity::Arithmetic(*domain)
                    }
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => {
                        indexed_domain_identity(
                            syntax,
                            selection,
                            &domain.name,
                            &base,
                            domain.arguments,
                        )?
                    }
                    TypeConstraintNode::Named(name)
                    | TypeConstraintNode::Domain(syntax_trees::types::DomainConstraint {
                        name,
                        arguments: _,
                    }) => {
                        if let Some(selection) = selection {
                            match selection.domain(syntax, name.as_str(syntax), name.source_span(), None) {
                                Some(domain) => {
                                    if closed_argument_identity(
                                        syntax,
                                        Some(selection),
                                        domain.target_type,
                                        false,
                                    )
                                    .as_ref()
                                        != Some(&base)
                                    {
                                        return None;
                                    }
                                    let declaration = syntax
                                        .root_item_handles()
                                        .iter()
                                        .copied()
                                        .find(|handle| {
                                            matches!(syntax.root_item(*handle), Item::Domain(candidate)
                                                if std::ptr::eq(candidate, domain))
                                        })?;
                                    ClosedConstraintIdentity::Declaration(declaration)
                                }
                                None => {
                                    if selection.contested_domain(
                                        syntax,
                                        name.as_str(syntax),
                                        name.source_span(),
                                    ) {
                                        return None;
                                    }
                                    let carrier = syntax_type_references(syntax).type_reference(*base_type).as_named()?;
                                    ClosedConstraintIdentity::RetainedDeclaration(
                                        selection.retained_domain_identity(syntax, carrier, name)?,
                                    )
                                }
                            }
                        } else {
                            let mut candidates =
                                syntax.root_item_handles().iter().copied().filter(|handle| {
                                    let Some(domain) = syntax.root_item(*handle).as_domain() else {
                                        return false;
                                    };
                                    let matching_name = domain.name.as_str(syntax) == name.as_str(syntax)
                                        || (!name.as_str(syntax).contains("::")
                                            && domain.name.as_str(syntax).rsplit("::").next()
                                                == Some(name.as_str(syntax)));
                                    matching_name
                                        && closed_argument_identity(
                                            syntax,
                                            None,
                                            domain.target_type,
                                            false,
                                        )
                                        .as_ref()
                                            == Some(&base)
                                });
                            let declaration = candidates.next()?;
                            if candidates.next().is_some() {
                                return None;
                            }
                            ClosedConstraintIdentity::Declaration(declaration)
                        }
                    }

                });
                }
                Some(ClosedArgumentIdentity::Constrained(
                    Box::new(base),
                    identities,
                ))
            }
            _ => None,
        }
    }

    /*
        The family's index telescope and whether its leading type binder names the
        carrier: `domain<T> T::Foreign` reserves its first parameter for the
        generic carrier, while a concrete carrier like
        `domain<const C: u64> u64::AtMost<C>` keeps every declared parameter as an
        index. This is the one rule shared by identity, argument normalization and
        the structural matcher in `type_structure::applications`.
    */
    pub(crate) fn domain_index_telescope(
        syntax: &SyntaxTrees,
        definition: &DomainDefinition,
    ) -> (Vec<TypeParameter>, bool) {
        let parameters = syntax.items.type_parameters(definition.type_parameters);
        let target_name =
            match syntax_type_references(syntax).type_reference(definition.target_type) {
                TypeReferenceNode::Named(target) => Some(target.as_str(syntax)),
                _ => None,
            };
        let generic_carrier = parameters.first().is_some_and(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && target_name == Some(parameter.name.as_str(syntax))
        });
        (
            parameters[usize::from(generic_carrier)..].to_vec(),
            generic_carrier,
        )
    }

    /*
        Rejoin an authored domain family through the shared namespace authority.
        Without a supplied package selection, build the same local header authority;
        never rescue a contested or inaccessible owner with a leaf-name scan.
    */
    pub(crate) fn selected_domain_family(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
        carrier: Option<&str>,
    ) -> Option<ItemHandle> {
        let fallback;
        let selection = match selection {
            Some(selection) => selection,
            None => {
                fallback =
                    declaration_selection::ConstantSelection::new(syntax, None, Vec::new()).ok()?;
                &fallback
            }
        };
        let definition = selection
            .domain_family(syntax, name.as_str(syntax), name.source_span(), carrier)
            .map(|(_, definition)| definition)
            .or_else(|| {
                selection.domain(syntax, name.as_str(syntax), name.source_span(), carrier)
            })?;
        syntax.root_item_handles().iter().copied().find(|handle| {
            syntax
                .root_item(*handle)
                .as_domain()
                .is_some_and(|candidate| std::ptr::eq(candidate, definition))
        })
    }

    /*
        Identity for an indexed declared-domain application: the declaration the
        authored name selects on this carrier plus each index argument's own
        closed identity under the family's declared index telescope. Selection
        follows the shared declaration authority used by structural matching.
        Fixed families require their declared carrier; generic families retain the
        actual carrier in the enclosing constrained identity independently of the
        family's index tuple. Competing owners decline rather than equate spellings.
    */
    fn indexed_domain_identity(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        authored: &Identifier,
        carrier: &ClosedArgumentIdentity,
        arguments: HandleSpan<TypeReferenceHandle>,
    ) -> Option<ClosedConstraintIdentity> {
        let declaration = selected_domain_family(syntax, selection, authored, None)?;
        let definition = syntax.root_item(declaration).as_domain()?;
        let (index_parameters, generic_carrier) = domain_index_telescope(syntax, definition);
        if !generic_carrier
            && closed_argument_identity(syntax, selection, definition.target_type, false).as_ref()
                != Some(carrier)
        {
            return None;
        }
        let arguments = syntax.type_references.type_reference_handles(arguments);
        let indices = closed_argument_identities(syntax, selection, &index_parameters, arguments)?;
        Some(ClosedConstraintIdentity::IndexedDeclaration(
            declaration,
            indices,
        ))
    }

    /*
        One in-forest `domain` declaration selected as a carrier-qualified
        application head.
    */
    pub(crate) struct DomainHead {
        pub(crate) declaration: ItemHandle,
        /*
            The declaration's own name. A generic family's retains only the domain
            segments (`domain<const C: u64> u64::AtMost<C>` declares `AtMost`),
            which is also the canonical constraint name a rewritten spelling
            carries.
        */
        pub(crate) declared_name: Identifier,
        /*
            The family's index telescope: its declared parameters minus a leading
            type binder that names a generic carrier (`domain<T> T::Foreign`).
        */
        pub(crate) index_parameters: Vec<TypeParameter>,
        /* The declared carrier, retained as the canonical constrained base. */
        pub(crate) carrier: DomainCarrier,
    }

    /*
        A qualified head retains its authored carrier until it can join an ordinary
        type tree. Fixed families retain their declared tree; generic families bind
        the spelled carrier independently from their index telescope.
    */
    #[derive(Clone)]
    pub(crate) enum DomainCarrier {
        Type(TypeReferenceHandle),
        Named(Identifier),
    }

    impl DomainCarrier {
        pub(crate) fn materialize(self, syntax: &mut SyntaxTrees) -> TypeReferenceHandle {
            match self {
                Self::Type(reference) => reference,
                Self::Named(name) => syntax
                    .type_references
                    .insert(TypeReferenceNode::Named(name)),
            }
        }

        pub(crate) fn identity(
            &self,
            syntax: &SyntaxTrees,
            selection: Option<&declaration_selection::ConstantSelection>,
        ) -> Option<ClosedArgumentIdentity> {
            match self {
                Self::Type(reference) => {
                    closed_argument_identity(syntax, selection, *reference, false)
                }
                Self::Named(name) => super::closed_name_identity(syntax, selection, name),
            }
        }
    }

    /*
        Select the domain a `Carrier::Name` or bare `Name` head applies: the
        shared declaration authority owns the spelled leaf. Fixed families check
        the prefix against their declared target; generic families retain the
        explicit prefix as a type tree for ordinary structural binding. Competing
        or carrier-mismatched owners decline rather than equate spellings. Identity
        sees exactly the declaration chosen by the structural matcher.
    */
    pub(crate) fn domain_application_head(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<DomainHead> {
        let spelled = name.as_str(syntax);
        let (carrier, leaf) = match spelled.rsplit_once("::") {
            Some((carrier, leaf)) => (Some(carrier), leaf),
            None => (None, spelled),
        };
        let family = Identifier::new(leaf, name.source_span());
        let declaration = selected_domain_family(syntax, selection, &family, carrier)?;
        let definition = syntax.root_item(declaration).as_domain()?;
        let (index_parameters, generic_carrier) = domain_index_telescope(syntax, definition);
        let carrier = if generic_carrier {
            DomainCarrier::Named(Identifier::new(carrier?, name.source_span()))
        } else {
            if let Some(carrier) = carrier {
                let authored = Identifier::new(carrier, name.source_span());
                if super::closed_name_identity(syntax, selection, &authored)?
                    != closed_argument_identity(syntax, selection, definition.target_type, false)?
                {
                    return None;
                }
            }
            DomainCarrier::Type(definition.target_type)
        };
        Some(DomainHead {
            declaration,
            declared_name: definition.name.clone(),
            index_parameters,
            carrier,
        })
    }

    /*
        A carrier-qualified `Carrier::Name<indices>` type spells the same domain
        application as the constrained `Carrier in Name<indices>` node. Rewrite
        every reachable head in `positions` before the closed-index
        canonicalization snapshot so a module constant inside one folds exactly as
        an authored constraint, and solving, identity, naming and substitution all
        observe the one canonical shape the constrained spelling produces.
        `positions` is the owner-position walk: every type reference reachable
        from a concrete owner or an open template member.
    */
    pub(crate) fn normalize_domain_head_positions(
        syntax: &mut SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        positions: &[TypeReferenceHandle],
    ) {
        for &position in positions {
            normalize_domain_head_node(syntax, selection, position);
        }
    }

    fn normalize_domain_head_node(
        syntax: &mut SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        handle: TypeReferenceHandle,
    ) {
        match syntax.type_references.type_reference(handle).clone() {
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let nested = syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .to_vec();
                if lifetime_arguments.is_empty()
                    && selected_data_item(syntax, selection, &base_name).is_none()
                    && let Some(head) = domain_application_head(syntax, selection, &base_name)
                {
                    let name =
                        Identifier::new(head.declared_name.as_str(syntax), base_name.source_span());
                    let constraints =
                        syntax
                            .type_references
                            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                                name,
                                arguments,
                            })]);
                    let base_type = head.carrier.materialize(syntax);
                    syntax.type_references.replace_type_reference(
                        handle,
                        TypeReferenceNode::Constrained {
                            base_type,
                            constraints,
                        },
                    );
                }
                for nested in nested {
                    normalize_domain_head_node(syntax, selection, nested);
                }
            }
            TypeReferenceNode::Reference { referee, .. } => {
                normalize_domain_head_node(syntax, selection, referee);
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                normalize_domain_head_node(syntax, selection, element_type);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                normalize_domain_head_node(syntax, selection, base_type);
                for constraint in syntax.type_references.constraints(constraints).to_vec() {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    for argument in syntax_type_references(syntax)
                        .type_reference_handles(domain.arguments)
                        .to_vec()
                    {
                        normalize_domain_head_node(syntax, selection, argument);
                    }
                }
            }
            _ => {}
        }
    }
}
#[path = "preparation_const_evaluation.rs"]
mod const_evaluation;
mod discovery {
    /* Private discovered templates and pending rewrites; not a program representation. */
    use crate::preparation::declaration_selection;
    use arena::HandleSpan;
    use std::collections::HashMap;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::Item;
    use syntax_trees::item::ProofFact;
    use syntax_trees::types::TypeReferenceHandle;

    pub(super) struct GenericData {
        pub(super) declaration: syntax_trees::item::ItemHandle,
        pub(super) name: String,
        pub(super) origin_name: Identifier,
        pub(super) is_public: bool,
        pub(super) lifetime_parameters: Vec<Identifier>,
        pub(super) parameter_names: Vec<String>,
        pub(super) const_parameter_types: Vec<Option<TypeReferenceHandle>>,
        pub(super) cleanup_selection: Option<syntax_trees::expression::StaticMachineArgument>,
        pub(super) requires_facts: HandleSpan<ProofFact>,
        /*
            The `requires` facts that equate a type binder with known structure.
            Applying the template decides them; instances never carry them.
        */
        pub(super) type_equations: Vec<super::equations::TypeEquation>,
        pub(super) members: HandleSpan<DataMember>,
        pub(super) properties: syntax_trees::item::DataProperties,
        pub(super) supply_mode: language_semantics::DataSupplyMode,
    }

    pub(super) struct PendingRewrite {
        pub(super) type_reference: TypeReferenceHandle,
        pub(super) synthetic_name: String,
        pub(super) lifetime_arguments: Vec<Identifier>,
    }

    /*
        A canonical closed use, before its owner decides whether a declaration is
        demanded or an existing specialization can serve the identity edge.
    */
    pub(super) struct ClosedApplication {
        pub(super) base_name: Identifier,
        pub(super) lifetime_arguments: Vec<Identifier>,
        pub(super) template: syntax_trees::item::ItemHandle,
        pub(super) argument_handles: Vec<TypeReferenceHandle>,
        pub(super) argument_identity: Vec<ClosedArgumentIdentity>,
        pub(super) argument_names: Vec<String>,
    }

    /*
        One discovered instantiation: the base generic definition and the argument
        type references spelled for it and its materialized declaration. The name
        is only lookup metadata; the live item and argument tuple own identity.
    */
    #[derive(Clone)]
    pub(super) struct Instantiation {
        pub(super) synthetic_name: String,
        pub(super) declaration: syntax_trees::item::ItemHandle,
        pub(super) template: syntax_trees::item::ItemHandle,
        pub(super) argument_handles: Vec<TypeReferenceHandle>,
        pub(super) argument_identity: Vec<ClosedArgumentIdentity>,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum GenericDataShape {
        Record,
        PureSum,
        MixedSum,
    }

    /*
        Rejoin the selected declaration to this syntax owner. Header symbols never
        escape, and generated declarations retain distinct arena identities even if
        they share an absent or authored derivation span.
    */
    pub(crate) fn selected_data_item(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<syntax_trees::item::ItemHandle> {
        if let Some(selection) = selection {
            let declaration = selection.data(syntax, name).ok()?;
            return syntax.root_item_handles().iter().copied().find(|handle| {
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, declaration))
        });
        }
        let mut candidates = syntax.root_item_handles().iter().copied().filter(|handle| {
        matches!(syntax.root_item(*handle), Item::Data(data) if data.name.as_str(syntax) == name.as_str(syntax))
    });
        let first = candidates.next()?;
        candidates.next().is_none().then_some(first)
    }

    /*
        `selected_data_item` through a root index built once for many lookups in
        the same unchanged forest.
    */
    pub(crate) fn selected_data_item_in(
        roots: &declaration_selection::RootDeclarations<'_>,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<syntax_trees::item::ItemHandle> {
        let Some(selection) = selection else {
            return selected_data_item(roots.syntax(), None, name);
        };
        let declaration = selection.data_in(roots, name).ok()?;
        let syntax = roots.syntax();
        roots.at(declaration.name.source_span()).find(|handle| {
        matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, declaration))
    })
    }

    pub(super) fn selected_generic_data<'a>(
        syntax: &SyntaxTrees,
        templates: &'a HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<&'a GenericData> {
        templates.get(&selected_data_item(syntax, selection, name)?)
    }

    /*
        Equality for the already admitted closed argument shapes. Rendered names are
        only lookup/diagnostic metadata; nominal equality names a live declaration.
    */
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum ClosedArgumentIdentity {
        Builtin(symbols::BuiltinTypeAtom),
        Nominal(syntax_trees::item::ItemHandle),
        RetainedNominal(symbols::SymbolHandle),
        Instance(syntax_trees::item::ItemHandle, Vec<ClosedArgumentIdentity>),
        /*
            Code arguments are distinct from data even when their lookup names
            coincide. The implementation's complete own telescope closes the key.
        */
        Implementation(syntax_trees::item::ItemHandle, Vec<ClosedArgumentIdentity>),
        Constant(String),
        Array(Box<ClosedArgumentIdentity>, usize),
        Reference(language_core::ReferenceAccess, Box<ClosedArgumentIdentity>),
        Slice(Box<ClosedArgumentIdentity>),
        Constrained(Box<ClosedArgumentIdentity>, Vec<ClosedConstraintIdentity>),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) enum ClosedConstraintIdentity {
        Arithmetic(numerics::arithmetic::ArithmeticDomain),
        Declaration(syntax_trees::item::ItemHandle),
        /*
            An indexed declared-domain application: the selected declaration plus
            each index argument's own closed identity under the family's declared
            index telescope (`u64 in AtMost<256>`).
        */
        IndexedDeclaration(syntax_trees::item::ItemHandle, Vec<ClosedArgumentIdentity>),
        RetainedDeclaration(symbols::SymbolHandle),
    }
}
pub(crate) mod domain_heads {
    /*
        Resolve carrier-binder domain heads before substituting their carrier.

        `T in T::Marked` selects a family in the template author's scope. Once
        `T` becomes an array or a foreign nominal type, the original carrier prefix
        can no longer resolve there. Select the exact exposed generic-carrier family
        while the owning telescope is available and retain its complete declaration
        address with the authored use span. Original templates and their instances
        then use the same ordinary domain lookup and authored-selection evidence;
        the receiving package's imports cannot redirect either one.
        Alias constituents use the same selection: their carrier prefix refers to
        the alias's binder, not a top-level module with the same spelling. This
        binds names only; typed alias application still checks subject compatibility
        and the selected family's requirements before expanding its atoms.
    */

    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use diagnostics::Diagnostic;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::{DataMember, Item, TypeParameterKind};
    use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    use crate::preparation::declaration_selection::ConstantSelection;

    pub(crate) fn normalize(
        syntax: &mut SyntaxTrees,
        selection: Option<&ConstantSelection>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut pending = Vec::new();
        let mut alias_heads = Vec::new();
        for &declaration in syntax.root_item_handles() {
            let item = syntax.root_item(declaration);
            if let Some(domain) = item.as_domain() {
                let Some(alias) = &domain.alias else {
                    continue;
                };
                let parameters = syntax.items.type_parameters(domain.type_parameters);
                for (ordinal, constituent) in alias.constituents.iter().enumerate() {
                    let members = syntax.items.identifier_path_members(*constituent);
                    if members.len() < 2 {
                        continue;
                    }
                    let Some(carrier) = members.first() else {
                        continue;
                    };
                    let Some(parameter) = parameters
                        .iter()
                        .find(|parameter| parameter.name.as_str(syntax) == carrier.as_str(syntax))
                    else {
                        continue;
                    };
                    let [_, leaf] = members else {
                        return Err(vec![Diagnostic::error(format!(
                        "domain alias carrier type binder `{}` must be followed by one domain name",
                        carrier.as_str(syntax)
                    )).with_source_span(carrier.source_span())]);
                    };
                    if !matches!(parameter.kind, TypeParameterKind::Type)
                        || named_carrier(syntax, domain.target_type) != Some(carrier.as_str(syntax))
                    {
                        return Err(vec![Diagnostic::error(format!(
                        "domain alias constituent `{}::{}` must name this alias's carrier type binder",
                        carrier.as_str(syntax), leaf.as_str(syntax)
                    )).with_source_span(carrier.source_span())]);
                    }
                    alias_heads.push((
                        declaration,
                        ordinal,
                        leaf.as_str(syntax).to_owned(),
                        carrier.source_span(),
                    ));
                }
                continue;
            }
            let Some(definition) = item.as_data() else {
                continue;
            };
            let parameters = syntax.items.type_parameters(definition.type_parameters);
            if parameters.is_empty() {
                continue;
            }
            let mut types = Vec::new();
            for member in syntax.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => types.push(field.type_reference),
                    DataMember::Variant(variant) => types.extend(
                        syntax_items(syntax)
                            .data_payload_fields(variant.payload)
                            .iter()
                            .map(|field| field.type_reference),
                    ),
                    DataMember::Retired(_) => {}
                }
            }
            while let Some(site) = types.pop() {
                match syntax.type_references.type_reference(site) {
                    TypeReferenceNode::Reference { referee, .. } => types.push(*referee),
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => types.push(*element_type),
                    TypeReferenceNode::Generic { arguments, .. } => {
                        types.extend(syntax.type_references.type_reference_handles(*arguments))
                    }
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        types.push(*base_type);
                        for (ordinal, constraint) in syntax_type_references(syntax)
                            .constraints(*constraints)
                            .iter()
                            .enumerate()
                        {
                            let TypeConstraintNode::Domain(domain) = constraint else {
                                continue;
                            };
                            types.extend(
                                syntax_type_references(syntax)
                                    .type_reference_handles(domain.arguments),
                            );
                            let Some((carrier, leaf)) =
                                domain.name.as_str(syntax).rsplit_once("::")
                            else {
                                continue;
                            };
                            let Some(parameter) = parameters
                                .iter()
                                .find(|parameter| parameter.name.as_str(syntax) == carrier)
                            else {
                                continue;
                            };
                            if !matches!(parameter.kind, TypeParameterKind::Type)
                                || named_carrier(syntax, *base_type) != Some(carrier)
                            {
                                return Err(vec![Diagnostic::error(format!(
                                "domain qualification `{}` must name this field's carrier type binder",
                                domain.name.as_str(syntax)
                            )).with_source_span(domain.name.source_span())]);
                            }
                            pending.push((
                                site,
                                ordinal,
                                leaf.to_owned(),
                                domain.name.source_span(),
                            ));
                        }
                    }
                    TypeReferenceNode::StaticSelection(selection) => {
                        super::uses::visit_static_argument_type_inputs(
                            syntax,
                            std::slice::from_ref(selection),
                            &mut |reference| types.push(reference),
                        );
                    }
                    TypeReferenceNode::Named(_)
                    | TypeReferenceNode::DynamicTrait { .. }
                    | TypeReferenceNode::ConstExpression(_)
                    | TypeReferenceNode::Unit => {}
                }
            }
        }
        if pending.is_empty() && alias_heads.is_empty() {
            return Ok(());
        }
        let fallback;
        let selection = if let Some(selection) = selection {
            selection
        } else {
            fallback = ConstantSelection::new(syntax, None, Vec::new())?;
            &fallback
        };
        for (site, ordinal, leaf, reference) in pending {
            let Some(address) = selection.generic_carrier_domain_address(syntax, &leaf, reference)
            else {
                return Err(vec![Diagnostic::error(format!(
                "carrier-qualified domain `{leaf}` does not select one exposed generic-carrier family"
            )).with_source_span(reference)]);
            };
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = syntax.type_references.type_reference(site).clone()
            else {
                continue;
            };
            let mut constraints = syntax.type_references.constraints(constraints).to_vec();
            let Some(TypeConstraintNode::Domain(domain)) = constraints.get_mut(ordinal) else {
                continue;
            };
            domain.name = Identifier::new(address, reference);
            let constraints = syntax.type_references.insert_constraints(constraints);
            syntax.type_references.replace_type_reference(
                site,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                },
            );
        }
        for (declaration, ordinal, leaf, reference) in alias_heads {
            let Some(address) = selection.generic_carrier_domain_address(syntax, &leaf, reference)
            else {
                return Err(vec![Diagnostic::error(format!(
                "carrier-qualified domain `{leaf}` does not select one exposed generic-carrier family"
            )).with_source_span(reference)]);
            };
            let Item::Domain(mut domain) = syntax.root_item(declaration).clone() else {
                continue;
            };
            let Some(alias) = &mut domain.alias else {
                continue;
            };
            alias.constituents[ordinal] = syntax
                .tables
                .items
                .insert_identifier_path_members([Identifier::new(address, reference)]);
            syntax.items.replace_item(declaration, Item::Domain(domain));
        }
        Ok(())
    }

    fn named_carrier(syntax: &SyntaxTrees, mut carrier: TypeReferenceHandle) -> Option<&str> {
        loop {
            match syntax.type_references.type_reference(carrier) {
                TypeReferenceNode::Named(name) => return Some(name.as_str(syntax)),
                TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                _ => return None,
            }
        }
    }
}
mod eligibility {
    /*
        Eligible data shapes, recursion and substitutable types.
        Named array extents either belong to the template's binder roster or select
        a closed declaration in their authored scope. Reuse the closed extent reader
        for declaration readiness without rewriting the identifier: substitution and
        receiving typing still need its selected origin. Binder-headed paths cannot
        fall through to a same-spelled global declaration during specialization.
        Generic arguments follow the same recursive substitution rules as fields.
        Closed children keep their existing sharing route; parameter-bearing arrays
        and applications must be substitutable throughout, rather than only at a
        named leaf. Receiving typing still checks each constructor's argument kinds.
    */
    use crate::preparation::declaration_selection;
    use crate::preparation::generic_data::GenericData;
    use crate::preparation::generic_data::GenericDataShape;
    use crate::preparation::generic_data::selected_generic_data;
    use crate::preparation::generic_data::type_reference_mentions_parameter;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::DataMember;
    use syntax_trees::types::FixedArrayLength;
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;

    /*
        Whether every field of a record or sum can be substituted soundly. A
        field may be exactly the parameter, a concrete Named, a parameter-free
        composite, or a nested known generic whose arguments are substitutable.
    */
    pub(in crate::preparation::generic_data) fn base_is_fully_monomorphizable(
        syntax: &SyntaxTrees,
        generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        base_info: &GenericData,
    ) -> bool {
        if generic_data_is_recursive(syntax, generic_data, selection, base_info) {
            return false;
        }
        let parameters: HashMap<String, TypeReferenceHandle> = base_info
            .parameter_names
            .iter()
            .map(|name| (name.clone(), TypeReferenceHandle::default()))
            .collect();
        let Some(shape) = generic_data_shape(syntax, base_info) else {
            return false;
        };
        syntax_items(syntax)
            .data_members(base_info.members)
            .iter()
            .all(|member| match member {
                DataMember::Field(field)
                    if matches!(shape, GenericDataShape::Record | GenericDataShape::MixedSum) =>
                {
                    type_reference_is_substitutable(
                        syntax,
                        generic_data,
                        selection,
                        base_info,
                        field,
                        &parameters,
                    )
                }
                DataMember::Variant(variant)
                    if matches!(
                        shape,
                        GenericDataShape::PureSum | GenericDataShape::MixedSum
                    ) =>
                {
                    syntax_items(syntax)
                        .data_payload_fields(variant.payload)
                        .iter()
                        .all(|field| {
                            type_reference_is_substitutable(
                                syntax,
                                generic_data,
                                selection,
                                base_info,
                                field,
                                &parameters,
                            )
                        })
                }
                DataMember::Retired(_) => true,
                _ => false,
            })
    }

    pub(in crate::preparation::generic_data) fn generic_data_is_recursive(
        syntax: &SyntaxTrees,
        generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        base: &GenericData,
    ) -> bool {
        fn reaches(
            syntax: &SyntaxTrees,
            templates: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
            selection: Option<&declaration_selection::ConstantSelection>,
            current: &GenericData,
            goal: syntax_trees::item::ItemHandle,
            visited: &mut HashSet<syntax_trees::item::ItemHandle>,
        ) -> bool {
            if !visited.insert(current.declaration) {
                return false;
            }
            generic_inline_data_edges(syntax, current)
                .iter()
                .any(|name| {
                    if current
                        .parameter_names
                        .iter()
                        .any(|parameter| parameter == name.as_str(syntax))
                    {
                        return false;
                    }
                    let Some(next) = selected_generic_data(syntax, templates, selection, name)
                    else {
                        return false;
                    };
                    next.declaration == goal
                        || reaches(syntax, templates, selection, next, goal, visited)
                })
        }
        reaches(
            syntax,
            generic_data,
            selection,
            base,
            base.declaration,
            &mut HashSet::new(),
        )
    }

    pub(in crate::preparation::generic_data) fn generic_inline_data_edges(
        syntax: &SyntaxTrees,
        definition: &GenericData,
    ) -> Vec<Identifier> {
        fn collect(
            syntax: &SyntaxTrees,
            type_reference: TypeReferenceHandle,
            edges: &mut Vec<Identifier>,
        ) {
            match syntax.tables.type_references.type_reference(type_reference) {
                TypeReferenceNode::Named(name) => {
                    edges.push(name.clone());
                }
                TypeReferenceNode::Generic {
                    base_name,
                    arguments,
                    ..
                } => {
                    edges.push(base_name.clone());
                    for argument in
                        syntax_type_references(syntax).type_reference_handles(*arguments)
                    {
                        collect(syntax, *argument, edges);
                    }
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    collect(syntax, *base_type, edges)
                }
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    collect(syntax, *element_type, edges)
                }
                TypeReferenceNode::StaticSelection(selection) => {
                    super::uses::visit_static_argument_type_inputs(
                        syntax,
                        std::slice::from_ref(selection),
                        &mut |reference| collect(syntax, reference, edges),
                    );
                }
                TypeReferenceNode::Reference { .. }
                | TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => {}
            }
        }

        let mut edges = Vec::new();
        for member in syntax.tables.items.data_members(definition.members) {
            match member {
                DataMember::Field(field) => collect(syntax, field.type_reference, &mut edges),
                DataMember::Variant(variant) => {
                    for field in syntax.tables.items.data_payload_fields(variant.payload) {
                        collect(syntax, field.type_reference, &mut edges);
                    }
                }
                DataMember::Retired(_) => {}
            }
        }
        edges
    }

    pub(in crate::preparation::generic_data) fn generic_data_shape(
        syntax: &SyntaxTrees,
        base_info: &GenericData,
    ) -> Option<GenericDataShape> {
        let mut has_fields = false;
        let mut has_variants = false;
        for member in syntax.tables.items.data_members(base_info.members) {
            match member {
                DataMember::Field(_) => has_fields = true,
                DataMember::Variant(_) => has_variants = true,
                DataMember::Retired(_) => {}
            }
        }
        match (has_fields, has_variants) {
            (true, false) | (false, false) => Some(GenericDataShape::Record),
            (false, true) => Some(GenericDataShape::PureSum),
            (true, true) => Some(GenericDataShape::MixedSum),
        }
    }

    pub(in crate::preparation::generic_data) fn type_reference_is_substitutable(
        syntax: &SyntaxTrees,
        generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        base_info: &GenericData,
        field: &syntax_trees::item::DataField,
        parameters: &HashMap<String, TypeReferenceHandle>,
    ) -> bool {
        type_reference_handle_is_substitutable(
            syntax,
            generic_data,
            selection,
            base_info,
            field.type_reference,
            parameters,
        )
    }

    pub(in crate::preparation::generic_data) fn type_reference_handle_is_substitutable(
        syntax: &SyntaxTrees,
        generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        selection: Option<&declaration_selection::ConstantSelection>,
        base_info: &GenericData,
        type_reference: TypeReferenceHandle,
        parameters: &HashMap<String, TypeReferenceHandle>,
    ) -> bool {
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Named(_) => true,
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let head = base_name
                    .as_str(syntax)
                    .split_once("::")
                    .map_or(base_name.as_str(syntax), |(head, _)| head);
                !parameters.contains_key(head)
                    && selected_generic_data(syntax, generic_data, selection, base_name).is_some()
                    && syntax_type_references(syntax)
                        .type_reference_handles(*arguments)
                        .iter()
                        .all(|&argument| {
                            matches!(
                                syntax.tables.type_references.type_reference(argument),
                                TypeReferenceNode::ConstExpression(_)
                            ) || !type_reference_mentions_parameter(syntax, argument, parameters)
                                || type_reference_handle_is_substitutable(
                                    syntax,
                                    generic_data,
                                    selection,
                                    base_info,
                                    argument,
                                    parameters,
                                )
                        })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_is_substitutable = type_reference_handle_is_substitutable(
                    syntax,
                    generic_data,
                    selection,
                    base_info,
                    *element_type,
                    parameters,
                );
                let length_is_substitutable = match length {
                    FixedArrayLength::Literal(_)
                    | FixedArrayLength::ConstCall(_)
                    | FixedArrayLength::Expression(_) => true,
                    FixedArrayLength::ConstParameter(name) => {
                        let head = name
                            .as_str(syntax)
                            .split_once("::")
                            .map_or(name.as_str(syntax), |(head, _)| head);
                        if let Some((parameter_name, parameter_type)) = base_info
                            .parameter_names
                            .iter()
                            .zip(&base_info.const_parameter_types)
                            .find(|(parameter_name, _)| parameter_name.as_str() == head)
                        {
                            parameter_name == name.as_str(syntax) && parameter_type.is_some()
                        } else {
                            super::closed_fixed_array_length(
                                syntax,
                                length,
                                &declaration_selection::GenericApplicationSubstitution::new(),
                                selection,
                            )
                            .is_ok()
                        }
                    }
                };
                element_is_substitutable && length_is_substitutable
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference_handle_is_substitutable(
                syntax,
                generic_data,
                selection,
                base_info,
                *referee,
                parameters,
            ),
            TypeReferenceNode::Slice { element_type } => type_reference_handle_is_substitutable(
                syntax,
                generic_data,
                selection,
                base_info,
                *element_type,
                parameters,
            ),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                type_reference_handle_is_substitutable(
                    syntax,
                    generic_data,
                    selection,
                    base_info,
                    *base_type,
                    parameters,
                ) && syntax_type_references(syntax)
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| match constraint {
                        TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {
                            true
                        }
                        TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                            .type_reference_handles(domain.arguments)
                            .iter()
                            .all(|&argument| {
                                matches!(
                                    syntax.tables.type_references.type_reference(argument),
                                    TypeReferenceNode::Named(_)
                                        | TypeReferenceNode::ConstExpression(_)
                                ) || !type_reference_mentions_parameter(
                                    syntax, argument, parameters,
                                )
                            }),
                    })
            }
            _ => !type_reference_mentions_parameter(syntax, type_reference, parameters),
        }
    }
}
pub(crate) use crate::preparation::type_equations as equations;
pub(crate) use equations::{template_type_equation_offsets, validate_materialized_type_equations};
pub(crate) mod source_modules {
    /* Source module ownership used by declaration selection and domain naming. */
    use source::SourceId;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::identifier::JoinedSyntaxPath;
    use syntax_trees::item::Item;

    pub(crate) fn module_path(syntax: &SyntaxTrees, source: SourceId) -> Option<String> {
        syntax.root_items().find_map(|item| {
            let Item::Module(module) = item else {
                return None;
            };
            let members = syntax.items.identifier_path_members(module.path);
            members
                .first()
                .filter(|member| member.source_span().source_id == source)?;
            Some(members.joined_path_in(syntax))
        })
    }
}
mod static_argument_positions {
    /*
        Static calls and nested implementation applications supply the same const
        destinations as data applications. Reachability comes from the existing
        concrete-owner walk; declaration lookup supplies only the binder carrier.
        The shared evaluator still resolves original lexical operands and admits
        every invocation before publishing a canonical value.
    */

    use super::{
        HashSet, Identifier, SyntaxTrees, TypeReferenceHandle, TypeReferenceNode,
        declaration_selection::ConstantSelection,
    };
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use symbols::SymbolKind;
    use syntax_trees::expression::{ExpressionNode, StaticMachineArgument};
    use syntax_trees::statement::StatementNode;

    pub(super) fn collect(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection<'_>,
        declaration_syntax: Option<&SyntaxTrees>,
        positions: &[TypeReferenceHandle],
        public_positions: &HashSet<TypeReferenceHandle>,
        pending: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
    ) {
        let mut applications = Vec::new();
        let mut visit = |name: Identifier, arguments: &[StaticMachineArgument]| {
            applications.push((name, arguments.to_vec()));
        };
        for (_, expression) in syntax.expressions.iter_expressions() {
            if let ExpressionNode::Call(call) = expression {
                if !has_const_expression_argument(syntax, &call.machine_arguments) {
                    continue;
                }
                let mut name = call.target.as_str(syntax).to_owned();
                if call.receiver.is_valid() {
                    let ExpressionNode::Name {
                        members: path,
                        application: path_application,
                    } = syntax.expressions.expression(call.receiver)
                    else {
                        continue;
                    };
                    if path_application.is_some() {
                        continue;
                    }
                    let prefix = syntax_expressions(syntax)
                        .identifier_path_members(path.to_owned())
                        .iter()
                        .map(|part| part.as_str(syntax))
                        .collect::<Vec<_>>()
                        .join("::");
                    name = format!("{prefix}::{name}");
                }
                visit(
                    Identifier::new(name, call.target.source_span()),
                    &call.machine_arguments,
                );
            }
        }
        for state in super::uses::selected_machine_state_handles(syntax, syntax.root_item_handles())
        {
            for statement in syntax_items(syntax).statements(syntax.items.state(state).statements) {
                if let StatementNode::Call(call) = syntax.statements.statement(*statement) {
                    if !has_const_expression_argument(syntax, &call.machine_arguments) {
                        continue;
                    }
                    let mut path = syntax
                        .statements
                        .identifier_path_members(call.receiver)
                        .iter()
                        .map(|part| part.as_str(syntax))
                        .collect::<Vec<_>>();
                    path.push(call.target.as_str(syntax));
                    visit(
                        Identifier::new(path.join("::"), call.target.source_span()),
                        &call.machine_arguments,
                    );
                }
            }
        }
        let positions = positions.iter().copied().collect::<HashSet<_>>();
        for (name, arguments) in applications {
            collect_application(
                syntax,
                selection,
                declaration_syntax,
                &name,
                &arguments,
                &positions,
                public_positions,
                pending,
            );
        }
    }

    fn has_const_expression_argument(
        syntax: &SyntaxTrees,
        arguments: &[StaticMachineArgument],
    ) -> bool {
        arguments.iter().any(|argument| {
            (argument.type_reference.is_valid()
                && matches!(
                    syntax_type_references(syntax).type_reference(argument.type_reference),
                    TypeReferenceNode::ConstExpression(_)
                ))
                || argument.application.as_ref().is_some_and(|application| {
                    has_const_expression_argument(syntax, &application.arguments)
                })
        })
    }

    fn collect_application(
        syntax: &mut SyntaxTrees,
        selection: &ConstantSelection<'_>,
        declaration_syntax: Option<&SyntaxTrees>,
        name: &Identifier,
        arguments: &[StaticMachineArgument],
        positions: &HashSet<TypeReferenceHandle>,
        public_positions: &HashSet<TypeReferenceHandle>,
        pending: &mut Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
    ) {
        if arguments.iter().any(|argument| {
            argument.type_reference.is_valid()
                && matches!(
                    syntax_type_references(syntax).type_reference(argument.type_reference),
                    TypeReferenceNode::ConstExpression(_)
                )
        }) && let Some(parameters) = selection.const_parameter_carriers(
            syntax,
            declaration_syntax,
            name,
            &[SymbolKind::Machine, SymbolKind::Conformance],
        ) && parameters.len() == arguments.len()
        {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if let Some(destination) = *parameter
                    && positions.contains(&argument.type_reference)
                    && matches!(
                        syntax_type_references(syntax).type_reference(argument.type_reference),
                        TypeReferenceNode::ConstExpression(_)
                    )
                    && !pending
                        .iter()
                        .any(|(existing, _, _)| *existing == argument.type_reference)
                {
                    pending.push((
                        argument.type_reference,
                        destination,
                        public_positions.contains(&argument.type_reference),
                    ));
                }
            }
        }
        for argument in arguments {
            if let Some(application) = &argument.application
                && let Some(first) = argument.path.first()
            {
                let name = Identifier::new(
                    argument
                        .path
                        .iter()
                        .map(|part| part.as_str(syntax))
                        .collect::<Vec<_>>()
                        .join("::"),
                    first.source_span(),
                );
                collect_application(
                    syntax,
                    selection,
                    declaration_syntax,
                    &name,
                    &application.arguments,
                    positions,
                    public_positions,
                    pending,
                );
            }
        }
    }
}
mod substitution {
    /* Member and type-reference substitution. */
    use crate::preparation::declaration_selection;
    use crate::preparation::generic_data::ClosedArgumentIdentity;
    use crate::preparation::generic_data::EvaluatedConst;
    use crate::preparation::generic_data::evaluate_const_argument_expression;
    use crate::preparation::generic_data::generic_const_integer_types;
    use crate::preparation::generic_data::selected_data_item;
    use crate::tables::syntax_expressions;
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;
    use arena::Handle;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use numerics::literals::IntegerLiteral;
    use numerics::literals::IntegerRadix;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::BinaryOperator;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::expression::UnaryOperator;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::ProofFact;
    use syntax_trees::types::DomainConstraint;
    use syntax_trees::types::FixedArrayLength;
    use syntax_trees::types::TypeConstraintNode;
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;

    /*
        Close the copied type graph in any declaration, including type payloads
        nested inside range expressions and static applications. Replacing only
        field roots leaves those copied children referring to erased binders.
    */
    pub(super) fn substitute_copied_type_names(
        syntax: &mut SyntaxTrees,
        copied_names: impl IntoIterator<Item = (TypeReferenceHandle, String)>,
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) {
        for (handle, name) in copied_names {
            let Some(argument) = substitution.get(&name) else {
                continue;
            };
            let replacement = syntax.type_references.type_reference(*argument).clone();
            syntax
                .type_references
                .replace_type_reference(handle, replacement);
            let application = syntax.type_references.generic_application_origin(*argument);
            if application.is_valid() {
                syntax
                    .type_references
                    .retain_generic_application_origin(handle, application);
            }
            syntax
                .type_references
                .retain_copied_operand_owner(*argument, handle);
        }
    }

    /*
        Clone a member with the type parameters substituted. Only reached for a base
        `base_is_fully_monomorphizable` accepted. A field that IS a parameter points
        at the argument; a NESTED generic (`a: Box<T>`) becomes a fresh concrete
        spelling (`Box<i32>`) the fixpoint monomorphizes; a parameter-free field is
        shared unchanged.
    */
    pub(crate) fn substitute_member(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        member: DataMember,
        substitution: &HashMap<String, TypeReferenceHandle>,
        impossible_variants: &HashSet<String>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<DataMember, Diagnostic> {
        let member = match member {
            DataMember::Field(field) => DataMember::Field(substitute_data_field(
                syntax,
                snapshot,
                field,
                substitution,
                selection,
                warnings,
            )),
            DataMember::Variant(mut variant) => {
                let payload = syntax_items(syntax)
                    .data_payload_fields(variant.payload)
                    .to_vec();
                let mut first = Handle::invalid();
                let mut count = 0u32;
                for field in payload {
                    let field = substitute_data_field(
                        syntax,
                        snapshot,
                        field,
                        substitution,
                        selection,
                        warnings,
                    );
                    let handle = syntax.tables.items.append_data_payload_field(field);
                    if count == 0 {
                        first = handle;
                    }
                    count = count
                        .checked_add(1)
                        .expect("generic sum payload field count overflow");
                }
                variant.payload = HandleSpan::from_parts(first, count);
                let mut copied_facts = HandleSpan::empty();
                for fact in variant.requires_facts.handles() {
                    copied_facts.push_contiguous(syntax.copy_proof_fact_from(snapshot, fact));
                }
                variant.requires_facts = copied_facts;
                if impossible_variants.contains(variant.name.as_str(syntax)) {
                    let witness = syntax.expressions.insert(ExpressionNode::Integer(
                        IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "0")
                            .expect("literal `0` is a valid integer literal"),
                    ));
                    let fact = syntax
                        .items
                        .append_proof_fact(ProofFact::Expression(witness));
                    variant.requires_facts.push_contiguous(fact);
                }
                DataMember::Variant(variant)
            }
            DataMember::Retired(identity) => DataMember::Retired(identity),
        };
        Ok(member)
    }

    fn expression_mentions_in<V>(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        members: &HashMap<String, V>,
    ) -> bool {
        match syntax.expressions.expression(expression) {
            ExpressionNode::Name {
                members: path,
                application: path_application,
            } if path_application.is_none() => {
                let [member] = syntax.expressions.identifier_path_members(path.to_owned()) else {
                    return false;
                };
                members.contains_key(member.as_str(syntax))
            }
            ExpressionNode::Binary(binary) => {
                expression_mentions_in(syntax, binary.left, members)
                    || expression_mentions_in(syntax, binary.right, members)
            }
            ExpressionNode::Unary(unary) => expression_mentions_in(syntax, unary.operand, members),
            ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => false,
            _ => true,
        }
    }

    /* Substitute and decide type equations in the complete declaration predicate. */
    pub(crate) fn decide_instance_type_equalities(
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        type_identities: &HashMap<String, ClosedArgumentIdentity>,
        selection: Option<&declaration_selection::ConstantSelection>,
        instance_name: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        if type_identities.is_empty() {
            return Ok(());
        }
        let single_name = |side: ExpressionHandle| -> Option<Identifier> {
            let ExpressionNode::Name {
                members: path,
                application: path_application,
            } = syntax.expressions.expression(side)
            else {
                return None;
            };
            if path_application.is_some() {
                return None;
            }
            let [member] = syntax.expressions.identifier_path_members(path.to_owned()) else {
                return None;
            };
            Some(member.clone())
        };
        if let ExpressionNode::Binary(binary) = syntax.expressions.expression(expression).clone() {
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                let binder = match (single_name(binary.left), single_name(binary.right)) {
                    (Some(left), Some(_)) if type_identities.contains_key(left.as_str(syntax)) => {
                        Some((left, binary.right))
                    }
                    (Some(_), Some(right))
                        if type_identities.contains_key(right.as_str(syntax)) =>
                    {
                        Some((right, binary.left))
                    }
                    _ => None,
                };
                if let Some((parameter, other)) = binder {
                    let parameter_identity = type_identities
                        .get(parameter.as_str(syntax))
                        .expect("the gate admits only listed type binders");
                    let other_identity = single_name(other)
                        .and_then(|other| closed_name_identity(syntax, selection, &other));
                    let Some(other_identity) = other_identity else {
                        return Err(vec![Diagnostic::error(format!(
                            "data constraint on generic instance `{instance_name}` cannot decide `{}`: the opposite side does not name a closed type",
                            parameter.as_str(syntax),
                        ))]);
                    };
                    let equal = *parameter_identity == other_identity;
                    let holds = match binary.operator {
                        BinaryOperator::Equal => equal,
                        BinaryOperator::NotEqual => !equal,
                        _ => unreachable!("the equality check above admits only == or !="),
                    };
                    syntax
                        .expressions
                        .replace_expression(expression, ExpressionNode::Boolean(holds));
                    return Ok(());
                }
            }
            for operand in [binary.left, binary.right] {
                decide_instance_type_equalities(
                    syntax,
                    operand,
                    type_identities,
                    selection,
                    instance_name,
                )?;
            }
            return Ok(());
        }
        if let ExpressionNode::Unary(unary) = syntax.expressions.expression(expression).clone() {
            decide_instance_type_equalities(
                syntax,
                unary.operand,
                type_identities,
                selection,
                instance_name,
            )?;
        }
        Ok(())
    }

    /*
        Fold each `self in Type::Case` atom of a copied data `requires` against
        the instance's impossible-case roster: a case the replay refuted has no
        admissible value on this carrier, so the atom is a constant `false` and
        the surrounding `&&`/`||`/`!` propagate the literals it leaves behind.
        The impossible case has no structural representation on the closed
        instance, so the atom must not survive to lowering as a membership;
        possible-case memberships stay symbolic -- `self`'s case is not known
        inside the fact, and they remain the terms the carrier must satisfy.
    */
    pub(crate) fn fold_impossible_case_memberships(
        syntax: &mut SyntaxTrees,
        expression: ExpressionHandle,
        type_name: &str,
        impossible_variants: &HashSet<String>,
    ) {
        match syntax.expressions.expression(expression).clone() {
            ExpressionNode::Membership(membership) => {
                let is_self = matches!(
                    syntax.expressions.expression(membership.value),
                    ExpressionNode::Name { members: path, application: path_application }
                        if path_application.is_none() && matches!(
                            syntax.expressions.identifier_path_members(path.to_owned()),
                            [member] if member.as_str(syntax) == "self"
                        )
                );
                let impossible_case = matches!(
                    syntax.expressions.identifier_path_members(membership.domain),
                    [head, case]
                        if head.as_str(syntax) == type_name
                            && impossible_variants.contains(case.as_str(syntax))
                );
                if is_self && impossible_case {
                    syntax
                        .expressions
                        .replace_expression(expression, ExpressionNode::Boolean(false));
                }
            }
            ExpressionNode::Binary(binary) => {
                fold_impossible_case_memberships(
                    syntax,
                    binary.left,
                    type_name,
                    impossible_variants,
                );
                fold_impossible_case_memberships(
                    syntax,
                    binary.right,
                    type_name,
                    impossible_variants,
                );
                fold_instance_boolean_node(syntax, expression);
            }
            ExpressionNode::Unary(unary) => {
                fold_impossible_case_memberships(
                    syntax,
                    unary.operand,
                    type_name,
                    impossible_variants,
                );
                fold_instance_boolean_node(syntax, expression);
            }
            _ => {}
        }
    }

    /*
        Propagate `Boolean` literals through a copied fact's boolean skeleton
        (`x && false`, `x || true`, `!true`), replacing a decided node with its
        surviving operand's subtree so the closed instance carries only the
        terms it still owes.
    */
    fn fold_instance_boolean_node(syntax: &mut SyntaxTrees, expression: ExpressionHandle) {
        let truth = |handle: ExpressionHandle| -> Option<bool> {
            match syntax.expressions.expression(handle) {
                ExpressionNode::Boolean(value) => Some(*value),
                _ => None,
            }
        };
        match syntax.expressions.expression(expression).clone() {
            ExpressionNode::Binary(binary) => {
                let decided = match binary.operator {
                    BinaryOperator::And => match (truth(binary.left), truth(binary.right)) {
                        (Some(false), _) | (_, Some(false)) => Some(ExpressionNode::Boolean(false)),
                        (Some(true), _) => {
                            Some(syntax.expressions.expression(binary.right).clone())
                        }
                        (_, Some(true)) => Some(syntax.expressions.expression(binary.left).clone()),
                        _ => None,
                    },
                    BinaryOperator::Or => match (truth(binary.left), truth(binary.right)) {
                        (Some(true), _) | (_, Some(true)) => Some(ExpressionNode::Boolean(true)),
                        (Some(false), _) => {
                            Some(syntax.expressions.expression(binary.right).clone())
                        }
                        (_, Some(false)) => {
                            Some(syntax.expressions.expression(binary.left).clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(node) = decided {
                    syntax.expressions.replace_expression(expression, node);
                }
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                if let Some(value) = truth(unary.operand) {
                    syntax
                        .expressions
                        .replace_expression(expression, ExpressionNode::Boolean(!value));
                }
            }
            _ => {}
        }
    }

    /*
        What one copied data `requires` fact says about one case holding on this
        instance: `self in DataName::Case` membership leaves decide for and
        against the candidate, literal `0`/`1` leaves (the folded type-equality
        witnesses) read themselves, `&&`/`||`/`!` fold, and every other leaf —
        value couplings, named-domain memberships, calls — stays undecided.
        `Refuted` means the fact cannot hold while the value's case is
        `variant_name`, so this instance can never carry it; `Undecided` keeps the
        case possible, since establishment and observation still own those terms.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum VariantFactVerdict {
        Holds,
        Refuted,
        Undecided,
    }

    pub(crate) fn variant_fact_verdict(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        type_name: &str,
        variant_name: &str,
    ) -> VariantFactVerdict {
        use VariantFactVerdict::*;
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(literal) => {
                if literal.decimal_value_i128() == Some(0) {
                    Refuted
                } else {
                    Holds
                }
            }
            ExpressionNode::Boolean(value) => {
                if *value {
                    Holds
                } else {
                    Refuted
                }
            }
            ExpressionNode::Membership(membership) => {
                let ExpressionNode::Name {
                    members: path,
                    application: path_application,
                } = syntax.expressions.expression(membership.value)
                else {
                    return Undecided;
                };
                if path_application.is_some() {
                    return Undecided;
                }
                let [self_member] = syntax.expressions.identifier_path_members(path.to_owned())
                else {
                    return Undecided;
                };
                if self_member.as_str(syntax) != "self" {
                    return Undecided;
                }
                match syntax_expressions(syntax).identifier_path_members(membership.domain) {
                    [head, case] if head.as_str(syntax) == type_name => {
                        if case.as_str(syntax) == variant_name {
                            Holds
                        } else {
                            Refuted
                        }
                    }
                    _ => Undecided,
                }
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                match (
                    variant_fact_verdict(syntax, binary.left, type_name, variant_name),
                    variant_fact_verdict(syntax, binary.right, type_name, variant_name),
                ) {
                    (Refuted, _) | (_, Refuted) => Refuted,
                    (Holds, Holds) => Holds,
                    _ => Undecided,
                }
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Or => {
                match (
                    variant_fact_verdict(syntax, binary.left, type_name, variant_name),
                    variant_fact_verdict(syntax, binary.right, type_name, variant_name),
                ) {
                    (Holds, _) | (_, Holds) => Holds,
                    (Refuted, Refuted) => Refuted,
                    _ => Undecided,
                }
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                match variant_fact_verdict(syntax, unary.operand, type_name, variant_name) {
                    Holds => Refuted,
                    Refuted => Holds,
                    Undecided => Undecided,
                }
            }
            _ => Undecided,
        }
    }

    /*
        The closed identity of a fact-position type name: builtins by their atom,
        declared data by item, and module-retained declarations by symbol. Mirrors
        the `TypeReferenceNode::Named` arm of `closed_argument_identity` without
        materializing a reference node.
    */
    pub(crate) fn closed_name_identity(
        syntax: &SyntaxTrees,
        selection: Option<&declaration_selection::ConstantSelection>,
        name: &Identifier,
    ) -> Option<ClosedArgumentIdentity> {
        if let Some(atom) = crate::symbols::builtin_type_atom_by_symbol_name(name.as_str(syntax)) {
            return Some(ClosedArgumentIdentity::Builtin(atom));
        }
        if let Some(declaration) = selected_data_item(syntax, selection, name) {
            Some(ClosedArgumentIdentity::Nominal(declaration))
        } else {
            Some(ClosedArgumentIdentity::RetainedNominal(
                selection?.retained_identity(syntax, name, symbols::SymbolKind::Data)?,
            ))
        }
    }

    pub(crate) fn substitute_data_field(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        mut field: syntax_trees::item::DataField,
        substitution: &HashMap<String, TypeReferenceHandle>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> syntax_trees::item::DataField {
        field.type_reference = substitute_type_reference(
            syntax,
            snapshot,
            field.type_reference,
            substitution,
            selection,
            warnings,
        );
        field
    }

    pub(crate) fn substitute_type_reference(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> TypeReferenceHandle {
        let node = syntax_type_references(syntax)
            .type_reference(type_reference)
            .clone();
        match node {
            TypeReferenceNode::Named(name) => substitution
                .get(name.as_str(syntax))
                .copied()
                .unwrap_or(type_reference),
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let argument_handles: Vec<TypeReferenceHandle> = syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .to_vec();
                let integer_types = generic_const_integer_types(syntax, base_name.as_str(syntax));
                let const_bindings: HashMap<String, i128> = substitution
                    .iter()
                    .filter_map(|(name, argument)| {
                        let value = syntax_type_references(syntax)
                            .type_reference(*argument)
                            .as_named()?;
                        Some((name.clone(), value.as_str(syntax).parse::<i128>().ok()?))
                    })
                    .collect();
                let mut substituted_arguments = Vec::with_capacity(argument_handles.len());
                for (index, argument) in argument_handles.into_iter().enumerate() {
                    let node = syntax_type_references(syntax)
                        .type_reference(argument)
                        .clone();
                    let substituted = match node {
                        TypeReferenceNode::Named(name) => substitution
                            .get(name.as_str(syntax))
                            .copied()
                            .unwrap_or(argument),
                        TypeReferenceNode::ConstExpression(expression) => {
                            let warning_watermark = warnings.len();
                            match evaluate_const_argument_expression(
                                syntax,
                                expression,
                                selection,
                                &const_bindings,
                                &HashSet::new(),
                                integer_types.get(index).copied().flatten(),
                                warnings,
                            )
                            .and_then(EvaluatedConst::into_concrete)
                            {
                                Ok(value) => syntax
                                    .tables
                                    .type_references
                                    .insert_named(Identifier::generated(value.to_string())),
                                Err(_) => {
                                    warnings.truncate(warning_watermark);
                                    argument
                                }
                            }
                        }
                        _ => substitute_type_reference(
                            syntax,
                            snapshot,
                            argument,
                            substitution,
                            selection,
                            warnings,
                        ),
                    };
                    substituted_arguments.push(substituted);
                }
                let new_span = syntax
                    .tables
                    .type_references
                    .insert_type_reference_handles(substituted_arguments);
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Generic {
                        base_name,
                        lifetime_arguments,
                        arguments: new_span,
                    })
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let substituted_element = substitute_type_reference(
                    syntax,
                    snapshot,
                    element_type,
                    substitution,
                    selection,
                    warnings,
                );
                let substituted_length = match length {
                    FixedArrayLength::Expression(value) => FixedArrayLength::Expression(
                        syntax.copy_type_reference_handle(snapshot, value),
                    ),
                    FixedArrayLength::ConstParameter(name) => substitution
                        .get(name.as_str(syntax))
                        .and_then(|argument| {
                            match syntax.tables.type_references.type_reference(*argument) {
                                TypeReferenceNode::Named(value) => {
                                    value.as_str(syntax).parse::<usize>().ok()
                                }
                                _ => None,
                            }
                        })
                        .map(FixedArrayLength::Literal)
                        .unwrap_or(FixedArrayLength::ConstParameter(name)),
                    length => length,
                };
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::FixedArray {
                        element_type: substituted_element,
                        length: substituted_length,
                    })
            }
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let referee = substitute_type_reference(
                    syntax,
                    snapshot,
                    referee,
                    substitution,
                    selection,
                    warnings,
                );
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    })
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = substitute_type_reference(
                    syntax,
                    snapshot,
                    element_type,
                    substitution,
                    selection,
                    warnings,
                );
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::Slice { element_type })
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => substitute_constrained_type_reference(
                syntax,
                snapshot,
                type_reference,
                base_type,
                constraints,
                substitution,
                selection,
                warnings,
            ),
            _ => type_reference,
        }
    }

    /*
        Substitute through one constrained type. A constraint that carries no
        substituted binder keeps the template's node so identity and provenance
        keyed on the authored occurrence still match; a `Counted<N>` domain index
        or a `0..N` range endpoint that does mention one is rebuilt onto the
        instance so the open binder never escapes its owning telescope.
    */
    fn substitute_constrained_type_reference(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        base_type: TypeReferenceHandle,
        constraints: HandleSpan<TypeConstraintNode>,
        substitution: &HashMap<String, TypeReferenceHandle>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> TypeReferenceHandle {
        let substituted_base = substitute_type_reference(
            syntax,
            snapshot,
            base_type,
            substitution,
            selection,
            warnings,
        );
        let source_constraints = syntax_type_references(syntax)
            .constraints(constraints)
            .to_vec();
        let mut changed = substituted_base != base_type;
        let mut rewritten = Vec::with_capacity(source_constraints.len());
        for constraint in source_constraints {
            let (constraint, constraint_changed) = match constraint {
                TypeConstraintNode::Domain(domain) => {
                    let arguments = syntax_type_references(syntax)
                        .type_reference_handles(domain.arguments)
                        .to_vec();
                    let mut domain_changed = false;
                    let mut substituted_arguments = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        let substituted = substitute_domain_index_argument(
                            syntax,
                            snapshot,
                            argument,
                            substitution,
                            selection,
                            warnings,
                        );
                        domain_changed |= substituted != argument;
                        substituted_arguments.push(substituted);
                    }
                    if domain_changed {
                        let arguments = syntax
                            .tables
                            .type_references
                            .insert_type_reference_handles(substituted_arguments);
                        (
                            TypeConstraintNode::Domain(DomainConstraint {
                                name: domain.name,
                                arguments,
                            }),
                            true,
                        )
                    } else {
                        (TypeConstraintNode::Domain(domain), false)
                    }
                }

                constraint => (constraint, false),
            };
            changed |= constraint_changed;
            rewritten.push(constraint);
        }
        if !changed {
            return type_reference;
        }
        let constraints = syntax.tables.type_references.insert_constraints(rewritten);
        syntax
            .tables
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type: substituted_base,
                constraints,
            })
    }

    /*
        Substitute one domain-index argument inside a constrained type. A bare
        `N` takes the closed argument handle directly; an open `ConstExpression`
        is copied out of the snapshot so the caller's const-binder rewrite lands
        on the instance's own subtree and the authored operator spelling (and any
        retained normalization provenance on the template node) is preserved;
        anything richer recurses through the ordinary substitution.
    */
    fn substitute_domain_index_argument(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        argument: TypeReferenceHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> TypeReferenceHandle {
        match syntax_type_references(syntax)
            .type_reference(argument)
            .clone()
        {
            TypeReferenceNode::Named(name) => substitution
                .get(name.as_str(syntax))
                .copied()
                .unwrap_or(argument),
            TypeReferenceNode::ConstExpression(expression) => {
                if !expression_mentions_parameter(syntax, expression, substitution) {
                    return argument;
                }
                let copied = syntax.copy_expression_from(snapshot, expression);
                syntax
                    .tables
                    .type_references
                    .insert(TypeReferenceNode::ConstExpression(copied))
            }
            _ => substitute_type_reference(
                syntax,
                snapshot,
                argument,
                substitution,
                selection,
                warnings,
            ),
        }
    }

    /*
        Whether an expression mentions a bare name the substitution rewrites. Only
        the expression shapes generic substitution rebuilds leaf-by-leaf unfold;
        anything richer conservatively counts as a mention so the endpoint or index
        is copied rather than shared into the instance unchanged.
    */
    fn expression_mentions_parameter(
        syntax: &SyntaxTrees,
        expression: ExpressionHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) -> bool {
        expression_mentions_in(syntax, expression, substitution)
    }

    /*
        Whether a type reference mentions any of the substituted parameter names
        (recursively through composite nodes). Conservative: on an unhandled node
        shape it returns `true` so the caller rejects rather than silently sharing a
        parameter-bearing type.
    */
    pub(crate) fn type_reference_mentions_parameter(
        syntax: &SyntaxTrees,
        handle: TypeReferenceHandle,
        substitution: &HashMap<String, TypeReferenceHandle>,
    ) -> bool {
        match syntax.tables.type_references.type_reference(handle) {
            TypeReferenceNode::Named(name) => substitution.contains_key(name.as_str(syntax)),
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let head = base_name
                    .as_str(syntax)
                    .split_once("::")
                    .map_or(base_name.as_str(syntax), |(head, _)| head);
                substitution.contains_key(head)
                    || syntax_type_references(syntax)
                        .type_reference_handles(*arguments)
                        .iter()
                        .any(|&argument| {
                            type_reference_mentions_parameter(syntax, argument, substitution)
                        })
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                type_reference_mentions_parameter(syntax, *base_type, substitution)
                    || syntax_type_references(syntax)
                        .constraints(*constraints)
                        .iter()
                        .any(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                                .type_reference_handles(domain.arguments)
                                .iter()
                                .any(|&argument| {
                                    type_reference_mentions_parameter(
                                        syntax,
                                        argument,
                                        substitution,
                                    )
                                }),

                            TypeConstraintNode::Named(_)
                            | TypeConstraintNode::ArithmeticDomain(_) => false,
                        })
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                type_reference_mentions_parameter(syntax, *element_type, substitution)
            }
            TypeReferenceNode::Slice { element_type } => {
                type_reference_mentions_parameter(syntax, *element_type, substitution)
            }
            TypeReferenceNode::Reference { referee, .. } => {
                type_reference_mentions_parameter(syntax, *referee, substitution)
            }
            _ => true,
        }
    }

    /*
        Substitute the complete static application selected by a data default
        domain. Arguments keep their structural kind, including nested applications;
        the declaration path itself remains an exact authored selection.
    */
    pub(super) fn substitute_cleanup_selection(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        mut cleanup: syntax_trees::expression::StaticMachineArgument,
        substitution: &HashMap<String, TypeReferenceHandle>,
        selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
        warnings: &mut Vec<Diagnostic>,
    ) -> syntax_trees::expression::StaticMachineArgument {
        if cleanup.type_reference.is_valid() {
            cleanup.type_reference = substitute_type_reference(
                syntax,
                snapshot,
                cleanup.type_reference,
                substitution,
                selection,
                warnings,
            );
        } else if let [name] = cleanup.path.as_ref()
            && cleanup.application.is_none()
            && let Some(actual) = substitution.get(name.as_str(syntax))
        {
            cleanup.type_reference = *actual;
            cleanup.path = Box::default();
        }
        if let Some(application) = &mut cleanup.application {
            for argument in application.arguments.iter_mut() {
                *argument = substitute_cleanup_selection(
                    syntax,
                    snapshot,
                    argument.clone(),
                    substitution,
                    selection,
                    warnings,
                );
            }
        }
        cleanup
    }
}
mod synthesis {
    /* Closed-instance discovery, fixed-point synthesis and the ordered rewrite. */
    use crate::preparation::generic_data::ClosedArgumentIdentity;
    use crate::preparation::generic_data::ConstFactValue;
    use crate::preparation::generic_data::ConstScalarValue;
    use crate::preparation::generic_data::GenericData;
    use crate::preparation::generic_data::Instantiation;
    use crate::preparation::generic_data::PendingRewrite;
    use crate::preparation::generic_data::canonicalize_closed_domain_indices;
    use crate::preparation::generic_data::capture_machine_runtime_template_names;
    use crate::preparation::generic_data::uses::{
        closed_constructor_carrier, relabel_closed_data_uses_in_constants,
        relabel_closed_data_uses_in_exact_assignments,
        relabel_closed_data_uses_in_exact_calls_and_returns,
        relabel_closed_sum_memberships_from_local_types,
    };
    use crate::tables::syntax_items;
    use crate::tables::syntax_type_references;

    use super::substitution::substitute_copied_type_names;
    use super::work_domain::{GenericDataWorkDomain, SynthesisWork, TypePositionPurpose};
    use crate::preparation::generic_data::evaluate_const_fact_expression;
    use crate::preparation::generic_data::evaluate_const_membership_fact;
    use crate::preparation::generic_data::normalize_domain_head_positions;
    use crate::preparation::generic_data::normalize_generic_template_const_expressions;
    use crate::preparation::generic_data::prepare_closed_application;

    use super::source_modules::module_path;
    use super::substitution::fold_impossible_case_memberships;
    use super::substitution::variant_fact_verdict;
    use crate::preparation::declaration_selection::ConstantSelection;
    use crate::preparation::generic_data::replace_const_expression_names_from;
    use crate::preparation::generic_data::replace_machine_const_expression_names_from;
    use crate::preparation::generic_data::substitute_member;
    use crate::preparation::type_equations::normalized_boolean_argument;
    use arena::Handle;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use numerics::literals::IntegerLiteral;
    use numerics::literals::IntegerRadix;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use syntax_trees::SyntaxTrees;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::expression::ExpressionNode;
    use syntax_trees::identifier::Identifier;
    use syntax_trees::item::DataDefinition;
    use syntax_trees::item::DataMember;
    use syntax_trees::item::Item;
    use syntax_trees::item::ProofFact;
    use syntax_trees::item::TypeParameterKind;
    use syntax_trees::types::FixedArrayLength;
    use syntax_trees::types::TypeReferenceHandle;
    use syntax_trees::types::TypeReferenceNode;

    pub(in crate::preparation::generic_data) fn desugar_generic_data_instances(
        syntax: &mut SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<(), Vec<Diagnostic>> {
        let selection = ConstantSelection::new(syntax, None, Vec::new())?;
        super::domain_heads::normalize(syntax, Some(&selection))?;
        desugar_generic_data_instances_with_selection(syntax, warnings, Some(&selection), &[])
    }

    pub(in crate::preparation) fn desugar_generic_data_instances_with_selection(
        syntax: &mut SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
        selection: Option<&ConstantSelection>,
        closed_roots: &[TypeReferenceHandle],
    ) -> Result<(), Vec<Diagnostic>> {
        desugar_generic_data_instances_in(
            syntax,
            warnings,
            selection,
            GenericDataWorkDomain::All {
                additional_type_roots: closed_roots,
            },
        )
    }

    pub(crate) fn desugar_generic_data_instances_in(
        syntax: &mut SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
        selection: Option<&ConstantSelection>,
        domain: GenericDataWorkDomain<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        if let GenericDataWorkDomain::Selected {
            type_roots,
            declarations,
        } = domain
            && type_roots.is_empty()
            && declarations.is_empty()
        {
            return Ok(());
        }
        let work = SynthesisWork::new(syntax, domain);
        let mut attached_machines: HashMap<syntax_trees::item::ItemHandle, Vec<usize>> =
            HashMap::new();
        let roots = crate::preparation::declaration_selection::RootDeclarations::of(syntax);
        for (item_index, item) in syntax.root_items().enumerate() {
            if let Some(machine) = item.as_machine()
                && let Some(attached) = &machine.attached_data
                && let Some(declaration) =
                    super::discovery::selected_data_item_in(&roots, selection, attached)
            {
                attached_machines
                    .entry(declaration)
                    .or_default()
                    .push(item_index);
            }
        }

        let mut generic_data: HashMap<syntax_trees::item::ItemHandle, GenericData> = HashMap::new();
        for &declaration in syntax.root_item_handles() {
            let Some(definition) = syntax.root_item(declaration).as_data() else {
                continue;
            };
            if definition.type_parameters.is_empty() {
                continue;
            }
            if definition.quotient.is_some() || !definition.conformance_bounds.is_empty() {
                continue;
            }
            if matches!(
                definition.name.as_str(syntax),
                "IntervalSet" | "CountedQuantity"
            ) && module_path(syntax, definition.name.source_span().source_id).is_none()
            {
                continue;
            }
            let definition_parameters =
                syntax_items(syntax).type_parameters(definition.type_parameters);
            if definition_parameters.iter().any(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Value { .. } | TypeParameterKind::Machine { .. }
                )
            }) {
                continue;
            }
            if attached_machines.contains_key(&declaration) {
                let data_parameters: Vec<(String, bool)> = syntax_items(syntax)
                    .type_parameters(definition.type_parameters)
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.as_str(syntax).to_string(),
                            parameter.kind.is_const_or_value(),
                        )
                    })
                    .collect();
                let all_methods_covered =
                    attached_machines[&declaration].iter().all(|&item_index| {
                        let Some(Item::Machine(machine)) = syntax.root_items().nth(item_index)
                        else {
                            return false;
                        };
                        syntax_items(syntax)
                            .type_parameters(machine.type_parameters)
                            .iter()
                            .all(|parameter| {
                                let method_is_const = parameter.kind.is_const_or_value();
                                data_parameters.iter().any(|(name, data_is_const)| {
                                    name == parameter.name.as_str(syntax)
                                        && *data_is_const == method_is_const
                                })
                            })
                    });
                if !all_methods_covered {
                    continue;
                }
            }
            let parameter_names = definition_parameters
                .iter()
                .map(|parameter| parameter.name.as_str(syntax).to_string())
                .collect::<Vec<_>>();
            let const_parameter_types = definition_parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => Some(type_reference),
                    _ => None,
                })
                .collect();
            generic_data.insert(
                declaration,
                GenericData {
                    declaration,
                    name: definition.name.as_str(syntax).to_owned(),
                    origin_name: definition.name.clone(),
                    is_public: definition.is_public,
                    lifetime_parameters: definition.lifetime_parameters.clone(),
                    parameter_names,
                    const_parameter_types,
                    cleanup_selection: definition.cleanup_selection.clone(),
                    requires_facts: definition.requires_facts,
                    type_equations: super::equations::classify_type_equations(
                        syntax,
                        definition_parameters,
                        syntax.tables.items.proof_facts(definition.requires_facts),
                    ),
                    members: definition.members,
                    properties: definition.properties,
                    supply_mode: definition.supply_mode,
                },
            );
        }
        let mut domain_head_positions =
            work.type_positions(syntax, TypePositionPurpose::Identity, true);
        for item in syntax.root_items().filter(|_| work.is_all()) {
            let Some(definition) = item.as_data() else {
                continue;
            };
            if definition.type_parameters.is_empty() {
                continue;
            }
            for member in syntax.items.data_members(definition.members) {
                match member {
                    DataMember::Field(field) => super::collect_type_positions(
                        syntax,
                        field.type_reference,
                        &mut domain_head_positions,
                        true,
                    ),
                    DataMember::Variant(variant) => {
                        for field in syntax.items.data_payload_fields(variant.payload) {
                            super::collect_type_positions(
                                syntax,
                                field.type_reference,
                                &mut domain_head_positions,
                                true,
                            );
                        }
                    }
                    DataMember::Retired(_) => {}
                }
            }
        }
        normalize_domain_head_positions(syntax, selection, &domain_head_positions);

        canonicalize_closed_domain_indices(syntax, selection, warnings, &work)
            .map_err(|diagnostic| vec![diagnostic])?;

        if generic_data.is_empty() {
            return Ok(());
        }
        let mut synthesized = Vec::new();
        for &declaration in syntax.root_item_handles() {
            let Some(definition) = syntax.root_item(declaration).as_data() else {
                continue;
            };
            let Some(origin) = definition.generic_instance else {
                continue;
            };
            let Some(ClosedArgumentIdentity::Instance(template, argument_identity)) =
                super::closed_argument_identity(syntax, selection, origin, false)
            else {
                return Err(vec![
                Diagnostic::error(format!(
                    "existing generic instance `{}` lost its exact template and argument identity",
                    definition.name.as_str(syntax),
                ))
                .with_source_span(definition.name.source_span()),
            ]);
            };
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(origin)
            else {
                return Err(vec![Diagnostic::error(
                    "existing generic instance lost its original application structure",
                )]);
            };
            synthesized.push(Instantiation {
                synthetic_name: definition.name.as_str(syntax).to_owned(),
                declaration,
                template,
                argument_handles: syntax_type_references(syntax)
                    .type_reference_handles(*arguments)
                    .to_vec(),
                argument_identity,
            });
        }
        loop {
            if !work.is_all() {
                let domain_positions =
                    work.type_positions(syntax, TypePositionPurpose::Identity, true);
                normalize_domain_head_positions(syntax, selection, &domain_positions);
                canonicalize_closed_domain_indices(syntax, selection, warnings, &work)
                    .map_err(|diagnostic| vec![diagnostic])?;
            }
            let storage_positions: std::collections::HashSet<TypeReferenceHandle> = work
                .type_positions(syntax, TypePositionPurpose::Materialization, false)
                .into_iter()
                .collect();
            let mut rewrites: Vec<PendingRewrite> = Vec::new();
            let mut instantiations: Vec<Instantiation> = Vec::new();
            for position in work.type_positions(syntax, TypePositionPurpose::Identity, false) {
                let Some(application) = prepare_closed_application(
                    syntax,
                    &generic_data,
                    selection,
                    position,
                    warnings,
                )
                .map_err(|diagnostic| vec![diagnostic])?
                else {
                    continue;
                };
                let prior = synthesized
                    .iter()
                    .chain(instantiations.iter())
                    .find(|instance| {
                        instance.template == application.template
                            && instance.argument_identity == application.argument_identity
                    });
                if prior.is_none() && !storage_positions.contains(&position) {
                    continue;
                }
                let synthetic_name = if let Some(prior) = prior {
                    prior.synthetic_name.clone()
                } else {
                    let display = format!(
                        "{}<{}>",
                        generic_data[&application.template].name,
                        application.argument_names.join(", ")
                    );
                    if synthesized
                        .iter()
                        .chain(instantiations.iter())
                        .any(|instance| instance.synthetic_name == display)
                    {
                        format!("{display}__{}", synthesized.len() + instantiations.len())
                    } else {
                        display
                    }
                };
                let lookup_name = if let Some(selection) = selection {
                    let Some(path) = selection.data_lookup_path(syntax, &application.base_name)
                    else {
                        continue;
                    };
                    if let Some((prefix, _)) = path.rsplit_once("::") {
                        format!("{prefix}::{synthetic_name}")
                    } else {
                        synthetic_name.clone()
                    }
                } else {
                    synthetic_name.clone()
                };
                rewrites.push(PendingRewrite {
                    type_reference: position,
                    synthetic_name: lookup_name,
                    lifetime_arguments: application.lifetime_arguments,
                });
                if prior.is_none() {
                    instantiations.push(Instantiation {
                        synthetic_name,
                        declaration: Handle::invalid(),
                        template: application.template,
                        argument_handles: application.argument_handles,
                        argument_identity: application.argument_identity,
                    });
                }
            }
            if rewrites.is_empty() {
                break;
            }
            if !instantiations.is_empty() {
                let snapshot = syntax.clone();
                for instance in &instantiations {
                    synthesize_instance(
                        syntax,
                        &snapshot,
                        warnings,
                        &mut synthesized,
                        &generic_data,
                        &attached_machines,
                        selection,
                        instance,
                    )?;
                }
            }

            for rewrite in rewrites {
                let original = syntax_type_references(syntax)
                    .type_reference(rewrite.type_reference)
                    .clone();
                let TypeReferenceNode::Generic { base_name, .. } = &original else {
                    continue;
                };
                let instance_name =
                    Identifier::new(rewrite.synthetic_name, base_name.source_span());
                let application = syntax.tables.type_references.insert(original);
                syntax
                    .tables
                    .type_references
                    .retain_generic_application_origin(rewrite.type_reference, application);
                let rewritten = if rewrite.lifetime_arguments.is_empty() {
                    TypeReferenceNode::Named(instance_name)
                } else {
                    TypeReferenceNode::Generic {
                        base_name: instance_name,
                        lifetime_arguments: rewrite.lifetime_arguments,
                        arguments: HandleSpan::empty(),
                    }
                };
                syntax
                    .tables
                    .type_references
                    .replace_type_reference(rewrite.type_reference, rewritten);
            }
        }

        let declarations = work.declarations(syntax);
        relabel_closed_data_uses_in_constants(syntax, &synthesized, selection, &declarations);
        relabel_closed_data_uses_in_exact_assignments(
            syntax,
            &synthesized,
            selection,
            &declarations,
        );
        relabel_closed_data_uses_in_exact_calls_and_returns(
            syntax,
            &synthesized,
            selection,
            &declarations,
        );
        relabel_closed_sum_memberships_from_local_types(
            syntax,
            &synthesized,
            selection,
            &declarations,
        );

        normalize_generic_template_const_expressions(syntax, selection, warnings, &declarations)
            .map_err(|diagnostic| vec![diagnostic])?;
        Ok(())
    }

    /*
        Build one closed instance of a generic data declaration, unless an
        earlier round already built the same template and argument tuple: the
        template's members, facts and attached machines with its type and
        `const` parameters substituted, as a new concrete record.
    */
    fn synthesize_instance(
        syntax: &mut SyntaxTrees,
        snapshot: &SyntaxTrees,
        warnings: &mut Vec<Diagnostic>,
        synthesized: &mut Vec<Instantiation>,
        generic_data: &HashMap<syntax_trees::item::ItemHandle, GenericData>,
        attached_machines: &HashMap<syntax_trees::item::ItemHandle, Vec<usize>>,
        selection: Option<&ConstantSelection>,
        instance: &Instantiation,
    ) -> Result<(), Vec<Diagnostic>> {
        if synthesized.iter().any(|prior| {
            prior.template == instance.template
                && prior.argument_identity == instance.argument_identity
        }) {
            return Ok(());
        }
        let base_info = &generic_data[&instance.template];
        let substitution: HashMap<String, TypeReferenceHandle> = base_info
            .parameter_names
            .iter()
            .cloned()
            .zip(instance.argument_handles.iter().copied())
            .collect();
        let const_parameter_values: HashMap<String, i128> = base_info
            .parameter_names
            .iter()
            .zip(&base_info.const_parameter_types)
            .filter_map(|(name, parameter_type)| {
                parameter_type.as_ref()?;
                let argument = substitution.get(name)?;
                let value = syntax_type_references(syntax)
                    .type_reference(*argument)
                    .as_named()?;
                Some((name.clone(), value.as_str(syntax).parse().ok()?))
            })
            .collect();
        let const_parameter_type_names: HashMap<String, String> = base_info
            .parameter_names
            .iter()
            .zip(&base_info.const_parameter_types)
            .filter_map(|(name, parameter_type)| {
                let parameter_type = parameter_type.as_ref()?;
                let type_name = syntax_type_references(syntax)
                    .type_reference(*parameter_type)
                    .as_named()?;
                Some((name.clone(), type_name.as_str(syntax).to_string()))
            })
            .collect();
        let mut const_expressions: HashMap<String, ExpressionNode> = const_parameter_values
            .iter()
            .map(|(name, value)| {
                use numerics::literals::{IntegerLanding, LandedIntegerType};
                let Some(landed_type) = const_parameter_type_names
                    .get(name)
                    .map(String::as_str)
                    .and_then(LandedIntegerType::from_name)
                else {
                    return Err(vec![Diagnostic::error(format!(
                        "const parameter `{name}` has no supported integer carrier for substitution"
                    ))]);
                };
                let literal = IntegerLiteral::from_parts(
                    *value < 0,
                    IntegerRadix::Decimal,
                    value.unsigned_abs().to_string().as_str(),
                )
                .expect("a concrete const argument is a valid decimal integer literal")
                .with_landing(IntegerLanding {
                    landed_type,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                });
                Ok((name.clone(), ExpressionNode::Integer(literal)))
            })
            .collect::<Result<_, Vec<Diagnostic>>>()?;
        let mut const_parameter_scalars = const_parameter_values
            .iter()
            .map(|(name, value)| (name.clone(), ConstScalarValue::Integer(*value)))
            .collect::<HashMap<_, _>>();
        for (name, parameter_type) in base_info
            .parameter_names
            .iter()
            .zip(&base_info.const_parameter_types)
        {
            if parameter_type.is_none() {
                continue;
            }
            let Some(argument) = substitution.get(name) else {
                continue;
            };
            let Some(value) = syntax.type_references.type_reference(*argument).as_named() else {
                continue;
            };
            if let Some(value) = normalized_boolean_argument(value.as_str(syntax)) {
                const_parameter_scalars.insert(name.clone(), ConstScalarValue::Boolean(value));
                const_expressions.insert(name.clone(), ExpressionNode::Boolean(value));
            }
        }

        let type_identities: HashMap<String, ClosedArgumentIdentity> = {
            let Some(template) = snapshot.root_item(instance.template).as_data() else {
                unreachable!("a discovered template is a data item");
            };
            syntax_items(snapshot)
                .type_parameters(template.type_parameters)
                .iter()
                .zip(instance.argument_identity.iter())
                .filter(|(parameter, _)| matches!(parameter.kind, TypeParameterKind::Type))
                .map(|(parameter, identity)| {
                    (parameter.name.as_str(syntax).to_owned(), identity.clone())
                })
                .collect()
        };

        let type_watermark = syntax.type_references.node_count();
        let fact_expression_watermark = syntax.expressions.frontier();
        let template_variant_names: Vec<String> = syntax_items(snapshot)
            .data_members(base_info.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) => Some(variant.name.as_str(snapshot).to_owned()),
                _ => None,
            })
            .collect();
        let mut impossible_variants: HashSet<String> = HashSet::new();
        let mut copied_fact_roots: Vec<ExpressionHandle> = Vec::new();
        let mut first_fact = Handle::invalid();
        let mut fact_count = 0u32;
        for (offset, fact) in syntax_items(snapshot)
            .proof_facts(base_info.requires_facts)
            .iter()
            .enumerate()
        {
            if base_info
                .type_equations
                .iter()
                .any(|equation| equation.fact_offset == offset)
            {
                continue;
            }
            let fact_warning_start = warnings.len();
            let const_result = match fact {
                ProofFact::Expression(expression) => evaluate_const_fact_expression(
                    snapshot,
                    *expression,
                    &const_parameter_scalars,
                    selection,
                    warnings,
                )
                .map(|value| match value {
                    Some(ConstFactValue::Boolean(value)) => Some(value),
                    _ => None,
                }),
                ProofFact::Membership(membership) => evaluate_const_membership_fact(
                    snapshot,
                    membership,
                    &const_parameter_values,
                    &const_parameter_type_names,
                    selection,
                    warnings,
                ),
            }
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "const fact for generic instance `{}` is invalid: {reason}",
                    instance.synthetic_name
                ))]
            })?;
            if const_result == Some(true) {
                continue;
            }
            warnings.truncate(fact_warning_start);
            let source = Handle::from_parts(
                base_info
                    .requires_facts
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                    .expect("proof fact source handle overflow"),
                base_info.requires_facts.start().generation(),
            );
            let handle = syntax.copy_proof_fact_from(snapshot, source);
            let copied_root = match syntax.items.proof_fact(handle) {
                ProofFact::Expression(root) => Some(*root),
                _ => None,
            };
            if let Some(copied_root) = copied_root {
                super::substitution::decide_instance_type_equalities(
                    syntax,
                    copied_root,
                    &type_identities,
                    selection,
                    &instance.synthetic_name,
                )?;
                copied_fact_roots.push(copied_root);
                let type_name = base_info.origin_name.as_str(syntax);
                for variant_name in &template_variant_names {
                    if matches!(
                        variant_fact_verdict(syntax, copied_root, type_name, variant_name,),
                        super::substitution::VariantFactVerdict::Refuted
                    ) {
                        impossible_variants.insert(variant_name.clone());
                    }
                }
            }
            if fact_count == 0 {
                first_fact = handle;
            }
            fact_count += 1;
        }
        if !impossible_variants.is_empty() {
            let type_name = base_info.origin_name.as_str(syntax).to_owned();
            for copied_root in copied_fact_roots {
                fold_impossible_case_memberships(
                    syntax,
                    copied_root,
                    &type_name,
                    &impossible_variants,
                );
            }
        }
        let requires_facts = HandleSpan::from_parts(first_fact, fact_count);

        let origin_arguments = syntax
            .tables
            .type_references
            .insert_type_reference_handles(instance.argument_handles.iter().copied());
        let generic_instance = syntax
            .tables
            .type_references
            .insert(TypeReferenceNode::Generic {
                base_name: base_info.origin_name.clone(),
                lifetime_arguments: Vec::new(),
                arguments: origin_arguments,
            });

        let members: Vec<DataMember> = syntax.tables.items.data_members(base_info.members).to_vec();
        let properties = base_info.properties;
        let mut first: Handle<DataMember> = Handle::invalid();
        let mut count = 0u32;
        for member in members {
            let substituted = substitute_member(
                syntax,
                snapshot,
                member,
                &substitution,
                &impossible_variants,
                selection,
                warnings,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
            let handle = syntax.tables.items.append_data_member(substituted);
            if count == 0 {
                first = handle;
            }
            count += 1;
        }
        let copied_types = syntax_type_references(syntax).named_nodes_from(type_watermark, syntax);
        substitute_copied_type_names(syntax, copied_types, &substitution);
        replace_const_expression_names_from(syntax, fact_expression_watermark, &const_expressions);
        let cleanup_selection = base_info.cleanup_selection.clone().map(|cleanup| {
            super::substitution::substitute_cleanup_selection(
                syntax,
                snapshot,
                cleanup,
                &substitution,
                selection,
                warnings,
            )
        });
        let declaration = syntax.push_root_item(Item::Data(DataDefinition {
            name: Identifier::new(
                instance.synthetic_name.as_str(),
                base_info.origin_name.source_span(),
            ),
            is_public: base_info.is_public,
            supply_mode: base_info.supply_mode,
            lifetime_parameters: base_info.lifetime_parameters.clone(),
            type_parameters: HandleSpan::default(),
            conformance_bounds: Vec::new(),
            generic_instance: Some(generic_instance),
            cleanup_selection,
            properties,
            requires_facts,
            members: HandleSpan::from_parts(first, count),
            quotient: None,
        }));

        let mut materialized = instance.clone();
        materialized.declaration = declaration;
        synthesized.push(materialized);

        let Some(machine_items) = attached_machines.get(&instance.template) else {
            return Ok(());
        };
        for &item_index in machine_items {
            let Some(Item::Machine(machine)) = snapshot.root_items().nth(item_index) else {
                continue;
            };
            let type_watermark = syntax.tables.type_references.node_count();
            let expression_watermark = syntax.expressions.frontier();
            let Item::Machine(mut clone) =
                syntax.copy_item_from(snapshot, &Item::Machine(machine.clone()))
            else {
                continue;
            };
            let method_tail = machine
                .name
                .as_str(syntax)
                .rsplit("::")
                .next()
                .unwrap_or(machine.name.as_str(syntax))
                .to_string();
            clone.name = Identifier::new(
                format!("{}::{}", instance.synthetic_name, method_tail),
                machine.name.source_span(),
            );
            clone.attached_data = Some(
                closed_constructor_carrier(
                    snapshot,
                    selection,
                    instance,
                    machine.attached_data.as_ref().expect("selected attachment"),
                    false,
                )
                .ok_or_else(|| {
                    vec![
                        Diagnostic::error(
                            "generic method lost its selected closed carrier lookup context",
                        )
                        .with_source_span(machine.name.source_span()),
                    ]
                })?,
            );
            clone.generic_data_template = machine.name.clone();
            clone.type_parameters = HandleSpan::default();
            let runtime_captures = capture_machine_runtime_template_names(
                syntax,
                &clone,
                type_watermark,
                expression_watermark,
            );
            let copied_types = syntax_type_references(syntax)
                .named_nodes_from(type_watermark, syntax)
                .into_iter()
                .filter(|(handle, name)| {
                    !const_expressions.contains_key(name)
                        || !runtime_captures.captures_type_reference(*handle)
                });
            substitute_copied_type_names(syntax, copied_types, &substitution);
            for (handle, element_type, name) in syntax_type_references(syntax)
                .const_parameter_array_nodes_from(type_watermark, syntax)
            {
                let Some(length) = substitution.get(&name).and_then(|argument| {
                    match syntax.tables.type_references.type_reference(*argument) {
                        TypeReferenceNode::Named(value) => {
                            value.as_str(syntax).parse::<usize>().ok()
                        }
                        _ => None,
                    }
                }) else {
                    continue;
                };
                syntax.tables.type_references.replace_type_reference(
                    handle,
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    },
                );
            }
            replace_machine_const_expression_names_from(
                syntax,
                expression_watermark,
                &const_expressions,
                &runtime_captures,
            );
            syntax.push_root_item(Item::Machine(clone));
        }
        Ok(())
    }
}
#[path = "preparation_generic_uses.rs"]
mod uses;
mod work_domain {
    /*
        Synthesis reads the full declaration context but mutates only owned work.

        Equation completion introduces exact type roots after earlier declarations
        have been published. Those declarations remain lookup context, not another
        normalization worklist. New instances and their attached machines extend the
        same selected domain so nested applications and constructor uses settle by
        the ordinary fixed point without changing an unrelated retained declaration.
        Authored applications are a finite root roster. A synthesized field adds
        inline storage edges; a reference target remains identity. Argument admission
        is independent of copying a declaration. Data and case predicates describe
        default domains, not a requirement that every named application be inhabited.
        Typed elaboration owns the later field views of identity-only applications.
    */

    use super::const_evaluation::collect_selected_type_reference_positions;
    use super::const_evaluation::collect_stored_type_reference_positions;
    use syntax_trees::expression::ExpressionFrontier;
    use syntax_trees::expression::ExpressionHandle;
    use syntax_trees::{SyntaxTrees, item::ItemHandle, types::TypeReferenceHandle};

    #[derive(Clone, Copy)]
    pub(crate) enum GenericDataWorkDomain<'input> {
        All {
            additional_type_roots: &'input [TypeReferenceHandle],
        },
        Selected {
            type_roots: &'input [TypeReferenceHandle],
            declarations: &'input [ItemHandle],
        },
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum TypePositionPurpose {
        Identity,
        Materialization,
    }

    pub(super) struct SynthesisWork<'input> {
        domain: GenericDataWorkDomain<'input>,
        declaration_frontier: usize,
        expression_frontier: ExpressionFrontier,
    }

    impl<'input> SynthesisWork<'input> {
        pub(super) fn new(syntax: &SyntaxTrees, domain: GenericDataWorkDomain<'input>) -> Self {
            Self {
                domain,
                declaration_frontier: syntax.root_item_handles().len(),
                expression_frontier: syntax.expressions.frontier(),
            }
        }

        pub(super) fn is_all(&self) -> bool {
            matches!(self.domain, GenericDataWorkDomain::All { .. })
        }

        /*
            Synthesis also copies data facts and other declaration-side expressions,
            not just machine bodies. Their new arena suffix belongs to this work
            even when no executable machine inventory reaches them.
        */
        pub(super) fn owns_new_expression(&self, expression: ExpressionHandle) -> bool {
            expression.is_valid() && self.expression_frontier.contains(expression)
        }

        pub(super) fn declarations(&self, syntax: &SyntaxTrees) -> Vec<ItemHandle> {
            match self.domain {
                GenericDataWorkDomain::All { .. } => syntax.root_item_handles().to_vec(),
                GenericDataWorkDomain::Selected { declarations, .. } => {
                    let mut selected = declarations.to_vec();
                    for &declaration in &syntax.root_item_handles()[self.declaration_frontier..] {
                        if !selected.contains(&declaration) {
                            selected.push(declaration);
                        }
                    }
                    selected
                }
            }
        }

        pub(super) fn type_positions(
            &self,
            syntax: &SyntaxTrees,
            purpose: TypePositionPurpose,
            include_domain_arguments: bool,
        ) -> Vec<TypeReferenceHandle> {
            let declarations = if purpose == TypePositionPurpose::Identity {
                self.declarations(syntax)
            } else {
                match self.domain {
                    GenericDataWorkDomain::All { .. } => {
                        syntax.root_item_handles()[..self.declaration_frontier].to_vec()
                    }
                    GenericDataWorkDomain::Selected { declarations, .. } => declarations.to_vec(),
                }
            };
            let mut positions = collect_selected_type_reference_positions(syntax, &declarations);
            if purpose == TypePositionPurpose::Materialization {
                positions.extend(collect_stored_type_reference_positions(
                    syntax,
                    &syntax.root_item_handles()[self.declaration_frontier..],
                ));
            }
            let roots = match self.domain {
                GenericDataWorkDomain::All {
                    additional_type_roots,
                } => additional_type_roots,
                GenericDataWorkDomain::Selected { type_roots, .. } => type_roots,
            };
            for &root in roots {
                super::collect_type_positions(
                    syntax,
                    root,
                    &mut positions,
                    include_domain_arguments,
                );
            }
            positions
        }
    }
}

pub(super) use uses::{
    collect_expression_handles, collect_statement_expression_handles, visit_expression_type_inputs,
};
pub(crate) use uses::{statement_call_inputs, statement_type_inputs};

use crate::preparation::module_normalization::validate_with_const_resolution_mode;
pub(crate) use arguments::{
    DomainCarrier, DomainHead, closed_argument_identity, closed_parameter_argument_identity,
    domain_application_head, domain_index_telescope, monomorphizable_argument_slugs,
    normalize_domain_head_positions, selected_domain_family, type_reference_slug,
};
pub(in crate::preparation::generic_data) use arguments::{
    canonicalize_monomorphizable_argument_handles, complete_argument_tuple,
};
pub(crate) use const_evaluation::{
    ConstFactValue, ConstScalarSpelling, ConstScalarValue, closed_fixed_array_length,
    evaluate_const_fact_binary, evaluate_const_fact_expression, evaluate_const_membership_fact,
    fold_closed_scalar_initializers, prove_const_literal_predicates,
};
pub(in crate::preparation::generic_data) use const_evaluation::{
    ConstPredicateAdmission, EvaluatedConst, canonicalize_closed_domain_indices,
    canonicalize_selected_declaration_value,
    canonicalize_selected_declaration_value_with_admission, canonicalize_selected_index_expression,
    capture_machine_runtime_template_names, checked_fact_integer,
    collect_callable_type_reference_positions, collect_data_type_reference_positions,
    collect_selected_type_reference_positions, const_integer_in_envelope, const_integer_type,
    domain_index_parameters, evaluate_const_argument_expression, generic_const_integer_types,
    integer_literal_value, normalize_generic_template_const_expressions,
    prepare_closed_application, prove_const_leaf_domain_constraints, qualified_const_name,
    replace_const_expression_names_from, replace_machine_const_expression_names_from,
    syntax_type_identity, validate_const_index_type, validate_syntax_integer_range,
};
pub(in crate::preparation) use const_evaluation::{
    canonicalize_selected_const_definition, collect_type_positions,
};
use declaration_selection::ConstantSelection;
pub(crate) use discovery::{ClosedArgumentIdentity, ClosedConstraintIdentity, selected_data_item};
pub(in crate::preparation::generic_data) use discovery::{
    GenericData, GenericDataShape, Instantiation, PendingRewrite, selected_generic_data,
};
pub(crate) use substitution::{
    closed_name_identity, substitute_member, type_reference_mentions_parameter,
};
use synthesis::desugar_generic_data_instances;
pub(crate) use synthesis::desugar_generic_data_instances_in;
pub(crate) use work_domain::GenericDataWorkDomain;

/* Combined closed constant arguments across data declarations, array extents, and machines. */
pub struct ClosedConstArguments {
    pub data_arguments: Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
    pub extent_arguments: Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
    pub machine_arguments: Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
}

pub fn closed_const_arguments(
    syntax: &mut SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declaration_syntax: Option<&SyntaxTrees>,
) -> Result<ClosedConstArguments, Vec<Diagnostic>> {
    let data_positions = collect_data_type_reference_positions(syntax, false);
    let data_public_positions: HashSet<TypeReferenceHandle> =
        collect_data_type_reference_positions(syntax, true)
            .into_iter()
            .collect();
    let (machine_positions, machine_public_positions_vec) =
        collect_callable_type_reference_positions(syntax);
    let machine_public_positions: HashSet<TypeReferenceHandle> =
        machine_public_positions_vec.into_iter().collect();
    let (data_arguments, machine_arguments) = {
        let selection = ConstantSelection::with_retained_base(
            syntax,
            sources,
            top_level_bindings,
            retained_base,
        )?;
        let data_arguments = collect_closed_const_arguments(
            syntax,
            &selection,
            declaration_syntax,
            &data_positions,
            &data_public_positions,
            false,
        );
        let mut machine_arguments = collect_closed_const_arguments(
            syntax,
            &selection,
            declaration_syntax,
            &machine_positions,
            &machine_public_positions,
            true,
        );
        static_argument_positions::collect(
            syntax,
            &selection,
            declaration_syntax,
            &machine_positions,
            &machine_public_positions,
            &mut machine_arguments,
        );
        (data_arguments, machine_arguments)
    };
    let destination = syntax
        .type_references
        .insert_named(Identifier::generated("u64"));
    let mut extent_arguments = Vec::new();
    for owner in data_positions.iter().chain(machine_positions.iter()) {
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Expression(value),
            ..
        } = syntax.type_references.type_reference(*owner)
        else {
            continue;
        };
        if matches!(
            syntax.type_references.type_reference(*value),
            TypeReferenceNode::ConstExpression(_)
        ) && !extent_arguments
            .iter()
            .any(|(existing, _, _)| existing == value)
        {
            let is_public =
                data_public_positions.contains(owner) || machine_public_positions.contains(owner);
            extent_arguments.push((*value, destination, is_public));
        }
    }
    Ok(ClosedConstArguments {
        data_arguments,
        extent_arguments,
        machine_arguments,
    })
}

/*
    Authored constant arguments in concrete data fields and constant declarations.
    Open templates and machine lexical scopes cannot borrow a standalone probe.
    These handles borrow the input syntax; no provisional program escapes. That
    borrow is why discovery names the forest directly instead of taking the
    resolution request, which consumes the forest it prepares.
    Each tuple retains the argument, parameter type, and real owner's public exposure.
*/
pub fn closed_data_const_argument_expressions(
    syntax: &mut SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declaration_syntax: Option<&SyntaxTrees>,
) -> Result<Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>, Vec<Diagnostic>> {
    let positions = collect_data_type_reference_positions(syntax, false);
    if !positions_may_have_closed_const_arguments(syntax, &positions) {
        return Ok(Vec::new());
    }
    let selection =
        ConstantSelection::with_retained_base(syntax, sources, top_level_bindings, retained_base)?;
    let public_positions: HashSet<TypeReferenceHandle> =
        collect_data_type_reference_positions(syntax, true)
            .into_iter()
            .collect();
    Ok(collect_closed_const_arguments(
        syntax,
        &selection,
        declaration_syntax,
        &positions,
        &public_positions,
        false,
    ))
}

/*
    Whether this authored argument can still denote a closed const value: a
    literal const expression, or a bare name that is neither a scalar spelling
    nor a normalized const argument.
*/
fn may_supply_closed_const_argument(syntax: &SyntaxTrees, argument: TypeReferenceHandle) -> bool {
    match syntax.type_references.type_reference(argument) {
        TypeReferenceNode::ConstExpression(_) => true,
        TypeReferenceNode::Named(name) => {
            syntax_type_references(syntax)
                .const_argument_normalization(argument)
                .is_none()
                && ConstScalarSpelling::from_bare_name(name.as_str(syntax)).is_none()
        }
        _ => false,
    }
}

fn positions_may_have_closed_const_arguments(
    syntax: &SyntaxTrees,
    positions: &[TypeReferenceHandle],
) -> bool {
    positions.iter().any(
        |position| match syntax.type_references.type_reference(*position) {
            TypeReferenceNode::Generic { arguments, .. } => syntax_type_references(syntax)
                .type_reference_handles(*arguments)
                .iter()
                .any(|argument| may_supply_closed_const_argument(syntax, *argument)),
            TypeReferenceNode::Constrained { constraints, .. } => syntax_type_references(syntax)
                .constraints(*constraints)
                .iter()
                .any(|constraint| match constraint {
                    TypeConstraintNode::Domain(domain) => syntax_type_references(syntax)
                        .type_reference_handles(domain.arguments)
                        .iter()
                        .any(|argument| may_supply_closed_const_argument(syntax, *argument)),
                    _ => false,
                }),
            _ => false,
        },
    )
}

/*
    Reachable concrete array-extent uses share ordinary constant-expression
    admission. The use is an owned child, not a source span or a fabricated
    generic application; open data/machine telescopes are excluded by discovery.
*/
pub fn closed_array_extent_expressions(
    syntax: &mut SyntaxTrees,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let mut positions = collect_data_type_reference_positions(syntax, false);
    let mut public = collect_data_type_reference_positions(syntax, true);
    let (machine, public_machine) = collect_callable_type_reference_positions(syntax);
    positions.extend(machine);
    public.extend(public_machine);
    let public = public.into_iter().collect();
    closed_array_extents_at(syntax, &positions, &public)
}

/*
    Constant positions demanded by one declaration cohort. Array extents have
    their own checked length-job continuation, rather than a scalar leaf job.
*/
pub struct ClosedDeclarationConstantUses {
    pub arguments: Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
    pub array_extents: Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>,
}

fn closed_array_extents_at(
    syntax: &mut SyntaxTrees,
    positions: &[TypeReferenceHandle],
    public: &HashSet<TypeReferenceHandle>,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let destination = syntax
        .type_references
        .insert_named(Identifier::generated("u64"));
    let mut uses = Vec::new();
    for &owner in positions {
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Expression(value),
            ..
        } = syntax.type_references.type_reference(owner)
        else {
            continue;
        };
        if matches!(
            syntax.type_references.type_reference(*value),
            TypeReferenceNode::ConstExpression(_)
        ) && !uses.iter().any(|(existing, _, _)| existing == value)
        {
            uses.push((*value, destination, public.contains(&owner)));
        }
    }
    uses
}

/*
    Publish a validated deferred job as an ordinary closed private entry.
    Only reachable concrete owners of this exact use change representation.
*/
pub fn retain_deferred_array_extent(
    syntax: &mut SyntaxTrees,
    argument: TypeReferenceHandle,
    entry: Identifier,
) {
    let mut owners = collect_data_type_reference_positions(syntax, false);
    owners.extend(collect_callable_type_reference_positions(syntax).0);
    for owner in owners {
        let node = syntax.type_references.type_reference(owner).clone();
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Expression(value),
        } = node
            && value == argument
        {
            syntax.type_references.replace_type_reference(
                owner,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::ConstCall(entry.clone()),
                },
            );
        }
    }
}

/*
    Authored constant arguments in nongeneric callable type owners. Discovery
    supplies destinations, not value-selection authority: callers must resolve
    each expression in its original declaration scope before evaluation.
*/
pub fn closed_machine_const_arguments(
    syntax: &mut SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declaration_syntax: Option<&SyntaxTrees>,
) -> Result<Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)>, Vec<Diagnostic>> {
    let selection =
        ConstantSelection::with_retained_base(syntax, sources, top_level_bindings, retained_base)?;
    let (positions, public_positions_vec) = collect_callable_type_reference_positions(syntax);
    let public_positions: HashSet<TypeReferenceHandle> = public_positions_vec.into_iter().collect();
    let mut arguments = collect_closed_const_arguments(
        syntax,
        &selection,
        declaration_syntax,
        &positions,
        &public_positions,
        true,
    );
    static_argument_positions::collect(
        syntax,
        &selection,
        declaration_syntax,
        &positions,
        &public_positions,
        &mut arguments,
    );
    Ok(arguments)
}

/*
    Constant type prerequisites of exact ready declarations. Initializer
    scheduling owns readiness; this inventory only supplies receiving carriers.
    It neither evaluates unrelated declarations nor replaces lexical selection.
*/
pub fn closed_declaration_constant_uses(
    syntax: &mut SyntaxTrees,
    sources: Option<std::sync::Arc<source::SourceMap>>,
    top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    retained_base: Option<&symbol_resolved_trees::SymbolResolvedTrees>,
    declaration_syntax: Option<&SyntaxTrees>,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Result<ClosedDeclarationConstantUses, Vec<Diagnostic>> {
    let selection =
        ConstantSelection::with_retained_base(syntax, sources, top_level_bindings, retained_base)?;
    let mut positions = collect_selected_type_reference_positions(syntax, declarations);
    for declaration in declarations {
        if let Item::Const(definition) = syntax.root_item(*declaration) {
            let mut expressions = HashSet::new();
            collect_expression_handles(syntax, definition.value, &mut expressions);
            for expression in expressions {
                visit_expression_type_inputs(syntax, expression, &mut |reference| {
                    collect_type_positions(syntax, reference, &mut positions, true);
                });
            }
        }
    }
    let public_positions = HashSet::new();
    let mut arguments = collect_closed_const_arguments(
        syntax,
        &selection,
        declaration_syntax,
        &positions,
        &public_positions,
        true,
    );
    static_argument_positions::collect(
        syntax,
        &selection,
        declaration_syntax,
        &positions,
        &public_positions,
        &mut arguments,
    );
    let array_extents = closed_array_extents_at(syntax, &positions, &public_positions);
    Ok(ClosedDeclarationConstantUses {
        arguments,
        array_extents,
    })
}

fn collect_closed_const_arguments(
    syntax: &mut SyntaxTrees,
    selection: &ConstantSelection<'_>,
    declaration_syntax: Option<&SyntaxTrees>,
    positions: &[TypeReferenceHandle],
    public_positions: &HashSet<TypeReferenceHandle>,
    include_callable_inputs: bool,
) -> Vec<(TypeReferenceHandle, TypeReferenceHandle, bool)> {
    let mut pending = Vec::new();
    let mut seen_pending = HashSet::new();
    let mut retain_arguments = |syntax: &SyntaxTrees,
                                parameters: &[Option<TypeReferenceHandle>],
                                arguments: &[TypeReferenceHandle]| {
        if parameters.len() != arguments.len() {
            return;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let Some(type_reference) = *parameter
                && may_supply_closed_const_argument(syntax, *argument)
                && seen_pending.insert(*argument)
            {
                pending.push((
                    *argument,
                    type_reference,
                    public_positions.contains(argument),
                ));
            }
        }
    };
    for position in positions {
        match syntax.type_references.type_reference(*position).clone() {
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => {
                let arguments = syntax_type_references(syntax)
                    .type_reference_handles(arguments)
                    .to_vec();
                if let Some(parameters) = selection.const_parameter_carriers(
                    syntax,
                    declaration_syntax,
                    &base_name,
                    &[symbols::SymbolKind::Data],
                ) {
                    retain_arguments(syntax, &parameters, &arguments);
                }
            }
            TypeReferenceNode::Constrained { constraints, .. } => {
                for constraint in syntax.type_references.constraints(constraints) {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    let Some(parameters) = unique_domain_index_parameters(
                        syntax,
                        selection,
                        domain.name.as_str(syntax),
                        domain.name.source_span(),
                    ) else {
                        continue;
                    };
                    let carriers = parameters
                        .iter()
                        .map(|parameter| match parameter.kind {
                            TypeParameterKind::Const { type_reference } => Some(type_reference),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    retain_arguments(
                        syntax,
                        &carriers,
                        syntax_type_references(syntax).type_reference_handles(domain.arguments),
                    );
                }
            }
            _ => {}
        }
    }
    if include_callable_inputs {
        let selected_positions = positions.iter().copied().collect::<HashSet<_>>();
        let applications = uses::concrete_machine_declarations(syntax, syntax.root_item_handles())
            .into_iter()
            .flat_map(|machine| syntax.items.satisfies_clauses(machine.satisfies))
            .filter_map(|clause| {
                let arguments =
                    syntax_type_references(syntax).type_reference_handles(clause.arguments);
                arguments
                    .iter()
                    .any(|argument| selected_positions.contains(argument))
                    .then(|| (clause.trait_name.clone(), arguments.to_vec()))
            })
            .collect::<Vec<_>>();
        for (name, arguments) in applications {
            if let Some(parameters) = selection.const_parameter_carriers(
                syntax,
                declaration_syntax,
                &name,
                &[symbols::SymbolKind::Trait],
            ) {
                let selected = parameters
                    .into_iter()
                    .zip(&arguments)
                    .map(|(carrier, argument)| {
                        selected_positions
                            .contains(argument)
                            .then_some(carrier)
                            .flatten()
                    })
                    .collect::<Vec<_>>();
                retain_arguments(syntax, &selected, &arguments);
            }
        }
        let mut concrete = None;
        for (handle, expression) in syntax.expressions.iter_expressions() {
            let ExpressionNode::Cast(cast) = expression else {
                continue;
            };
            if cast.semantic_domain_arguments.is_empty() {
                continue;
            }
            let members = syntax_expressions(syntax).identifier_path_members(cast.semantic_domain);
            let Some(first) = members.first() else {
                continue;
            };
            let concrete =
                concrete.get_or_insert_with(|| concrete_machine_expression_handles(syntax));
            if !concrete.contains(handle) {
                continue;
            }
            if members.iter().any(|member| {
                let span = member.source_span();
                span.source_id != first.source_span().source_id || span.span.start > span.span.end
            }) || members
                .windows(2)
                .any(|pair| pair[0].source_span().span.end > pair[1].source_span().span.start)
            {
                continue;
            }
            let mut reference = first.source_span();
            for member in members.iter().skip(1) {
                reference.span.end = member.source_span().span.end;
            }
            let name = members.joined_path_in(syntax);
            let Some(parameters) =
                unique_domain_index_parameters(syntax, selection, &name, reference)
            else {
                continue;
            };
            let carriers = parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    TypeParameterKind::Const { type_reference } => Some(type_reference),
                    _ => None,
                })
                .collect::<Vec<_>>();
            retain_arguments(
                syntax,
                &carriers,
                syntax_type_references(syntax)
                    .type_reference_handles(cast.semantic_domain_arguments),
            );
        }
    }
    pending
}

fn unique_domain_index_parameters<'syntax>(
    syntax: &'syntax SyntaxTrees,
    selection: &ConstantSelection<'_>,
    name: &str,
    reference: source::SourceSpan,
) -> Option<&'syntax [TypeParameter]> {
    let (_, definition) = selection.domain_family(syntax, name, reference, None)?;
    domain_index_parameters(syntax, definition)
}

/*
    Whether this exact selected destination carries declaration predicates.
    This schedules the ordinary checked result judgment; it proves nothing.
*/
pub(crate) fn const_destination_requires_proof(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> bool {
    const_evaluation::const_type_requires_declaration_proof(
        syntax,
        definition.type_reference,
        &mut Vec::new(),
        selection,
    )
}

/*
    Whether the definition's retained qualification names its own destination
    and initializer: the qualification the definition itself already satisfies.
*/
pub(super) fn qualification_names_itself(definition: &ConstDefinition) -> bool {
    definition
        .qualification
        .as_ref()
        .is_some_and(|qualification| {
            qualification.destination == definition.type_reference
                && qualification.initializer == definition.value
        })
}

pub(crate) fn canonicalize_pending_initializer_payload(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> Result<crate::resolution::ConstInitializerPayload, String> {
    let mut payload = definition.clone();
    while let TypeReferenceNode::Constrained { base_type, .. } =
        syntax_type_references(syntax).type_reference(payload.type_reference)
    {
        payload.type_reference = *base_type;
    }
    let value = canonicalize_selected_declaration_value_with_admission(
        syntax,
        &payload,
        selection,
        ConstPredicateAdmission::PendingDestination,
    )?;
    Ok(crate::resolution::ConstInitializerPayload {
        value,
        pending_destination: if let Some(obligation) = &definition.pending_qualification {
            if obligation.declaration != definition.name.source_span()
                || obligation.initializer != definition.value
            {
                return Err(
                    "private constant payload drifted from its pending destination".to_owned(),
                );
            }
            Some(obligation.clone())
        } else if !qualification_names_itself(definition)
            && const_destination_requires_proof(syntax, definition, selection)
        {
            Some(crate::resolution::ConstDestinationObligation {
                declaration: definition.name.source_span(),
                destination: definition.type_reference,
                initializer: definition.value,
            })
        } else {
            None
        },
    })
}

/*
    Canonicalize one source const declaration against its own declared type.

    This is the narrow handoff used by declaration/API retention. The returned
    value's structural encoding is semantic material; its display text remains
    diagnostic-only. A constrained carrier canonicalizes at its base and then
    owes the compile-time owner's checked declaration-site result judgment.
    The judgment retains the exact destination and completed initializer;
    encoding alone cannot publish an unproved qualification.
*/
pub fn canonicalize_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<CanonicalConstValue, String> {
    canonicalize_selected_declared_const_definition(syntax, definition, None)
}

pub(crate) fn canonicalize_selected_declared_const_definition(
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
    selection: Option<&ConstantSelection>,
) -> Result<CanonicalConstValue, String> {
    if definition.pending_qualification.is_some() {
        return Err("constant destination qualification remains pending".to_owned());
    }
    let mut payload = definition.clone();
    if let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = syntax_type_references(syntax).type_reference(definition.type_reference)
    {
        let has_domain = syntax_type_references(syntax)
            .constraints(*constraints)
            .iter()
            .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)));
        if has_domain && !qualification_names_itself(definition) {
            return Err(
                "constant qualification requires a checked declaration-site result judgment"
                    .to_owned(),
            );
        }
        if has_domain {
            payload.type_reference = *base_type;
        }
    }
    if qualification_names_itself(definition) {
        canonicalize_selected_declaration_value_with_admission(
            syntax,
            &payload,
            selection,
            ConstPredicateAdmission::CheckedDestination,
        )
    } else {
        canonicalize_selected_declaration_value(syntax, &payload, selection)
    }
}

/*
    Find `Base<Args..>` spellings in FIELD type position where `Base` is a
    generic data definition, synthesize one concrete instance record per
    distinct spelling (the parameter substituted for the argument), and rewrite
    the field spellings to the instances' plain names.
    One forest and the custody its generic data applications close under.
*/
pub struct GenericDataRequest<'a> {
    pub syntax: SyntaxTrees,
    /*
        The loader's exact source/import custody. A source-free forest has
        none; its declarations still use the syntax forest's header selection.
    */
    pub sources: Option<std::sync::Arc<source::SourceMap>>,
    pub top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    /*
        A generated unit's immutable predecessor, whose retained nominal
        arguments the unit may name without owning their syntax. Templates
        still come only from this unit. Requires `sources`.
    */
    pub retained_base: Option<&'a symbol_resolved_trees::SymbolResolvedTrees>,
    pub declaration_syntax: Option<&'a SyntaxTrees>,
    pub retained_declaration_syntax: Option<&'a SyntaxTrees>,
}

impl GenericDataRequest<'_> {
    /* A source-free forest with no scoped top-level bindings. */
    pub fn new(syntax: SyntaxTrees) -> Self {
        Self {
            syntax,
            sources: None,
            top_level_bindings: Vec::new(),
            retained_base: None,
            declaration_syntax: None,
            retained_declaration_syntax: None,
        }
    }
}

/*
    Run Psi's target-neutral pre-resolution generic-data normalization and
    return the only syntax tree downstream stages may consume.

    Taking ownership prevents orchestration code from retaining an unnormalized
    sibling or reaching into the elaborator as an in-place syntax mutator. With
    source custody, temporary header symbols stay private and each erased
    constant argument retains its selected declaration coordinates for the
    complete resolver's checked join.
*/
pub fn normalize_generic_data(
    request: GenericDataRequest<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    normalize_generic_data_in_mode(
        request,
        crate::resolution::lowerer::ConstResolutionMode::Complete,
        GenericDataWorkDomain::All {
            additional_type_roots: &[],
        },
    )
}

/*
    Private evaluation preparation retains unfinished values while closing data
    applications. Complete resolution still owns their mandatory value admission.
*/
pub fn normalize_initializer_probe_data(
    request: GenericDataRequest<'_>,
    declarations: &[syntax_trees::item::ItemHandle],
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    normalize_generic_data_in_mode(
        request,
        crate::resolution::lowerer::ConstResolutionMode::InitializerSelection,
        GenericDataWorkDomain::Selected {
            type_roots: &[],
            declarations,
        },
    )
}

fn normalize_generic_data_in_mode(
    request: GenericDataRequest<'_>,
    mode: crate::resolution::lowerer::ConstResolutionMode,
    domain: GenericDataWorkDomain<'_>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let GenericDataRequest {
        mut syntax,
        sources,
        top_level_bindings,
        retained_base,
        declaration_syntax,
        retained_declaration_syntax,
    } = request;
    let Some(sources) = sources else {
        if retained_base.is_some() || !top_level_bindings.is_empty() {
            return Err(vec![Diagnostic::error(
                "generic data normalization against a retained base or scoped bindings requires source custody",
            )]);
        }
        let selection = declaration_selection::ConstantSelection::new(&syntax, None, Vec::new())?;
        let declarations = declaration_syntax
            .into_iter()
            .chain(retained_declaration_syntax)
            .collect::<Vec<_>>();
        super::declared_types::normalize_with_declarations(&mut syntax, &selection, &declarations)?;
        let (syntax, warnings) = normalize_generic_data_with_warnings(syntax, mode, domain)?;
        report_preparation_diagnostics(warnings)?;
        return Ok(syntax);
    };
    let selection = ConstantSelection::with_retained_base(
        &syntax,
        Some(sources),
        top_level_bindings,
        retained_base,
    )?;
    let declarations = declaration_syntax
        .into_iter()
        .chain(retained_declaration_syntax)
        .collect::<Vec<_>>();
    super::declared_types::normalize_with_declarations(&mut syntax, &selection, &declarations)?;
    domain_heads::normalize(&mut syntax, Some(&selection))?;
    /*
        Generic operands read the same completed declaration values as resolution,
        including aliases. Probe modes keep the authored initializers for evaluation.
    */
    if mode == crate::resolution::lowerer::ConstResolutionMode::Complete {
        fold_closed_scalar_initializers(&mut syntax, &selection)?;
    }
    validate_with_const_resolution_mode(&syntax, &selection, mode)?;
    let mut warnings = Vec::new();
    desugar_generic_data_instances_in(&mut syntax, &mut warnings, Some(&selection), domain)?;
    deduplicate_generic_warnings(&mut warnings);
    report_preparation_diagnostics(warnings)?;
    Ok(syntax)
}

/* Preparation collects const-argument landing diagnostics beside folded
values. Warnings are printed; an error (implicit integer landing after a
fractional intermediate) rejects instead of being printed and folded. */
pub(super) fn report_preparation_diagnostics(
    diagnostics: Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let (errors, warnings): (Vec<_>, Vec<_>) =
        diagnostics.into_iter().partition(Diagnostic::is_error);
    for warning in warnings {
        eprintln!("{warning}");
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/*
    Replay direct structural arguments at their actual generic owner. The
    encoded label is consistency data; the selected template's parameter owns
    the carrier used by the existing canonical encoder.
*/
pub(crate) fn validate_direct_const_arguments(
    syntax: &SyntaxTrees,
    base_name: &Identifier,
    arguments: HandleSpan<TypeReferenceHandle>,
    selection: Option<&ConstantSelection>,
) -> Result<(), Diagnostic> {
    let arguments = syntax.type_references.type_reference_handles(arguments);
    let has_direct = arguments.iter().any(|argument| {
        syntax_type_references(syntax)
            .const_argument_normalization(*argument)
            .is_some_and(crate::constant::normalization_requires_expression)
    });
    if !has_direct {
        return Ok(());
    }
    let selection = selection.ok_or_else(|| {
        Diagnostic::error("direct constant argument has no declaration selection context")
            .with_source_span(base_name.source_span())
    })?;
    let definition = selection
        .data(syntax, base_name)
        .map_err(|message| Diagnostic::error(message).with_source_span(base_name.source_span()))?;
    let parameters = syntax.items.type_parameters(definition.type_parameters);
    if parameters.len() != arguments.len() {
        return Err(
            Diagnostic::error("direct constant argument lost its exact template slot")
                .with_source_span(base_name.source_span()),
        );
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        let Some(normalization) =
            syntax_type_references(syntax).const_argument_normalization(*argument)
        else {
            continue;
        };
        if !crate::constant::normalization_requires_expression(normalization) {
            continue;
        }
        crate::constant::validate_normalized_expression(syntax, normalization)?;
        let TypeParameterKind::Const { type_reference } = parameter.kind else {
            return Err(Diagnostic::error(
                "direct constant argument no longer selects a const parameter",
            )
            .with_source_span(normalization.reference));
        };
        let value = canonicalize_selected_index_expression(
            syntax,
            type_reference,
            type_reference,
            if normalization.materialized_expression.is_valid() {
                normalization.materialized_expression
            } else {
                normalization.authored_expression
            },
            Some(selection),
        )
        .map_err(|message| Diagnostic::error(message).with_source_span(normalization.reference))?;
        if value.encoding != normalization.canonical_result_encoding {
            return Err(Diagnostic::error(
                "direct constant expression differs from its normalized value",
            )
            .with_source_span(normalization.reference));
        }
    }
    Ok(())
}

fn normalize_generic_data_with_warnings(
    mut syntax: SyntaxTrees,
    mode: crate::resolution::lowerer::ConstResolutionMode,
    domain: GenericDataWorkDomain<'_>,
) -> Result<(SyntaxTrees, Vec<Diagnostic>), Vec<Diagnostic>> {
    domain_heads::normalize(&mut syntax, None)?;
    let selection = ConstantSelection::new(&syntax, None, Vec::new())?;
    if mode == crate::resolution::lowerer::ConstResolutionMode::Complete {
        fold_closed_scalar_initializers(&mut syntax, &selection)?;
    }
    validate_with_const_resolution_mode(&syntax, &selection, mode)?;
    let mut warnings = Vec::new();
    match domain {
        GenericDataWorkDomain::All { .. } => {
            desugar_generic_data_instances(&mut syntax, &mut warnings)?
        }
        GenericDataWorkDomain::Selected { .. } => {
            desugar_generic_data_instances_in(&mut syntax, &mut warnings, Some(&selection), domain)?
        }
    }
    deduplicate_generic_warnings(&mut warnings);
    Ok((syntax, warnings))
}

fn deduplicate_generic_warnings(warnings: &mut Vec<Diagnostic>) {
    let mut origins = Vec::new();
    warnings.retain(|warning| {
        let Some(origin) = warning.source_span else {
            return true;
        };
        if warning.is_error()
            || origin.source_id.0 == usize::MAX
            || origin.span.start >= origin.span.end
        {
            return true;
        }
        if origins.contains(&origin) {
            return false;
        }
        origins.push(origin);
        true
    });
}

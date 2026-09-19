//! Closed argument identities and lifetime-bearing type substitution.
use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::ClosedConstraintIdentity;
use crate::preparation::generic_data::GenericData;
use crate::preparation::generic_data::constant_selection;
use crate::preparation::generic_data::selected_data_item;
use crate::preparation::type_equations::{EquationTemplate, complete_equation_arguments};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::Item;
use syntax_trees::item::TypeParameterKind;
use syntax_trees::types::FixedArrayLength;
use syntax_trees::types::TypeConstraintNode;
use syntax_trees::types::TypeReferenceHandle;
use syntax_trees::types::TypeReferenceNode;

/// Recover every omitted trailing binder of `base_info` from its type
/// equations and verify the equations against the supplied prefix. The
/// returned tuple is complete; recovered const binders are the canonical
/// decimal `Named` leaves literal const arguments already use.
pub(super) fn complete_argument_tuple(
    syntax: &mut SyntaxTrees,
    base_info: &GenericData,
    base_name: &Identifier,
    supplied: &[TypeReferenceHandle],
    const_values: &HashMap<String, i128>,
    selection: Option<&constant_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<TypeReferenceHandle>, Diagnostic> {
    let syntax_trees::item::Item::Data(definition) = syntax.root_item(base_info.declaration) else {
        return Err(Diagnostic::error(
            "structural data equation lost its declaration",
        ));
    };
    let template = EquationTemplate {
        kind: "data",
        name: &base_info.name,
        parameters: definition.type_parameters,
        parameter_names: &base_info.parameter_names,
        const_parameter_types: &base_info.const_parameter_types,
        type_equations: &base_info.type_equations,
    };
    complete_equation_arguments(
        syntax,
        template,
        base_name,
        supplied,
        const_values,
        selection,
        warnings,
    )
}

/// Diagnostic names for admitted closed arguments, not application identity.
/// Range shells require structured observations from typed numeric evaluation;
/// this syntax owner never evaluates their bounds. Substitution retains the
/// original argument type and its constraints. Instance sharing uses the exact
/// declaration/argument identities below, not matching rendered names.
pub(crate) fn monomorphizable_argument_slugs(
    syntax: &SyntaxTrees,
    argument_handles: &[TypeReferenceHandle],
) -> Option<Vec<String>> {
    argument_handles
        .iter()
        .map(|&argument| type_reference_slug(syntax, argument))
        .collect()
}

/// Rebind erased lifetimes carried by an already-synthesized local instance
/// from one concrete outer use to the outer template's own binder roster.
///
/// This first exact cohort is deliberately positional: the nested instance
/// must forward the complete outer lifetime application in the same order.
/// That preserves one stable synthesized definition across differently named
/// use-site lifetimes without inventing binders or choosing an alias/routing
/// policy. Broader permutations remain on the unnormalized path.
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
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if !lifetime_arguments.is_empty()
            && syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            if outer_lifetime_parameters.len() != outer_lifetime_arguments.len()
                || lifetime_arguments.len() != outer_lifetime_arguments.len()
                || !lifetime_arguments
                    .iter()
                    .zip(outer_lifetime_arguments)
                    .all(|(nested, outer)| nested.as_str() == outer.as_str())
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
                    if definition.name.as_str() == name
                        && definition.generic_instance.is_some()
                        && definition.type_parameters.is_empty()
                        && definition.lifetime_parameters.len() == lifetime_arity
            )
        })
}

/// Naming metadata for closed types, arrays, qualifications and observed ranges.
/// Unsupported or open shapes remain on the ordinary generic path.
pub(crate) fn type_reference_slug(
    syntax: &SyntaxTrees,
    handle: TypeReferenceHandle,
) -> Option<String> {
    match syntax.tables.type_references.type_reference(handle) {
        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } if syntax
            .tables
            .type_references
            .type_reference_handles(*arguments)
            .is_empty()
            && exact_synthesized_lifetime_instance(
                syntax,
                base_name.as_str(),
                lifetime_arguments.len(),
            ) =>
        {
            Some(base_name.as_str().to_owned())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } if *length > 0 => Some(format!(
            "[{}; {length}]",
            type_reference_slug(syntax, *element_type)?
        )),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base = type_reference_slug(syntax, *base_type)?;
            let mut rendered = Vec::new();
            for (ordinal, constraint) in syntax
                .tables
                .type_references
                .constraints(*constraints)
                .iter()
                .enumerate()
            {
                if matches!(constraint, TypeConstraintNode::Range { .. }) {
                    let range = syntax
                        .type_references
                        .integer_range_normalization(handle, ordinal)?;
                    rendered.push(format!("[{}..={}]", range.minimum, range.maximum));
                } else {
                    rendered.push(constraint_slug(constraint)?);
                }
            }
            if rendered.is_empty() {
                return Some(base);
            }
            Some(format!("{base} in {}", rendered.join(" + ")))
        }
        _ => None,
    }
}

/// Nameable behavior/domain tags. Ranges need their exact owner observation,
/// handled by the caller, rather than rendering an arbitrary expression here.
pub(crate) fn constraint_slug(constraint: &TypeConstraintNode) -> Option<String> {
    match constraint {
        TypeConstraintNode::Named(name) => Some(name.as_str().to_string()),
        TypeConstraintNode::Domain(domain) if domain.arguments.is_empty() => {
            Some(domain.name.as_str().to_string())
        }
        TypeConstraintNode::Domain(_) => None,
        TypeConstraintNode::ArithmeticDomain(domain) => Some(domain.name().to_string()),
        TypeConstraintNode::Range { .. } => None,
    }
}

/// Classify only arguments the existing closed-shape gate has admitted. Open
/// binders remain on the ordinary generic path; no unresolved name is an atom.
pub(crate) fn closed_argument_identity(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
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
        let Item::Data(data) = syntax.root_item(declaration) else {
            return None;
        };
        let parameters = syntax.items.type_parameters(data.type_parameters);
        let arguments = syntax.type_references.type_reference_handles(*arguments);
        if parameters.len() != arguments.len() {
            return None;
        }
        let identities = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| {
                closed_argument_identity(
                    syntax,
                    selection,
                    *argument,
                    matches!(parameter.kind, TypeParameterKind::Const { .. }),
                )
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(ClosedArgumentIdentity::Instance(declaration, identities));
    }
    match syntax.type_references.type_reference(handle) {
        TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            if arguments.is_empty() {
                let mut candidates = syntax.root_items().filter_map(|item| match item {
                    Item::Data(data)
                        if data.name.as_str() == base_name.as_str()
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
            let Item::Data(data) = syntax.root_item(declaration) else {
                return None;
            };
            let parameters = syntax.items.type_parameters(data.type_parameters);
            let arguments = syntax.type_references.type_reference_handles(*arguments);
            if parameters.len() != arguments.len() {
                return None;
            }
            Some(ClosedArgumentIdentity::Instance(
                declaration,
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| {
                        closed_argument_identity(
                            syntax,
                            selection,
                            *argument,
                            matches!(parameter.kind, TypeParameterKind::Const { .. }),
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            ))
        }
        TypeReferenceNode::Named(name) if is_constant => {
            Some(ClosedArgumentIdentity::Constant(name.as_str().to_owned()))
        }
        TypeReferenceNode::Named(name) => {
            if let Some(atom) = symbols::BuiltinTypeAtom::ALL
                .into_iter()
                .find(|atom| atom.symbol_name() == name.as_str())
            {
                return Some(ClosedArgumentIdentity::Builtin(atom));
            }
            if let Some(declaration) = selected_data_item(syntax, selection, name) {
                Some(ClosedArgumentIdentity::Nominal(declaration))
            } else {
                Some(ClosedArgumentIdentity::RetainedNominal(
                    selection?.retained_identity(name, symbols::SymbolKind::Data)?,
                ))
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Some(ClosedArgumentIdentity::Array(
            Box::new(closed_argument_identity(
                syntax,
                selection,
                *element_type,
                false,
            )?),
            *length,
        )),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base = closed_argument_identity(syntax, selection, *base_type, false)?;
            let mut identities = Vec::new();
            for (ordinal, constraint) in syntax
                .type_references
                .constraints(*constraints)
                .iter()
                .enumerate()
            {
                identities.push(match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        ClosedConstraintIdentity::Arithmetic(*domain)
                    }
                    TypeConstraintNode::Domain(domain) if !domain.arguments.is_empty() => {
                        return None;
                    }
                    TypeConstraintNode::Named(name)
                    | TypeConstraintNode::Domain(syntax_trees::types::DomainConstraint {
                        name,
                        arguments: _,
                    }) => {
                        if let Some(selection) = selection {
                            // The constraint's domain is selected by the same
                            // module name law resolution applies: qualified
                            // spellings name one exact declaration, relative and
                            // leaf spellings prefer the reference's own module,
                            // and narrow imports expose only the selected
                            // declaration. Competing owners or an unreachable
                            // same-spelled declaration decline rather than
                            // equating rendered text.
                            match selection.domain(syntax, name.as_str(), name.source_span()) {
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
                                    // `domain` returned no settled owner. When
                                    // the spelling still reaches an in-forest
                                    // declaration — competing owners or a
                                    // generic family — the constraint declines
                                    // rather than rescuing a retained domain
                                    // through a contested name law selection.
                                    // With nothing reachable, a retained-base
                                    // domain remains a lawful identity.
                                    if selection.contested_domain(
                                        syntax,
                                        name.as_str(),
                                        name.source_span(),
                                    ) {
                                        return None;
                                    }
                                    let TypeReferenceNode::Named(carrier) = syntax
                                        .type_references
                                        .type_reference(*base_type)
                                    else {
                                        return None;
                                    };
                                    ClosedConstraintIdentity::RetainedDeclaration(
                                        selection.retained_domain_identity(carrier, name)?,
                                    )
                                }
                            }
                        } else {
                            // Headerless forests cannot run import-aware
                            // selection; only a unique same-carrier declaration
                            // stands for the constraint, and same-spelled
                            // siblings still decline rather than equating text.
                            let mut candidates =
                                syntax.root_item_handles().iter().copied().filter(|handle| {
                                    let Item::Domain(domain) = syntax.root_item(*handle) else {
                                        return false;
                                    };
                                    let matching_name = domain.name.as_str() == name.as_str()
                                        || (!name.as_str().contains("::")
                                            && domain.name.as_str().rsplit("::").next()
                                                == Some(name.as_str()));
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
                    TypeConstraintNode::Range { .. } => ClosedConstraintIdentity::Range(
                        syntax
                            .type_references
                            .integer_range_normalization(handle, ordinal)?
                            .clone(),
                    ),
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

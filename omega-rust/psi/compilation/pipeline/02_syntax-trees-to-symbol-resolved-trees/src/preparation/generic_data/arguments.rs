//! Closed argument identities and lifetime-bearing type substitution.
//! Closed array extents share the declared-constant reader with structural
//! matching. Observe geometry for identity and display without rewriting the
//! authored type: receiving resolution still owns its selected extent origin.
use crate::preparation::declaration_selection;
use crate::preparation::declaration_selection::GenericApplicationSubstitution;
use crate::preparation::generic_data::ClosedArgumentIdentity;
use crate::preparation::generic_data::ClosedConstraintIdentity;
use crate::preparation::generic_data::GenericData;
use crate::preparation::generic_data::selected_data_item;
use crate::preparation::type_equations::{EquationTemplate, complete_equation_arguments};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use std::collections::HashMap;
use syntax_trees::SyntaxTrees;
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
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Option<Vec<String>> {
    argument_handles
        .iter()
        .map(|&argument| type_reference_slug(syntax, argument, selection))
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
            && syntax
                .tables
                .type_references
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

/// Naming metadata for closed types, arrays, qualifications and observed ranges.
/// Unsupported or open shapes remain on the ordinary generic path.
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
                syntax_trees::expression::ExpressionNode::Integer(literal) => {
                    super::integer_literal_value(literal).map(|value| value.to_string())
                }
                syntax_trees::expression::ExpressionNode::Name(path) => {
                    let [member] = syntax.expressions.identifier_path_members(*path) else {
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
        } if syntax
            .tables
            .type_references
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
                    rendered.push(constraint_slug(syntax, constraint, selection)?);
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
/// An indexed declared domain renders its name with the index tuple.
pub(crate) fn constraint_slug(
    syntax: &SyntaxTrees,
    constraint: &TypeConstraintNode,
    selection: Option<&declaration_selection::ConstantSelection>,
) -> Option<String> {
    match constraint {
        TypeConstraintNode::Named(name) => Some(name.as_str(syntax).to_string()),
        TypeConstraintNode::Domain(domain) => {
            let arguments = syntax
                .type_references
                .type_reference_handles(domain.arguments);
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
        TypeConstraintNode::Range { .. } => None,
    }
}

/// Classify only arguments the existing closed-shape gate has admitted. Open
/// binders remain on the ordinary generic path; no unresolved name is an atom.
/// The shared parameter-argument zip: each argument's closed identity —
/// `Const`-kinded binders marking it const — or `None` when lengths disagree
/// or any argument stays open.
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
            closed_argument_identity(
                syntax,
                selection,
                *argument,
                parameter.kind.is_const_or_value(),
            )
        })
        .collect::<Option<Vec<_>>>()
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
        // Named lifetime occurrences have not selected lexical binders here.
        // Keep them undecidable rather than equating spellings or erasing the
        // relation. Anonymous constructors retain exact access and child type;
        // this identity supplies no runtime loan or lifetime evidence.
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
        TypeReferenceNode::Slice { element_type } => Some(ClosedArgumentIdentity::Slice(Box::new(
            closed_argument_identity(syntax, selection, *element_type, false)?,
        ))),
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
        TypeReferenceNode::Named(name) if is_constant => Some(ClosedArgumentIdentity::Constant(
            name.as_str(syntax).to_owned(),
        )),
        TypeReferenceNode::ConstExpression(expression) if is_constant => {
            match syntax.expressions.expression(*expression) {
                syntax_trees::expression::ExpressionNode::Integer(literal) => {
                    super::integer_literal_value(literal)
                        .map(|value| ClosedArgumentIdentity::Constant(value.to_string()))
                }
                syntax_trees::expression::ExpressionNode::Name(path) => {
                    let [member] = syntax.expressions.identifier_path_members(*path) else {
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
            if let Some(atom) =
                crate::symbols::builtin_type_atom_by_symbol_name(name.as_str(syntax))
            {
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
                            // The constraint's domain is selected by the same
                            // module name law resolution applies: qualified
                            // spellings name one exact declaration, relative and
                            // leaf spellings prefer the reference's own module,
                            // and narrow imports expose only the selected
                            // declaration. Competing owners or an unreachable
                            // same-spelled declaration decline rather than
                            // equating rendered text.
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
                                        name.as_str(syntax),
                                        name.source_span(),
                                    ) {
                                        return None;
                                    }
                                    let carrier = syntax
                                        .type_references
                                        .type_reference(*base_type).as_named()?;
                                    ClosedConstraintIdentity::RetainedDeclaration(
                                        selection.retained_domain_identity(syntax, carrier, name)?,
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

/// The family's index telescope and whether its leading type binder names the
/// carrier: `domain<T> T::Foreign` reserves its first parameter for the
/// generic carrier, while a concrete carrier like
/// `domain<const C: u64> u64::AtMost<C>` keeps every declared parameter as an
/// index. This is the one rule shared by identity, argument normalization and
/// the structural matcher in `type_structure::applications`.
pub(crate) fn domain_index_telescope(
    syntax: &SyntaxTrees,
    definition: &DomainDefinition,
) -> (Vec<TypeParameter>, bool) {
    let parameters = syntax.items.type_parameters(definition.type_parameters);
    let target_name = match syntax
        .type_references
        .type_reference(definition.target_type)
    {
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

/// Rejoin an authored domain family through the shared namespace authority.
/// Without a supplied package selection, build the same local header authority;
/// never rescue a contested or inaccessible owner with a leaf-name scan.
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
    // Structural decomposition also reaches fixed, zero-index domains inside
    // arrays and other applications. Both authority readers reject contested
    // ownership; the non-generic reader must not be replaced by a leaf scan or
    // omitted merely because this caller also handles indexed families.
    let definition = selection
        .domain_family(syntax, name.as_str(syntax), name.source_span(), carrier)
        .map(|(_, definition)| definition)
        .or_else(|| selection.domain(syntax, name.as_str(syntax), name.source_span(), carrier))?;
    syntax.root_item_handles().iter().copied().find(|handle| {
        syntax
            .root_item(*handle)
            .as_domain()
            .is_some_and(|candidate| std::ptr::eq(candidate, definition))
    })
}

/// Identity for an indexed declared-domain application: the declaration the
/// authored name selects on this carrier plus each index argument's own
/// closed identity under the family's declared index telescope. Selection
/// follows the shared declaration authority used by structural matching.
/// Fixed families require their declared carrier; generic families retain the
/// actual carrier in the enclosing constrained identity independently of the
/// family's index tuple. Competing owners decline rather than equate spellings.
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

/// One in-forest `domain` declaration selected as a carrier-qualified
/// application head.
pub(crate) struct DomainHead {
    pub(crate) declaration: ItemHandle,
    /// The declaration's own name. A generic family's retains only the domain
    /// segments (`domain<const C: u64> u64::AtMost<C>` declares `AtMost`),
    /// which is also the canonical constraint name a rewritten spelling
    /// carries.
    pub(crate) declared_name: Identifier,
    /// The family's index telescope: its declared parameters minus a leading
    /// type binder that names a generic carrier (`domain<T> T::Foreign`).
    pub(crate) index_parameters: Vec<TypeParameter>,
    /// The declared carrier, retained as the canonical constrained base.
    pub(crate) carrier: DomainCarrier,
}

/// A qualified head retains its authored carrier until it can join an ordinary
/// type tree. Fixed families retain their declared tree; generic families bind
/// the spelled carrier independently from their index telescope.
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
            Self::Type(reference) => closed_argument_identity(syntax, selection, *reference, false),
            Self::Named(name) => super::closed_name_identity(syntax, selection, name),
        }
    }
}

/// Select the domain a `Carrier::Name` or bare `Name` head applies: the
/// shared declaration authority owns the spelled leaf. Fixed families check
/// the prefix against their declared target; generic families retain the
/// explicit prefix as a type tree for ordinary structural binding. Competing
/// or carrier-mismatched owners decline rather than equate spellings. Identity
/// sees exactly the declaration chosen by the structural matcher.
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

/// A carrier-qualified `Carrier::Name<indices>` type spells the same domain
/// application as the constrained `Carrier in Name<indices>` node. Rewrite
/// every reachable head in `positions` before the closed-index
/// canonicalization snapshot so a module constant inside one folds exactly as
/// an authored constraint, and solving, identity, naming and substitution all
/// observe the one canonical shape the constrained spelling produces.
/// `positions` is the owner-position walk: every type reference reachable
/// from a concrete owner or an open template member.
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
            let nested = syntax
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            // A data application keeps its nominal head; only a node whose
            // head selects no in-forest data is a domain candidate, the same
            // precedence the structural matcher gives `closed_name_identity`.
            // Arity is not rechecked here: a wrong tuple becomes the same
            // constraint the authored `in` spelling writes, and the domain
            // family's own arity diagnostic reports it.
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
            // The arguments move into the rewritten constraint unchanged, so
            // one descent covers a nested head in either shape.
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
                for argument in syntax
                    .type_references
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

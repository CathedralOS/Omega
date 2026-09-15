//! Normalizing constrained bases, constraints, range endpoints, domain
//! identities and array lengths into canonical atoms.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::type_identity::substitution;
use crate::typed_trees::type_system::type_identity::NormalizedDomainTerm;
use crate::typed_trees::type_system::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_index_expression,
    normalize_type_reference,
};
use crate::types::{
    DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode,
};
use arena::HandleSpan;

pub(crate) fn normalize_constrained_base(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> (String, Vec<NormalizedConstraint>) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints, context));
            (base, all_constraints)
        }
        _ => (
            normalize_type_reference(program, type_reference, context),
            Vec::new(),
        ),
    }
}

pub(crate) fn normalized_constraints(
    program: &TypedTrees,
    constraints: HandleSpan<TypeConstraintNode>,
    context: &TypeIdentityContext<'_>,
) -> Vec<NormalizedConstraint> {
    program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .map(|constraint| match constraint {
            TypeConstraintNode::Named(name) => {
                NormalizedConstraint::Named(name.as_str().to_owned())
            }
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
                // Canonical identity and range checking share exact endpoint
                // evaluation. Keep symbolic end-kind when evaluation is open;
                // flow facts never supply static type identity.
                let minimum = normalized_range_endpoint(program, *minimum, true, context);
                let maximum = normalized_range_endpoint(program, *maximum, *end_inclusive, context);
                NormalizedConstraint::Range { minimum, maximum }
            }
            TypeConstraintNode::ArithmeticDomain(domain) => {
                NormalizedConstraint::Arithmetic(domain.name().to_owned())
            }
            TypeConstraintNode::Domain(domain) => match domain.subject {
                DomainConstraintSubject::Declared => NormalizedConstraint::DeclaredDomain(
                    normalized_declared_domain_identity(program, domain, context),
                ),
                DomainConstraintSubject::Carry(_)
                | DomainConstraintSubject::Value(_)
                | DomainConstraintSubject::OmegaLayout { .. } => {
                    NormalizedConstraint::CompilerDomain(normalized_compiler_domain_identity(
                        program, domain, context,
                    ))
                }
            },
        })
        .collect()
}

pub(crate) fn normalized_range_endpoint(
    program: &TypedTrees,
    expression: ExpressionHandle,
    end_inclusive: bool,
    context: &TypeIdentityContext<'_>,
) -> String {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && let Some(identity) =
            substitution::range_endpoint(program, path.symbol, end_inclusive, context)
    {
        return identity;
    }
    if let Some(value) = program.closed_integer_range_endpoint(expression, end_inclusive) {
        return normalized_range_integer(&value, context);
    }
    // Open endpoints retain the same binder slots, exact substitutions and
    // declaration ownership as other const indices. Diagnostic spelling cannot
    // distinguish two owners' same-named binders, even in ordinary identity.
    let identity = normalize_index_expression(program, expression, context);
    if end_inclusive {
        identity
    } else {
        compound("exclusive-end", [identity])
    }
}

pub(crate) fn normalized_range_integer(
    value: &numerics::bignum::BigInt,
    context: &TypeIdentityContext<'_>,
) -> String {
    if context.qualification == TypeIdentityQualification::PackageQualified {
        atom("integer", &value.to_string())
    } else {
        value.to_string()
    }
}

fn normalized_declared_domain_identity(
    program: &TypedTrees,
    domain: &DomainConstraint,
    context: &TypeIdentityContext<'_>,
) -> String {
    let ordinary_name = declared_domain_identity(program, domain);
    if context.qualification == TypeIdentityQualification::Ordinary {
        return ordinary_name;
    }
    compound(
        "declared-domain",
        std::iter::once(atom(
            "name",
            &context.name(program, domain.symbol, &ordinary_name),
        ))
        .chain(
            domain
                .arguments
                .iter()
                .map(|argument| normalize_type_reference(program, *argument, context)),
        ),
    )
}

pub(crate) fn normalized_compiler_domain_identity(
    program: &TypedTrees,
    domain: &DomainConstraint,
    context: &TypeIdentityContext<'_>,
) -> String {
    match domain.subject {
        DomainConstraintSubject::Declared => {
            normalized_declared_domain_identity(program, domain, context)
        }
        DomainConstraintSubject::Carry(permission) => compound(
            "compiler-domain",
            [
                atom("family", "carry"),
                atom(
                    "permission",
                    match permission {
                        language_semantics::CarryPermission::AcrossSuspend => "across-suspend",
                        language_semantics::CarryPermission::AnyCpu => "any-cpu",
                        language_semantics::CarryPermission::AnyThread => "any-thread",
                        language_semantics::CarryPermission::MovableAddress => "movable-address",
                    },
                ),
            ],
        ),
        DomainConstraintSubject::Value(value_domain) => compound(
            "compiler-domain",
            [
                atom("family", "value"),
                atom(
                    "domain",
                    match value_domain {
                        language_semantics::value_domain::ValueDomain::Finite => "finite",
                    },
                ),
            ],
        ),
        DomainConstraintSubject::OmegaLayout { grammar } => compound(
            "compiler-domain",
            [
                atom("family", "omega-layout"),
                atom(
                    "grammar",
                    match grammar {
                        crate::types::OmegaLayoutGrammar::Derived => "derived",
                    },
                ),
            ]
            .into_iter()
            .chain(domain.arguments.iter().map(|argument| {
                compound(
                    "schema",
                    [normalize_type_reference(program, *argument, context)],
                )
            })),
        ),
    }
}

pub(crate) fn normalized_domain_term(
    program: &TypedTrees,
    constraint: &TypeConstraintNode,
) -> Option<NormalizedDomainTerm> {
    match constraint {
        TypeConstraintNode::ArithmeticDomain(domain) => {
            Some(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
        }
        TypeConstraintNode::Domain(domain) => Some(match domain.subject {
            DomainConstraintSubject::Declared => {
                NormalizedDomainTerm::Declared(declared_domain_identity(program, domain))
            }
            DomainConstraintSubject::Carry(_)
            | DomainConstraintSubject::Value(_)
            | DomainConstraintSubject::OmegaLayout { .. } => {
                NormalizedDomainTerm::Compiler(normalized_compiler_domain_identity(
                    program,
                    domain,
                    &TypeIdentityContext::default(),
                ))
            }
        }),
        TypeConstraintNode::Named(_) | TypeConstraintNode::Range { .. } => None,
    }
}

fn declared_domain_identity(program: &TypedTrees, domain: &DomainConstraint) -> String {
    if domain.semantic_id.is_valid()
        && let Some(name) = program.semantic_domains.name(domain.semantic_id)
    {
        return name.to_owned();
    }
    domain.name.as_str().to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NormalizedConstraint {
    Arithmetic(String),
    CompilerDomain(String),
    DeclaredDomain(String),
    Named(String),
    Range { minimum: String, maximum: String },
}

impl NormalizedConstraint {
    pub(crate) fn encode(self) -> String {
        match self {
            Self::Arithmetic(name) => compound("arithmetic-domain", [atom("name", &name)]),
            Self::CompilerDomain(identity) => identity,
            Self::DeclaredDomain(name) => compound("declared-domain", [atom("name", &name)]),
            Self::Named(name) => compound("named-constraint", [atom("name", &name)]),
            Self::Range { minimum, maximum } => compound(
                "range",
                [atom("minimum", &minimum), atom("maximum", &maximum)],
            ),
        }
    }
}

pub(crate) fn normalize_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    context: &TypeIdentityContext<'_>,
) -> String {
    match length {
        FixedArrayLength::Literal(value) => atom("literal", &value.to_string()),
        FixedArrayLength::ConstParameter { symbol, name } => {
            substitution::array_length(program, *symbol, name.as_str(), context).unwrap_or_else(
                || {
                    atom(
                        "const-parameter",
                        &context.name(program, *symbol, name.as_str()),
                    )
                },
            )
        }
        FixedArrayLength::ConstCall { name, .. } => atom("const-call", name.as_str()),
    }
}

pub(crate) fn atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}

pub(crate) fn byte_atom(tag: &str, value: &[u8]) -> String {
    let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
    output.push_str(tag);
    output.push('(');
    output.push_str(&value.len().to_string());
    output.push(':');
    for byte in value {
        output.push_str(&format!("{byte:02x}"));
    }
    output.push(')');
    output
}

pub(crate) fn compound(tag: &str, parts: impl IntoIterator<Item = String>) -> String {
    let parts = parts.into_iter().collect::<Vec<_>>();
    let mut output =
        String::with_capacity(tag.len() + parts.iter().map(String::len).sum::<usize>() + 2);
    output.push_str(tag);
    output.push('(');
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&part);
    }
    output.push(')');
    output
}

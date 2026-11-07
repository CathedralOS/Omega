//! Normalizing constrained bases, constraints, range endpoints, domain
//! identities and array lengths into canonical atoms.

use crate::TypedTrees;
use crate::type_identity::substitution;
use crate::typed_trees::type_system::type_identity::identity_context::{
    TypeIdentityContext, TypeIdentityQualification, normalize_type_reference,
};
use crate::types::{
    DomainConstraint, DomainConstraintSubject, FixedArrayLength, TypeConstraintNode,
    TypeReferenceHandle, TypeReferenceNode,
};
use arena::HandleSpan;
use language_semantics::type_identity::vocabulary::{
    ArrayLengthForm, ConstraintForm, DomainFamily, IndexForm, Label, Spelled,
};

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

fn normalized_declared_domain_identity(
    program: &TypedTrees,
    domain: &DomainConstraint,
    context: &TypeIdentityContext<'_>,
) -> String {
    let ordinary_name = declared_domain_identity(program, domain);
    // Namespace qualification and complete static application are separate
    // axes. Ordinary identity may erase regions for layout, but cannot merge
    // Tagged<First> with Tagged<Second> merely because their domain head agrees.
    // Each argument uses the same supplying substitution context as the base.
    if context.qualification == TypeIdentityQualification::Ordinary && domain.arguments.is_empty() {
        return ordinary_name;
    }
    compound(
        ConstraintForm::DeclaredDomain,
        std::iter::once(atom(
            Label::Name,
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
            ConstraintForm::CompilerDomain,
            [
                atom(Label::Family, DomainFamily::Carry.spelling()),
                atom(
                    Label::Permission,
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
            ConstraintForm::CompilerDomain,
            [
                atom(Label::Family, DomainFamily::Value.spelling()),
                atom(
                    Label::Domain,
                    match value_domain {
                        language_semantics::value_domain::ValueDomain::Finite => "finite",
                    },
                ),
            ],
        ),
        DomainConstraintSubject::OmegaLayout { grammar } => compound(
            ConstraintForm::CompilerDomain,
            [
                atom(Label::Family, DomainFamily::OmegaLayout.spelling()),
                atom(
                    Label::Grammar,
                    match grammar {
                        crate::types::OmegaLayoutGrammar::Derived => "derived",
                    },
                ),
            ]
            .into_iter()
            .chain(domain.arguments.iter().map(|argument| {
                compound(
                    Label::Schema,
                    [normalize_type_reference(program, *argument, context)],
                )
            })),
        ),
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
}

impl NormalizedConstraint {
    pub(crate) fn encode(self) -> String {
        match self {
            Self::Arithmetic(name) => {
                compound(ConstraintForm::ArithmeticDomain, [atom(Label::Name, &name)])
            }
            Self::CompilerDomain(identity) => identity,
            Self::DeclaredDomain(name) => {
                compound(ConstraintForm::DeclaredDomain, [atom(Label::Name, &name)])
            }
            Self::Named(name) => {
                compound(ConstraintForm::NamedConstraint, [atom(Label::Name, &name)])
            }
        }
    }
}

pub(crate) fn normalize_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    context: &TypeIdentityContext<'_>,
) -> String {
    match length {
        FixedArrayLength::Literal(value) if context.invalid_application.is_some() => {
            atom(IndexForm::Integer, &value.to_string())
        }
        FixedArrayLength::ConstParameter { symbol, name }
            if context.invalid_application.is_some() =>
        {
            substitution::index_name(program, *symbol, name.as_str(), context)
        }
        FixedArrayLength::Expression(value) => {
            let TypeReferenceNode::ConstExpression(expression) =
                program.type_reference_table.type_reference(*value)
            else {
                return atom(
                    ArrayLengthForm::InvalidExpression,
                    "missing constant recipe",
                );
            };
            compound(
                ArrayLengthForm::ConstExpression,
                [super::identity_context::normalize_index_expression(
                    program,
                    *expression,
                    context,
                )],
            )
        }
        FixedArrayLength::Literal(value) => atom(ArrayLengthForm::Literal, &value.to_string()),
        FixedArrayLength::ConstParameter { symbol, name } => {
            substitution::array_length(program, *symbol, name.as_str(), context).unwrap_or_else(
                || {
                    atom(
                        ArrayLengthForm::ConstParameter,
                        &context.name(program, *symbol, name.as_str()),
                    )
                },
            )
        }
        FixedArrayLength::ConstCall { machine, name, .. } => atom(
            ArrayLengthForm::ConstCall,
            &context.name(program, *machine, name.as_str()),
        ),
    }
}

/// The text of a production with no children.
pub(crate) fn bare(tag: impl Spelled) -> String {
    tag.spelling().to_owned()
}

pub(crate) fn atom(tag: impl Spelled, value: &str) -> String {
    let tag = tag.spelling();
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    // The escaped characters are ASCII, so a value without any of them is
    // copied whole; almost every identity segment is such a value.
    if value
        .bytes()
        .any(|byte| matches!(byte, b'\\' | b'(' | b')' | b','))
    {
        for character in value.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                output.push('\\');
            }
            output.push(character);
        }
    } else {
        output.push_str(value);
    }
    output.push(')');
    output
}

pub(crate) fn byte_atom(tag: impl Spelled, value: &[u8]) -> String {
    let tag = tag.spelling();
    let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
    output.push_str(tag);
    output.push('(');
    output.push_str(&value.len().to_string());
    output.push(':');
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in value {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output.push(')');
    output
}

pub(crate) fn compound(tag: impl Spelled, parts: impl IntoIterator<Item = String>) -> String {
    let tag = tag.spelling();
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

#[cfg(test)]
mod byte_atom_tests;

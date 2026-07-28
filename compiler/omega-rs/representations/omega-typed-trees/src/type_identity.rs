//! Deterministic semantic identity for typed type references.
//!
//! Diagnostic rendering is deliberately not an identity oracle. In
//! particular, a domain conjunction is commutative and idempotent, while its
//! authored spelling has an order and may repeat terms. This module owns the
//! canonical form used by type equality, specialization, and published plan
//! identities (decision 19 / DOM4).

use crate::TypedTrees;
use crate::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedTypeIdentity(String);

impl NormalizedTypeIdentity {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NormalizedTypeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The canonical conjunction of semantic domains selected on one binding.
///
/// Terms are sorted and deduplicated. Declared domains are keyed by their
/// normalizer-owned semantic name, not by source order, local arena handles, or
/// a carrier-specialized declaration symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NormalizedDomainExpression {
    terms: Vec<NormalizedDomainTerm>,
}

impl NormalizedDomainExpression {
    pub fn terms(&self) -> &[NormalizedDomainTerm] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    fn from_constraints(program: &TypedTrees, constraints: HandleSpan<TypeConstraintNode>) -> Self {
        let mut terms = program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .filter_map(|constraint| normalized_domain_term(program, constraint))
            .collect::<Vec<_>>();
        terms.sort();
        terms.dedup();
        Self { terms }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Declared(String),
}

impl TypedTrees {
    pub fn normalized_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext::default(),
        ))
    }

    /// Binder-aware form for generic template identity. A parameter symbol is
    /// replaced before serialization, so renaming the source binder cannot
    /// change the normalized contract while concrete declaration paths remain
    /// fully qualified.
    pub fn normalized_type_identity_with_binders(
        &self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext { binders },
        ))
    }

    pub fn normalized_domain_expression(
        &self,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> NormalizedDomainExpression {
        NormalizedDomainExpression::from_constraints(self, constraints)
    }
}

#[derive(Default)]
struct TypeIdentityContext<'binders> {
    binders: &'binders [(SymbolHandle, String)],
}

impl TypeIdentityContext<'_> {
    fn name(&self, program: &TypedTrees, symbol: SymbolHandle, fallback: &str) -> String {
        if let Some((_, replacement)) = self
            .binders
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
        {
            return replacement.clone();
        }
        if symbol.is_valid() {
            let path = program.symbols.display_path(symbol, "::");
            if !path.is_empty() {
                return path;
            }
        }
        fallback.to_owned()
    }
}

fn normalize_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
            lifetime: _,
        } => compound(
            if *is_mutable { "ref-mut" } else { "ref" },
            [normalize_type_reference(program, *referee, context)],
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints));
            all_constraints.sort();
            all_constraints.dedup();
            compound(
                "constrained",
                std::iter::once(base).chain(
                    all_constraints
                        .into_iter()
                        .map(NormalizedConstraint::encode),
                ),
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => compound(
            "array",
            [
                normalize_type_reference(program, *element_type, context),
                normalize_array_length(program, length, context),
            ],
        ),
        TypeReferenceNode::Slice { element_type } => compound(
            "slice",
            [normalize_type_reference(program, *element_type, context)],
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments: _,
            arguments,
            ..
        } => compound(
            "generic",
            std::iter::once(atom(
                "name",
                &context.name(program, *base_symbol, base_name.as_str()),
            ))
            .chain(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| normalize_type_reference(program, *argument, context)),
            ),
        ),
        TypeReferenceNode::DynamicTrait { symbol, name } => compound(
            "dynamic-trait",
            [atom("name", &context.name(program, *symbol, name.as_str()))],
        ),
        TypeReferenceNode::Named { symbol, name } => compound(
            "named",
            [atom("name", &context.name(program, *symbol, name.as_str()))],
        ),
        TypeReferenceNode::Unit => "unit".to_owned(),
    }
}

fn normalize_constrained_base(
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
            all_constraints.extend(normalized_constraints(program, *constraints));
            (base, all_constraints)
        }
        _ => (
            normalize_type_reference(program, type_reference, context),
            Vec::new(),
        ),
    }
}

fn normalized_constraints(
    program: &TypedTrees,
    constraints: HandleSpan<TypeConstraintNode>,
) -> Vec<NormalizedConstraint> {
    program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .map(|constraint| match constraint {
            TypeConstraintNode::Named(name) => {
                NormalizedConstraint::Named(name.as_str().to_owned())
            }
            TypeConstraintNode::Range { minimum, maximum } => NormalizedConstraint::Range {
                minimum: program.expression_table.display_name(*minimum),
                maximum: program.expression_table.display_name(*maximum),
            },
            TypeConstraintNode::ArithmeticDomain(domain) => {
                NormalizedConstraint::Arithmetic(domain.name().to_owned())
            }
            TypeConstraintNode::Domain(domain) => {
                NormalizedConstraint::DeclaredDomain(declared_domain_identity(program, domain))
            }
        })
        .collect()
}

fn normalized_domain_term(
    program: &TypedTrees,
    constraint: &TypeConstraintNode,
) -> Option<NormalizedDomainTerm> {
    match constraint {
        TypeConstraintNode::ArithmeticDomain(domain) => {
            Some(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
        }
        TypeConstraintNode::Domain(domain) => Some(NormalizedDomainTerm::Declared(
            declared_domain_identity(program, domain),
        )),
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
enum NormalizedConstraint {
    Arithmetic(String),
    DeclaredDomain(String),
    Named(String),
    Range { minimum: String, maximum: String },
}

impl NormalizedConstraint {
    fn encode(self) -> String {
        match self {
            Self::Arithmetic(name) => compound("arithmetic-domain", [atom("name", &name)]),
            Self::DeclaredDomain(name) => compound("declared-domain", [atom("name", &name)]),
            Self::Named(name) => compound("named-constraint", [atom("name", &name)]),
            Self::Range { minimum, maximum } => compound(
                "range",
                [atom("minimum", &minimum), atom("maximum", &maximum)],
            ),
        }
    }
}

fn normalize_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    context: &TypeIdentityContext<'_>,
) -> String {
    match length {
        FixedArrayLength::Literal(value) => atom("literal", &value.to_string()),
        FixedArrayLength::ConstParameter { symbol, name } => atom(
            "const-parameter",
            &context.name(program, *symbol, name.as_str()),
        ),
        FixedArrayLength::ConstCall { name } => atom("const-call", name.as_str()),
    }
}

fn atom(tag: &str, value: &str) -> String {
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

fn compound(tag: &str, parts: impl IntoIterator<Item = String>) -> String {
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
mod tests {
    use super::{NormalizedDomainTerm, NormalizedTypeIdentity};
    use crate::TypedTrees;
    use crate::name::Identifier;
    use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
    use omega_core::semantics::{DomainPredicateBody, DomainSemanticRoles, SemanticDomainId};
    use omega_core::symbols::SymbolHandle;

    fn declared(name: &str, semantic_id: SemanticDomainId) -> TypeConstraintNode {
        TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated(name),
            symbol: SymbolHandle::invalid(),
            semantic_id,
            predicate_body: DomainPredicateBody::Present,
            semantic_roles: DomainSemanticRoles::default(),
        })
    }

    fn constrained(
        program: &mut TypedTrees,
        base_type: omega_core::arena::Handle<TypeReferenceNode>,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> omega_core::arena::Handle<TypeReferenceNode> {
        let constraints = program.type_reference_table.insert_constraints(constraints);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            })
    }

    #[test]
    fn domain_conjunction_identity_is_sorted_deduplicated_and_semantic() {
        let mut program = TypedTrees::default();
        let utf8 = program.semantic_domains.intern("Utf8");
        let no_nul = program.semantic_domains.intern("NoNul");
        let constraints = program.type_reference_table.insert_constraints([
            declared("AliasForNoNul", no_nul),
            declared("Utf8", utf8),
            declared("Utf8Again", utf8),
        ]);

        let normalized = program.normalized_domain_expression(constraints);
        assert_eq!(
            normalized.terms(),
            [
                NormalizedDomainTerm::Declared("NoNul".to_owned()),
                NormalizedDomainTerm::Declared("Utf8".to_owned()),
            ]
        );
    }

    #[test]
    fn reordered_flat_and_nested_domain_conjunctions_have_one_type_identity() {
        let mut program = TypedTrees::default();
        let alpha = program.semantic_domains.intern("Alpha");
        let beta = program.semantic_domains.intern("Beta");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });

        let flat = constrained(
            &mut program,
            base,
            [declared("Alpha", alpha), declared("Beta", beta)],
        );
        let nested_inner = constrained(&mut program, base, [declared("Beta", beta)]);
        let nested = constrained(
            &mut program,
            nested_inner,
            [declared("AlphaAlias", alpha), declared("Alpha", alpha)],
        );

        assert_eq!(
            program.normalized_type_identity(flat),
            program.normalized_type_identity(nested)
        );
    }

    #[test]
    fn same_constraint_count_does_not_collapse_distinct_domains() {
        let mut program = TypedTrees::default();
        let alpha = program.semantic_domains.intern("Alpha");
        let beta = program.semantic_domains.intern("Beta");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let alpha_type = constrained(&mut program, base, [declared("Alpha", alpha)]);
        let beta_type = constrained(&mut program, base, [declared("Beta", beta)]);

        let alpha_identity: NormalizedTypeIdentity = program.normalized_type_identity(alpha_type);
        assert_ne!(alpha_identity, program.normalized_type_identity(beta_type));
    }
}

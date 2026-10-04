//! The normalized identity carriers: type identities, domain expressions,
//! result dispatch sets and domain terms.
//!
//! A named callable's identity is not among them: it is interned once per
//! program revision in `callable_identity.rs` and named by a handle, so no
//! carrier holds its path and parameter signature as rendered text.

use crate::TypedTrees;
use crate::typed_trees::type_system::type_identity::constraint_identity::normalized_domain_term;
use crate::types::TypeConstraintNode;
use arena::HandleSpan;
use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NormalizedTypeIdentity(pub(crate) String);

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

    pub(crate) fn from_constraints(
        program: &TypedTrees,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> Self {
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

/// The canonical set of domains that may select a named-machine or
/// requirement overload from its expected result type.
///
/// Unlike [`NormalizedDomainExpression`], this set expands transparent domain
/// aliases and omits predicate-only refinements. Arithmetic policies, domains
/// with a semantic role or establishment route, and explicit empty tags remain
/// dispatch-bearing. Terms are sorted and deduplicated so authored conjunction
/// order cannot affect overload identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NormalizedResultDispatchSet {
    pub(crate) terms: Vec<NormalizedDomainTerm>,
}

impl NormalizedResultDispatchSet {
    pub fn terms(&self) -> &[NormalizedDomainTerm] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn identity(&self) -> String {
        self.terms
            .iter()
            .map(|term| match term {
                NormalizedDomainTerm::Arithmetic(name) => format!("arithmetic:{name}"),
                NormalizedDomainTerm::Compiler(identity) => format!("compiler:{identity}"),
                NormalizedDomainTerm::Declared(name) => format!("declared:{name}"),
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Compiler(String),
    Declared(String),
}

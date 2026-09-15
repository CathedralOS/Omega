//! The normalized identity carriers: type identities, domain expressions,
//! result dispatch sets, named callable identities and domain terms.

use crate::TypedTrees;
use crate::typed_trees::type_system::type_identity::constraint_identity::{
    atom, compound, normalized_domain_term,
};
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

/// Canonical overload identity for an explicit named machine or requirement.
/// The result carrier and predicate-only refinements are deliberately absent:
/// a declaration is selected by path, parameter signature, and the exact set
/// of dispatch-bearing result domains.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedNamedCallableIdentity {
    pub(crate) path: String,
    pub(crate) parameters: String,
    pub(crate) result_dispatch: NormalizedResultDispatchSet,
}

impl NormalizedNamedCallableIdentity {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn parameters(&self) -> &str {
        &self.parameters
    }

    pub fn result_dispatch(&self) -> &NormalizedResultDispatchSet {
        &self.result_dispatch
    }

    pub fn identity(&self) -> String {
        compound(
            "named-callable",
            [
                atom("path", &self.path),
                atom("parameters", &self.parameters),
                atom("result-dispatch", &self.result_dispatch.identity()),
            ],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Compiler(String),
    Declared(String),
}

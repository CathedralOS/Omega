//! Deterministic semantic identity for typed type references.
//!
//! Diagnostic rendering is deliberately not an identity oracle. In
//! particular, a domain conjunction is commutative and idempotent, while its
//! authored spelling has an order and may repeat terms. This module owns the
//! canonical form used by type equality, specialization, and published plan
//! identities (decision 19 / DOM4).
//!
//! This file owns the `TypedTrees` identity queries. `normalized_identities.rs`
//! holds the identity carriers, `identity_context.rs` normalizes type
//! references in context, `constraint_identity.rs` normalizes constraints and
//! domains, `open_index_identity.rs` identifies open index operations and
//! `result_dispatch_terms.rs` collects result dispatch terms.

mod constraint_identity;
mod identity_context;
mod normalized_identities;
mod open_index_identity;
mod result_dispatch_terms;
mod substitution;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod visitor_tests;

pub use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};
pub use normalized_identities::{
    NormalizedDomainExpression, NormalizedDomainTerm, NormalizedNamedCallableIdentity,
    NormalizedResultDispatchSet, NormalizedTypeIdentity,
};

use crate::TypedTrees;
use crate::typed_trees::type_system::type_identity::constraint_identity::atom;
use crate::typed_trees::type_system::type_identity::constraint_identity::compound;
use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext;
pub use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification;
use crate::typed_trees::type_system::type_identity::identity_context::normalize_type_reference;
use crate::typed_trees::type_system::type_identity::result_dispatch_terms::collect_result_dispatch_terms;
use crate::types::{TypeConstraintNode, TypeReferenceHandle};
use arena::HandleSpan;
use std::cell::Cell;
use symbols::SymbolHandle;

/// One request for a normalized type identity: the type reference, the
/// generic binders it may mention, the exact type-parameter substitutions
/// to follow first, and whether nominals carry their package owner.
#[derive(Clone, Copy)]
pub struct TypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub qualification: TypeIdentityQualification,
}

impl TypeIdentityRequest<'_> {
    /// The plain local identity of one type reference.
    pub const fn ordinary(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::Ordinary,
        }
    }

    /// The package-graph identity of one type reference.
    pub const fn package_qualified(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::PackageQualified,
        }
    }
}

/// One request for an exact-owner identity: package qualification where every
/// source-backed toolchain nominal must match one of the exact toolchain
/// sources and every other non-binder nominal must have a managed package
/// owner, failing closed otherwise.
#[derive(Clone, Copy)]
pub struct ExactOwnerTypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
}

impl TypedTrees {
    /// Canonical identity for an exact resolved declaration that may cross a
    /// target-neutral/target-closed compiler boundary. Managed package symbols
    /// retain their package digest; toolchain symbols retain their closed
    /// toolchain owner. Local or provenance-free declarations fail closed.
    pub fn normalized_hermetic_symbol_identity(
        &self,
        symbol: SymbolHandle,
    ) -> Result<String, String> {
        if !symbol.is_valid() {
            return Err("a hermetic identity contains an unresolved declaration".to_owned());
        }
        let path = self.symbols.display_path(symbol, "::");
        if let Some(package) = self.symbols.symbol_package_identity(symbol) {
            let mut owner = String::with_capacity(64);
            use std::fmt::Write as _;
            for byte in package.digest() {
                let _ = write!(owner, "{byte:02x}");
            }
            return Ok(format!("package:{owner}::{path}"));
        }
        match self.symbols.symbol_source_origin(symbol) {
            Some(source::SourceOrigin::Toolchain) => Ok(format!("toolchain::{path}")),
            Some(origin) => Err(format!(
                "declaration `{path}` has non-hermetic source origin `{origin:?}`"
            )),
            None => Err(format!(
                "declaration `{path}` has no retained source/package provenance"
            )),
        }
    }

    pub fn normalized_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::ordinary(type_reference))
    }

    /// Canonical type identity for a package graph. Every non-binder nominal
    /// carries both its stable declaration path and its exact source owner:
    /// the managed package-key digest, the toolchain marker, or an explicit
    /// unresolved marker. Ordinary type identity deliberately remains local
    /// to one compilation and is unchanged by this stronger form.
    pub fn package_qualified_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::package_qualified(type_reference))
    }

    /// The one binder- and substitution-aware identity entry. A binder symbol
    /// is replaced before serialization, so renaming the source binder cannot
    /// change the normalized contract while concrete declaration paths remain
    /// fully qualified; substitutions replace exact type-parameter symbols
    /// with concrete type references first. Binder substitutions are applied
    /// before owner qualification, preserving alpha-normalization without
    /// falsely assigning a package owner to a local telescope variable.
    pub fn type_identity(&self, request: TypeIdentityRequest<'_>) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            request.type_reference,
            &TypeIdentityContext {
                binders: request.binders,
                substitutions: request.substitutions,
                active_const_substitutions: &[],
                exact_toolchain_sources: &[],
                missing_exact_nominal_owner: None,
                qualification: request.qualification,
            },
        ))
    }

    /// Package-review identity that replaces the generic toolchain marker
    /// with the exact compiler-validated source identity for every source-
    /// backed toolchain nominal. Every other non-binder nominal must have a
    /// managed package owner; unresolved ownership fails closed. `SourceId`
    /// remains an internal join key and never enters the normalized output.
    pub fn exact_owner_type_identity(
        &self,
        request: ExactOwnerTypeIdentityRequest<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = NormalizedTypeIdentity(normalize_type_reference(
            self,
            request.type_reference,
            &TypeIdentityContext {
                binders: request.binders,
                substitutions: request.substitutions,
                active_const_substitutions: &[],
                exact_toolchain_sources: request.exact_toolchain_sources,
                missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                qualification: TypeIdentityQualification::PackageQualified,
            },
        ));
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    /// Exact-owner identity for one already-resolved nominal declaration.
    /// Compiler builtins use their closed semantic atom; authored nominals
    /// require a managed package owner or exact toolchain source.
    pub fn exact_owner_nominal_identity(
        &self,
        symbol: SymbolHandle,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Option<NormalizedTypeIdentity> {
        if !symbol.is_valid() {
            return None;
        }
        let path = self.symbols.display_path(symbol, "::");
        if path.is_empty() {
            return None;
        }
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = NormalizedTypeIdentity(
            TypeIdentityContext {
                binders: &[],
                substitutions: &[],
                active_const_substitutions: &[],
                exact_toolchain_sources,
                missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                qualification: TypeIdentityQualification::PackageQualified,
            }
            .qualify_non_binder_name(self, symbol, path),
        );
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    pub fn normalized_domain_expression(
        &self,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> NormalizedDomainExpression {
        NormalizedDomainExpression::from_constraints(self, constraints)
    }

    /// Normalize the dispatch-bearing domain set on the outer result value.
    /// Reference and nested constrained shells are transparent; domains inside
    /// an aggregate element or generic argument belong to that nested value and
    /// do not select the enclosing result overload.
    pub fn normalized_result_dispatch_set(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedResultDispatchSet {
        let mut terms = Vec::new();
        collect_result_dispatch_terms(self, type_reference, &mut terms, &mut Vec::new());
        terms.sort();
        terms.dedup();
        NormalizedResultDispatchSet { terms }
    }

    /// Canonical identity of one top-level named machine overload. A machine's
    /// first state is its callable entry signature; explicit sibling states do
    /// not create additional top-level overloads.
    pub fn normalized_machine_overload_identity(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<NormalizedNamedCallableIdentity> {
        let entry = self.machine_states(machine).first()?;
        // Module ownership lives on the exact symbol, not the authored
        // machine spelling. Module-free and source-free semantic fixtures
        // retain their existing declaration path.
        let module_path = self
            .symbols
            .symbol_module(machine.symbol)
            .is_valid()
            .then(|| self.symbols.display_path(machine.symbol, "::"));
        Some(self.normalized_named_callable_identity(
            module_path.as_deref().unwrap_or(machine.name.as_str()),
            machine.symbol,
            self.machine_type_parameters(machine),
            self.state_parameters(entry),
            entry.return_type,
        ))
    }

    /// Resolve one canonical machine-overload identity exactly. A missing or
    /// duplicate identity returns `None`; callers must not fall back to a
    /// short machine spelling.
    pub fn machine_by_normalized_overload_identity(
        &self,
        identity: &str,
    ) -> Option<&crate::machine::Machine> {
        let mut matches = self.machines().iter().filter(|machine| {
            self.normalized_machine_overload_identity(machine)
                .is_some_and(|candidate| candidate.identity() == identity)
        });
        let machine = matches.next()?;
        matches.next().is_none().then_some(machine)
    }

    /// Canonical identity of one trait machine requirement overload.
    pub fn normalized_trait_requirement_overload_identity(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
        requirement: &crate::signature::StateSignature,
    ) -> NormalizedNamedCallableIdentity {
        let mut type_parameters = self.trait_type_parameters(trait_definition).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        self.normalized_named_callable_identity(
            &format!("{}::{}", trait_definition.name, requirement.name),
            trait_definition.symbol,
            &type_parameters,
            self.state_signature_parameters(requirement),
            requirement.return_type,
        )
    }

    /// Canonical identity of one compile-time machine-parameter callable.
    /// The declaring machine and all of its generic binders participate in
    /// the identity, followed by binders authored directly on the callable
    /// contract. This is the requirement-side analogue of a top-level named
    /// machine overload; consumers must not reconstruct it from the parameter
    /// spelling alone.
    pub fn normalized_machine_parameter_overload_identity(
        &self,
        declaring_machine: &crate::machine::Machine,
        requirement: &crate::signature::StateSignature,
    ) -> NormalizedNamedCallableIdentity {
        let mut type_parameters = self.machine_type_parameters(declaring_machine).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        self.normalized_named_callable_identity(
            &format!("{}::{}", declaring_machine.name, requirement.name),
            declaring_machine.symbol,
            &type_parameters,
            self.state_signature_parameters(requirement),
            requirement.return_type,
        )
    }

    /// Canonical identity of one explicitly named operator requirement.
    /// Unspelled boundary operators participate in the same result-domain
    /// lookup as other named requirements; fixed spellings remain
    /// operand-directed and use this only for declaration diagnostics.
    pub fn normalized_operator_overload_identity(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> NormalizedNamedCallableIdentity {
        let path = self
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let flags = self
            .operator_parameters(operator)
            .iter()
            .map(|parameter| {
                format!(
                    "self={};mutable={};const={}",
                    parameter.is_self, parameter.is_mutable, parameter.is_const
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        NormalizedNamedCallableIdentity {
            path,
            // Operator operand identity already normalizes generic binders by
            // first structural occurrence, including declarations whose type
            // parameter lists are reordered. Retain parameter modes beside it.
            parameters: format!(
                "operands({});flags({flags})",
                crate::operator::operator_operand_signature(self, operator)
            ),
            result_dispatch: self.normalized_result_dispatch_set(operator.return_type),
        }
    }

    fn normalized_named_callable_identity(
        &self,
        path: &str,
        owner_symbol: SymbolHandle,
        type_parameters: &[crate::data::TypeParameter],
        parameters: &[crate::signature::StateParameter],
        return_type: TypeReferenceHandle,
    ) -> NormalizedNamedCallableIdentity {
        let mut binders = Vec::with_capacity(type_parameters.len() + 1);
        if owner_symbol.is_valid() {
            binders.push((owner_symbol, "$Self".to_owned()));
        }
        binders.extend(
            type_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol.is_valid())
                .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
        );
        let parameters = parameters
            .iter()
            .map(|parameter| {
                compound(
                    "parameter",
                    [
                        atom("self", if parameter.is_self { "yes" } else { "no" }),
                        atom("mutable", if parameter.is_mutable { "yes" } else { "no" }),
                        atom("const", if parameter.is_const { "yes" } else { "no" }),
                        self.type_identity(TypeIdentityRequest {
                            binders: &binders,
                            ..TypeIdentityRequest::ordinary(parameter.type_reference)
                        })
                        .into_string(),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        NormalizedNamedCallableIdentity {
            path: path.to_owned(),
            parameters,
            result_dispatch: self.normalized_result_dispatch_set(return_type),
        }
    }
}

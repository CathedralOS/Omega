//! Typed preparation: every program's typed trees cross this stage, by one
//! path, on their way to the checker.
//!
//! The stage is the ordered list of passes in [`PASSES`]. Each pass takes
//! typed trees and returns typed trees. A pass either does its work or
//! refuses, by name, a request it does not serve; a pass with nothing to do
//! for a program changes nothing. Only a program that has completed the list
//! can enter source lowering. The checker judges what the passes write.

#![forbid(unsafe_code)]

use source_declarations::declaration_selection;
#[cfg(test)]
use source_symbol_table::source_inputs;
use source_symbol_table::source_symbols;
use typed_trees::source_state;

mod passes;
mod product_operands;

#[cfg(any(test, feature = "test-support"))]
pub use product_operands::PreparedProductRequirement;
pub use product_operands::{
    prepare_product_operand_application, prepare_product_requirement_operand,
    prepare_retained_product_application,
};

use crate::source_state::{FrozenTypedTrees, TypedTrees};
use diagnostics::Diagnostic;

/// One step of typed preparation and the name its refusals carry.
struct Pass {
    name: &'static str,
    run: fn(&mut TypedTrees) -> Result<(), Vec<Diagnostic>>,
}

/// The order is part of the stage's contract: a later pass may read what an
/// earlier one wrote. Specialization and selection passes belong before the
/// annotations, which describe the bodies those passes produce.
const PASSES: &[Pass] = &[
    Pass {
        name: "specialization",
        run: crate::passes::specialization::prepare,
    },
    Pass {
        name: "domain applications",
        run: crate::passes::domain_applications::prepare,
    },
    Pass {
        name: "record patterns",
        run: crate::passes::record_patterns::prepare,
    },
];

pub use source_declarations::source_selections::WireCompatibilityDemand;

/// Work the caller asks of this stage beside the program itself. It is a
/// request, not evidence that preparation or checking occurred.
#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    admits_pending_inputs: bool,
    deferred_layout_applications: &'request [crate::source_state::types::TypeReferenceHandle],
    wire_compatibility_demands: &'request [WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            deferred_layout_applications: &[],
            wire_compatibility_demands: &[],
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub const fn preliminary() -> Self {
        Self {
            admits_pending_inputs: true,
            ..Self::settled()
        }
    }

    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'request [WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }

    pub fn admission(&self) -> PreparationAdmission {
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            deferred_layout_applications: self.deferred_layout_applications.to_vec(),
            wire_compatibility_demands: self.wire_compatibility_demands.to_vec(),
        }
    }

    /// Deferred layouts and wire compatibility still need dedicated passes.
    /// Dropping either request would publish an incompletely prepared product.
    fn require_served(&self) -> Result<(), Vec<Diagnostic>> {
        let unserved = [
            (
                !self.deferred_layout_applications.is_empty(),
                "deferred layout applications",
            ),
            (
                !self.wire_compatibility_demands.is_empty(),
                "wire compatibility demands",
            ),
        ]
        .into_iter()
        .filter(|(requested, _)| *requested)
        .map(|(_, request)| refusal("requested work", request))
        .collect::<Vec<_>>();
        if unserved.is_empty() {
            Ok(())
        } else {
            Err(unserved)
        }
    }
}

/// An owned copy of the requested policy only; it carries no checked receipt.
#[derive(Debug, Clone, PartialEq, record_storage::Record)]
pub struct PreparationAdmission {
    admits_pending_inputs: bool,
    deferred_layout_applications: Vec<crate::source_state::types::TypeReferenceHandle>,
    wire_compatibility_demands: Vec<WireCompatibilityDemand>,
}

impl PreparationAdmission {
    #[cfg(any(test, feature = "test-support"))]
    pub fn deferred_layout_applications(
        &self,
    ) -> &[crate::source_state::types::TypeReferenceHandle] {
        &self.deferred_layout_applications
    }
}

/// The one wording of a refusal from this stage, so a reader can tell which
/// pass declined which request.
fn refusal(pass: &str, request: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "no product route yet: typed preparation: {pass}: {request}"
    ))
}

/// A shared snapshot of the exact trees that completed every stage 04 pass.
///
/// Cloning the result retains the same source owner for later judgments.
///
/// ```
/// use typed_trees::source_state::TypedTrees;
/// use typed_trees_to_prepared_typed_trees::{PreparationRequest, prepare_typed_trees};
/// let (prepared, _) = prepare_typed_trees(TypedTrees::default(), &PreparationRequest::settled()).unwrap();
/// assert!(std::sync::Arc::ptr_eq(&prepared, &prepared.clone()));
/// ```
///
/// A read-only snapshot alone cannot establish that those passes ran.
///
/// ```compile_fail,E0277
/// use typed_trees::source_state::FrozenTypedTrees;
/// use typed_trees_to_prepared_typed_trees::PreparedTypedTrees;
/// let _: PreparedTypedTrees = FrozenTypedTrees::default().into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTypedTrees(std::sync::Arc<FrozenTypedTrees>);

impl PreparedTypedTrees {
    /// Return to mutable trees for a new preparation after edits.
    pub fn into_inner(self) -> TypedTrees {
        std::sync::Arc::unwrap_or_clone(self.0).into_inner()
    }
}

impl std::ops::Deref for PreparedTypedTrees {
    type Target = std::sync::Arc<FrozenTypedTrees>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn prepare_typed_trees(
    mut program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(PreparedTypedTrees, PreparationAdmission), Vec<Diagnostic>> {
    request.require_served()?;
    for pass in PASSES {
        (pass.run)(&mut program).map_err(|refused| {
            refused
                .into_iter()
                .map(|mut diagnostic| {
                    diagnostic.message = refusal(pass.name, &diagnostic.message).message;
                    diagnostic
                })
                .collect::<Vec<_>>()
        })?;
    }
    Ok((PreparedTypedTrees(std::sync::Arc::new(program.into())), request.admission()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_domain_family_prepares_each_retained_carrier_without_rewriting_the_source() {
        use crate::source_state::data::{TypeParameter, TypeParameterKind};
        use crate::source_state::domain::DomainDefinition;
        use crate::source_state::name::Identifier;
        use crate::source_state::types::TypeReferenceNode;
        use crate::source_symbols::SymbolTableBuilder;
        use symbols::{SymbolKind, SymbolNameRef};
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = builder.insert_children(
            root,
            crate::source_symbols::builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Domain, SymbolNameRef::Static("Distance"))]),
        );
        let domain = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let children = builder.insert_children(
            domain,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::Parameter, SymbolNameRef::Static("self")),
            ],
        );
        let parameter = children.handle_at(0);
        let subject = children.handle_at(1);
        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let target = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: parameter,
                name: Identifier::generated("T"),
            });
        let parameters = program.data_type_parameters.insert_many([TypeParameter {
            symbol: parameter,
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        }]);
        let identity = program.semantic_domains.intern("Distance");
        program.push_domain_definition(DomainDefinition {
            symbol: domain,
            name: Identifier::generated("Distance"),
            target_type: target,
            type_parameters: parameters,
            subject_symbol: subject,
            semantic_id: identity,
            ..Default::default()
        });
        let mut retained = Vec::new();
        for width in [32, 64] {
            let symbol = program
                .symbols
                .builtin_atom_symbol(symbols::BuiltinTypeAtom::Integer(
                    numerics::integer_carrier::FixedInteger::unsigned(width),
                ))
                .unwrap();
            let carrier = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated("diagnostic alias"),
                });
            let identity = program
                .retain_domain_application(domain, carrier, &[])
                .unwrap();
            assert_eq!(
                program
                    .retain_domain_application(domain, carrier, &[])
                    .unwrap(),
                identity
            );
            assert!(
                program
                    .domain_definition_for_instance(domain, identity)
                    .is_none()
            );
            assert!(
                program
                    .retain_domain_application(domain, carrier, &[carrier])
                    .is_err()
            );
            retained.push((carrier, identity));
        }
        assert_ne!(retained[0].1, retained[1].1);
        let (prepared, _) = prepare_typed_trees(program, &PreparationRequest::settled()).unwrap();
        let prepared = &*prepared;
        for (carrier, identity) in retained {
            let theory = prepared
                .domain_definition_for_instance(domain, identity)
                .unwrap();
            assert!(prepared.domain_type_parameters(theory).is_empty());
            assert_eq!(
                prepared.package_qualified_type_identity(theory.target_type),
                prepared.package_qualified_type_identity(carrier)
            );
            assert_eq!(
                prepared.symbols.authored_origin(theory.subject_symbol),
                subject
            );
            assert_eq!(
                theory.symbol, domain,
                "application retains the declaring owner"
            );
        }
        let declaration = prepared.domain_definition(domain).unwrap();
        assert_eq!(declaration.target_type, target);
        assert_eq!(declaration.subject_symbol, subject);
        assert_eq!(prepared.domain_type_parameters(declaration).len(), 1);
    }

    #[test]
    fn the_program_keeps_its_identity_and_the_request_its_policy() {
        let program = TypedTrees::default();
        let identity = program.identity;
        let request = PreparationRequest::preliminary();
        let expected = request.admission();
        let (frozen, metadata) = prepare_typed_trees(program, &request).unwrap();
        assert_eq!(frozen.identity.get(), identity.get());
        assert!(metadata.admits_pending_inputs);
        assert_eq!(metadata, expected);
    }

    #[test]
    fn a_program_no_pass_has_work_for_crosses_unchanged() {
        // A retained declaration no pass reads is not a reason to refuse.
        let mut program = TypedTrees::default();
        program
            .tables
            .mathematical_definitions
            .insert(crate::source_state::mathematical::MathematicalDefinition::default());
        let before = program.clone();
        let (frozen, _) = prepare_typed_trees(program, &PreparationRequest::settled()).unwrap();
        assert_eq!(frozen.into_inner(), before);
    }

    #[test]
    fn requested_work_without_a_pass_is_refused_by_name() {
        let deferred = [crate::source_state::types::TypeReferenceHandle::invalid()];
        let request = PreparationRequest {
            deferred_layout_applications: &deferred,
            ..PreparationRequest::settled()
        };
        let Err(refused) = prepare_typed_trees(TypedTrees::default(), &request) else {
            panic!("requested layout work must not be dropped");
        };
        assert_eq!(
            refused[0].message,
            "no product route yet: typed preparation: requested work: deferred layout applications"
        );
    }
}

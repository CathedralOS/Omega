//! Typed preparation: every program's typed trees cross this stage, by one
//! path, on their way to the checker.
//!
//! The stage is the ordered list of passes in [`PASSES`]. Each pass takes
//! typed trees and returns typed trees. A pass either does its work or
//! refuses, by name, a request it does not serve; a pass with nothing to do
//! for a program changes nothing. No program is frozen without running the
//! list, and nothing here judges a program: what a pass writes is input the
//! checker reads, never a verdict.

use diagnostics::Diagnostic;
use language_core::inline_assembly::AsmAuthorityAdmission;
use typed_trees::opaque_representations::OpaqueRepresentationSelection;
use typed_trees::{FrozenTypedTrees, TypedTrees};

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
        run: |program| {
            program.prepare_domain_applications();
            Ok(())
        },
    },
    Pass {
        name: "record patterns",
        run: crate::passes::record_patterns::prepare,
    },
];

/// Work the caller asks of this stage beside the program itself. It is a
/// request, not evidence that preparation or checking occurred.
#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    admits_pending_inputs: bool,
    deferred_layout_applications: &'request [typed_trees::types::TypeReferenceHandle],
    asm_authority: Option<AsmAuthorityAdmission>,
    opaque_representation_selections: &'request [OpaqueRepresentationSelection],
    wire_compatibility_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            deferred_layout_applications: &[],
            asm_authority: None,
            opaque_representation_selections: &[],
            wire_compatibility_demands: &[],
        }
    }

    pub const fn preliminary() -> Self {
        Self {
            admits_pending_inputs: true,
            ..Self::settled()
        }
    }

    pub const fn with_asm_authority(mut self, authority: AsmAuthorityAdmission) -> Self {
        self.asm_authority = Some(authority);
        self
    }

    pub const fn with_opaque_representation_selections(
        mut self,
        selections: &'request [OpaqueRepresentationSelection],
    ) -> Self {
        self.opaque_representation_selections = selections;
        self
    }

    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }

    pub fn admission(&self) -> PreparationAdmission {
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            deferred_layout_applications: self.deferred_layout_applications.to_vec(),
            asm_authority: self.asm_authority,
            opaque_representation_selections: self.opaque_representation_selections.to_vec(),
            wire_compatibility_demands: self.wire_compatibility_demands.to_vec(),
        }
    }

    /// The three kinds of requested work have no pass yet. Each was served by
    /// the deleted checker (layout evaluation, representation admission, wire
    /// compatibility), so none may be dropped and the program presented as
    /// prepared.
    fn require_served(&self) -> Result<(), Vec<Diagnostic>> {
        let unserved = [
            (
                !self.deferred_layout_applications.is_empty(),
                "deferred layout applications",
            ),
            (
                !self.opaque_representation_selections.is_empty(),
                "opaque representation selections",
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
#[derive(Debug, Clone, PartialEq, compiler_storage::Record)]
pub struct PreparationAdmission {
    admits_pending_inputs: bool,
    deferred_layout_applications: Vec<typed_trees::types::TypeReferenceHandle>,
    asm_authority: Option<AsmAuthorityAdmission>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    wire_compatibility_demands: Vec<language_semantics::wire::WireCompatibilityDemand>,
}

impl PreparationAdmission {
    pub fn deferred_layout_applications(&self) -> &[typed_trees::types::TypeReferenceHandle] {
        &self.deferred_layout_applications
    }
}

/// Compatibility name for the request metadata; it has no judgment output.
pub type TypedPreparation = PreparationAdmission;

/// The one wording of a refusal from this stage, so a reader can tell which
/// pass declined which request.
fn refusal(pass: &str, request: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "no product route yet: typed preparation: {pass}: {request}"
    ))
}

pub fn prepare_typed_trees(
    mut program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(FrozenTypedTrees, TypedPreparation), Vec<Diagnostic>> {
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
    Ok((program.into(), request.admission()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_domain_family_prepares_each_retained_carrier_without_rewriting_the_source() {
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::domain::DomainDefinition;
        use typed_trees::name::Identifier;
        use typed_trees::types::TypeReferenceNode;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = builder.insert_children(
            root,
            symbols::builtin_type_symbols()
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
        let request =
            PreparationRequest::preliminary().with_asm_authority(AsmAuthorityAdmission::HOSTED);
        let expected = request.admission();
        let (frozen, metadata) = prepare_typed_trees(program, &request).unwrap();
        assert_eq!(frozen.identity.get(), identity.get());
        assert_eq!(metadata, expected);
    }

    #[test]
    fn a_program_no_pass_has_work_for_crosses_unchanged() {
        // A retained declaration no pass reads is not a reason to refuse.
        let mut program = TypedTrees::default();
        program
            .tables
            .mathematical_definitions
            .insert(typed_trees::mathematical::MathematicalDefinition::default());
        let before = program.clone();
        let (frozen, _) = prepare_typed_trees(program, &PreparationRequest::settled()).unwrap();
        assert!(*frozen == before);
    }

    #[test]
    fn requested_work_without_a_pass_is_refused_by_name() {
        let deferred = [typed_trees::types::TypeReferenceHandle::invalid()];
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

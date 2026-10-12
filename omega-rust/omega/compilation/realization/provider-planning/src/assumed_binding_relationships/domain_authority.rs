//! Public qualification demands and the original checked issuer evidence.
//!
//! A domain fact, an owner-authorized issuance route, and an admitted receipt
//! are different things. This projection retains exact source coordinates and
//! checking's authorization. It neither turns a missing route into permission
//! nor manufactures a receipt from a Build registration or semantic assumption.

mod content;
pub use content::{AssumedBindingDomainAuthorityContent, retain_assumed_binding_domain_authority};

use super::AssumedBindingContractSide;
use super::declarations::DeclaredRequirement;
use super::semantic_obligations::owns_contract;
use arena::Handle;
use checked_trees::typed_trees::domain::ProofFact;
use checked_trees::typed_trees::expression::ExpressionHandle;
use checked_trees::typed_trees::types::{
    TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use checked_trees::{
    BoundaryQualificationAuthorization, CheckedTrees, ContractProofFact, ContractProofFactKind,
};
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

/// Subject identity in that side's public declaration. An authored field or
/// other expression remains exact; it is not redirected onto the whole result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingDomainSubject {
    Parameter {
        ordinal: usize,
        symbol: SymbolHandle,
    },
    Result,
    Expression(ExpressionHandle),
}

/// Both variants point into the prepared relationship's original checked owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssumedBindingDomainSource {
    Contract(Handle<ContractProofFact>),
    DeclaredType {
        type_reference: TypeReferenceHandle,
        constraint_ordinal: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingDomainDemand {
    side: AssumedBindingContractSide,
    subject: AssumedBindingDomainSubject,
    source: AssumedBindingDomainSource,
    kind: ContractProofFactKind,
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
    authorization: Option<BoundaryQualificationAuthorization>,
}

impl AssumedBindingDomainDemand {
    pub const fn side(&self) -> AssumedBindingContractSide {
        self.side
    }
    pub const fn subject(&self) -> AssumedBindingDomainSubject {
        self.subject
    }
    pub const fn source(&self) -> AssumedBindingDomainSource {
        self.source
    }
    pub const fn kind(&self) -> ContractProofFactKind {
        self.kind
    }
    pub const fn domain(&self) -> SymbolHandle {
        self.domain
    }
    pub const fn semantic_domain(&self) -> SemanticDomainId {
        self.semantic_domain
    }
    /// An exact checked boundary fact authorization, never consent or a
    /// completed fresh-claim receipt. Declared qualifications alone return None.
    pub const fn authorization(&self) -> Option<BoundaryQualificationAuthorization> {
        self.authorization
    }
}

/// The demand coordinate chooses its original source, rather than rediscovering
/// a domain by a semantic label. Type qualifications retain the instantiated
/// domain reference's routes; membership clauses retain the authored domain
/// declaration and their arguments in the contract fact. The returned slice
/// grants no authority and must still join the exact subject at installation.
pub(super) fn establishment_routes<'owner>(
    program: &'owner CheckedTrees,
    demand: &AssumedBindingDomainDemand,
) -> Option<&'owner [language_semantics::DomainEstablishmentRoute]> {
    match demand.source {
        AssumedBindingDomainSource::DeclaredType {
            type_reference,
            constraint_ordinal,
        } => {
            let TypeReferenceNode::Constrained { constraints, .. } =
                program.type_reference_table.type_reference(type_reference)
            else {
                return None;
            };
            let TypeConstraintNode::Domain(domain) = program
                .type_reference_table
                .constraints(*constraints)
                .get(constraint_ordinal)?
            else {
                return None;
            };
            (domain.symbol == demand.domain && domain.semantic_id == demand.semantic_domain)
                .then_some(domain.establishment_routes.as_slice())
        }
        AssumedBindingDomainSource::Contract(handle) => {
            let checked = program.facts.proof.contract_facts.get(handle);
            let ProofFact::Membership(membership) = program.proof_facts.get(checked.fact) else {
                return None;
            };
            if membership.domain_symbol != demand.domain
                || membership.semantic_domain != demand.semantic_domain
            {
                return None;
            }
            // Indexed memberships retain their original applied arguments in
            // this fact; routes belong to the selected family declaration.
            // A family declaration's semantic id need not equal the instance.
            Some(
                program
                    .domain_definition(membership.domain_symbol)?
                    .establishment_routes
                    .as_slice(),
            )
        }
    }
}

pub(super) fn capture(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
) -> Vec<AssumedBindingDomainDemand> {
    let mut demands = Vec::new();
    for (side, declaration) in [
        (AssumedBindingContractSide::Required, required),
        (AssumedBindingContractSide::Supplied, supplied),
    ] {
        for (ordinal, parameter) in declaration.parameters.iter().enumerate() {
            capture_type(
                program,
                side,
                AssumedBindingDomainSubject::Parameter {
                    ordinal,
                    symbol: parameter.symbol,
                },
                ContractProofFactKind::Requires,
                parameter.type_reference,
                &mut demands,
            );
        }
        capture_type(
            program,
            side,
            AssumedBindingDomainSubject::Result,
            ContractProofFactKind::Ensures,
            declaration.result,
            &mut demands,
        );
        for (handle, checked) in program.facts.proof.contract_facts.iter() {
            if !owns_contract(declaration, checked.owner) {
                continue;
            }
            let ProofFact::Membership(membership) = program.proof_facts.get(checked.fact) else {
                continue;
            };
            let subject = if program
                .expression_table
                .names_reserved_result(membership.value)
            {
                AssumedBindingDomainSubject::Result
            } else {
                declaration
                    .parameters
                    .iter()
                    .enumerate()
                    .find_map(|(ordinal, parameter)| {
                        program
                            .expression_table
                            .names_parameter(membership.value, parameter)
                            .then_some(AssumedBindingDomainSubject::Parameter {
                                ordinal,
                                symbol: parameter.symbol,
                            })
                    })
                    .unwrap_or(AssumedBindingDomainSubject::Expression(membership.value))
            };
            demands.push(AssumedBindingDomainDemand {
                side,
                subject,
                source: AssumedBindingDomainSource::Contract(handle),
                kind: checked.kind,
                domain: membership.domain_symbol,
                semantic_domain: membership.semantic_domain,
                authorization: checked.qualification_authorization,
            });
        }
    }
    demands
}

fn capture_type(
    program: &CheckedTrees,
    side: AssumedBindingContractSide,
    subject: AssumedBindingDomainSubject,
    kind: ContractProofFactKind,
    mut type_reference: TypeReferenceHandle,
    demands: &mut Vec<AssumedBindingDomainDemand>,
) {
    // Only qualifications of this public subject and its borrowed referent
    // belong here. Record-field qualifications retain their own declaration
    // and checked dependency facts; they are not whole-subject memberships.
    let mut visited = Vec::new();
    while type_reference.is_valid() && !visited.contains(&type_reference) {
        visited.push(type_reference);
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for (constraint_ordinal, constraint) in program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .enumerate()
                {
                    let TypeConstraintNode::Domain(domain) = constraint else {
                        continue;
                    };
                    demands.push(AssumedBindingDomainDemand {
                        side,
                        subject,
                        source: AssumedBindingDomainSource::DeclaredType {
                            type_reference,
                            constraint_ordinal,
                        },
                        kind,
                        domain: domain.symbol,
                        semantic_domain: domain.semantic_id,
                        authorization: None,
                    });
                }
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            _ => break,
        }
    }
}

//! Portable issuer authority from original checked public demands and source custody.
//!
//! The existing public contracts and original requirement applications own the
//! applied domain arguments and captured lexical scopes. Reuse their content
//! with the same authored claim coordinates as semantic obligations; normalizing
//! a copied domain tuple here would substitute a later arena or erase regions.
//! Issuer routes add their exact source-backed declaration identities. A checked
//! boundary authorization is retained separately from a route and from absent
//! authorization on a declared type. None means unavailable original evidence;
//! even a complete component is neither call issuance nor receiving consent.

use super::super::{
    AssumedBindingContractSide, AssumedBindingPublicClaim, AssumedBindingSemanticSource,
    AssumedBindingSemanticSubject, PreparedAssumedBindingRelationship,
};
use super::{AssumedBindingDomainSource, AssumedBindingDomainSubject};
use crate::ProviderPlanningContext;
use crate::digest_encoder::DomainDigestEncoder;
use checked_trees::{CheckedTrees, ContractProofFactKind};
use language_semantics::DomainEstablishmentRoute;
use package_compilation::CheckedSourceCustody;
use symbols::SymbolHandle;

/// Canonical authority content, privately produced after original source sealing.
/// An authenticated empty demand roster has its own digest; missing original
/// ownership is unavailable rather than an empty roster or an issuance receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssumedBindingDomainAuthorityContent([u8; 32]);

impl AssumedBindingDomainAuthorityContent {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Join already checked issuer records to the existing source seal. No source
/// checking, proof discharge, constant execution or source hashing repeats.
pub fn retain_assumed_binding_domain_authority(
    relationships: &mut [PreparedAssumedBindingRelationship],
    custody: &CheckedSourceCustody,
) {
    for relationship in relationships {
        relationship.domain_authority_content = capture(relationship, custody);
    }
}

fn capture(
    relationship: &PreparedAssumedBindingRelationship,
    custody: &CheckedSourceCustody,
) -> Option<AssumedBindingDomainAuthorityContent> {
    let program = relationship.declaration_owner();
    if !custody.owns_sources(&program.symbols) {
        return None;
    }
    let revisions = relationship.declaration_revisions()?;
    let applications = relationship.application_content()?;
    let context = ProviderPlanningContext::new(&program.typed, source::DependencyScope::Product);
    let required =
        super::super::declarations::requirement(context, relationship.required()).ok()?;
    let supplied =
        super::super::declarations::requirement(context, relationship.supplied()).ok()?;
    let mut encoder = DomainDigestEncoder::new(b"omega.assumed-binding-domain-authority.v1\0");
    encoder.raw(&revisions.required.digest());
    encoder.raw(&revisions.supplied.digest());
    encoder.raw(&applications.required());
    encoder.raw(&applications.supplied());
    encoder.raw(&relationship.required_public_contract());
    encoder.raw(&relationship.supplied_public_contract());
    encoder.len(relationship.domain_demands().len());
    for (ordinal, demand) in relationship.domain_demands().iter().enumerate() {
        let subject = match demand.subject() {
            AssumedBindingDomainSubject::Parameter { ordinal, symbol } => {
                AssumedBindingSemanticSubject::Parameter { ordinal, symbol }
            }
            AssumedBindingDomainSubject::Result => AssumedBindingSemanticSubject::Result,
            // The original clause/fact coordinate owns the exact expression,
            // including field subjects. No local expression handle is encoded.
            AssumedBindingDomainSubject::Expression(_) => AssumedBindingSemanticSubject::Predicate,
        };
        let origin = match demand.source() {
            AssumedBindingDomainSource::Contract(handle) => {
                AssumedBindingSemanticSource::Contract(handle)
            }
            AssumedBindingDomainSource::DeclaredType {
                type_reference,
                constraint_ordinal,
            } => AssumedBindingSemanticSource::DeclaredType {
                type_reference,
                constraint_ordinal,
            },
        };
        let claim = AssumedBindingPublicClaim::new(demand.side(), subject, origin, None);
        super::super::semantic_content::encode_claim(
            &mut encoder,
            program,
            &required,
            &supplied,
            &claim,
        )?;
        encoder.byte(match demand.kind() {
            ContractProofFactKind::Requires => 1,
            ContractProofFactKind::Ensures => 2,
            ContractProofFactKind::Where => 3,
        });
        // A domain declaration's issuer roster belongs to that declaration's
        // original source revision, independently of this public use's arguments.
        encode_declaration(&mut encoder, program, custody, demand.domain())?;
        let routes = relationship.domain_establishment_routes(ordinal)?;
        encoder.len(routes.len());
        for route in routes {
            match *route {
                DomainEstablishmentRoute::CheckedRequirement {
                    trait_definition,
                    requirement,
                } => {
                    encoder.byte(1);
                    encode_declaration(&mut encoder, program, custody, trait_definition)?;
                    encode_declaration(&mut encoder, program, custody, requirement)?;
                }
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                } => {
                    encoder.byte(2);
                    encode_declaration(&mut encoder, program, custody, boundary_trait)?;
                    encode_declaration(&mut encoder, program, custody, requirement)?;
                }
                DomainEstablishmentRoute::ExactMachine { machine } => {
                    encoder.byte(3);
                    encode_declaration(&mut encoder, program, custody, machine)?;
                }
            }
        }
        match demand.authorization() {
            Some(authorization) => {
                // Authorization is already a checked producer record. It must
                // remain attached to its original required/supplied clause;
                // registration cannot move it to a different issuer or subject.
                let declaration = match demand.side() {
                    AssumedBindingContractSide::Required => &required,
                    AssumedBindingContractSide::Supplied => &supplied,
                };
                if !matches!(demand.source(), AssumedBindingDomainSource::Contract(_))
                    || demand.kind() != ContractProofFactKind::Ensures
                    || authorization.requirement_symbol != declaration.own_service()
                    || authorization.signature_symbol != declaration.symbol()
                {
                    return None;
                }
                encoder.byte(1);
                encode_declaration(
                    &mut encoder,
                    program,
                    custody,
                    authorization.requirement_symbol,
                )?;
                encode_declaration(
                    &mut encoder,
                    program,
                    custody,
                    authorization.signature_symbol,
                )?;
            }
            None => encoder.byte(0),
        }
    }
    Some(AssumedBindingDomainAuthorityContent(encoder.finish()))
}

fn encode_declaration(
    encoder: &mut DomainDigestEncoder,
    program: &CheckedTrees,
    custody: &CheckedSourceCustody,
    symbol: SymbolHandle,
) -> Option<()> {
    let source = custody.declaration_source(&program.symbols, symbol)?;
    let identity = program
        .typed
        .exact_owner_nominal_identity(symbol, custody.exact_toolchain_sources())?;
    let content = program.typed.type_identity_text(identity)?;
    if content.is_empty() {
        return None;
    }
    encoder.raw(&source.unit().revision_identity());
    encoder.u64(source.start());
    encoder.u64(source.end());
    encoder.bytes(content.as_bytes());
    Some(())
}

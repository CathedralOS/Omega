//! Retain authored normal membership promises in the complete service surface.
//! Runtime IDs are assigned later; this roster uses the exact domain application
//! and the result's own named or numbered member identities.

use std::cell::OnceCell;

use arena::HandleSpan;
use declaration_contracts::service_surface::{
    ServiceNormalResultMembership, ServiceResultProjection,
};
use symbols::SymbolHandle;

use crate::source_state::TypedTrees;
use crate::source_state::domain::{DomainIssuance, ProofFact};
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::signature::{SignatureContract, SignatureContractKind};
use crate::source_state::trait_definition::AppliedTraitScope;

pub(super) struct NormalMemberships<'program> {
    program: &'program TypedTrees,
    sources: OnceCell<Result<Vec<(source_text::SourceId, [u8; 32])>, &'static str>>,
}

impl<'program> NormalMemberships<'program> {
    pub(super) fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            sources: OnceCell::new(),
        }
    }

    pub(super) fn collect(
        &self,
        owner: SymbolHandle,
        contracts: HandleSpan<SignatureContract>,
        scope: Option<&AppliedTraitScope<'_>>,
    ) -> Result<Vec<ServiceNormalResultMembership>, &'static str> {
        let program = self.program;
        let contracts = program
            .signature_contracts
            .span(contracts)
            .ok_or("normal membership contract span does not resolve")?;
        let mut memberships = Vec::new();
        for contract in contracts {
            if contract.kind != SignatureContractKind::Ensures {
                continue;
            }
            let facts = program
                .proof_facts
                .span(contract.facts)
                .ok_or("normal membership fact span does not resolve")?;
            for fact in facts {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                let definition = program
                    .domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .ok_or("normal membership has no exact domain definition")?;
                if matches!(definition.issuance(), DomainIssuance::Unrestricted)
                    && definition.access_policy.is_none()
                {
                    continue;
                }
                let sources = self
                    .sources
                    .get_or_init(|| {
                        source_admission::source_identity::toolchain_source_identities(
                            program.symbols.source_files(),
                        )
                        .map_err(|_| "normal membership lost its exact toolchain source identities")
                    })
                    .as_ref()
                    .map_err(|reason| *reason)?;
                let (substitutions, starts) = scope.map_or((&[][..], &[][..]), |scope| {
                    (
                        scope.substitutions.as_slice(),
                        scope.substitution_scope_starts.as_slice(),
                    )
                });
                let domain = program.runtime_membership_domain_identity(
                    membership,
                    sources,
                    substitutions,
                    starts,
                )?;
                let path = self.result_path(owner, membership.value)?;
                memberships.push(ServiceNormalResultMembership { path, domain });
            }
        }
        memberships.sort();
        memberships.dedup();
        Ok(memberships)
    }

    fn result_path(
        &self,
        owner: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Result<Vec<ServiceResultProjection>, &'static str> {
        use declaration_contracts::content::{ContentPlaceRoot, ContentPlaceSegment};
        let subject = crate::source_state::contract_structure::structural_subject_with(
            self.program,
            owner,
            expression,
            |name| {
                if name.reserved_result_of.is_valid() && name.reserved_result_of == owner {
                    Ok(ContentPlaceRoot::Result)
                } else {
                    Err(
                        "normal membership guarantee does not name its exact result or result projection",
                    )
                }
            },
        )?;
        Ok(subject
            .segments
            .into_iter()
            .map(|segment| match segment {
                ContentPlaceSegment::Field(field) => ServiceResultProjection::Field(field.name),
                ContentPlaceSegment::Case(case) => ServiceResultProjection::Case(case.name),
                ContentPlaceSegment::FixedIndex(index) => {
                    ServiceResultProjection::FixedIndex(index)
                }
            })
            .collect())
    }
}

//! Retain authored normal membership promises in the complete service surface.
//! Runtime IDs are assigned later; this roster uses the exact domain application
//! and the result's own named or numbered member identities.

use std::cell::OnceCell;

use arena::HandleSpan;
use language_semantics::service_surface::{ServiceNormalResultMembership, ServiceResultProjection};
use symbols::SymbolHandle;

use crate::TypedTrees;
use crate::domain::{DomainIssuance, ProofFact};
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::signature::{SignatureContract, SignatureContractKind};
use crate::trait_definition::AppliedTraitScope;

pub(super) struct NormalMemberships<'program> {
    program: &'program TypedTrees,
    sources: OnceCell<Result<Vec<(source::SourceId, [u8; 32])>, &'static str>>,
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
                        source::source_identity::toolchain_source_identities(
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
        mut expression: ExpressionHandle,
    ) -> Result<Vec<ServiceResultProjection>, &'static str> {
        let program = self.program;
        let mut reversed = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(expression) {
                return Err("normal membership result expression does not resolve");
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(name)
                    if name.reserved_result_of.is_valid() && name.reserved_result_of == owner =>
                {
                    reversed.reverse();
                    return Ok(reversed);
                }
                ExpressionNode::Member(member) => {
                    let symbol = member.selection.field_symbol();
                    let field = program.unique_declared_field_by_symbol(symbol).ok_or(
                        "normal membership result field has no unique declaration identity",
                    )?;
                    if !field.member_identity().is_valid() {
                        return Err(
                            "normal membership result field has no declared member identity",
                        );
                    }
                    reversed.push(ServiceResultProjection::Field(field.path_identity()));
                    let parent = program.symbols.get(symbol).parent;
                    if !program.symbols.symbols().nodes().is_valid(parent) {
                        return Err("normal membership result field lost its declaration owner");
                    }
                    if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                        let (_, case) = program
                            .variant_owner(parent)
                            .ok_or("normal membership result field lost its exact case")?;
                        reversed.push(ServiceResultProjection::Case(case.path_identity()));
                    }
                    expression = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    let index = program
                        .closed_integer_expression_value_in(indexed.index, owner)
                        .and_then(|index| index.to_u64())
                        .ok_or("normal membership result index is not an exact closed element")?;
                    reversed.push(ServiceResultProjection::FixedIndex(index));
                    expression = indexed.collection;
                }
                _ => {
                    return Err(
                        "normal membership guarantee does not name its exact result or result projection",
                    );
                }
            }
        }
    }
}

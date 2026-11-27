//! Retain authored input and normal-result memberships in the complete service surface.
//! Runtime IDs are assigned later; this roster uses the exact domain application
//! and the result's own named or numbered member identities.

use std::cell::OnceCell;

use arena::HandleSpan;
use service_surface::{
    ServiceDomainMembership, ServiceMembershipProjection, ServiceMembershipSubject,
};
use symbols::SymbolHandle;

use crate::source_state::TypedTrees;
use crate::source_state::domain::ProofFact;
use crate::source_state::expression::{ExpressionHandle, ExpressionNode};
use crate::source_state::signature::{SignatureContract, SignatureContractKind};
use crate::source_state::trait_definition::AppliedTraitScope;

pub(super) struct ServiceMemberships<'program> {
    program: &'program TypedTrees,
    sources: OnceCell<Result<Vec<(source_text::SourceId, [u8; 32])>, &'static str>>,
}

impl<'program> ServiceMemberships<'program> {
    pub(super) fn new(program: &'program TypedTrees) -> Self {
        Self {
            program,
            sources: OnceCell::new(),
        }
    }

    pub(super) fn collect(
        &self,
        owner: SymbolHandle,
        formals: arena::RootRows<'_, crate::source_state::signature::StateParameter>,
        contracts: HandleSpan<SignatureContract>,
        scope: Option<&AppliedTraitScope<'_>>,
    ) -> Result<Vec<ServiceDomainMembership>, &'static str> {
        let program = self.program;
        let contracts = program
            .signature_contracts
            .span(contracts)
            .ok_or("method membership contract span does not resolve")?;
        let mut memberships = Vec::new();
        for contract in contracts {
            if !matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            ) {
                continue;
            }
            let facts = program
                .proof_facts
                .span(contract.facts)
                .ok_or("method membership fact span does not resolve")?;
            for fact in facts {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if contract.kind == SignatureContractKind::Requires
                    && program.fact_restates_formal_domain(formals, fact)
                {
                    continue;
                }
                let definition = program
                    .domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )
                    .ok_or("method membership has no exact domain definition")?;
                if !definition.requires_result_membership() {
                    continue;
                }
                let sources = self
                    .sources
                    .get_or_init(|| {
                        source_admission::source_identity::toolchain_source_identities(
                            program.symbols.source_files(),
                        )
                        .map_err(|_| "method membership lost its exact toolchain source identities")
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
                let (subject, path) = self.subject_path(owner, formals, membership.value)?;
                match (contract.kind, subject) {
                    (SignatureContractKind::Requires, ServiceMembershipSubject::Input(_))
                    | (SignatureContractKind::Ensures, ServiceMembershipSubject::NormalResult) => {}
                    _ => {
                        return Err(
                            "routed membership has no matching method input or normal result",
                        );
                    }
                }
                memberships.push(ServiceDomainMembership {
                    subject,
                    path,
                    domain,
                });
            }
        }
        memberships.sort();
        memberships.dedup();
        Ok(memberships)
    }

    fn subject_path(
        &self,
        owner: SymbolHandle,
        formals: arena::RootRows<'_, crate::source_state::signature::StateParameter>,
        mut expression: ExpressionHandle,
    ) -> Result<(ServiceMembershipSubject, Vec<ServiceMembershipProjection>), &'static str> {
        let program = self.program;
        let mut reversed = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(expression) {
                return Err("method membership expression does not resolve");
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Name(name)
                    if name.reserved_result_of.is_valid() && name.reserved_result_of == owner =>
                {
                    reversed.reverse();
                    return Ok((ServiceMembershipSubject::NormalResult, reversed));
                }
                ExpressionNode::Name(name)
                    if !name.reserved_result_of.is_valid()
                        && !name.static_application.is_valid() =>
                {
                    let position = name
                        .resolved_symbol()
                        .and_then(|symbol| {
                            formals
                                .iter()
                                .filter(|formal| !formal.relevance.is_erased())
                                .position(|formal| formal.symbol == symbol)
                        })
                        .and_then(|position| u32::try_from(position).ok())
                        .ok_or("routed membership does not select an exact formal")?;
                    reversed.reverse();
                    return Ok((ServiceMembershipSubject::Input(position), reversed));
                }
                ExpressionNode::Member(member) => {
                    let symbol = member.selection.field_symbol();
                    let field = program
                        .unique_declared_field_by_symbol(symbol)
                        .ok_or("method membership field has no unique declaration identity")?;
                    if !field.member_identity().is_valid() {
                        return Err("method membership field has no declared member identity");
                    }
                    reversed.push(ServiceMembershipProjection::Field(field.path_identity()));
                    let parent = program.symbols.get(symbol).parent;
                    if !program.symbols.symbols().nodes().is_valid(parent) {
                        return Err("method membership field lost its declaration owner");
                    }
                    if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
                        let (_, case) = program
                            .variant_owner(parent)
                            .ok_or("method membership field lost its exact case")?;
                        reversed.push(ServiceMembershipProjection::Case(case.path_identity()));
                    }
                    expression = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    let index = program
                        .closed_integer_expression_value_in(indexed.index, owner)
                        .and_then(|index| index.to_u64())
                        .ok_or("method membership index is not an exact closed element")?;
                    reversed.push(ServiceMembershipProjection::FixedIndex(index));
                    expression = indexed.collection;
                }
                _ => {
                    return Err(
                        "method membership must name a formal, the normal result, or a projection of either",
                    );
                }
            }
        }
    }
}

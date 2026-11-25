//! Resolve competing names against the domain applications on a written carrier.

use super::domain_constraints::select_domain_candidates_where;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_state::domain::{
    DomainDefinition, domain_accepts_underlying_carrier, expanded_alias_members,
    has_generic_carrier, index_parameters, indexed_domain_instance_name_with_substitutions,
    refinement_parent,
};
use crate::source_state::type_identity::IndexIdentityArgument;
use crate::source_state::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use crate::source_state::{CallableApplication, TypedTrees};
use numerics::arithmetic::ArithmeticDomain;
use source_text::SourceSpan;
use symbols::SymbolHandle;

pub(crate) struct DomainUse<'a> {
    pub(crate) name: &'a str,
    pub(crate) arguments: &'a [TypeReferenceHandle],
    pub(crate) reference: Option<SourceSpan>,
    pub(crate) retained: SymbolHandle,
}

impl<'a> From<&'a DomainConstraint> for DomainUse<'a> {
    fn from(domain: &'a DomainConstraint) -> Self {
        Self {
            name: domain.name.as_str(),
            arguments: &domain.arguments,
            reference: domain
                .source_selection
                .map(|selection| selection.source_span)
                .filter(|span| span.span.start != span.span.end),
            retained: domain.symbol,
        }
    }
}

type Application = (SymbolHandle, Vec<TypeReferenceHandle>);

impl DomainUse<'_> {
    fn candidates(
        &self,
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        carrier: TypeReferenceHandle,
    ) -> Vec<DomainDefinition> {
        if self.reference.is_none()
            && let Some(retained) = program.domain_definition(self.retained)
        {
            return vec![retained.clone()];
        }
        select_domain_candidates_where(
            source,
            program,
            self.name,
            self.reference,
            self.retained,
            |domain| domain_accepts_underlying_carrier(program, domain, carrier, self.arguments),
        )
    }

    /// A membership expression asks about the actual value. Only its existing
    /// qualifications can satisfy the selected declaration's carrier.
    pub(crate) fn on_operand(
        &self,
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        carrier: TypeReferenceHandle,
    ) -> Vec<DomainDefinition> {
        let plain = program
            .type_reference_table
            .unconstrained_type_reference(carrier);
        let mut candidates = self.candidates(source, program, plain);
        if self.reference.is_none() && self.retained.is_valid() {
            candidates.retain(|domain| {
                domain_accepts_underlying_carrier(program, domain, plain, self.arguments)
            });
        }
        if candidates.iter().any(|domain| {
            domain.target_type.is_valid()
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(domain.target_type),
                    TypeReferenceNode::Constrained { .. }
                )
        }) {
            let available = CarrierQualifications::for_type(source, program, carrier, plain);
            available.retain_possible(source, program, &mut candidates, self.arguments);
        }
        candidates
    }

    fn members(
        &self,
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        domain: &DomainDefinition,
    ) -> Option<Vec<Application>> {
        if domain.alias.is_none() {
            return Some(vec![(domain.symbol, self.arguments.to_vec())]);
        }
        if let Ok(members) = expanded_alias_members(program, domain.symbol) {
            return members
                .into_iter()
                .map(|(member, identity)| {
                    let complete =
                        program.domain_arguments_for_instance(member.symbol, identity)?;
                    let first_index = usize::from(has_generic_carrier(program, member));
                    Some((member.symbol, complete.get(first_index..)?.to_vec()))
                })
                .collect();
        }
        // Written qualifications can be visited before a fixed alias has retained
        // its implicit carrier. Its resolved source edges already identify leaves
        // with no explicit indices on this carrier.
        super::domain_aliases::expand_domain_reference(source, domain.symbol, Vec::new())
            .ok()?
            .into_iter()
            .map(|leaf| {
                let member = program.domain_definition(leaf.symbol)?;
                index_parameters(program, member)
                    .is_empty()
                    .then_some((member.symbol, Vec::new()))
            })
            .collect()
    }
}

/// Applications that the surrounding entries could supply. Selection removes
/// a candidate only when its declared carrier requires an absent application.
/// Formation and evidence for the selected application are checked downstream.
#[derive(Default)]
pub(crate) struct CarrierQualifications {
    domains: Vec<Application>,
    policies: Vec<ArithmeticDomain>,
}

impl CarrierQualifications {
    pub(crate) fn for_type(
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        mut qualified: TypeReferenceHandle,
        carrier: TypeReferenceHandle,
    ) -> Self {
        let mut available = Self::default();
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = *program.type_reference_table.type_reference(qualified)
        {
            for node in program
                .type_reference_table
                .constraints(constraints)
                .to_vec()
            {
                match node {
                    TypeConstraintNode::Domain(domain) => {
                        available.include(source, program, carrier, DomainUse::from(&domain))
                    }
                    TypeConstraintNode::ArithmeticDomain(policy) => {
                        available.include_policy(policy)
                    }
                    TypeConstraintNode::Named(_) => {}
                }
            }
            qualified = base_type;
        }
        available
    }

    pub(crate) fn include_policy(&mut self, policy: ArithmeticDomain) {
        self.policies.push(policy);
    }

    pub(crate) fn include(
        &mut self,
        source: &SymbolResolvedTrees,
        program: &mut TypedTrees,
        carrier: TypeReferenceHandle,
        written: DomainUse<'_>,
    ) {
        for domain in written.candidates(source, program, carrier) {
            let Some(members) = written.members(source, program, &domain) else {
                continue;
            };
            for (mut symbol, mut indices) in members {
                while let Some(declaration) = program.domain_definition(symbol) {
                    self.domains.push((symbol, indices.clone()));
                    let Some(parent) = refinement_parent(program, declaration) else {
                        break;
                    };
                    let parent_symbol = parent.domain_symbol;
                    let mut arguments = program
                        .type_reference_table
                        .type_reference_handles(parent.domain_arguments)
                        .to_vec();
                    if has_generic_carrier(program, declaration) {
                        indices.insert(0, carrier);
                    }
                    if !substitute_indices(program, symbol, &indices, &mut arguments) {
                        break;
                    }
                    symbol = parent_symbol;
                    indices = arguments;
                }
            }
        }
    }

    pub(crate) fn retain_possible(
        &self,
        source: &SymbolResolvedTrees,
        program: &TypedTrees,
        candidates: &mut Vec<DomainDefinition>,
        indices: &[TypeReferenceHandle],
    ) {
        candidates.retain(|candidate| {
            let substitutions = index_parameters(program, candidate)
                .iter()
                .zip(indices)
                .map(|(binder, index)| (binder.symbol, IndexIdentityArgument::TypeArgument(*index)))
                .collect::<Vec<_>>();
            let plain = program
                .type_reference_table
                .unconstrained_type_reference(candidate.target_type);
            let mut target = candidate.target_type;
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(target)
            {
                for node in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::ArithmeticDomain(policy) = node
                        && !self.policies.contains(policy)
                    {
                        return false;
                    }
                    if let TypeConstraintNode::Domain(domain) = node {
                        let required = DomainUse::from(domain);
                        let declarations = required.candidates(source, program, plain);
                        if !declarations.is_empty()
                            && !declarations.iter().any(|domain| {
                                required
                                    .members(source, program, domain)
                                    .is_none_or(|members| {
                                        members.iter().all(|(symbol, arguments)| {
                                            self.contains(
                                                program,
                                                *symbol,
                                                arguments,
                                                &substitutions,
                                            )
                                        })
                                    })
                            })
                        {
                            return false;
                        }
                    }
                }
                target = *base_type;
            }
            true
        });
    }

    fn contains(
        &self,
        program: &TypedTrees,
        symbol: SymbolHandle,
        arguments: &[TypeReferenceHandle],
        substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    ) -> bool {
        let Some(declaration) = program.domain_definition(symbol) else {
            return false;
        };
        let parameters = index_parameters(program, declaration);
        let expected = indexed_domain_instance_name_with_substitutions(
            program,
            declaration,
            parameters,
            arguments,
            substitutions,
        );
        self.domains.iter().any(|(supplied, indices)| {
            if *supplied != symbol {
                return false;
            }
            let supplied = indexed_domain_instance_name_with_substitutions(
                program,
                declaration,
                parameters,
                indices,
                &[],
            );
            match (&expected, supplied) {
                (Ok(expected), Ok(supplied)) => expected == &supplied,
                _ => true,
            }
        })
    }
}

/// The parent names its indices in the child's telescope. Project those
/// operands at the child's application before looking up the parent.
pub(crate) fn substitute_indices(
    program: &mut TypedTrees,
    child: SymbolHandle,
    bindings: &[TypeReferenceHandle],
    arguments: &mut [TypeReferenceHandle],
) -> bool {
    let Some(declaration) = program.domain_definition(child) else {
        return false;
    };
    let parameters = program.domain_type_parameters(declaration);
    if parameters.len() != bindings.len() {
        return false;
    }
    let mut application = CallableApplication::for_type_parameters(
        child,
        parameters.iter().zip(bindings.iter().copied()),
        None,
    );
    for argument in arguments {
        let Ok(applied) = application.type_reference(program, *argument) else {
            return false;
        };
        *argument = applied;
    }
    true
}

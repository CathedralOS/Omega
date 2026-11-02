//! Callable proof context and live non-contract uses are distinct occurrences.
//! Applied signatures own copied facts but do not become callable theorem
//! sources. The shared typed-reference traversal supplies their actual scopes,
//! including nested machine binders and selected required/provider contracts.

use super::{ContentConservationSourcePlan, ContentProjectionFormation, collect_callable_plans};
use arena::{Arena, Handle};
use diagnostics::Diagnostic;
use language_semantics::content::{ContentConservationOwnerKind, ContentProjectionPlan};
use std::collections::HashSet;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::{self, TypedReferences};
use typed_trees::signature::StateSignature;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn non_contract_expression_nodes(
    program: &dyn crate::CheckingInput,
    projections: &[ContentProjectionPlan],
    proof_nodes: &mut HashSet<(u32, u32)>,
    diagnostics: &mut Vec<Diagnostic>,
    checking_plans: &mut Vec<ContentConservationSourcePlan>,
    formations: &mut Arena<ContentProjectionFormation>,
) -> HashSet<ExpressionHandle> {
    let mut contexts = ContractContexts {
        program,
        projections,
        proof_nodes,
        diagnostics,
        checking_plans,
        formations,
    };
    // Declaration roots include structural callable binders. Prepared views
    // also belong to checking even when no executable invocation selects them.
    references::visit_program(program.typed_program(), &mut contexts);
    for signature in program.prepared_callable_signatures() {
        references::visit_signature(program.typed_program(), &mut contexts, signature);
    }
    let mut uses = NonContractUses {
        program,
        proof_nodes,
        expressions: HashSet::new(),
        facts: HashSet::new(),
        types: HashSet::new(),
    };
    references::visit_program(program.typed_program(), &mut uses);
    uses.expressions
}

struct ContractContexts<'program, 'output> {
    program: &'program dyn crate::CheckingInput,
    projections: &'program [ContentProjectionPlan],
    proof_nodes: &'output mut HashSet<(u32, u32)>,
    diagnostics: &'output mut Vec<Diagnostic>,
    checking_plans: &'output mut Vec<ContentConservationSourcePlan>,
    formations: &'output mut Arena<ContentProjectionFormation>,
}

impl Extend<SymbolHandle> for ContractContexts<'_, '_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl TypedReferences for ContractContexts<'_, '_> {
    fn expression(&mut self, _: ExpressionHandle) -> bool {
        false
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        false
    }
    fn proof_fact(&mut self, _: Handle<ProofFact>) -> bool {
        false
    }

    fn callable_signature(&mut self, signature: &StateSignature) {
        let contracts = self
            .program
            .state_signature_contracts(signature)
            .iter()
            .collect::<Vec<_>>();
        let has_unvisited_fact = contracts.iter().any(|contract| {
            self.program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .any(|fact| {
                    let root = fact
                        .as_expression()
                        .copied()
                        .or_else(|| fact.as_membership().map(|membership| membership.value));
                    root.is_some_and(|root| {
                        !self
                            .proof_nodes
                            .contains(&(root.arena_index(), root.generation()))
                    })
                })
        });
        if !has_unvisited_fact {
            return;
        }
        // Validate formation in this signature's own substituted parameter and
        // result scope. Exact operand formation remains available to checking,
        // but these contexts are not published as callable proof sources.
        collect_callable_plans(
            self.program,
            self.projections,
            ContentConservationOwnerKind::Machine,
            signature.symbol,
            signature.symbol,
            signature.name.as_str(),
            self.program.state_signature_parameters(signature),
            signature.return_type,
            SymbolHandle::invalid(),
            &contracts,
            &Arc::from([]),
            self.proof_nodes,
            self.checking_plans,
            self.formations,
            self.diagnostics,
        );
    }
}

struct NonContractUses<'program> {
    program: &'program dyn crate::CheckingInput,
    proof_nodes: &'program HashSet<(u32, u32)>,
    expressions: HashSet<ExpressionHandle>,
    facts: HashSet<Handle<ProofFact>>,
    types: HashSet<TypeReferenceHandle>,
}

impl Extend<SymbolHandle> for NonContractUses<'_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl TypedReferences for NonContractUses<'_> {
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        self.expressions.insert(expression)
    }
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        self.types.insert(reference)
    }
    fn proof_fact(&mut self, handle: Handle<ProofFact>) -> bool {
        if !self.facts.insert(handle) {
            return false;
        }
        let fact = self.program.proof_facts.get(handle);
        let root = fact
            .as_expression()
            .copied()
            .or_else(|| fact.as_membership().map(|membership| membership.value));
        !root.is_some_and(|root| {
            self.proof_nodes
                .contains(&(root.arena_index(), root.generation()))
        })
    }
}

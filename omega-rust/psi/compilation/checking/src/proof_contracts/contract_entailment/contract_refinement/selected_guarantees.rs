//! Contract correspondence through selected public result equations. Calls
//! retain their original source telescope, while the two compared signatures
//! bind corresponding actuals by position. Neither bodies nor goal facts are
//! inputs to normalization.

use super::super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::{Conjunct, ContractFacts};
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::signature::SignatureContractKind;
use typed_trees::signature::StateParameter;

pub(super) fn entailed(
    program: &dyn crate::CheckingInput,
    premises: &[Conjunct],
    premise_scope: &ContractFacts,
    goal: &Conjunct,
    goal_scope: &ContractFacts,
) -> bool {
    let Some(first) = premises.first() else {
        return false;
    };
    let form = |scope: &ContractFacts, fact: &Conjunct| {
        crate::proof_contracts::contract_results::fact_has_source_builtin_index_meaning(
            program, &fact.fact,
        )
        .then(|| {
            contract_term(
                program,
                scope.owner,
                &scope.parameters,
                &scope.binders,
                &fact.fact,
            )
        })
        .flatten()
    };
    let normalizer = |scope: &ContractFacts, kind: &SignatureContractKind| {
        // Only an Ensures term may consume the declaration's original entry
        // premises. A Requires term never assumes itself to form an operand.
        let requires = if kind == &SignatureContractKind::Ensures {
            scope
                .requires
                .iter()
                .filter(|fact| stable_entry_premise(program, scope, fact))
                .filter_map(|fact| form(scope, fact))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        PublishedContractNormalizer::new(program, scope.implementation, &requires)
    };
    let Some(goal) =
        form(goal_scope, goal).and_then(|term| normalizer(goal_scope, &goal.kind).normalize(term))
    else {
        return false;
    };
    let normalizer = normalizer(premise_scope, &first.kind);
    premises
        .iter()
        .filter_map(|fact| form(premise_scope, fact))
        .filter_map(|term| normalizer.normalize(term))
        .any(|premise| premise == goal)
}

/// Entry scalar values are immutable snapshots. Entry observations through a
/// reference or aggregate home need a versioned preservation judgment before
/// they can discharge a post-state call's premises; positional correspondence
/// alone does not supply one.
fn stable_entry_premise(
    program: &dyn crate::CheckingInput,
    scope: &ContractFacts,
    fact: &Conjunct,
) -> bool {
    let ProofFact::Expression(expression) = fact.fact else {
        return false;
    };
    let mut nodes = Vec::new();
    crate::collect_expression_tree(program, expression, &mut nodes);
    nodes.into_iter().all(|expression| {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            return true;
        };
        [path.symbol, path.head_symbol].into_iter().all(|symbol| {
            if !symbol.is_valid() {
                return true;
            }
            if let Some(parameter) = scope
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return !parameter.is_mutable
                    && crate::reference_shell(program, parameter.type_reference).is_none()
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some();
            }
            if matches!(
                program.symbols.get(symbol).kind,
                symbols::SymbolKind::Unknown
                    | symbols::SymbolKind::Field
                    | symbols::SymbolKind::Local
                    | symbols::SymbolKind::Parameter
                    | symbols::SymbolKind::HostCapability
                    | symbols::SymbolKind::Object
                    | symbols::SymbolKind::Function
                    | symbols::SymbolKind::Section
                    | symbols::SymbolKind::Import
            ) {
                return false;
            }
            program
                .data_type_parameter_by_symbol(symbol)
                .is_none_or(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                            | typed_trees::data::TypeParameterKind::Type
                    )
                })
        })
    })
}

fn contract_term(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    binders: &[(SymbolHandle, String)],
    fact: &ProofFact,
) -> Option<StructuralTerm> {
    let ProofFact::Expression(expression) = fact else {
        return None;
    };
    let bindings = binders
        .iter()
        .map(|(symbol, identity)| {
            (
                *symbol,
                StructuralTerm::Variable(format!("\0contract:{identity}")),
            )
        })
        .collect::<Vec<_>>();
    StructuralJudge::from_declaration_requires(program, owner, parameters, &[], false)
        .bound_contract_term(
            *expression,
            owner,
            parameters,
            owner,
            &bindings,
            &[],
            Some(&StructuralTerm::Variable("\0contract-result".to_owned())),
            0,
        )
}

struct PublishedContractNormalizer<'program> {
    judge: StructuralJudge<'program>,
}

impl<'program> PublishedContractNormalizer<'program> {
    fn new(
        program: &'program dyn crate::CheckingInput,
        implementation: SymbolHandle,
        premises: &[StructuralTerm],
    ) -> Self {
        // Keep body/application unfolding disabled. The separate public
        // equation operation still checks the selected callee's independence,
        // value eligibility, exact carrier and all substituted entry premises.
        let mut judge =
            StructuralJudge::from_declaration_requires(program, implementation, &[], &[], false);
        if judge.published_contract_tracing() {
            eprintln!(
                "PUBLISHED caller premises implementation={implementation:?} premises={premises:?}"
            );
        }
        for premise in premises {
            judge.intake_equation(
                premise.clone(),
                StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: "true".to_owned(),
                    fields: Vec::new(),
                },
                0,
            );
        }
        if judge.published_contract_tracing() {
            judge.trace_published_contract_context();
            for premise in premises {
                let resolved = judge.resolve(premise.clone());
                eprintln!("PUBLISHED installed premise={premise:?} resolved={resolved:?}");
                if let StructuralTerm::ScalarBinary {
                    references,
                    left,
                    right,
                    ..
                } = premise
                {
                    let types = references.map(|reference| {
                        if program
                            .type_reference_table
                            .contains_type_reference(reference)
                        {
                            program.display_type_reference(reference)
                        } else {
                            "<missing>".to_owned()
                        }
                    });
                    let observed = [left.as_ref(), right.as_ref()]
                        .into_iter()
                        .zip(references)
                        .map(|(value, reference)| {
                            program
                                .type_reference_table
                                .contains_type_reference(*reference)
                                && super::super::structural_terms::scalar_operand_is_observed(
                                    program, *reference, value,
                                )
                        })
                        .collect::<Vec<_>>();
                    eprintln!("PUBLISHED premise carriers={types:?} observed={observed:?}");
                }
            }
        }
        Self { judge }
    }

    fn normalize(&self, value: StructuralTerm) -> Option<StructuralTerm> {
        self.normalize_at(value, &mut Vec::new(), 0)
    }

    fn normalize_at(
        &self,
        value: StructuralTerm,
        expanding: &mut Vec<StructuralTerm>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 || expanding.contains(&value) {
            return None;
        }
        // Opaque diagnostic terms cannot become correspondence identities.
        value.observation_identity_key()?;
        expanding.push(value.clone());
        let mut complete = true;
        let value = value.map_children(|child| {
            match self.normalize_at(child.clone(), expanding, depth + 1) {
                Some(normalized) => normalized,
                None => {
                    complete = false;
                    child
                }
            }
        });
        let normalized = if !complete {
            None
        } else if let Some(replacement) = self.judge.published_contract_normal_form(&value, depth) {
            self.normalize_at(replacement, expanding, depth + 1)
        } else {
            Some(value)
        };
        expanding.pop();
        normalized
    }
}

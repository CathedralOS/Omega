//! Exact precondition partitioning for quotient operations and correspondence
//! for faithful quotient definitions.
//!
//! This is a deliberately narrow structural judgment. It separates public
//! quotient-dependent facts (`Q`) from representative-dependent facts (`P`)
//! and proves only an exact position-renamed bijection for `define`. The
//! bounded direct-lift inclusion judgment consumes the same partitions from
//! its certificate owner. General entailment remains a separate obligation.

use super::proof_fact_identity::{ProofFactIdentityContext, proof_facts_match};
use super::{
    DefineRuntimeCorrespondence, DefineRuntimePosition, InputRelation, RelationPlanError,
    RepresentativeStaticBinding, RepresentativeTelescope,
};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind};
use psi_typed_trees::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepresentativeContractOwner {
    Machine,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeContractFactLocation {
    pub(super) owner: RepresentativeContractOwner,
    pub(super) contract_position: usize,
    pub(super) fact_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct RepresentativePreconditionPartition {
    /// Exact `requires` facts whose expression depends on at least one
    /// quotient-bearing representative position. This is the future `P`
    /// surface; retaining it proves no implication or invariance law.
    pub(super) dependent: Vec<RepresentativeContractFactLocation>,
    /// Exact `requires` facts independent of quotient-bearing positions.
    pub(super) fixed: Vec<RepresentativeContractFactLocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DefinePreconditionFactPair {
    pub(super) public: RepresentativeContractFactLocation,
    pub(super) representative: RepresentativeContractFactLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct DefinePreconditionCorrespondence {
    pub(super) dependent: Vec<DefinePreconditionFactPair>,
}

pub(super) fn derive_public_precondition_partition(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    input_relations: &[InputRelation],
    runtime_positions: &[DefineRuntimePosition],
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let public_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .collect::<Vec<_>>();
    if public_parameters.len() != input_relations.len()
        || runtime_positions.len() != input_relations.len()
    {
        return Err(RelationPlanError::DefineRuntimeArityMismatch);
    }
    let varying_parameters = input_relations
        .iter()
        .zip(runtime_positions)
        .filter_map(|(relation, position)| {
            matches!(relation, InputRelation::Quotient(_)).then_some(position.public_parameter)
        })
        .collect::<Vec<_>>();
    derive_precondition_partition(
        program,
        machine.contracts,
        state.contracts,
        &varying_parameters,
    )
}

pub(super) fn derive_representative_precondition_partition(
    program: &TypedTrees,
    input_relations: &[InputRelation],
    representative: &RepresentativeTelescope,
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let varying_parameters = input_relations
        .iter()
        .zip(&representative.parameters)
        .filter_map(|(relation, parameter)| {
            matches!(relation, InputRelation::Quotient(_)).then_some(parameter.symbol)
        })
        .collect::<Vec<_>>();
    derive_precondition_partition(
        program,
        representative.machine_contracts,
        representative.state_contracts,
        &varying_parameters,
    )
}

fn derive_precondition_partition(
    program: &TypedTrees,
    machine_contracts: HandleSpan<SignatureContract>,
    state_contracts: HandleSpan<SignatureContract>,
    varying_parameters: &[SymbolHandle],
) -> Result<RepresentativePreconditionPartition, RelationPlanError> {
    let mut partition = RepresentativePreconditionPartition {
        dependent: Vec::new(),
        fixed: Vec::new(),
    };
    for (owner, contracts) in [
        (
            RepresentativeContractOwner::Machine,
            program.signature_contracts.span_or_empty(machine_contracts),
        ),
        (
            RepresentativeContractOwner::State,
            program.signature_contracts.span_or_empty(state_contracts),
        ),
    ] {
        for (contract_position, contract) in contracts.iter().enumerate() {
            if contract.kind != SignatureContractKind::Requires {
                continue;
            }
            for (fact_position, fact) in program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .enumerate()
            {
                let location = RepresentativeContractFactLocation {
                    owner,
                    contract_position,
                    fact_position,
                };
                if proof_fact_depends_on_any(program, fact, varying_parameters)? {
                    partition.dependent.push(location);
                } else {
                    partition.fixed.push(location);
                }
            }
        }
    }
    Ok(partition)
}

pub(super) fn derive_define_precondition_correspondence(
    program: &TypedTrees,
    public_machine: &Machine,
    public_state: &State,
    representative: &RepresentativeTelescope,
    public: &RepresentativePreconditionPartition,
    representative_partition: &RepresentativePreconditionPartition,
    runtime: &DefineRuntimeCorrespondence,
) -> Result<DefinePreconditionCorrespondence, RelationPlanError> {
    let public_substitutions = runtime
        .positions
        .iter()
        .enumerate()
        .map(|(position, binding)| (binding.public_parameter, format!("${position}")))
        .collect::<Vec<_>>();
    let representative_substitutions = runtime
        .positions
        .iter()
        .enumerate()
        .map(|(position, binding)| (binding.representative_parameter, format!("${position}")))
        .collect::<Vec<_>>();
    let dependent = pair_precondition_facts(
        program,
        public_machine.contracts,
        public_state.contracts,
        representative.machine_contracts,
        representative.state_contracts,
        &public.dependent,
        &representative_partition.dependent,
        &public_substitutions,
        &representative_substitutions,
        &representative.static_application.bindings,
    )?;
    Ok(DefinePreconditionCorrespondence { dependent })
}

#[allow(clippy::too_many_arguments)]
fn pair_precondition_facts(
    program: &TypedTrees,
    public_machine_contracts: HandleSpan<SignatureContract>,
    public_state_contracts: HandleSpan<SignatureContract>,
    representative_machine_contracts: HandleSpan<SignatureContract>,
    representative_state_contracts: HandleSpan<SignatureContract>,
    public_locations: &[RepresentativeContractFactLocation],
    representative_locations: &[RepresentativeContractFactLocation],
    public_substitutions: &[(SymbolHandle, String)],
    representative_substitutions: &[(SymbolHandle, String)],
    representative_static: &[RepresentativeStaticBinding],
) -> Result<Vec<DefinePreconditionFactPair>, RelationPlanError> {
    if public_locations.len() != representative_locations.len() {
        return Err(RelationPlanError::DefinePreconditionMismatch);
    }
    let mut unmatched = representative_locations.to_vec();
    let mut pairs = Vec::with_capacity(public_locations.len());
    for public_location in public_locations {
        let public_fact = precondition_fact_at(
            program,
            public_machine_contracts,
            public_state_contracts,
            *public_location,
        )
        .ok_or(RelationPlanError::DefinePreconditionMismatch)?;
        let Some((position, representative_location)) =
            unmatched.iter().copied().enumerate().find(|(_, location)| {
                precondition_fact_at(
                    program,
                    representative_machine_contracts,
                    representative_state_contracts,
                    *location,
                )
                .is_some_and(|representative_fact| {
                    proof_facts_match(
                        program,
                        public_fact,
                        representative_fact,
                        ProofFactIdentityContext {
                            values: public_substitutions,
                            static_bindings: &[],
                        },
                        ProofFactIdentityContext {
                            values: representative_substitutions,
                            static_bindings: representative_static,
                        },
                    )
                })
            })
        else {
            return Err(RelationPlanError::DefinePreconditionMismatch);
        };
        unmatched.remove(position);
        pairs.push(DefinePreconditionFactPair {
            public: *public_location,
            representative: representative_location,
        });
    }
    Ok(pairs)
}

pub(super) fn precondition_fact_at(
    program: &TypedTrees,
    machine_contracts: HandleSpan<SignatureContract>,
    state_contracts: HandleSpan<SignatureContract>,
    location: RepresentativeContractFactLocation,
) -> Option<&ProofFact> {
    let contracts = match location.owner {
        RepresentativeContractOwner::Machine => {
            program.signature_contracts.span_or_empty(machine_contracts)
        }
        RepresentativeContractOwner::State => {
            program.signature_contracts.span_or_empty(state_contracts)
        }
    };
    let contract = contracts.get(location.contract_position)?;
    if contract.kind != SignatureContractKind::Requires {
        return None;
    }
    program
        .proof_facts
        .span_or_empty(contract.facts)
        .get(location.fact_position)
}

fn proof_fact_depends_on_any(
    program: &TypedTrees,
    fact: &ProofFact,
    parameters: &[SymbolHandle],
) -> Result<bool, RelationPlanError> {
    match fact {
        ProofFact::Expression(expression) => {
            expression_depends_on_any(program, *expression, parameters)
        }
        ProofFact::Membership(membership) => {
            expression_depends_on_any(program, membership.value, parameters)
        }
        ProofFact::Proposition(application) => program
            .expression_table
            .expression_handles(application.arguments)
            .iter()
            .try_fold(false, |depends, expression| {
                let expression_depends =
                    expression_depends_on_any(program, *expression, parameters)?;
                Ok(depends || expression_depends)
            }),
    }
}

fn expression_depends_on_any(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[SymbolHandle],
) -> Result<bool, RelationPlanError> {
    let depends = |expression| expression_depends_on_any(program, expression, parameters);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            let value = depends(atomic.value)?;
            let result = if atomic.result.is_valid() {
                depends(atomic.result)?
            } else {
                false
            };
            Ok(value || result)
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .try_fold(false, |found, expression| {
                let expression_depends = depends(*expression)?;
                Ok(found || expression_depends)
            }),
        ExpressionNode::Binary(binary) => {
            let left = depends(binary.left)?;
            let right = depends(binary.right)?;
            Ok(left || right)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => Ok(false),
        ExpressionNode::Cast(cast) => depends(cast.value),
        ExpressionNode::Call(call) => {
            let receiver_depends = if call.receiver.is_valid() {
                depends(call.receiver)?
            } else {
                false
            };
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .try_fold(receiver_depends, |found, expression| {
                    let expression_depends = depends(*expression)?;
                    Ok(found || expression_depends)
                })
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = depends(indexed.collection)?;
            let index = depends(indexed.index)?;
            Ok(collection || index)
        }
        ExpressionNode::Member(member) => depends(member.receiver),
        ExpressionNode::Borrow(inner) => depends(inner.target),
        ExpressionNode::Unary(unary) => depends(unary.operand),
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() && !path.head_symbol.is_valid() {
                return Err(RelationPlanError::PreconditionDependencyUnresolved);
            }
            Ok(parameters.contains(&path.symbol) || parameters.contains(&path.head_symbol))
        }
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                depends(range.start)?
            } else {
                false
            };
            let end = if range.end.is_valid() {
                depends(range.end)?
            } else {
                false
            };
            Ok(start || end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .try_fold(false, |found, field| {
                let field_depends = depends(field.value)?;
                Ok(found || field_depends)
            }),
    }
}

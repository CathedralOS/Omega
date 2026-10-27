//! Structural ranking for a complete proof-call component.
//!
//! Authored ranks constrain the component. Private helpers may infer a rank
//! parameter from their exact incoming and outgoing arguments; lexical nesting
//! grants no premise. Every retained edge is a structural projection of that
//! parameter, and removing strict edges must leave an acyclic graph.

use super::{
    ExactProofCallEdge, ValidatedProofRankingRelation, ValidatedProofRecursiveComponent,
    ValidatedProofRecursiveEdge, ValidatedProofRecursiveMember, expression_member_path,
    proof_rank_parameter, strongly_connected_components,
};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(Clone)]
struct RankParameter {
    position: usize,
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
}

struct ComponentEdge<'a> {
    source: usize,
    target: usize,
    call: &'a ExactProofCallEdge,
}

pub(super) fn validate_component(
    program: &dyn crate::CheckingInput,
    members: &[usize],
    calls: &[ExactProofCallEdge],
) -> Option<ValidatedProofRecursiveComponent> {
    let machines = program.machines();
    let rank_type_identity = members
        .iter()
        .find_map(|member| proof_rank_parameter(program, &machines[*member]))?
        .2;
    let mut candidates = Vec::with_capacity(members.len());
    for member in members {
        let machine = &machines[*member];
        let authored = proof_rank_parameter(program, machine);
        // An unsupported authored witness is not permission to invent another.
        if authored.is_none()
            && (machine.termination_plan.implementation_witness.is_some() || machine.is_public)
        {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        let choices = program
            .state_parameters(entry)
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                parameter.symbol.is_valid()
                    && !parameter.is_mutable
                    && authored
                        .as_ref()
                        .is_none_or(|rank| rank.0 == parameter.symbol)
                    && program.package_qualified_type_identity(parameter.type_reference)
                        == rank_type_identity
            })
            .map(|(position, parameter)| RankParameter {
                position,
                symbol: parameter.symbol,
                type_reference: parameter.type_reference,
            })
            .collect::<Vec<_>>();
        if choices.is_empty() {
            return None;
        }
        candidates.push(choices);
    }
    let edges = calls
        .iter()
        .map(|call| {
            let callee_entry = program.machine_states(&machines[call.callee]).first()?;
            if call.arguments.len() != program.state_parameters(callee_entry).len() {
                return None;
            }
            Some(ComponentEdge {
                source: members.iter().position(|member| *member == call.caller)?,
                target: members.iter().position(|member| *member == call.callee)?,
                call,
            })
        })
        .collect::<Option<Vec<_>>>()?;

    // Arc consistency propagates declared ranks through ordinary calls. Each
    // successful iteration removes a candidate; there is no path unrolling.
    loop {
        let mut changed = false;
        for edge in &edges {
            let source = candidates[edge.source].clone();
            let target = candidates[edge.target].clone();
            candidates[edge.source].retain(|rank| {
                target
                    .iter()
                    .any(|next| member_path(program, edge.call, rank, next).is_some())
            });
            candidates[edge.target].retain(|rank| {
                source
                    .iter()
                    .any(|previous| member_path(program, edge.call, previous, rank).is_some())
            });
            if candidates[edge.source].is_empty() || candidates[edge.target].is_empty() {
                return None;
            }
            changed |= source.len() != candidates[edge.source].len()
                || target.len() != candidates[edge.target].len();
        }
        if !changed {
            break;
        }
    }
    // Ambiguous private inference remains incomplete, not an assumed ranking.
    // An authored private witness can select one without changing its contract.
    if candidates.iter().any(|choices| choices.len() != 1) {
        return None;
    }
    let mut preserving = vec![Vec::new(); members.len()];
    let mut witnesses = Vec::with_capacity(edges.len());
    for edge in edges {
        let source = &candidates[edge.source][0];
        let target = &candidates[edge.target][0];
        let path = member_path(program, edge.call, source, target)?;
        if path.is_empty() {
            preserving[edge.source].push(edge.target);
        }
        witnesses.push(ValidatedProofRecursiveEdge {
            caller: machines[edge.call.caller].symbol,
            callee: machines[edge.call.callee].symbol,
            site: edge.call.site,
            caller_rank_parameter: source.symbol,
            callee_rank_parameter: target.symbol,
            strict_member_path: path,
        });
    }
    if strongly_connected_components(&preserving)
        .iter()
        .any(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|member| preserving[*member].contains(member))
        })
    {
        return None;
    }
    let ranking_relation = if preserving.iter().all(Vec::is_empty) {
        ValidatedProofRankingRelation::StructuralSubterm
    } else {
        ValidatedProofRankingRelation::StructuralSubtermCycles
    };
    Some(ValidatedProofRecursiveComponent {
        members: members
            .iter()
            .enumerate()
            .map(|(position, member)| ValidatedProofRecursiveMember {
                machine: machines[*member].symbol,
                rank_parameter: candidates[position][0].symbol,
            })
            .collect(),
        ranking_relation,
        rank_type_identity,
        edges: witnesses,
    })
}

fn member_path(
    program: &dyn crate::CheckingInput,
    edge: &ExactProofCallEdge,
    source: &RankParameter,
    target: &RankParameter,
) -> Option<Vec<SymbolHandle>> {
    let (path, resulting_type) = expression_member_path(
        program,
        *edge.arguments.get(target.position)?,
        source.symbol,
        source.type_reference,
    )?;
    (program.package_qualified_type_identity(resulting_type)
        == program.package_qualified_type_identity(target.type_reference))
    .then_some(path)
}

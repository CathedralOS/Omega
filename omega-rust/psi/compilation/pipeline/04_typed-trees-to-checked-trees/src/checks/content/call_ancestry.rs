//! Discharge conditional call ancestry after all checked bodies have completed.
//!
//! A numeric callee guarantee cannot certify backing identity: a callee could
//! return a freshly supplied root with the same geometry. Used replacement
//! edges must rejoin the exact normal-result receipts and the callee's actual
//! transformations, ending at the formal whose caller-owned claim was moved.
//! Unused candidate equations grant nothing and need no ancestry judgment.

use checked_trees::facts::{
    ContentBodyClaimBinding, ContentClaimTransformation, ContentClaimTransformationSource,
};
use checked_trees::{CheckFacts, FlowClaimJoinAlternativeSource, FlowClaimOutcomeSource};
use language_semantics::{PermissionClaimIdentity, PermissionEventKind, PermissionEventSource};

pub(super) fn check_used_calls(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let content = &facts.qualifications.content;
    let mut checked = Vec::new();
    let mut traversal = OriginTraversal::default();
    for (_, derived) in content.body_derivations.iter() {
        let equation = content.body_conservations.get(derived.conservation);
        let completed = facts.flow.control.operands.get(equation.completed_value);
        let frontier = content.descendant_frontier(
            equation.inputs,
            equation.machine_symbol,
            equation.state_symbol,
            completed.statement_index,
            equation.evaluation,
            derived.conservation,
        );
        let result = frontier.and_then(|frontier| {
            for (handle, _, _) in frontier.replacements {
                if checked.contains(&handle) {
                    continue;
                }
                let transformation = content.claim_transformations.get(handle);
                if matches!(
                    transformation.source,
                    ContentClaimTransformationSource::Call { .. }
                ) {
                    authenticate_call(facts, transformation, &mut traversal)?;
                    checked.push(handle);
                }
            }
            Ok(())
        });
        if let Err(reason) = result {
            diagnostics.push(diagnostics::Diagnostic::error(format!(
                "cannot derive content custody in `{}`, state `{}` at statement {}: {reason}",
                crate::labels::symbol_name(program, equation.machine_symbol),
                crate::labels::symbol_name(program, equation.state_symbol),
                completed.statement_index,
            )));
        }
    }
}

type ActiveClaim = (
    PermissionClaimIdentity,
    symbols::SymbolHandle,
    language_semantics::SemanticDomainId,
    symbols::SymbolHandle,
);

/// Completed origins belong to this fixed checked graph. Share them across
/// reconvergent uses, while bounding a deeply nested or expanding source graph
/// before it can exhaust the host stack or receiving work.
#[derive(Default)]
struct OriginTraversal {
    active: Vec<ActiveClaim>,
    completed: Vec<(ActiveClaim, Vec<PermissionClaimIdentity>)>,
    visits: usize,
}

impl OriginTraversal {
    fn visit(&mut self) -> Result<(), &'static str> {
        if self.visits >= 65_536 {
            return Err("callee content lineage work budget exhausted");
        }
        self.visits += 1;
        Ok(())
    }
}

fn authenticate_call(
    facts: &CheckFacts,
    transformation: &ContentClaimTransformation,
    traversal: &mut OriginTraversal,
) -> Result<(), &'static str> {
    for output in transformation.outputs.handles() {
        call_parents(
            facts,
            transformation,
            facts.qualifications.content.body_claim_bindings.get(output),
            traversal,
        )?;
    }
    Ok(())
}

fn call_parents(
    facts: &CheckFacts,
    transformation: &ContentClaimTransformation,
    output: &ContentBodyClaimBinding,
    traversal: &mut OriginTraversal,
) -> Result<Vec<PermissionClaimIdentity>, &'static str> {
    let ContentClaimTransformationSource::Call { call_ordinal, .. } = transformation.source else {
        return Err("ancestry edge is not a completed call");
    };
    let ownership = &facts.flow.ownership;
    let mut receipts = ownership.claim_join_receipts.iter().filter(|(_, receipt)| {
        receipt.machine_symbol == transformation.machine_symbol
            && receipt.state_symbol == transformation.state_symbol
            && receipt.statement_index == transformation.statement_index
            && receipt.call_ordinal == call_ordinal
            && receipt.claim_identity == output.claim_identity
    });
    let (_, receipt) = receipts
        .next()
        .ok_or("conserved call result has no exact normal-result receipt")?;
    if receipts.next().is_some() {
        return Err("conserved call result has ambiguous normal-result receipts");
    }
    let mut parents: Option<Vec<PermissionClaimIdentity>> = None;
    for alternative in ownership
        .claim_join_alternatives
        .span_or_empty(receipt.alternatives)
    {
        traversal.visit()?;
        let source = match alternative.source {
            FlowClaimJoinAlternativeSource::Claim { source, .. } => source,
            FlowClaimJoinAlternativeSource::Inactive
            | FlowClaimJoinAlternativeSource::ExcludedInput { .. } => continue,
        };
        let formals = match source {
            FlowClaimOutcomeSource::Input {
                parameter_symbol,
                segments,
            } => vec![(parameter_symbol, segments)],
            FlowClaimOutcomeSource::Established { claim_identity, .. } => {
                let roots = qualified_roots(facts, claim_identity, output, traversal)?;
                let mut formals = Vec::new();
                for root in roots {
                    let mut entries = ownership.permissions.iter().filter(|(_, event)| {
                        event.claim_identity == root
                            && event.source == PermissionEventSource::StateEntry
                            && event.kind == PermissionEventKind::Establish
                    });
                    let (_, entry) = entries.next().ok_or(
                        "equal content does not relate a freshly established callee root to an input",
                    )?;
                    if entries.next().is_some() {
                        return Err("callee lineage has ambiguous entry custody");
                    }
                    let facts::PlaceRoot::Symbol(parameter) = entry.root else {
                        return Err("callee lineage entry is not a formal");
                    };
                    formals.push((
                        callable_entry_parameter(facts, entry, parameter)?,
                        entry.segments,
                    ));
                }
                formals
            }
            FlowClaimOutcomeSource::Unknown => return Err("callee result lineage is unknown"),
        };
        let mut candidate = Vec::new();
        for (parameter, path) in formals {
            let mut inputs = facts
                .qualifications
                .content
                .body_claim_bindings
                .span_or_empty(transformation.inputs)
                .iter()
                .filter(|input| {
                    input.parameter_symbol == parameter
                        && ownership.segments.span_or_empty(input.segments)
                            == ownership.segments.span_or_empty(path)
                        && same_projection(input, output)
                });
            let input = inputs
                .next()
                .ok_or("callee result lineage does not bind a conserved actual input")?;
            if inputs.next().is_some() {
                return Err("callee result lineage has ambiguous conserved actual inputs");
            }
            if !candidate.contains(&input.claim_identity) {
                candidate.push(input.claim_identity);
            }
        }
        if candidate.is_empty()
            || parents
                .as_ref()
                .is_some_and(|previous| !same_roots(previous, &candidate))
        {
            return Err("callee result alternatives have incompatible input rosters");
        }
        parents = Some(candidate);
    }
    parents.ok_or("conserved call result has no live normal-result alternative")
}

fn callable_entry_parameter(
    facts: &CheckFacts,
    entry: &checked_trees::FlowPermissionEventFact,
    parameter: symbols::SymbolHandle,
) -> Result<symbols::SymbolHandle, &'static str> {
    let flow = facts
        .flow
        .control
        .state(entry.machine_symbol, entry.state_symbol)
        .ok_or("callee lineage entry has no state flow")?;
    let origins = crate::flow::state_exit_parameter_origins(facts, flow);
    let mut matching = origins
        .iter()
        .filter(|origin| origin.state_parameter == parameter);
    let Some(first) = matching.next() else {
        // An entry state needs no rebasing. If this is an unrelated state
        // parameter, the exact actual-input lookup will refuse it below.
        return Ok(parameter);
    };
    if matching.any(|origin| origin.entry_parameter != first.entry_parameter)
        || origins.iter().any(|origin| {
            origin.entry_parameter == first.entry_parameter && origin.state_parameter != parameter
        })
    {
        return Err("callee lineage has ambiguous state-arrival origins");
    }
    Ok(first.entry_parameter)
}

fn qualified_roots(
    facts: &CheckFacts,
    claim: PermissionClaimIdentity,
    projection: &ContentBodyClaimBinding,
    traversal: &mut OriginTraversal,
) -> Result<Vec<PermissionClaimIdentity>, &'static str> {
    traversal.visit()?;
    let key = (
        claim,
        projection.domain,
        projection.semantic_domain,
        projection.projection_machine,
    );
    if let Some((_, roots)) = traversal
        .completed
        .iter()
        .find(|(completed, _)| *completed == key)
    {
        return Ok(roots.clone());
    }
    if traversal.active.contains(&key) {
        return Err("callee content lineage is circular");
    }
    if traversal.active.len() >= 128 {
        return Err("callee content lineage depth budget exhausted");
    }
    traversal.active.push(key);
    let content = &facts.qualifications.content;
    let result = (|| {
        let mut roots: Option<Vec<PermissionClaimIdentity>> = None;
        for (_, transformation) in
            content
                .claim_transformations
                .iter()
                .filter(|(_, transformation)| {
                    content
                        .body_claim_bindings
                        .span_or_empty(transformation.outputs)
                        .iter()
                        .any(|output| {
                            output.claim_identity == claim && same_projection(output, projection)
                        })
                })
        {
            traversal.visit()?;
            let parents = match transformation.source {
                ContentClaimTransformationSource::Call { .. } => call_parents(
                    facts,
                    transformation,
                    &ContentBodyClaimBinding {
                        claim_identity: claim,
                        ..projection.clone()
                    },
                    traversal,
                )?,
                ContentClaimTransformationSource::Body { .. } => content
                    .body_claim_bindings
                    .span_or_empty(transformation.inputs)
                    .iter()
                    .map(|input| input.claim_identity)
                    .collect(),
                ContentClaimTransformationSource::Unknown => {
                    return Err("callee lineage has an unknown producer");
                }
            };
            if parents.is_empty() {
                return Err("callee content transformation has no parent");
            }
            let mut candidate = Vec::new();
            for parent in parents {
                let ancestors = if parent == claim {
                    vec![parent]
                } else {
                    qualified_roots(facts, parent, projection, traversal)?
                };
                for ancestor in ancestors {
                    if !candidate.contains(&ancestor) {
                        candidate.push(ancestor);
                    }
                }
            }
            if roots
                .as_ref()
                .is_some_and(|previous| !same_roots(previous, &candidate))
            {
                return Err("callee content transformation has conflicting input rosters");
            }
            roots = Some(candidate);
        }
        Ok(roots.unwrap_or_else(|| vec![claim]))
    })();
    traversal.active.pop();
    if let Ok(roots) = &result {
        traversal.completed.push((key, roots.clone()));
    }
    result
}

fn same_roots(left: &[PermissionClaimIdentity], right: &[PermissionClaimIdentity]) -> bool {
    left.len() == right.len() && left.iter().all(|claim| right.contains(claim))
}

fn same_projection(left: &ContentBodyClaimBinding, right: &ContentBodyClaimBinding) -> bool {
    left.domain == right.domain
        && left.semantic_domain == right.semantic_domain
        && left.projection_machine == right.projection_machine
}

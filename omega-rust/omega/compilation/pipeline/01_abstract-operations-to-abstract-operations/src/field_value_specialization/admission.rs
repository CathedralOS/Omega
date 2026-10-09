//! Optimizer module role: admission leaf. Proven field-value predicates behind the plan.
//!
//! The plan is built from two predicates only: the place's
//! declaration/type/proof evidence and the per-node admissibility that
//! resolves one field observation's proven stored value — the shared node
//! gate lives in `optimization_unit` so the independent validator derives
//! the same roster rather than trusting this enumeration.

use super::{FieldValueRow, PsiOptimizationFunction, PsiOptimizationUnit, ScalarType};
use crate::representation_specialization::admission::bound_place;
use optimization_unit::{
    FieldEvidence, ForwardedFieldValue, FunctionAnalysis, NodeLocation, OptimizationBlock,
    OptimizationNode, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::ValueId;
use std::collections::BTreeSet;

/// The admissibility of one node under `evidence`: a Boolean or integer
/// `StructuralScalarFieldRead` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node.
pub(super) fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FieldValueRow> {
    optimization_unit::admit_field_node(
        unit,
        evidence,
        function,
        analysis,
        block,
        node_index,
        node,
        &bound_place,
        &forwarded_resolution,
    )
}

/// The `Forward` resolution for one nonconstant-initialized read, or `None`
/// when the substitution cannot retire the observation exactly: the
/// initializer must resolve to a same-function definition of the read's own
/// scalar type and dominate every use site, every scalar-operand use of the
/// read's result must sit in an operation the substitution lane rewrites,
/// and the observation node itself must be cleanly removable — exactly its
/// result definition, no successors, no ownership events, a following node
/// to absorb its custody, and no shared provenance with that receiver. Field
/// observations emit no optimization facts, so no fact custody is owed.
fn forwarded_resolution(
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    initializer: ValueId,
) -> Option<ForwardedFieldValue> {
    if initializer == result {
        return None;
    }
    let definition = analysis.definitions.get(&initializer)?;
    if definition.scalar_type != scalar_type {
        return None;
    }
    let node_index_u32 = u32::try_from(node_index).ok()?;
    if node.definitions
        != [ValueDefinition {
            value: result,
            scalar_type,
            site: ValueDefinitionSite::Node {
                block: block.id,
                node: node_index_u32,
            },
        }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return None;
    }
    let receiver = block.nodes.get(node_index.checked_add(1)?)?;
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return None;
    }
    // Every site that references the read's result: the tracked scalar-operand
    // uses, which include each runtime-selected path element's selector. A
    // use inside a byte-sequence operation, or as a runtime selector whose
    // accepted certificate names that exact value, is outside the lane, so
    // the row is inadmissible rather than partially substituted.
    let mut uses = BTreeSet::new();
    for use_block in &function.blocks {
        for (use_index, use_node) in use_block.nodes.iter().enumerate() {
            let referenced = use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == result);
            if !referenced {
                continue;
            }
            if use_node.operation.operates_on_byte_sequence_storage()
                || use_node
                    .operation
                    .runtime_indices()
                    .iter()
                    .any(|(index, _)| *index == result)
            {
                return None;
            }
            let site = NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(use_index).ok()?,
            };
            let dominates = match definition.site {
                ValueDefinitionSite::FunctionParameter(_) => true,
                ValueDefinitionSite::BlockParameter {
                    block: defining, ..
                } => analysis
                    .dominators
                    .get(&use_block.id)
                    .is_some_and(|set| set.contains(&defining)),
                ValueDefinitionSite::Node {
                    block: defining,
                    node: defined_at,
                } => {
                    if defining == use_block.id {
                        defined_at < site.node
                    } else {
                        analysis
                            .dominators
                            .get(&use_block.id)
                            .is_some_and(|set| set.contains(&defining))
                    }
                }
            };
            if !dominates {
                return None;
            }
            uses.insert(site);
        }
    }
    Some(ForwardedFieldValue {
        initializer,
        scalar_type,
        uses: uses.into_iter().collect(),
    })
}

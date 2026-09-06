//! Bind physical spans to semantic ordinals in the current abstract program.

use abstract_operations::{AbstractFunction, AbstractOperation};
use machine_code::{
    FunctionFragment, FunctionFragmentControlProvenance, SemanticCodeAttribution, SemanticCodeSite,
};

pub(super) fn project(
    fragment: &FunctionFragment,
    source: &AbstractFunction,
) -> Result<Vec<SemanticCodeAttribution>, &'static str> {
    let mut rows = Vec::new();
    for block in &fragment.blocks {
        for instruction in &block.instructions {
            let offset = usize::try_from(instruction.offset)
                .map_err(|_| "scalar instruction offset exceeds host size")?;
            for operation in &instruction.provenance.operations {
                push(
                    &mut rows,
                    source,
                    SemanticCodeSite::Operation(*operation),
                    offset,
                    instruction.bytes.len(),
                )?;
            }
            match &instruction.control {
                FunctionFragmentControlProvenance::Return { psi_return_edge } => {
                    push(
                        &mut rows,
                        source,
                        SemanticCodeSite::Edge(*psi_return_edge),
                        offset,
                        instruction.bytes.len(),
                    )?;
                }
                FunctionFragmentControlProvenance::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => {
                    let branch = instruction
                        .branch
                        .as_ref()
                        .ok_or("scalar branch lacks decoded evidence")?;
                    push(
                        &mut rows,
                        source,
                        SemanticCodeSite::Edge(when_taken.psi_edge),
                        offset,
                        instruction.bytes.len(),
                    )?;
                    push(
                        &mut rows,
                        source,
                        SemanticCodeSite::Edge(when_fallthrough.psi_edge),
                        usize::try_from(branch.when_fallthrough_offset)
                            .map_err(|_| "scalar fallthrough offset exceeds host size")?,
                        0,
                    )?;
                }
                FunctionFragmentControlProvenance::None => {}
                FunctionFragmentControlProvenance::DirectInternalCall { .. } => {
                    return Err("scalar native publication does not admit calls");
                }
            }
        }
    }
    rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset));
    if rows.windows(2).any(|pair| {
        (pair[0].operation_ordinal, pair[0].code_offset)
            == (pair[1].operation_ordinal, pair[1].code_offset)
    }) {
        return Err("scalar attribution has duplicate semantic coordinates");
    }
    Ok(rows)
}

fn push(
    rows: &mut Vec<SemanticCodeAttribution>,
    source: &AbstractFunction,
    site: SemanticCodeSite,
    code_offset: usize,
    byte_count: usize,
) -> Result<(), &'static str> {
    let mut ordinals = source
        .operations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, operation)| matches_site(operation, site).then_some(ordinal));
    let operation_ordinal = ordinals
        .next()
        .ok_or("scalar fragment names a foreign semantic site")?;
    if ordinals.next().is_some() {
        return Err("scalar semantic site occurs more than once");
    }
    rows.push(SemanticCodeAttribution {
        site,
        operation_ordinal,
        code_offset,
        byte_count,
    });
    Ok(())
}

fn matches_site(operation: &AbstractOperation, site: SemanticCodeSite) -> bool {
    match operation {
        AbstractOperation::IntegerConstant { psi_operation, .. }
        | AbstractOperation::IntegerEqual { psi_operation, .. }
        | AbstractOperation::IntegerLessThan { psi_operation, .. }
        | AbstractOperation::IntegerLessOrEqual { psi_operation, .. }
        | AbstractOperation::BooleanNot { psi_operation, .. }
        | AbstractOperation::IntegerWiden { psi_operation, .. }
        | AbstractOperation::ExactIntegerAdd { psi_operation, .. }
        | AbstractOperation::ExactIntegerSubtract { psi_operation, .. } => {
            site == SemanticCodeSite::Operation(*psi_operation)
        }
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            site == SemanticCodeSite::Edge(when_true.psi_edge)
                || site == SemanticCodeSite::Edge(when_false.psi_edge)
        }
        AbstractOperation::Return {
            psi_edge,
            cleanup_actions,
            ..
        } if cleanup_actions.is_empty() => site == SemanticCodeSite::Edge(*psi_edge),
        _ => false,
    }
}

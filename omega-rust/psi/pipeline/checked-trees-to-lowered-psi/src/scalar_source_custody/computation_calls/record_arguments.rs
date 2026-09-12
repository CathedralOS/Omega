//! Shared record operands retain their declared owner and exact call occurrence.
//! The implicit receiver precedes explicit actuals but has no row in their
//! observation roster. Validate it separately, including overlap with that roster.
//! A projected receiver has two identities: the caller's storage root and the
//! authored endpoint captured by the call. Reconstruct both from the source;
//! an equal endpoint type does not permit substituting a sibling or another root.

use crate::{LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::machine::Machine;
use checked_trees::signature::StateParameter;
use checked_trees::state::State;
use checked_trees::statement::StatementNode;
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    BorrowCallFact, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};
use language_core::ReferenceAccess;

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &State,
    statement: u32,
    target_owner: &Machine,
    target: &StateParameter,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &BorrowCallFact,
    explicit_position: Option<usize>,
) -> Result<(), LoweringError> {
    let TypeReferenceNode::Reference {
        access: ReferenceAccess::Shared,
        referee,
        ..
    } = checked
        .type_reference_table
        .type_reference(target.type_reference)
    else {
        return unsupported("record operand requires a shared reference formal");
    };
    if argument.access != CheckedStructuralAccess::SharedBorrow {
        return unsupported("record operand changed its shared custody");
    }
    if !target.is_self && !argument.path.is_empty() {
        return unsupported("explicit record operands require whole-place observation custody");
    }
    let table = &checked.expression_table;
    let named = match table.expression(expression) {
        ExpressionNode::Borrow(borrow) if borrow.access == ReferenceAccess::Shared => borrow.target,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => expression,
        _ => return unsupported("record operand has no named or projected source"),
    };
    let (caller, _) = super::authored_state(checked, state.symbol)?;
    let (root, endpoint) = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. } => {
            let ExpressionNode::Name(name) = table.expression(named) else {
                return unsupported("record local has no whole named referent");
            };
            if !name.symbol.is_valid()
                || name.head_symbol != name.symbol
                || name.members.count() != 1
                || table.name_path_members(name.members).len() != 1
                || !argument.path.is_empty()
            {
                return unsupported("record local substituted its source identity");
            }
            (name.symbol, name.symbol)
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => {
            let source = crate::call_source_custody::projected_receivers::source(
                checked,
                caller.symbol,
                state.symbol,
                statement as usize,
                named,
            )?;
            if source.path != argument.path
                || argument.path.iter().any(|segment| {
                    !matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                    )
                })
            {
                return unsupported("record operand substituted its projected path");
            }
            (source.root, source.endpoint())
        }
        _ => return unsupported("record operand has no supported source owner"),
    };
    let parameters = checked.state_parameters(state);
    let reference = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            if symbol != root
                || parameters
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("record operand substituted its local owner");
            }
            let mut locals = checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some((ordinal, local))
                    }
                    _ => None,
                });
            let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
                "record operand lost its local declaration",
            ))?;
            if locals.next().is_some()
                || ordinal >= statement as usize
                || local.is_mutable
                || !table.expression_is_valid(local.initial_value)
            {
                return unsupported("record operand has no preceding immutable establishment");
            }
            local.type_reference
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let source = parameters
                .iter()
                .filter(|parameter| {
                    checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                })
                .nth(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "record operand lost its structural parameter",
                ))?;
            if source.is_const || source.symbol != root
                || parameters.iter().filter(|parameter| parameter.symbol == root).count() != 1
                || checked.statement_table.statements(state.statement_nodes).iter().any(|statement|
                    matches!(statement, StatementNode::LocalData(local) if local.symbol == root))
            { return unsupported("record operand substituted its source parameter"); }
            let reference = match checked
                .type_reference_table
                .type_reference(source.type_reference)
            {
                TypeReferenceNode::Reference {
                    access: ReferenceAccess::Shared | ReferenceAccess::Mutable,
                    referee,
                    ..
                } => *referee,
                TypeReferenceNode::Named { .. } => source.type_reference,
                _ => return unsupported("record operand widens source access"),
            };
            if source.is_self {
                let TypeReferenceNode::Named { symbol, .. } =
                    checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("record source self lost its nominal attachment");
                };
                if *symbol != caller.symbol && *symbol != caller.attached_data_symbol {
                    return unsupported("record source self changed its attachment");
                }
                checked
                    .type_reference_table
                    .find_named_type_reference(caller.attached_data_symbol)
                    .ok_or(LoweringError::Unsupported(
                        "record source attachment has no declared type",
                    ))?
            } else {
                reference
            }
        }
        _ => return unsupported("record operand has no supported source owner"),
    };
    let reference = if argument.path.is_empty() {
        reference
    } else {
        validation::declared_place_type_raw(&checked.typed, caller, Some(state), named).ok_or(
            LoweringError::Unsupported("record projection lost its declared type"),
        )?
    };
    let TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("record operand lost its exact nominal source type");
    };
    if !checked
        .data_definitions()
        .iter()
        .any(|data| data.symbol == *symbol)
        || !validation::has_plain_owned_contents_with_numeric_constraints(checked, reference)
        || checked.normalized_type_identity(reference).as_str() != argument.type_identity
    {
        return unsupported("record operand changed its declared referent type");
    }
    if target.is_self {
        let TypeReferenceNode::Named {
            symbol: self_symbol,
            ..
        } = checked.type_reference_table.type_reference(*referee)
        else {
            return unsupported("record receiver lost its self referent");
        };
        if (*self_symbol != target_owner.symbol
            && *self_symbol != target_owner.attached_data_symbol)
            || *symbol != target_owner.attached_data_symbol
            || !call.has_receiver
            || call.receiver_symbol != endpoint
            || explicit_position.is_some()
        {
            return unsupported("record receiver disagrees with its resolved attachment");
        }
        let accesses = checked
            .facts
            .borrow
            .argument_accesses
            .span(call.accesses)
            .ok_or(LoweringError::Unsupported(
                "record receiver has stale explicit accesses",
            ))?;
        if accesses.iter().any(|access| {
            (access.root_symbol == root || access.root_symbol == endpoint)
                && access.kind.is_exclusive()
        }) {
            return unsupported("shared record receiver overlaps an exclusive actual");
        }
    } else {
        if checked.normalized_type_identity(*referee) != checked.normalized_type_identity(reference)
        {
            return unsupported("record argument differs from its exact formal referent");
        }
        super::borrow_rows::validate_shared_argument_at(
            checked,
            call,
            explicit_position.ok_or(LoweringError::Unsupported(
                "record argument lost its observation position",
            ))?,
            root,
        )?;
    }
    Ok(())
}

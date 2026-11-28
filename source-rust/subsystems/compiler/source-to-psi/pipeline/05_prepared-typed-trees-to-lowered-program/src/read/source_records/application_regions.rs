//! A closed layout may be shared by uses at different lifetimes. Carry the
//! declared region of each borrowed operand, following its supplying scope.
use super::*;
use crate::source_state::type_identity::{
    LifetimeParameterIdentity, lifetime_parameter_identity, lifetime_parameter_name,
};
use crate::source_state::types::TypeConstraintNode;
use std::sync::Arc;

pub(in crate::read) fn retained_regions(
    typed: &crate::source_state::TypedTrees,
    scope: &Arc<crate::source_state::SourceTypeScope>,
    reference: TypeReferenceHandle,
) -> Option<Vec<LifetimeParameterIdentity>> {
    let mut output = Vec::new();
    let mut ancestors = Vec::new();
    let mut pending = vec![(scope.clone(), reference, 0usize)];
    let table = &typed.type_reference_table;
    while let Some((scope, reference, depth)) = pending.pop() {
        let supplied = scope.supplied_type_reference(typed, reference).ok()?;
        let scope = supplied.scope;
        let reference = supplied.reference;
        ancestors.truncate(depth);
        if ancestors
            .iter()
            .any(|(frame, parent)| *parent == reference && Arc::ptr_eq(frame, &scope))
        {
            return None;
        }
        if scope.region_free_type_identity(typed, reference).is_some() {
            continue;
        }
        ancestors.push((scope.clone(), reference));
        let (tags, children) = match table.type_reference(reference) {
            TypeReferenceNode::Reference {
                lifetime: Some(tag),
                referee,
                ..
            } => {
                let pointee = scope.supplied_type_reference(typed, *referee).ok()?;
                if !pointee.scope.argument_is_closed(typed, pointee.reference) {
                    return None;
                }
                (std::slice::from_ref(tag), vec![*referee])
            }
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                let declared = typed.unique_data_definition_by_symbol(*base_symbol)?;
                if lifetime_arguments.len() != declared.lifetime_parameters.len() {
                    return None;
                }
                (
                    lifetime_arguments.as_slice(),
                    table.type_reference_handles(*arguments).to_vec(),
                )
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let mut children = vec![*base_type];
                for constraint in table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        children.extend_from_slice(&domain.arguments);
                    }
                }
                (&[][..], children)
            }
            TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. } => (&[][..], vec![*element_type]),
            _ => return None,
        };
        if !tags.is_empty() {
            let owner = table.operand_lifetime_owner(reference);
            let actuals = table.operand_lifetimes(reference);
            if !actuals.is_empty() && actuals.len() != tags.len() {
                return None;
            }
            for (ordinal, tag) in tags.iter().enumerate() {
                let named = owner
                    .is_valid()
                    .then(|| lifetime_parameter_identity(typed, owner, tag.as_str()))
                    .flatten();
                let parameter = match actuals.get(ordinal) {
                    Some(parameter) if parameter.declaration.is_valid() => *parameter,
                    None => named?,
                    _ => return None,
                };
                if owner.is_valid() && named != Some(parameter)
                    || lifetime_parameter_name(typed, parameter)?.as_str() != tag.as_str()
                {
                    return None;
                }
                output.push(parameter);
            }
        }
        pending.extend(
            children
                .into_iter()
                .rev()
                .map(|child| (scope.clone(), child, depth + 1)),
        );
    }
    Some(output)
}

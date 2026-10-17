//! Resolve receiver addresses while retaining access at each pointer crossing.
//! A readable exclusive carrier can load its stored exclusive descriptor.
//! A write-only referent cannot: following its stored pointer would read content.

use super::super::{
    TypeReferenceHandle, direct_write_only_root, fixed_discardable_write_only_array_shape,
};
use super::{ExpressionHandle, ExpressionNode, WriteOnlyRoot, bare_field, write_only_record_shape};

pub(in crate::value_custody::write_only_borrows) struct ResolvedProjection {
    pub(in crate::value_custody::write_only_borrows) type_reference: TypeReferenceHandle,
    pub(in crate::value_custody::write_only_borrows) attached_root: bool,
    pub(in crate::value_custody::write_only_borrows) access: language_semantics::ReferenceAccess,
}

#[derive(Clone, Copy)]
enum ProjectionEndpoint {
    Storage,
    Referent,
}

pub(in crate::value_custody::write_only_borrows) fn storage_projection(
    program: &dyn crate::CheckingInput,
    machine: &super::super::Machine,
    state: &super::super::State,
    expression: ExpressionHandle,
    sources: &[WriteOnlyRoot],
    observation_roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<super::super::Diagnostic>,
) -> Option<ResolvedProjection> {
    resolve_projection(
        program,
        expression,
        sources,
        ProjectionEndpoint::Storage,
        &mut |selector, _| {
            super::super::validate_expression(
                program,
                machine,
                state,
                selector,
                observation_roots,
                diagnostics,
            );
            true
        },
    )
}

/// Resolve a borrowed place while checking every selector for observation.
/// Calls and persistent captures evaluate the same content-independent address.
/// Bounds and authority belong to the ordinary place/borrow checkers; a local
/// capture subsequently retains that formation's evaluated coordinates rather
/// than reevaluating the selector when the reference is used.
pub(in crate::value_custody::write_only_borrows) fn nonobserving_projection(
    program: &dyn crate::CheckingInput,
    machine: &super::super::Machine,
    state: &super::super::State,
    expression: ExpressionHandle,
    sources: &[WriteOnlyRoot],
    observation_roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<super::super::Diagnostic>,
) -> Option<ResolvedProjection> {
    projected(program, expression, sources, &mut |selector, _| {
        super::super::validate_expression(
            program,
            machine,
            state,
            selector,
            observation_roots,
            diagnostics,
        );
        true
    })
}

/// Resolve the declared address without reading its contents. Consumers select
/// whether indexes are permitted and check their evaluated expressions; field
/// eligibility, access and the exact referee do not depend on the use's spelling.
pub(in crate::value_custody::write_only_borrows) fn projected(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    admit_selector: &mut dyn FnMut(ExpressionHandle, usize) -> bool,
) -> Option<ResolvedProjection> {
    resolve_projection(
        program,
        expression,
        roots,
        ProjectionEndpoint::Referent,
        admit_selector,
    )
}

fn resolve_projection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    roots: &[WriteOnlyRoot],
    endpoint: ProjectionEndpoint,
    admit_selector: &mut dyn FnMut(ExpressionHandle, usize) -> bool,
) -> Option<ResolvedProjection> {
    let (projections, cursor) =
        crate::expression_traversal::projection_climb(program, expression, usize::MAX)?;
    let (root, mut referee, mut attached_root) =
        if let Some(root) = direct_write_only_root(program, cursor, roots) {
            (root, root.referee, root.receiver_machine.is_valid())
        } else {
            let (root, field) = bare_field(program, cursor, roots)?;
            if field.relevance.is_erased() {
                return None;
            }
            (root, field.type_reference, false)
        };
    let mut access = root.access;
    for projection in projections.into_iter().rev() {
        referee = projected_referee(program, referee, &mut access)?;
        match program.expression_table.expression(projection) {
            ExpressionNode::Member(member) => {
                let field = if attached_root {
                    super::field(program, root, projection)?
                } else {
                    let field = crate::value_custody::places::applied_data_member_field(
                        program, referee, member,
                    )?;
                    if access == language_semantics::ReferenceAccess::WriteOnly {
                        // Only the suffix below a write-only crossing must be
                        // located without observing carrier content. A readable
                        // prefix may load its stored pointer; a write-only prefix
                        // may neither inspect a tag nor read another descriptor.
                        write_only_record_shape(program, referee)?;
                    }
                    // Readability locates a field; it does not discharge the
                    // parent's standing predicates when that field is lent or
                    // written. Preserve this obligation on the entire path,
                    // including readable prefixes before write-only attenuation.
                    let owner =
                        crate::value_custody::places::data_definition_for_type(program, referee)?;
                    if !crate::declarations::data_queries::data_field_is_where_independent(
                        program, owner, field,
                    ) {
                        return None;
                    }
                    field
                };
                if field.relevance.is_erased() {
                    return None;
                }
                referee = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                // Pointer crossings above require readable mutable storage;
                // the array coordinates themselves come from its declared shape.
                let (element, length) = if access == language_semantics::ReferenceAccess::WriteOnly
                {
                    fixed_discardable_write_only_array_shape(program, referee)?
                } else {
                    // A readable prefix merely selects descriptor storage.
                    // Its reference elements need not be discardable values;
                    // the later write-only suffix owns displacement admission.
                    let array = program
                        .type_reference_table
                        .unconstrained_type_reference(referee);
                    let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(array)
                    else {
                        return None;
                    };
                    (
                        *element_type,
                        usize::try_from(program.declared_array_extent(array)?.literal()?).ok()?,
                    )
                };
                if !program.expression_table.expression_is_valid(indexed.index)
                    || !admit_selector(indexed.index, length)
                {
                    return None;
                }
                let (machine, state) = crate::machine_calls::calls::machine_state_by_symbol(
                    program,
                    program.symbols.get(root.symbol).parent,
                )?;
                if !crate::place_has_builtin_coordinates(program, machine, Some(state), projection)
                {
                    return None;
                }
                referee = element;
            }
            _ => return None,
        }
        attached_root = false;
    }
    if matches!(endpoint, ProjectionEndpoint::Referent) {
        referee = projected_referee(program, referee, &mut access)?;
    }
    Some(ResolvedProjection {
        type_reference: referee,
        attached_root,
        access,
    })
}

/// Loading a descriptor requires readable exclusive carrier storage. Its own
/// access becomes the new ceiling; a write-only referent cannot then supply
/// another stored pointer. This loads descriptor content, never referent content.
fn projected_referee(
    program: &dyn crate::CheckingInput,
    mut referee: TypeReferenceHandle,
    access: &mut language_semantics::ReferenceAccess,
) -> Option<TypeReferenceHandle> {
    use language_semantics::ReferenceAccess;
    use typed_trees::types::TypeReferenceNode;
    while let TypeReferenceNode::Reference {
        referee: target,
        access: target_access,
        ..
    } = program.type_reference_table.type_reference(referee)
    {
        if *access != ReferenceAccess::Mutable || !target_access.is_exclusive() {
            return None;
        }
        *access = *target_access;
        referee = *target;
    }
    Some(referee)
}

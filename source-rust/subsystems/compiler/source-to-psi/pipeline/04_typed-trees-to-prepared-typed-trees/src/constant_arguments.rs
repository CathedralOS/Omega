//! Apply the computed value of one type argument to its dependent types and domain facts.
//! The evaluator uses the updated types when planning the next argument. Specialization
//! uses the same operation for expressions copied into a concrete machine.

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionNode;
use crate::source_state::types::{
    DomainConstraintSubject, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle,
    TypeReferenceNode,
};
use typed_trees::data_applications::prepare;

/// Keep the computed scalar as a typed literal at the argument's original handle.
/// Typing applied the data type before the value existed: an array
/// length taken from the argument is still that argument
/// (`FixedArrayLength::Expression`), and the views of the applications
/// that mention it are open. Both are settled here, in place, so a
/// handle typing already handed out keeps naming the same type. A value
/// that is no array length (`-1`) is the error it is for a literal.
pub fn settle_const_argument(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
    literal: ExpressionNode,
) -> Result<(), String> {
    let TypeReferenceNode::ConstExpression(settled_expression) =
        *program.type_reference_table.type_reference(argument)
    else {
        return Err("a settled const argument is not an expression to evaluate".into());
    };

    let extent = match &literal {
        ExpressionNode::Integer(value) => value
            .value_u64()
            .and_then(|value| usize::try_from(value).ok()),
        ExpressionNode::Boolean(_) => None,
        _ => return Err("the value is neither an integer nor a Boolean".into()),
    };
    let mut domain_sites = Vec::new();
    let mut extents = Vec::new();
    let mut applications = Vec::new();
    for (reference, node) in program.type_reference_table.type_reference_rows() {
        match node {
            TypeReferenceNode::Constrained { constraints, .. } => {
                let authored = program.type_reference_table.constraints(*constraints);
                if authored.iter().any(|constraint| matches!(constraint,
                    TypeConstraintNode::Domain(domain) if domain.subject == DomainConstraintSubject::Declared && domain.arguments.contains(&argument)
                )) {
                    domain_sites.push(reference);
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Expression(extent),
            } if *extent == argument => extents.push((reference, *element_type)),
            TypeReferenceNode::Generic { arguments, .. }
                if program.data_application_definition(reference).is_some()
                    && !program.data_application_is_closed(reference) =>
            {
                for child in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    if mentions(program, *child, argument, 0)? {
                        applications.push(reference);
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    // A bad extent leaves the expression and every dependent view available
    // for a later settlement at the same source handle.
    let length = if extents.is_empty() {
        None
    } else {
        Some(extent.ok_or("an applied array length is not a closed nonnegative integer")?)
    };
    finalize_const_expression_operators(program, settled_expression);
    let span = program.expression_table.source_span(settled_expression);
    let value = program.expression_table.insert(literal);
    program.expression_table.set_source_span(value, span);
    program
        .type_reference_table
        .substitute_node(argument, TypeReferenceNode::ConstExpression(value));
    settle_domain_indices(program, argument, &domain_sites)?;
    if let Some(length) = length {
        for (reference, element_type) in extents {
            program.type_reference_table.substitute_node(
                reference,
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                },
            );
        }
    }
    // Forget every stale view before forming any: a view formed while a
    // nested one is still stale would read that one's old verdict.
    for reference in &applications {
        program.discard_open_data_application(*reference);
    }
    for reference in applications {
        prepare(program, reference)?;
    }
    Ok(())
}

/// Complete any `CheckedOperator` selections inside a const expression whose
/// value has just been settled in place, so later selection audits do not see
/// an orphaned late-bound operator on the replaced expression.
fn finalize_const_expression_operators(
    program: &mut TypedTrees,
    root: crate::source_state::expression::ExpressionHandle,
) {
    use source_declarations::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionLateBinding as Late,
        AuthoredDeclarationSelectionTarget as Target,
    };
    let mut pending = vec![root];
    let mut occurrences = Vec::new();
    while let Some(handle) = pending.pop() {
        if !program.expression_table.expression_is_valid(handle) {
            continue;
        }
        match program.expression_table.expression(handle) {
            ExpressionNode::Binary(binary) => {
                for occurrence in program
                    .expression_table
                    .authored_selection_occurrences(handle)
                {
                    if program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|row| {
                            row.kind() == Kind::Operator
                                && row.target() == Target::LateBound(Late::CheckedOperator)
                        })
                    {
                        occurrences.push(occurrence);
                    }
                }
                pending.push(binary.left);
                pending.push(binary.right);
            }
            ExpressionNode::Unary(unary) => {
                for occurrence in program
                    .expression_table
                    .authored_selection_occurrences(handle)
                {
                    if program
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|row| {
                            row.kind() == Kind::Operator
                                && row.target() == Target::LateBound(Late::CheckedOperator)
                        })
                    {
                        occurrences.push(occurrence);
                    }
                }
                pending.push(unary.operand);
            }
            _ => {}
        }
    }
    for occurrence in occurrences {
        let _ = program.finalize_authored_intrinsic_selection(
            occurrence,
            Late::CheckedOperator,
            Intrinsic::BuiltinOperator,
        );
    }
}

/// Replace the former application coordinate wherever the same type occurrence
/// supplied it. Another occurrence of a similar expression keeps its own request.
fn settle_domain_indices(
    program: &mut TypedTrees,
    argument: TypeReferenceHandle,
    domain_sites: &[TypeReferenceHandle],
) -> Result<(), String> {
    let mut domain_changes = Vec::new();
    // The same constraint span can occur on two carriers. Retain the value
    // application for each site before replacing that site's constraint span.
    for &site in domain_sites {
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = *program.type_reference_table.type_reference(site)
        else {
            return Err("an evaluated domain index lost its constrained type".into());
        };
        let carrier = base_type;
        let mut constraints = program
            .type_reference_table
            .constraints(constraints)
            .to_vec();
        for constraint in &mut constraints {
            let TypeConstraintNode::Domain(domain) = constraint else {
                continue;
            };
            if domain.subject != DomainConstraintSubject::Declared
                || !domain.arguments.contains(&argument)
            {
                continue;
            }
            let previous = domain.semantic_id;
            program.rebind_domain_constraint(carrier, domain)?;
            domain_changes.push((
                domain.symbol,
                previous,
                domain.arguments.clone(),
                domain.semantic_id,
            ));
        }
        let constraints = program.type_reference_table.insert_constraints(constraints);
        program
            .type_reference_table
            .set_constraint_span(site, constraints);
    }
    if !domain_changes.is_empty() {
        // A parameter's synthesized membership contract and a cast can retain the
        // same authored index handles. A separate occurrence needs its own evaluation.
        let updated = |symbol, previous, arguments: &[TypeReferenceHandle]| {
            domain_changes
                .iter()
                .find_map(|(declaration, old, original, next)| {
                    (*declaration == symbol && *old == previous && original == arguments)
                        .then_some(*next)
                })
        };
        let facts = program
            .proof_facts
            .iter()
            .filter_map(|(handle, fact)| {
                let membership = fact.as_membership()?;
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments);
                updated(
                    membership.domain_symbol,
                    membership.semantic_domain,
                    arguments,
                )
                .map(|identity| (handle, identity))
            })
            .collect::<Vec<_>>();
        for (handle, identity) in facts {
            if let crate::source_state::domain::ProofFact::Membership(membership) =
                program.proof_facts.get_mut(handle)
            {
                membership.semantic_domain = identity;
            }
        }
        let casts = program
            .expression_table
            .expression_entries()
            .filter_map(|(handle, expression)| {
                let crate::source_state::expression::ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(cast.semantic_domain_arguments);
                updated(
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_id,
                    arguments,
                )
                .map(|identity| (handle, identity))
            })
            .collect::<Vec<_>>();
        for (handle, identity) in casts {
            if let crate::source_state::expression::ExpressionNode::Cast(cast) =
                program.expression_table.expression_mut(handle)
            {
                cast.semantic_domain_id = identity;
            }
        }
    }
    Ok(())
}

/// Whether the type `reference` is or contains the reference `argument`.
/// A malformed or excessively deep dependency cannot establish that an
/// application view is independent of the argument being settled.
fn mentions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    depth: usize,
) -> Result<bool, String> {
    if reference == argument {
        return Ok(true);
    }
    if reference == TypeReferenceHandle::invalid() {
        return Ok(false);
    }
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err("data application has a stale type dependency".into());
    }
    if depth > 64 {
        return Err("data application has an excessively deep type dependency".into());
    }
    let mentions = |child: &TypeReferenceHandle| mentions(program, *child, argument, depth + 1);
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { referee, .. } => mentions(referee),
        TypeReferenceNode::Slice { element_type } => mentions(element_type),
        TypeReferenceNode::Constrained { base_type, .. } => mentions(base_type),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            if let FixedArrayLength::Expression(extent) = length
                && mentions(extent)?
            {
                return Ok(true);
            }
            mentions(element_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .try_fold(
                false,
                |found, child| {
                    if found { Ok(true) } else { mentions(child) }
                },
            ),
        TypeReferenceNode::Callable { contract } => {
            let signature = &program
                .declared_callable_contract(*contract)
                .map_err(str::to_owned)?
                .signature;
            crate::source_state::references::signature_type_inputs(program, signature)
                .types
                .iter()
                .try_fold(
                    false,
                    |found, child| {
                        if found { Ok(true) } else { mentions(child) }
                    },
                )
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => Ok(false),
    }
}

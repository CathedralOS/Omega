//! Type references, constraints, and const-argument slots.
//!
//! Normalized const arguments are validated before their expressions lower.
//! Dynamic trait references and fixed array lengths lower here as well.

use crate::lowering::expression::lower_expression_into_table;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::types::{
    ConstrainedTypeReference, ConstrainedTypeReferenceStorage, FixedArrayLength,
    FixedArrayTypeReference, FixedArrayTypeReferenceStorage, GenericTypeReference,
    GenericTypeReferenceStorage, ReferenceTypeReference, ReferenceTypeReferenceStorage,
    SliceTypeReference, SliceTypeReferenceStorage, TypeConstraint, TypeReference,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_type_reference_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    let prior = lowerer.lowering_const_argument_expressions.len();
    let prior_operators = lowerer.lowering_const_operator_expressions.len();
    let prior_matches = lowerer.lowering_const_match_selections.len();
    let result = lower_type_reference_with_const_arguments(lowerer, syntax_trees, type_reference);
    lowerer.lowering_const_argument_expressions.truncate(prior);
    lowerer
        .lowering_const_operator_expressions
        .truncate(prior_operators);
    lowerer
        .lowering_const_match_selections
        .truncate(prior_matches);
    result
}

fn lower_type_reference_with_const_arguments(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    lowerer.encountered_pending_type_arguments.extend(
        syntax_trees
            .type_references
            .pending_const_arguments(type_reference),
    );
    // Provisional and probe drives lower references before generic synthesis;
    // a surviving generic application is only an obligation on the final pass.
    if !lowerer.structural_type_equations_pending {
        crate::preparation::generic_data::validate_materialized_type_equations(
            syntax_trees,
            type_reference,
            lowerer.constant_selection.as_ref(),
        )?;
    }
    if let Some(normalization) = syntax_trees
        .type_references
        .const_argument_normalization(type_reference)
    {
        crate::constant::validate_normalized_const_argument(
            syntax_trees.type_references.type_reference(type_reference),
            normalization,
        )?;
        crate::constant::validate_normalized_expression(syntax_trees, normalization)?;
        if !lowerer
            .derived_const_argument_expressions
            .contains(&normalization.authored_expression)
        {
            let expressions = syntax_trees
                .type_references
                .const_argument_operator_expressions(normalization.operator_expressions);
            let spans = syntax_trees
                .type_references
                .const_argument_builtin_operators(normalization.builtin_operators);
            if !expressions.is_empty()
                && (expressions.len() != spans.len()
                    || expressions.iter().zip(spans).any(|(expression, span)| {
                        !syntax_trees.expressions.contains_expression(*expression)
                            || syntax_trees.expressions.source_span(*expression) != *span
                    }))
            {
                return Err(Diagnostic::error(
                    "normalized operator lost its exact original expression",
                )
                .with_source_span(normalization.reference));
            }
            lowerer
                .lowering_const_operator_expressions
                .extend_from_slice(expressions);
        }
        lowerer.lowering_const_match_selections.extend_from_slice(
            syntax_trees
                .type_references
                .const_argument_match_selections(normalization.selected_matches),
        );
        if normalization.authored_expression.is_valid()
            && !lowerer
                .derived_const_argument_expressions
                .contains(&normalization.authored_expression)
        {
            if !lowerer
                .lowering_const_argument_expressions
                .iter()
                .any(|(source, _)| *source == normalization.authored_expression)
            {
                let expression = lower_expression_into_table(
                    lowerer,
                    syntax_trees,
                    normalization.authored_expression,
                )?;
                lowerer
                    .lowering_const_argument_expressions
                    .push((normalization.authored_expression, expression));
                lowerer.pending_const_argument_expressions.push(expression);
            }
        }
        for origin in syntax_trees
            .type_references
            .const_argument_origins(normalization.selections)
        {
            let authored_exposure = if lowerer.derived_const_argument_origins.contains(origin) {
                // Copying a template argument preserves its selected operand,
                // without publishing another authored public occurrence.
                None
            } else {
                // Transitive declaration implementation does not become a new
                // public index occurrence merely because its value is copied.
                let authored_here =
                    !syntax_trees.constant_initializer_owns_selection(origin.reference);
                let exposure = if authored_here {
                    lowerer.current_authored_expression_exposure.unwrap_or(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    )
                } else {
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                };
                Some(exposure)
            };
            lowerer.pending_const_argument_selections.push(
                crate::resolution::lowerer::PendingConstArgumentSelection {
                    origin: origin.clone(),
                    authored_exposure,
                    // Exact occurrence custody distinguishes selection of
                    // the whole argument from leaves used to compute it.
                    // An initializer's record dependency may produce a
                    // scalar projection without supplying that record to
                    // the scalar binder. Equal payloads establish no link.
                    is_argument_value: origin.reference == normalization.reference,
                },
            );
        }
    }
    let mut lowered = lower_type_reference_node(lowerer, syntax_trees, type_reference)?;
    let origin = syntax_trees
        .type_references
        .generic_application_origin(type_reference);
    if origin.is_valid() {
        let application = lower_generic_application_origin(lowerer, syntax_trees, origin)?;
        let instance = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .insert(lowered.clone());
        let application = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .insert(application);
        let origin = lowerer
            .symbol_resolved_trees
            .tables
            .types
            .generic_application_origins
            .insert(symbol_resolved_trees::types::GenericApplicationOrigin {
                instance,
                application,
            });
        match &mut lowered {
            TypeReference::Named {
                application_origin, ..
            } => *application_origin = origin,
            TypeReference::Generic(application) => application.application_origin = origin,
            _ => {
                return Err(Diagnostic::error(
                    "generated application lost its occurrence payload",
                ));
            }
        }
    }
    Ok(lowered)
}

/// An already synthesized instance retains its generic tuple as identity
/// metadata, not as an unspecialized application. Its argument types still
/// take ordinary lowering and checking; only this outer metadata node bypasses
/// the requirement that executable type equations have been specialized.
pub(crate) fn lower_generic_application_origin(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    origin: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    if !matches!(
        syntax_trees.type_references.type_reference(origin),
        syntax::types::TypeReferenceNode::Generic { .. }
    ) {
        return Err(Diagnostic::error(
            "generic instance origin must retain a generic application",
        ));
    }
    lower_type_reference_node(lowerer, syntax_trees, origin)
}

fn lower_type_reference_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    if let Some(normalization) = syntax_trees
        .type_references
        .const_argument_normalization(type_reference)
        && matches!(
            language_semantics::const_value::CanonicalConstValue::new(
                "",
                &normalization.canonical_result_encoding,
                "",
            )
            .decode_encoding(),
            Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { .. })
        )
    {
        // Integer value bytes do not carry arithmetic policy. Lower the
        // original operand to produce that meaning from its own graph/carrier.
        // Other completed value categories retain their canonical payload and
        // existing exact selected-declaration/receiving-slot custody above.
        return lower_type_reference_handle(lowerer, syntax_trees, normalization.original_operand);
    }
    let mut lowered = match syntax_trees.type_references.type_reference(type_reference) {
        syntax::types::TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => Ok(TypeReference::Reference(ReferenceTypeReference {
            storage: ReferenceTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                referee: lower_type_reference_child(lowerer, syntax_trees, *referee)?,
                access: *access,
                lifetime: lifetime.as_ref().map(crate::lowering::name::lower_name),
            },
        })),
        syntax::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => Ok(TypeReference::Constrained(ConstrainedTypeReference {
            storage: ConstrainedTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                base_type: lower_type_reference_child(lowerer, syntax_trees, *base_type)?,
                constraints: lower_type_constraint_handles(lowerer, syntax_trees, *constraints)?,
            },
        })),
        syntax::types::TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray(FixedArrayTypeReference {
            storage: FixedArrayTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                element_type: lower_type_reference_child(lowerer, syntax_trees, *element_type)?,
                length: lower_fixed_array_length(length),
            },
        })),
        syntax::types::TypeReferenceNode::Slice { element_type } => {
            Ok(TypeReference::Slice(SliceTypeReference {
                storage: SliceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    element_type: lower_type_reference_child(lowerer, syntax_trees, *element_type)?,
                },
            }))
        }
        syntax::types::TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            crate::preparation::generic_data::validate_direct_const_arguments(
                syntax_trees,
                base_name,
                *arguments,
                lowerer.constant_selection.as_ref(),
            )?;
            let lowered_arguments = lower_child_type_references(lowerer, syntax_trees, *arguments)?;
            Ok(TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    application_origin: arena::Handle::invalid(),
                    base_symbol: SymbolHandle::invalid(),
                    base_name: crate::lowering::name::lower_name(base_name),
                    lifetime_arguments: lifetime_arguments
                        .iter()
                        .map(crate::lowering::name::lower_name)
                        .collect(),
                    arguments: lowered_arguments,
                },
            }))
        }
        syntax::types::TypeReferenceNode::ConstExpression(expression) => {
            let lowered = if let Some((_, lowered)) = lowerer
                .lowering_const_argument_expressions
                .iter()
                .rev()
                .find(|(source, _)| source == expression)
            {
                *lowered
            } else {
                lower_expression_into_table(lowerer, syntax_trees, *expression)?
            };
            Ok(TypeReference::ConstExpression(lowered))
        }
        syntax::types::TypeReferenceNode::DynamicTrait {
            trait_name,
            trait_arguments,
        } => Ok(TypeReference::DynamicTrait {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(trait_name),
            arguments: lower_child_type_references(lowerer, syntax_trees, *trait_arguments)?,
        }),
        syntax::types::TypeReferenceNode::Named(name) => Ok(TypeReference::Named {
            supplied_operand_owner: Default::default(),
            application_origin: arena::Handle::invalid(),
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(name),
        }),
        syntax::types::TypeReferenceNode::Unit => Ok(TypeReference::Unit),
    }?;
    let input = syntax_trees
        .type_references
        .supplied_operand_owner(type_reference)
        .map_err(Diagnostic::error)?;
    let owner = crate::resolution::operand_owners::retain(lowerer, input);
    lowered.retain_supplied_operand_owner(owner);
    Ok(lowered)
}

pub(crate) fn lower_fixed_array_length(
    length: &syntax::types::FixedArrayLength,
) -> FixedArrayLength {
    match length {
        syntax::types::FixedArrayLength::Literal(value) => FixedArrayLength::Literal(*value),
        syntax::types::FixedArrayLength::ConstParameter(name) => FixedArrayLength::ConstParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(name),
        },
        syntax::types::FixedArrayLength::ConstCall(name) => FixedArrayLength::ConstCall {
            machine: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(name),
        },
    }
}

pub(crate) fn lower_type_reference_child(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<arena::Handle<TypeReference>, Diagnostic> {
    let selection_start = lowerer.pending_const_argument_selections.len();
    let lowered = lower_type_reference_handle(lowerer, syntax_trees, type_reference)?;
    let arguments = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .insert_many([lowered]);
    let argument = arguments.handles().next().expect("single lowered child");
    retain_const_argument_slots(
        lowerer,
        syntax_trees,
        [(
            type_reference,
            argument,
            crate::resolution::lowerer::PendingConstArgumentReceiving::OriginalOperand { argument },
        )],
        selection_start,
    );
    Ok(argument)
}

pub(crate) fn lower_child_type_references(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
) -> Result<HandleSpan<TypeReference>, Diagnostic> {
    // An argument's own children (its base type, a nested application's
    // arguments) land in this same arena. Lowering an argument directly into an
    // open span therefore interleaves the next argument's children with the
    // span, and the arena refuses the discontiguous append. Lower every
    // argument first, then place the finished run as one contiguous block.
    let selection_start = lowerer.pending_const_argument_selections.len();
    let mut lowered = Vec::with_capacity(arguments.len());

    for argument in syntax_trees
        .type_references
        .type_reference_handles(arguments)
    {
        lowered.push(lower_type_reference_handle(
            lowerer,
            syntax_trees,
            *argument,
        )?);
    }

    let lowered = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .insert_many(lowered);
    retain_const_argument_slots(
        lowerer,
        syntax_trees,
        syntax_trees
            .type_references
            .type_reference_handles(arguments)
            .iter()
            .copied()
            .enumerate()
            .zip(lowered.handles())
            .map(|((ordinal, source), argument)| {
                (
                    source,
                    argument,
                    crate::resolution::lowerer::PendingConstArgumentReceiving::Generic {
                        arguments: lowered,
                        ordinal,
                    },
                )
            }),
        selection_start,
    );
    Ok(lowered)
}

pub(crate) fn lower_type_constraint_handles(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    constraints: HandleSpan<syntax::types::TypeConstraintNode>,
) -> Result<HandleSpan<TypeConstraint>, Diagnostic> {
    // A domain constraint's arguments may themselves be constrained types, so
    // lowering one constraint can append to this same constraint arena. Collect
    // the whole list before placing it, exactly as the child type references do.
    let mut lowered = Vec::with_capacity(constraints.len());

    for constraint in syntax_trees.type_references.constraints(constraints) {
        lowered.push(lower_type_constraint_handle(
            lowerer,
            syntax_trees,
            constraint,
        )?);
    }

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .types
        .constraints
        .insert_many(lowered))
}

fn lower_type_constraint_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    constraint: &syntax::types::TypeConstraintNode,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        syntax::types::TypeConstraintNode::Named(name) => Ok(TypeConstraint::Named(
            crate::lowering::name::lower_name(name),
        )),
        syntax::types::TypeConstraintNode::Domain(domain) => {
            let arguments = lower_child_type_references(lowerer, syntax_trees, domain.arguments)?;
            Ok(TypeConstraint::Domain(
                symbol_resolved_trees::types::DomainConstraint {
                    name: crate::lowering::name::lower_name(&domain.name),
                    arguments,
                },
            ))
        }
        syntax::types::TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive,
        } => {
            let minimum = lower_expression_into_table(lowerer, syntax_trees, *minimum)?;
            let maximum = lower_expression_into_table(lowerer, syntax_trees, *maximum)?;
            Ok(TypeConstraint::Range {
                minimum,
                maximum,
                end_inclusive: *end_inclusive,
            })
        }
        syntax::types::TypeConstraintNode::ArithmeticDomain(domain) => {
            Ok(TypeConstraint::ArithmeticDomain(*domain))
        }
    }
}

// Static applications own their exact argument spans; standalone operands own
// their exact child handles. Carry those relationships through resolution.
// Source names and equal encoded atoms cannot reconstruct a receiving slot.
fn retain_const_argument_slots(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    arguments: impl IntoIterator<
        Item = (
            syntax::types::TypeReferenceHandle,
            arena::Handle<TypeReference>,
            crate::resolution::lowerer::PendingConstArgumentReceiving,
        ),
    >,
    selection_start: usize,
) {
    for (argument, lowered_argument, receiving) in arguments {
        let Some(normalization) = syntax_trees
            .type_references
            .const_argument_normalization(argument)
        else {
            continue;
        };
        lowerer
            .symbol_resolved_trees
            .tables
            .types
            .normalized_const_arguments
            .insert(symbol_resolved_trees::types::NormalizedConstArgument {
                argument: lowered_argument,
                canonical_result_encoding: normalization.canonical_result_encoding.clone(),
            });
        // Retain the actual child relation. Generic receivers keep their own
        // spans; standalone original operands do not invent a receiving
        // telescope from equal value encodings.
        for selected in syntax_trees
            .type_references
            .const_argument_origins(normalization.selections)
        {
            for (selection, pending) in lowerer
                .pending_const_argument_selections
                .iter()
                .enumerate()
                .skip(selection_start)
            {
                if pending.origin == *selected {
                    lowerer.pending_const_argument_slots.push(
                        crate::resolution::lowerer::PendingConstArgumentSlot {
                            selection,
                            receiving,
                        },
                    );
                }
            }
        }
    }
}

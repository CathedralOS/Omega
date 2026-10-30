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
            syntax_trees,
            syntax_trees.type_references.type_reference(type_reference),
            normalization,
        )?;
        crate::constant::validate_normalized_expression(syntax_trees, normalization)?;
        if normalization.authored_expression.is_valid()
            && !lowerer
                .derived_const_argument_expressions
                .contains(&normalization.authored_expression)
        {
            let expression = lower_expression_into_table(
                lowerer,
                syntax_trees,
                normalization.authored_expression,
            )?;
            lowerer.pending_const_argument_expressions.push(expression);
        }
        for origin in syntax_trees
            .type_references
            .const_argument_origins(normalization.selections)
        {
            // Derived instance arguments still owe the same declaration,
            // value, and receiving-slot custody. Only authored exposure is
            // absent: copying a selected value creates no new public use.
            let exposure = if lowerer.derived_const_argument_origins.contains(origin) {
                None
            } else {
                let authored_here =
                    !syntax_trees.constant_initializer_owns_selection(origin.reference);
                Some(if authored_here {
                    lowerer.current_authored_expression_exposure.unwrap_or(
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                    )
                } else {
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
                })
            };
            lowerer.pending_const_argument_selections.push(
                crate::resolution::lowerer::PendingConstArgumentSelection {
                    origin: origin.clone(),
                    exposure,
                    // Exact occurrence custody distinguishes selection of
                    // the whole argument from leaves used to compute it.
                    // Equal dependency payloads establish no carrier link.
                    is_argument_value: origin.reference == normalization.reference,
                },
            );
        }
        for reference in syntax_trees
            .type_references
            .const_argument_builtin_operators(normalization.builtin_operators)
        {
            if lowerer
                .derived_const_argument_builtin_operators
                .contains(reference)
            {
                continue;
            }
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionExposure as Exposure,
                AuthoredDeclarationSelectionIntrinsic as Intrinsic,
                AuthoredDeclarationSelectionKind as Kind,
                AuthoredDeclarationSelectionLateBinding as LateBinding,
            };
            let exposure = if syntax_trees.constant_initializer_owns_selection(*reference) {
                Exposure::PrivateImplementation
            } else {
                lowerer
                    .current_authored_expression_exposure
                    .unwrap_or(Exposure::PrivateImplementation)
            };
            let occurrence = lowerer
                .symbol_resolved_trees
                .record_late_bound_authored_declaration_selection(
                    *reference,
                    exposure,
                    Kind::Operator,
                    LateBinding::CheckedOperator,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain normalized builtin operator: {error:?}"
                    ))
                    .with_source_span(*reference)
                })?;
            lowerer
                .symbol_resolved_trees
                .finalize_intrinsic_authored_declaration_selection(
                    occurrence,
                    LateBinding::CheckedOperator,
                    Intrinsic::BuiltinOperator,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to finalize normalized builtin operator: {error:?}"
                    ))
                    .with_source_span(*reference)
                })?;
        }
    }
    let lowered = lower_type_reference_node(lowerer, syntax_trees, type_reference)?;
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
        lowerer
            .symbol_resolved_trees
            .tables
            .types
            .generic_application_origins
            .insert(symbol_resolved_trees::types::GenericApplicationOrigin {
                instance,
                application,
            });
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
                lifetime: lifetime
                    .as_ref()
                    .map(|name| crate::lowering::name::lower_name(syntax_trees, name)),
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
                length: lower_fixed_array_length(lowerer, syntax_trees, length)?,
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
            let lowered_arguments =
                lower_indexed_application_arguments(lowerer, syntax_trees, *arguments)?;
            Ok(TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    base_symbol: SymbolHandle::invalid(),
                    base_name: crate::lowering::name::lower_name(syntax_trees, base_name),
                    lifetime_arguments: lifetime_arguments
                        .iter()
                        .map(|name| crate::lowering::name::lower_name(syntax_trees, name))
                        .collect(),
                    arguments: lowered_arguments,
                },
            }))
        }
        syntax::types::TypeReferenceNode::ConstExpression(expression) => {
            let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
            Ok(TypeReference::ConstExpression(expression))
        }
        syntax::types::TypeReferenceNode::DynamicTrait {
            trait_name,
            trait_arguments,
        } => Ok(TypeReference::DynamicTrait {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, trait_name),
            arguments: lower_child_type_references(lowerer, syntax_trees, *trait_arguments)?,
        }),
        syntax::types::TypeReferenceNode::Named(name) => Ok(TypeReference::Named {
            supplied_operand_owner: Default::default(),
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
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
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    length: &syntax::types::FixedArrayLength,
) -> Result<FixedArrayLength, Diagnostic> {
    Ok(match length {
        syntax::types::FixedArrayLength::Expression(value) => {
            // Lower the owned use even after closure: it validates retained
            // normalization and carries declaration/operator custody forward.
            let child = lower_type_reference_child(lowerer, syntax_trees, *value)?;
            if syntax_trees
                .type_references
                .const_argument_normalization(*value)
                .is_some()
                && let Some(name) = syntax_trees
                    .type_references
                    .type_reference(*value)
                    .as_named()
            {
                FixedArrayLength::Literal(name.as_str(syntax_trees).parse::<usize>().map_err(
                    |_| {
                        Diagnostic::error("array extent must be a nonnegative host-sized integer")
                            .with_source_span(name.source_span())
                    },
                )?)
            } else {
                FixedArrayLength::Expression(child)
            }
        }
        syntax::types::FixedArrayLength::Literal(value) => FixedArrayLength::Literal(*value),
        syntax::types::FixedArrayLength::ConstParameter(name) => FixedArrayLength::ConstParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
        },
        syntax::types::FixedArrayLength::ConstCall(name) => FixedArrayLength::ConstCall {
            machine: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(syntax_trees, name),
        },
    })
}

fn lower_type_reference_child(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<arena::Handle<TypeReference>, Diagnostic> {
    let type_reference = lower_type_reference_handle(lowerer, syntax_trees, type_reference)?;

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .append(type_reference))
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

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .insert_many(lowered))
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
            crate::lowering::name::lower_name(syntax_trees, name),
        )),
        syntax::types::TypeConstraintNode::Domain(domain) => {
            let arguments =
                lower_indexed_application_arguments(lowerer, syntax_trees, domain.arguments)?;
            Ok(TypeConstraint::Domain(
                symbol_resolved_trees::types::DomainConstraint {
                    name: crate::lowering::name::lower_name(syntax_trees, &domain.name),
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

/// Lower one indexed application's argument positions: every argument becomes
/// a child type reference, and the const-argument selections the arguments
/// retain receive the slot custody that ties each value to the parameter that
/// received it. Ordinary data applications, indexed domain constraints and
/// membership facts all read their arguments through this one operation.
pub(crate) fn lower_indexed_application_arguments(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
) -> Result<HandleSpan<TypeReference>, Diagnostic> {
    let selection_start = lowerer.pending_const_argument_selections.len();
    let lowered = lower_child_type_references(lowerer, syntax_trees, arguments)?;
    retain_const_argument_slots(lowerer, syntax_trees, arguments, lowered, selection_start);
    Ok(lowered)
}

// Both ordinary data applications and indexed domain constraints own their
// exact argument spans. Carry that relationship privately through resolution;
// source names and equal encoded atoms cannot reconstruct a receiving slot.
pub(super) fn retain_const_argument_slots(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    arguments: HandleSpan<syntax::types::TypeReferenceHandle>,
    lowered_arguments: HandleSpan<TypeReference>,
    selection_start: usize,
) {
    for (ordinal, argument) in syntax_trees
        .type_references
        .type_reference_handles(arguments)
        .iter()
        .enumerate()
    {
        let Some(normalization) = syntax_trees
            .type_references
            .const_argument_normalization(*argument)
        else {
            continue;
        };
        // The actual Generic owns this immutable child span. Nested
        // applications carry their own spans; equal value encodings
        // never establish which parent parameter received a value.
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
                            arguments: lowered_arguments,
                            ordinal,
                        },
                    );
                }
            }
        }
    }
}

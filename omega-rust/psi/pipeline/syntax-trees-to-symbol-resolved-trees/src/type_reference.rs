use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
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
    if let Some(normalization) = syntax_trees
        .type_references
        .const_argument_normalization(type_reference)
    {
        crate::constant::validate_normalized_const_argument(
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
            if !lowerer.derived_const_argument_origins.contains(origin) {
                lowerer.pending_const_argument_selections.push(
            crate::lowerer::PendingConstArgumentSelection {
                origin: origin.clone(),
                exposure: lowerer.current_authored_expression_exposure.unwrap_or(
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
                ),
            },
            );
            }
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
            let occurrence = lowerer
                .symbol_resolved_trees
                .record_late_bound_authored_declaration_selection(
                    *reference,
                    lowerer
                        .current_authored_expression_exposure
                        .unwrap_or(Exposure::PrivateImplementation),
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
        let application = lower_type_reference_handle(lowerer, syntax_trees, origin)?;
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

fn lower_type_reference_node(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    match syntax_trees.type_references.type_reference(type_reference) {
        syntax::types::TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => Ok(TypeReference::Reference(ReferenceTypeReference {
            storage: ReferenceTypeReferenceStorage {
                referee: lower_type_reference_child(lowerer, syntax_trees, *referee)?,
                access: *access,
                lifetime: lifetime.as_ref().map(crate::name::lower_name),
            },
        })),
        syntax::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => Ok(TypeReference::Constrained(ConstrainedTypeReference {
            storage: ConstrainedTypeReferenceStorage {
                base_type: lower_type_reference_child(lowerer, syntax_trees, *base_type)?,
                constraints: lower_type_constraint_handles(lowerer, syntax_trees, *constraints)?,
            },
        })),
        syntax::types::TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => Ok(TypeReference::FixedArray(FixedArrayTypeReference {
            storage: FixedArrayTypeReferenceStorage {
                element_type: lower_type_reference_child(lowerer, syntax_trees, *element_type)?,
                length: lower_fixed_array_length(length),
            },
        })),
        syntax::types::TypeReferenceNode::Slice { element_type } => {
            Ok(TypeReference::Slice(SliceTypeReference {
                storage: SliceTypeReferenceStorage {
                    element_type: lower_type_reference_child(lowerer, syntax_trees, *element_type)?,
                },
            }))
        }
        syntax::types::TypeReferenceNode::Generic {
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            crate::generic_data::validate_direct_const_arguments(
                syntax_trees,
                base_name,
                *arguments,
                lowerer.constant_selection.as_ref(),
            )?;
            let selection_start = lowerer.pending_const_argument_selections.len();
            let lowered_arguments = lower_child_type_references(lowerer, syntax_trees, *arguments)?;
            retain_const_argument_slots(
                lowerer,
                syntax_trees,
                *arguments,
                lowered_arguments,
                selection_start,
            );
            Ok(TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    base_symbol: SymbolHandle::invalid(),
                    base_name: crate::name::lower_name(base_name),
                    lifetime_arguments: lifetime_arguments
                        .iter()
                        .map(crate::name::lower_name)
                        .collect(),
                    arguments: lowered_arguments,
                },
            }))
        }
        syntax::types::TypeReferenceNode::ConstExpression(expression) => {
            let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
            Ok(TypeReference::ConstExpression(expression))
        }
        syntax::types::TypeReferenceNode::DynamicTrait { name, conformance } => {
            lower_dynamic_trait_reference(lowerer, syntax_trees, name, conformance.as_ref())
        }
        syntax::types::TypeReferenceNode::Named(name) => Ok(TypeReference::Named {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        }),
        syntax::types::TypeReferenceNode::SelfType => Ok(TypeReference::SelfType {
            symbol: SymbolHandle::invalid(),
        }),
        syntax::types::TypeReferenceNode::Unit => Ok(TypeReference::Unit),
    }
}

fn lower_dynamic_trait_reference(
    lowerer: &Lowerer,
    syntax_trees: &SyntaxTrees,
    name: &syntax::identifier::Identifier,
    conformance_name: Option<&syntax::identifier::Identifier>,
) -> Result<TypeReference, Diagnostic> {
    let Some(conformance_name) = conformance_name else {
        return Ok(TypeReference::DynamicTrait {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
            conformance: None,
            conformance_carrier: None,
            conformance_name: None,
        });
    };

    let matches = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            syntax::item::Item::Conformance(conformance)
                if matches!(
                    &conformance.subject,
                    syntax::item::ConformanceSubject::Carrier(type_name) if type_name == name
                ) && conformance.alias.as_ref() == Some(conformance_name)
                    && lowerer.source_reference_can_see_declaration(
                        name.source_span(),
                        conformance.trait_name.source_span(),
                    ) =>
            {
                Some(conformance)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [conformance] = matches.as_slice() else {
        let selection = format!("{name}::{conformance_name}");
        return Err(if matches.is_empty() {
            Diagnostic::error(format!(
                "dynamic coercion selects unknown named conformance `{selection}`"
            ))
        } else {
            Diagnostic::error(format!(
                "dynamic coercion selects ambiguous named conformance `{selection}`"
            ))
        });
    };

    Ok(TypeReference::DynamicTrait {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&conformance.trait_name),
        conformance: None,
        conformance_carrier: Some(crate::name::lower_name(name)),
        conformance_name: Some(crate::name::lower_name(conformance_name)),
    })
}

pub(crate) fn lower_fixed_array_length(
    length: &syntax::types::FixedArrayLength,
) -> FixedArrayLength {
    match length {
        syntax::types::FixedArrayLength::Literal(value) => FixedArrayLength::Literal(*value),
        syntax::types::FixedArrayLength::ConstParameter(name) => FixedArrayLength::ConstParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        },
        syntax::types::FixedArrayLength::ConstCall(name) => FixedArrayLength::ConstCall {
            name: crate::name::lower_name(name),
        },
    }
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
    let mut span = HandleSpan::empty();

    for argument in syntax_trees
        .type_references
        .type_reference_handles(arguments)
    {
        let argument = lower_type_reference_handle(lowerer, syntax_trees, *argument)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .append_to_span(&mut span, argument);
    }

    Ok(span)
}

pub(crate) fn lower_type_constraint_handles(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    constraints: HandleSpan<syntax::types::TypeConstraintNode>,
) -> Result<HandleSpan<TypeConstraint>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for constraint in syntax_trees.type_references.constraints(constraints) {
        let constraint = lower_type_constraint_handle(lowerer, syntax_trees, constraint)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .types
            .constraints
            .append_to_span(&mut span, constraint);
    }

    Ok(span)
}

fn lower_type_constraint_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    constraint: &syntax::types::TypeConstraintNode,
) -> Result<TypeConstraint, Diagnostic> {
    match constraint {
        syntax::types::TypeConstraintNode::Named(name) => {
            Ok(TypeConstraint::Named(crate::name::lower_name(name)))
        }
        syntax::types::TypeConstraintNode::Domain(domain) => {
            let selection_start = lowerer.pending_const_argument_selections.len();
            let mut arguments = HandleSpan::empty();
            for argument in syntax_trees
                .type_references
                .type_reference_handles(domain.arguments)
            {
                let argument = lower_type_reference_handle(lowerer, syntax_trees, *argument)?;
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .child_type_references
                    .append_to_span(&mut arguments, argument);
            }
            retain_const_argument_slots(
                lowerer,
                syntax_trees,
                domain.arguments,
                arguments,
                selection_start,
            );
            Ok(TypeConstraint::Domain(
                symbol_resolved_trees::types::DomainConstraint {
                    name: crate::name::lower_name(&domain.name),
                    arguments,
                },
            ))
        }
        syntax::types::TypeConstraintNode::Range { minimum, maximum } => {
            let minimum = lower_expression_into_table(lowerer, syntax_trees, *minimum)?;
            let maximum = lower_expression_into_table(lowerer, syntax_trees, *maximum)?;
            Ok(TypeConstraint::Range { minimum, maximum })
        }
        syntax::types::TypeConstraintNode::ArithmeticDomain(domain) => {
            Ok(TypeConstraint::ArithmeticDomain(*domain))
        }
    }
}

// Both ordinary data applications and indexed domain constraints own their
// exact argument spans. Carry that relationship privately through resolution;
// source names and equal encoded atoms cannot reconstruct a receiving slot.
fn retain_const_argument_slots(
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
                        crate::lowerer::PendingConstArgumentSlot {
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

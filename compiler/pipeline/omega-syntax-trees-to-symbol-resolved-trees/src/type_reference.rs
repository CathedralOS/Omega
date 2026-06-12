use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::types::{
    ConstrainedTypeReference, ConstrainedTypeReferenceStorage, FixedArrayLength,
    FixedArrayTypeReference, FixedArrayTypeReferenceStorage, GenericTypeReference,
    GenericTypeReferenceStorage, ReferenceTypeReference, ReferenceTypeReferenceStorage,
    SliceTypeReference, SliceTypeReferenceStorage, TypeConstraint, TypeReference,
};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_type_reference_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
    match syntax_trees.type_references.type_reference(type_reference) {
        syntax::types::TypeReferenceNode::Reference {
            referee,
            is_mutable,
            is_relaxed,
        } => Ok(TypeReference::Reference(ReferenceTypeReference {
            storage: ReferenceTypeReferenceStorage {
                referee: lower_type_reference_child(lowerer, syntax_trees, *referee)?,
                is_mutable: *is_mutable,
                is_relaxed: *is_relaxed,
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
            arguments,
        } => {
            // `Versioned<T>` is the BUILTIN era-tagged container (frozen
            // decision 14), not a user generic: fold the reference into the
            // synthesized container type's NAME (`Versioned<Counter>`), the
            // same way `Counter::v1` folds into a nameable historical shape.
            // Only a single plain named argument can name a versioned data
            // type; any other shape keeps the generic form (and its
            // unknown-generic diagnostics).
            if base_name.as_str() == "Versioned" {
                let argument_handles = syntax_trees
                    .type_references
                    .type_reference_handles(*arguments);
                if let [argument] = argument_handles
                    && let syntax::types::TypeReferenceNode::Named(argument_name) =
                        syntax_trees.type_references.type_reference(*argument)
                {
                    return Ok(TypeReference::Named {
                        symbol: SymbolHandle::invalid(),
                        name: crate::name::lower_name(
                            &omega_syntax_trees::identifier::Identifier::generated(
                                omega_core::versioning::versioned_container_name(
                                    argument_name.as_str(),
                                ),
                            ),
                        ),
                    });
                }
                return Err(Diagnostic::error(
                    "`Versioned<T>` takes exactly one plain data type argument that declares `version vN { ... }` blocks",
                ));
            }

            Ok(TypeReference::Generic(GenericTypeReference {
                storage: GenericTypeReferenceStorage {
                    base_symbol: SymbolHandle::invalid(),
                    base_name: crate::name::lower_name(base_name),
                    arguments: lower_child_type_references(lowerer, syntax_trees, *arguments)?,
                },
            }))
        }
        syntax::types::TypeReferenceNode::DynamicTrait(name) => Ok(TypeReference::DynamicTrait {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        }),
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

pub(crate) fn lower_fixed_array_length(
    length: &syntax::types::FixedArrayLength,
) -> FixedArrayLength {
    match length {
        syntax::types::FixedArrayLength::Literal(value) => FixedArrayLength::Literal(*value),
        syntax::types::FixedArrayLength::ConstParameter(name) => FixedArrayLength::ConstParameter {
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(name),
        },
    }
}

fn lower_type_reference_child(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<omega_core::arena::Handle<TypeReference>, Diagnostic> {
    let type_reference = lower_type_reference_handle(lowerer, syntax_trees, type_reference)?;

    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .child_type_references
        .append(type_reference))
}

fn lower_child_type_references(
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
        syntax::types::TypeConstraintNode::Range { minimum, maximum } => {
            let expressions = &mut lowerer.symbol_resolved_trees.tables.bodies.expressions;
            let minimum = lower_expression_into_table(syntax_trees, expressions, *minimum)?;
            let maximum = lower_expression_into_table(syntax_trees, expressions, *maximum)?;
            Ok(TypeConstraint::Range { minimum, maximum })
        }
    }
}

use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::types::{
    ConstrainedTypeReference, ConstrainedTypeReferenceStorage, FixedArrayLength,
    FixedArrayTypeReference, FixedArrayTypeReferenceStorage, GenericTypeReference,
    GenericTypeReferenceStorage, ReferenceTypeReference, ReferenceTypeReferenceStorage,
    SliceTypeReference, SliceTypeReferenceStorage, TypeConstraint, TypeReference,
};
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_type_reference_handle(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_reference: syntax::types::TypeReferenceHandle,
) -> Result<TypeReference, Diagnostic> {
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
            .insert(psi_symbol_resolved_trees::types::GenericApplicationOrigin {
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
        } => Ok(TypeReference::Generic(GenericTypeReference {
            storage: GenericTypeReferenceStorage {
                base_symbol: SymbolHandle::invalid(),
                base_name: crate::name::lower_name(base_name),
                lifetime_arguments: lifetime_arguments
                    .iter()
                    .map(crate::name::lower_name)
                    .collect(),
                arguments: lower_child_type_references(lowerer, syntax_trees, *arguments)?,
            },
        })),
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
) -> Result<psi_arena::Handle<TypeReference>, Diagnostic> {
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
            Ok(TypeConstraint::Domain(
                psi_symbol_resolved_trees::types::DomainConstraint {
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

//! Closed representative static applications and immutable type substitution.
//!
//! The quotient planner never mutates the checked type arena. This module owns
//! the one fail-closed judgment that validates a selected representative's full
//! static telescope and compares its substituted declaration types with the
//! concrete quotient-facing runtime telescope.

use typed_trees::TypedTrees;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::expression::{QuotientOperationRequest, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::{
    RelationPlanError, RepresentativeStaticApplication, RepresentativeStaticBinding,
    RepresentativeStaticBindingKind, representative_machine_state,
};

/// Compare one declaration type against a concrete use without mutating the
/// checked type arena. Only the exact, closed static application retained on
/// the representative telescope may replace a declaration binder.
pub(super) fn substituted_type_matches(
    program: &TypedTrees,
    template: TypeReferenceHandle,
    concrete: TypeReferenceHandle,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    let template_node = program.type_reference_table.type_reference(template);
    let concrete_node = program.type_reference_table.type_reference(concrete);
    match (template_node, concrete_node) {
        (
            TypeReferenceNode::Named { symbol, .. },
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. },
        ) => substitutions
            .iter()
            .find(|binding| binding.parameter == *symbol)
            .map_or_else(
                || {
                    program.normalized_type_identity(template)
                        == program.normalized_type_identity(concrete)
                },
                |binding| {
                    binding.kind != RepresentativeStaticBindingKind::Const
                        && static_argument_matches_type(
                            program,
                            &binding.argument,
                            concrete,
                            substitutions,
                        )
                },
            ),
        (
            TypeReferenceNode::Reference {
                referee: template_referee,
                access: template_access,
                ..
            },
            TypeReferenceNode::Reference {
                referee: concrete_referee,
                access: concrete_access,
                ..
            },
        ) => {
            template_access == concrete_access
                && substituted_type_matches(
                    program,
                    *template_referee,
                    *concrete_referee,
                    substitutions,
                )
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: template_element,
                length: template_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: concrete_element,
                length: concrete_length,
            },
        ) => {
            substituted_type_matches(program, *template_element, *concrete_element, substitutions)
                && substituted_array_length_matches(template_length, concrete_length, substitutions)
        }
        (
            TypeReferenceNode::Slice {
                element_type: template_element,
            },
            TypeReferenceNode::Slice {
                element_type: concrete_element,
            },
        ) => substituted_type_matches(program, *template_element, *concrete_element, substitutions),
        (
            TypeReferenceNode::Generic {
                base_symbol: template_base,
                lifetime_arguments: template_lifetimes,
                arguments: template_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: concrete_base,
                lifetime_arguments: concrete_lifetimes,
                arguments: concrete_arguments,
                ..
            },
        ) => {
            if let Some(binding) = substitutions
                .iter()
                .find(|binding| binding.parameter == *template_base)
            {
                return binding.kind != RepresentativeStaticBindingKind::Const
                    && static_argument_matches_type(
                        program,
                        &binding.argument,
                        concrete,
                        substitutions,
                    );
            }
            let template_arguments = program
                .type_reference_table
                .type_reference_handles(*template_arguments);
            let concrete_arguments = program
                .type_reference_table
                .type_reference_handles(*concrete_arguments);
            template_base == concrete_base
                && template_lifetimes == concrete_lifetimes
                && template_arguments.len() == concrete_arguments.len()
                && template_arguments
                    .iter()
                    .zip(concrete_arguments)
                    .all(|(template, concrete)| {
                        substituted_type_matches(program, *template, *concrete, substitutions)
                    })
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        // Constrained/const-expression/dynamic-trait identities can contain
        // more than a closed type/const/machine binder. Until their own exact
        // substitution judgments exist, only an already-identical type passes.
        _ => {
            program.normalized_type_identity(template) == program.normalized_type_identity(concrete)
        }
    }
}

fn substituted_array_length_matches(
    template: &FixedArrayLength,
    concrete: &FixedArrayLength,
    substitutions: &[RepresentativeStaticBinding],
) -> bool {
    match (template, concrete) {
        (FixedArrayLength::Literal(template), FixedArrayLength::Literal(concrete)) => {
            template == concrete
        }
        (FixedArrayLength::ConstParameter { symbol, .. }, FixedArrayLength::Literal(concrete)) => {
            substitutions
                .iter()
                .find(|binding| {
                    binding.parameter == *symbol
                        && binding.kind == RepresentativeStaticBindingKind::Const
                })
                .and_then(|binding| binding.argument.const_literal.as_ref())
                .and_then(|literal| literal.value_u64())
                .and_then(|literal| usize::try_from(literal).ok())
                == Some(*concrete)
        }
        (FixedArrayLength::ConstParameter { symbol, .. }, _) => {
            !substitutions
                .iter()
                .any(|binding| binding.parameter == *symbol)
                && template == concrete
        }
        _ => template == concrete,
    }
}

fn static_argument_matches_type(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    concrete: TypeReferenceHandle,
    _substitutions: &[RepresentativeStaticBinding],
) -> bool {
    if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
        return false;
    }
    let concrete_node = program.type_reference_table.type_reference(concrete);
    let Some(application) = argument.application.as_ref() else {
        let TypeReferenceNode::Named { symbol, name } = concrete_node else {
            return false;
        };
        return if argument.symbol.is_valid() {
            argument.symbol == *symbol
        } else {
            !symbol.is_valid()
                && argument.path.len() == 1
                && argument.path[0].as_str() == name.as_str()
        };
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = concrete_node
    else {
        return false;
    };
    if argument.symbol != *base_symbol
        || application.lifetime_arguments.as_ref() != lifetime_arguments.as_slice()
    {
        return false;
    }
    let concrete_arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    application.arguments.len() == concrete_arguments.len()
        && application
            .arguments
            .iter()
            .zip(concrete_arguments)
            .all(|(argument, concrete)| {
                static_argument_matches_type(program, argument, *concrete, _substitutions)
            })
}

pub(super) fn derive_exact_representative_static_application(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<RepresentativeStaticApplication, RelationPlanError> {
    let (machine, _) =
        representative_machine_state(program, request.representative_operation.symbol)?;
    validate_static_application(
        program,
        &machine.lifetime_parameters,
        program.machine_type_parameters(machine),
        &request.representative_operation,
    )
}

pub(super) fn validate_static_application(
    program: &TypedTrees,
    lifetime_parameters: &[Identifier],
    parameters: &[TypeParameter],
    selected: &StaticMachineArgument,
) -> Result<RepresentativeStaticApplication, RelationPlanError> {
    if !lifetime_parameters.is_empty() {
        return Err(RelationPlanError::RepresentativeLifetimeApplicationRequiresElision);
    }
    let empty_lifetimes: &[Identifier] = &[];
    let empty_arguments: &[StaticMachineArgument] = &[];
    let (lifetime_arguments, arguments) = selected
        .application
        .as_ref()
        .map(|application| {
            (
                application.lifetime_arguments.as_ref(),
                application.arguments.as_ref(),
            )
        })
        .unwrap_or((empty_lifetimes, empty_arguments));
    if !lifetime_arguments.is_empty() || arguments.len() != parameters.len() {
        return Err(RelationPlanError::RepresentativeStaticArityMismatch);
    }

    let mut bindings = Vec::with_capacity(arguments.len());
    for (position, (parameter, argument)) in parameters.iter().zip(arguments).enumerate() {
        let kind = match &parameter.kind {
            TypeParameterKind::Type => {
                validate_closed_type_argument(program, argument, position)?;
                RepresentativeStaticBindingKind::Type
            }
            TypeParameterKind::Const { .. } => {
                if argument.const_literal.is_none()
                    || argument.symbol.is_valid()
                    || argument.application.is_some()
                    || argument.evidence_projection.is_some()
                {
                    return Err(
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                    );
                }
                RepresentativeStaticBindingKind::Const
            }
            TypeParameterKind::Machine { .. } => {
                if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
                    return Err(
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position),
                    );
                }
                let (machine, _) =
                    representative_machine_state(program, argument.symbol).map_err(|_| {
                        RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position)
                    })?;
                validate_static_application(
                    program,
                    &machine.lifetime_parameters,
                    program.machine_type_parameters(machine),
                    argument,
                )?;
                RepresentativeStaticBindingKind::Machine
            }
            TypeParameterKind::Proposition { .. } => {
                return Err(
                    RelationPlanError::RepresentativePropositionApplicationUnsupported(position),
                );
            }
        };
        bindings.push(RepresentativeStaticBinding {
            parameter: parameter.symbol,
            kind,
            argument: argument.clone(),
        });
    }
    Ok(RepresentativeStaticApplication {
        lifetime_arguments: lifetime_arguments.to_vec(),
        bindings,
    })
}

fn validate_closed_type_argument(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    position: usize,
) -> Result<(), RelationPlanError> {
    if argument.const_literal.is_some() || argument.evidence_projection.is_some() {
        return Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position));
    }
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == argument.symbol)
    else {
        let primitive = argument.application.is_none()
            && argument.path.len() == 1
            && PrimitiveType::from_name(argument.path[0].as_str()).is_some();
        if primitive {
            return Ok(());
        }
        return Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(position));
    };
    let nested = validate_static_application(
        program,
        &data.lifetime_parameters,
        program.data_type_parameters(data),
        argument,
    );
    match nested {
        Ok(_) => Ok(()),
        Err(RelationPlanError::RepresentativeStaticArityMismatch) => Err(
            RelationPlanError::RepresentativeStaticArgumentIsOpen(position),
        ),
        Err(error) => Err(error),
    }
}

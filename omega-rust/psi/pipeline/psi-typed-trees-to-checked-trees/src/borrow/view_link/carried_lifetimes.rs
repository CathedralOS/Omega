//! Structural, declaration-derived lifetime frontiers shared by result and
//! input selection. Incomplete frontiers never establish a partial contract.

use super::*;

pub(super) struct CarriedLifetime {
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) lifetime: Option<String>,
    pub(super) access: psi_language_semantics::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeclarationLifetimeFrontier {
    Complete,
    TemplateDependent,
    Incomplete,
}

#[derive(Default)]
struct TemplateFrontier<'a> {
    parameters: &'a [SymbolHandle],
    dependent: bool,
}

/// A declaration may describe a type parameter without knowing its carried
/// references. This is not a complete frontier and never supplies a loan.
pub(crate) fn declaration_lifetime_frontier(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    parameters: &[SymbolHandle],
) -> DeclarationLifetimeFrontier {
    let mut template = TemplateFrontier {
        parameters,
        dependent: false,
    };
    let complete = collect_type(
        program,
        reference,
        &[],
        &[],
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        false,
        &mut template,
    );
    if !complete {
        DeclarationLifetimeFrontier::Incomplete
    } else if template.dependent {
        DeclarationLifetimeFrontier::TemplateDependent
    } else {
        DeclarationLifetimeFrontier::Complete
    }
}

pub(super) fn carried_lifetimes(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<Vec<CarriedLifetime>> {
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        &[],
        &[],
        &mut Vec::new(),
        &mut output,
        false,
        &mut TemplateFrontier::default(),
    )
    .then_some(output)
}

/// Exact call substitutions may prove that a formerly dependent result has
/// no carried views. A closed nonempty frontier still requires per-call loan
/// attribution and cannot use this deliberately narrower result.
pub(crate) fn substituted_result_is_view_free(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    if substitutions.iter().any(|(_, reference)| {
        reference.is_valid()
            && !program
                .type_reference_table
                .contains_type_reference(*reference)
    }) {
        return false;
    }
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        substitutions,
        &[],
        &mut Vec::new(),
        &mut output,
        false,
        &mut TemplateFrontier::default(),
    ) && output.is_empty()
}

/// A nongeneric recursive result can retain one whole input loan under ordinary
/// elision. Repeated declarations need no repeated path expansion here, because
/// no projected frontier is published. Explicit lifetimes or substitutions
/// require the ordinary complete contract and cannot use this fallback.
pub(super) fn whole_elided_result_accesses(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<Vec<psi_language_semantics::ReferenceAccess>> {
    let mut output = Vec::new();
    collect_type(
        program,
        reference,
        &[],
        &[],
        &[],
        &mut Vec::new(),
        &mut output,
        true,
        &mut TemplateFrontier::default(),
    )
    .then(|| output.into_iter().map(|leaf| leaf.access).collect())
}

fn collect_type(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lifetimes: &[(String, String)],
    types: &[(SymbolHandle, TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<SymbolHandle>,
    output: &mut Vec<CarriedLifetime>,
    whole_elided_contract: bool,
    template: &mut TemplateFrontier<'_>,
) -> bool {
    // Zero remains the ordinary absence/Unit convention. A stale nonzero
    // generational handle resolving to the same dummy node is not a type.
    if reference.is_valid()
        && !program
            .type_reference_table
            .contains_type_reference(reference)
    {
        return false;
    }
    if program.primitive_type_reference(reference).is_some() {
        return true;
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference {
            access, lifetime, ..
        } => {
            if whole_elided_contract && lifetime.is_some() {
                return false;
            }
            output.push(CarriedLifetime {
                owner_path: owner_path.to_vec(),
                lifetime: lifetime.as_ref().map(|lifetime| {
                    lifetimes
                        .iter()
                        .rev()
                        .find_map(|(parameter, argument)| {
                            (parameter == lifetime.as_str()).then_some(argument.clone())
                        })
                        .unwrap_or_else(|| lifetime.as_str().to_owned())
                }),
                access: *access,
            });
            true
        }
        TypeReferenceNode::Constrained { base_type, .. } => collect_type(
            program,
            *base_type,
            lifetimes,
            types,
            owner_path,
            visiting,
            output,
            whole_elided_contract,
            template,
        ),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let psi_typed_trees::types::FixedArrayLength::Literal(length) = length else {
                return false;
            };
            (0..*length).all(|index| {
                let mut path = owner_path.to_vec();
                path.push(BorrowOwnerSegment::FixedIndex(index));
                collect_type(
                    program,
                    *element_type,
                    lifetimes,
                    types,
                    &path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                )
            })
        }
        TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } => {
            if whole_elided_contract {
                return false;
            }
            let Some(definition) = data_definition(program, *base_symbol) else {
                return false;
            };
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(definition);
            if definition.lifetime_parameters.len() != lifetime_arguments.len()
                || parameters.len() != arguments.len()
            {
                return false;
            }
            let nested_lifetimes = definition
                .lifetime_parameters
                .iter()
                .zip(lifetime_arguments)
                .map(|(parameter, argument)| {
                    let argument = lifetimes
                        .iter()
                        .rev()
                        .find_map(|(outer, concrete)| {
                            (outer == argument.as_str()).then_some(concrete.clone())
                        })
                        .unwrap_or_else(|| argument.as_str().to_owned());
                    (parameter.as_str().to_owned(), argument)
                })
                .collect::<Vec<_>>();
            let mut nested_types = types.to_vec();
            nested_types.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            collect_data(
                program,
                definition,
                &nested_lifetimes,
                &nested_types,
                owner_path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, concrete)) = types
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                // An unresolved self-substitution is not a finite concrete type.
                if *concrete == reference && template.parameters.contains(symbol) {
                    template.dependent = true;
                    return true;
                }
                if *concrete == reference || visiting.contains(symbol) {
                    return false;
                }
                visiting.push(*symbol);
                let complete = collect_type(
                    program,
                    *concrete,
                    lifetimes,
                    types,
                    owner_path,
                    visiting,
                    output,
                    whole_elided_contract,
                    template,
                );
                visiting.pop();
                return complete;
            }
            let Some(definition) = data_definition(program, *symbol) else {
                if template.parameters.contains(symbol) {
                    template.dependent = true;
                    return true;
                }
                return false;
            };
            if !definition.type_parameters.is_empty()
                || (whole_elided_contract && !definition.lifetime_parameters.is_empty())
            {
                return false;
            }
            collect_data(
                program,
                definition,
                lifetimes,
                types,
                owner_path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        }
        TypeReferenceNode::Unit => true,
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => false,
    }
}

fn collect_data(
    program: &TypedTrees,
    definition: &psi_typed_trees::data::DataDefinition,
    lifetimes: &[(String, String)],
    types: &[(SymbolHandle, TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
    visiting: &mut Vec<SymbolHandle>,
    output: &mut Vec<CarriedLifetime>,
    whole_elided_contract: bool,
    template: &mut TemplateFrontier<'_>,
) -> bool {
    if visiting.contains(&definition.symbol) {
        return whole_elided_contract;
    }
    visiting.push(definition.symbol);
    let complete = program.data_members(definition).iter().all(|member| {
        let fields: &[psi_typed_trees::data::DataField] = match member {
            psi_typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            psi_typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant)
            }
        };
        fields.iter().all(|field| {
            let mut path = owner_path.to_vec();
            if let Some(variant) = psi_facts::payload_variant_for_field(program, field.symbol) {
                path.push(BorrowOwnerSegment::Case(variant));
            }
            path.push(BorrowOwnerSegment::Field(field.symbol));
            collect_type(
                program,
                field.type_reference,
                lifetimes,
                types,
                &path,
                visiting,
                output,
                whole_elided_contract,
                template,
            )
        })
    });
    visiting.pop();
    complete
}

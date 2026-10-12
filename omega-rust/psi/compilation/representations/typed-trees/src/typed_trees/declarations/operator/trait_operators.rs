//! Trait operator applications and selected trait operator meanings.
//! Lexical implementation bounds and abstract trait contracts share the applied
//! operand walk. Inherited arguments remain scoped substitutions, not new type
//! nodes or fabricated closed implementations. Law consumers resolve comparison
//! meaning here before treating an authored equality as mathematical identity.

use crate::TypedTrees;
use crate::type_identity::TypeIdentityRequest;
use crate::typed_trees::declarations::operator::operand_signatures::{
    TypeParameterNormalizer, collect_type_parameter_occurrences, normalized_operand_parameters,
};
use crate::typed_trees::declarations::operator::type_matching::{
    closed_const_identity_from_type_reference, type_reference_matches_with_policy,
};
use crate::types::TypeReferenceHandle;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

/// Whether one exact selected conformance application supplies this
/// trait-owned fixed-token requirement for the complete operand tuple.
///
/// The application is already the proof-static binder selected by generic
/// specialization. Matching therefore consults no visible conformance set: it
/// binds the declared type parameters from the requirement telescope, then
/// checks those bindings against the closed application's trait arguments.
pub fn trait_operator_matches_application(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
    application: &crate::typed_trees::ClosedConformanceApplication,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let Some(bindings) = trait_operator_operand_bindings(
        program,
        trait_definition,
        requirement,
        operand_types,
        None,
    ) else {
        return false;
    };

    let binding_identity = |symbol| {
        bindings.types.iter().find_map(|(bound, actual)| {
            (*bound == symbol).then(|| program.display_type_reference(*actual))
        })
    };
    application.trait_definition != trait_definition.symbol
        || program
            .trait_type_parameters(trait_definition)
            .iter()
            .zip(&application.trait_arguments)
            .all(|(parameter, expected)| {
                binding_identity(parameter.symbol).is_none_or(|actual| actual == *expected)
            })
}

struct TraitOperatorOperandBindings {
    types: Vec<(SymbolHandle, TypeReferenceHandle)>,
    constants: Vec<super::OperatorConstBinding>,
}

/// Public boundary slots are selected by their complete operand telescope,
/// independently of the implementation Build will install. Only original
/// declarations participate; generated headers retain that same slot rather
/// than introducing another spelling candidate.
pub fn boundary_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    use_site: source::SourceSpan,
) -> Vec<(
    &'program crate::trait_definition::TraitDefinition,
    &'program crate::signature::StateSignature,
)> {
    program
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary
                && program
                    .symbols
                    .source_reference_may_select_symbol(use_site, definition.symbol)
        })
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter_map(move |requirement| {
                    (requirement.spelling == Some(spelling)
                        && trait_operator_operand_bindings(
                            program,
                            definition,
                            requirement,
                            operand_types,
                            None,
                        )
                        .is_some())
                    .then_some((definition, requirement))
                })
        })
        .collect()
}

fn trait_operator_operand_bindings(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
    operand_types: &[Option<TypeReferenceHandle>],
    application: Option<&[(SymbolHandle, TypeReferenceHandle)]>,
) -> Option<TraitOperatorOperandBindings> {
    let parameters = program.state_signature_parameters(requirement);
    if parameters.len() != operand_types.len() || operand_types.iter().all(Option::is_none) {
        return None;
    }

    let type_parameters = program
        .trait_type_parameters(trait_definition)
        .iter()
        .chain(program.state_signature_type_parameters(requirement))
        .cloned()
        .collect::<Vec<_>>();
    let mut bindings = Vec::new();
    let mut const_bindings = Vec::new();
    if !operand_types
        .iter()
        .zip(normalized_operand_parameters(parameters))
        .enumerate()
        .all(|(position, (actual, expected))| {
            actual.is_none_or(|actual| {
                let (matched_actual, matched_expected) = super::spellings::operator_operand_types(
                    program,
                    actual,
                    expected.type_reference,
                    position == 0
                        && matches!(
                            requirement.spelling,
                            Some(OperatorSpelling::Index | OperatorSpelling::Range)
                        ),
                );
                if !super::spellings::declared_domain_constraints_match(
                    program,
                    actual,
                    expected.type_reference,
                ) || !super::spellings::declared_domain_constraints_match(
                    program,
                    matched_actual,
                    matched_expected,
                ) {
                    return false;
                }
                // An applied telescope already fixes its trait parameters.
                // Compare those types directly before inferring any remaining
                // requirement parameters. This also preserves symbolic const
                // arguments through inherited applications without type nodes.
                if let Some(substitutions) = application
                    && program.normalized_type_identity(matched_actual)
                        == program.type_identity(TypeIdentityRequest {
                            substitutions,
                            ..TypeIdentityRequest::ordinary(matched_expected)
                        })
                {
                    return true;
                }
                type_reference_matches_with_policy(
                    program,
                    matched_actual,
                    matched_expected,
                    None,
                    &type_parameters,
                    &mut bindings,
                    &mut const_bindings,
                    false,
                )
            })
        })
    {
        return None;
    }
    Some(TraitOperatorOperandBindings {
        types: bindings,
        constants: const_bindings,
    })
}

/// An explicitly selected generic implementation's operation. This is a
/// requirement, not a closed realization map. The substitution chain describes
/// the declaring trait's application through inherited edges without creating
/// type nodes or guessing an implementation from visible declarations.
#[derive(Debug, Clone)]
pub struct BoundTraitOperatorMeaning<'program> {
    pub bound: &'program crate::machine::GenericConformanceBound,
    pub trait_definition: &'program crate::trait_definition::TraitDefinition,
    pub requirement: &'program crate::signature::StateSignature,
    substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

impl BoundTraitOperatorMeaning<'_> {
    /// The requirement result under this exact generic trait application.
    pub fn result_identity(
        &self,
        program: &TypedTrees,
    ) -> crate::type_identity::NormalizedTypeIdentity {
        program.type_identity(TypeIdentityRequest {
            substitutions: &self.substitutions,
            ..TypeIdentityRequest::ordinary(self.requirement.return_type)
        })
    }
}

/// Selected lexical implementation parameters use the same complete operand
/// matching as closed maps. Parent applications remain declaration-scoped
/// substitution chains, so nested arguments compose without spelling lookup.
/// Multiple matching bounds or distinct inherited applications stay ambiguous.
pub fn bound_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    machine_symbol: SymbolHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<BoundTraitOperatorMeaning<'program>> {
    let Some(machine) = program.machine_by_symbol(machine_symbol) else {
        return Vec::new();
    };
    let mut meanings = Vec::new();
    for bound in &machine.conformance_bounds {
        let Some(trait_definition) = program.trait_definition_by_symbol(bound.trait_symbol) else {
            continue;
        };
        let parameters = program.trait_type_parameters(trait_definition);
        if parameters.len() != bound.arguments.len() {
            continue;
        }
        let substitutions = parameters
            .iter()
            .zip(&bound.arguments)
            .map(|(parameter, argument)| (parameter.symbol, *argument))
            .collect::<Vec<_>>();
        visit_trait_operator_meanings(
            program,
            trait_definition,
            spelling,
            operand_types,
            &substitutions,
            &mut |trait_definition, requirement, substitutions| {
                let meaning = BoundTraitOperatorMeaning {
                    bound,
                    trait_definition,
                    requirement,
                    substitutions,
                };
                let same_application = |known: &BoundTraitOperatorMeaning<'_>| {
                    known.bound.binder == bound.binder
                        && known.requirement.symbol == requirement.symbol
                        && program
                            .trait_type_parameters(trait_definition)
                            .iter()
                            .all(|parameter| {
                                let identity =
                                    |substitutions: &[(SymbolHandle, TypeReferenceHandle)]| {
                                        substitutions
                                            .iter()
                                            .find(|(binder, _)| *binder == parameter.symbol)
                                            .map(|(_, argument)| {
                                                program.type_identity(TypeIdentityRequest {
                                                    substitutions,
                                                    ..TypeIdentityRequest::ordinary(*argument)
                                                })
                                            })
                                    };
                                identity(&known.substitutions) == identity(&meaning.substitutions)
                            })
                };
                if !meanings.iter().any(same_application) {
                    meanings.push(meaning);
                }
            },
        );
    }
    meanings
}

/// Operator meaning in a trait's own abstract contract scope, including its
/// inherited applications. The trait's parameters stay rigid lexical symbols;
/// this query neither selects a visible implementation nor closes that scope.
pub fn trait_scope_operator_meaning_exists(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let mut found = false;
    visit_trait_operator_meanings(
        program,
        trait_definition,
        spelling,
        operand_types,
        &[],
        &mut |_, _, _| found = true,
    );
    found
}

fn visit_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    trait_definition: &'program crate::trait_definition::TraitDefinition,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    visit: &mut impl FnMut(
        &'program crate::trait_definition::TraitDefinition,
        &'program crate::signature::StateSignature,
        Vec<(SymbolHandle, TypeReferenceHandle)>,
    ),
) {
    let mut scope = crate::trait_definition::AppliedTraitScope::abstract_scope(trait_definition);
    scope.substitutions = substitutions.to_vec();
    // This meaning query owns erased operand shapes, not source-region custody.
    scope.argument_owner = SymbolHandle::invalid();
    scope.visit(program, &mut |scope| {
        let trait_definition = scope.declaration;
        let substitutions = scope.substitutions.as_slice();
        for requirement in program.trait_machine_signatures(trait_definition) {
            if requirement.spelling != Some(spelling) {
                continue;
            }
            let Some(bindings) = trait_operator_operand_bindings(
                program,
                trait_definition,
                requirement,
                operand_types,
                Some(substitutions),
            ) else {
                continue;
            };
            let matches = program
                .trait_type_parameters(trait_definition)
                .iter()
                .all(|parameter| {
                    let actual = bindings
                        .types
                        .iter()
                        .find(|(binder, _)| *binder == parameter.symbol);
                    let constant = bindings
                        .constants
                        .iter()
                        .find(|binding| binding.symbol == parameter.symbol);
                    if actual.is_none() && constant.is_none() {
                        return true;
                    }
                    let Some(expected) = substitutions
                        .iter()
                        .find(|(binder, _)| *binder == parameter.symbol)
                        .map(|(_, reference)| *reference)
                        .or_else(|| {
                            program
                                .type_reference_table
                                .find_named_type_reference(parameter.symbol)
                        })
                    else {
                        return false;
                    };
                    if let Some(binding) = constant {
                        let crate::data::TypeParameterKind::Const { type_reference } =
                            parameter.kind
                        else {
                            return false;
                        };
                        return closed_const_identity_from_type_reference(
                            program,
                            expected,
                            type_reference,
                        )
                        .is_some_and(|expected| expected == binding.value);
                    }
                    let Some((_, actual)) = actual else {
                        return true;
                    };
                    program.normalized_type_identity(*actual)
                        == program.type_identity(TypeIdentityRequest {
                            substitutions,
                            ..TypeIdentityRequest::ordinary(expected)
                        })
                });
            if !matches {
                continue;
            }
            let mut applied_substitutions = substitutions.to_vec();
            applied_substitutions.extend(bindings.types.into_iter().filter(|(binder, _)| {
                !substitutions
                    .iter()
                    .any(|(parameter, _)| parameter == binder)
            }));
            visit(trait_definition, requirement, applied_substitutions);
        }
    });
}

#[derive(Debug, Clone, Copy)]
pub struct SelectedTraitOperatorMeaning<'program> {
    pub trait_definition: &'program crate::trait_definition::TraitDefinition,
    pub requirement: &'program crate::signature::StateSignature,
    pub application: &'program crate::typed_trees::ClosedConformanceApplication,
    pub row: &'program crate::typed_trees::ClosedConformanceRowIdentity,
}

/// Fixed-token meanings supplied by the proof-static conformance applications
/// already selected on one specialized machine. This is the sole lookup: it
/// walks no package-visible conformance declarations and cannot manufacture an
/// application from operand types.
pub fn selected_trait_operator_meanings<'program>(
    program: &'program TypedTrees,
    machine_symbol: SymbolHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> Vec<SelectedTraitOperatorMeaning<'program>> {
    let Some(specialization) = program.machine_specialization(machine_symbol) else {
        return Vec::new();
    };

    specialization
        .conformance_applications
        .iter()
        .flat_map(|application| {
            application.rows.iter().filter_map(move |row| {
                let trait_definition = program.trait_definition_by_symbol(row.declaring_trait)?;
                let requirement =
                    program.trait_machine_signature_by_symbol(trait_definition, row.requirement)?;
                (requirement.spelling == Some(spelling)
                    && trait_operator_matches_application(
                        program,
                        trait_definition,
                        requirement,
                        application,
                        operand_types,
                    ))
                .then_some(SelectedTraitOperatorMeaning {
                    trait_definition,
                    requirement,
                    application,
                    row,
                })
            })
        })
        .collect()
}

/// [`selected_trait_operator_meanings`] existence without collecting: stops at
/// the first selected meaning instead of materializing the whole row set.
pub fn selected_trait_operator_meaning_exists(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    spelling: OperatorSpelling,
    operand_types: &[Option<TypeReferenceHandle>],
) -> bool {
    let Some(specialization) = program.machine_specialization(machine_symbol) else {
        return false;
    };

    specialization
        .conformance_applications
        .iter()
        .any(|application| {
            application.rows.iter().any(|row| {
                let Some(trait_definition) =
                    program.trait_definition_by_symbol(row.declaring_trait)
                else {
                    return false;
                };
                let Some(requirement) =
                    program.trait_machine_signature_by_symbol(trait_definition, row.requirement)
                else {
                    return false;
                };
                requirement.spelling == Some(spelling)
                    && trait_operator_matches_application(
                        program,
                        trait_definition,
                        requirement,
                        application,
                        operand_types,
                    )
            })
        })
}

/// Canonical operand-type signature for a trait-owned operator requirement.
/// Trait and requirement binders share one alpha-normalized telescope. An
/// attached receiver is always position zero; otherwise the first explicit
/// parameter is position zero, matching expression operand order.
pub fn trait_operator_operand_signature(
    program: &TypedTrees,
    trait_definition: &crate::trait_definition::TraitDefinition,
    requirement: &crate::signature::StateSignature,
) -> String {
    let mut normalizer = TypeParameterNormalizer::new(
        program
            .trait_type_parameters(trait_definition)
            .iter()
            .chain(program.state_signature_type_parameters(requirement))
            .map(|parameter| parameter.symbol)
            .collect(),
    );
    let parameters = program.state_signature_parameters(requirement);
    for parameter in normalized_operand_parameters(parameters) {
        collect_type_parameter_occurrences(program, parameter.type_reference, &mut normalizer);
    }
    let binders = normalizer.bindings();
    normalized_operand_parameters(parameters)
        .map(|parameter| {
            program.type_identity_request_text(TypeIdentityRequest {
                binders: &binders,
                ..TypeIdentityRequest::ordinary(parameter.type_reference)
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

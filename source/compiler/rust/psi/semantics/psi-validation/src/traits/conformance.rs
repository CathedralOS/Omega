use super::shared::trait_definition_by_symbol;
use crate::type_references::{type_reference_label, type_references_match};
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataMember, TypeParameter, TypeParameterKind};
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::{SignatureContractKind, StateParameter, StateSignature};
use psi_typed_trees::state::State;
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(crate) struct GenericBoundRequirement<'program> {
    pub(crate) signature: &'program StateSignature,
    pub(crate) trait_definition: &'program TraitDefinition,
    pub(crate) bound: &'program psi_typed_trees::machine::GenericConformanceBound,
}

pub(crate) fn generic_bound_argument_matches(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    receiver: TypeReferenceHandle,
    requirement: &GenericBoundRequirement<'_>,
) -> bool {
    let mut bindings = vec![TraitTypeBinding {
        parameter_symbol: SymbolHandle::invalid(),
        parameter_name: "Self".to_owned(),
        target: TraitTypeBindingTarget::Type(receiver),
    }];
    bindings.extend(
        program
            .trait_type_parameters(requirement.trait_definition)
            .iter()
            .zip(requirement.bound.arguments.iter())
            .map(|(parameter, argument)| TraitTypeBinding {
                parameter_symbol: parameter.symbol,
                parameter_name: parameter.name.to_string(),
                target: TraitTypeBindingTarget::Type(*argument),
            }),
    );
    type_references_match_with_trait_bindings(
        program,
        actual,
        required,
        program.trait_type_parameters(requirement.trait_definition),
        &mut bindings,
    )
}

pub(crate) fn generic_bound_requirement_call<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Result<Option<GenericBoundRequirement<'program>>, String> {
    let Some(subject) = generic_subject_symbol(program, receiver_type) else {
        return Ok(None);
    };
    let is_type_parameter = program
        .machine_type_parameters(machine)
        .iter()
        .any(|parameter| parameter.symbol == subject);
    if !is_type_parameter {
        return Ok(None);
    }

    let bounds = machine
        .conformance_bounds
        .iter()
        .filter(|bound| bound.subject == subject)
        .collect::<Vec<_>>();
    if bounds.is_empty() {
        let subject_name = program.symbols.name(subject);
        return Err(format!(
            "machine `{}` cannot call `{target}` through unconstrained generic parameter `{subject_name}`; add `where {subject_name} satisfies Trait`",
            machine.name,
        ));
    }

    let mut trait_names = Vec::new();
    let mut requirements = Vec::new();
    for bound in &bounds {
        let trait_definition = if let Some(selected) = bound.selected_conformance_symbol() {
            let declaration = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
                .ok_or_else(|| {
                    format!(
                        "machine `{}` generic call uses unresolved conformance `{}::{}`",
                        machine.name,
                        bound.carrier_name,
                        bound
                            .selected_conformance_name()
                            .map_or("<missing>", |name| name.as_str())
                    )
                })?;
            program
                .traits()
                .iter()
                .find(|candidate| candidate.name == declaration.trait_name)
        } else {
            trait_definition_by_symbol(program, bound.carrier)
        }
        .ok_or_else(|| {
            format!(
                "machine `{}` generic call has no resolved trait for bound on `{}`",
                machine.name, bound.subject_name
            )
        })?;
        trait_names.push(trait_definition.name.as_str());
        requirements.extend(
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .filter(|requirement| requirement.name.as_str() == target)
                .map(|signature| GenericBoundRequirement {
                    signature,
                    trait_definition,
                    bound,
                }),
        );
    }

    let Some(requirement) = requirements.pop() else {
        let subject_name = bounds[0].subject_name.as_str();
        if trait_names.len() == 1 {
            return Err(format!(
                "machine `{}` generic parameter `{subject_name}` is bounded by trait `{}`, which has no requirement `{target}`",
                machine.name, trait_names[0],
            ));
        }
        return Err(format!(
            "machine `{}` generic parameter `{subject_name}` is bounded by traits {}, none of which has requirement `{target}`",
            machine.name,
            trait_names
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    };
    if !requirements.is_empty() {
        return Err(format!(
            "machine `{}` generic call `{target}` is ambiguous across its conformance bounds",
            machine.name,
        ));
    }
    Ok(Some(requirement))
}

fn generic_subject_symbol(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => generic_subject_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            generic_subject_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        _ => None,
    }
}

/// Default native leaves may only expose types whose public shape determines
/// every ABI fact. A source-selected `Calling<C>` policy is the explicit escape
/// hatch: its checked plan can publish a canonical descriptor representation.
/// Without one, private slice/text/vector carriers must stop at a checked
/// adapter rather than silently inheriting the compiler's storage layout.
pub(crate) fn validate_external_leaf_native_shapes(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| conformance.external_binding.is_some())
    {
        // Compiler intrinsics are not
        // foreign ABI leaves: they select a
        // compiler-owned lowering whose safe carrier semantics are already
        // part of the target plan. In particular, Console::read_line may
        // retain its checked mutable-slice surface while the lowering derives
        // the concrete owned destination's capacity and live-length write.
        if matches!(
            machine.supply_mode,
            psi_language_semantics::MachineSupplyMode::ExternalRealization {
                mechanism: psi_language_semantics::ExternalBindingMechanism::CompilerIntrinsic,
                ..
            }
        ) {
            continue;
        }
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        if boundary_has_explicit_calling_policy(program, trait_definition) {
            continue;
        }
        let Some(requirement_name) = conformance.requirement.as_ref() else {
            continue;
        };
        let Some(requirement) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|requirement| requirement.name == *requirement_name)
        else {
            continue;
        };

        for parameter in program
            .state_signature_parameters(requirement)
            .iter()
            .filter(|parameter| !parameter.is_self)
        {
            if let Some(carrier) =
                private_native_carrier(program, parameter.type_reference, &mut Vec::new())
            {
                diagnostics.push(native_carrier_diagnostic(
                    program,
                    machine,
                    trait_definition,
                    requirement,
                    parameter.type_reference,
                    carrier,
                    "parameter",
                ));
            }
        }
        if requirement.return_type.is_valid()
            && let Some(carrier) =
                private_native_carrier(program, requirement.return_type, &mut Vec::new())
        {
            diagnostics.push(native_carrier_diagnostic(
                program,
                machine,
                trait_definition,
                requirement,
                requirement.return_type,
                carrier,
                "result",
            ));
        }
    }
}

fn boundary_has_explicit_calling_policy(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
) -> bool {
    let calling = program.traits().iter().find(|candidate| {
        candidate.name.as_str().rsplit("::").next() == Some("Calling")
            && program.trait_type_parameters(candidate).len() == 1
            && program.trait_machine_signatures(candidate).is_empty()
    });
    calling.is_some_and(|calling| {
        program
            .trait_requirements(trait_definition)
            .iter()
            .any(|requirement| requirement.symbol == calling.symbol)
    })
}

#[derive(Debug, Clone, Copy)]
enum PrivateNativeCarrier {
    Slice,
    Text,
    Vector,
}

impl PrivateNativeCarrier {
    const fn label(self) -> &'static str {
        match self {
            Self::Slice => "safe slice",
            Self::Text => "text view or bounded-text carrier",
            Self::Vector => "vector carrier",
        }
    }
}

fn private_native_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<PrivateNativeCarrier> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
                && carrier_is_slice_or_fixed_array(program, *base_type)
            {
                return Some(PrivateNativeCarrier::Text);
            }
            private_native_carrier(program, *base_type, visiting)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            private_native_carrier(program, *referee, visiting)
        }
        TypeReferenceNode::Slice { .. } => Some(PrivateNativeCarrier::Slice),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            private_native_carrier(program, *element_type, visiting)
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if base_name.as_str().rsplit("::").next() == Some("Vec") {
                return Some(PrivateNativeCarrier::Vector);
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| private_native_carrier(program, *argument, visiting))
        }
        TypeReferenceNode::Named { symbol, name } => {
            if name.as_str().rsplit("::").next() == Some("Vec") {
                return Some(PrivateNativeCarrier::Vector);
            }
            if !symbol.is_valid() || visiting.contains(symbol) {
                return None;
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)?;
            visiting.push(*symbol);
            let carrier = program.data_members(definition).iter().find_map(|member| {
                let DataMember::Field(field) = member else {
                    return None;
                };
                private_native_carrier(program, field.type_reference, visiting)
            });
            visiting.pop();
            carrier
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn carrier_is_slice_or_fixed_array(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => carrier_is_slice_or_fixed_array(program, *referee),
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => true,
        _ => false,
    }
}

fn native_carrier_diagnostic(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    type_reference: TypeReferenceHandle,
    carrier: PrivateNativeCarrier,
    position: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "external leaf `{}` cannot use {} `{}` as a default-native {} for `{}::{}`; declare the foreign pointer/length/terminator or record shape explicitly in a checked adapter, or select a custom `Calling<C>` policy that publishes a canonical representation",
        machine.name,
        carrier.label(),
        program.display_type_reference(type_reference),
        position,
        trait_definition.name,
        requirement.name,
    ))
}

pub(crate) fn validate_machine_trait_conformances(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            if validate_machine_operator_conformance(program, machine, conformance, diagnostics) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies unknown trait `{}`",
                machine.name, conformance.name
            )));
            continue;
        };
        let explicit_type_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let expected_type_arguments = program.trait_type_parameters(trait_definition).len();
        if explicit_type_arguments.len() != expected_type_arguments {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` conformance to trait `{}` expects {expected_type_arguments} generic argument(s), got {}",
                machine.name,
                trait_definition.name,
                explicit_type_arguments.len()
            )));
            continue;
        }

        validate_trait_application_obligations(
            program,
            trait_definition,
            explicit_type_arguments,
            &machine.conformance_bounds,
            &format!(
                "machine `{}` conformance to trait `{}`",
                machine.name, trait_definition.name
            ),
            diagnostics,
        );

        let Some(requirement_name) = conformance.requirement.as_ref() else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has a malformed conformance to `{}` without an exact requirement",
                machine.name, trait_definition.name
            )));
            continue;
        };
        validate_machine_single_requirement(
            program,
            machine,
            trait_definition,
            requirement_name,
            conformance.alias.as_ref().map(|alias| alias.as_str()),
            explicit_type_arguments,
            diagnostics,
        );
    }
}

pub(crate) fn validate_generic_conformance_bounds(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "machine",
        machine.name.as_str(),
        &machine.conformance_bounds,
        diagnostics,
    );
}

pub(crate) fn validate_trait_conformance_bounds(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_conformance_bounds(
        program,
        "trait",
        trait_definition.name.as_str(),
        &trait_definition.conformance_bounds,
        diagnostics,
    );
}

fn validate_conformance_bounds(
    program: &TypedTrees,
    owner_kind: &str,
    owner_name: &str,
    bounds: &[psi_typed_trees::machine::GenericConformanceBound],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for bound in bounds {
        if !bound.subject.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound names unknown type parameter `{}`",
                bound.subject_name
            )));
            continue;
        }

        if let Some(selected) = bound.selected_conformance_symbol() {
            let Some(declaration) = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner_kind} `{owner_name}` conformance bound selects unknown conformance `{}::{}`",
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str())
                )));
                continue;
            };
            let carrier_is_application_parameter = program
                .conformance_type_parameters(declaration)
                .iter()
                .any(|parameter| parameter.symbol == declaration.carrier_symbol);
            if declaration.carrier_symbol != bound.carrier && !carrier_is_application_parameter {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner_kind} `{owner_name}` names conformance `{}::{}`, but that declaration belongs to `{}`",
                    bound.carrier_name,
                    bound
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    declaration
                        .carrier_name()
                        .map_or("<subjectless>", |name| name.as_str()),
                )));
            }
            continue;
        }

        if bound.selected_conformance.is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound selects unknown conformance `{}::{}`",
                bound.carrier_name,
                bound
                    .selected_conformance_name()
                    .map_or("<missing>", |name| name.as_str())
            )));
            continue;
        }

        let Some(trait_definition) = trait_definition_by_symbol(program, bound.carrier) else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound names unknown trait `{}`",
                bound.carrier_name
            )));
            continue;
        };
        let expected = program.trait_type_parameters(trait_definition).len();
        if bound.arguments.len() != expected {
            diagnostics.push(Diagnostic::error(format!(
                "{owner_kind} `{owner_name}` conformance bound for trait `{}` expects {expected} generic argument(s), got {}",
                bound.carrier_name,
                bound.arguments.len(),
            )));
            continue;
        }

        validate_trait_application_obligations(
            program,
            trait_definition,
            &bound.arguments,
            bounds,
            &format!(
                "{owner_kind} `{owner_name}` conformance bound `{} satisfies {}`",
                bound.subject_name, bound.carrier_name
            ),
            diagnostics,
        );
    }
}

/// Validate the obligations declared by a generic trait header at one use of
/// that trait. For example, applying `Calling<C>` also proves the declaration
/// site constraint from `trait Calling<C> where C satisfies CallingPolicy`.
///
/// `available_bounds` is the enclosing generic owner's evidence. Concrete
/// arguments instead discharge obligations through standalone nominal
/// conformance items. This keeps header constraints static and dictionary-free
/// while rejecting an invalid relationship at the site that authored it.
pub(super) fn validate_trait_application_obligations(
    program: &TypedTrees,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    available_bounds: &[psi_typed_trees::machine::GenericConformanceBound],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    if applied_arguments.len() != parameters.len() {
        return;
    }

    validate_machine_declaration_identity_arguments(
        program,
        applied_trait,
        applied_arguments,
        application_label,
        diagnostics,
    );
    validate_proposition_family_arguments(
        program,
        applied_trait,
        applied_arguments,
        application_label,
        diagnostics,
    );

    for obligation in &applied_trait.conformance_bounds {
        let Some(subject_index) = parameters.iter().position(|parameter| {
            (parameter.symbol.is_valid() && parameter.symbol == obligation.subject)
                || parameter.name == obligation.subject_name
        }) else {
            // Declaration validation reports an unknown header subject.
            continue;
        };
        let actual_subject = applied_arguments[subject_index];

        if let Some(selected) = obligation.selected_conformance_symbol() {
            let Some(declaration) = program
                .conformances()
                .iter()
                .find(|declaration| declaration.symbol == selected)
            else {
                // Declaration validation reports the unresolved selection.
                continue;
            };
            let has_exact_evidence =
                generic_argument_symbol(program, actual_subject).is_some_and(|subject_symbol| {
                    available_bounds.iter().any(|candidate| {
                        candidate.subject == subject_symbol
                            && candidate.selected_conformance_symbol() == Some(selected)
                            && candidate.selected_conformance == obligation.selected_conformance
                    })
                }) || declaration.carrier_name().is_some_and(|carrier| {
                    concrete_data_type_name(program, actual_subject) == Some(carrier.as_str())
                });
            if !has_exact_evidence {
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} does not meet trait `{}` header obligation `{}` satisfies exact conformance `{}::{}`: argument `{}` is not data `{}`",
                    applied_trait.name,
                    obligation.subject_name,
                    obligation.carrier_name,
                    obligation
                        .selected_conformance_name()
                        .map_or("<missing>", |name| name.as_str()),
                    program.display_type_reference(actual_subject),
                    declaration
                        .carrier_name()
                        .map_or("<subjectless>", |name| name.as_str()),
                )));
            }
            continue;
        }

        let Some(required_trait) = trait_definition_by_symbol(program, obligation.carrier) else {
            // Declaration validation reports the unresolved trait.
            continue;
        };
        if obligation.arguments.len() != program.trait_type_parameters(required_trait).len() {
            // Declaration validation reports the arity mismatch.
            continue;
        }
        let evidence_count =
            if let Some(subject_symbol) = generic_argument_symbol(program, actual_subject) {
                available_bounds
                    .iter()
                    .filter(|candidate| {
                        candidate.subject == subject_symbol
                            && bound_proves_trait_application(
                                program,
                                candidate,
                                required_trait,
                                &obligation.arguments,
                                applied_trait,
                                applied_arguments,
                            )
                    })
                    .count()
            } else if let Some(type_name) = concrete_data_type_name(program, actual_subject) {
                program
                    .conformances()
                    .iter()
                    .filter(|candidate| {
                        candidate
                            .carrier_name()
                            .is_some_and(|carrier| carrier.as_str() == type_name)
                            && candidate.trait_name == required_trait.name
                            && conformance_arguments_match_application(
                                program,
                                program
                                    .type_reference_table
                                    .type_reference_handles(candidate.arguments),
                                &obligation.arguments,
                                applied_trait,
                                applied_arguments,
                            )
                    })
                    .count()
            } else {
                0
            };

        match evidence_count {
            1 => {}
            0 => diagnostics.push(Diagnostic::error(format!(
                "{application_label} does not meet trait `{}` header obligation `{} satisfies {}` for argument `{}`",
                applied_trait.name,
                obligation.subject_name,
                trait_application_label(program, required_trait, &obligation.arguments),
                program.display_type_reference(actual_subject),
            ))),
            count => diagnostics.push(Diagnostic::error(format!(
                "{application_label} has {count} matching conformances for trait `{}` header obligation `{} satisfies {}`; select one exact named conformance",
                applied_trait.name,
                obligation.subject_name,
                trait_application_label(program, required_trait, &obligation.arguments),
            ))),
        }
    }
}

fn validate_machine_declaration_identity_arguments(
    program: &TypedTrees,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (parameter, argument) in program
        .trait_type_parameters(applied_trait)
        .iter()
        .zip(applied_arguments)
    {
        let TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::RequirementIdentity,
        } = &parameter.kind
        else {
            continue;
        };
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} declaration-identity argument `{}` must be one exact machine or trait-requirement name",
                parameter.name,
            )));
            continue;
        };

        if symbol.is_valid()
            && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::MachineParameter
            && program.data_type_parameters.iter().any(|(_, candidate)| {
                candidate.symbol == *symbol
                    && matches!(
                        candidate.kind,
                        TypeParameterKind::Machine {
                            contract:
                                psi_typed_trees::data::MachineParameterContract::RequirementIdentity
                        }
                    )
            })
        {
            continue;
        }

        if !symbol.is_valid() || program.symbols.get(*symbol).kind != psi_symbols::SymbolKind::State
        {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{name}` for declaration-identity parameter `{}` is not an exact machine or trait requirement",
                parameter.name,
            )));
            continue;
        }

        // Trait requirement paths are signature-free. An overload addition
        // therefore invalidates every such identity use rather than allowing
        // an expected call shape or a visible satisfier to choose one row.
        if let Some((declaring_trait, selected_requirement)) =
            program.traits().iter().find_map(|trait_definition| {
                program
                    .trait_machine_signatures(trait_definition)
                    .iter()
                    .find(|requirement| requirement.symbol == *symbol)
                    .map(|requirement| (trait_definition, requirement))
            })
        {
            let same_name_count = program
                .trait_machine_signatures(declaring_trait)
                .iter()
                .filter(|requirement| requirement.name == selected_requirement.name)
                .count();
            if same_name_count != 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} declaration-identity argument `{name}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                )));
            }
        }
    }
}

fn validate_proposition_family_arguments(
    program: &TypedTrees,
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parameters = program.trait_type_parameters(applied_trait);
    let mut bindings = parameters
        .iter()
        .zip(applied_arguments)
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
        })
        .collect::<Vec<_>>();

    for (parameter, argument) in parameters.iter().zip(applied_arguments) {
        let TypeParameterKind::Proposition { contract: expected } = &parameter.kind else {
            continue;
        };
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition-family argument `{}` must be a direct proposition name",
                parameter.name,
            )));
            continue;
        };
        let actual_declaration = if symbol.is_valid()
            && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::Proposition
        {
            program
                .propositions()
                .iter()
                .find(|proposition| proposition.symbol == *symbol)
        } else {
            None
        };
        let actual_parameters = if let Some(declaration) = actual_declaration {
            Some(program.proposition_parameters(declaration))
        } else if symbol.is_valid()
            && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::PropositionParameter
        {
            program
                .data_type_parameters
                .iter()
                .map(|(_, parameter)| parameter)
                .find_map(|parameter| match &parameter.kind {
                    TypeParameterKind::Proposition { contract } if parameter.symbol == *symbol => {
                        Some(program.state_parameters.span_or_empty(contract.parameters))
                    }
                    _ => None,
                })
        } else {
            None
        };
        let Some(actual_parameters) = actual_parameters else {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{}` for proposition parameter `{}` is not a proposition family",
                name, parameter.name,
            )));
            continue;
        };
        let expected_parameters = program.state_parameters.span_or_empty(expected.parameters);
        if actual_parameters.len() != expected_parameters.len() {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} value parameter(s), but `{}` requires {}",
                actual_parameters.len(),
                parameter.name,
                expected_parameters.len(),
            )));
            continue;
        }
        let actual_binders = actual_declaration
            .map(|declaration| program.proposition_binders(declaration))
            .unwrap_or(&[]);
        let mut indexed_binder_offset = 0usize;
        let mut used_indexed_carrier = false;
        for (index, (actual, expected)) in actual_parameters
            .iter()
            .zip(expected_parameters)
            .enumerate()
        {
            let indexed_match = if actual_binders.is_empty() {
                None
            } else {
                indexed_carrier_parameter_matches(
                    program,
                    expected.type_reference,
                    actual.type_reference,
                    parameters,
                    applied_arguments,
                    actual_binders,
                    &mut indexed_binder_offset,
                )
            };
            used_indexed_carrier |= indexed_match.is_some();
            let matches = indexed_match.unwrap_or_else(|| {
                type_references_match_with_trait_bindings(
                    program,
                    actual.type_reference,
                    expected.type_reference,
                    parameters,
                    &mut bindings,
                )
            });
            if !matches {
                let expected_type =
                    required_trait_type_parameter(program, expected.type_reference, parameters)
                        .and_then(|expected_parameter| {
                            parameters
                                .iter()
                                .position(|candidate| candidate.symbol == expected_parameter.symbol)
                        })
                        .and_then(|index| applied_arguments.get(index).copied())
                        .map(|argument| program.display_type_reference(argument))
                        .unwrap_or_else(|| program.display_type_reference(expected.type_reference));
                diagnostics.push(Diagnostic::error(format!(
                    "{application_label} proposition family `{name}` value parameter {} has type `{}`, but `{}` requires `{}` after substitution",
                    index + 1,
                    program.display_type_reference(actual.type_reference),
                    parameter.name,
                    expected_type,
                )));
            }
        }
        if used_indexed_carrier && indexed_binder_offset != actual_binders.len() {
            diagnostics.push(Diagnostic::error(format!(
                "{application_label} proposition family `{name}` has {} proof-static binder(s), but the substituted carrier telescopes consume {}",
                actual_binders.len(),
                indexed_binder_offset,
            )));
        }
    }
}

fn indexed_carrier_parameter_matches(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
    actual: TypeReferenceHandle,
    trait_parameters: &[TypeParameter],
    applied_arguments: &[TypeReferenceHandle],
    proposition_binders: &[psi_typed_trees::proposition::PropositionBinder],
    binder_offset: &mut usize,
) -> Option<bool> {
    let carrier_parameter = required_trait_type_parameter(program, expected, trait_parameters)?;
    let carrier_index = trait_parameters
        .iter()
        .position(|parameter| parameter.symbol == carrier_parameter.symbol)?;
    let carrier_argument = *applied_arguments.get(carrier_index)?;
    let TypeReferenceNode::Named {
        symbol: carrier_symbol,
        ..
    } = program
        .type_reference_table
        .type_reference(carrier_argument)
    else {
        return None;
    };
    let carrier = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *carrier_symbol)?;
    let carrier_telescope = program.data_type_parameters(carrier);
    if carrier_telescope.is_empty() {
        return None;
    }

    let start = *binder_offset;
    let end = start.saturating_add(carrier_telescope.len());
    *binder_offset = end;
    let Some(binder_group) = proposition_binders.get(start..end) else {
        return Some(false);
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = program.type_reference_table.type_reference(actual)
    else {
        return Some(false);
    };
    if *base_symbol != carrier.symbol {
        return Some(false);
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    if arguments.len() != binder_group.len() {
        return Some(false);
    }

    Some(
        carrier_telescope
            .iter()
            .zip(binder_group)
            .zip(arguments)
            .all(|((carrier_parameter, proposition_binder), argument)| {
                let kind_matches = match (&carrier_parameter.kind, &proposition_binder.kind) {
                    (
                        TypeParameterKind::Type,
                        psi_typed_trees::proposition::PropositionBinderKind::Type,
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        psi_typed_trees::proposition::PropositionBinderKind::Machine,
                    ) => true,
                    (
                        TypeParameterKind::Const {
                            type_reference: carrier_type,
                        },
                        psi_typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference: binder_type,
                        },
                    ) => type_references_match(program, *binder_type, *carrier_type),
                    _ => false,
                };
                kind_matches
                    && matches!(
                        program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::Named { symbol, name }
                            if ((*symbol).is_valid() && *symbol == proposition_binder.symbol)
                                || name == &proposition_binder.name
                    )
            }),
    )
}

fn bound_proves_trait_application(
    program: &TypedTrees,
    bound: &psi_typed_trees::machine::GenericConformanceBound,
    required_trait: &TraitDefinition,
    required_arguments: &[TypeReferenceHandle],
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
) -> bool {
    if let Some(selected) = bound.selected_conformance_symbol() {
        return program
            .conformances()
            .iter()
            .find(|declaration| declaration.symbol == selected)
            .is_some_and(|declaration| {
                declaration.trait_name == required_trait.name
                    && conformance_arguments_match_application(
                        program,
                        program
                            .type_reference_table
                            .type_reference_handles(declaration.arguments),
                        required_arguments,
                        applied_trait,
                        applied_arguments,
                    )
            });
    }
    bound.carrier == required_trait.symbol
        && conformance_arguments_match_application(
            program,
            &bound.arguments,
            required_arguments,
            applied_trait,
            applied_arguments,
        )
}

fn conformance_arguments_match_application(
    program: &TypedTrees,
    actual: &[TypeReferenceHandle],
    required: &[TypeReferenceHandle],
    applied_trait: &TraitDefinition,
    applied_arguments: &[TypeReferenceHandle],
) -> bool {
    let parameters = program.trait_type_parameters(applied_trait);
    let mut bindings = parameters
        .iter()
        .zip(applied_arguments.iter())
        .map(|(parameter, argument)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(*argument),
        })
        .collect::<Vec<_>>();
    actual.len() == required.len()
        && actual
            .iter()
            .zip(required.iter())
            .all(|(actual, required)| {
                type_references_match_with_trait_bindings(
                    program,
                    *actual,
                    *required,
                    parameters,
                    &mut bindings,
                )
            })
}

fn generic_argument_symbol(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => generic_argument_symbol(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            generic_argument_symbol(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, .. }
            if symbol.is_valid()
                && matches!(
                    program.symbols.get(*symbol).kind,
                    psi_symbols::SymbolKind::TypeParameter
                ) =>
        {
            Some(*symbol)
        }
        _ => None,
    }
}

fn concrete_data_type_name(program: &TypedTrees, handle: TypeReferenceHandle) -> Option<&str> {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            concrete_data_type_name(program, *base_type)
        }
        TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && matches!(
                    program.symbols.get(*symbol).kind,
                    psi_symbols::SymbolKind::Data
                ) =>
        {
            Some(name.as_str())
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } if base_symbol.is_valid()
            && matches!(
                program.symbols.get(*base_symbol).kind,
                psi_symbols::SymbolKind::Data
            ) =>
        {
            Some(base_name.as_str())
        }
        _ => None,
    }
}

fn trait_application_label(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    arguments: &[TypeReferenceHandle],
) -> String {
    if arguments.is_empty() {
        return trait_definition.name.to_string();
    }
    format!(
        "{}<{}>",
        trait_definition.name,
        arguments
            .iter()
            .map(|argument| program.display_type_reference(*argument))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Operator requirements share the ordinary machine `satisfies` spelling with
/// trait requirements, but resolve by exact overloaded signature rather than
/// a trait symbol. Boundary leaves retain their admitted-binding rule; checked
/// software providers for either boundary or ordinary operators must cover the
/// selected declaration's supported contract.
fn validate_machine_operator_conformance(
    program: &TypedTrees,
    machine: &Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(requirement) = conformance.requirement.as_ref() else {
        return false;
    };
    let namespace = conformance.name.as_str();
    let requirement_name = requirement.as_str();
    let names_operator = program.operators().iter().any(|operator| {
        let path = program.operator_path_members(operator.name);
        matches!(path, [owner, member]
            if owner.as_str() == namespace
                && member.as_str() == requirement_name)
    });
    if !names_operator {
        return false;
    }

    if !program
        .type_reference_table
        .type_reference_handles(conformance.arguments)
        .is_empty()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` supplies type arguments to operator requirement `{}::{}`; the exact overloaded operator is selected from the machine signature",
            machine.name, namespace, requirement_name,
        )));
        return true;
    }

    let Some(operator) = psi_typed_trees::operator::resolve_satisfied_checked_operator(
        program,
        machine,
        namespace,
        requirement_name,
    ) else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not match one exact overload of operator requirement `{}::{}`; its entry parameter and result types must equal one declared requirement signature",
            machine.name, namespace, requirement_name,
        )));
        return true;
    };

    if conformance.external_binding.is_none() {
        crate::contract_entailment::check_operator_contract_conformance(
            program,
            machine,
            operator,
            diagnostics,
        );
    }
    true
}

/// Conform THIS machine to ONE trait requirement (the machine-by-machine
/// carrier model): the machine's ENTRY signature must match the requirement's
/// (with `Self` binding to the carrier type on first use). LAW requirements
/// (an `ensures` on the requirement) additionally demand a proven-ensures |=
/// declared-law match (rung B: contract_entailment::check_law_conformance).
fn validate_machine_single_requirement(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement_name: &psi_typed_trees::name::Identifier,
    conformance_alias: Option<&str>,
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The machine's conforming signature is its ENTRY state (the first state:
    // an implicit entry always parses first, and single-entry proof machines
    // are the shape this mode serves).
    let Some(entry_state) = program.machine_states(machine).first() else {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}` but has no states",
            machine.name, trait_definition.name, requirement_name
        )));
        return;
    };
    let implementation_dispatch = program.normalized_result_dispatch_set(entry_state.return_type);
    let named_requirements = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|requirement| requirement.name == *requirement_name)
        .collect::<Vec<_>>();
    if named_requirements.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}`, but trait `{}` has no requirement named `{}`",
            machine.name,
            trait_definition.name,
            requirement_name,
            trait_definition.name,
            requirement_name
        )));
        return;
    }
    let matching_requirements = if named_requirements.len() == 1 {
        named_requirements
    } else {
        named_requirements
            .into_iter()
            .filter(|requirement| {
                program.normalized_result_dispatch_set(requirement.return_type)
                    == implementation_dispatch
            })
            .collect::<Vec<_>>()
    };
    let [requirement] = matching_requirements.as_slice() else {
        let dispatch = if implementation_dispatch.is_empty() {
            "<empty>".to_owned()
        } else {
            implementation_dispatch.identity()
        };
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` satisfies `{}::{}`, but its entry result dispatch set `{dispatch}` selects {} matching requirement overload(s); exactly one is required",
            machine.name,
            trait_definition.name,
            requirement_name,
            matching_requirements.len(),
        )));
        return;
    };

    validate_machine_state_satisfies_trait_signature_with_arguments(
        program,
        machine,
        entry_state,
        trait_definition,
        requirement,
        explicit_type_arguments,
        diagnostics,
    );

    // A LAW requirement (ensures on the requirement) demands the satisfier
    // PROVE the law: proven-ensures |= declared-law, forall-to-forall.
    crate::contract_entailment::check_law_conformance(
        program,
        machine,
        conformance_alias,
        trait_definition,
        requirement,
        explicit_type_arguments,
        diagnostics,
    );
}

/// Compose a parent edge such as `Forwarded<U>: Sink<U>` with the concrete
/// arguments of the child instance. The returned handles are existing nodes;
/// exact forwarded parameters need no allocation and cover the canonical
/// parent-binding form while preserving concrete/composite arguments as-is.
pub(super) fn compose_forwarded_trait_arguments(
    program: &TypedTrees,
    source_trait: &TraitDefinition,
    source_arguments: &[TypeReferenceHandle],
    parent_arguments: &[TypeReferenceHandle],
) -> Vec<TypeReferenceHandle> {
    let source_parameters = program.trait_type_parameters(source_trait);
    parent_arguments
        .iter()
        .map(|argument| {
            let TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*argument)
            else {
                return *argument;
            };
            source_parameters
                .iter()
                .zip(source_arguments.iter())
                .find(|(parameter, _)| {
                    (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                        || parameter.name.as_str() == name.as_str()
                })
                .map(|(_, concrete)| *concrete)
                .unwrap_or(*argument)
        })
        .collect()
}

pub(super) fn validate_machine_state_satisfies_trait_signature_with_arguments(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut actual_parameters = program.state_parameters(state);
    let required_parameters = program.state_signature_parameters(requirement);
    // PRV4 self-forwarding adapters: a machine satisfying a BOUNDARY trait
    // requirement may take the trait ITSELF as one extra LEADING parameter
    // (adapter dispatch forwards the call's receiver there); the tail must
    // match the requirement exactly. This includes nominal provider adapters:
    // attachment groups rows into one provider closure but does not add a
    // runtime provider receiver. Plain traits keep the strict positional
    // match.
    if trait_definition.is_boundary
        && actual_parameters.len() == required_parameters.len() + 1
        && actual_parameters.first().is_some_and(|parameter| {
            let label = type_reference_label(program, parameter.type_reference);
            let leaf = label
                .rsplit("::")
                .next()
                .unwrap_or(label.as_str())
                .to_owned();
            let trait_leaf = trait_definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(trait_definition.name.as_str());
            leaf == trait_leaf
        })
    {
        actual_parameters = &actual_parameters[1..];
    }
    if actual_parameters.len() != required_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected {} parameter(s), got {}",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            required_parameters.len(),
            actual_parameters.len()
        )));
        return;
    }

    let trait_type_parameters = program.trait_type_parameters(trait_definition);
    let requirement_type_parameters = program.state_signature_type_parameters(requirement);
    // A machine authored inline in a generic conformance closes over that
    // conformance name's telescope. Those captured parameters specialize the
    // row realization, but they are not callable parameters of the trait
    // requirement itself.
    let captured_parameter_symbols = program
        .conformances()
        .iter()
        .filter(|conformance| {
            program
                .closed_conformance_rows(conformance)
                .is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row.realization_machine == machine.symbol
                            && matches!(
                                row.source,
                                psi_typed_trees::trait_definition::ConformanceRowSource::Inline
                                    | psi_typed_trees::trait_definition::ConformanceRowSource::TraitDefault
                            )
                    })
                })
        })
        .flat_map(|conformance| program.conformance_type_parameters(conformance))
        .map(|parameter| parameter.symbol)
        .collect::<Vec<_>>();
    let actual_type_parameters = program
        .machine_type_parameters(machine)
        .iter()
        .filter(|parameter| !captured_parameter_symbols.contains(&parameter.symbol))
        .cloned()
        .collect::<Vec<_>>();
    let indexed_law_telescope_groups = indexed_law_callable_telescope_groups(
        program,
        trait_definition,
        requirement,
        explicit_type_arguments,
    );
    let expected_callable_parameter_count = requirement_type_parameters.len()
        + indexed_law_telescope_groups
            .iter()
            .map(|group| group.len())
            .sum::<usize>();
    if expected_callable_parameter_count != actual_type_parameters.len() {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: expected {} callable generic parameter(s), got {}",
            machine.name,
            trait_definition.name,
            requirement.name,
            expected_callable_parameter_count,
            actual_type_parameters.len(),
        )));
        return;
    }
    let effective_requirement_type_parameters = requirement_type_parameters
        .iter()
        .chain(
            indexed_law_telescope_groups
                .iter()
                .flat_map(|group| group.iter()),
        )
        .collect::<Vec<_>>();
    if let Some((index, (required, actual))) = effective_requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
        .find(|(_, (required, actual))| {
            !matches!(
                (&required.kind, &actual.kind),
                (TypeParameterKind::Type, TypeParameterKind::Type)
                    | (
                        TypeParameterKind::Const { .. },
                        TypeParameterKind::Const { .. }
                    )
                    | (
                        TypeParameterKind::Machine { .. },
                        TypeParameterKind::Machine { .. }
                    )
            )
        })
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} has incompatible kinds (`{}` versus `{}`)",
            machine.name,
            trait_definition.name,
            requirement.name,
            index,
            required.name,
            actual.name,
        )));
        return;
    }
    let mut required_generic_types = trait_type_parameters.to_vec();
    required_generic_types.extend(
        requirement_type_parameters
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
            .cloned(),
    );
    let generic_type_parameters = required_generic_types.iter().collect::<Vec<_>>();
    crate::machine_parameters::validate_trait_callable_parameter_refinement(
        program,
        &format!(
            "provider machine `{}` for trait requirement `{}::{}`",
            machine.name, trait_definition.name, requirement.name
        ),
        requirement_type_parameters,
        &actual_type_parameters[..requirement_type_parameters.len()],
        &generic_type_parameters,
        diagnostics,
    );
    let mut actual_offset = requirement_type_parameters.len();
    for (representative, carrier_telescope) in indexed_law_telescope_groups.iter().enumerate() {
        let actual_end = actual_offset + carrier_telescope.len();
        let actual_group = &actual_type_parameters[actual_offset..actual_end];
        let carrier_generic_parameters = carrier_telescope.iter().collect::<Vec<_>>();
        crate::machine_parameters::validate_trait_callable_parameter_refinement(
            program,
            &format!(
                "provider machine `{}` indexed representative {} for proposition-law requirement `{}::{}`",
                machine.name,
                representative + 1,
                trait_definition.name,
                requirement.name
            ),
            carrier_telescope,
            actual_group,
            &carrier_generic_parameters,
            diagnostics,
        );
        actual_offset = actual_end;
    }
    let mut type_bindings = trait_type_parameters
        .iter()
        .zip(explicit_type_arguments.iter().copied())
        .map(|(parameter, actual)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            target: TraitTypeBindingTarget::Type(actual),
        })
        .collect::<Vec<_>>();
    type_bindings.extend(
        requirement_type_parameters
            .iter()
            .zip(actual_type_parameters.iter())
            .filter_map(|(required, actual)| {
                matches!(required.kind, TypeParameterKind::Type).then(|| TraitTypeBinding {
                    parameter_symbol: required.symbol,
                    parameter_name: required.name.as_str().to_owned(),
                    target: TraitTypeBindingTarget::Parameter(actual.symbol),
                })
            }),
    );
    for (index, (required, actual)) in requirement_type_parameters
        .iter()
        .zip(actual_type_parameters.iter())
        .enumerate()
    {
        match (&required.kind, &actual.kind) {
            (TypeParameterKind::Type, TypeParameterKind::Type) => {
                if (actual.bounds.multiplicity
                    == psi_language_semantics::Multiplicity::Unrestricted
                    && required.bounds.multiplicity
                        != psi_language_semantics::Multiplicity::Unrestricted)
                    || actual.bounds.carry.is_some()
                        && actual.bounds.carry != required.bounds.carry
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` does not satisfy trait `{}` machine `{}`: callable generic parameter {} demands stronger type properties",
                        machine.name, trait_definition.name, requirement.name, index
                    )));
                }
            }
            (
                TypeParameterKind::Const {
                    type_reference: required_type,
                },
                TypeParameterKind::Const {
                    type_reference: actual_type,
                },
            ) if !type_references_match_with_trait_bindings(
                program,
                *actual_type,
                *required_type,
                &required_generic_types,
                &mut type_bindings,
            ) => diagnostics.push(Diagnostic::error(format!(
                "machine `{}` does not satisfy trait `{}` machine `{}`: callable const generic parameter {} has a different type",
                machine.name, trait_definition.name, requirement.name, index
            ))),
            _ => {}
        }
    }

    for (index, (actual, required)) in actual_parameters
        .iter()
        .zip(required_parameters.iter())
        .enumerate()
    {
        validate_trait_parameter_match(
            program,
            machine,
            state,
            trait_definition.name.as_str(),
            requirement,
            &required_generic_types,
            &mut type_bindings,
            index,
            actual,
            required,
            diagnostics,
        );
    }

    if !type_references_match_with_trait_bindings(
        program,
        state.return_type,
        requirement.return_type,
        &required_generic_types,
        &mut type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: expected return `{}`, got `{}`",
            machine.name,
            state.name,
            trait_definition.name,
            requirement.name,
            type_reference_label(program, requirement.return_type),
            type_reference_label(program, state.return_type)
        )));
    }

    validate_trait_effect_ceiling(
        program,
        machine,
        state,
        trait_definition.name.as_str(),
        requirement,
        diagnostics,
    );
}

fn validate_trait_effect_ceiling(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let allowed_services = program
        .service_reach_rows
        .services(requirement.service_reach_row);
    for service in program
        .service_reach_rows
        .services(machine.service_reach_row)
    {
        if !allowed_services.contains(service) {
            let service_name = program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>");
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: service `{service_name}` is not allowed by the trait requirement",
                machine.name,
                state.name,
                trait_name,
                requirement.name,
            )));
        }
    }

    if machine.suspends && !requirement.suspends {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `suspends;` exceeds the trait requirement's operational ceiling",
            machine.name, state.name, trait_name, requirement.name,
        )));
    }
    if machine.blocks && !requirement.blocks {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}`: `blocks;` exceeds the trait requirement's operational ceiling",
            machine.name, state.name, trait_name, requirement.name,
        )));
    }
}

fn validate_trait_parameter_match(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    trait_name: &str,
    requirement: &StateSignature,
    trait_type_parameters: &[TypeParameter],
    type_bindings: &mut Vec<TraitTypeBinding>,
    index: usize,
    actual: &StateParameter,
    required: &StateParameter,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if actual.is_self != required.is_self || actual.is_mutable != required.is_mutable {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter {}: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            index,
            parameter_shape_label(program, required),
            parameter_shape_label(program, actual)
        )));
        return;
    }

    // `&self` establishes receiver shape, not a reusable type-variable
    // binding. Attached machine self parameters carry the syntax placeholder
    // `Self`; binding the trait's `Self` to that placeholder makes a later
    // concrete `other: &Type` spuriously mismatch. The carrier is inferred
    // from the first non-receiver `Self` occurrence instead.
    if required.is_self {
        return;
    }

    if !type_references_match_with_trait_bindings(
        program,
        actual.type_reference,
        required.type_reference,
        trait_type_parameters,
        type_bindings,
    ) {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` does not satisfy trait `{}` machine `{}` parameter `{}`: expected `{}`, got `{}`",
            machine.name,
            state.name,
            trait_name,
            requirement.name,
            required.name,
            type_reference_label(program, required.type_reference),
            type_reference_label(program, actual.type_reference)
        )));
    }
}

#[derive(Debug, Clone)]
struct TraitTypeBinding {
    parameter_symbol: SymbolHandle,
    parameter_name: String,
    target: TraitTypeBindingTarget,
}

#[derive(Debug, Clone)]
enum TraitTypeBindingTarget {
    Type(TypeReferenceHandle),
    Parameter(SymbolHandle),
}

fn type_references_match_with_trait_bindings(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
    trait_type_parameters: &[TypeParameter],
    bindings: &mut Vec<TraitTypeBinding>,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    // `Self` in a requirement type binds to the CARRIER on first use and must
    // match on every later use (rearrange settle 2026-07-18: free-machine
    // requirements are Self-shaped -- `machine add(a: Self, b: Self) -> Self`
    // -- and the carrier type is INFERRED from the satisfier's signature).
    if required_is_self_type(program, required) {
        if let Some(binding) = bindings.iter().find(|binding| {
            !binding.parameter_symbol.is_valid() && binding.parameter_name == "Self"
        }) {
            return binding_matches_actual(program, binding, actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: "Self".to_owned(),
            target: TraitTypeBindingTarget::Type(actual),
        });
        return true;
    }

    if let Some(parameter) = required_trait_type_parameter(program, required, trait_type_parameters)
    {
        if let Some(binding) = bindings.iter().find(|binding| {
            binding.parameter_symbol == parameter.symbol
                && binding.parameter_name == parameter.name.as_str()
        }) {
            return binding_matches_actual(program, binding, actual);
        }

        bindings.push(TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.to_string(),
            target: TraitTypeBindingTarget::Type(actual),
        });
        return true;
    }

    match (
        program.type_reference_table.type_reference(actual),
        program.type_reference_table.type_reference(required),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: actual_referee,
                access: actual_access,
                // Lifetimes do not participate in trait-conformance matching.
                lifetime: _,
            },
            TypeReferenceNode::Reference {
                referee: required_referee,
                access: required_access,
                lifetime: _,
            },
        ) => {
            actual_access == required_access
                && type_references_match_with_trait_bindings(
                    program,
                    *actual_referee,
                    *required_referee,
                    trait_type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Constrained {
                base_type: actual_base,
                ..
            },
            TypeReferenceNode::Constrained {
                base_type: required_base,
                ..
            },
        ) => type_references_match_with_trait_bindings(
            program,
            *actual_base,
            *required_base,
            trait_type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::FixedArray {
                element_type: actual_element,
                length: actual_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: required_element,
                length: required_length,
            },
        ) => {
            actual_length == required_length
                && type_references_match_with_trait_bindings(
                    program,
                    *actual_element,
                    *required_element,
                    trait_type_parameters,
                    bindings,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: actual_element,
            },
            TypeReferenceNode::Slice {
                element_type: required_element,
            },
        ) => type_references_match_with_trait_bindings(
            program,
            *actual_element,
            *required_element,
            trait_type_parameters,
            bindings,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: actual_base_symbol,
                arguments: actual_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: required_base_symbol,
                arguments: required_arguments,
                ..
            },
        ) => {
            actual_base_symbol.is_valid()
                && actual_base_symbol == required_base_symbol
                && actual_arguments.count() == required_arguments.count()
                && program
                    .type_reference_table
                    .type_reference_handles(*actual_arguments)
                    .iter()
                    .zip(
                        program
                            .type_reference_table
                            .type_reference_handles(*required_arguments)
                            .iter(),
                    )
                    .all(|(actual_argument, required_argument)| {
                        type_references_match_with_trait_bindings(
                            program,
                            *actual_argument,
                            *required_argument,
                            trait_type_parameters,
                            bindings,
                        )
                    })
        }
        (
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            },
            TypeReferenceNode::Generic { .. },
        ) if actual_symbol.is_valid() => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *actual_symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|origin| {
                type_references_match_with_trait_bindings(
                    program,
                    origin,
                    required,
                    trait_type_parameters,
                    bindings,
                )
            }),
        _ => type_references_match(program, actual, required),
    }
}

fn binding_matches_actual(
    program: &TypedTrees,
    binding: &TraitTypeBinding,
    actual: TypeReferenceHandle,
) -> bool {
    match &binding.target {
        TraitTypeBindingTarget::Type(expected) => match (
            program.type_reference_table.type_reference(actual),
            program.type_reference_table.type_reference(*expected),
        ) {
            (
                TypeReferenceNode::Named {
                    symbol: actual_symbol,
                    ..
                },
                TypeReferenceNode::Named {
                    symbol: expected_symbol,
                    ..
                },
            ) if actual_symbol.is_valid() && expected_symbol.is_valid() => {
                actual_symbol == expected_symbol
                    || type_reference_is_instance_of_family(program, actual, *expected)
            }
            _ => {
                type_references_match(program, actual, *expected)
                    || type_reference_is_instance_of_family(program, actual, *expected)
            }
        },
        TraitTypeBindingTarget::Parameter(symbol) => matches!(
            program.type_reference_table.type_reference(actual),
            TypeReferenceNode::Named {
                symbol: actual_symbol,
                ..
            } if (*symbol).is_valid() && *actual_symbol == *symbol
        ),
    }
}

fn type_reference_is_instance_of_family(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    family: TypeReferenceHandle,
) -> bool {
    let TypeReferenceNode::Named {
        symbol: family_symbol,
        ..
    } = program.type_reference_table.type_reference(family)
    else {
        return false;
    };
    let Some(family_definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *family_symbol)
    else {
        return false;
    };
    if program.data_type_parameters(family_definition).is_empty() {
        return false;
    }
    matches!(
        program.type_reference_table.type_reference(actual),
        TypeReferenceNode::Generic { base_symbol, .. } if base_symbol == family_symbol
    )
}

fn indexed_law_callable_telescope_groups<'program>(
    program: &'program TypedTrees,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_type_arguments: &[TypeReferenceHandle],
) -> Vec<&'program [TypeParameter]> {
    let has_proposition_law = program
        .state_signature_contracts(requirement)
        .iter()
        .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| matches!(fact, ProofFact::Proposition(_)));
    if !has_proposition_law {
        return Vec::new();
    }
    let trait_parameters = program.trait_type_parameters(trait_definition);
    program
        .state_signature_parameters(requirement)
        .iter()
        .filter_map(|representative| {
            let carrier_parameter = required_trait_type_parameter(
                program,
                representative.type_reference,
                trait_parameters,
            )?;
            let carrier_index = trait_parameters
                .iter()
                .position(|parameter| parameter.symbol == carrier_parameter.symbol)?;
            let carrier_argument = *explicit_type_arguments.get(carrier_index)?;
            let TypeReferenceNode::Named {
                symbol: carrier_symbol,
                ..
            } = program
                .type_reference_table
                .type_reference(carrier_argument)
            else {
                return None;
            };
            let carrier = program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *carrier_symbol)?;
            let telescope = program.data_type_parameters(carrier);
            (!telescope.is_empty()).then_some(telescope)
        })
        .collect()
}

fn required_is_self_type(program: &TypedTrees, required: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(required),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "Self"
    )
}

fn required_trait_type_parameter<'program>(
    program: &'program TypedTrees,
    required: TypeReferenceHandle,
    trait_type_parameters: &'program [TypeParameter],
) -> Option<&'program TypeParameter> {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(required)
    else {
        return None;
    };

    symbol.is_valid().then_some(())?;
    trait_type_parameters
        .iter()
        .find(|parameter| parameter.symbol.is_valid() && parameter.symbol == *symbol)
}

fn parameter_shape_label(program: &TypedTrees, parameter: &StateParameter) -> String {
    let qualifier = if parameter.is_mutable { "mut " } else { "" };
    if parameter.is_self {
        format!("&{qualifier}self")
    } else {
        format!(
            "{}: {}",
            parameter.name,
            type_reference_label(program, parameter.type_reference)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TraitTypeBinding, TraitTypeBindingTarget, type_references_match_with_trait_bindings,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::{DataDefinition, TypeParameter};
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn named(program: &mut TypedTrees, symbol: SymbolHandle, name: &str) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(name),
            })
    }

    fn generic(
        program: &mut TypedTrees,
        base_symbol: SymbolHandle,
        base_name: &str,
        arguments: impl IntoIterator<Item = TypeReferenceHandle>,
    ) -> TypeReferenceHandle {
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles(arguments);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name: Identifier::generated(base_name),
                lifetime_arguments: Vec::new(),
                arguments,
            })
    }

    fn generated_instance(
        program: &mut TypedTrees,
        symbol: SymbolHandle,
        diagnostic_name: &str,
        origin: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        program.push_data_definition(DataDefinition {
            symbol,
            name: Identifier::generated(diagnostic_name),
            generic_instance: Some(origin),
            ..DataDefinition::default()
        });
        named(program, symbol, diagnostic_name)
    }

    fn value_binding(
        parameter_symbol: SymbolHandle,
        concrete: TypeReferenceHandle,
    ) -> Vec<TraitTypeBinding> {
        vec![TraitTypeBinding {
            parameter_symbol,
            parameter_name: "Value".to_owned(),
            target: TraitTypeBindingTarget::Type(concrete),
        }]
    }

    #[test]
    fn generated_nested_generic_origin_substitutes_exact_trait_argument() {
        let mut program = TypedTrees::default();
        let value_symbol = SymbolHandle::from_arena_index(10);
        let outer_symbol = SymbolHandle::from_arena_index(20);
        let inner_symbol = SymbolHandle::from_arena_index(21);
        let message_symbol = SymbolHandle::from_arena_index(30);
        let message = named(&mut program, message_symbol, "Message");
        let value = named(&mut program, value_symbol, "Value");
        let actual_inner_origin = generic(&mut program, inner_symbol, "ignored-inner", [message]);
        let actual_inner = generated_instance(
            &mut program,
            SymbolHandle::from_arena_index(40),
            "untrusted synthetic spelling",
            actual_inner_origin,
        );
        let actual_outer_origin =
            generic(&mut program, outer_symbol, "ignored-outer", [actual_inner]);
        let actual = generated_instance(
            &mut program,
            SymbolHandle::from_arena_index(41),
            "another irrelevant spelling",
            actual_outer_origin,
        );
        let required_inner = generic(&mut program, inner_symbol, "Relayed", [value]);
        let required = generic(&mut program, outer_symbol, "DecodeResult", [required_inner]);
        let parameters = [TypeParameter {
            symbol: value_symbol,
            name: Identifier::generated("Value"),
            ..TypeParameter::default()
        }];

        assert!(type_references_match_with_trait_bindings(
            &program,
            actual,
            required,
            &parameters,
            &mut value_binding(value_symbol, message),
        ));
    }

    #[test]
    fn generated_nested_generic_origin_rejects_mismatched_concrete_argument() {
        let mut program = TypedTrees::default();
        let value_symbol = SymbolHandle::from_arena_index(10);
        let outer_symbol = SymbolHandle::from_arena_index(20);
        let inner_symbol = SymbolHandle::from_arena_index(21);
        let message_symbol = SymbolHandle::from_arena_index(30);
        let other_symbol = SymbolHandle::from_arena_index(31);
        let message = named(&mut program, message_symbol, "SameDisplayName");
        let other = named(&mut program, other_symbol, "SameDisplayName");
        let value = named(&mut program, value_symbol, "Value");
        let actual_inner_origin = generic(&mut program, inner_symbol, "Relayed", [other]);
        let actual_inner = generated_instance(
            &mut program,
            SymbolHandle::from_arena_index(40),
            "Relayed<SameDisplayName>",
            actual_inner_origin,
        );
        let actual_outer_origin =
            generic(&mut program, outer_symbol, "DecodeResult", [actual_inner]);
        let actual = generated_instance(
            &mut program,
            SymbolHandle::from_arena_index(41),
            "DecodeResult<Relayed<SameDisplayName>>",
            actual_outer_origin,
        );
        let required_inner = generic(&mut program, inner_symbol, "Relayed", [value]);
        let required = generic(&mut program, outer_symbol, "DecodeResult", [required_inner]);
        let parameters = [TypeParameter {
            symbol: value_symbol,
            name: Identifier::generated("Value"),
            ..TypeParameter::default()
        }];

        assert!(!type_references_match_with_trait_bindings(
            &program,
            actual,
            required,
            &parameters,
            &mut value_binding(value_symbol, message),
        ));
    }
}

//! Pre-resolution synthesis for trait machines with authored bodies.
//!
//! A standalone `Data satisfies Trait;` conformance may omit a machine when
//! the trait supplies its body. Materialize that body as an ordinary attached
//! machine before symbol resolution, so typing, effects, proofs, dispatch, both
//! execution engines, and override precedence all reuse the established paths.

use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{
    ConformanceBody, ConformanceItem, ConformanceMember, Item, ItemHandle, Machine,
    SatisfiesClause, State, StateSignatureNode,
};
use psi_syntax_trees::statement::StatementNode;
use psi_syntax_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone)]
struct TraitDefaultsInput {
    has_lifetime_parameters: bool,
    parameter_names: Vec<String>,
    signatures: Vec<TraitSignatureInput>,
    requirements: Vec<TraitRequirementInput>,
}

#[derive(Clone)]
struct TraitSignatureInput {
    ordinal: usize,
    signature: StateSignatureNode,
}

#[derive(Clone)]
struct TraitRequirementInput {
    name: String,
    arguments: Vec<TypeReferenceHandle>,
}

#[derive(Clone)]
struct DefaultCandidate {
    origin: String,
    requirement_owner: Option<String>,
    signature: StateSignatureNode,
    substitution: HashMap<String, TypeReferenceHandle>,
}

type EffectiveDefaults = BTreeMap<String, Vec<DefaultCandidate>>;

#[derive(Clone)]
struct ConformanceInput {
    handle: ItemHandle,
    declaration: ConformanceItem,
}

#[derive(Clone)]
struct RequirementInstance {
    declaring_trait: String,
    requirement_ordinal: usize,
    signature: StateSignatureNode,
    substitution: HashMap<String, TypeReferenceHandle>,
}

pub fn synthesize_trait_defaults(syntax: &mut SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    let data_names = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(data) => Some(data.name.as_str().to_string()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let traits = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Trait(trait_definition) = item else {
                return None;
            };
            let parameter_names = syntax
                .items
                .type_parameters(trait_definition.type_parameters)
                .iter()
                .map(|parameter| parameter.name.as_str().to_string())
                .collect();
            let signatures = syntax
                .items
                .state_signatures(trait_definition.machines)
                .iter()
                .enumerate()
                .map(|(ordinal, handle)| TraitSignatureInput {
                    ordinal,
                    signature: syntax.items.state_signature(*handle).clone(),
                })
                .collect::<Vec<_>>();
            let mut requirements = Vec::new();
            for handle in syntax
                .type_references
                .type_reference_handles(trait_definition.parents)
            {
                match syntax.type_references.type_reference(*handle) {
                    TypeReferenceNode::Named(name) => requirements.push(TraitRequirementInput {
                        name: name.as_str().to_string(),
                        arguments: Vec::new(),
                    }),
                    TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } => requirements.push(TraitRequirementInput {
                        name: base_name.as_str().to_string(),
                        arguments: syntax
                            .type_references
                            .type_reference_handles(*arguments)
                            .to_vec(),
                    }),
                    _ => {}
                }
            }
            requirements.extend(
                syntax
                    .items
                    .identifier_path_members(trait_definition.requires)
                    .iter()
                    .map(|name| TraitRequirementInput {
                        name: name.as_str().to_string(),
                        arguments: Vec::new(),
                    }),
            );
            Some((
                trait_definition.name.as_str().to_string(),
                TraitDefaultsInput {
                    has_lifetime_parameters: !trait_definition.lifetime_parameters.is_empty(),
                    parameter_names,
                    signatures,
                    requirements,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let conformances = syntax
        .root_item_handles()
        .iter()
        .filter_map(|handle| match syntax.root_item(*handle) {
            Item::Conformance(conformance) => Some(ConformanceInput {
                handle: *handle,
                declaration: conformance.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut attached_methods = HashSet::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        for state in syntax.items.state_handles(machine.states) {
            attached_methods.insert((
                attached.as_str().to_string(),
                syntax.items.state(*state).name.as_str().to_string(),
            ));
        }
    }

    let mut diagnostics = Vec::new();
    let mut reported_conflicts = HashSet::new();
    for conformance in conformances {
        let carrier_name = match &conformance.declaration.subject {
            psi_syntax_trees::item::ConformanceSubject::Carrier(type_name) => {
                Some(type_name.as_str().to_string())
            }
            psi_syntax_trees::item::ConformanceSubject::Subjectless => None,
        };
        let conformance_name = carrier_name.clone().unwrap_or_else(|| {
            conformance
                .declaration
                .alias
                .as_ref()
                .expect("parsed subjectless conformances are named")
                .as_str()
                .to_string()
        });
        let trait_name = conformance.declaration.trait_name.as_str().to_string();
        let arguments = syntax
            .type_references
            .type_reference_handles(conformance.declaration.trait_arguments)
            .to_vec();
        if carrier_name
            .as_ref()
            .is_some_and(|type_name| !data_names.contains(type_name))
        {
            continue;
        }
        let Some(conformed_trait) = traits.get(&trait_name) else {
            continue;
        };
        if arguments.len() != conformed_trait.parameter_names.len() {
            diagnostics.push(Diagnostic::error(format!(
                "conformance `{conformance_name} satisfies {trait_name}` expects {} generic argument(s), got {}",
                conformed_trait.parameter_names.len(),
                arguments.len()
            )));
            continue;
        }
        let substitution = conformed_trait
            .parameter_names
            .iter()
            .cloned()
            .zip(arguments.iter().copied())
            .collect::<HashMap<_, _>>();

        if let ConformanceBody::Closed { members } = conformance.declaration.body.clone() {
            let mut closed_members = syntax.items.conformance_members(members).to_vec();
            let mut requirements = Vec::new();
            collect_requirement_instances(
                syntax,
                &trait_name,
                &substitution,
                &traits,
                &mut HashSet::new(),
                &mut HashSet::new(),
                &mut requirements,
            );
            let mut existing_defaults = HashSet::new();
            for member in &closed_members {
                if let ConformanceMember::TraitDefault {
                    declaring_trait,
                    requirement_ordinal,
                    ..
                } = member
                {
                    existing_defaults
                        .insert((declaring_trait.as_str().to_string(), *requirement_ordinal));
                }
            }

            let mut added = false;
            for requirement in requirements {
                let key = (
                    requirement.declaring_trait.clone(),
                    requirement.requirement_ordinal,
                );
                if existing_defaults.contains(&key) || !requirement.signature.is_default {
                    continue;
                }
                let signature = if requirement.substitution.is_empty() {
                    requirement.signature
                } else {
                    instantiate_default_signature(
                        syntax,
                        &requirement.signature,
                        &requirement.substitution,
                    )
                };
                let machine = machine_from_signature(
                    syntax,
                    &conformance_name,
                    Identifier::generated(signature.name.as_str()),
                    &signature,
                    exact_unargumented_requirement_owner(&traits, &requirement.declaring_trait),
                );
                closed_members.push(ConformanceMember::TraitDefault {
                    declaring_trait: Identifier::generated(requirement.declaring_trait),
                    requirement_ordinal: requirement.requirement_ordinal,
                    machine,
                });
                existing_defaults.insert(key);
                added = true;
            }
            if added {
                replace_closed_members(
                    syntax,
                    conformance.handle,
                    conformance.declaration,
                    closed_members,
                );
            }
            continue;
        }

        let Some(type_name) = carrier_name else {
            continue;
        };

        if trait_name == "Equatable"
            && let Some(signature) = conformed_trait
                .signatures
                .iter()
                .map(|declaration| &declaration.signature)
                .find(|signature| signature.name.as_str() == "equals" && !signature.is_default)
        {
            let attached_method = (type_name.clone(), "equals".to_string());
            if !attached_methods.contains(&attached_method)
                && synthesize_equatable_machine(syntax, &type_name, signature)
            {
                attached_methods.insert(attached_method);
            }
        }
        let defaults = collect_effective_defaults(
            syntax,
            &trait_name,
            &substitution,
            &traits,
            &mut HashSet::new(),
        );
        for (method_name, candidates) in defaults {
            let attached_method = (type_name.clone(), method_name.clone());
            if attached_methods.contains(&attached_method) {
                continue;
            }
            if candidates.len() > 1 {
                if reported_conflicts.insert(attached_method) {
                    let origins = candidates
                        .iter()
                        .map(|candidate| format!("`{}`", candidate.origin))
                        .collect::<Vec<_>>()
                        .join(", ");
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` inherits conflicting default machine \
                         `{type_name}::{method_name}` from traits {origins}; write an override"
                    )));
                }
                continue;
            }
            let Some(candidate) = candidates.first() else {
                continue;
            };
            attached_methods.insert(attached_method);
            let signature = if candidate.substitution.is_empty() {
                candidate.signature.clone()
            } else {
                instantiate_default_signature(syntax, &candidate.signature, &candidate.substitution)
            };
            synthesize_default_machine(
                syntax,
                &type_name,
                &signature,
                candidate.requirement_owner.as_deref(),
            );
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Collect every exact requirement identity in the inherited closure together
/// with the generic substitution at this conformance. Closed conformances keep
/// exact `(declaring trait, requirement)` rows, so unlike bodyless attached
/// lookup this deliberately does not collapse or shadow same-leaf names.
fn collect_requirement_instances(
    syntax: &mut SyntaxTrees,
    trait_name: &str,
    substitution: &HashMap<String, TypeReferenceHandle>,
    traits: &HashMap<String, TraitDefaultsInput>,
    visiting: &mut HashSet<String>,
    seen: &mut HashSet<(String, usize)>,
    output: &mut Vec<RequirementInstance>,
) {
    if !visiting.insert(trait_name.to_string()) {
        return;
    }
    let Some(trait_definition) = traits.get(trait_name) else {
        visiting.remove(trait_name);
        return;
    };

    for declaration in &trait_definition.signatures {
        let key = (trait_name.to_string(), declaration.ordinal);
        if seen.insert(key) {
            output.push(RequirementInstance {
                declaring_trait: trait_name.to_string(),
                requirement_ordinal: declaration.ordinal,
                signature: declaration.signature.clone(),
                substitution: substitution.clone(),
            });
        }
    }

    for requirement in &trait_definition.requirements {
        let Some(required_trait) = traits.get(&requirement.name) else {
            continue;
        };
        if requirement.arguments.len() != required_trait.parameter_names.len() {
            continue;
        }
        let required_arguments = requirement
            .arguments
            .iter()
            .map(|argument| substitute_type_reference(syntax, *argument, substitution))
            .collect::<Vec<_>>();
        let required_substitution = required_trait
            .parameter_names
            .iter()
            .cloned()
            .zip(required_arguments)
            .collect::<HashMap<_, _>>();
        collect_requirement_instances(
            syntax,
            &requirement.name,
            &required_substitution,
            traits,
            visiting,
            seen,
            output,
        );
    }

    visiting.remove(trait_name);
}

fn replace_closed_members(
    syntax: &mut SyntaxTrees,
    handle: ItemHandle,
    mut conformance: ConformanceItem,
    members: Vec<ConformanceMember>,
) {
    let mut start = psi_arena::Handle::invalid();
    let mut count = 0u32;
    for member in members {
        let member = syntax.items.append_conformance_member(member);
        if count == 0 {
            start = member;
        }
        count = count
            .checked_add(1)
            .expect("conformance member span count overflow");
    }
    conformance.body = ConformanceBody::Closed {
        members: if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        },
    };
    syntax
        .items
        .replace_item(handle, Item::Conformance(conformance));
}

fn synthesize_equatable_machine(
    syntax: &mut SyntaxTrees,
    type_name: &str,
    signature: &StateSignatureNode,
) -> bool {
    let [receiver_handle, other_handle] = syntax.items.state_parameters(signature.parameters)
    else {
        return false;
    };
    let receiver = syntax.items.state_parameter(*receiver_handle);
    let other = syntax.items.state_parameter(*other_handle);
    if !receiver.is_self || other.is_self {
        return false;
    }
    let other_name = other.name.as_str().to_string();
    let snapshot = syntax.clone();
    let type_watermark = syntax.type_references.node_count();
    let mut signature = syntax.copy_state_signature_node_from(&snapshot, signature);
    for handle in syntax.type_references.self_type_nodes_from(type_watermark) {
        syntax.type_references.replace_type_reference(
            handle,
            TypeReferenceNode::Named(Identifier::generated(type_name)),
        );
    }

    let receiver = syntax.expressions.insert(ExpressionNode::SelfValue);
    let other_member = syntax
        .expressions
        .append_identifier_path_member(Identifier::generated(other_name));
    let other = syntax
        .expressions
        .insert(ExpressionNode::Name(HandleSpan::from_parts(
            other_member,
            1,
        )));
    let equality = syntax
        .expressions
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: receiver,
            operator: BinaryOperator::Equal,
            right: other,
        }));
    let statement = syntax
        .statements
        .insert(StatementNode::Expression(equality));
    let statement = syntax.items.append_statement_handle(statement);
    signature.default_body = HandleSpan::from_parts(statement, 1);

    synthesize_machine_named(
        syntax,
        type_name,
        Identifier::generated(format!(
            "__omega_synthesized_equatable::{type_name}::equals"
        )),
        &signature,
        None,
    );
    true
}

/// Compute the defaults visible from one trait. A declaration on the child,
/// including a bodyless requirement, shadows all inherited declarations of
/// the same name. Distinct inherited bodies remain separate candidates so the
/// conformance site can require a written override; repeated paths to the same
/// originating trait (a diamond) are deduplicated.
fn collect_effective_defaults(
    syntax: &mut SyntaxTrees,
    trait_name: &str,
    substitution: &HashMap<String, TypeReferenceHandle>,
    traits: &HashMap<String, TraitDefaultsInput>,
    visiting: &mut HashSet<String>,
) -> EffectiveDefaults {
    if !visiting.insert(trait_name.to_string()) {
        // Requirement-cycle validation owns the diagnostic. Avoid recursing
        // forever in this earlier desugaring pass.
        return EffectiveDefaults::new();
    }
    let Some(trait_definition) = traits.get(trait_name) else {
        visiting.remove(trait_name);
        return EffectiveDefaults::new();
    };

    let mut defaults = EffectiveDefaults::new();
    for requirement in &trait_definition.requirements {
        let Some(required_trait) = traits.get(&requirement.name) else {
            continue;
        };
        if requirement.arguments.len() != required_trait.parameter_names.len() {
            // The ordinary requirement validator owns the arity diagnostic.
            continue;
        }
        let required_arguments = requirement
            .arguments
            .iter()
            .map(|argument| substitute_type_reference(syntax, *argument, substitution))
            .collect::<Vec<_>>();
        let required_substitution = required_trait
            .parameter_names
            .iter()
            .cloned()
            .zip(required_arguments)
            .collect::<HashMap<_, _>>();
        for (method_name, candidates) in collect_effective_defaults(
            syntax,
            &requirement.name,
            &required_substitution,
            traits,
            visiting,
        ) {
            let inherited = defaults.entry(method_name).or_default();
            for candidate in candidates {
                if !inherited
                    .iter()
                    .any(|existing: &DefaultCandidate| existing.origin == candidate.origin)
                {
                    inherited.push(candidate);
                }
            }
        }
    }

    for declaration in &trait_definition.signatures {
        let signature = &declaration.signature;
        let method_name = signature.name.as_str().to_string();
        defaults.remove(&method_name);
        if signature.is_default {
            defaults.insert(
                method_name,
                vec![DefaultCandidate {
                    origin: trait_instance_label(syntax, trait_name, substitution),
                    requirement_owner: exact_unargumented_requirement_owner(traits, trait_name)
                        .map(str::to_owned),
                    signature: signature.clone(),
                    substitution: substitution.clone(),
                }],
            );
        }
    }

    visiting.remove(trait_name);
    defaults
}

fn trait_instance_label(
    syntax: &SyntaxTrees,
    trait_name: &str,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> String {
    if substitution.is_empty() {
        return trait_name.to_string();
    }
    let mut arguments = substitution
        .iter()
        .map(|(name, handle)| (name, type_reference_key(syntax, *handle)))
        .collect::<Vec<_>>();
    arguments.sort_by(|left, right| left.0.cmp(right.0));
    format!(
        "{trait_name}<{}>",
        arguments
            .into_iter()
            .map(|(_, argument)| argument)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn type_reference_key(syntax: &SyntaxTrees, handle: TypeReferenceHandle) -> String {
    match syntax.type_references.type_reference(handle) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => format!(
            "&{}{}",
            match access {
                psi_language_semantics::ReferenceAccess::Shared => "",
                psi_language_semantics::ReferenceAccess::Mutable => "mut ",
                psi_language_semantics::ReferenceAccess::WriteOnly => "write ",
            },
            type_reference_key(syntax, *referee)
        ),
        TypeReferenceNode::Constrained { base_type, .. } => type_reference_key(syntax, *base_type),
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let length = match length {
                FixedArrayLength::Literal(value) => value.to_string(),
                FixedArrayLength::ConstParameter(name) | FixedArrayLength::ConstCall(name) => {
                    name.as_str().to_string()
                }
            };
            format!("[{}; {length}]", type_reference_key(syntax, *element_type))
        }
        TypeReferenceNode::Slice { element_type } => {
            format!("[{}]", type_reference_key(syntax, *element_type))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => format!(
            "{}<{}>",
            base_name.as_str(),
            syntax
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_key(syntax, *argument))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeReferenceNode::ConstExpression(expression) => format!("const#{:?}", expression),
        TypeReferenceNode::DynamicTrait { name, conformance } => conformance
            .as_ref()
            .map(|selection| format!("dyn {}::{}", name.as_str(), selection.as_str()))
            .unwrap_or_else(|| format!("dyn {}", name.as_str())),
        TypeReferenceNode::Named(name) => name.as_str().to_string(),
        TypeReferenceNode::SelfType => "Self".to_string(),
        TypeReferenceNode::Unit => "()".to_string(),
    }
}

fn substitute_type_reference(
    syntax: &mut SyntaxTrees,
    handle: TypeReferenceHandle,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> TypeReferenceHandle {
    let node = syntax.type_references.type_reference(handle).clone();
    match node {
        TypeReferenceNode::Named(name) => {
            substitution.get(name.as_str()).copied().unwrap_or(handle)
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let substituted = substitute_type_reference(syntax, referee, substitution);
            if substituted == referee {
                handle
            } else {
                syntax
                    .type_references
                    .insert_reference_with_lifetime(substituted, access, lifetime)
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let substituted = substitute_type_reference(syntax, base_type, substitution);
            if substituted == base_type {
                handle
            } else {
                syntax
                    .type_references
                    .insert_constrained(substituted, constraints)
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let substituted_element = substitute_type_reference(syntax, element_type, substitution);
            let substituted_length = match &length {
                FixedArrayLength::ConstParameter(name) => substitution
                    .get(name.as_str())
                    .and_then(
                        |argument| match syntax.type_references.type_reference(*argument) {
                            TypeReferenceNode::Named(value) => value.as_str().parse().ok(),
                            _ => None,
                        },
                    )
                    .map(FixedArrayLength::Literal)
                    .unwrap_or_else(|| length.clone()),
                _ => length.clone(),
            };
            if substituted_element == element_type && substituted_length == length {
                handle
            } else {
                syntax
                    .type_references
                    .insert_fixed_array(substituted_element, substituted_length)
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            let substituted = substitute_type_reference(syntax, element_type, substitution);
            if substituted == element_type {
                handle
            } else {
                syntax.type_references.insert_slice(substituted)
            }
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let original = syntax
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let substituted = original
                .iter()
                .map(|argument| substitute_type_reference(syntax, *argument, substitution))
                .collect::<Vec<_>>();
            if substituted == original {
                handle
            } else {
                let arguments = syntax
                    .type_references
                    .insert_type_reference_handles(substituted);
                syntax.type_references.insert_generic(base_name, arguments)
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => handle,
    }
}

fn instantiate_default_signature(
    syntax: &mut SyntaxTrees,
    signature: &StateSignatureNode,
    substitution: &HashMap<String, TypeReferenceHandle>,
) -> StateSignatureNode {
    let snapshot = syntax.clone();
    let type_watermark = syntax.type_references.node_count();
    let copied = syntax.copy_state_signature_node_from(&snapshot, signature);
    for (handle, name) in syntax.type_references.named_nodes_from(type_watermark) {
        let Some(argument) = substitution.get(&name) else {
            continue;
        };
        let replacement = syntax.type_references.type_reference(*argument).clone();
        syntax
            .type_references
            .replace_type_reference(handle, replacement);
    }
    copied
}

fn synthesize_default_machine(
    syntax: &mut SyntaxTrees,
    type_name: &str,
    signature: &StateSignatureNode,
    requirement_owner: Option<&str>,
) {
    synthesize_machine_named(
        syntax,
        type_name,
        Identifier::generated(format!("{type_name}::{}", signature.name.as_str())),
        signature,
        requirement_owner,
    );
}

fn synthesize_machine_named(
    syntax: &mut SyntaxTrees,
    type_name: &str,
    machine_name: Identifier,
    signature: &StateSignatureNode,
    requirement_owner: Option<&str>,
) {
    let machine = machine_from_signature(
        syntax,
        type_name,
        machine_name,
        signature,
        requirement_owner,
    );
    syntax.push_root_item(Item::Machine(machine));
}

fn exact_unargumented_requirement_owner<'name>(
    traits: &HashMap<String, TraitDefaultsInput>,
    trait_name: &'name str,
) -> Option<&'name str> {
    let definition = traits.get(trait_name)?;
    (!definition.has_lifetime_parameters && definition.parameter_names.is_empty())
        .then_some(trait_name)
}

fn machine_from_signature(
    syntax: &mut SyntaxTrees,
    type_name: &str,
    machine_name: Identifier,
    signature: &StateSignatureNode,
    requirement_owner: Option<&str>,
) -> Machine {
    let state = State {
        name: signature.name.clone(),
        parameters: signature.parameters,
        return_type: signature.return_type,
        contracts: HandleSpan::empty(),
        statements: signature.default_body,
    };
    let state = syntax.items.insert_state(&state);
    let state = syntax.items.append_state_handle(state);
    let satisfies = requirement_owner.map_or_else(HandleSpan::empty, |trait_name| {
        let clause = syntax.items.append_satisfies_clause(SatisfiesClause {
            trait_name: Identifier::generated(trait_name),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
            requirement: Some(Identifier::generated(signature.name.as_str())),
            alias: None,
            via: None,
            via_expression: ExpressionHandle::invalid(),
            via_keyword_source_span: None,
        });
        HandleSpan::from_parts(clause, 1)
    });
    Machine {
        name: machine_name,
        attached_data: Some(Identifier::generated(type_name)),
        is_public: false,
        bodyless: false,
        target: None,
        boundary: false,
        is_top_level_boundary_requirement: false,
        generic_data_template: Default::default(),
        lifetime_parameters: signature.lifetime_parameters.clone(),
        type_parameters: HandleSpan::empty(),
        satisfies,
        conformance_bounds: Vec::new(),
        terminates_guarantee: signature.terminates_guarantee,
        ranking_subjects: HandleSpan::<ExpressionHandle>::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: ExpressionHandle::invalid(),
        service_reach_is_installation_bound: false,
        service_reach_keyword_source_spans: signature.service_reach_keyword_source_spans.clone(),
        service_reaches: signature.service_reaches,
        invokes: signature.invokes,
        suspends: signature.suspends,
        suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
        blocks: signature.blocks,
        blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
        contracts: signature.contracts,
        states: HandleSpan::from_parts(state, 1),
    }
}

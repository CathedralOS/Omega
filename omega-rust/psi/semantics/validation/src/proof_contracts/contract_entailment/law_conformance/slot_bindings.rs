//! Carrier slot bindings, slot application rewrites and diagnostic
//! shapes.

use crate::proof_contracts::contract_entailment::{
    Diagnostic, StructuralTerm, TraitDefinition, TypedTrees,
};

/// The CARRIER's op-slot bindings: for each requirement of the trait, the
/// machine conforming to it whose carrier type matches. Alias preference
/// (plural algebras): a binding sharing the checking conformance's alias
/// wins; otherwise unaliased bindings win; a remaining tie is ambiguous and
/// reported.
pub(crate) fn carrier_slot_bindings(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    carrier: typed_trees::types::TypeReferenceHandle,
    prefer_alias: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut bindings: Vec<(String, String)> = Vec::new();

    for requirement in program.trait_machine_signatures(trait_definition) {
        // (slot machine name, alias) candidates for this carrier.
        let mut candidates: Vec<(String, Option<String>)> = Vec::new();
        for candidate in program.machines() {
            for conformance in program.machine_trait_conformances(candidate) {
                if conformance.symbol != trait_definition.symbol {
                    continue;
                }
                let bound_requirement = conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        candidate
                            .attached_data
                            .is_none()
                            .then(|| candidate.name.as_str().to_owned())
                    });
                if bound_requirement.as_deref() != Some(requirement.name.as_str()) {
                    continue;
                }
                let Some(candidate_entry) = program.machine_states(candidate).first() else {
                    continue;
                };
                let candidate_carrier = program
                    .state_parameters(candidate_entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(candidate_entry.return_type);
                if !crate::value_custody::type_references::type_references_match(
                    program,
                    candidate_carrier,
                    carrier,
                ) {
                    continue;
                }
                candidates.push((
                    candidate.name.as_str().to_owned(),
                    conformance
                        .alias
                        .as_ref()
                        .map(|alias| alias.as_str().to_owned()),
                ));
            }
        }

        if candidates.is_empty() {
            continue; // an unbound slot only matters if a law mentions it
        }
        let chosen = if let Some(preferred) = candidates
            .iter()
            .filter(|(_, alias)| alias.as_deref() == prefer_alias)
            .collect::<Vec<_>>()
            .split_first()
            .filter(|(_, rest)| rest.is_empty())
            .map(|(first, _)| (*first).clone())
        {
            Some(preferred)
        } else {
            let unaliased: Vec<_> = candidates
                .iter()
                .filter(|(_, alias)| alias.is_none())
                .collect();
            match unaliased.as_slice() {
                [single] => Some((*single).clone()),
                [] if candidates.len() == 1 => Some(candidates[0].clone()),
                [] => None,
                _ => None,
            }
        };
        match chosen {
            Some((machine_name, _)) => {
                bindings.push((requirement.name.as_str().to_owned(), machine_name));
            }
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requirement `{}` has AMBIGUOUS satisfiers for this carrier -- \
                     name the family with `as <Alias>` on each conformance so the law check \
                     (and the judge) can pick one",
                    trait_definition.name, requirement.name,
                )));
            }
        }
    }

    bindings
}

/// Rewrite the law's op-slot applications (`add(a, b)` where `add` is a
/// requirement of the SAME trait) to the carrier's bound machine names;
/// slots with no binding are collected for the missing-slot diagnostic.
pub(crate) fn rewrite_slot_applications(
    term: &StructuralTerm,
    slot_names: &[String],
    slot_bindings: &[(String, String)],
    missing: &mut Vec<String>,
) -> StructuralTerm {
    match term {
        StructuralTerm::Application { machine, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    rewrite_slot_applications(argument, slot_names, slot_bindings, missing)
                })
                .collect();
            let machine = if slot_names.iter().any(|slot| slot == machine) {
                match slot_bindings
                    .iter()
                    .find(|(slot, _)| slot == machine)
                    .map(|(_, bound)| bound.clone())
                {
                    Some(bound) => bound,
                    None => {
                        missing.push(machine.clone());
                        machine.clone()
                    }
                }
            } else {
                machine.clone()
            };
            StructuralTerm::Application { machine, arguments }
        }
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data: data.clone(),
            case: case.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        rewrite_slot_applications(value, slot_names, slot_bindings, missing),
                    )
                })
                .collect(),
        },
        StructuralTerm::CallProjection {
            target,
            machine,
            result_type,
            field,
            field_name,
            arguments,
        } => StructuralTerm::CallProjection {
            target: *target,
            machine: machine.clone(),
            result_type: *result_type,
            field: *field,
            field_name: field_name.clone(),
            arguments: arguments
                .iter()
                .map(|argument| {
                    rewrite_slot_applications(argument, slot_names, slot_bindings, missing)
                })
                .collect(),
        },
        other => other.clone(),
    }
}

pub(crate) fn term_mentions_variable(term: &StructuralTerm, variable: &String) -> bool {
    match term {
        StructuralTerm::Variable(name) => name == variable,
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_mentions_variable(value, variable)),
        StructuralTerm::Application { arguments, .. } => arguments
            .iter()
            .any(|argument| term_mentions_variable(argument, variable)),
        StructuralTerm::CallProjection { arguments, .. } => arguments
            .iter()
            .any(|argument| term_mentions_variable(argument, variable)),
        StructuralTerm::Opaque(_) | StructuralTerm::Integer(_) => false,
    }
}

/// First-order matching for the SUGGESTION diagnostic only (the proving
/// path never pattern-matches -- citations instantiate at written
/// operands): occurrences of `variables` in `pattern` bind consistently
/// against the goal's subterms; everything else must agree exactly.
pub(crate) fn diagnostic_shape_match(
    pattern: &StructuralTerm,
    term: &StructuralTerm,
    variables: &[String],
    bindings: &mut Vec<(String, StructuralTerm)>,
) -> bool {
    match (pattern, term) {
        (StructuralTerm::Variable(name), _) if variables.iter().any(|v| v == name) => {
            if let Some((_, bound)) = bindings.iter().find(|(n, _)| n == name) {
                bound == term
            } else {
                bindings.push((name.clone(), term.clone()));
                true
            }
        }
        (StructuralTerm::Variable(left), StructuralTerm::Variable(right)) => left == right,
        (
            StructuralTerm::Constructor { data, case, fields },
            StructuralTerm::Constructor {
                data: data_t,
                case: case_t,
                fields: fields_t,
            },
        ) => {
            data == data_t
                && case == case_t
                && fields.len() == fields_t.len()
                && fields
                    .iter()
                    .zip(fields_t)
                    .all(|((name, value), (name_t, value_t))| {
                        name == name_t
                            && diagnostic_shape_match(value, value_t, variables, bindings)
                    })
        }
        (
            StructuralTerm::Application { machine, arguments },
            StructuralTerm::Application {
                machine: machine_t,
                arguments: arguments_t,
            },
        ) => {
            machine == machine_t
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(argument, argument_t, variables, bindings)
                    })
        }
        (StructuralTerm::Opaque(left), StructuralTerm::Opaque(right)) => left == right,
        (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) => left == right,
        (
            StructuralTerm::CallProjection {
                target,
                machine,
                result_type,
                field,
                arguments,
                ..
            },
            StructuralTerm::CallProjection {
                target: target_t,
                machine: machine_t,
                result_type: result_type_t,
                field: field_t,
                arguments: arguments_t,
                ..
            },
        ) => {
            target == target_t
                && machine == machine_t
                && result_type == result_type_t
                && field == field_t
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(argument, argument_t, variables, bindings)
                    })
        }
        _ => false,
    }
}

/// Render a term back into citation-argument spelling.
pub(crate) fn display_structural_term(term: &StructuralTerm) -> String {
    match term {
        StructuralTerm::Variable(name) => name.clone(),
        StructuralTerm::Integer(value) => value.to_string(),
        StructuralTerm::Constructor { data, case, fields } => {
            if fields.is_empty() {
                format!("{data}::{case}")
            } else {
                let rendered: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", display_structural_term(value)))
                    .collect();
                format!("{data}::{case} {{ {} }}", rendered.join(", "))
            }
        }
        StructuralTerm::Application { machine, arguments } => {
            let rendered: Vec<String> = arguments.iter().map(display_structural_term).collect();
            format!("{machine}({})", rendered.join(", "))
        }
        StructuralTerm::CallProjection {
            machine,
            field_name,
            arguments,
            ..
        } => {
            let rendered: Vec<String> = arguments.iter().map(display_structural_term).collect();
            format!("{machine}({}).{field_name}", rendered.join(", "))
        }
        StructuralTerm::Opaque(display) => display.clone(),
    }
}

//! Carrier slot bindings, slot application rewrites and diagnostic
//! shapes.

use crate::proof_contracts::contract_entailment::{Diagnostic, StructuralTerm, TraitDefinition};

/// Exact declaration slots of one trait application. A same-spelled callable
/// outside this roster remains a different operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraitOperationSlot {
    pub(crate) requirement: symbols::SymbolHandle,
    application: typed_trees::type_identity::NormalizedTypeIdentity,
}

pub(crate) struct TraitOperationSlots {
    declaring_scopes: Vec<(
        symbols::SymbolHandle,
        Option<typed_trees::type_identity::NormalizedTypeIdentity>,
    )>,
}

impl TraitOperationSlots {
    pub(crate) fn new<'program>(
        program: &'program dyn crate::CheckingInput,
        declaration: &'program TraitDefinition,
        application: Option<&typed_trees::trait_definition::AppliedTraitScope<'program>>,
    ) -> Self {
        let abstract_scope =
            typed_trees::trait_definition::AppliedTraitScope::abstract_scope(declaration);
        let application = application.unwrap_or(&abstract_scope);
        let mut declaring_scopes = Vec::new();
        application.visit(program.typed_program(), &mut |scope| {
            declaring_scopes.push((
                scope.declaration.symbol,
                scope.application_identity(program.typed_program()),
            ));
        });
        Self { declaring_scopes }
    }

    pub(crate) fn resolve<'input>(
        &self,
        program: &'input dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(
        TraitOperationSlot,
        &'input typed_trees::signature::StateSignature,
    )> {
        let (owner, template, application) =
            if let Some(closed) = program.closed_requirement_application(target) {
                self.declaring_scopes.iter().find(|(owner, identity)| {
                    *owner == closed.declaring_trait
                        && *identity == Some(closed.trait_application_identity)
                })?;
                (
                    closed.declaring_trait,
                    closed.template,
                    closed.trait_application_identity,
                )
            } else {
                if !target.is_valid() {
                    return None;
                }
                let owner = program.symbols.get(target).parent;
                let mut scopes = self
                    .declaring_scopes
                    .iter()
                    .filter(|(candidate, _)| *candidate == owner);
                let application = scopes.next()?.1?;
                if scopes.any(|(_, identity)| *identity != Some(application)) {
                    return None;
                }
                (owner, target, application)
            };
        let declaration = program.trait_definition_by_symbol(owner)?;
        let requirement = program.trait_machine_signature_by_symbol(declaration, template)?;
        Some((
            TraitOperationSlot {
                requirement: requirement.symbol,
                application,
            },
            requirement,
        ))
    }

    pub(crate) fn covers_term(
        &self,
        program: &dyn crate::CheckingInput,
        term: &StructuralTerm,
    ) -> bool {
        let term = term.semantic_value();
        if let StructuralTerm::Application {
            target, machine, ..
        } = term
            && !self
                .resolve(program, *target)
                .is_some_and(|(_, slot)| slot.name.as_str() == machine)
        {
            return false;
        }
        !term.any_child(|child| !self.covers_term(program, child))
    }
}

/// Select an existing row under the same source family preference everywhere.
/// A family label narrows candidates; only the selected row supplies identity.
pub(crate) fn select_slot_candidate<'row, T>(
    candidates: &'row [T],
    preferred_alias: Option<&str>,
    alias: impl Fn(&T) -> Option<&str>,
) -> Option<&'row T> {
    let mut preferred = candidates
        .iter()
        .filter(|candidate| alias(candidate) == preferred_alias);
    if let Some(first) = preferred.next()
        && preferred.next().is_none()
    {
        return Some(first);
    }
    let mut unaliased = candidates
        .iter()
        .filter(|candidate| alias(candidate).is_none());
    match (unaliased.next(), unaliased.next(), candidates) {
        (Some(single), None, _) => Some(single),
        (None, _, [single]) => Some(single),
        _ => None,
    }
}

/// The CARRIER's op-slot bindings: for each used requirement of the trait, the
/// machine conforming to it whose carrier type matches. Alias preference
/// (plural algebras): a binding sharing the checking conformance's alias
/// wins; otherwise unaliased bindings win; a remaining tie is ambiguous and
/// reported.
pub(crate) fn carrier_slot_bindings(
    program: &dyn crate::CheckingInput,
    slots: &TraitOperationSlots,
    carrier: typed_trees::types::TypeReferenceHandle,
    prefer_alias: Option<&str>,
    required_slots: &[TraitOperationSlot],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(TraitOperationSlot, (String, symbols::SymbolHandle))> {
    let mut bindings = Vec::new();

    for (trait_symbol, application_identity) in &slots.declaring_scopes {
        let Some(trait_definition) = program.trait_definition_by_symbol(*trait_symbol) else {
            continue;
        };
        for requirement in program.trait_machine_signatures(trait_definition) {
            let Some(application) = *application_identity else {
                continue;
            };
            let slot = TraitOperationSlot {
                requirement: requirement.symbol,
                application,
            };
            if !required_slots.contains(&slot) {
                continue;
            }
            // (slot machine name, alias) candidates for this carrier.
            let mut candidates: Vec<((String, symbols::SymbolHandle), Option<String>)> = Vec::new();
            for candidate in program.machines() {
                for conformance in program.machine_trait_conformances(candidate) {
                    if conformance.symbol != trait_definition.symbol
                        || conformance.requirement_symbol != requirement.symbol
                    {
                        continue;
                    }
                    let selected_application =
                        typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                            program.typed_program(),
                            candidate,
                            conformance,
                        )
                        .and_then(|scope| scope.application_identity(program.typed_program()));
                    if selected_application.is_none()
                        || selected_application != *application_identity
                    {
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
                        (candidate.name.as_str().to_owned(), candidate_entry.symbol),
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
            let chosen =
                select_slot_candidate(&candidates, prefer_alias, |(_, alias)| alias.as_deref());
            match chosen {
                Some((machine_name, _)) => {
                    bindings.push((slot, machine_name.clone()));
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
    }
    bindings
}

/// Rewrite the law's op-slot applications (`add(a, b)` where `add` is a
/// requirement of the SAME trait) to the carrier's bound machine names;
/// slots with no binding are collected for the missing-slot diagnostic.
pub(crate) fn rewrite_slot_applications(
    program: &dyn crate::CheckingInput,
    term: &StructuralTerm,
    slots: &TraitOperationSlots,
    slot_bindings: &[(TraitOperationSlot, (String, symbols::SymbolHandle))],
    missing: &mut Vec<TraitOperationSlot>,
) -> StructuralTerm {
    let rewrite = |value: StructuralTerm| {
        rewrite_slot_applications(program, &value, slots, slot_bindings, missing)
    };
    match term {
        StructuralTerm::Application {
            result_type,
            target,
            selections,
            machine,
            arguments,
        } => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    rewrite_slot_applications(program, argument, slots, slot_bindings, missing)
                })
                .collect();
            let mut result_type = *result_type;
            let (machine, target) = if let Some((slot, _)) = slots.resolve(program, *target) {
                match slot_bindings
                    .iter()
                    .find(|(requirement, _)| *requirement == slot)
                    .map(|(_, bound)| bound.clone())
                {
                    Some(bound) => {
                        // Selection replaces the complete callable carrier,
                        // not just its name. A generic requirement result is
                        // not the selected concrete operation's return type.
                        let Some(entry) = program.state_by_symbol(bound.1) else {
                            missing.push(slot);
                            return term.clone();
                        };
                        result_type = entry.return_type;
                        bound
                    }
                    None => {
                        missing.push(slot);
                        (machine.clone(), *target)
                    }
                }
            } else {
                (machine.clone(), *target)
            };
            StructuralTerm::Application {
                result_type,
                target,
                selections: selections.clone(),
                machine,
                arguments,
            }
        }
        // Every other variant, including a projection call's arguments, walks
        // the shared child table; a slot binding only ever replaces an
        // application's callable, so the rest of the term keeps its shape.
        other => other.clone().map_children(rewrite),
    }
}

pub(crate) fn term_mentions_variable(term: &StructuralTerm, variable: &String) -> bool {
    let term = term.semantic_value();
    matches!(term, StructuralTerm::Variable(name) if name == variable)
        || term.any_child(|child| term_mentions_variable(child, variable))
}

/// First-order correspondence for law conformance and advisory citations.
/// Occurrences of `variables` bind consistently against the goal's subterms;
/// consumers separately require universal binder correspondence and proved
/// source guarantees. Diagnostic text cannot supply a missing term identity.
pub(crate) fn diagnostic_shape_match(
    program: &dyn crate::CheckingInput,
    pattern: &StructuralTerm,
    term: &StructuralTerm,
    variables: &[String],
    bindings: &mut Vec<(String, StructuralTerm)>,
) -> bool {
    match (pattern.semantic_value(), term.semantic_value()) {
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
            StructuralTerm::Projection { subject, path },
            StructuralTerm::Projection {
                subject: actual_subject,
                path: actual_path,
            },
        ) => {
            path == actual_path
                && diagnostic_shape_match(program, subject, actual_subject, variables, bindings)
        }
        (
            StructuralTerm::CollectionMeasure {
                measure,
                collection,
            },
            StructuralTerm::CollectionMeasure {
                measure: actual_measure,
                collection: actual_collection,
            },
        ) => {
            measure == actual_measure
                && diagnostic_shape_match(
                    program,
                    collection,
                    actual_collection,
                    variables,
                    bindings,
                )
        }
        (
            StructuralTerm::Constructor {
                identity, fields, ..
            },
            StructuralTerm::Constructor {
                identity: identity_t,
                fields: fields_t,
                ..
            },
        ) => {
            identity == identity_t
                && fields.len() == fields_t.len()
                && fields
                    .iter()
                    .zip(fields_t)
                    .all(|((name, value), (name_t, value_t))| {
                        name == name_t
                            && diagnostic_shape_match(program, value, value_t, variables, bindings)
                    })
        }
        (
            StructuralTerm::Application {
                target,
                selections,
                result_type,
                arguments,
                ..
            },
            StructuralTerm::Application {
                target: target_t,
                selections: selections_t,
                result_type: result_type_t,
                arguments: arguments_t,
                ..
            },
        ) => {
            target.is_valid()
                && target == target_t
                && selections == selections_t
                && result_type.is_valid()
                && result_type_t.is_valid()
                && program.normalized_type_identity(*result_type)
                    == program.normalized_type_identity(*result_type_t)
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(program, argument, argument_t, variables, bindings)
                    })
        }
        // An unsupported projection may hide its result binder and selected
        // operator scope. Equal display text is not correspondence between a
        // requirement's law and the provider's independently proved contract.
        (StructuralTerm::Opaque(_), StructuralTerm::Opaque(_)) => false,
        (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) => left == right,
        (
            StructuralTerm::CallProjection {
                target,
                selections,
                machine,
                result_type,
                field,
                arguments,
                ..
            },
            StructuralTerm::CallProjection {
                target: target_t,
                selections: selections_t,
                machine: machine_t,
                result_type: result_type_t,
                field: field_t,
                arguments: arguments_t,
                ..
            },
        ) => {
            target.is_valid()
                && target == target_t
                && selections == selections_t
                && machine == machine_t
                && result_type == result_type_t
                && field == field_t
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(program, argument, argument_t, variables, bindings)
                    })
        }
        _ => false,
    }
}

/// Render a term back into citation-argument spelling.
pub(crate) fn display_structural_term(term: &StructuralTerm) -> String {
    match term.semantic_value() {
        StructuralTerm::RepeatedArray { length, element } => match element {
            Some(element) => format!("[{}; {length}]", display_structural_term(element)),
            None => "[]".to_owned(),
        },
        StructuralTerm::CollectionMeasure {
            measure,
            collection,
        } => format!(
            "{}.{}",
            display_structural_term(collection),
            measure.authored_spelling(),
        ),
        StructuralTerm::ObservedValue { .. } => unreachable!("semantic_value strips observations"),
        StructuralTerm::Variable(name) => name.clone(),
        StructuralTerm::BoundValue(symbol) => format!("<completed value {symbol:?}>"),
        StructuralTerm::CapturedValue {
            expression,
            callable,
            ..
        } => format!("<completed value at {expression:?}, callable {callable:?}>"),
        StructuralTerm::BoundProjection { subject, path } => {
            format!("<completed value {subject:?}>.{path}")
        }
        StructuralTerm::ScalarRepresentation {
            source,
            target,
            value,
        } => format!(
            "representation<{}, {}>({})",
            source.name(),
            target.name(),
            display_structural_term(value)
        ),
        StructuralTerm::Integer(value) => value.to_string(),
        StructuralTerm::IntegerEmbedding { value, .. } => {
            format!("embed({})", display_structural_term(value))
        }
        StructuralTerm::MathematicalBinary {
            operator,
            left,
            right,
        } => format!(
            "math:{operator:?}({},{})",
            display_structural_term(left),
            display_structural_term(right)
        ),
        StructuralTerm::Float(value) => value.text().to_owned(),
        StructuralTerm::Constructor {
            data, case, fields, ..
        } => {
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
        StructuralTerm::Application {
            machine, arguments, ..
        } => {
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
        StructuralTerm::ScalarBinary {
            operator,
            meaning,
            left,
            right,
            ..
        } => format!(
            "binary:{operator:?}:{meaning:?}({},{})",
            display_structural_term(left),
            display_structural_term(right)
        ),
        StructuralTerm::IndexedValue {
            collection, index, ..
        } => format!(
            "{}[{}]",
            display_structural_term(collection),
            display_structural_term(index)
        ),
        StructuralTerm::Projection { subject, path } => {
            format!("{}.{path}", display_structural_term(subject))
        }
        StructuralTerm::Identity {
            carrier,
            left,
            right,
        } => format!(
            "identity:{carrier:?}({},{})",
            display_structural_term(left),
            display_structural_term(right)
        ),
        StructuralTerm::ContentValue { algebra, value } => {
            format!("canonical:{algebra:?}({})", display_structural_term(value))
        }
        StructuralTerm::Opaque(display) => display.clone(),
    }
}

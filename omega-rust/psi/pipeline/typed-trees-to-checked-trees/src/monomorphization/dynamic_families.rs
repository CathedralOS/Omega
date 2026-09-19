//! Dynamic-selection family generation.
//!
//! A local `dyn` selection whose closed conformance realizes a finite generic
//! requirement must own one checked specialization of the row's realization
//! template per declared roster tuple. Boundary adapter rows settle against
//! the same commitment: a selected provider's complete family is generated
//! here — before ordinary specialization and checked-fact construction — from
//! the one roster authority, rather than the settle boundary discovering a
//! tuple only when a static call site happened to demand it. Either way the
//! conformance's tuple rows are real checked evidence rather than an external
//! promise.
//!
//! The conformance row stays requirement-level (`realization_machine` names
//! the provider template): `MachineSpecialization.const_argument_identities`
//! carries each exact tuple, the same canonical coordinate the selected
//! dispatch boundary joins on. Missing or shape-substituted coverage is a
//! program error — a requirement declares the roster and one selected
//! conformance must cover every tuple.

use super::body_cloning::clone_specialized_machine;
use super::candidate;
use super::const_arguments;
use super::identities::{
    accepted_template_commitment, canonical_template_contract_bytes, fnv1a_report_fingerprint,
    machine_template_commitment,
};
use super::normalized_machine_identity;
use super::selection::{approved_type_bounds, validate_candidate_conformance_bounds};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::finite_family::{FamilyProbe, FamilyTuple};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Generate every missing tuple specialization demanded by the program's
/// dynamic conformance selections and by each selected boundary adapter row
/// whose requirement declares a finite family. Returns the number of
/// specialization instances materialized this pass; callers re-run the
/// ordinary specialization round when it is nonzero because a generated body
/// may itself select generic callees or further dynamic families.
///
/// Selection collection is a pure typed query: an invalid program returns its
/// diagnostics from `validate_typed_program` authoritatively, so a failed
/// collection here simply defers generation rather than double-reporting.
pub(crate) fn generate_dynamic_family_specializations(
    program: &mut TypedTrees,
    boundary_families: &[crate::SelectedBoundaryFamilySpecialization],
) -> Result<usize, Vec<Diagnostic>> {
    // Collect the complete demand set before mutating the graph: (realization
    // template symbol, roster tuples). One selected conformance covers the
    // roster alone — the same row reused by several selections generates once,
    // and a boundary demand and a `dyn` selection naming one template share
    // its union.
    let mut diagnostics = Vec::new();
    let mut demands: Vec<(SymbolHandle, Vec<FamilyTuple>)> = Vec::new();

    // A selected boundary adapter row commits its provider to the complete
    // roster before any call resolves. Demand collection upstream already
    // keeps only finite rosters on conforming, coverable providers, so the
    // `NotFinite` arm below is pure defense; an unresolvable signature or
    // template here is orchestration drift and rejects.
    for family in boundary_families {
        let Some(requirement) = program
            .traits()
            .iter()
            .flat_map(|definition| program.trait_machine_signatures(definition).iter())
            .find(|signature| signature.symbol == family.requirement_signature)
        else {
            diagnostics.push(Diagnostic::error(
                "selected boundary family demand names no requirement signature",
            ));
            continue;
        };
        let FamilyProbe::Finite { arity, tuples } = program.finite_signature_family(requirement)
        else {
            continue;
        };
        let Some(realization) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == family.realization_machine)
        else {
            diagnostics.push(Diagnostic::error(
                "selected boundary family demand names no realization machine template",
            ));
            continue;
        };
        let realization_binders = program.machine_type_parameters(realization);
        let value_binders = realization_binders
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Const { .. }
                        | typed_trees::data::TypeParameterKind::Value { .. }
                )
            })
            .count();
        if realization_binders.len() != value_binders || value_binders != arity {
            diagnostics.push(Diagnostic::error(format!(
                "selected boundary provider `{}` cannot cover the declared finite family of `{}`: a family provider must be generic over exactly the requirement's {arity} const/value binders",
                realization.name, requirement.name
            )));
            continue;
        }
        queue_family_demand(&mut demands, realization.symbol, tuples);
    }

    // A failed dynamic-selection collection defers that generation to
    // `validate_typed_program`'s authoritative diagnostics, as documented
    // above; boundary demands collected already still apply.
    if let Ok(selections) = validation::collect_dynamic_conformance_selections(program)
        && !selections.is_empty()
    {
        collect_selection_demands(program, &selections, &mut demands, &mut diagnostics);
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if demands.is_empty() {
        return Ok(0);
    }

    let operational = validation::infer_operational_may(program);
    let service_reaches = validation::infer_service_reaches(program, &operational);
    let mut generated = 0;
    for (template_symbol, tuples) in demands {
        let Some(machine_index) = program
            .machines()
            .iter()
            .position(|machine| machine.symbol == template_symbol)
        else {
            return Err(vec![Diagnostic::error(
                "dynamic family realization lost its machine template",
            )]);
        };
        for tuple in &tuples {
            if tuple_specialization_exists(program, template_symbol, tuple) {
                continue;
            }
            generate_tuple_specialization(program, machine_index, tuple, &service_reaches)?;
            generated += 1;
        }
    }
    Ok(generated)
}

/// Queue `template`'s roster `tuples` for generation, merging into an
/// existing demand for the same template without duplicating a tuple.
fn queue_family_demand(
    demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
    template: SymbolHandle,
    tuples: Vec<FamilyTuple>,
) {
    match demands
        .iter_mut()
        .find(|(existing, _)| *existing == template)
    {
        Some((_, existing)) => {
            for tuple in tuples {
                if !existing
                    .iter()
                    .any(|candidate| candidate.identities.as_ref() == tuple.identities.as_ref())
                {
                    existing.push(tuple);
                }
            }
        }
        None => demands.push((template, tuples)),
    }
}

/// Collect (realization template, roster tuples) demands from the program's
/// dynamic conformance selections and descriptor storages.
fn collect_selection_demands(
    program: &TypedTrees,
    selections: &[validation::DynamicConformanceSelection],
    demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let storages = validation::collect_dynamic_descriptor_storages(program, selections);
    let mut seen_conformances: Vec<SymbolHandle> = Vec::new();
    let all_selections = selections
        .iter()
        .cloned()
        .chain(storages.iter().map(|storage| storage.selection.clone()));
    for selection in all_selections {
        let Some(conformance) = selected_data_conformance(program, &selection) else {
            continue;
        };
        if seen_conformances.contains(&conformance.symbol) {
            continue;
        }
        seen_conformances.push(conformance.symbol);
        let Some(rows) = program.closed_conformance_rows(conformance) else {
            continue;
        };
        for row in rows {
            let Some((_, requirement)) = program
                .traits()
                .iter()
                .flat_map(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .map(move |signature| (definition, signature))
                })
                .find(|(definition, signature)| {
                    definition.symbol == row.declaring_trait && signature.symbol == row.requirement
                })
            else {
                continue;
            };
            let FamilyProbe::Finite { arity, tuples } =
                program.finite_signature_family(requirement)
            else {
                // A non-finite generic requirement stays dynamically
                // ineligible per requirement; the conformance's other rows
                // still settle.
                continue;
            };
            let Some(realization) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == row.realization_machine)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic conformance row `{}::{}` has no exact realization machine",
                    row.declaring_trait_name, row.requirement_name
                )));
                continue;
            };
            let realization_binders = program.machine_type_parameters(realization);
            let value_binders = realization_binders
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Const { .. }
                            | typed_trees::data::TypeParameterKind::Value { .. }
                    )
                })
                .count();
            if realization_binders.len() != value_binders || value_binders != arity {
                diagnostics.push(Diagnostic::error(format!(
                    "dynamic conformance row `{}::{}` realization `{}` cannot cover the declared finite family: a family provider must be generic over exactly the requirement's {arity} const/value binders",
                    row.declaring_trait_name,
                    row.requirement_name,
                    realization.name
                )));
                continue;
            }
            queue_family_demand(demands, realization.symbol, tuples);
        }
    }
}

/// Whether a bare value-tuple specialization of `template` already exists for
/// `tuple`: retained const identities equal the tuple exactly and every
/// non-value argument coordinate stays empty, matching the boundary's roster
/// join.
fn tuple_specialization_exists(
    program: &TypedTrees,
    template: SymbolHandle,
    tuple: &FamilyTuple,
) -> bool {
    program
        .machine_specializations
        .iter()
        .any(|specialization| {
            specialization.template == template
                && specialization.const_argument_identities.as_slice() == tuple.identities.as_ref()
                && specialization.type_argument_identities.is_empty()
                && specialization.machine_arguments.is_empty()
                && specialization.conformance_arguments.is_empty()
                && specialization.inferred_conformance_arguments.is_empty()
                && specialization.conformance_applications.is_empty()
        })
}

/// Materialize one roster tuple's specialization of a value-binder template.
/// The tuple's canonical const identities become the candidate's closed const
/// bindings, so the retained specialization record joins
/// `const_argument_identities == tuple` exactly.
fn generate_tuple_specialization(
    program: &mut TypedTrees,
    machine_index: usize,
    tuple: &FamilyTuple,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut candidate = candidate::from_machine(program, machine_index);
    if !candidate.template.machine_parameters.is_empty()
        || !candidate.template.evidence_parameters.is_empty()
    {
        return Err(vec![Diagnostic::error(format!(
            "dynamic family provider `{}` carries machine or conformance binders, which a bare value tuple cannot close",
            candidate.template.template_name
        ))]);
    }
    for (binding, identity) in candidate.const_bindings.iter_mut().zip(&tuple.identities) {
        let Some(type_reference) = const_identity_type_reference(program, identity) else {
            return Err(vec![Diagnostic::error(format!(
                "dynamic family tuple `({})` has a canonical value with no closed const carrier",
                tuple.display.join(", ")
            ))]);
        };
        *binding = Some(type_reference);
    }
    if approved_type_bounds(program, std::slice::from_ref(&candidate)) != [true] {
        return Err(vec![Diagnostic::error(format!(
            "dynamic family specialization of `{}` for tuple `({})` does not satisfy its authored type bounds",
            candidate.template.template_name,
            tuple.display.join(", ")
        ))]);
    }
    validate_candidate_conformance_bounds(program, &mut candidate)?;
    const_arguments::validate_bindings(program, &candidate).map_err(|error| vec![error])?;

    let canonical_template_contract_bytes = canonical_template_contract_bytes(
        program,
        candidate.template.machine_index,
        service_reaches,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let template_contract_report_fingerprint =
        fnv1a_report_fingerprint(&canonical_template_contract_bytes);
    let template_contract_commitment =
        machine_template_commitment(&canonical_template_contract_bytes);
    let normalized_template_identity = normalized_machine_identity(
        program,
        &program.machines()[candidate.template.machine_index],
    )
    .expect("generic template must retain a normalized callable identity");
    let accepted_template_commitment =
        accepted_template_commitment(program, candidate.template.machine_index);
    let ordinal = program
        .machine_specializations
        .iter()
        .filter(|instance| instance.template == candidate.template.template_symbol)
        .count();
    clone_specialized_machine(
        None,
        program,
        &candidate,
        ordinal,
        template_contract_report_fingerprint,
        template_contract_commitment,
        canonical_template_contract_bytes,
        normalized_template_identity,
        accepted_template_commitment,
    )
    .map_err(|error| vec![error])?;
    Ok(())
}

/// The anonymous named type reference one canonical const identity binds to:
/// `named(integer-const(16))` spells `16`, and a canonical Boolean encodes the
/// `true`/`false` literal reference. The reference is interned on demand so a
/// dynamic selection needs no static call site to produce its family.
fn const_identity_type_reference(
    program: &mut TypedTrees,
    identity: &str,
) -> Option<TypeReferenceHandle> {
    let spelling = const_identity_spelling(identity)?;
    let existing = program
        .type_reference_table
        .named_references()
        .find(|(_, symbol, name)| !symbol.is_valid() && *name == spelling)
        .map(|(handle, _, _)| handle);
    if let Some(handle) = existing {
        return Some(handle);
    }
    Some(
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: typed_trees::name::Identifier::generated(spelling),
            }),
    )
}

/// Decode one canonical specialization const identity back to its literal
/// type-reference spelling.
fn const_identity_spelling(identity: &str) -> Option<String> {
    if let Some(value) = identity
        .strip_prefix("named(integer-const(")
        .and_then(|inner| inner.strip_suffix("))"))
    {
        return value.parse::<i128>().ok().map(|value| value.to_string());
    }
    let encoded = identity
        .strip_prefix("named(canonical-const(type(")
        .and_then(|inner| inner.strip_suffix(")))"))?;
    let (type_name, encoding) = encoded.split_once("),encoding(")?;
    let value = language_semantics::const_value::CanonicalConstValue::new(
        unescape_identity_component(type_name),
        unescape_identity_component(encoding),
        "",
    );
    match value.decode_encoding()? {
        language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } => {
            Some(value.to_string())
        }
        language_semantics::const_value::DecodedCanonicalConstValue::Boolean(value) => {
            // The identity visitor re-encodes the canonical atom; a bare
            // `true`/`false` display would spell `named(name(true))` instead.
            Some(language_semantics::const_value::CanonicalConstValue::boolean(value).atom())
        }
        _ => None,
    }
}

fn unescape_identity_component(component: &str) -> String {
    let mut out = String::with_capacity(component.len());
    let mut escaped = false;
    for character in component.chars() {
        if escaped {
            out.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            out.push(character);
        }
    }
    out
}

/// The conformance one dynamic selection binds, mirroring the checked-fact
/// producer's rule: an exact symbol when the coercion named it, otherwise the
/// sole nominal closed conformance for the source's data and the target trait.
fn selected_data_conformance<'program>(
    program: &'program TypedTrees,
    selection: &validation::DynamicConformanceSelection,
) -> Option<&'program typed_trees::trait_definition::Conformance> {
    if let Some(symbol) = selection.conformance {
        return program
            .conformances()
            .iter()
            .find(|conformance| conformance.symbol == symbol);
    }
    let source_name = program.symbols.name(selection.source_data);
    let trait_name = program.symbols.name(selection.target_trait);
    let mut matches = program.conformances().iter().filter(|conformance| {
        conformance
            .carrier_name()
            .is_some_and(|carrier| carrier.as_str() == source_name)
            && conformance.trait_name.as_str() == trait_name
            && conformance.alias.is_none()
    });
    let selected = matches.next()?;
    matches.next().is_none().then_some(selected)
}

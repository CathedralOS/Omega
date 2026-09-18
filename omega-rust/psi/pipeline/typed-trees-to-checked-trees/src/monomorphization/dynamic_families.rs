//! Dynamic-selection family generation.
//!
//! A local `dyn` selection whose closed conformance realizes a finite generic
//! requirement must own one checked specialization of the row's realization
//! template per declared roster tuple. Boundary adapter rows discover those
//! specializations only when a static call site demanded them; a dynamic
//! selection instead generates the complete family here, before ordinary
//! specialization and checked-fact construction, so the conformance's tuple
//! rows are real checked evidence rather than an external promise.
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
/// dynamic conformance selections. Returns the number of specialization
/// instances materialized this pass; callers re-run the ordinary
/// specialization round when it is nonzero because a generated body may
/// itself select generic callees or further dynamic families.
///
/// Selection collection is a pure typed query: an invalid program returns its
/// diagnostics from `validate_typed_program` authoritatively, so a failed
/// collection here simply defers generation rather than double-reporting.
pub(crate) fn generate_dynamic_family_specializations(
    program: &mut TypedTrees,
) -> Result<usize, Vec<Diagnostic>> {
    let Ok(selections) = validation::collect_dynamic_conformance_selections(program) else {
        return Ok(0);
    };
    if selections.is_empty() {
        return Ok(0);
    }
    let storages = validation::collect_dynamic_descriptor_storages(program, &selections);

    // Collect the complete demand set before mutating the graph: (realization
    // template symbol, roster tuples). One selected conformance covers the
    // roster alone — the same row reused by several selections generates once.
    let mut diagnostics = Vec::new();
    let mut demands: Vec<(SymbolHandle, Vec<FamilyTuple>)> = Vec::new();
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
            match demands
                .iter_mut()
                .find(|(template, _)| *template == realization.symbol)
            {
                Some((_, existing)) => {
                    for tuple in tuples {
                        if !existing.iter().any(|candidate| {
                            candidate.identities.as_ref() == tuple.identities.as_ref()
                        }) {
                            existing.push(tuple.clone());
                        }
                    }
                }
                None => demands.push((realization.symbol, tuples)),
            }
        }
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
    if let Err(errors) = validate_candidate_conformance_bounds(program, &mut candidate) {
        return Err(errors);
    }
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

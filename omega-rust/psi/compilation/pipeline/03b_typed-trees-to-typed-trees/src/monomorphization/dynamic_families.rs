//! Dynamic-selection family generation.
//!
//! A local `dyn` selection whose closed conformance realizes a finite generic
//! requirement must own one checked specialization of the row's realization
//! template per declared roster tuple. Boundary adapter rows settle against
//! the same commitment: every checked candidate's complete family is generated
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
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::finite_family::{FamilyProbe, FamilyTuple};
use typed_trees::trait_definition::{AppliedTraitScope, Conformance};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::contract_identities::{accepted_template_commitment, normalized_machine_identity};

/// Generate every missing tuple specialization demanded by the program's
/// dynamic conformance selections and by each authored boundary conformance
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
) -> Result<usize, Vec<Diagnostic>> {
    // Stage03 prepared the original scopes. Specialization can introduce
    // another applied map before this fixed-point round; its producer must
    // prepare the complete callable view before demand inspection. The existing
    // preparation query retains prior views and appends only missing scopes.
    program
        .prepare_applied_callable_signatures()
        .map_err(|message| vec![Diagnostic::error(message)])?;
    // Collect the complete demand set before mutating the graph: (realization
    // template symbol, roster tuples). One selected conformance covers the
    // roster alone — the same row reused by several selections generates once,
    // and a boundary demand and a `dyn` selection naming one template share
    // its union.
    let mut diagnostics = Vec::new();
    let mut demands: Vec<(SymbolHandle, Vec<FamilyTuple>)> = Vec::new();

    // Source owns the candidate set. Follow exact authored satisfies edges,
    // not Build selections or similarly-shaped machines. A finite boundary
    // requirement commits every checked candidate to the whole roster.
    for realization in program.machines() {
        if !realization.supply_mode.is_checked_body()
            || !realization.body_is_present
            || program.machine_type_parameters(realization).is_empty()
        {
            continue;
        }
        for conformance in program.machine_trait_conformances(realization) {
            if conformance.external_binding.is_some() || conformance.via_expression.is_valid() {
                continue;
            }
            let Some(typed_trees::machine::SatisfiedDeclaration::Trait {
                definition,
                requirement,
            }) = typed_trees::machine::resolve_satisfied_declaration(
                program,
                realization,
                conformance,
            )
            else {
                continue;
            };
            if !definition.is_boundary {
                continue;
            }
            let FamilyProbe::Finite { arity, tuples } =
                program.finite_signature_family(requirement)
            else {
                // Open generic requirements remain ordinary static contracts;
                // no closed finite roster is invented for them.
                continue;
            };
            let binders = program.machine_type_parameters(realization);
            let value_binders = binders
                .iter()
                .filter(|parameter| parameter.kind.is_const_or_value())
                .count();
            if binders.len() != value_binders || value_binders != arity {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary provider candidate `{}` cannot cover the declared finite family of `{}`: a family provider must be generic over exactly the requirement's {arity} const/value binders",
                    realization.name, requirement.name
                )));
                continue;
            }
            queue_family_demand(&mut demands, realization.symbol, tuples);
        }
    }

    // A closed named boundary map is a candidate even when no local `dyn`
    // expression selects it. Its requirement, not the current calls or Build
    // choice, owns the whole family. Open conformance declarations still need
    // their own complete application before they can produce executable rows.
    for conformance in program.conformances() {
        if !conformance.type_parameters.is_empty()
            || !program
                .trait_definition_by_symbol(conformance.trait_symbol)
                .is_some_and(|definition| definition.is_boundary)
        {
            continue;
        }
        collect_conformance_demands(program, conformance, &mut demands, &mut diagnostics);
    }

    // A failed dynamic-selection collection defers that generation to
    // `validate_typed_program`'s authoritative diagnostics, as documented
    // above; boundary demands collected already still apply.
    if let Ok(selections) = validation::collect_dynamic_implementation_selections(program)
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

    let mut generated = 0;
    for (template_symbol, tuples) in demands {
        let Some(machine_index) = program.machine_position_by_symbol(template_symbol) else {
            return Err(vec![Diagnostic::error(
                "dynamic family realization lost its machine template",
            )]);
        };
        for tuple in &tuples {
            if tuple_specialization_exists(program, template_symbol, tuple) {
                continue;
            }
            generate_tuple_specialization(program, machine_index, tuple)?;
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
/// dynamic implementation selections. Each selection names its implementation
/// exactly, so every finite-family row it can dispatch is generated once.
fn collect_selection_demands(
    program: &dyn ::validation::CheckingInput,
    selections: &[validation::DynamicImplementationSelection],
    demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen_conformances: Vec<SymbolHandle> = Vec::new();
    for selection in selections {
        let Some(conformance) = program.conformance_by_symbol(selection.implementation) else {
            continue;
        };
        if seen_conformances.contains(&conformance.symbol) {
            continue;
        }
        seen_conformances.push(conformance.symbol);
        collect_conformance_demands(program, conformance, demands, diagnostics);
    }
}

/// Both local selection and boundary candidates read the same exact map and
/// declaring scope. Inspect its prepared applied requirement, not a raw parent
/// signature: inherited trait arguments can change the family carrier and
/// predicates. This query produces demands, never conformance admission.
fn collect_conformance_demands(
    program: &dyn ::validation::CheckingInput,
    conformance: &Conformance,
    demands: &mut Vec<(SymbolHandle, Vec<FamilyTuple>)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(application) =
        AppliedTraitScope::from_conformance(program.typed_program(), conformance)
    else {
        return;
    };
    for row in &conformance.rows {
        let Some(scope) = application.declaring_scope(program.typed_program(), row.declaring_trait)
        else {
            continue;
        };
        let Some(requirement) = program.applied_callable_signature(&scope, row.requirement) else {
            // Nongeneric rows need no prepared local telescope. An absent
            // generic application must not acquire a roster from another view.
            continue;
        };
        let FamilyProbe::Finite { arity, tuples } = program.finite_signature_family(requirement)
        else {
            continue;
        };
        let Some(realization) = program.machine_by_symbol(row.realization_machine) else {
            diagnostics.push(Diagnostic::error(format!(
                "dynamic conformance row `{}::{}` has no exact realization machine",
                row.declaring_trait_name, row.requirement_name
            )));
            continue;
        };
        let realization_binders = program.machine_type_parameters(realization);
        let value_binders = realization_binders
            .iter()
            .filter(|parameter| parameter.kind.is_const_or_value())
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

/// Whether a bare value-tuple specialization of `template` already exists for
/// `tuple`: retained const identities equal the tuple exactly and every
/// non-value argument coordinate stays empty, matching the boundary's roster
/// join.
fn tuple_specialization_exists(
    program: &dyn ::validation::CheckingInput,
    template: SymbolHandle,
    tuple: &FamilyTuple,
) -> bool {
    program
        .machine_specializations
        .iter()
        .any(|specialization| {
            specialization.template == template
                && specialization.const_argument_identities.as_slice() == tuple.identities.as_ref()
                && specialization.is_const_only()
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
        normalized_template_identity,
        accepted_template_commitment,
    )?;
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
pub(super) fn const_identity_spelling(identity: &str) -> Option<String> {
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

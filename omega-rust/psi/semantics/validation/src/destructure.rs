//! Record-pattern exhaustiveness (owner spec 2026-07-18, ch6 growth).
//!
//! `let { x, y as horizontal, z as _ } = place;` desugars at parse time to a
//! MARKER let named `__destructure#x#y#z` (spelled set: bound AND waived
//! fields) whose initializer is the place, plus one Unit-sentinel let per
//! bound field. This pass enforces the LAW half: the spelled set must equal
//! the data definition's declared field set exactly -- a missing field
//! refuses naming it (that is the point of the pattern: adding a field to
//! the data breaks every destructuring site until each decides bind-or-
//! waive), and an unknown field refuses naming it (typo fence).

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

const MARKER_PREFIX: &str = "__destructure#";
/// Arm-position twin (`..`-free destructure arms): `#`/`=` cannot appear in
/// identifiers, so the encoding splits unambiguously -- the first segment
/// after `V=` is the case variant (empty for a record arm), the rest are the
/// spelled fields.
const ARM_MARKER_PREFIX: &str = "__arm_destructure#V=";
const TRANSITION_SUBJECT_PREFIX: &str = "__transition_subject#";

pub(crate) fn validate_destructure_exhaustiveness(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let proof_only = typed_trees::proof_only::classify(program);
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if let Some(encoded) = local.name.as_str().strip_prefix(ARM_MARKER_PREFIX) {
                    validate_arm_pattern_marker(
                        program,
                        machine,
                        state,
                        local,
                        encoded,
                        &proof_only,
                        diagnostics,
                    );
                    continue;
                }
                let Some(encoded) = local.name.as_str().strip_prefix(MARKER_PREFIX) else {
                    continue;
                };
                // `#` is not legal inside authored identifiers, so each
                // marker component is exactly one spelled field even when a
                // field name contains repeated underscores.
                let spelled: Vec<&str> = encoded.split('#').collect();

                // The marker's own type resolves through hoist inference
                // (its declared type is the Unit sentinel); the DECLARED
                // place type is the honest carrier for the definition.
                let declared = crate::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    local.initial_value,
                );
                let resolved = declared.or_else(|| {
                    local
                        .type_reference
                        .is_valid()
                        .then_some(local.type_reference)
                });
                let Some(resolved) = resolved else {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern in machine `{}` state `{}` destructures a place whose declared type cannot be resolved -- the exhaustiveness law needs the data definition",
                        machine.name, state.name,
                    )));
                    continue;
                };
                let Some(data) = crate::places::data_definition_for_type(program, resolved) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern in machine `{}` state `{}` destructures a value that is not a data record -- only plain data definitions destructure",
                        machine.name, state.name,
                    )));
                    continue;
                };
                if data.quotient.is_some() {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern in machine `{}` state `{}` cannot destructure quotient `{}`: retained representatives are opaque and pattern matching requires a named lifted operation",
                        machine.name, state.name, data.name,
                    )));
                    continue;
                }

                let mut declared_fields: Vec<&str> = Vec::new();
                let mut has_variants = false;
                for member in program.data_members(data) {
                    match member {
                        typed_trees::data::DataMember::Field(field) => {
                            declared_fields.push(field.name.as_str());
                        }
                        typed_trees::data::DataMember::Variant(_) => {
                            has_variants = true;
                        }
                    }
                }
                if has_variants {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern in machine `{}` state `{}` destructures sum type `{}` -- case variants need a `case` arm, not a record pattern",
                        machine.name, state.name, data.name,
                    )));
                    continue;
                }

                for field in &declared_fields {
                    if !spelled.contains(field) {
                        diagnostics.push(Diagnostic::error(format!(
                            "record pattern in machine `{}` state `{}` does not mention field `{}` of `{}` -- record patterns are exhaustive by law: bind it, rename it with `as`, or waive it with `as _`",
                            machine.name, state.name, field, data.name,
                        )));
                    }
                }
                for field in &spelled {
                    if !declared_fields.contains(field) {
                        diagnostics.push(Diagnostic::error(format!(
                            "record pattern in machine `{}` state `{}` names `{}`, which is not a field of `{}`",
                            machine.name, state.name, field, data.name,
                        )));
                    }
                }
            }
        }
    }
}

/// The arm-position exhaustiveness law: a `..`-free destructure arm's marker
/// (`__arm_destructure#V=<variant>#<f1>...#~subject=<id>`) must spell every
/// field of the record (empty variant) or of the named case's payload. `..` in
/// the arm opts out of missing-field validation via a trailing `#~rest`.
fn validate_arm_pattern_marker(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    local: &typed_trees::statement::TableLocalData,
    encoded: &str,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut parts = encoded.split('#');
    let variant = parts.next().unwrap_or("");
    let mut spelled: Vec<&str> = parts.collect();
    // The trailing `#~rest` sentinel = the pattern spelled `..`: unknown
    // spelled fields still refuse, but the missing-field law is waived.
    let has_rest = spelled.last() == Some(&"~rest");
    if has_rest {
        spelled.pop();
    }
    // Tuple destructures can carry the same spelling on multiple subjects.
    // The parser gives each validation carrier a generated subject suffix so
    // their local names remain distinct; it is metadata, not a field.
    if spelled
        .last()
        .is_some_and(|part| part.starts_with("~subject="))
    {
        spelled.pop();
    }

    let declared =
        crate::places::declared_place_type_raw(program, machine, Some(state), local.initial_value)
            .or_else(|| {
                local
                    .type_reference
                    .is_valid()
                    .then_some(local.type_reference)
            });
    let Some(declared) = declared else {
        // The parse-time place gate admitted the subject but the declared
        // type is unresolvable here (e.g. a shape places.rs does not walk).
        // The law cannot check what it cannot see -- stay silent rather
        // than refuse working code; bound fields are still checked as
        // member reads.
        return;
    };
    let Some(data) = crate::places::data_definition_for_type(program, declared) else {
        return;
    };
    if data.quotient.is_some() {
        diagnostics.push(Diagnostic::error(format!(
            "destructure arm in machine `{}` state `{}` cannot pattern-match quotient `{}`: retained representatives are opaque and pattern matching requires a named lifted operation",
            machine.name, state.name, data.name,
        )));
        return;
    }

    // A computed subject is captured once before dispatch, then its pattern
    // fields are read independently from that local. That extraction is only
    // sound for an unrestricted (`[copy]`) RUNTIME record until affine/linear
    // destructuring gains move-aware field projection. Proof-only recursive
    // data has no runtime storage to copy and is exempt. Authored places retain
    // their existing ownership behavior; this fence is specifically for the
    // newly admitted non-place runtime face.
    if is_captured_transition_subject(program, local.initial_value)
        && !proof_only.is_proof_only(data.symbol)
        && data.properties.multiplicity != language_semantics::Multiplicity::Unrestricted
    {
        diagnostics.push(Diagnostic::error(format!(
            "destructure arm in machine `{}` state `{}` has a computed subject of non-copy data `{}` -- non-place record extraction currently requires `data {} [copy]`; otherwise bind the value to an authored typed place before the transition",
            machine.name, state.name, data.name, data.name,
        )));
        return;
    }

    let declared_fields: Vec<&str> = if variant.is_empty() {
        let mut fields = Vec::new();
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) => {
                    fields.push(field.name.as_str());
                }
                typed_trees::data::DataMember::Variant(_) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern arm in machine `{}` state `{}` destructures sum type `{}` without naming a case -- spell the variant (`{}::Case {{ .. }}`)",
                        machine.name, state.name, data.name, data.name,
                    )));
                    return;
                }
            }
        }
        fields
    } else {
        let qualified_pattern = format!("{}::{variant}", data.name);
        if program.domain_definitions().iter().any(|domain| {
            domain.name.as_str() == qualified_pattern
                && crate::type_references::type_references_match(
                    program,
                    declared,
                    domain.target_type,
                )
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "domain pattern `{qualified_pattern}` cannot bind payload fields; payload destructuring is only valid for a data case"
            )));
            return;
        }
        let Some(case) = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(case) if case.name.as_str() == variant => {
                    Some(case)
                }
                _ => None,
            })
        else {
            // An unknown case name already refuses at the tag-membership
            // guard; do not double-report here.
            return;
        };
        program
            .data_payload_fields(case)
            .iter()
            .map(|field| field.name.as_str())
            .collect()
    };

    let subject_name = if variant.is_empty() {
        data.name.to_string()
    } else {
        format!("{}::{}", data.name, variant)
    };
    if !has_rest {
        for field in &declared_fields {
            if !spelled.contains(field) {
                diagnostics.push(Diagnostic::error(format!(
                    "destructure arm in machine `{}` state `{}` does not mention field `{}` of `{}` -- record patterns are exhaustive by law: bind it, rename it with `as`, waive it with `as _`, or opt out with `..`",
                    machine.name, state.name, field, subject_name,
                )));
            }
        }
    }
    for field in &spelled {
        if !declared_fields.contains(field) {
            diagnostics.push(Diagnostic::error(format!(
                "destructure arm in machine `{}` state `{}` names `{}`, which is not a field of `{}`",
                machine.name, state.name, field, subject_name,
            )));
        }
    }
}

fn is_captured_transition_subject(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    let members = program.expression_table.name_path_members(path.members);
    members.len() == 1 && members[0].as_str().starts_with(TRANSITION_SUBJECT_PREFIX)
}

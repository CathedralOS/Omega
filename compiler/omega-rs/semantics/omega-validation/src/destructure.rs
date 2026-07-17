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

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

const MARKER_PREFIX: &str = "__destructure#";
/// Arm-position twin (`..`-free destructure arms): `#`/`=` cannot appear in
/// identifiers, so the encoding splits unambiguously -- the first segment
/// after `V=` is the case variant (empty for a record arm), the rest are the
/// spelled fields.
const ARM_MARKER_PREFIX: &str = "__arm_destructure#V=";

pub(crate) fn validate_destructure_exhaustiveness(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let omega_typed_trees::statement::StatementNode::LocalData(local) = statement
                else {
                    continue;
                };
                if let Some(encoded) = local.name.as_str().strip_prefix(ARM_MARKER_PREFIX) {
                    validate_arm_pattern_marker(
                        program,
                        machine,
                        state,
                        local,
                        encoded,
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
                let Some(data) =
                    crate::places::data_definition_for_type(program, resolved)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "record pattern in machine `{}` state `{}` destructures a value that is not a data record -- only plain data definitions destructure",
                        machine.name, state.name,
                    )));
                    continue;
                };

                let mut declared_fields: Vec<&str> = Vec::new();
                let mut has_variants = false;
                for member in program.data_members(data) {
                    match member {
                        omega_typed_trees::data::DataMember::Field(field) => {
                            declared_fields.push(field.name.as_str());
                        }
                        omega_typed_trees::data::DataMember::Variant(_) => {
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
/// (`__arm_destructure#V=<variant>#<f1>...`) must spell every field of the
/// record (empty variant) or of the named case's payload. `..` in the arm
/// opts out at parse time (no marker is minted).
fn validate_arm_pattern_marker(
    program: &TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    local: &omega_typed_trees::statement::TableLocalData,
    encoded: &str,
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

    let declared = crate::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        local.initial_value,
    )
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

    let declared_fields: Vec<&str> = if variant.is_empty() {
        let mut fields = Vec::new();
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field) => {
                    fields.push(field.name.as_str());
                }
                omega_typed_trees::data::DataMember::Variant(_) => {
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
        let Some(case) = program.data_members(data).iter().find_map(|member| {
            match member {
                omega_typed_trees::data::DataMember::Variant(case)
                    if case.name.as_str() == variant =>
                {
                    Some(case)
                }
                _ => None,
            }
        }) else {
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

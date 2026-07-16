//! Record-pattern exhaustiveness (owner spec 2026-07-18, ch6 growth).
//!
//! `let { x, y as horizontal, z as _ } = place;` desugars at parse time to a
//! MARKER let named `__destructure__x__y__z` (spelled set: bound AND waived
//! fields) whose initializer is the place, plus one Unit-sentinel let per
//! bound field. This pass enforces the LAW half: the spelled set must equal
//! the data definition's declared field set exactly -- a missing field
//! refuses naming it (that is the point of the pattern: adding a field to
//! the data breaks every destructuring site until each decides bind-or-
//! waive), and an unknown field refuses naming it (typo fence).

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

const MARKER_PREFIX: &str = "__destructure__";

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
                let Some(encoded) = local.name.as_str().strip_prefix(MARKER_PREFIX) else {
                    continue;
                };
                // The marker name encodes the spelled fields joined by `__`.
                // (A field whose own name contains `__` would mis-split;
                // acceptable v1 -- the split parts still cover the same
                // characters, so a missing-field refusal cannot be masked,
                // only a spurious unknown-field error produced.)
                let spelled: Vec<&str> = encoded.split("__").collect();

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

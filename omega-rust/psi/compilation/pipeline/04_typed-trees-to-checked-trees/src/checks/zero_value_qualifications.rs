//! Retain the membership judgment for an admitted zero construction.
//!
//! Implicit locals and omitted constructor fields share this producer. Shape
//! availability alone does not establish their value: the complete zero-value
//! judgment checks carrier validity, home-case fields and domain obligations.
//! Store successful memberships on the exact structural value, since a
//! synthesized field's expression identifies its enclosing authored literal.
//! Receiving bindings only transport this evidence; they cannot create it.

use checked_trees::{CheckFacts, CheckedStructuralValueKind, CheckedValueQualification};

pub(super) fn record(program: &dyn validation::CheckingInput, facts: &mut CheckFacts) {
    for (handle, value) in facts.values.structural_values.nodes.iter() {
        if !matches!(
            value.kind,
            CheckedStructuralValueKind::ZeroedStructural { .. }
                | CheckedStructuralValueKind::ZeroedScalarArray { .. }
        ) || validation::type_requires_establishment(program, value.type_reference)
        {
            continue;
        }
        for (_, semantic_domain) in
            validation::field_domains::domain_constraint_identities(program, value.type_reference)
        {
            facts
                .qualifications
                .value_formations
                .append(CheckedValueQualification {
                    machine: value.machine,
                    state: value.state,
                    statement_index: value.statement_index,
                    expression: value.expression,
                    semantic_domain,
                    structural_value: handle,
                });
        }
    }
}

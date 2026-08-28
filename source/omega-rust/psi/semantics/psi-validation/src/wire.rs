use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
#[cfg(test)]
use psi_typed_trees::wire::{WireField, WireMember, WireSchema};

mod decode_call;
mod encode_call;
mod schema_validation;
mod value_fields;

use decode_call::validate_wire_decode_call;
use encode_call::validate_wire_encode_call;
#[cfg(test)]
use schema_validation::validate_nested_schema_cycles;
pub(crate) use schema_validation::validate_wire_schemas;

/// Validate a call whose receiver names a wire schema: the synthesized
/// `Schema::encode(&value, &mut out, &mut written)` encoder (wire stage
/// 2a) or `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)`
/// decoder (wire stage 2b). Returns `true` when the receiver names a wire
/// schema (the call belongs to this module whether or not it validates).
pub(crate) fn validate_wire_schema_call(
    program: &TypedTrees,
    call: &psi_typed_trees::statement::TableCall,
    current_machine: &psi_typed_trees::machine::Machine,
    current_state: Option<&psi_typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let receiver_members = program.statement_table.name_path_members(call.receiver);
    let [schema_name] = receiver_members else {
        return false;
    };
    let Some(schema) = program
        .wire_schemas()
        .iter()
        .find(|schema| schema.name.as_str() == schema_name.as_str())
    else {
        return false;
    };

    match call.target.as_str() {
        psi_typed_trees::wire::WIRE_ENCODE_MACHINE_NAME => {
            validate_wire_encode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        psi_typed_trees::wire::WIRE_DECODE_MACHINE_NAME => {
            validate_wire_decode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        // The pre-rename spellings (retired 2026-07-02): a guided error, not
        // a mystery "no machine" message.
        "encode_wire" | "decode_wire" => {
            diagnostics.push(Diagnostic::error(format!(
                "`{}::{}` was renamed: the synthesized codec entries are `{}::encode(&value, \
                 &mut out, &mut written)` and `{}::decode(&mut value, &buffer, &mut read, \
                 &mut verdict)`",
                schema.name, call.target, schema.name, schema.name
            )));
        }
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has no machine `{}`; the compiler only synthesizes `encode(&value, &mut out, &mut written)` and `decode(&mut value, &buffer, &mut read, &mut verdict)` (wire stage 2)",
                schema.name, call.target
            )));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_language_core::BindingRelevance;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::types::TypeReferenceNode;

    fn self_referential_schema(relevance: BindingRelevance) -> TypedTrees {
        let mut program = TypedTrees::default();
        let self_reference = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: Default::default(),
                name: Identifier::generated("Recursive"),
            });
        let members = program.append_wire_members(vec![WireMember::Field(WireField {
            number: 0,
            name: Identifier::generated("next"),
            relevance,
            type_reference: self_reference,
        })]);
        program.push_wire_schema(WireSchema {
            name: Identifier::generated("Recursive"),
            members,
            ..WireSchema::default()
        });
        program
    }

    #[test]
    fn erased_nested_field_does_not_form_a_physical_schema_cycle() {
        let program = self_referential_schema(BindingRelevance::Erased);
        let mut diagnostics = Vec::new();

        validate_nested_schema_cycles(&program, &program.wire_schemas()[0], &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn relevant_nested_field_still_forms_a_physical_schema_cycle() {
        let program = self_referential_schema(BindingRelevance::Relevant);
        let mut diagnostics = Vec::new();

        validate_nested_schema_cycles(&program, &program.wire_schemas()[0], &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
    }
}

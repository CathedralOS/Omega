//! Wire schemas: the schema declarations, and calls to the `encode` and
//! `decode` machines the compiler synthesizes for each schema.
//!
//! Two entrances. `validate_wire_schemas` (`schema_validation`) runs once from
//! `program_validation::validate`. For each schema it rejects a duplicate
//! schema name and duplicate version names, checks field numbering, field
//! types and nested version blocks in each version era and in the current
//! body, checks each era against its successor, and rejects a schema that
//! reaches itself through relevant current-era nested fields.
//!
//! `validate_wire_schema_call` runs from `machine_calls::calls::validate_call_node`
//! before the call's receiver is checked as state storage. It returns `true`
//! when it owns the call: the receiver names a wire schema, or an `encode` or
//! `decode` receiver matches several schemas. For a schema receiver it
//! dispatches `encode` to `encode_call` and `decode` to `decode_call`, and
//! reports a retired or unknown codec name. `value_fields` holds the nested
//! and repeated field checks both call validators share.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

mod decode_call;
mod encode_call;
mod schema_validation;
mod value_fields;

use decode_call::validate_wire_decode_call;
use encode_call::validate_wire_encode_call;
pub(crate) use schema_validation::validate_wire_schemas;

/// Validate a call whose receiver names a wire schema: the synthesized
/// `Schema::encode(&value, &mut out, &mut written)` encoder (wire stage
/// 2a) or `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)`
/// decoder (wire stage 2b). Returns `true` when the receiver names a wire
/// schema (the call belongs to this module whether or not it validates).
///
/// The receiver binds by its resolved declaration symbol
/// (`wire_call_receiver_schema`), so a qualified `module::Schema::encode`
/// reaches here and same-named schemas in sibling modules cannot be
/// confused by the receiver's leaf spelling.
pub(crate) fn validate_wire_schema_call(
    program: &TypedTrees,
    call: &typed_trees::statement::TableCall,
    current_machine: &typed_trees::machine::Machine,
    current_state: Option<&typed_trees::state::State>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let Some(schema) = program.wire_call_receiver_schema(call) else {
        return ambiguous_schema_receiver(program, call, diagnostics);
    };

    match call.target.as_str() {
        typed_trees::wire::WIRE_ENCODE_MACHINE_NAME => {
            validate_wire_encode_call(
                program,
                schema,
                call,
                current_machine,
                current_state,
                diagnostics,
            );
        }
        typed_trees::wire::WIRE_DECODE_MACHINE_NAME => {
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

/// A codec-spelled call (`encode`/`decode`, including the retired spellings)
/// whose receiver did not resolve to one schema still belongs to this module
/// when its leaf names more than one wire schema: the call is rejected with
/// the ambiguity rather than silently binding the first declaration or
/// reporting the receiver as undeclared state storage.
fn ambiguous_schema_receiver(
    program: &TypedTrees,
    call: &typed_trees::statement::TableCall,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !matches!(
        call.target.as_str(),
        typed_trees::wire::WIRE_ENCODE_MACHINE_NAME
            | typed_trees::wire::WIRE_DECODE_MACHINE_NAME
            | "encode_wire"
            | "decode_wire"
    ) {
        return false;
    }
    let members = program.statement_table.name_path_members(call.receiver);
    let Some(leaf) = members.last() else {
        return false;
    };
    let candidates: Vec<String> = program
        .wire_schemas()
        .iter()
        .map(|schema| program.symbols.display_path(schema.symbol, "::"))
        .filter(|path| path.rsplit("::").next() == Some(leaf.as_str()))
        .collect();
    if candidates.len() < 2 {
        return false;
    }
    let authored = members
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    diagnostics.push(Diagnostic::error(format!(
        "codec call receiver `{authored}` is ambiguous between wire schemas {}; qualify the receiver path",
        candidates.join(", ")
    )));
    true
}

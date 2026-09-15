//! Shared JSON encoding for strings, permission identities, and carry policies.

use crate::encoding::manifest_coordinates::{state_label_from_symbol, symbol_label};
use checked_trees::CheckedTrees;

pub(crate) fn push_claim_identity_json(
    json: &mut String,
    program: &CheckedTrees,
    identity: language_semantics::PermissionClaimIdentity,
) {
    match identity {
        language_semantics::PermissionClaimIdentity::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        language_semantics::PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push_str(", \"ordinal\": ");
            json.push_str(&ordinal.to_string());
            json.push('}');
        }
    }
}

pub(crate) fn push_carry_policy_json(output: &mut String, policy: language_semantics::CarryPolicy) {
    use language_semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    output.push_str("{\"suspension\": ");
    push_json_string(
        output,
        match policy.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        },
    );
    output.push_str(", \"cpu\": ");
    push_json_string(
        output,
        match policy.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        },
    );
    output.push_str(", \"thread\": ");
    push_json_string(
        output,
        match policy.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        },
    );
    output.push_str(", \"address\": ");
    push_json_string(
        output,
        match policy.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        },
    );
    output.push('}');
}

pub(crate) fn push_json_strings(json: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
}

pub(crate) fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", c as u32);
            }
            c => output.push(c),
        }
    }
    output.push('"');
}
pub(crate) fn push_permission_event_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: language_semantics::PermissionEventSource,
) {
    use language_semantics::PermissionEventSource;
    match source {
        PermissionEventSource::StateEntry => json.push_str("{\"kind\": \"state_entry\"}"),
        PermissionEventSource::Statement { statement_index } => {
            json.push_str("{\"kind\": \"statement\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push('}');
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            json.push_str("{\"kind\": \"call\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push_str(", \"call_ordinal\": ");
            json.push_str(&call_ordinal.to_string());
            json.push_str(", \"target\": ");
            push_json_string(json, &state_label_from_symbol(program, target_symbol));
            json.push('}');
        }
        PermissionEventSource::StateExit => json.push_str("{\"kind\": \"state_exit\"}"),
    }
}

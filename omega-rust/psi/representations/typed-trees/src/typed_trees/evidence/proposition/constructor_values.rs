//! Constructor values in proof arguments retain declaration and field identity.
//!
//! A type-only label would let evidence about one case or payload satisfy a
//! proposition about another. Original propositions and substituted call/result
//! arguments therefore share this encoder, using selected declarations rather
//! than display names supplied by expressions. These labels identify values;
//! ordinary construction checking still establishes their validity.

use crate::data::DataMember;
use crate::expression::{ExpressionHandle, TableStructLiteralField};
use symbols::SymbolHandle;

pub(super) fn render(
    program: &crate::TypedTrees,
    owner: SymbolHandle,
    case: Option<SymbolHandle>,
    fields: &[TableStructLiteralField],
    render_value: impl Fn(ExpressionHandle) -> String,
) -> Option<String> {
    let mut owners = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == owner);
    let data = owners.next()?;
    if owners.next().is_some() {
        return None;
    }
    let members = program.data_members(data);
    let selected_case = if let Some(case) = case {
        let mut cases = members.iter().filter_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == case => Some(variant),
            _ => None,
        });
        let selected = cases.next()?;
        if cases.next().is_some() {
            return None;
        }
        Some(selected)
    } else {
        None
    };
    // This is the same local versus managed-owner distinction as ordinary
    // type identity. Never serialize arena indices into portable proof labels.
    let owner = program
        .normalized_hermetic_symbol_identity(owner)
        .unwrap_or_else(|_| program.symbols.display_path(owner, "::"));
    let case = selected_case.map_or_else(
        || "record".to_owned(),
        |case| member_identity(case.identity, case.name.as_str()),
    );
    let mut values = Vec::with_capacity(fields.len());
    for field in fields {
        let common = members.iter().filter_map(|member| match member {
            DataMember::Field(declared) if declared.symbol == field.field_symbol => {
                Some(("common", declared))
            }
            _ => None,
        });
        let payload = selected_case
            .into_iter()
            .flat_map(|case| program.data_payload_fields(case))
            .filter(|declared| declared.symbol == field.field_symbol)
            .map(|declared| ("payload", declared));
        let mut declarations = common.chain(payload);
        let (scope, declared) = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let identity = format!(
            "{scope}:{}",
            member_identity(declared.identity, declared.name.as_str())
        );
        values.push((identity, render_value(field.value)));
    }
    // Only the proof value's field association is normalized; the authored
    // expression and its evaluation order remain untouched.
    values.sort_by(|left, right| left.0.cmp(&right.0));
    if values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut label = "constructor:".to_owned();
    append(&mut label, &owner);
    append(&mut label, &case);
    for (field, value) in values {
        append(&mut label, &field);
        append(&mut label, &value);
    }
    Some(label)
}

fn member_identity(identity: Option<u64>, name: &str) -> String {
    identity.map_or_else(
        || format!("name:{name}"),
        |identity| format!("schema:{identity}"),
    )
}

fn append(label: &mut String, component: &str) {
    use std::fmt::Write;
    let _ = write!(label, "{}:{component}", component.len());
}

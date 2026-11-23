use crate::evaluation::numeric_literals::literal_token;
use crate::type_error::SourceTypeError;
use crate::{TypeError, ValueType};
use num_traits::ToPrimitive;
use omega_program_contracts::{NumberedMember, SchemaNumbering};
use omega_resolved::IndexedSource;
use omega_syntax::NodeKind::{Case, CaseField, Field, RetiredNumber, StableNumber};
use std::collections::HashSet;

/// Each record, sum and payload has its own numbering scope. Nested types and
/// the number on a case itself belong to other scopes.
pub(super) fn member_scope(
    file: &IndexedSource<'_>,
    owner: usize,
) -> Result<Option<SchemaNumbering>, TypeError> {
    let members = file.nodes[owner]
        .children
        .iter()
        .copied()
        .filter(|&node| matches!(file.nodes[node].kind, Field | Case | CaseField | RetiredNumber));
    if !members
        .clone()
        .any(|node| file.child(node, StableNumber).is_some())
    {
        return Ok(None);
    }
    let mut result = SchemaNumbering::default();
    let mut used = HashSet::new();
    for node in members {
        let marker = file
            .child(node, StableNumber)
            .ok_or_else(|| TypeError::at(file, node, "every member in a numbered schema scope needs a stable number"))?;
        let token = *file.nodes[marker]
            .leaves
            .last()
            .expect("stable number contains an integer");
        let (value, suffix) = literal_token(file, marker, token)?;
        let ValueType::Exact { value, .. } = value else {
            return Err(TypeError::unsupported(file, marker));
        };
        let magnitude = value.numer();
        let number = magnitude
            .to_u64()
            .filter(|_| suffix.as_ref().is_none_or(|ty| ty.contains_integer(magnitude)))
            .ok_or_else(|| {
                TypeError::at(
                    file,
                    marker,
                    "stable schema number must be a nonnegative integer representable as u64 and in its written carrier",
                )
            })?;
        if !used.insert(number) {
            return Err(TypeError::at(file, marker, format!("duplicate stable schema number #{number} in this scope")));
        }
        if file.nodes[node].kind == RetiredNumber {
            result.retired.push(number);
        } else {
            result.members.push(NumberedMember {
                number,
                declaration_index: result.members.len(),
            });
        }
    }
    result.members.sort_unstable_by_key(|member| member.number);
    result.retired.sort_unstable();
    Ok(Some(result))
}

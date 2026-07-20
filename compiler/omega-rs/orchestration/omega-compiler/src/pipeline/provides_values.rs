//! Portable-values consumption (rung V2 of the 2026-07-07 settle): substitute
//! the SELECTED target's provides VALUE rows into `Trait::NAME` expression
//! paths BEFORE symbol resolution. The per-target row IS the declaration --
//! `windows_x64 provides FilesystemHost { O_CREATE -> 32768 }` makes
//! `FilesystemHost::O_CREATE` spell 32768 on windows_x64 -- mirroring
//! const-v0's substitution discipline exactly: each use becomes the literal,
//! and no downstream stage (resolution, types, proofs, backends, the
//! interpreter) grows a per-target-value concept. The interpreter therefore
//! sees the SELECTED target's numbers, which is precisely the differential
//! contract (it oracles the program AS COMPILED for that target).
//!
//! Loud edges:
//! - a source `const Trait::NAME` colliding with a selected-target row is a
//!   duplicate-declaration error (rows extend, never override);
//! - two selected-target rows for one `Trait::NAME` with different values are
//!   ambiguous;
//! - a reference whose name is provided ONLY by non-selected targets gets a
//!   targeted error naming both targets (otherwise it would fall through to
//!   the generic unresolved-name diagnostic).

use omega_core::diagnostics::Diagnostic;
use omega_core::literals::{IntegerLiteral, IntegerRadix};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::ExpressionNode;
use omega_syntax_trees::item::{HostProviderMappingKind, Item};
use omega_target::NativeTarget;
use std::collections::{HashMap, HashSet};

pub(crate) fn substitute_provides_values(
    syntax: &mut SyntaxTrees,
    target_name: Option<&str>,
) -> Result<(), Vec<Diagnostic>> {
    let selected =
        NativeTarget::from_omega_target_name(target_name).map_err(|diagnostic| vec![diagnostic])?;

    // (trait_tail, name) -> value for the SELECTED target; the full name set
    // across OTHER targets feeds the targeted wrong-target diagnostic.
    let mut selected_rows: HashMap<(String, String), i64> = HashMap::new();
    let mut other_target_rows: HashMap<(String, String), String> = HashMap::new();
    for item in syntax.root_items() {
        let Item::HostProvider(provider) = item else {
            continue;
        };
        let row_target = provider.target.as_str();
        let row_selected = NativeTarget::from_omega_target_name(Some(row_target))
            .is_ok_and(|resolved| resolved == selected);
        let trait_tail = syntax
            .items
            .identifier_path_members(provider.boundary_trait)
            .last()
            .map(|member| member.as_str().to_owned())
            .unwrap_or_default();
        for mapping in syntax.items.host_provider_mappings(provider.mappings) {
            let HostProviderMappingKind::Value { value } = mapping.binding else {
                continue;
            };
            let key = (trait_tail.clone(), mapping.machine.as_str().to_owned());
            if row_selected {
                if let Some(prior) = selected_rows.insert(key.clone(), value)
                    && prior != value
                {
                    return Err(vec![Diagnostic::error(format!(
                        "provides value `{}::{}` is declared twice for this target \
                         with different values ({prior} and {value})",
                        key.0, key.1
                    ))]);
                }
            } else {
                other_target_rows
                    .entry(key)
                    .or_insert_with(|| row_target.to_owned());
            }
        }
    }

    // A source const colliding with a selected-target row is a duplicate
    // declaration -- rows extend the program, they never override it.
    for item in syntax.root_items() {
        let Item::Const(definition) = item else {
            continue;
        };
        let key = (
            definition.scope.as_str().to_owned(),
            definition.name.as_str().to_owned(),
        );
        if selected_rows.contains_key(&key) {
            return Err(vec![Diagnostic::error(format!(
                "`const {}::{}` collides with a provides VALUE row for the same \
                 name on the selected target -- declare it in exactly one place",
                key.0, key.1
            ))]);
        }
    }

    if selected_rows.is_empty() && other_target_rows.is_empty() {
        return Ok(());
    }

    // Substitute two-segment Name paths. Collect first (the walk borrows the
    // table), then replace.
    let mut replacements: Vec<(omega_syntax_trees::expression::ExpressionHandle, i64)> = Vec::new();
    let mut wrong_target: HashSet<(String, String)> = HashSet::new();
    for (handle, node) in syntax.expressions.iter_expressions() {
        let ExpressionNode::Name(path) = node else {
            continue;
        };
        let members = syntax.expressions.identifier_path_members(*path);
        let [scope, name] = members else {
            continue;
        };
        let key = (scope.as_str().to_owned(), name.as_str().to_owned());
        if let Some(value) = selected_rows.get(&key) {
            replacements.push((handle, *value));
        } else if other_target_rows.contains_key(&key) {
            wrong_target.insert(key);
        }
    }
    if let Some((scope, name)) = wrong_target.into_iter().next() {
        let declared_on = other_target_rows
            .get(&(scope.clone(), name.clone()))
            .cloned()
            .unwrap_or_default();
        return Err(vec![Diagnostic::error(format!(
            "`{scope}::{name}` is a provides VALUE on target `{declared_on}`, but the \
             selected target provides no value for it -- add the row to this \
             target's provides table",
        ))]);
    }
    for (handle, value) in replacements {
        let literal = IntegerLiteral::from_parts(
            value < 0,
            IntegerRadix::Decimal,
            &value.unsigned_abs().to_string(),
        )
        .map_err(|error| vec![Diagnostic::error(format!("provides value: {error}"))])?;
        syntax
            .expressions
            .replace_expression(handle, ExpressionNode::Integer(literal));
    }
    Ok(())
}

//! Reject module constant selection before legacy lexical evaluation can erase it.

use source::{SourceId, SourceSpan};
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{ConstDefinition, Item};

pub(super) fn is_module_constant(syntax: &SyntaxTrees, definition: &ConstDefinition) -> bool {
    module_path(syntax, definition.name.source_span().source_id).is_some()
}

pub(super) fn reject_module_constant_selection(
    syntax: &SyntaxTrees,
    spelling: &str,
    reference: SourceSpan,
) -> Result<(), String> {
    let current_module = module_path(syntax, reference.source_id);
    for item in syntax.root_items() {
        let Item::Const(definition) = item else {
            continue;
        };
        let Some(module) = module_path(syntax, definition.name.source_span().source_id) else {
            continue;
        };
        let local_name = super::qualified_const_name(definition);
        let full_name = format!("{module}::{local_name}");
        let local = current_module.as_ref() == Some(&module) && spelling == local_name;
        // Package aliases are requester-local and unavailable at this stage.
        // Matching a possible suffix only rejects early evaluation; it never
        // establishes that the reference may select that declaration.
        let qualified = spelling == full_name || spelling.ends_with(&format!("::{full_name}"));
        let imported = syntax.root_items().any(|item| {
            let Item::Use(import) = item else {
                return false;
            };
            let members = syntax.items.identifier_path_members(import.path);
            if !members
                .first()
                .is_some_and(|member| member.source_span().source_id == reference.source_id)
            {
                return false;
            }
            let path = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let imports_declaration =
                path == full_name || path.ends_with(&format!("::{full_name}"));
            if imports_declaration && (spelling == definition.name.as_str() || spelling == path) {
                return true;
            }
            let imports_module = path == module || path.ends_with(&format!("::{module}"));
            imports_module
                && (spelling == format!("{path}::{local_name}")
                    || spelling
                        == format!(
                            "{}::{local_name}",
                            module.rsplit("::").next().unwrap_or(&module)
                        ))
        });
        if local || qualified || imported {
            return Err(format!(
                "module constant `{spelling}` requires resolved declaration selection before generic or domain constant evaluation"
            ));
        }
    }
    Ok(())
}

fn module_path(syntax: &SyntaxTrees, source: SourceId) -> Option<String> {
    syntax.root_items().find_map(|item| {
        let Item::Module(module) = item else {
            return None;
        };
        let members = syntax.items.identifier_path_members(module.path);
        members
            .first()
            .filter(|member| member.source_span().source_id == source)?;
        Some(
            members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::"),
        )
    })
}

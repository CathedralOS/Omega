/* Source module ownership used by declaration selection and domain naming. */
use source::SourceId;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::item::Item;

pub(crate) fn module_path(syntax: &SyntaxTrees, source: SourceId) -> Option<String> {
    syntax.root_items().find_map(|item| {
        let Item::Module(module) = item else {
            return None;
        };
        let members = syntax.items.identifier_path_members(module.path);
        members
            .first()
            .filter(|member| member.source_span().source_id == source)?;
        Some(members.joined_path_in(syntax))
    })
}

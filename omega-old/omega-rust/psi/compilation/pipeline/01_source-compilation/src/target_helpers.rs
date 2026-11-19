/*
Bind `Build`-scope helper machines to the activation's execution target before
Psi name resolution (`select_build_helpers`).

For every root machine declaration inside the `Build` source set (`sources`), if
it carries an authored target qualification matching `execution`, we preserve
`authored_target` and clear `target` so Psi resolves it as the active helper for
this `Build` execution while leaving Product target family formation untouched.
*/

use source_text::SourceId;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;

pub(crate) fn select_build_helpers(
    syntax: &mut SyntaxTrees,
    execution: &str,
    sources: &HashSet<SourceId>,
) {
    for handle in syntax.root_item_handles().to_vec() {
        let Some(machine) = syntax.root_item(handle).as_machine() else {
            continue;
        };
        if !sources.contains(&machine.name.source_span().source_id) {
            continue;
        }
        let Some(target) = machine.authored_target.as_ref().or(machine.target.as_ref()) else {
            continue;
        };
        // The request carries the execution target's canonical name. The old
        // profile lookup accepted exactly these names, without aliases.
        let selected = target.as_str(syntax) == execution;
        let mut declaration = machine.clone();
        declaration.authored_target = Some(target.clone());
        declaration.target = (!selected).then(|| target.clone());
        syntax
            .items
            .replace_item(handle, Item::Machine(declaration));
    }
}

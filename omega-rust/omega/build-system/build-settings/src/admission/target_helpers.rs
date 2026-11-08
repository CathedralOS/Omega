/*
Bind `Build`-scope helper machines to the activation's execution target before
Psi name resolution (`select_build_helpers`).

For every root machine declaration inside the `Build` source set (`sources`), if
it carries an authored target qualification matching `execution`, we preserve
`authored_target` and clear `target` so Psi resolves it as the active helper for
this `Build` execution while leaving Product target family formation untouched.
*/

use source::SourceId;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use target::TargetProfile;

pub fn select_build_helpers(
    syntax: &mut SyntaxTrees,
    execution: TargetProfile,
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
        let selected = TargetProfile::from_canonical_target_name(target.as_str(syntax))
            .is_ok_and(|target| target == execution);
        let mut declaration = machine.clone();
        declaration.authored_target = Some(target.clone());
        declaration.target = (!selected).then(|| target.clone());
        syntax
            .items
            .replace_item(handle, Item::Machine(declaration));
    }
}

//! Bind Build helpers to this activation's execution target before resolution.
//! Product family formation remains target-neutral and belongs to Psi.

use source::SourceId;
use std::collections::HashSet;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use target::TargetProfile;

/// Select Build helpers without consuming their authored target identity.
/// The source set comes from package loading, not from declaration spelling.
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

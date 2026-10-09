//! Give product families stable source identities before assigning symbols.
//!
//! Every authored body survives. The first target in lexical order occupies
//! the family path; siblings get distinct symbols during ordinary resolution.
//! This choice is not an executable selection or a family-wide guarantee.
//! Build helpers arrive with a separately admitted execution selection and
//! are not changed here. No native target catalog participates in this pass.

use diagnostics::Diagnostic;
use source::{DependencyScope, SourceFile, SourceMap};
use std::cmp::Ordering;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, ItemHandle};

struct Member<'source> {
    handle: ItemHandle,
    name: &'source Identifier,
    target: &'source Identifier,
    file: Option<&'source SourceFile>,
    module: &'source [Identifier],
}

impl Member<'_> {
    fn compare_family(&self, other: &Self) -> Ordering {
        let package = self.file.and_then(|file| file.package_identity);
        let other_package = other.file.and_then(|file| file.package_identity);
        package
            .cmp(&other_package)
            .then_with(|| self.is_build().cmp(&other.is_build()))
            .then_with(|| {
                // Unadmitted standalone sources use their loading namespace.
                // An admitted package identity supersedes physical location.
                self.file
                    .filter(|_| package.is_none())
                    .map(|file| &file.package_root)
                    .cmp(
                        &other
                            .file
                            .filter(|_| other_package.is_none())
                            .map(|file| &file.package_root),
                    )
            })
            .then_with(|| {
                self.module
                    .iter()
                    .map(Identifier::as_str)
                    .cmp(other.module.iter().map(Identifier::as_str))
            })
            .then_with(|| self.name.as_str().cmp(other.name.as_str()))
    }

    fn is_build(&self) -> bool {
        self.file
            .is_some_and(|file| file.dependency_scope == DependencyScope::Build)
    }
}

pub(super) fn normalize(
    syntax: &mut SyntaxTrees,
    sources: Option<&SourceMap>,
) -> Result<(), Vec<Diagnostic>> {
    let modules = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Module(module) = item else {
                return None;
            };
            let path = syntax.items.identifier_path_members(module.path);
            Some((path.first()?.source_span().source_id, path))
        })
        .collect::<Vec<_>>();
    let mut members = Vec::new();
    for &handle in syntax.root_item_handles() {
        let Some(machine) = syntax.root_item(handle).as_machine() else {
            continue;
        };
        let Some(target) = machine.authored_target.as_ref().or(machine.target.as_ref()) else {
            continue;
        };
        let source = machine.name.source_span().source_id;
        let file = sources
            .map(|sources| {
                sources.get(source).ok_or_else(|| {
                    vec![
                        Diagnostic::error(
                            "target declaration has no source in the supplied source map",
                        )
                        .with_source_span(machine.name.source_span()),
                    ]
                })
            })
            .transpose()?;
        members.push(Member {
            handle,
            name: &machine.name,
            target,
            file,
            module: modules
                .iter()
                .find(|(owner, _)| *owner == source)
                .map(|(_, path)| *path)
                .unwrap_or_default(),
        });
    }
    members.sort_by(|left, right| {
        left.compare_family(right)
            .then_with(|| left.target.as_str().cmp(right.target.as_str()))
    });
    let mut declarations = Vec::with_capacity(members.len());
    for family in members.chunk_by(|left, right| left.compare_family(right).is_eq()) {
        for same_target in
            family.chunk_by(|left, right| left.target.as_str() == right.target.as_str())
        {
            if same_target.len() > 1 {
                return Err(vec![Diagnostic::error(format!(
                    "machine `{}` is implemented twice for `{}` -- a target supplies exactly one implementation of a contract machine",
                    same_target[0].name, same_target[0].target,
                )).with_source_span(same_target[1].name.source_span())]);
            }
        }
        if family[0].is_build() {
            continue;
        }
        declarations.extend(family.iter().enumerate().map(|(position, member)| {
            (
                member.handle,
                (position != 0).then(|| member.target.clone()),
            )
        }));
    }
    // Release syntax borrows before changing the selected symbol spellings.
    // Only these immediate handles are needed; no source-name roster escapes.
    drop(members);
    for (handle, target) in declarations {
        let Some(machine) = syntax.root_item(handle).as_machine() else {
            continue;
        };
        if machine.target == target && machine.authored_target.is_some() {
            continue;
        }
        let mut machine = machine.clone();
        machine.authored_target = machine.authored_target.or(machine.target);
        machine.target = target;
        syntax.items.replace_item(handle, Item::Machine(machine));
    }
    Ok(())
}

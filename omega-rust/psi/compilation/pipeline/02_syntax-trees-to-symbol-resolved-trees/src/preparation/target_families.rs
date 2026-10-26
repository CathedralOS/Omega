//! Give product families stable source identities before assigning symbols.
//!
//! Every authored body keeps its exact target identity. A separate bodyless
//! Requirement occupies the portable path, so a portable call and a private
//! same-target call can never accidentally share an executable identity.
//! Ordinary resolution gives the interface its own telescope; the resolved
//! family owner joins contracts after member symbols have settled.
//! Build helpers arrive with a separately admitted execution selection and
//! are not changed here. No native target catalog participates in this pass.

use arena::HandleSpan;
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
    fn compare_family(&self, syntax_trees: &syntax_trees::SyntaxTrees, other: &Self) -> Ordering {
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
                    .map(|name| name.as_str(syntax_trees))
                    .cmp(other.module.iter().map(|name| name.as_str(syntax_trees)))
            })
            .then_with(|| {
                self.name
                    .as_str(syntax_trees)
                    .cmp(other.name.as_str(syntax_trees))
            })
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
        left.compare_family(syntax, right)
            .then_with(|| left.target.as_str(syntax).cmp(right.target.as_str(syntax)))
    });
    let mut declarations = Vec::with_capacity(members.len());
    let mut interfaces = Vec::new();
    for family in members.chunk_by(|left, right| left.compare_family(syntax, right).is_eq()) {
        for same_target in
            family.chunk_by(|left, right| left.target.as_str(syntax) == right.target.as_str(syntax))
        {
            if same_target.len() > 1 {
                return Err(vec![Diagnostic::error(format!(
                    "machine `{}` is implemented twice for `{}` -- a target supplies exactly one implementation of a contract machine",
                    same_target[0].name.as_str(syntax), same_target[0].target.as_str(syntax),
                )).with_source_span(same_target[1].name.source_span())]);
            }
        }
        if family[0].is_build() {
            continue;
        }
        interfaces.push(family[0].handle);
        declarations.extend(
            family
                .iter()
                .map(|member| (member.handle, Some(member.target.clone()))),
        );
    }
    // Release syntax borrows before changing the selected symbol spellings.
    // Only these immediate handles are needed; no source-name roster escapes.
    drop(members);
    for prototype in interfaces {
        let Some(machine) = syntax.root_item(prototype).as_machine() else {
            continue;
        };
        // Preparation can run again while type equations settle. The generated
        // head is stable syntax, not another authored target member. Closed
        // carrier attachments retain their template span but have distinct
        // prepared names; neither provenance nor spelling alone identifies
        // the declaration before symbols are assigned.
        if syntax.root_items().any(|item| {
            item.as_machine().is_some_and(|candidate| {
                candidate.target_family_interface
                    && candidate.name.source_span() == machine.name.source_span()
                    && candidate.name.as_str(syntax) == machine.name.as_str(syntax)
            })
        }) {
            continue;
        }
        let mut interface = machine.clone();
        let Some(&entry) = syntax.items.state_handles(interface.states).first() else {
            return Err(vec![
                Diagnostic::error("target family has no entry signature")
                    .with_source_span(interface.name.source_span()),
            ]);
        };
        let signature = syntax.items.state(entry);
        let entry = syntax_trees::item::State {
            name: signature.name.clone(),
            parameters: signature.parameters,
            return_type: signature.return_type,
            statements: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
        };
        let entry = syntax.items.insert_state(&entry);
        let entry = syntax.items.append_state_handle(entry);
        interface.states = HandleSpan::from_parts(entry, 1);
        // Private implementation children stay in their authored member scope.
        // A bodyless callable interface contains no implementation declarations.
        interface.nested_machines = HandleSpan::empty();
        interface.target_family_interface = true;
        interface.bodyless = true;
        interface.target = None;
        interface.authored_target = None;
        interface.boundary = false;
        interface.is_top_level_boundary_requirement = false;
        interface.satisfies = HandleSpan::empty();
        interface.contracts = HandleSpan::empty();
        interface.terminates_guarantee = false;
        // A lexical prototype supplies the shared carrier telescope, never
        // one deployment's operational contract. Checking joins all retained
        // members, including private transitive effects, for this head.
        interface.service_reach_keyword_source_spans.clear();
        interface.service_reaches = HandleSpan::empty();
        interface.service_reach_is_installation_bound = false;
        interface.invokes = HandleSpan::empty();
        interface.suspends_keyword_source_spans.clear();
        interface.blocks_keyword_source_spans.clear();
        interface.suspends = false;
        interface.blocks = false;
        interface.ranking_subjects = HandleSpan::empty();
        interface.ranking_view = HandleSpan::empty();
        interface.ranking_view_arguments = HandleSpan::empty();
        interface.ranking_range = syntax_trees::expression::ExpressionHandle::invalid();
        syntax.push_root_item(Item::Machine(interface));
    }
    for (handle, target) in declarations {
        let Some(machine) = syntax.root_item(handle).as_machine() else {
            continue;
        };
        if machine.target.as_ref().map(|name| name.as_str(syntax))
            == target.as_ref().map(|name| name.as_str(syntax))
            && machine.authored_target.is_some()
        {
            continue;
        }
        let mut machine = machine.clone();
        machine.authored_target = machine.authored_target.or(machine.target);
        machine.target = target;
        syntax.items.replace_item(handle, Item::Machine(machine));
    }
    Ok(())
}

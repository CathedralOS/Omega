//! Locate the free Build entry and its borrowed parameter for dependency source edits.
use super::{BUILD_MACHINE_NAME, machine_leaf_name};
use crate::declarations::dependencies::read::{
    DependencyProjectionError, reject_authored_toolchain_vocabulary,
};
use package_compilation::declarations::BUILD_TYPE_NAME;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{Item, StateHandle, StateParameterHandle};
use syntax_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntrySyntaxProjection {
    build_entry: StateHandle,
    builder_parameter: StateParameterHandle,
}

impl BuildEntrySyntaxProjection {
    pub const fn build_entry(&self) -> StateHandle {
        self.build_entry
    }

    pub(crate) fn receiver_name<'s>(&self, syntax: &'s SyntaxTrees) -> &'s str {
        syntax
            .items
            .state_parameter(self.builder_parameter)
            .name
            .as_str(syntax)
    }
}

pub fn project_build_entry_syntax(
    syntax_trees: &SyntaxTrees,
) -> Result<BuildEntrySyntaxProjection, DependencyProjectionError> {
    reject_authored_toolchain_vocabulary(syntax_trees)?;
    let named_builds = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine)
                if machine_leaf_name(machine.name.as_str(syntax_trees)) == BUILD_MACHINE_NAME =>
            {
                Some(machine)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if let Some(scoped) = named_builds
        .iter()
        .find(|machine| machine.attached_data.is_some())
    {
        return Err(DependencyProjectionError::ScopedBuildMachine {
            scope: scoped
                .attached_data
                .as_ref()
                .expect("scoped build has an owner")
                .as_str(syntax_trees)
                .to_owned(),
        });
    }
    if named_builds.len() > 1 {
        return Err(DependencyProjectionError::DuplicateBuildMachines {
            count: named_builds.len(),
        });
    }

    let Some(build) = named_builds.first() else {
        return Err(DependencyProjectionError::MissingBuildMachine);
    };
    if build.bodyless
        || build.boundary
        || build.target.is_some()
        || !build.lifetime_parameters.is_empty()
        || !build.type_parameters.is_empty()
    {
        return Err(DependencyProjectionError::InvalidBuildMachine);
    }

    let build_entry = *syntax_trees
        .items
        .state_handles(build.states)
        .first()
        .ok_or(DependencyProjectionError::MissingBuildEntry)?;
    let entry = syntax_trees.items.state(build_entry);
    let parameters = syntax_trees.items.state_parameters(entry.parameters);
    let [builder_parameter] = parameters else {
        return Err(DependencyProjectionError::InvalidBuildParameter);
    };
    let builder_parameter = *builder_parameter;
    let builder = syntax_trees.items.state_parameter(builder_parameter);
    if builder.is_const {
        return Err(DependencyProjectionError::InvalidBuildParameter);
    }
    let TypeReferenceNode::Reference {
        referee,
        access,
        lifetime,
    } = syntax_trees
        .type_references
        .type_reference(builder.type_reference)
    else {
        return Err(DependencyProjectionError::InvalidBuildParameter);
    };
    if lifetime.is_some() || !access.is_exclusive() || !access.is_readable() {
        return Err(DependencyProjectionError::InvalidBuildParameter);
    }
    if !matches!(
        syntax_trees.type_references.type_reference(*referee),
        TypeReferenceNode::Named(name) if name.as_str(syntax_trees) == BUILD_TYPE_NAME
    ) {
        return Err(DependencyProjectionError::InvalidBuildParameter);
    }

    Ok(BuildEntrySyntaxProjection {
        build_entry,
        builder_parameter,
    })
}

pub fn package_authored_type_name<'syntax>(
    syntax_trees: &'syntax SyntaxTrees,
    item: &'syntax Item,
) -> Option<&'syntax str> {
    match item {
        Item::Data(data) => Some(data.name.as_str(syntax_trees)),
        Item::Domain(domain) => Some(domain.name.as_str(syntax_trees)),
        Item::Trait(definition) => Some(definition.name.as_str(syntax_trees)),
        _ => None,
    }
}

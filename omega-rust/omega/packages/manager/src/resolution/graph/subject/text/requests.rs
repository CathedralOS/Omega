//! Authored root/dependency requests, selection, and package navigation.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalRootSourceSelection,
};
use super::framing::{Reader, Writer};
use super::source::{read_lineage, read_source, write_lineage, write_source};
use super::{Error, Limits};
use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::declarations::{AliasName, BuildDeclarationKind, PackageName};
use crate::resolution::source::PackageSourceNavigation;
use omega_package_source::{ExternalSourceContext, SourceRelativePath};

pub(super) fn write_root(
    writer: &mut Writer,
    root: &CanonicalRootSourceSelection,
) -> Result<(), Error> {
    writer.row("root", &[])?;
    writer.row(
        match root.role() {
            BuildDeclarationKind::Package => "role package",
            BuildDeclarationKind::Application => "role application",
            BuildDeclarationKind::Workspace => "role workspace",
        },
        &[],
    )?;
    match root.request() {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
            selection,
        } => {
            writer.row(
                "request git",
                &[requested_locator.as_bytes(), requested_revision.as_bytes()],
            )?;
            write_selection(writer, selection)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            writer.row(
                "request workspace-member",
                &[member_path.as_str().as_bytes(), requested_workspace_root],
            )?;
            write_lineage(writer, workspace_root_source)?;
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            writer.row(
                "request external-local",
                &[requested_root, source_context.to_hex().as_bytes()],
            )?;
        }
    }
    writer.row("selected", &[])?;
    write_source(writer, root.selected())
}

pub(super) fn read_root(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<CanonicalRootSourceSelection, Error> {
    reader.expect("root")?;
    reader.expect("role")?;
    let role = match reader.atom()? {
        "package" => BuildDeclarationKind::Package,
        "application" => BuildDeclarationKind::Application,
        "workspace" => BuildDeclarationKind::Workspace,
        _ => return Err(Error::new("unknown text root role")),
    };
    reader.expect("request")?;
    let request = match reader.atom()? {
        "git" => CanonicalRootSourceRequest::Git {
            requested_locator: reader.string(limits.maximum_request_bytes)?,
            requested_revision: reader.string(limits.maximum_request_bytes)?,
            selection: read_selection(reader, limits)?,
        },
        "workspace-member" => {
            let member_path =
                SourceRelativePath::parse(&reader.string(limits.maximum_request_bytes)?)
                    .map_err(|_| Error::new("invalid text root workspace member"))?;
            let requested_workspace_root = reader.bytes(limits.maximum_request_bytes)?;
            CanonicalRootSourceRequest::WorkspaceMember {
                workspace_root_source: read_lineage(reader, limits)?,
                member_path,
                requested_workspace_root,
            }
        }
        "external-local" => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: reader.bytes(limits.maximum_request_bytes)?,
            source_context: ExternalSourceContext::parse_hex(&reader.string(64)?)
                .map_err(|_| Error::new("invalid text root source context"))?,
        },
        _ => return Err(Error::new("unknown text root source request")),
    };
    reader.expect("selected")?;
    Ok(CanonicalRootSourceSelection {
        request,
        role,
        selected: read_source(reader, limits)?,
    })
}

pub(super) fn write_request(
    writer: &mut Writer,
    request: &CanonicalDependencySourceRequest,
) -> Result<(), Error> {
    match request {
        CanonicalDependencySourceRequest::Path { location, .. } => {
            writer.row("request path", &[location.as_bytes()])?
        }
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            selection,
            ..
        } => {
            writer.row("request git", &[repository.as_bytes(), revision.as_bytes()])?;
            write_selection(writer, selection)?;
        }
    }
    match request.explicit_alias() {
        None => writer.row("alias none", &[]),
        Some(alias) => writer.row("alias named", &[alias.as_str().as_bytes()]),
    }
}

pub(super) fn read_request(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<CanonicalDependencySourceRequest, Error> {
    reader.expect("request")?;
    let request = match reader.atom()? {
        "path" => CanonicalDependencySourceRequest::Path {
            explicit_alias: None,
            location: reader.string(limits.maximum_request_bytes)?,
        },
        "git" => CanonicalDependencySourceRequest::Git {
            explicit_alias: None,
            repository: reader.string(limits.maximum_request_bytes)?,
            revision: reader.string(limits.maximum_request_bytes)?,
            selection: read_selection(reader, limits)?,
        },
        _ => return Err(Error::new("unknown text dependency request")),
    };
    reader.expect("alias")?;
    let alias = match reader.atom()? {
        "none" => None,
        "named" => Some(
            AliasName::parse(reader.string(limits.maximum_identity_bytes)?)
                .map_err(|_| Error::new("invalid text explicit alias"))?,
        ),
        _ => return Err(Error::new("invalid text alias option")),
    };
    Ok(match request {
        CanonicalDependencySourceRequest::Path { location, .. } => {
            CanonicalDependencySourceRequest::Path {
                explicit_alias: alias,
                location,
            }
        }
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            selection,
            ..
        } => CanonicalDependencySourceRequest::Git {
            explicit_alias: alias,
            repository,
            revision,
            selection,
        },
    })
}

pub(super) fn into_authored(request: CanonicalDependencySourceRequest) -> DependencySourceRequest {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => DependencySourceRequest::Path {
            explicit_alias,
            location,
        },
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        } => DependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        },
    }
}

fn write_selection(writer: &mut Writer, selection: &PackageSelection) -> Result<(), Error> {
    match selection {
        PackageSelection::Root => writer.row("selection root", &[]),
        PackageSelection::Named(name) => writer.row("selection named", &[name.as_str().as_bytes()]),
    }
}

fn read_selection(reader: &mut Reader<'_>, limits: Limits) -> Result<PackageSelection, Error> {
    reader.expect("selection")?;
    match reader.atom()? {
        "root" => Ok(PackageSelection::Root),
        "named" => PackageName::parse(reader.string(limits.maximum_identity_bytes)?)
            .map(PackageSelection::Named)
            .map_err(|_| Error::new("invalid text selected package name")),
        _ => Err(Error::new("unknown text package selection")),
    }
}

pub(super) fn write_navigation(
    writer: &mut Writer,
    navigation: &PackageSourceNavigation,
) -> Result<(), Error> {
    match navigation {
        PackageSourceNavigation::Root => writer.row("navigation root", &[]),
        PackageSourceNavigation::Member(path) => {
            writer.row("navigation member", &[path.as_str().as_bytes()])
        }
    }
}

pub(super) fn read_navigation(
    reader: &mut Reader<'_>,
    limits: Limits,
) -> Result<PackageSourceNavigation, Error> {
    reader.expect("navigation")?;
    match reader.atom()? {
        "root" => Ok(PackageSourceNavigation::Root),
        "member" => SourceRelativePath::parse(&reader.string(limits.maximum_request_bytes)?)
            .map(PackageSourceNavigation::Member)
            .map_err(|_| Error::new("invalid text package navigation")),
        _ => Err(Error::new("unknown text package navigation")),
    }
}

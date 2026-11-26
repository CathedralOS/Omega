// We format and parse authored root and dependency requests, package
// selections, and package navigation in canonical source-closure text records.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalRootSourceSelection,
};
use super::framing::{Reader, Writer};
use super::source::{read_lineage, read_source, write_lineage, write_source};
use super::{Error, Limits};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::source::PackageSourceNavigation;
use source_acquisition::ExternalSourceContext;

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
        CanonicalRootSourceRequest::Bundled { name } => {
            writer.row("request bundled", &[name.as_str().as_bytes()])?
        }
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
            writer.budget.charge(64)?;
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
        "bundled" => CanonicalRootSourceRequest::Bundled {
            name: reader.package_name(limits.maximum_identity_bytes)?,
        },
        "git" => CanonicalRootSourceRequest::Git {
            requested_locator: reader.string(limits.maximum_request_bytes)?,
            requested_revision: reader.string(limits.maximum_request_bytes)?,
            selection: read_selection(reader, limits)?,
        },
        "workspace-member" => {
            let member_path = reader.relative_path(limits.maximum_request_bytes)?;
            let requested_workspace_root = reader.bytes(limits.maximum_request_bytes)?;
            CanonicalRootSourceRequest::WorkspaceMember {
                workspace_root_source: read_lineage(reader, limits)?,
                member_path,
                requested_workspace_root,
            }
        }
        "external-local" => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: reader.bytes(limits.maximum_request_bytes)?,
            source_context: ExternalSourceContext::parse_hex(&reader.hex_string(64)?)
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
        CanonicalDependencySourceRequest::Bundled { name, .. } => {
            writer.row("request bundled", &[name.as_str().as_bytes()])?
        }
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
        "bundled" => CanonicalDependencySourceRequest::Bundled {
            explicit_alias: None,
            name: reader.package_name(limits.maximum_identity_bytes)?,
        },
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
        "named" => Some(reader.alias(limits.maximum_identity_bytes)?),
        _ => return Err(Error::new("invalid text alias option")),
    };
    let mut request = request;
    match &mut request {
        CanonicalDependencySourceRequest::Bundled { explicit_alias, .. }
        | CanonicalDependencySourceRequest::Path { explicit_alias, .. }
        | CanonicalDependencySourceRequest::Git { explicit_alias, .. } => *explicit_alias = alias,
    }
    Ok(request)
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
        "named" => reader
            .package_name(limits.maximum_identity_bytes)
            .map(PackageSelection::Named),
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
        "member" => reader
            .relative_path(limits.maximum_request_bytes)
            .map(PackageSourceNavigation::Member),
        _ => Err(Error::new("unknown text package navigation")),
    }
}

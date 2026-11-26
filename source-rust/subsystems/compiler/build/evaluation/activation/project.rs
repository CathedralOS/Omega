//! Interpret the product in the plan lent to the currently executing Build.
use build_declarations::{
    ApplicationDeclaration, BuildDeclaration, BuildEntryPath, BuildRequest, PackageDeclaration,
    ProjectName, WorkspaceDeclaration,
};
use build_time_values::BuildTimeValue;
use package_compilation::build_execution::SelectedBuildMachine;
use terminal_interpreter::{TerminalInstance, TerminalObservationRoot};

use crate::host::requests::IssuedBuildRequest;

pub(super) enum Destination {
    Project(BuildDeclaration),
    Successor(BuildEntryPath),
}

impl Destination {
    pub(super) fn project(&self) -> Option<&BuildDeclaration> {
        match self {
            Self::Project(declaration) => Some(declaration),
            Self::Successor(_) => None,
        }
    }
}

pub(super) fn destination(
    instance: &TerminalInstance<'_, '_>,
    machine: &SelectedBuildMachine,
    root: TerminalObservationRoot,
    requests: &[IssuedBuildRequest],
    host: &crate::host::requests::TerminalBuildRequestHost,
) -> Result<Option<Destination>, String> {
    let program = instance.program();
    let receiver = machine.receiver_for(program.module())?;
    let plan = program
        .structural_type(receiver.structural_type)
        .ok_or("BuildPlan receiver has no structural declaration")?;
    let mut fields = plan
        .shape
        .fields()
        .filter(|field| program.module().identities.text(field.identity) == Some("product"));
    let (Some(field), None) = (fields.next(), fields.next()) else {
        return Err("BuildPlan must contain exactly one product field".into());
    };
    let selected = build_value_projection::field_value(instance, root, field)?;
    let BuildTimeValue::Case { variant, payload } = selected else {
        return Err("BuildPlan.product did not produce a BuildProduct".into());
    };
    let mut product = match variant.rsplit("::").next().unwrap_or(&variant) {
        "Unselected" if payload.is_empty() => return Ok(None),
        "Package" => Destination::Project(BuildDeclaration::Package(PackageDeclaration {
            name: ProjectName::parse(text(field_value(payload, "name")?)?)?,
        })),
        "Application" => {
            Destination::Project(BuildDeclaration::Application(ApplicationDeclaration {
                name: ProjectName::parse(text(field_value(payload, "name")?)?)?,
                artifact_only: false,
            }))
        }
        "InterpretedBuild" => Destination::Successor(BuildEntryPath::parse(text(field_value(
            payload, "entry",
        )?)?)?),
        "Workspace" => {
            if !matches!(root, TerminalObservationRoot::ReturnedArgument(0)) {
                return Err("a workspace has no named Product declaration".into());
            }
            let marker = returned_case_handle(instance, field, &variant, "members")?;
            let members = host.workspace_members(&marker)?;
            Destination::Project(BuildDeclaration::Workspace(WorkspaceDeclaration {
                members,
            }))
        }
        _ => return Err(format!("unknown Build product case `{variant}`")),
    };
    for request in requests {
        if matches!(request.request(), BuildRequest::ArtifactOnly) {
            let Destination::Project(BuildDeclaration::Application(application)) = &mut product
            else {
                return Err("artifact-only output requires an application role".into());
            };
            if application.artifact_only {
                return Err("Build requested artifact-only output more than once".into());
            }
            application.artifact_only = true;
        }
    }
    Ok(Some(product))
}

/// Read the selected opaque handle only after normal return. Its issuing host authenticates it.
fn returned_case_handle(
    instance: &TerminalInstance<'_, '_>,
    field: &terminal_psi::StructuralFieldDeclaration,
    variant: &str,
    member: &str,
) -> Result<terminal_interpreter::TerminalStructuralValue, String> {
    let program = instance.program();
    let terminal_psi::StructuralFieldType::Structural(product) = field.field_type else {
        return Err("returned selection has no structural carrier".into());
    };
    let product = program
        .structural_type(product)
        .ok_or("returned selection has no structural declaration")?;
    let case = product
        .shape
        .cases()
        .iter()
        .find(|case| program.module().identities.text(case.identity) == Some(variant))
        .ok_or("returned selection has no active case declaration")?;
    let [members] = case.fields.as_slice() else {
        return Err("returned selection must contain exactly one handle".into());
    };
    if program.module().identities.text(members.identity) != Some(member) {
        return Err(format!("returned selection requires its {member} field"));
    }
    use terminal_psi::StructuralPathSegment as Path;
    instance
        .borrowed_entry_host_atom(
            0,
            &[
                Path::Field(field.identity),
                Path::Case(case.id),
                Path::Field(members.identity),
            ],
        )
        .map_err(|reason| format!("cannot read returned {member} handle: {reason:?}"))?
        .ok_or_else(|| format!("returned {member} requires an issued handle"))
}

pub(super) fn successor_directory(
    instance: &TerminalInstance<'_, '_>,
    machine: &SelectedBuildMachine,
    host: &crate::host::filesystem::TerminalBuildFilesystemHost,
) -> Result<Option<package_compilation::build_execution::SuccessorDirectory>, String> {
    let program = instance.program();
    let receiver = machine.receiver_for(program.module())?;
    let plan = program
        .structural_type(receiver.structural_type)
        .ok_or("BuildPlan has no declaration")?;
    let field = plan
        .shape
        .fields()
        .find(|field| program.module().identities.text(field.identity) == Some("package_root"))
        .ok_or("BuildPlan has no package_root field")?;
    let selected = build_value_projection::field_value(
        instance,
        TerminalObservationRoot::ReturnedArgument(0),
        field,
    )?;
    let BuildTimeValue::Case { variant, .. } = selected else {
        return Err("BuildPlan.package_root requires a BuildPackageRoot".into());
    };
    match variant.rsplit("::").next() {
        Some("Current") => Ok(None),
        Some("Directory") => {
            let handle = returned_case_handle(instance, field, &variant, "path")?;
            host.successor_directory(&handle).map(Some)
        }
        _ => Err(format!("unknown Build package root {variant}")),
    }
}

pub(super) fn successor_directories(
    instance: &TerminalInstance<'_, '_>,
    machine: &SelectedBuildMachine,
    host: &crate::host::filesystem::TerminalBuildFilesystemHost,
) -> Result<package_compilation::build_execution::BuildDirectoryMappings, String> {
    let program = instance.program();
    let receiver = machine.receiver_for(program.module())?;
    let plan = program
        .structural_type(receiver.structural_type)
        .ok_or("BuildPlan has no declaration")?;
    let field = plan
        .shape
        .fields()
        .find(|field| program.module().identities.text(field.identity) == Some("directories"))
        .ok_or("BuildPlan has no directories field")?;
    let marker = instance
        .borrowed_entry_host_atom(
            0,
            &[terminal_psi::StructuralPathSegment::Field(field.identity)],
        )
        .map_err(|reason| format!("cannot read returned directory list: {reason:?}"))?;
    host.successor_directories(marker.as_ref())
}

fn field_value(
    fields: Vec<(String, BuildTimeValue)>,
    expected: &str,
) -> Result<BuildTimeValue, String> {
    let mut fields = fields.into_iter();
    match (fields.next(), fields.next()) {
        (Some((name, value)), None) if name == expected => Ok(value),
        _ => Err(format!(
            "Build product requires exactly one `{expected}` field"
        )),
    }
}

fn text(value: BuildTimeValue) -> Result<String, String> {
    let BuildTimeValue::Text(bytes) = value else {
        return Err("Build product name or path must be a byte view".into());
    };
    String::from_utf8(bytes).map_err(|_| "Build product name or path must be UTF-8".into())
}

pub(super) fn named_identity(
    declaration: &BuildDeclaration,
) -> Option<(build_declarations::BuildDeclarationKind, ProjectName)> {
    let name = match declaration {
        BuildDeclaration::Application(application) => &application.name,
        BuildDeclaration::Package(package) => &package.name,
        BuildDeclaration::Workspace(_) => return None,
    };
    Some((declaration.kind(), name.clone()))
}

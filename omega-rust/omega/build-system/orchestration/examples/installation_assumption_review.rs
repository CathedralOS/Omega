//! Check a project once, review its exact environment assumptions, then finish
//! native compilation from the saved program. No physical truth is certified.
#[path = "installation_assumption_review/saved_project.rs"]
mod saved_project;
use compilation_products::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
use orchestration::{BuildRequest, BuildTargets, ProjectProduct};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Read, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [mode] if mode == "--optimization-level" => {
            println!("{}", compiler::compiler_optimization_level());
            Ok(())
        }
        [mode, root, target, output] if mode == "inspect" => inspect(root, target, output),
        [mode, root, target, output] if mode == "prepare" => saved_project::prepare(root, target, output),
        [mode, prepared, output, records @ ..] if mode == "finish" => saved_project::finish(prepared, output, records),
        [mode, artifact, target, entry, installation, environment, caller, output] if mode == "accept-bootstrap" || mode == "accept-interrupt-delivery" => {
            accept(mode, artifact, target, entry, installation, environment, caller, output)
        }
        [mode, record] if mode == "show" => {
            for row in compilation_products::decode_installation_assumptions(&read_bounded(record, 1024 * 1024)?)? {
                println!("Assumption (not observed or proved): {}\n{row:#?}", row.description());
            }
            Ok(())
        }
        _ => Err("usage: installation_assumption_review inspect|prepare ROOT TARGET NEW_DIRECTORY | accept-bootstrap|accept-interrupt-delivery TERMINAL TARGET ENTRY_ID INSTALLATION_ID ENVIRONMENT_DESCRIPTION CALLER NEW_RECORD | finish PREPARED_DIRECTORY NEW_DIRECTORY [REVIEW_RECORD...] | show RECORD".into()),
    }
}

fn read_bounded(path: &str, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err("review input exceeds its size bound".into());
    }
    Ok(bytes)
}

fn profile(name: &str) -> Result<target::TargetProfile> {
    target::TargetProfile::from_omega_target_name(Some(name))
        .map_err(|error| format!("target: {error:?}").into())
}

fn inspect(root: &str, target: &str, output: &str) -> Result<()> {
    let profile = profile(target)?;
    let output = Path::new(output);
    fs::create_dir(output)?;
    let mut outcomes = orchestration::build(BuildRequest {
        root_path: root.into(),
        build_directory: Some(output.join("build")),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Terminal,
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })?;
    if outcomes.len() != 1 {
        return Err("inspection did not retain exactly one target".into());
    }
    let report = outcomes.remove(0).result?.into_report()?.report;
    let retained = report
        .into_retained_terminal_artifact()
        .ok_or("inspection has no Terminal artifact")?;
    write_inspection(&retained, target, output)
}

fn write_inspection(
    retained: &compilation_products::RetainedTerminalArtifact,
    target: &str,
    output: &Path,
) -> Result<()> {
    let proposal = retained
        .native_realization_proposal()
        .ok_or("inspection has no target proposal")?;
    let artifact = retained.artifact();
    let module = artifact.semantic_module();
    let mut catalog = format!(
        "Target: {target}\nTerminal identity: {:?}\nSelected program entry: {:?}\n",
        artifact.view().identity(),
        proposal.program_entry().checked_entry().terminal_entry()
    );
    for callback in &proposal.occurrences().callbacks {
        catalog.push_str(&format!(
            "Advertised private entry: {:?}; function: {:?}\n",
            callback.callback_thunk_artifact().entry(),
            callback.callback_thunk_identity()
        ));
    }
    for registration in &module.installed_interrupt_registrations {
        let body = module
            .machines
            .iter()
            .find(|machine| machine.operation(registration.operation).is_some())
            .ok_or("installation has no owning machine")?;
        let declaration = body
            .declaration
            .as_ref()
            .and_then(|declaration| module.identities.text(declaration.callable));
        catalog.push_str(&format!(
            "Installation: {:?}; owner: {:?}; exact declaration: {:?}\n",
            registration.operation, body.id, declaration
        ));
    }
    catalog.push_str("These are compiler identities, not permission to assume an environment. Review the selected entry, operation, and external launch description before running accept.\n");
    fs::write(output.join("terminal.psi"), artifact.to_bytes())?;
    fs::write(output.join("catalog.txt"), &catalog)?;
    print!("{catalog}");
    Ok(())
}

fn accept(
    mode: &str,
    artifact: &str,
    target: &str,
    entry: &str,
    installation: &str,
    environment: &str,
    caller: &str,
    output: &str,
) -> Result<()> {
    let bytes = read_bounded(artifact, 64 * 1024 * 1024)?;
    let canonical = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .map_err(|error| format!("canonical Terminal: {error:?}"))?;
    let entry = semantic_vocabulary::MachineId::new(entry.parse()?).ok_or("invalid entry ID")?;
    let installation = semantic_vocabulary::OperationId::new(installation.parse()?)
        .ok_or("invalid installation ID")?;
    let module = canonical.semantic_module();
    if module.machine(entry).is_none()
        || !module
            .installed_interrupt_registrations
            .iter()
            .any(|row| row.operation == installation)
    {
        return Err("chosen entry or installation is absent from this artifact".into());
    }
    let description = read_bounded(environment, 1024 * 1024)?;
    if description.is_empty() {
        return Err("environment description is empty".into());
    }
    let row = BootstrapEnvironmentAssumption {
        psi: canonical.view().identity(),
        profile: profile(target)?,
        entry,
        installation,
        interval: BootstrapAssumptionInterval::EntryUntilInstallationPublished,
        environment_description_sha256: Sha256::digest(&description).into(),
        source_provenance: format!(
            "reviewed Terminal {}",
            fs::canonicalize(artifact)?.display()
        ),
        caller_provenance: caller.to_owned(),
    };
    let row = if mode == "accept-bootstrap" {
        InstallationEnvironmentAssumption::Bootstrap(row)
    } else {
        InstallationEnvironmentAssumption::InterruptDelivery(X86InterruptDeliveryAssumption {
            psi: row.psi,
            profile: row.profile,
            entry: row.entry,
            installation: row.installation,
            interval: InterruptDeliveryInterval::InstalledScopeLifetime,
            delivery: X86InterruptDeliveryProfile::ArchitecturalExceptionsAndExternalIrqs,
            environment_description_sha256: row.environment_description_sha256,
            source_provenance: row.source_provenance,
            caller_provenance: row.caller_provenance,
        })
    };
    let record = compilation_products::encode_installation_assumptions(std::slice::from_ref(&row))?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&record)?;
    println!(
        "Recorded caller assumption, not proof or observation:\n{}\n{row:#?}\nRetain the reviewed description at {environment}. Normal compilation still checks exact entry reach, target, installation and all physical obligations.",
        row.description()
    );
    Ok(())
}

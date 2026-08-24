use std::fmt::Write;
use std::path::PathBuf;

use omega_compiler::{
    ArtifactEmissionPolicy, CompileOptions, compile, compile_to_checked,
    compile_with_worker_count_and_artifact_policy,
};
use omega_core::allocations::CountingAllocator;
use psi_core::{ServiceId, StructuralTypeId};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{OperationKind, TerminalMachineResult, TerminalModule, Terminator};
use psi_terminal_fixed_fuel::{derive_fixed_entry_fuel, validate_fixed_entry_fuel};
use psi_terminal_verifier::verify_module;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

fn main() {
    // `omega refresh-samples [samples-dir]`: compile every sample main.omg
    // under samples/, in place, in parallel, so each sample folder holds a
    // current, runnable build/omega-program.exe. Cross-platform (no shell
    // script) and links this binary's own compiler. This package is a member of
    // the root Cargo workspace.
    let mut raw_arguments = std::env::args_os().skip(1);
    let first_argument = raw_arguments.next();
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "refresh-samples")
    {
        let samples_root = raw_arguments
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("samples"));
        refresh_samples(&samples_root);
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "inspect-terminal")
    {
        inspect_terminal(raw_arguments);
        return;
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "audit")
    {
        audit(raw_arguments);
        return;
    }
    if first_argument
        .as_deref()
        .is_some_and(|first| first == "review" || first == "plan" || first == "lock")
    {
        quarantined_package_command();
    }

    let Some(arguments) = parse_arguments() else {
        eprintln!(
            "usage: omega [--check] [--build-dir <dir>] [--target <name>] <root.omg>\n       omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>\n       omega audit source <locator> [--rev <rev>] [--cache-dir <dir>]\n       omega audit source-cache-policy <locator> [--rev <rev>] [--cache-dir <dir>] [--out <record.json>]\n       omega refresh-samples [samples-dir]"
        );
        std::process::exit(2);
    };

    let options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
        target_name: arguments.target_name,
        write_output: !arguments.check_only,
    };

    match compile(options) {
        Ok(report) => {
            println!("{}", report.summary());
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }

            std::process::exit(1);
        }
    };
}

fn audit(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let mut arguments = arguments;
    let Some(subcommand) = arguments.next() else {
        eprintln!("usage: omega audit source <locator> [--rev <rev>] [--cache-dir <dir>]");
        eprintln!(
            "       omega audit source-cache-policy <locator> [--rev <rev>] [--cache-dir <dir>] [--out <record.json>]"
        );
        std::process::exit(2);
    };
    if subcommand == "source" {
        audit_source(arguments);
        return;
    }
    if subcommand == "source-cache-policy" {
        audit_source_cache_policy(arguments);
        return;
    }
    if subcommand == "packages" {
        quarantined_package_command();
    }
    eprintln!("unknown audit command `{}`", subcommand.to_string_lossy());
    eprintln!("usage: omega audit source <locator> [--rev <rev>] [--cache-dir <dir>]");
    eprintln!(
        "       omega audit source-cache-policy <locator> [--rev <rev>] [--cache-dir <dir>] [--out <record.json>]"
    );
    std::process::exit(2);
}

fn audit_source(arguments: impl Iterator<Item = std::ffi::OsString>) {
    warn_unhardened_source_resolver();
    let Some(arguments) = parse_audit_source_arguments(arguments) else {
        eprintln!("usage: omega audit source <locator> [--rev <rev>] [--cache-dir <dir>]");
        std::process::exit(2);
    };
    match omega_packages::audit_package_source_locator(
        arguments.locator,
        arguments.rev,
        &arguments.cache_dir,
        omega_packages::LocalSourceLimits::default(),
    ) {
        Ok(report) => {
            print!("{}", report.to_text());
        }
        Err(error) => {
            eprintln!("cannot audit package source: {error:?}");
            std::process::exit(1);
        }
    }
}

fn audit_source_cache_policy(arguments: impl Iterator<Item = std::ffi::OsString>) {
    warn_unhardened_source_resolver();
    let Some(arguments) = parse_audit_source_cache_policy_arguments(arguments) else {
        eprintln!(
            "usage: omega audit source-cache-policy <locator> [--rev <rev>] [--cache-dir <dir>] [--out <record.json>]"
        );
        std::process::exit(2);
    };
    let record = if let Some(out_path) = &arguments.out_path {
        omega_packages::write_source_cache_record_locator(
            arguments.locator,
            arguments.rev,
            &arguments.cache_dir,
            omega_packages::LocalSourceLimits::default(),
            out_path,
        )
    } else {
        omega_packages::resolve_source_cache_record_locator(
            arguments.locator,
            arguments.rev,
            &arguments.cache_dir,
            omega_packages::LocalSourceLimits::default(),
        )
    };
    match record {
        Ok(record) => {
            print!("{}", record.to_json());
        }
        Err(error) => {
            eprintln!("cannot resolve source-cache policy: {error:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(any())] // Quarantined legacy manifest workflow; retained only as deletion context.
fn audit_packages(arguments: impl Iterator<Item = std::ffi::OsString>) {
    if package_prototype_is_quarantined() {
        std::process::exit(2);
    }
    let Some(arguments) = parse_audit_packages_arguments(arguments) else {
        eprintln!(
            "usage: omega audit packages [--lock <omega.lock>] --manifest <manifest.json>..."
        );
        std::process::exit(2);
    };
    match omega_packages::audit_package_graph_from_paths(
        &arguments.lock_path,
        &arguments.manifest_paths,
    ) {
        Ok(report) => {
            print!("{}", report.to_text());
        }
        Err(error) => {
            eprintln!("cannot audit packages: {error:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(any())]
fn lock(arguments: impl Iterator<Item = std::ffi::OsString>) {
    if package_prototype_is_quarantined() {
        std::process::exit(2);
    }
    let mut arguments = arguments;
    let Some(subcommand) = arguments.next() else {
        eprintln!(
            "usage: omega lock assemble --root-package <package> --manifest <manifest.json>... --out <omega.lock>"
        );
        std::process::exit(2);
    };
    if subcommand != "assemble" {
        eprintln!("unknown lock command `{}`", subcommand.to_string_lossy());
        eprintln!(
            "usage: omega lock assemble --root-package <package> --manifest <manifest.json>... --out <omega.lock>"
        );
        std::process::exit(2);
    }
    lock_assemble(arguments);
}

#[cfg(any())]
fn lock_assemble(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_lock_assemble_arguments(arguments) else {
        eprintln!(
            "usage: omega lock assemble --root-package <package> --manifest <manifest.json>... --out <omega.lock>"
        );
        std::process::exit(2);
    };
    let root_package = match omega_packages::PackageName::parse(arguments.root_package) {
        Ok(package) => package,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    match omega_packages::assemble_package_lock_from_paths(
        &root_package,
        &arguments.manifest_paths,
        &arguments.out_path,
    ) {
        Ok(command) => {
            print!("{}", command.to_text());
        }
        Err(error) => {
            eprintln!("cannot assemble package lock: {error:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(any())]
fn plan(arguments: impl Iterator<Item = std::ffi::OsString>) {
    if package_prototype_is_quarantined() {
        std::process::exit(2);
    }
    let mut arguments = arguments;
    let Some(subcommand) = arguments.next() else {
        eprintln!(
            "usage: omega plan install --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --alias <alias> --package <package>"
        );
        eprintln!(
            "       omega plan update --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --package <package> [--receipt <receipt.json>]"
        );
        std::process::exit(2);
    };
    if subcommand == "install" {
        plan_install(arguments);
        return;
    }
    if subcommand == "update" {
        plan_update(arguments);
        return;
    }
    eprintln!("unknown plan command `{}`", subcommand.to_string_lossy());
    eprintln!(
        "usage: omega plan install --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --alias <alias> --package <package>"
    );
    eprintln!(
        "       omega plan update --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --package <package> [--receipt <receipt.json>]"
    );
    std::process::exit(2);
}

#[cfg(any())]
fn plan_install(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_plan_install_arguments(arguments) else {
        eprintln!(
            "usage: omega plan install --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --alias <alias> --package <package>"
        );
        std::process::exit(2);
    };
    let alias = match omega_packages::AliasName::parse(arguments.alias) {
        Ok(alias) => alias,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let package = match omega_packages::PackageName::parse(arguments.package) {
        Ok(package) => package,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let current_manifests =
        read_package_capability_manifests(&arguments.current_manifest_paths, "current");
    let candidate_manifests =
        read_package_capability_manifests(&arguments.candidate_manifest_paths, "candidate");
    match omega_packages::plan_package_install_from_lock(
        &arguments.lock_path,
        &current_manifests,
        &candidate_manifests,
        &alias,
        &package,
    ) {
        Ok(command) => {
            print!("{}", command.to_text());
        }
        Err(error) => {
            eprintln!("cannot plan package install: {error:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(any())]
fn plan_update(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_plan_update_arguments(arguments) else {
        eprintln!(
            "usage: omega plan update --lock <omega.lock> --current-manifest <manifest.json>... --candidate-manifest <manifest.json>... --package <package> [--receipt <receipt.json>]"
        );
        std::process::exit(2);
    };
    let package = match omega_packages::PackageName::parse(arguments.package) {
        Ok(package) => package,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let current_manifests =
        read_package_capability_manifests(&arguments.current_manifest_paths, "current");
    let candidate_manifests =
        read_package_capability_manifests(&arguments.candidate_manifest_paths, "candidate");
    match omega_packages::plan_package_lock_update_from_lock(
        &arguments.lock_path,
        &current_manifests,
        &candidate_manifests,
        &package,
        arguments.receipt_path.as_deref(),
    ) {
        Ok(command) => {
            print!("{}", command.to_text());
        }
        Err(error) => {
            eprintln!("cannot plan package update: {error:?}");
            std::process::exit(1);
        }
    }
}

#[cfg(any())]
fn read_package_capability_manifests(
    paths: &[PathBuf],
    label: &str,
) -> Vec<omega_packages::PackageCapabilityManifest> {
    paths
        .iter()
        .map(
            |path| match omega_packages::PackageCapabilityManifest::read_from_path(path) {
                Ok(manifest) => manifest,
                Err(error) => {
                    eprintln!("cannot read {label} manifest {}: {error:?}", path.display());
                    std::process::exit(1);
                }
            },
        )
        .collect()
}

#[cfg(any())]
fn review(arguments: impl Iterator<Item = std::ffi::OsString>) {
    if package_prototype_is_quarantined() {
        std::process::exit(2);
    }
    let mut arguments = arguments;
    let Some(subcommand) = arguments.next() else {
        eprintln!(
            "usage: omega review capability-change --old-manifest <manifest.json> --new-manifest <manifest.json> --reviewer <id> --reason <text> --out <receipt.json>"
        );
        std::process::exit(2);
    };
    if subcommand != "capability-change" {
        eprintln!("unknown review command `{}`", subcommand.to_string_lossy());
        eprintln!(
            "usage: omega review capability-change --old-manifest <manifest.json> --new-manifest <manifest.json> --reviewer <id> --reason <text> --out <receipt.json>"
        );
        std::process::exit(2);
    }
    review_capability_change(arguments);
}

#[cfg(any())]
fn review_capability_change(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_review_capability_change_arguments(arguments) else {
        eprintln!(
            "usage: omega review capability-change --old-manifest <manifest.json> --new-manifest <manifest.json> --reviewer <id> --reason <text> --out <receipt.json>"
        );
        std::process::exit(2);
    };
    let old_manifest = match omega_packages::PackageCapabilityManifest::read_from_path(
        &arguments.old_manifest_path,
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!(
                "cannot read old package manifest {}: {error:?}",
                arguments.old_manifest_path.display()
            );
            std::process::exit(1);
        }
    };
    let new_manifest = match omega_packages::PackageCapabilityManifest::read_from_path(
        &arguments.new_manifest_path,
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!(
                "cannot read new package manifest {}: {error:?}",
                arguments.new_manifest_path.display()
            );
            std::process::exit(1);
        }
    };
    let command = match omega_packages::create_capability_change_review(
        &old_manifest,
        &new_manifest,
        arguments.reviewer,
        arguments.reason,
    ) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("cannot create capability-change receipt: {error:?}");
            std::process::exit(1);
        }
    };
    if let Err(error) = command.receipt.write_to_path(&arguments.out_path) {
        eprintln!(
            "cannot write capability-change receipt {}: {error:?}",
            arguments.out_path.display()
        );
        std::process::exit(1);
    }
    print!("{}", command.to_text());
}

#[cfg(any())]
fn package_prototype_is_quarantined() -> bool {
    print_package_prototype_quarantine();
    true
}

fn quarantined_package_command() -> ! {
    print_package_prototype_quarantine();
    std::process::exit(2);
}

fn print_package_prototype_quarantine() {
    eprintln!(
        "error: this caller-authored package manifest/lock/review prototype is \
         quarantined and unavailable from the production omega CLI; package \
         admission will return only with compiler-issued evidence"
    );
}

fn warn_unhardened_source_resolver() {
    eprintln!(
        "warning: the prototype source resolver is not yet a hardened \
         hostile-input boundary; Git execution currently inherits host \
         configuration and cache/source identity rules remain under audit"
    );
}

fn inspect_terminal(arguments: impl Iterator<Item = std::ffi::OsString>) {
    let Some(arguments) = parse_inspect_terminal_arguments(arguments) else {
        eprintln!(
            "usage: omega inspect-terminal --machine <qualified> [--target <name>] <root.omg>"
        );
        std::process::exit(2);
    };
    let checked = match compile_to_checked(&arguments.root_path, arguments.target_name.as_deref()) {
        Ok(checked) => checked,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    let lowered = match psi_checked_trees_to_terminal::lower_machine(&checked, &arguments.machine) {
        Ok(lowered) => lowered,
        Err(error) => {
            eprintln!(
                "cannot lower terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    let verified = match verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    ) {
        Ok(verified) => verified,
        Err(error) => {
            eprintln!(
                "cannot verify terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    let fixed_fuel = match derive_fixed_entry_fuel(&verified, lowered.semantic_module.entry) {
        Ok(fixed_fuel) => fixed_fuel,
        Err(error) => {
            eprintln!(
                "cannot derive fixed fuel for terminal machine `{}`: {error}",
                arguments.machine
            );
            std::process::exit(1);
        }
    };
    if let Err(error) = validate_fixed_entry_fuel(&verified, &fixed_fuel) {
        eprintln!(
            "cannot validate fixed fuel for terminal machine `{}`: {error}",
            arguments.machine
        );
        std::process::exit(1);
    }
    print!(
        "{}",
        terminal_summary(&arguments.machine, &lowered.semantic_module, &fixed_fuel,)
    );
}

struct InspectTerminalArguments {
    machine: String,
    root_path: PathBuf,
    target_name: Option<String>,
}

#[cfg(any())]
struct AuditPackagesArguments {
    lock_path: PathBuf,
    manifest_paths: Vec<PathBuf>,
}

struct AuditSourceArguments {
    locator: String,
    rev: Option<String>,
    cache_dir: PathBuf,
}

struct AuditSourceCachePolicyArguments {
    locator: String,
    rev: Option<String>,
    cache_dir: PathBuf,
    out_path: Option<PathBuf>,
}

#[cfg(any())]
struct LockAssembleArguments {
    root_package: String,
    manifest_paths: Vec<PathBuf>,
    out_path: PathBuf,
}

#[cfg(any())]
struct PlanInstallArguments {
    lock_path: PathBuf,
    current_manifest_paths: Vec<PathBuf>,
    candidate_manifest_paths: Vec<PathBuf>,
    alias: String,
    package: String,
}

#[cfg(any())]
struct PlanUpdateArguments {
    lock_path: PathBuf,
    current_manifest_paths: Vec<PathBuf>,
    candidate_manifest_paths: Vec<PathBuf>,
    package: String,
    receipt_path: Option<PathBuf>,
}

#[cfg(any())]
struct ReviewCapabilityChangeArguments {
    old_manifest_path: PathBuf,
    new_manifest_path: PathBuf,
    reviewer: String,
    reason: String,
    out_path: PathBuf,
}

#[cfg(any())]
fn parse_lock_assemble_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<LockAssembleArguments> {
    let mut root_package = None;
    let mut manifest_paths = Vec::new();
    let mut out_path = None;
    while let Some(argument) = arguments.next() {
        if argument == "--root-package" {
            if root_package.is_some() {
                return None;
            }
            root_package = arguments.next().and_then(|value| value.into_string().ok());
            root_package.as_ref()?;
            continue;
        }
        if argument == "--manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            manifest_paths.push(manifest_path);
            continue;
        }
        if argument == "--out" {
            if out_path.is_some() {
                return None;
            }
            out_path = arguments.next().map(PathBuf::from);
            out_path.as_ref()?;
            continue;
        }
        return None;
    }
    if manifest_paths.is_empty() {
        return None;
    }
    Some(LockAssembleArguments {
        root_package: root_package?,
        manifest_paths,
        out_path: out_path?,
    })
}

fn parse_audit_source_cache_policy_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<AuditSourceCachePolicyArguments> {
    let mut locator = None;
    let mut rev = None;
    let mut cache_dir = None;
    let mut out_path = None;
    while let Some(argument) = arguments.next() {
        if argument == "--rev" {
            if rev.is_some() {
                return None;
            }
            rev = arguments.next().and_then(|value| value.into_string().ok());
            rev.as_ref()?;
            continue;
        }
        if argument == "--cache-dir" {
            if cache_dir.is_some() {
                return None;
            }
            cache_dir = arguments.next().map(PathBuf::from);
            cache_dir.as_ref()?;
            continue;
        }
        if argument == "--out" {
            if out_path.is_some() {
                return None;
            }
            out_path = arguments.next().map(PathBuf::from);
            out_path.as_ref()?;
            continue;
        }
        if locator.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        locator = Some(argument.into_string().ok()?);
    }
    Some(AuditSourceCachePolicyArguments {
        locator: locator?,
        rev,
        cache_dir: cache_dir.unwrap_or_else(|| PathBuf::from(".omega/package-cache")),
        out_path,
    })
}

#[cfg(any())]
fn parse_plan_install_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<PlanInstallArguments> {
    let mut lock_path = None;
    let mut current_manifest_paths = Vec::new();
    let mut candidate_manifest_paths = Vec::new();
    let mut alias = None;
    let mut package = None;
    while let Some(argument) = arguments.next() {
        if argument == "--lock" {
            if lock_path.is_some() {
                return None;
            }
            lock_path = arguments.next().map(PathBuf::from);
            lock_path.as_ref()?;
            continue;
        }
        if argument == "--current-manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            current_manifest_paths.push(manifest_path);
            continue;
        }
        if argument == "--candidate-manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            candidate_manifest_paths.push(manifest_path);
            continue;
        }
        if argument == "--alias" {
            if alias.is_some() {
                return None;
            }
            alias = arguments.next().and_then(|value| value.into_string().ok());
            alias.as_ref()?;
            continue;
        }
        if argument == "--package" {
            if package.is_some() {
                return None;
            }
            package = arguments.next().and_then(|value| value.into_string().ok());
            package.as_ref()?;
            continue;
        }
        return None;
    }
    if current_manifest_paths.is_empty() || candidate_manifest_paths.is_empty() {
        return None;
    }
    Some(PlanInstallArguments {
        lock_path: lock_path?,
        current_manifest_paths,
        candidate_manifest_paths,
        alias: alias?,
        package: package?,
    })
}

#[cfg(any())]
fn parse_plan_update_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<PlanUpdateArguments> {
    let mut lock_path = None;
    let mut current_manifest_paths = Vec::new();
    let mut candidate_manifest_paths = Vec::new();
    let mut package = None;
    let mut receipt_path = None;
    while let Some(argument) = arguments.next() {
        if argument == "--lock" {
            if lock_path.is_some() {
                return None;
            }
            lock_path = arguments.next().map(PathBuf::from);
            lock_path.as_ref()?;
            continue;
        }
        if argument == "--current-manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            current_manifest_paths.push(manifest_path);
            continue;
        }
        if argument == "--candidate-manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            candidate_manifest_paths.push(manifest_path);
            continue;
        }
        if argument == "--package" {
            if package.is_some() {
                return None;
            }
            package = arguments.next().and_then(|value| value.into_string().ok());
            package.as_ref()?;
            continue;
        }
        if argument == "--receipt" {
            if receipt_path.is_some() {
                return None;
            }
            receipt_path = arguments.next().map(PathBuf::from);
            receipt_path.as_ref()?;
            continue;
        }
        return None;
    }
    if current_manifest_paths.is_empty() || candidate_manifest_paths.is_empty() {
        return None;
    }
    Some(PlanUpdateArguments {
        lock_path: lock_path?,
        current_manifest_paths,
        candidate_manifest_paths,
        package: package?,
        receipt_path,
    })
}

#[cfg(any())]
fn parse_review_capability_change_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<ReviewCapabilityChangeArguments> {
    let mut old_manifest_path = None;
    let mut new_manifest_path = None;
    let mut reviewer = None;
    let mut reason = None;
    let mut out_path = None;
    while let Some(argument) = arguments.next() {
        if argument == "--old-manifest" {
            if old_manifest_path.is_some() {
                return None;
            }
            old_manifest_path = arguments.next().map(PathBuf::from);
            old_manifest_path.as_ref()?;
            continue;
        }
        if argument == "--new-manifest" {
            if new_manifest_path.is_some() {
                return None;
            }
            new_manifest_path = arguments.next().map(PathBuf::from);
            new_manifest_path.as_ref()?;
            continue;
        }
        if argument == "--reviewer" {
            if reviewer.is_some() {
                return None;
            }
            reviewer = arguments.next().and_then(|value| value.into_string().ok());
            reviewer.as_ref()?;
            continue;
        }
        if argument == "--reason" {
            if reason.is_some() {
                return None;
            }
            reason = arguments.next().and_then(|value| value.into_string().ok());
            reason.as_ref()?;
            continue;
        }
        if argument == "--out" {
            if out_path.is_some() {
                return None;
            }
            out_path = arguments.next().map(PathBuf::from);
            out_path.as_ref()?;
            continue;
        }
        return None;
    }
    Some(ReviewCapabilityChangeArguments {
        old_manifest_path: old_manifest_path?,
        new_manifest_path: new_manifest_path?,
        reviewer: reviewer?,
        reason: reason?,
        out_path: out_path?,
    })
}

fn parse_audit_source_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<AuditSourceArguments> {
    let mut locator = None;
    let mut rev = None;
    let mut cache_dir = None;
    while let Some(argument) = arguments.next() {
        if argument == "--rev" {
            if rev.is_some() {
                return None;
            }
            rev = arguments.next().and_then(|value| value.into_string().ok());
            rev.as_ref()?;
            continue;
        }
        if argument == "--cache-dir" {
            if cache_dir.is_some() {
                return None;
            }
            cache_dir = arguments.next().map(PathBuf::from);
            cache_dir.as_ref()?;
            continue;
        }
        if locator.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        locator = Some(argument.into_string().ok()?);
    }
    Some(AuditSourceArguments {
        locator: locator?,
        rev,
        cache_dir: cache_dir.unwrap_or_else(|| PathBuf::from(".omega/package-cache")),
    })
}

#[cfg(any())]
fn parse_audit_packages_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<AuditPackagesArguments> {
    let mut lock_path = None;
    let mut manifest_paths = Vec::new();
    while let Some(argument) = arguments.next() {
        if argument == "--lock" {
            if lock_path.is_some() {
                return None;
            }
            lock_path = arguments.next().map(PathBuf::from);
            lock_path.as_ref()?;
            continue;
        }
        if argument == "--manifest" {
            let manifest_path = arguments.next().map(PathBuf::from)?;
            manifest_paths.push(manifest_path);
            continue;
        }
        return None;
    }
    if manifest_paths.is_empty() {
        return None;
    }
    Some(AuditPackagesArguments {
        lock_path: lock_path.unwrap_or_else(|| PathBuf::from("omega.lock")),
        manifest_paths,
    })
}

fn parse_inspect_terminal_arguments(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Option<InspectTerminalArguments> {
    let mut machine = None;
    let mut root_path = None;
    let mut target_name = None;
    while let Some(argument) = arguments.next() {
        if argument == "--machine" {
            if machine.is_some() {
                return None;
            }
            machine = arguments.next().and_then(|value| value.into_string().ok());
            machine.as_ref()?;
            continue;
        }
        if argument == "--target" {
            if target_name.is_some() {
                return None;
            }
            target_name = arguments.next().and_then(|value| value.into_string().ok());
            target_name.as_ref()?;
            continue;
        }
        if root_path.is_some() || argument.to_string_lossy().starts_with('-') {
            return None;
        }
        root_path = Some(PathBuf::from(argument));
    }
    Some(InspectTerminalArguments {
        machine: machine?,
        root_path: root_path?,
        target_name,
    })
}

fn terminal_summary(
    selected_machine: &str,
    module: &TerminalModule,
    fixed_fuel: &psi_terminal_fixed_fuel::FixedEntryFuelCertificate,
) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "terminal selected_machine={} entry=machine:{}",
        selected_machine,
        module.entry.get()
    )
    .expect("writing to a String cannot fail");
    for declaration in &module.structural_types {
        writeln!(
            output,
            "type id=type:{} identity={} shape={}",
            declaration.id.get(),
            declaration.identity,
            match &declaration.shape {
                psi_terminal::StructuralTypeShape::ByteSequence(carrier) => match carrier {
                    psi_terminal::ByteSequenceCarrier::BorrowedView => {
                        "byte_sequence(borrowed_view)".to_owned()
                    }
                    psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity } => {
                        format!("byte_sequence(bounded_owned,capacity={capacity})")
                    }
                },
                psi_terminal::StructuralTypeShape::Record { fields } => {
                    format!("record(fields={})", fields.len())
                }
                psi_terminal::StructuralTypeShape::FixedArray { element, length } => {
                    format!(
                        "fixed_array(element=type:{},length={length})",
                        element.get()
                    )
                }
                psi_terminal::StructuralTypeShape::Sum { cases } => {
                    format!("sum(cases={})", cases.len())
                }
            }
        )
        .expect("writing to a String cannot fail");
    }
    for declaration in &module.structural_domains {
        writeln!(
            output,
            "domain id=domain:{} identity={} carrier=type:{}",
            declaration.id.get(),
            declaration.identity,
            declaration.carrier.get()
        )
        .expect("writing to a String cannot fail");
    }
    for declaration in &module.services {
        writeln!(
            output,
            "service id=service:{} identity={} parents={}",
            declaration.id.get(),
            declaration.identity,
            format_ids(
                declaration
                    .parents
                    .iter()
                    .map(|parent| format!("service:{}", parent.get()))
            )
        )
        .expect("writing to a String cannot fail");
    }
    for boundary in &module.boundary_machines {
        writeln!(
            output,
            "boundary id=boundary:{} identity={} attachment={} services={} requirements={}",
            boundary.id.get(),
            boundary.identity,
            boundary
                .attachment
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none"),
            format_ids(boundary.published_service_ceiling.iter().map(|service| {
                format!(
                    "service:{}:{}",
                    service.get(),
                    service_identity(module, *service).unwrap_or("unknown")
                )
            })),
            format_ids(boundary.requires.iter().map(|requirement| format!(
                "argument:{}:domain:{}",
                requirement.argument_index,
                requirement.domain.get()
            )))
        )
        .expect("writing to a String cannot fail");
    }
    for machine in &module.machines {
        writeln!(
            output,
            "machine id=machine:{} attachment={} result={} services={}",
            machine.id.get(),
            machine
                .attachment
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none"),
            match machine.result {
                TerminalMachineResult::Unit => "unit",
                TerminalMachineResult::Scalar(_) => "scalar",
                TerminalMachineResult::Structural(_) => "structural",
            },
            format_ids(machine.published_service_ceiling.iter().map(|service| {
                format!(
                    "service:{}:{}",
                    service.get(),
                    service_identity(module, *service).unwrap_or("unknown")
                )
            }))
        )
        .expect("writing to a String cannot fail");
        for (index, parameter) in machine.structural_parameters.iter().enumerate() {
            writeln!(
                output,
                "parameter machine=machine:{} index={} place=place:{} type={} multiplicity={:?} qualifications={}",
                machine.id.get(),
                index,
                parameter.place.get(),
                structural_type_identity(module, parameter.structural_type).unwrap_or("unknown"),
                parameter.multiplicity,
                format_ids(
                    parameter
                        .qualifications
                        .iter()
                        .map(|domain| format!("domain:{}", domain.get()))
                )
            )
            .expect("writing to a String cannot fail");
        }
        for claim in &machine.entry_claims {
            writeln!(
                output,
                "claim machine=machine:{} id=claim:{} input=place:{}",
                machine.id.get(),
                claim.claim.get(),
                claim.input.get()
            )
            .expect("writing to a String cannot fail");
        }
        for block in &machine.blocks {
            for operation in &block.operations {
                write_operation_summary(
                    &mut output,
                    module,
                    machine.id.get(),
                    block.id.get(),
                    operation,
                );
            }
            match &block.terminator {
                Terminator::ReturnUnit {
                    edge,
                    trivial_affine_discards,
                } => writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind=ReturnUnit edge=edge:{} trivial_affine_discards={:?}",
                    machine.id.get(),
                    block.id.get(),
                    edge.get(),
                    trivial_affine_discards
                        .iter()
                        .map(|place| place.get())
                        .collect::<Vec<_>>()
                ),
                other => writeln!(
                    output,
                    "terminator machine=machine:{} block=block:{} kind={other:?}",
                    machine.id.get(),
                    block.id.get()
                ),
            }
            .expect("writing to a String cannot fail");
        }
    }
    let identity = fixed_fuel.terminal_psi();
    writeln!(
        output,
        "fixed_fuel terminal_vocabulary={} terminal_fingerprint={} schedule={} entry=machine:{} ceiling_units={} relevant_preconditions={}",
        identity.vocabulary_marker.get(),
        identity.program_fingerprint,
        fixed_fuel.schedule().marker(),
        fixed_fuel.entry().get(),
        fixed_fuel.ceiling_units(),
        fixed_fuel.relevant_preconditions().len(),
    )
    .expect("writing to a String cannot fail");
    output
}

fn write_operation_summary(
    output: &mut String,
    module: &TerminalModule,
    machine: u64,
    block: u64,
    operation: &psi_terminal::Operation,
) {
    match &operation.kind {
        OperationKind::CallUnit {
            callee,
            structural_arguments,
            claim_transfers,
            ..
        } => {
            let callee_attachment = module
                .machines
                .iter()
                .find(|machine| machine.id == *callee)
                .and_then(|machine| machine.attachment)
                .and_then(|id| structural_type_identity(module, id))
                .unwrap_or("none");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=CallUnit callee=machine:{} callee_attachment={} arguments={} transfers={}",
                operation.id.get(),
                callee.get(),
                callee_attachment,
                format_ids(
                    structural_arguments
                        .iter()
                        .map(|argument| format!("place:{}", argument.place.get()))
                ),
                format_ids(claim_transfers.iter().map(|transfer| format!(
                    "claim:{}->argument:{}",
                    transfer.claim.get(),
                    transfer.argument_index
                )))
            )
            .expect("writing to a String cannot fail");
        }
        OperationKind::BoundaryCall {
            boundary,
            structural_arguments,
            completion_receipts,
            ..
        } => {
            let identity = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .map(|boundary| boundary.identity.as_str())
                .unwrap_or("unknown");
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=BoundaryCall boundary=boundary:{} boundary_identity={} arguments={} completion_receipts={}",
                operation.id.get(),
                boundary.get(),
                identity,
                format_ids(
                    structural_arguments
                        .iter()
                        .map(|argument| format!("place:{}", argument.place.get()))
                ),
                format_ids(completion_receipts.iter().map(|receipt| format!(
                    "claim:{}->argument:{}",
                    receipt.claim.get(),
                    receipt.argument_index
                )))
            )
            .expect("writing to a String cannot fail");
        }
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => {
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind=PortWrite service=service:{} service_identity={} port=0x{port:04x} value=0x{value:02x}",
                operation.id.get(),
                service.get(),
                service_identity(module, *service).unwrap_or("unknown")
            )
            .expect("writing to a String cannot fail");
        }
        other => {
            writeln!(
                output,
                "operation machine=machine:{machine} block=block:{block} id=operation:{} kind={other:?}",
                operation.id.get()
            )
            .expect("writing to a String cannot fail");
        }
    }
}

fn format_ids(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}

fn structural_type_identity(module: &TerminalModule, id: StructuralTypeId) -> Option<&str> {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == id)
        .map(|declaration| declaration.identity.as_str())
}

fn service_identity(module: &TerminalModule, id: ServiceId) -> Option<&str> {
    module
        .services
        .iter()
        .find(|declaration| declaration.id == id)
        .map(|declaration| declaration.identity.as_str())
}

/// Compile every sample `main.omg` under `<samples_root>` into its own `build/` directory
/// for the exact host target, fanned across the machine's cores (each sample
/// owns a distinct build dir, so the parallel compiles never collide). Samples
/// without an authored host `ProgramEntry` are reported as failures; production
/// refresh never invents a legacy entry adapter. Prints a summary and exits.
fn refresh_samples(samples_root: &std::path::Path) -> ! {
    let mut mains = Vec::new();
    if let Err(error) = collect_sample_mains(samples_root, &mut mains) {
        eprintln!(
            "cannot read samples dir {}: {error}",
            samples_root.display()
        );
        std::process::exit(2);
    }
    mains.sort();
    let total = mains.len();
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .min(total.max(1));

    let queue = std::sync::Mutex::new(mains);
    let failures = std::sync::Mutex::new(Vec::<String>::new());
    let built = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some(main_path) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let build_dir = main_path
                        .parent()
                        .expect("main.omg has a sample directory")
                        .join("build");
                    match compile_with_worker_count_and_artifact_policy(
                        CompileOptions {
                            root_path: main_path.clone(),
                            build_dir: Some(build_dir),
                            target_name: Some(
                                omega_target::TargetProfile::host().target_name().to_owned(),
                            ),
                            write_output: true,
                        },
                        1,
                        ArtifactEmissionPolicy::OutputOnly,
                    ) {
                        Ok(_) => {
                            built.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Err(diagnostics) => {
                            let first = diagnostics
                                .first()
                                .map(|diagnostic| diagnostic.to_string())
                                .unwrap_or_else(|| "unknown error".to_owned());
                            failures
                                .lock()
                                .unwrap()
                                .push(format!("{}: {first}", main_path.display()));
                        }
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().unwrap();
    println!(
        "{} of {total} samples built across {workers} threads",
        built.load(std::sync::atomic::Ordering::SeqCst)
    );
    if failures.is_empty() {
        std::process::exit(0);
    }
    for failure in &failures {
        eprintln!("FAILED {failure}");
    }
    std::process::exit(1);
}

fn collect_sample_mains(
    directory: &std::path::Path,
    mains: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if directory.join("main.omg").is_file() {
        mains.push(directory.join("main.omg"));
        return Ok(());
    }

    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "build") {
            continue;
        }
        collect_sample_mains(&path, mains)?;
    }

    Ok(())
}

struct CliArguments {
    build_dir: Option<PathBuf>,
    check_only: bool,
    root_path: PathBuf,
    target_name: Option<String>,
}

fn parse_arguments() -> Option<CliArguments> {
    let mut build_dir = None;
    let mut check_only = false;
    let mut root_path = None;
    let mut target_name = None;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = arguments.next().map(PathBuf::from);
            build_dir.as_ref()?;
            continue;
        }

        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target_name| target_name.into_string().ok());
            target_name.as_ref()?;
            continue;
        }

        if root_path.is_some() {
            return None;
        }

        root_path = Some(PathBuf::from(argument));
    }

    Some(CliArguments {
        build_dir,
        check_only,
        root_path: root_path?,
        target_name,
    })
}

// Inspect the exact UEFI x86-64 root-policy conflicts before explicitly accepting
// their printed closure digest. The output lives outside the source package;
// normal compilation only replays it and never broadens it automatically.
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_external_local_project_closure_with_storage,
};
use package_manager::review::{
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyRootPolicyDirectory,
    ReviewOnlyRootPolicyDisposition, ReviewOnlyRootPolicyName, ReviewOnlyRootPolicyRecordLimits,
    compare_review_only_initial_capabilities, compile_resolved_package_candidate_for_production,
    resolve_review_only_root_policy_decisions,
};
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
use std::path::PathBuf;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 2 && arguments.len() != 4 {
        return Err("usage: root_policy PROJECT OUTPUT [--accept EXACT_CLOSURE_DIGEST]".into());
    }
    if arguments.len() == 4 && arguments[2] != "--accept" {
        return Err("expected --accept followed by the reviewed closure digest".into());
    }
    let project = std::fs::canonicalize(&arguments[0])?;
    if project.join("omega.lock").exists() {
        return Err("this initial-policy example requires an unlocked project".into());
    }
    let output = PathBuf::from(&arguments[1]);
    let parent = output.parent().ok_or("output needs a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let parent = std::fs::canonicalize(parent)?;
    if parent.starts_with(&project) {
        return Err("policy output must be outside the source package".into());
    }
    let storage = SourceResolverStorage::for_current_user_excluding_primary_git_roots(
        std::slice::from_ref(&project),
    )?;
    let closure = resolve_external_local_project_closure_with_storage(
        &project,
        ExternalSourceContext::derive(b"omega-local-project-v1"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )?;
    let target = target::TargetProfile::from_omega_target_name(Some("uefi_x86_64"))
        .map_err(|diagnostic| diagnostic.to_string())?;
    let target_closure = closure.for_exact_target(target);
    let candidate = compile_resolved_package_candidate_for_production(
        &target_closure,
        &parent.join("policy-build"),
    )?;
    let conflicts = compare_review_only_initial_capabilities(
        candidate.reviews(),
        &target_closure,
        ReviewOnlyCapabilityConflictLimits::default(),
    )?;
    let Some(first) = conflicts.packages().first() else {
        println!("No root-policy conflicts.");
        return Ok(());
    };
    let digest: String = first
        .candidate_closure()
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let review_path = output.with_extension("review.txt");
    let mut review = conflicts.render_bounded(16 * 1024 * 1024)?;
    for package in conflicts.packages() {
        for conflict in package
            .conflicts()
            .iter()
            .filter(|conflict| conflict.is_blocking())
        {
            review.push_str(&format!(
                "\nPACKAGE {} KIND {:?} SOURCE {:?}\n{}\n",
                package.key().name().as_str(),
                conflict.kind(),
                conflict.candidate_source(),
                String::from_utf8_lossy(conflict.candidate_row().unwrap_or_default())
            ));
        }
    }
    std::fs::write(&review_path, review)?;
    println!("closure {digest}\nreview {}", review_path.display());
    if arguments.len() == 2 {
        return Ok(());
    }
    if arguments[3] != digest {
        return Err("candidate changed; inspect the new review before accepting its digest".into());
    }
    let mut decisions = Vec::new();
    for package in conflicts.packages() {
        for conflict in package
            .conflicts()
            .iter()
            .filter(|conflict| conflict.is_blocking())
        {
            decisions.push(package.root_policy_decision(
                conflict,
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
            )?);
        }
    }
    let resolution = resolve_review_only_root_policy_decisions(&conflicts, &decisions)?;
    let directory = cap_std::fs::Dir::open_ambient_dir(&parent, cap_std::ambient_authority())?;
    let directory = ReviewOnlyRootPolicyDirectory::from_capability(directory, &parent)?;
    let name = ReviewOnlyRootPolicyName::parse(
        output
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid policy filename")?,
    )?;
    directory.persist_new_resolution(
        &name,
        &resolution,
        ReviewOnlyRootPolicyRecordLimits::default(),
    )?;
    println!("Saved exact root policy to {}", output.display());
    Ok(())
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(|| run().map_err(|error| error.to_string()))
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "root-policy worker panicked".to_owned())?
}

//! Preserve the checked project's outputs while its owner still exists.
//! Package consent is checked before saving; native admission waits for review.

use super::{Result, profile, read_bounded, write_inspection};
use compiler::{
    CompilationOutput, CompilationProduct, PackageCompileFailure, PreparedPackageProduct,
};
use package_manager::operations::{ProjectCompilationAdmissionRequest, admit_project_products};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, sync::Arc};

fn diagnostics(rows: Vec<diagnostics::Diagnostic>) -> Box<dyn std::error::Error> {
    let mut text = String::new();
    // Writing to a String cannot fail.
    diagnostics::write_diagnostics(&mut text, &rows).expect("write diagnostics to String");
    text.into()
}

pub(super) fn prepare(root: &str, target: &str, output: &str) -> Result<()> {
    compiler::checked::run_on_compile_thread(|| {
        prepare_saved_project(root, target, output)
            .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])
    })
    .map_err(diagnostics)
}

fn prepare_saved_project(root: &str, target: &str, output: &str) -> Result<()> {
    let output = Path::new(output);
    fs::create_dir(output)?;
    let target = profile(target)?;
    let preparation = package_manager::operations::begin_local_project_preparation(
        Path::new(root),
        &[target],
        true,
    )?
    .ok_or("preparation requires a package project with a sibling build.omg")?;
    let mut session = orchestration::BuildSession::create(&output.join("build"))?;
    let result = prepare_project(&mut session, preparation, target);
    // The admitted completion closes staging on both success and refusal. A
    // failure before admission still owes the same resource cleanup.
    let pending = match result {
        Ok(pending) => pending,
        Err(error) => {
            return match session.close() {
                Ok(()) => Err(error),
                Err(close) => Err(format!("{error}; session disposal: {close}").into()),
            };
        }
    };
    // The saved product no longer needs an earlier program or source continuations.
    // Release them before decoding a second copy for the round-trip check.
    drop(session);
    if !pending
        .components
        .as_ref()
        .map_err(|reason| reason.to_owned())?
        .is_empty()
    {
        return Err("saving independently compiled components is not supported; no component inputs were discarded".into());
    }
    if !pending.request.installation_assumptions.is_empty()
        || !pending.request.private_entry_ram.is_empty()
    {
        return Err(
            "preparation unexpectedly supplied installation assumptions or entry RAM".into(),
        );
    }
    let inputs = pending
        .retained_native_inputs
        .as_ref()
        .ok_or("checked producer did not retain native inputs")?;
    // Recovery must authenticate the exact saved pair, not a catalog or an
    // asserted source hash. It supplies no physical or receiver authority.
    let bytes = pending.terminal.artifact().to_bytes();
    let canonical = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
            .map_err(|error| format!("canonical Psi: {error:?}"))?,
    );
    let (recovered, permissions, assumptions) =
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(canonical, inputs)
            .map_err(diagnostics)?;
    if recovered != pending.terminal
        || permissions != pending.accepted_package_permissions
        || !assumptions.is_empty()
    {
        return Err("saved native inputs do not reproduce the checked project".into());
    }
    fs::write(output.join("native-inputs.bin"), inputs)?;
    write_inspection(&pending.terminal, target.target_name(), output)?;
    println!(
        "Saved Psi SHA-256: {:x}\nSaved native inputs SHA-256: {:x}",
        Sha256::digest(&bytes),
        Sha256::digest(inputs)
    );
    print_activity();
    Ok(())
}

fn prepare_project(
    session: &mut orchestration::BuildSession,
    preparation: package_manager::operations::LocalProjectPreparation,
    target: target::TargetProfile,
) -> Result<Box<compilation_products::PendingNativeProduct>> {
    let resolved = orchestration::resolve_project_sources(
        session,
        orchestration::ProjectSourceRequest {
            preparation,
            execution: Default::default(),
            collect_timings: false,
        },
    )?;
    let mut completed = Vec::with_capacity(resolved.builds.len());
    for build in resolved.builds {
        let inputs = build.package_inputs().clone();
        completed.push(
            build
                .settle(session.compiler(), resolved.execution_profile, inputs)
                .map_err(|error| diagnostics(error.into_diagnostics()))?,
        );
    }
    let admitted = admit_project_products(
        ProjectCompilationAdmissionRequest {
            prepared: resolved.prepared,
            target_profiles: &[target],
            snapshot: None,
            verify: false,
            timings: false,
            product: CompilationProduct::Native(compilation_products::NativeProductRequest {
                retain_native_inputs: true,
                installation_assumptions: Vec::new(),
                image: Default::default(),
                private_entry_ram: Vec::new(),
                terminal_authority_policy:
                    compilation_products::compile_request::current_terminal_authority_policy(),
                terminal_authority_permission_policy: None,
                optimization_rollback: Default::default(),
            }),
        },
        session.resources(),
        resolved.execution_profile,
    );
    let mut products = Vec::with_capacity(admitted.requests.len());
    for request in admitted.requests {
        let root = request
            .occurrences
            .len()
            .checked_sub(1)
            .ok_or("package preparation has no root occurrence")?;
        let product = match orchestration::prepare_package(
            session.compiler(),
            request.occurrences,
            &mut completed,
        ) {
            Err(error) => Err(error),
            Ok(build) => finish_checked_product(
                session.compiler(),
                root,
                PreparedPackageProduct {
                    build,
                    product: request.product,
                    permissions: request.terminal_permissions,
                },
            ),
        };
        products.push(product);
    }
    // This checks accepted-lock custody, target cardinality and the compiler
    // library again, then disposes staging. Pending is not a native verdict.
    let mut outcomes = admitted.completion.complete(products, session.resources());
    if outcomes.len() != 1 || outcomes[0].target != target {
        return Err("package preparation did not return exactly the requested target".into());
    }
    match outcomes.remove(0).result? {
        CompilationOutput::Native(pending) => Ok(pending),
        _ => Err("package preparation did not return pending native work".into()),
    }
}

fn finish_checked_product(
    compiler: &mut compiler::CompilationSession,
    root: usize,
    request: PreparedPackageProduct,
) -> std::result::Result<CompilationOutput, PackageCompileFailure> {
    let failure = |rows: Vec<diagnostics::Diagnostic>| PackageCompileFailure {
        occurrence: root,
        failure: rows.into(),
    };
    let mut outputs = compiler
        .complete_package_products(vec![((), request)])
        .map_err(failure)?;
    if outputs.len() != 1 {
        return Err(failure(vec![diagnostics::Diagnostic::error(
            "package preparation lost its single product",
        )]));
    }
    outputs
        .remove(0)
        .1
        .map_err(|error| failure(error.into_diagnostics()))
}

pub(super) fn finish(directory: &str, output: &str, records: &[String]) -> Result<()> {
    compiler::checked::run_on_compile_thread(|| {
        finish_saved_project(directory, output, records)
            .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])
    })
    .map_err(diagnostics)
}

fn finish_saved_project(directory: &str, output: &str, records: &[String]) -> Result<()> {
    let directory = Path::new(directory);
    let bytes = read_bounded(
        directory
            .join("terminal.psi")
            .to_str()
            .ok_or("non-UTF-8 artifact path")?,
        64 * 1024 * 1024,
    )?;
    let inputs = read_bounded(
        directory
            .join("native-inputs.bin")
            .to_str()
            .ok_or("non-UTF-8 input path")?,
        256 * 1024 * 1024,
    )?;
    let canonical = Arc::new(
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
            .map_err(|error| format!("canonical Psi: {error:?}"))?,
    );
    let (retained, permissions, original_assumptions) =
        terminal_psi_to_abstract_operations::recover_native_derivation_inputs(canonical, &inputs)
            .map_err(diagnostics)?;
    if !original_assumptions.is_empty() {
        return Err("prepared inputs already contain installation assumptions".into());
    }
    let mut assumptions = Vec::new();
    for record in records {
        assumptions.extend(compilation_products::decode_installation_assumptions(
            &read_bounded(record, 1024 * 1024)?,
        )?);
    }
    let proposal = retained
        .native_realization_proposal()
        .ok_or("saved program has no native proposal")?;
    let mut prepared = native_compiler::prepare(
        &retained,
        native_compiler::NativeCompileRequest {
            installation_assumptions: &assumptions,
            private_entry_ram: &[],
            optimization_selections: proposal.post_terminal_optimizations().selections(),
            terminal_authority_policy:
                compilation_products::compile_request::current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy: permissions,
            terminal_authority_permission_policy: None,
            image_request: image_emission::ExecutableImageEmissionRequest::direct(
                proposal.application().subsystem,
            ),
            imports: &[],
            components: &[],
        },
    )
    .map_err(|(_, rows)| diagnostics(rows))?;
    let mut accounting = Default::default();
    for completion in prepared.scalar_completions() {
        let completed = build_settings::scalar_completion::complete(
            completion.artifact(),
            completion.target(),
            completion.demand(),
            completion.provider_proposals(),
            &mut accounting,
        )?;
        completion.complete(completed)?;
    }
    let product = native_compiler::compile(prepared).map_err(|(_, rows)| diagnostics(rows))?;
    let native_artifact::RequestedNativeArtifact::Direct(artifact) = product.artifact else {
        return Err("this review tool publishes direct executable images only".into());
    };
    artifact.validate()?;
    let publication = native_artifact::PreparedExecutablePublication::new(&artifact)?;
    let output = Path::new(output);
    fs::create_dir(output)?;
    let image = artifact.image().output();
    let image_path = output.join(&image.file_name);
    fs::write(&image_path, &image.bytes)?;
    let _receipt = publication.into_receipt(image_path.clone())?;
    println!(
        "Native image: {}\nImage SHA-256: {:x}\nNative identity: {}",
        image_path.display(),
        Sha256::digest(&image.bytes),
        artifact.identity()
    );
    print_activity();
    Ok(())
}

fn print_activity() {
    use artifacts::compile_timings::counters;
    println!(
        "Source Build executions: {}\nProduct source checks: {}\nSource assemblies: {}",
        counters::source_build_execution_count(),
        counters::product_source_check_count(),
        counters::source_assembly_invocation_count()
    );
}

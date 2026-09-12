//! Prepare the requested project, compile it, settle admission, and publish its output.

use crate::{admissions::report_unsettled_admissions, compile_arguments::CompileArguments, output};
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
};

pub(crate) fn compile_project(arguments: CompileArguments) {
    let mut options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
        target_name: arguments.target_name,
    };
    // Artifact placement belongs to the user-authored entrypoint. Package
    // preparation may replace only the compilation root with an immutable
    // resolver snapshot.
    let build_dir = options.retain_build_dir();
    let policy_root_path = options.root_path.clone();

    let artifact_policy = if arguments.output_only {
        ArtifactEmissionPolicy::OutputOnly
    } else {
        ArtifactEmissionPolicy::Full
    };
    let target_profile =
        target::TargetProfile::from_omega_target_name(options.target_name.as_deref())
            .unwrap_or_else(|diagnostic| {
                eprintln!("{diagnostic}");
                std::process::exit(1);
            });
    let prepared_project = match package_manager::operations::prepare_local_project_with_options(
        &options.root_path,
        package_manager::operations::LocalProjectPreparationOptions {
            target: target_profile,
            offline: arguments.offline,
        },
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let requested_product = if arguments.check_only {
        RequestedCompileProduct::Check
    } else {
        RequestedCompileProduct::NativeArtifact
    };
    let accepted_admissions = match trust_ledger::read_trust_admissions(&policy_root_path) {
        Ok(admissions) => admissions,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    };
    let report = match prepared_project {
        Some(prepared) if !arguments.check_only => {
            let request = package_manager::operations::PreparedLocalProjectNativeRequest::new(
                prepared,
                &build_dir,
                target_profile,
            )
            .with_artifact_policy(artifact_policy)
            .with_accepted_trust_admissions(accepted_admissions)
            .with_optimization_rollback(arguments.optimization_rollback);
            package_manager::operations::compile_prepared_local_project_for_native(request)
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(1);
                })
        }
        Some(prepared) => {
            if !arguments.optimization_rollback.is_empty() {
                let names = arguments
                    .optimization_rollback
                    .requested_disabled()
                    .as_slice()
                    .iter()
                    .map(|optimization| format!("`{}`", optimization.build_case_name()))
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!("optimization rollback {names} names stages not executed by Check");
                std::process::exit(1);
            }
            let request = package_manager::operations::PreparedLocalProjectCheckRequest::new(
                prepared,
                &build_dir,
                target_profile,
            )
            .with_artifact_policy(artifact_policy)
            .with_accepted_trust_admissions(accepted_admissions);
            package_manager::operations::check_prepared_local_project(request).unwrap_or_else(
                |error| {
                    eprintln!("{error}");
                    std::process::exit(1);
                },
            )
        }
        None => {
            let request = CompileRequest::new(options)
                .with_requested_product(requested_product)
                .with_artifact_policy(artifact_policy)
                .with_optimization_rollback(arguments.optimization_rollback)
                .with_accepted_trust_admissions(accepted_admissions);
            compile(request).unwrap_or_else(|diagnostics| {
                for diagnostic in diagnostics {
                    eprintln!("{diagnostic}");
                }
                std::process::exit(1);
            })
        }
    };
    let settlement = report.trust_admission_settlement();
    if arguments.accept_admissions {
        if let Err(diagnostics) =
            trust_ledger::accept_trust_admissions(&policy_root_path, settlement.required())
        {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    } else if !settlement.is_exactly_admitted() {
        report_unsettled_admissions(settlement);
        std::process::exit(1);
    }
    if arguments.check_only {
        println!("{}", report.summary());
    } else {
        match output::publish_native_artifact(report, &build_dir) {
            Ok((published, path)) => {
                if let Some(receipt) = published.optimization_rollback_receipt() {
                    println!("optimizer rollback: {receipt}");
                }
                println!("published native output to {}", path.display());
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
}

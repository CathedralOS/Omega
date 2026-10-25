use omega_build::inspect_packages;
use package_manager::operations::PackageInspectionOptions;

pub(crate) fn run(options: PackageInspectionOptions) {
    match inspect_packages(options) {
        Ok(outcome) => {
            print!("{}", outcome.report);
            if !outcome.complete {
                std::process::exit(1);
            }
            if outcome.requires_decision {
                std::process::exit(3);
            }
        }
        Err(error) => {
            eprintln!("cannot inspect packages: {error}");
            std::process::exit(1);
        }
    }
}

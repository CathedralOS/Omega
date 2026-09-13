//! Install, update, and inspect package sources and accepted project policy.

pub(crate) mod audit;
pub(crate) mod source;

use package_manager::operations::{
    PackageCommand, PackageCommandOptions, PackageCommandStatus, execute_package_command,
};

pub(crate) fn run(command: PackageCommand, options: PackageCommandOptions) {
    let outcome = execute_package_command(command, options).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    if !outcome.report.is_empty() {
        print!("{}", outcome.report);
        if !outcome.report.ends_with('\n') {
            println!();
        }
    }
    for path in outcome.review_paths {
        println!("review: {}", path.display());
    }
    let status = exit_status(outcome.status);
    if status != 0 {
        std::process::exit(status);
    }
}

fn exit_status(status: PackageCommandStatus) -> i32 {
    match status {
        PackageCommandStatus::Published | PackageCommandStatus::ReviewDiscarded => 0,
        PackageCommandStatus::ReviewRequired => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manager_outcomes_map_to_command_exit_status() {
        assert_eq!(exit_status(PackageCommandStatus::Published), 0);
        assert_eq!(exit_status(PackageCommandStatus::ReviewDiscarded), 0);
        assert_eq!(exit_status(PackageCommandStatus::ReviewRequired), 3);
    }
}

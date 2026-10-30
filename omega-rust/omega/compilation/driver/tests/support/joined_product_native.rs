//! The joined Product journey: compile one native Product request and run the
//! resulting Terminal and native forms end to end. Targets that only need the
//! execution oracle include `joined_product_execution.rs` instead.

#[path = "joined_product_execution.rs"]
mod execution;

pub use execution::execute_report;

pub fn execute_joined_product(
    compiler: &mut compiler::CompilationSession,
    build: compiler::PackageBuild,
    output: &std::path::Path,
) -> Result<(), compiler::CompilationProductError> {
    let product = compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
        private_entry_ram: Vec::new(),
        terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut outcomes = compiler
        .complete_package_products(vec![(
            (),
            compiler::PreparedPackageProduct {
                build,
                product,
                permissions: None,
            },
        )])
        .expect("joined Product batch");
    let (_, outcome) = outcomes.pop().expect("one Product outcome");
    let report = outcome
        .and_then(omega_build::complete_product)
        .and_then(compiler::CompilationOutput::into_report)?;
    execution::execute_report(report, output);
    Ok(())
}

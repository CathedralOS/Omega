fn main() {
    if let Err(error) = omega_resolver_execution::run_resolver_connect_helper() {
        eprintln!("omega resolver CONNECT helper failed: {error}");
        std::process::exit(1);
    }
}

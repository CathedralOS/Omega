use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr};
use std::thread;

use super::connect_protocol::{open_connect_tunnel, parse_authority};
use super::model::invalid_input;

pub const RESOLVER_CONNECT_HELPER_BASENAME: &str = "omega-resolver-connect";
pub const RESOLVER_CONNECT_BROKER_ENVIRONMENT: &str = "OMEGA_RESOLVER_BROKER";
pub const RESOLVER_CONNECT_TARGET_ENVIRONMENT: &str = "OMEGA_RESOLVER_TARGET";

/// Run the compiler-owned SSH `ProxyCommand` connector.
///
/// The helper accepts no arguments. Its compiler-authored environment names
/// the loopback broker and the already-validated destination. It never opens a
/// socket to the destination itself.
pub fn run_resolver_connect_helper() -> io::Result<()> {
    if std::env::args_os().len() != 1 {
        return Err(invalid_input(
            "the resolver CONNECT helper accepts no arguments",
        ));
    }
    let broker = required_environment(RESOLVER_CONNECT_BROKER_ENVIRONMENT)?
        .parse::<SocketAddr>()
        .map_err(|_| invalid_input("the resolver broker endpoint is malformed"))?;
    if !broker.ip().is_loopback() || broker.port() == 0 {
        return Err(invalid_input(
            "the resolver broker endpoint is not a concrete loopback socket",
        ));
    }
    let target = parse_authority(&required_environment(RESOLVER_CONNECT_TARGET_ENVIRONMENT)?)?;
    let (mut stream, trailing) = open_connect_tunnel(broker, &target)?;
    let mut output = io::stdout().lock();
    if !trailing.is_empty() {
        output.write_all(&trailing)?;
        output.flush()?;
    }
    let mut upload = stream.try_clone()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut upload);
        let _ = upload.shutdown(Shutdown::Write);
    });
    io::copy(&mut stream, &mut output)?;
    output.flush()
}

fn required_environment(name: &str) -> io::Result<String> {
    let value = std::env::var_os(name)
        .ok_or_else(|| invalid_input("the resolver CONNECT helper environment is incomplete"))?;
    value
        .into_string()
        .map_err(|_| invalid_input("the resolver CONNECT helper environment is not UTF-8"))
}

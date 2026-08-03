use omega_compiler::compile_to_checked;
use psi_checked_trees_to_terminal::lower_machine;
use psi_core::{EdgeId, OperationId};
use psi_terminal_codec::{DebugSite, DebugSubject};
use std::path::{Path, PathBuf};

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("omega-interpreter lives under compiler/omega-rs/orchestration")
        .join("canaries/pass/terminal_psi/integer_control_contract/main.omg")
}

fn operation_site<'map>(sites: &'map [DebugSite], operation: OperationId) -> &'map DebugSite {
    sites
        .iter()
        .find(|site| site.subject == DebugSubject::Operation(operation))
        .expect("every terminal operation should retain a debug site")
}

fn edge_site(sites: &[DebugSite], edge: EdgeId) -> &DebugSite {
    sites
        .iter()
        .find(|site| site.subject == DebugSubject::Edge(edge))
        .expect("every terminal edge should retain a debug site")
}

#[test]
fn terminal_operations_and_jumps_retain_exact_authored_sites() {
    let source_path = source_canary();
    let source = std::fs::read_to_string(&source_path).expect("source canary should be readable");
    let checked = compile_to_checked(&source_path, None)
        .expect("terminal-Psi integer policy source canary should compile");
    let lowered = lower_machine(&checked, "terminal_wrapping_add")
        .expect("source wrapping add should lower to terminal Psi");
    let debug_map = lowered
        .debug_map
        .as_ref()
        .expect("the source producer should retain its debug map");
    let blocks = &lowered.semantic_module.machines[0].blocks;

    let site_text = |operation| {
        let span = operation_site(&debug_map.sites, operation).span;
        &source[usize::try_from(span.start).unwrap()..usize::try_from(span.end).unwrap()]
    };

    assert_eq!(site_text(blocks[0].operations[0].id), "200u8");
    assert_eq!(site_text(blocks[1].operations[0].id), "100u8");
    assert_eq!(site_text(blocks[1].operations[1].id), "+");

    let jump_span = edge_site(&debug_map.sites, blocks[0].terminator.edge()).span;
    assert_eq!(
        &source[usize::try_from(jump_span.start).unwrap()..usize::try_from(jump_span.end).unwrap()],
        "->"
    );
}

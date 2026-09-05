use checked_trees_to_terminal_psi::lower_machine;
use compiler::compile_to_checked;
use semantic_vocabulary::{ContractId, EdgeId, ObligationId, OperationId};
use std::path::{Path, PathBuf};
use terminal_codec::{DebugSite, DebugSubject};

fn source_canary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("native differential tests live under tests/native-differential")
        .join("tests/omega/pass/terminal_psi/integer_control_contract/main.omg")
}

fn operation_site(sites: &[DebugSite], operation: OperationId) -> &DebugSite {
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

fn contract_site(sites: &[DebugSite], contract: ContractId) -> &DebugSite {
    sites
        .iter()
        .find(|site| site.subject == DebugSubject::Contract(contract))
        .expect("every terminal contract should retain a debug site")
}

fn obligation_site(sites: &[DebugSite], obligation: ObligationId) -> &DebugSite {
    sites
        .iter()
        .find(|site| site.subject == DebugSubject::Obligation(obligation))
        .expect("every terminal obligation should retain a debug site")
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

    let terminal_machine = &lowered.semantic_module.machines[0];
    let contract_span = contract_site(&debug_map.sites, terminal_machine.contract.id).span;
    let obligation_span = obligation_site(
        &debug_map.sites,
        terminal_machine.contract.ensures[0].obligation,
    )
    .span;
    assert_eq!(contract_span, obligation_span);
    assert_eq!(
        &source[usize::try_from(contract_span.start).unwrap()
            ..usize::try_from(contract_span.end).unwrap()],
        "=="
    );

    let machine_start = source
        .find("machine terminal_wrapping_add")
        .expect("source canary should contain the selected machine");
    let ensures_start = machine_start
        + source[machine_start..]
            .find("ensures 44u8 == 44u8")
            .expect("selected machine should contain its authored ensures");
    let expected_equality_start = ensures_start
        + source[ensures_start..]
            .find("==")
            .expect("authored ensures should contain its equality token");
    assert_eq!(
        usize::try_from(contract_span.start).unwrap(),
        expected_equality_start
    );
}

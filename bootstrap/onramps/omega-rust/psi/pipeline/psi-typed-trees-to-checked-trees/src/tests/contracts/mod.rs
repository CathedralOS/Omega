use super::*;

mod assembly;
mod fact_call_projections;
mod indexing;
mod instantiation;
mod proof_obligations;
mod propositions;
mod qualification_evidence;
mod resultless_laws;
mod total_specification_arithmetic;

fn parse_typed_trees(source: &str) -> psi_typed_trees::TypedTrees {
    // The source loader supplies these canonical core declarations in real
    // compilations. This single-source unit harness installs the same service
    // identities directly so checked-asm rows exercise normalized reach.
    let source =
        format!("boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}");
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

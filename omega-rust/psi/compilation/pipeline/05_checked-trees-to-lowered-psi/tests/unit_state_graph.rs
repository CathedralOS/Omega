use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalEffect, TerminalExecutionResult, interpret_terminal_artifact_measured,
};

#[path = "unit_state_graph/cases.rs"]
mod cases;

#[path = "unit_state_graph/bindings.rs"]
mod bindings;

#[path = "unit_state_graph/scalars.rs"]
mod scalars;

#[path = "unit_state_graph/tails.rs"]
mod tails;

#[path = "unit_state_graph/reachable_states.rs"]
mod reachable_states;

#[path = "unit_state_graph/ranking.rs"]
mod ranking;

#[path = "unit_state_graph/provider_attachments.rs"]
mod provider_attachments;

#[path = "unit_state_graph/discarded_results.rs"]
mod discarded_results;

const SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8], marker: u8) reaches Output; }
    machine relay(bytes: &[u8], selected: bool, marker: u8) reaches Output {
        Output::write(bytes, marker);
        transition selected {
            true -> first(bytes, marker)
            false -> second(bytes, marker)
        }
        state first(bytes: &[u8], marker: u8) {
            Output::write(bytes, 1u8);
            transition { _ -> finish(bytes, marker) }
        }
        state second(bytes: &[u8], marker: u8) {
            Output::write(bytes, 2u8);
            transition { _ -> finish(bytes, marker) }
        }
        state finish(bytes: &[u8], marker: u8) {
            Output::write(bytes, marker);
        }
    }
    data Root {}
    machine Root::enter() reaches Output {
        relay("\x80A", true, 7u8);
        relay("", false, 9u8);
        Output::write("last", 3u8);
    }
"#;

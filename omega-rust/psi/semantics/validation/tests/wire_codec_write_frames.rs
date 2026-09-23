//! A synthesized wire codec resolves a divergent argument rather than refusing
//! it.
//!
//! `write_frames`' `demand.rs` and `state_write_walk.rs` used to drop the whole
//! caller frame when any actual mentioned a divergent alias, which discarded
//! candidate sets the codec resolver can spell. Both now hand the divergent
//! origins to `known_wire_codec_call_written_paths`, which unions every proven
//! referent and keeps an unproven binding opaque on its own. Removing a
//! conservative gate only widens what is accepted, so the union is pinned
//! positively here, alongside the unproven case that must still fail closed.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn codec_program(body: &str) -> TypedTrees {
    let source = format!(
        r#"
        data Blob {{ #0 value: u64; }}
        data View {{ body: &mut u64; }}
        data Main {{ value: u64; other: u64; tag: u64; buffer: [u8; 64]; written: u64; view: View; }}
        machine pick(a: &mut u64, b: &mut u64, tag: u64) -> &mut u64 {{ match tag {{ 0 -> a, _ -> b }} }}
        machine hold(value: &mut u64) -> &mut u64 {{ value }}
        machine hold_view(view: &mut View) -> &mut View {{ view }}
        machine opaque_ref(value: &mut u64) -> &mut u64 {{ opaque_ref(value) }}
        machine Main::run(&mut self) {{ {body} }}
        "#
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

/// The caller's own state write frame, narrowed to receiver-rooted places.
fn caller_frame(program: &TypedTrees) -> Option<Vec<String>> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = &program.machine_states(machine)[0];
    let resolver = validation::CallFrameResolver::new(program).expect("resolver");
    resolver
        .inferred_state_write_frame(machine, state)
        .into_complete_paths()
        .map(|paths| {
            let mut paths = paths
                .into_iter()
                .filter(|path| path == "self" || path.starts_with("self."))
                .collect::<Vec<_>>();
            paths.sort();
            paths.dedup();
            paths
        })
}

#[test]
fn a_codec_cursor_bound_to_a_divergent_alias_unions_both_referents() {
    let program = codec_program(
        "let sample: Blob = Blob { value: 7 }; \
         let cursor: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); \
         Blob::encode(&sample, &mut self.buffer, cursor);",
    );
    assert_eq!(
        caller_frame(&program),
        Some(vec![
            "self.buffer".to_owned(),
            "self.other".to_owned(),
            "self.value".to_owned(),
        ]),
        "the codec writes its buffer and either proven referent of the cursor"
    );
}

#[test]
fn a_codec_cursor_bound_to_an_unproven_reference_stays_opaque() {
    let program = codec_program(
        "let sample: Blob = Blob { value: 7 }; \
         let cursor: &mut u64 = opaque_ref(&mut self.value); \
         Blob::encode(&sample, &mut self.buffer, cursor);",
    );
    assert_eq!(
        caller_frame(&program),
        None,
        "an unproven referent has no spellable set, so the frame fails closed"
    );
}

/// The repair has to survive the hops between the binding and the call, which
/// is where a candidate set is easiest to collapse to one route. A rebind onto
/// a second binding keeps the whole set.
///
/// A helper result standing in the middle is the same set reached one hop
/// later, and the codec leaf defers to the shared reference-origin resolver
/// for it rather than learning another argument spelling.
#[test]
fn a_divergent_codec_cursor_survives_composition() {
    for (name, body) in [
        (
            "rebound_binding",
            "let sample: Blob = Blob { value: 7 }; \
             let cursor: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); \
             let again: &mut u64 = cursor; \
             Blob::encode(&sample, &mut self.buffer, again);",
        ),
        (
            "helper_result",
            "let sample: Blob = Blob { value: 7 }; \
             let cursor: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); \
             Blob::encode(&sample, &mut self.buffer, hold(cursor));",
        ),
        // The delegation composes recursively rather than covering one hop.
        (
            "nested_helper_results",
            "let sample: Blob = Blob { value: 7 }; \
             let cursor: &mut u64 = pick(&mut self.value, &mut self.other, self.tag); \
             Blob::encode(&sample, &mut self.buffer, hold(hold(cursor)));",
        ),
    ] {
        assert_eq!(
            caller_frame(&codec_program(body)),
            Some(vec![
                "self.buffer".to_owned(),
                "self.other".to_owned(),
                "self.value".to_owned(),
            ]),
            "{name}"
        );
    }
}

/// The hop must not become a way to guess: a helper result whose own route is
/// unresolvable leaves the whole frame opaque rather than naming one arm.
#[test]
fn an_unresolvable_helper_result_keeps_the_codec_frame_opaque() {
    let body = "let sample: Blob = Blob { value: 7 }; \
         Blob::encode(&sample, &mut self.buffer, opaque_ref(&mut self.value));";
    assert_eq!(caller_frame(&codec_program(body)), None);
}

/// A match argument is the same finite set spelled inline. Each arm still earns
/// its own treatment: a borrowed place and an owned carrier's declared
/// reference leaf both resolve, while a load reached BEHIND another reference
/// does not, so delegating the arm walk is not a way past the load-evidence
/// rule.
#[test]
fn a_match_codec_argument_unions_its_arms() {
    assert_eq!(
        caller_frame(&codec_program(
            "let sample: Blob = Blob { value: 7 }; \
             Blob::encode(&sample, &mut self.buffer, \
                 match self.tag { 0 -> &mut self.value, _ -> &mut self.other });",
        )),
        Some(vec![
            "self.buffer".to_owned(),
            "self.other".to_owned(),
            "self.value".to_owned(),
        ]),
        "both borrowed arms join the frame"
    );
}

#[test]
fn a_match_arm_loading_behind_another_reference_stays_opaque() {
    assert_eq!(
        caller_frame(&codec_program(
            "let sample: Blob = Blob { value: 7 }; \
             let carrier: &mut View = hold_view(&mut self.view); \
             Blob::encode(&sample, &mut self.buffer, \
                 match self.tag { 0 -> carrier.body, _ -> &mut self.other });",
        )),
        None,
        "an interior load behind another reference has no own load evidence"
    );
}

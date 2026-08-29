use super::encoding::{
    Decoder, Encoder, decode_replay_record_option, encode_replay_record_option,
    replay_parent_binding,
};
use super::validation::replay_record_limits;
use super::*;
use omega_build_evaluation::capture_verified_build_filesystem_replay_record;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

#[test]
fn replay_record_option_framing_round_trips_compiler_bytes() {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-baseline-replay-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create replay framing fixture");
    std::fs::write(
        project.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 { }

data ReplayProbe {
    filesystem: FilesystemHost;
    status: i32;
    bytes: [u8; 144];
}

machine ReplayProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
    let source: &[u8] in Path = builder.source.resolve("main.omg");
    self.status = self.filesystem.read_metadata(source, &mut self.bytes);
}
"#,
    )
    .expect("write replay framing build");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write replay framing source");
    let compilation =
        omega_compiler::compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .expect("compile replay framing fixture");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build publishes observations");
    assert!(summary.source_inputs_replay_verified());
    let limits = ReviewOnlyBaselineLimits::default();
    let replay =
        capture_verified_build_filesystem_replay_record(summary, replay_record_limits(limits))
            .expect("capture replay record")
            .expect("verified replay record");

    let mut encoder = Encoder::bounded(limits.maximum_capsule_bytes);
    encode_replay_record_option(&mut encoder, Some(&replay)).expect("frame replay option");
    let framed = encoder.finish().expect("finish replay option");
    let mut decoder = Decoder::new(&framed);
    let recovered = decode_replay_record_option(&mut decoder, limits)
        .expect("recover framed replay option")
        .expect("recovered replay option is present");
    decoder.finish().expect("replay option consumes its frame");
    assert_eq!(recovered, replay);

    let parent = [7; 32];
    assert_eq!(
        replay_parent_binding(parent, recovered.commitment()),
        replay_parent_binding(parent, replay.commitment())
    );
    assert_ne!(
        replay_parent_binding(parent, recovered.commitment()),
        replay_parent_binding([8; 32], recovered.commitment())
    );

    assert_eq!(
        decode_replay_record_option(&mut Decoder::new(&[0]), limits)
            .expect("absent replay option")
            .as_ref(),
        None
    );
    assert!(decode_replay_record_option(&mut Decoder::new(&[2]), limits).is_err());
    let _ = std::fs::remove_dir_all(project);
}

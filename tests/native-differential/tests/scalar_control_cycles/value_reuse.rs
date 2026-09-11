//! Acyclic scalar definitions have the same once-only identity as loop values.

fn artifact() -> terminal_codec::CanonicalTerminalArtifact {
    let source = "machine identity(value: u64) -> u64 { value }\n\
         machine pair(left: u64, right: u64) -> u64 {\n\
             transition left == right { true -> left false -> 0u64 }\n\
         }\n\
         machine reuse(value: u64) -> u64 {\n\
             let output: u64 = identity(value);\n\
             pair(output, output)\n\
         }\n";
    super::produce_candidate(source, "reuse")
        .unwrap_or_else(|error| panic!("produce repeated scalar values: {error:#?}\n{source}"))
}

#[test]
fn repeated_scalar_values_publish_each_authored_call_once_on_four_targets() {
    // Publication checks the complete retained call roster against the source,
    // including after allocation, text, object, image and installation replay.
    super::publication::assert_four_targets(&artifact(), 2);
}

#[test]
fn repeated_scalar_values_execute_without_repeating_the_call() {
    super::publication::assert_host_execution(
        &artifact(),
        2,
        "#include <stdint.h>\n\
         extern uint64_t omega_entry(uint64_t value);\n\
         int main(void) {\n\
             return omega_entry(0) != 0 || omega_entry(17) != 17 ||\n\
                    omega_entry(UINT64_MAX) != UINT64_MAX;\n\
         }\n",
    );
}

fn interleaved_unit_calls() -> terminal_codec::CanonicalTerminalArtifact {
    let source = "machine before() {}\n\
        machine after() {}\n\
        machine sequence(value: u64) -> u64 {\n\
            before();\n\
            let preserved: u64 = value;\n\
            after();\n\
            preserved\n\
        }\n";
    super::produce_candidate(source, "sequence")
        .unwrap_or_else(|error| panic!("produce interleaved Unit calls: {error:#?}\n{source}"))
}

#[test]
fn unit_calls_and_scalar_values_share_the_native_graph_on_four_targets() {
    super::publication::assert_four_targets(&interleaved_unit_calls(), 2);
}

#[test]
fn unit_calls_preserve_the_live_scalar_across_both_calls() {
    super::publication::assert_host_execution(
        &interleaved_unit_calls(),
        2,
        "#include <stdint.h>\n\
         extern uint64_t omega_entry(uint64_t value);\n\
         int main(void) {\n\
             return omega_entry(0) != 0 || omega_entry(17) != 17 ||\n\
                    omega_entry(UINT64_MAX) != UINT64_MAX;\n\
         }\n",
    );
}

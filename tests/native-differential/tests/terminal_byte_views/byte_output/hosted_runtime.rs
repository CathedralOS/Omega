//! Actual hosted output uses published image bytes, including caller continuation.
use super::*;

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
fn published(module: &TerminalModule) -> (image_emission::ExecutableImage, usize) {
    let source = std::sync::Arc::new(
        object_file::stage_optimized_relocation_free_object_container(stage_byte_output_module(
            NativeTarget::host(),
            module,
        ))
        .unwrap(),
    );
    let object = image_emission::build_function_fragment_object_artifact(source.clone()).unwrap();
    image_emission::validate_function_fragment_object_artifact(&source, &object).unwrap();
    let offset = object.entry_function().text_offset;
    let image = image_emission::emit_executable_image(&object, 3).unwrap();
    image_emission::validate_executable_image(&object, &image).unwrap();
    let record = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .unwrap();
    let decoded = image_emission::decode_installation_record(
        &image_emission::encode_installation_record(&record).unwrap(),
    )
    .unwrap();
    image_emission::validate_installation_record(&decoded, &image).unwrap();
    (image, offset)
}

#[test]
fn hosted_byte_output_executes_all_bytes_normalizes_i32_and_traps_failed_write() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, offset) = published(&byte_output_module());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            offset,
            r#"
            #include <stdint.h>
            #include <unistd.h>
            #include <signal.h>
            #include <sys/wait.h>
            #include <sys/resource.h>
            /* Generated input is i32; upper register bits are deliberately unspecified. */
            extern void omega_entry(uint64_t carrier);
            int main(void) {
                alarm(10);
                struct rlimit limit = {0, 0}; if (setrlimit(RLIMIT_CORE, &limit)) return 1;
                int channel[2]; if (pipe(channel)) return 2;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 3;
                close(channel[1]);
                const uint64_t upper[] = {0, UINT64_C(0xffffffffffffff00), UINT64_C(0xa580ff1780000000)};
                for (unsigned pattern = 0; pattern < 3; ++pattern)
                    for (unsigned byte = 0; byte < 256; ++byte) omega_entry(upper[pattern] | byte);
                if (dup2(saved, STDOUT_FILENO) < 0) return 4;
                close(saved);
                for (unsigned pattern = 0; pattern < 3; ++pattern)
                    for (unsigned byte = 0; byte < 256; ++byte) {
                        uint8_t actual;
                        if (read(channel[0], &actual, 1) != 1 || actual != byte) return 5;
                    }
                uint8_t extra; if (read(channel[0], &extra, 1) != 0) return 6;
                close(channel[0]);
                pid_t child = fork(); if (child < 0) return 7;
                if (!child) { close(STDOUT_FILENO); omega_entry(0xff); _exit(99); }
                int status; if (waitpid(child, &status, 0) != child || !WIFSIGNALED(status)) return 8;
                #if defined(__aarch64__) || defined(__arm64__)
                if (WTERMSIG(status) != SIGTRAP) return 9;
                #else
                if (WTERMSIG(status) != SIGILL) return 9;
                #endif
                return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: hosted byte-output runtime requires Linux x64/AArch64 or macOS AArch64");
}

#[test]
fn hosted_byte_output_executes_widened_calls_and_continuation_for_all_bytes() {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        let (image, offset) = published(&unit_calls::unit_byte_output_calls_module());
        native_function::assert_c_text(
            &image.output().final_text_bytes,
            offset,
            r#"
            #include <stdint.h>
            #include <unistd.h>
            extern void omega_entry(uint8_t byte);
            int main(void) {
                alarm(10);
                int channel[2]; if (pipe(channel)) return 1;
                int saved = dup(STDOUT_FILENO);
                if (saved < 0 || dup2(channel[1], STDOUT_FILENO) < 0) return 2;
                close(channel[1]);
                for (unsigned repeat = 0; repeat < 2; ++repeat)
                    for (unsigned byte = 0; byte < 256; ++byte) omega_entry((uint8_t)byte);
                if (dup2(saved, STDOUT_FILENO) < 0) return 3;
                close(saved);
                for (unsigned repeat = 0; repeat < 2; ++repeat)
                    for (unsigned byte = 0; byte < 256; ++byte) {
                        uint8_t actual, continuation;
                        if (read(channel[0], &actual, 1) != 1 || actual != byte) return 4;
                        if (read(channel[0], &continuation, 1) != 1 || continuation != '!') return 5;
                    }
                uint8_t extra; if (read(channel[0], &extra, 1) != 0) return 6;
                close(channel[0]); return 0;
            }
        "#,
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    eprintln!("SKIP: hosted byte-output runtime requires Linux x64/AArch64 or macOS AArch64");
}

//! Test-only ABI observation of a separately instrumented validated image.

use image_emission::{ExecutableImage, ObjectArtifact};
use semantic_vocabulary::MachineId;

#[cfg(unix)]
pub(super) fn execute(
    object: &ObjectArtifact,
    image: &ExecutableImage,
    entry: MachineId,
    observer_target: MachineId,
) {
    use image_emission::{
        build_installation_record, decode_installation_record, encode_installation_record,
        validate_executable_image, validate_installation_record,
    };
    use semantic_vocabulary::ProfileDecisionId;
    use target::{Architecture, NativeTarget};

    validate_executable_image(object, image).expect("original image matches its validated object");
    let installation = build_installation_record(image, ProfileDecisionId::new(1).unwrap())
        .expect("original image has canonical installation evidence");
    let decoded = decode_installation_record(&encode_installation_record(&installation).unwrap())
        .expect("original installation round trip");
    validate_installation_record(&decoded, image).expect("original installed image binds exactly");
    if image.target() != NativeTarget::host() {
        eprintln!(
            "SKIP instrumented scalar ABI execution: target {:?}, host {:?}",
            image.target(),
            NativeTarget::host()
        );
        return;
    }

    let output = image.output();
    assert_eq!(output.final_image_imports, 0);
    assert!(output.final_data_bytes.is_empty());
    let caller = image
        .functions()
        .iter()
        .find(|function| function.machine == entry)
        .unwrap();
    let callee = image
        .functions()
        .iter()
        .find(|function| function.machine == observer_target)
        .unwrap();
    let calls = caller
        .internal_unit_calls
        .iter()
        .filter(|call| call.target == observer_target)
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("fixture must have one scalar observer call");
    };
    assert!(call.arguments.is_empty());
    assert_eq!(call.scalar_arguments.len(), 1);
    assert!(call.result.is_none() && call.structural_result.is_none());
    let sites = caller
        .unit_call_stacks
        .iter()
        .filter(|site| site.owner == call.owner && site.target == observer_target)
        .collect::<Vec<_>>();
    let [site] = sites.as_slice() else {
        panic!("observer call has one exact placed site");
    };
    let text = &output.final_text_bytes;
    let (call_offset, width) = match image.target().architecture {
        Architecture::Aarch64 => {
            let instruction = u32::from_le_bytes(
                text[site.text_offset..site.text_offset + 4]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(instruction & 0xfc00_0000, 0x9400_0000);
            let displacement = i64::from(((instruction & 0x03ff_ffff) << 6) as i32 >> 6) * 4;
            assert_eq!(
                i64::try_from(site.text_offset).unwrap() + displacement,
                i64::try_from(callee.text_offset).unwrap()
            );
            (site.text_offset, 4)
        }
        Architecture::X86_64 => {
            let call_offset = site.text_offset.checked_sub(1).unwrap();
            assert_eq!(text[call_offset], 0xe8);
            let displacement = i32::from_le_bytes(
                text[site.text_offset..site.text_offset + 4]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(
                i64::try_from(site.text_offset + 4).unwrap() + i64::from(displacement),
                i64::try_from(callee.text_offset).unwrap()
            );
            (call_offset, 5)
        }
    };
    assert!(call_offset >= caller.text_offset + call.code_offset);
    assert!(call_offset + width <= caller.text_offset + call.code_offset + call.byte_count);
    let spelling = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("0x{byte:02x}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let prefix = spelling(&text[..call_offset]);
    let suffix = spelling(&text[call_offset + width..]);
    let (entry_symbol, observer_symbol) = if cfg!(target_os = "macos") {
        ("_entry", "_observe")
    } else {
        ("entry", "observe")
    };
    let instruction = match image.target().architecture {
        Architecture::Aarch64 => "bl",
        Architecture::X86_64 => "call",
    };
    // Only this call is instrumented. The source caller's spills, argument
    // loads, cleanup boundaries, and all other internal calls remain intact.
    // This assembly is not submitted as an artifact or given installation evidence.
    let mut assembly = format!(
        ".text\n.p2align 4\n.Lomega_text:\n.byte {prefix}\n.Lobserver_probe:\n{instruction} {observer_symbol}\n.if (. - .Lobserver_probe) != {width}\n.error \"observer call changed instruction width\"\n.endif\n.byte {suffix}\n.globl {entry_symbol}\n.set {entry_symbol}, .Lomega_text + {}\n",
        caller.text_offset,
    );
    if !cfg!(target_os = "macos") {
        assembly.push_str(".section .note.GNU-stack,\"\",@progbits\n");
    }
    let driver = r#"
        #include <stdint.h>
        #include <stdio.h>
        #include <unistd.h>
        typedef struct { uint64_t left; uint64_t right; } Pair;
        extern void entry(uint64_t number, Pair value);
        static volatile uint64_t observed;
        static volatile unsigned calls;
        void observe(uint64_t number) { observed = number; ++calls; }
        int main(void) {
            alarm(10);
            const uint64_t values[] = { 0, UINT64_MAX, UINT64_C(0x5eedcafedeadbeef), UINT64_C(0x8000000000000001) };
            for (unsigned trial = 0; trial < sizeof(values) / sizeof(values[0]); ++trial) {
                const uint64_t expected = values[trial];
                Pair value = { expected ^ UINT64_C(0x13579bdf2468ace0), ~expected };
                observed = expected ^ UINT64_MAX;
                calls = 0;
                entry(expected, value);
                if (calls != 1 || observed != expected) {
                    fprintf(stderr, "trial %u: calls=%u expected=%llx observed=%llx\n", trial, calls,
                        (unsigned long long)expected, (unsigned long long)observed);
                    return 1;
                }
            }
            return 0;
        }
    "#;
    super::super::affine_call_result_host::compile_and_run(&assembly, driver);
    eprintln!(
        "executed test-only scalar call observation on {:?}; original image independently validated, instrumented copy is not a product artifact",
        image.target()
    );
}

#[cfg(not(unix))]
pub(super) fn execute(
    _object: &ObjectArtifact,
    _image: &ExecutableImage,
    _entry: MachineId,
    _observer_target: MachineId,
) {
    eprintln!("SKIP instrumented scalar ABI execution: Unix host C harness required");
}

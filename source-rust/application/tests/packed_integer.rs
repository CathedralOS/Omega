#[path = "support/observations.rs"]
mod observations;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;

fn project_source(source: &str) -> TemporaryDirectory {
    let root = support::repository_root();
    let fixture = root.join("source-rust/application/tests/fixtures/simd/positive/wrapping_add");
    let scratch = TemporaryDirectory::create("packed-integer", false).unwrap();
    fs::write(scratch.path().join("main.omg"), source).unwrap();
    let build = fs::read_to_string(fixture.join("build.omg")).unwrap();
    let library = root
        .join("source/library/simd")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        scratch.path().join("build.omg"),
        build.replace("../../../../../../../source/library/simd", &library),
    )
    .unwrap();
    scratch
}

fn project(short_input: bool) -> TemporaryDirectory {
    let root = support::repository_root();
    let fixture = root.join("source-rust/application/tests/fixtures/simd/positive/wrapping_add");
    let source = fs::read_to_string(fixture.join("main.omg")).unwrap();
    let source = if short_input {
        let short = source
            .replace("left: [i16; 8]", "left: [i16; 7]")
            .replace("5i16, -6i16];", "5i16];");
        assert_ne!(source, short);
        short
    } else {
        source
    };
    project_source(&source)
}

#[test]
fn dependent_additions_wrap_each_lane() {
    observations::assert_silent_return(|| project(false), &["build.omg"]);
}

#[test]
fn u8x16_wrapping_addition_matches_psi_and_native() {
    let source = r#"
use {simd}/simd.omg as simd;
use {simd}/u8x16.omg as u8x16;

public machine main() crashes Trap {
    let left: [u8; 16] = [255u8, 200u8, 0u8, 127u8, 128u8, 10u8, 250u8, 1u8, 99u8, 254u8, 42u8, 16u8, 240u8, 3u8, 120u8, 200u8];
    let right: [u8; 16] = [1u8, 100u8, 0u8, 1u8, 128u8, 245u8, 10u8, 255u8, 157u8, 3u8, 214u8, 240u8, 20u8, 252u8, 136u8, 56u8];
    let a: |simd|::Lanes<u8, 16> = |u8x16|::load(@left);
    let b: |simd|::Lanes<u8, 16> = |u8x16|::load(@right);
    let sum: |simd|::Lanes<u8, 16> = |u8x16|::wrapping_add(@a, @b);
    let mutable out: [u8; 16];
    |u8x16|::store(@mutable out, @sum);
    transition out[0] == 0u8 && out[1] == 44u8 && out[2] == 0u8 && out[3] == 128u8 &&
               out[4] == 0u8 && out[5] == 255u8 && out[6] == 4u8 && out[7] == 0u8 &&
               out[8] == 0u8 && out[9] == 1u8 && out[10] == 0u8 && out[11] == 0u8 &&
               out[12] == 4u8 && out[13] == 255u8 && out[14] == 0u8 && out[15] == 0u8 {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    observations::assert_silent_return(|| project_source(source), &["build.omg"]);
}

#[test]
fn i16x8_abs_min_cmpgt_and_bitops_match_psi_and_native() {
    let source = r#"
use {simd}/simd.omg as simd;
use {simd}/i16x8.omg as i16x8;

public machine main() crashes Trap {
    let a_arr: [i16; 8] = [-32768i16, -5i16, 0i16, 7i16, -1i16, 32767i16, -256i16, 255i16];
    let b_arr: [i16; 8] = [10i16, -10i16, 0i16, 12i16, 5i16, -1i16, -300i16, 255i16];
    let mask_arr: [i16; 8] = [-1i16, 0i16, 255i16, 15i16, -256i16, 255i16, -1i16, 0i16];
    let val_arr: [i16; 8] = [42i16, 42i16, 4660i16, 4660i16, 4660i16, 255i16, -123i16, -123i16];

    let a: |simd|::Lanes<i16, 8> = |i16x8|::load(@a_arr);
    let b: |simd|::Lanes<i16, 8> = |i16x8|::load(@b_arr);
    let mask: |simd|::Lanes<i16, 8> = |i16x8|::load(@mask_arr);
    let val: |simd|::Lanes<i16, 8> = |i16x8|::load(@val_arr);

    let abs_v: |simd|::Lanes<i16, 8> = |i16x8|::wrapping_abs(@a);
    let min_v: |simd|::Lanes<i16, 8> = |i16x8|::min(@a, @b);
    let cmp_v: |simd|::Lanes<i16, 8> = |i16x8|::gt_mask(@a, @b);
    let and_v: |simd|::Lanes<i16, 8> = |i16x8|::bit_and(@mask, @val);
    let andn_v: |simd|::Lanes<i16, 8> = |i16x8|::bit_and_not(@mask, @val);

    let mutable out_abs: [i16; 8];
    let mutable out_min: [i16; 8];
    let mutable out_cmp: [i16; 8];
    let mutable out_and: [i16; 8];
    let mutable out_andn: [i16; 8];

    |i16x8|::store(@mutable out_abs, @abs_v);
    |i16x8|::store(@mutable out_min, @min_v);
    |i16x8|::store(@mutable out_cmp, @cmp_v);
    |i16x8|::store(@mutable out_and, @and_v);
    |i16x8|::store(@mutable out_andn, @andn_v);

    let ok_abs: bool = out_abs[0] == -32768i16 && out_abs[1] == 5i16 && out_abs[2] == 0i16 && out_abs[3] == 7i16 &&
                       out_abs[4] == 1i16 && out_abs[5] == 32767i16 && out_abs[6] == 256i16 && out_abs[7] == 255i16;
    let ok_min: bool = out_min[0] == -32768i16 && out_min[1] == -10i16 && out_min[2] == 0i16 && out_min[3] == 7i16 &&
                       out_min[4] == -1i16 && out_min[5] == -1i16 && out_min[6] == -300i16 && out_min[7] == 255i16;
    let ok_cmp: bool = out_cmp[0] == 0i16 && out_cmp[1] == -1i16 && out_cmp[2] == 0i16 && out_cmp[3] == 0i16 &&
                       out_cmp[4] == 0i16 && out_cmp[5] == -1i16 && out_cmp[6] == -1i16 && out_cmp[7] == 0i16;
    let ok_and: bool = out_and[0] == 42i16 && out_and[1] == 0i16 && out_and[2] == 52i16 && out_and[3] == 4i16 &&
                       out_and[4] == 4608i16 && out_and[5] == 255i16 && out_and[6] == -123i16 && out_and[7] == 0i16;
    let ok_andn: bool = out_andn[0] == 0i16 && out_andn[1] == 42i16 && out_andn[2] == 4608i16 && out_andn[3] == 4656i16 &&
                        out_andn[4] == 52i16 && out_andn[5] == 0i16 && out_andn[6] == 0i16 && out_andn[7] == -123i16;

    transition ok_abs && ok_min && ok_cmp && ok_and && ok_andn {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    observations::assert_silent_return(|| project_source(source), &["build.omg"]);
}

#[test]
fn loading_eight_lanes_requires_eight_elements() {
    let (output, report) =
        observations::run(&project(true), &["run", "--engine", "psi"], &["build.omg"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("expects an array with 8 elements but the supplied array has 7"),
        "{report}"
    );
}

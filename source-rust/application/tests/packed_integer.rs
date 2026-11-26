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
        .join("source/library/system")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        scratch.path().join("build.omg"),
        build.replace("../../../../../../../source/library/system", &library),
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
use {system}/simd/simd.omg as simd;
use {system}/simd/u8x16.omg as u8x16;

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
use {system}/simd/simd.omg as simd;
use {system}/simd/i16x8.omg as i16x8;

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

// Every remaining omega::simd operation is written as checked inline asm.
// Each must agree on Psi and native, including the interleaves the
// transposes compose.
#[test]
fn shifts_widening_narrowing_and_transposes_match_psi_and_native() {
    let source = r#"
use {system}/simd/simd.omg as simd;
use {system}/simd/i16x8.omg as i16x8;
use {system}/simd/transpose.omg as transpose;

public machine main() crashes Trap {
    let a_arr: [i16; 8] = [-3i16, 100i16, -32768i16, 7i16, 1i16, 2i16, 3i16, 4i16];
    let a: |simd|::Lanes<i16, 8> = |i16x8|::load(@a_arr);
    let shifted: |simd|::Lanes<i16, 8> = |i16x8|::shift_right_arithmetic::<1>(@a);
    let one_arr: [i16; 8] = [1i16, 1i16, 1i16, 1i16, 1i16, 1i16, 1i16, 1i16];
    let one: |simd|::Lanes<i16, 8> = |i16x8|::load(@one_arr);
    let less: |simd|::Lanes<i16, 8> = |i16x8|::wrapping_sub(@a, @one);
    let bytes8: [u8; 8] = [0u8, 128u8, 255u8, 1u8, 2u8, 3u8, 4u8, 5u8];
    let widened: |simd|::Lanes<i16, 8> = |i16x8|::widen_u8x8(@bytes8);
    let bytes4: [u8; 4] = [10u8, 200u8, 255u8, 0u8];
    let widened4: |simd|::Lanes<i16, 8> = |i16x8|::widen_u8x4(@bytes4);
    let clip_arr: [i16; 8] = [-1i16, 0i16, 127i16, 255i16, 256i16, 32767i16, -32768i16, 42i16];
    let clip: |simd|::Lanes<i16, 8> = |i16x8|::load(@clip_arr);
    let narrowed: [u8; 8] = |i16x8|::saturating_narrow_u8x8(@clip);
    let narrowed4: [u8; 4] = |i16x8|::saturating_narrow_u8x4(@clip);
    let quad: [i16; 4] = [-10i16, 20i16, 300i16, -400i16];
    let low4: |simd|::Lanes<i16, 8> = |i16x8|::load_low4(@quad);
    let cat: |simd|::Lanes<i16, 8> = |i16x8|::concat_low4(@a, @widened);
    let hi: |simd|::Lanes<i16, 8> = |i16x8|::high4(@a);
    let r0: [i16; 8] = [0i16, 1i16, 2i16, 3i16, 0i16, 0i16, 0i16, 0i16];
    let r1: [i16; 8] = [10i16, 11i16, 12i16, 13i16, 0i16, 0i16, 0i16, 0i16];
    let r2: [i16; 8] = [20i16, 21i16, 22i16, 23i16, 0i16, 0i16, 0i16, 0i16];
    let r3: [i16; 8] = [30i16, 31i16, 32i16, 33i16, 0i16, 0i16, 0i16, 0i16];
    let rows: |simd|::Rows4x4 = |simd|::Rows4x4 {
        row0: |i16x8|::load(@r0), row1: |i16x8|::load(@r1),
        row2: |i16x8|::load(@r2), row3: |i16x8|::load(@r3)
    };
    let t: |simd|::Rows4x4 = |transpose|::transpose4x4(@rows);
    let p0: [i16; 8] = [0i16, 1i16, 2i16, 3i16, 4i16, 5i16, 6i16, 7i16];
    let p1: [i16; 8] = [10i16, 11i16, 12i16, 13i16, 14i16, 15i16, 16i16, 17i16];
    let p2: [i16; 8] = [20i16, 21i16, 22i16, 23i16, 24i16, 25i16, 26i16, 27i16];
    let p3: [i16; 8] = [30i16, 31i16, 32i16, 33i16, 34i16, 35i16, 36i16, 37i16];
    let prow: |simd|::Rows4x4 = |simd|::Rows4x4 {
        row0: |i16x8|::load(@p0), row1: |i16x8|::load(@p1),
        row2: |i16x8|::load(@p2), row3: |i16x8|::load(@p3)
    };
    let tp: |simd|::Rows4x4 = |transpose|::transpose4x4_pair(@prow);
    let ok_shift: bool = shifted.values[0] == -2i16 && shifted.values[1] == 50i16 && shifted.values[2] == -16384i16 && shifted.values[3] == 3i16;
    let ok_widen: bool = widened.values[1] == 128i16 && widened.values[2] == 255i16 && widened.values[7] == 5i16;
    let ok_widen4: bool = widened4.values[0] == 10i16 && widened4.values[1] == 200i16 && widened4.values[2] == 255i16 && widened4.values[4] == 0i16 && widened4.values[7] == 0i16;
    let ok_narrow: bool = narrowed[0] == 0u8 && narrowed[2] == 127u8 && narrowed[4] == 255u8 && narrowed[6] == 0u8 && narrowed[7] == 42u8;
    let ok_narrow4: bool = narrowed4[0] == 0u8 && narrowed4[1] == 0u8 && narrowed4[2] == 127u8 && narrowed4[3] == 255u8;
    let ok_low4: bool = low4.values[0] == -10i16 && low4.values[3] == -400i16 && low4.values[4] == 0i16 && low4.values[7] == 0i16;
    let ok_cat: bool = cat.values[0] == -3i16 && cat.values[3] == 7i16 && cat.values[4] == 0i16 && cat.values[5] == 128i16;
    let ok_hi: bool = hi.values[0] == 1i16 && hi.values[3] == 4i16 && hi.values[4] == 1i16 && hi.values[7] == 4i16;
    let ok_t: bool = t.row0.values[0] == 0i16 && t.row0.values[1] == 10i16 && t.row0.values[2] == 20i16 && t.row0.values[3] == 30i16 && t.row0.values[4] == 0i16
        && t.row1.values[0] == 1i16 && t.row1.values[3] == 31i16 && t.row3.values[0] == 3i16 && t.row3.values[3] == 33i16 && t.row3.values[7] == 33i16;
    let ok_tp: bool = tp.row0.values[0] == 0i16 && tp.row0.values[1] == 10i16 && tp.row0.values[3] == 30i16 && tp.row0.values[4] == 4i16 && tp.row0.values[7] == 34i16
        && tp.row1.values[0] == 1i16 && tp.row1.values[4] == 5i16 && tp.row3.values[0] == 3i16 && tp.row3.values[7] == 37i16;
    let ok_sub: bool = less.values[0] == -4i16 && less.values[2] == 32767i16 && less.values[7] == 3i16;
    transition ok_sub && ok_shift && ok_widen && ok_widen4 && ok_narrow && ok_narrow4 && ok_low4 && ok_cat && ok_hi && ok_t && ok_tp {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    observations::assert_silent_return(|| project_source(source), &["build.omg"]);
}

// The instruction contracts are reached only through `asm`; importing the
// file as ordinary core API is refused.
#[test]
fn packed_instruction_contracts_are_not_importable() {
    let source = r#"
use {omega}/language/core/assembly_packed_integer.omg as packed;

public machine main() {}
"#;
    let (output, report) = observations::run(
        &project_source(source),
        &["run", "--engine", "psi"],
        &["build.omg"],
    );
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("holds the checked asm instruction contracts"),
        "{report}"
    );
}

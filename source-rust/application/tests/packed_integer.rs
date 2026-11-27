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

/// A packed sum computed in a helper machine survives when the helper is
/// inlined and the sum is copied through more than one stack slot before it
/// is stored and read.
#[test]
fn u8x16_wrapping_addition_in_a_helper_reaches_its_caller() {
    let source = r#"
use {system}/simd/simd.omg as simd;
use {system}/simd/u8x16.omg as u8x16;

machine sum() -> u64 {
    let x: [u8; 16] = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8, 16u8];
    let y: [u8; 16] = [17u8, 18u8, 19u8, 20u8, 21u8, 22u8, 23u8, 24u8, 25u8, 26u8, 27u8, 28u8, 29u8, 30u8, 31u8, 32u8];
    let a: |simd|::Lanes<u8, 16> = |u8x16|::load(@x);
    let b: |simd|::Lanes<u8, 16> = |u8x16|::load(@y);
    let s: |simd|::Lanes<u8, 16> = |u8x16|::wrapping_add(@a, @b);
    let mutable out: [u8; 16];
    |u8x16|::store(@mutable out, @s);
    (out[0] as u64) + (out[15] as u64)
}

public machine main() crashes Trap {
    transition sum() == 66u64 {
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

// Every SIMD library operation built from more than one instruction, or from
// an instruction whose lane order matters, is checked lane by lane against a
// scalar reference computed here in Rust, on both Psi and native.
fn i16_array(values: &[i16]) -> String {
    let lanes: Vec<String> = values.iter().map(|value| format!("{value}i16")).collect();
    format!("[{}]", lanes.join(", "))
}

fn u8_array(values: &[u8]) -> String {
    let lanes: Vec<String> = values.iter().map(|value| format!("{value}u8")).collect();
    format!("[{}]", lanes.join(", "))
}

fn lane_checks(name: &str, place: &str, suffix: &str, expected: &[String]) -> String {
    let checks: Vec<String> = expected
        .iter()
        .enumerate()
        .map(|(lane, value)| format!("{place}[{lane}] == {value}{suffix}"))
        .collect();
    format!("    let {name}: bool = {};\n", checks.join(" && "))
}

fn i16_checks(name: &str, place: &str, expected: &[i16]) -> String {
    let values: Vec<String> = expected.iter().map(i16::to_string).collect();
    lane_checks(name, place, "i16", &values)
}

fn narrow(value: i16) -> u8 {
    value.clamp(0, 255) as u8
}

fn transpose(rows: &[[i16; 8]; 4], half: usize) -> [[i16; 4]; 4] {
    std::array::from_fn(|column| std::array::from_fn(|row| rows[row][half * 4 + column]))
}

#[test]
fn every_lane_of_the_simd_library_matches_a_scalar_reference_on_psi_and_native() {
    let a: [i16; 8] = [-3, 100, -32768, 7, 32767, -1, 3, -4];
    let b: [i16; 8] = [1, -100, 1, -8, 1, 32767, -32768, 4];
    let bytes8: [u8; 8] = [0, 128, 255, 1, 2, 127, 4, 200];
    let bytes4: [u8; 4] = [10, 200, 255, 0];
    let clip: [i16; 8] = [-1, 0, 127, 255, 256, 32767, -32768, 42];
    let quad: [i16; 4] = [-10, 20, 300, -400];
    let rows: [[i16; 8]; 4] =
        std::array::from_fn(|row| std::array::from_fn(|lane| (row * 10 + lane) as i16 - 17));

    let sub: Vec<i16> = (0..8).map(|lane| a[lane].wrapping_sub(b[lane])).collect();
    let add: Vec<i16> = (0..8).map(|lane| a[lane].wrapping_add(b[lane])).collect();
    let shift1: Vec<i16> = a.iter().map(|value| value >> 1).collect();
    let shift15: Vec<i16> = a.iter().map(|value| value >> 15).collect();
    let widen8: Vec<i16> = bytes8.iter().map(|&value| i16::from(value)).collect();
    let widen4: Vec<i16> = (0..8)
        .map(|lane| bytes4.get(lane).map_or(0, |&value| i16::from(value)))
        .collect();
    let narrow8: Vec<String> = clip
        .iter()
        .map(|&value| narrow(value).to_string())
        .collect();
    let narrow4: Vec<String> = clip[..4]
        .iter()
        .map(|&value| narrow(value).to_string())
        .collect();
    let low4: Vec<i16> = (0..8)
        .map(|lane| quad.get(lane).copied().unwrap_or(0))
        .collect();
    let concat: Vec<i16> = a[..4].iter().chain(&b[..4]).copied().collect();
    let high: Vec<i16> = a[4..].iter().chain(&a[4..]).copied().collect();
    let low_t = transpose(&rows, 0);
    let high_t = transpose(&rows, 1);

    // One program keeps all 21 packed results live until its final
    // transition, so native register allocation must spill the frame
    // addresses its packed stores establish.
    let mut groups: Vec<Vec<(String, String)>> = vec![
        vec![
            ("ok_sub".into(), i16_checks("ok_sub", "sub.values", &sub)),
            ("ok_add".into(), i16_checks("ok_add", "add.values", &add)),
            (
                "ok_shift1".into(),
                i16_checks("ok_shift1", "shift1.values", &shift1),
            ),
            (
                "ok_shift15".into(),
                i16_checks("ok_shift15", "shift15.values", &shift15),
            ),
            (
                "ok_widen8".into(),
                i16_checks("ok_widen8", "widen8.values", &widen8),
            ),
            (
                "ok_widen4".into(),
                i16_checks("ok_widen4", "widen4.values", &widen4),
            ),
        ],
        vec![
            (
                "ok_narrow8".into(),
                lane_checks("ok_narrow8", "narrow8", "u8", &narrow8),
            ),
            (
                "ok_narrow4".into(),
                lane_checks("ok_narrow4", "narrow4", "u8", &narrow4),
            ),
            (
                "ok_low4".into(),
                i16_checks("ok_low4", "low4.values", &low4),
            ),
            (
                "ok_concat".into(),
                i16_checks("ok_concat", "concat.values", &concat),
            ),
            (
                "ok_high".into(),
                i16_checks("ok_high", "high.values", &high),
            ),
        ],
        Vec::new(),
    ];
    for row in 0..4 {
        // The single transpose repeats its four lanes in the upper half; the
        // pair transposes the upper four columns into the upper half.
        let single: Vec<i16> = low_t[row].iter().chain(&low_t[row]).copied().collect();
        let pair: Vec<i16> = low_t[row].iter().chain(&high_t[row]).copied().collect();
        let (t, p) = (format!("ok_t{row}"), format!("ok_p{row}"));
        groups[2].push((
            t.clone(),
            i16_checks(&t, &format!("t.row{row}.values"), &single),
        ));
        groups[2].push((
            p.clone(),
            i16_checks(&p, &format!("p.row{row}.values"), &pair),
        ));
    }

    {
        let group: Vec<(String, String)> = groups.into_iter().flatten().collect();
        let checks: String = group.iter().map(|(_, check)| check.as_str()).collect();
        let verdicts: Vec<&str> = group.iter().map(|(name, _)| name.as_str()).collect();
        let source = format!(
            r#"
use {{system}}/simd/simd.omg as simd;
use {{system}}/simd/i16x8.omg as i16x8;
use {{system}}/simd/transpose.omg as transpose;

public machine main() crashes Trap {{
    let a_arr: [i16; 8] = {a};
    let b_arr: [i16; 8] = {b};
    let a: |simd|::Lanes<i16, 8> = |i16x8|::load(@a_arr);
    let b: |simd|::Lanes<i16, 8> = |i16x8|::load(@b_arr);
    let sub: |simd|::Lanes<i16, 8> = |i16x8|::wrapping_sub(@a, @b);
    let add: |simd|::Lanes<i16, 8> = |i16x8|::wrapping_add(@a, @b);
    let shift1: |simd|::Lanes<i16, 8> = |i16x8|::shift_right_arithmetic::<1>(@a);
    let shift15: |simd|::Lanes<i16, 8> = |i16x8|::shift_right_arithmetic::<15>(@a);
    let bytes8: [u8; 8] = {bytes8};
    let widen8: |simd|::Lanes<i16, 8> = |i16x8|::widen_u8x8(@bytes8);
    let bytes4: [u8; 4] = {bytes4};
    let widen4: |simd|::Lanes<i16, 8> = |i16x8|::widen_u8x4(@bytes4);
    let clip_arr: [i16; 8] = {clip};
    let clip: |simd|::Lanes<i16, 8> = |i16x8|::load(@clip_arr);
    let narrow8: [u8; 8] = |i16x8|::saturating_narrow_u8x8(@clip);
    let narrow4: [u8; 4] = |i16x8|::saturating_narrow_u8x4(@clip);
    let quad: [i16; 4] = {quad};
    let low4: |simd|::Lanes<i16, 8> = |i16x8|::load_low4(@quad);
    let concat: |simd|::Lanes<i16, 8> = |i16x8|::concat_low4(@a, @b);
    let high: |simd|::Lanes<i16, 8> = |i16x8|::high4(@a);
    let r0: [i16; 8] = {r0};
    let r1: [i16; 8] = {r1};
    let r2: [i16; 8] = {r2};
    let r3: [i16; 8] = {r3};
    let rows: |simd|::Rows4x4 = |simd|::Rows4x4 {{
        row0: |i16x8|::load(@r0), row1: |i16x8|::load(@r1),
        row2: |i16x8|::load(@r2), row3: |i16x8|::load(@r3)
    }};
    let t: |simd|::Rows4x4 = |transpose|::transpose4x4(@rows);
    let p: |simd|::Rows4x4 = |transpose|::transpose4x4_pair(@rows);
{checks}    transition {verdict} {{
        true -> done(),
        false -> wrong()
    }}
    machine done() {{}}
    machine wrong() {{ crash Trap; }}
}}
"#,
            a = i16_array(&a),
            b = i16_array(&b),
            bytes8 = u8_array(&bytes8),
            bytes4 = u8_array(&bytes4),
            clip = i16_array(&clip),
            quad = i16_array(&quad),
            r0 = i16_array(&rows[0]),
            r1 = i16_array(&rows[1]),
            r2 = i16_array(&rows[2]),
            r3 = i16_array(&rows[3]),
            verdict = verdicts.join(" && "),
        );
        observations::assert_silent_return(|| project_source(&source), &["build.omg"]);
    }
}

// Only x86-64 implements packed integer instructions. Another target refuses
// the build by naming the machine and the missing target support.
#[test]
fn packed_instructions_refuse_arm64_native_builds_clearly() {
    let scratch = project(false);
    let (output, report) =
        observations::run(&scratch, &["--target", "linux_arm64"], &["build.omg"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("uses packed integer asm instructions, which only x86_64 implements")
            && report.contains("target `linux_arm64` has no packed integer instructions yet"),
        "{report}"
    );
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

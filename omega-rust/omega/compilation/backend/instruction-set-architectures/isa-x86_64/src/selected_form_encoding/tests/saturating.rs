//! Saturating-form tests: carrier-scaled encoding agreement against the
//! independent assembler, mutation/sibling-carrier rejection, output
//! pinning away from inputs, decoded-arithmetic clamp checks on edge
//! values, and the divide guard's dividend rewrite.

use super::*;

const SATURATING_OPERATIONS: [SaturatingOperation; 5] = [
    SaturatingOperation::Add,
    SaturatingOperation::Subtract,
    SaturatingOperation::Divide,
    SaturatingOperation::Remainder,
    SaturatingOperation::Multiply,
];

fn independently_assembled_saturating_forms() -> Vec<(
    SaturatingOperation,
    SaturatingCarrier,
    Vec<&'static str>,
    Vec<u8>,
)> {
    let signed_narrow_low = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1, 0x48, 0xb9]);
        bytes.extend(minimum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4c, 0xc1]);
        bytes
    };
    let signed_narrow_high = |maximum: [u8; 8], minimum: [u8; 8], arithmetic: u8| {
        let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x49, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1, 0x49, 0xb9]);
        bytes.extend(minimum);
        bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4c, 0xd1]);
        bytes
    };
    let unsigned_narrow_low = |maximum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x01, 0xf0, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1]);
        bytes
    };
    let signed_narrow_divide_low = |maximum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x99, 0x48, 0xf7, 0xfe, 0x48, 0xba];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2]);
        bytes
    };
    let i8_maximum = [0x7f, 0, 0, 0, 0, 0, 0, 0];
    let i8_minimum = [0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let i16_maximum = [0xff, 0x7f, 0, 0, 0, 0, 0, 0];
    let i16_minimum = [0x00, 0x80, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let i32_maximum = [0xff, 0xff, 0xff, 0x7f, 0, 0, 0, 0];
    let i32_minimum = [0x00, 0x00, 0x00, 0x80, 0xff, 0xff, 0xff, 0xff];
    let low4 = vec!["rdi", "rsi", "rax", "rcx"];
    let high4 = vec!["r8", "r11", "r10", "r9"];
    let low3 = vec!["rdi", "rsi", "rax"];
    let high3 = vec!["r8", "r9", "r10"];
    let low_divide = vec!["rax", "rsi", "rax", "rdx"];
    let high_divide = vec!["rax", "r9", "rax", "rdx"];
    let i64_overflow_select_low = |arithmetic: u8| {
        vec![
            0x48, 0x89, 0xf9, 0x48, 0xf7, 0xd1, 0x48, 0xc1, 0xf9, 0x3f, 0x48, 0x0f, 0xba, 0xf9,
            0x3f, 0x48, 0x89, 0xf8, 0x48, arithmetic, 0xf0, 0x48, 0x0f, 0x40, 0xc1,
        ]
    };
    let i64_overflow_select_high = |arithmetic: u8| {
        vec![
            0x4d, 0x89, 0xc1, 0x49, 0xf7, 0xd1, 0x49, 0xc1, 0xf9, 0x3f, 0x49, 0x0f, 0xba, 0xf9,
            0x3f, 0x4d, 0x89, 0xc2, 0x4d, arithmetic, 0xda, 0x4d, 0x0f, 0x40, 0xd1,
        ]
    };
    let unsigned_subtract_low = vec![
        0x48, 0x39, 0xf7, 0x48, 0x89, 0xf8, 0x48, 0x0f, 0x42, 0xc6, 0x48, 0x29, 0xf0,
    ];
    let unsigned_subtract_high = vec![
        0x4d, 0x39, 0xc8, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0x42, 0xd1, 0x4d, 0x29, 0xca,
    ];
    // clang: mov rax, rdi; imul rax, rsi; then the signed narrow clamps.
    let signed_narrow_multiply_low = |maximum: [u8; 8], minimum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4f, 0xc1, 0x48, 0xb9]);
        bytes.extend(minimum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x4c, 0xc1]);
        bytes
    };
    // clang: mov rax, rdi; imul rax, rsi; movabs rcx, MAX; cmp rax, rcx;
    // cmova rax, rcx.
    let unsigned_narrow_multiply_low = |maximum: [u8; 8]| {
        let mut bytes = vec![0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0xb9];
        bytes.extend(maximum);
        bytes.extend([0x48, 0x39, 0xc8, 0x48, 0x0f, 0x47, 0xc1]);
        bytes
    };
    use SaturatingCarrier::*;
    use SaturatingOperation::*;
    vec![
        (
            Multiply,
            I8,
            low4.clone(),
            signed_narrow_multiply_low(i8_maximum, i8_minimum),
        ),
        // The i8 clang bytes with the i16 bound immediates substituted.
        (
            Multiply,
            I16,
            low4.clone(),
            signed_narrow_multiply_low(i16_maximum, i16_minimum),
        ),
        // clang: mov r10, r8; imul r10, r11; movabs r9, 0x7fffffff; cmp r10,
        // r9; cmovg r10, r9; movabs r9, -0x80000000; cmp r10, r9; cmovl r10, r9.
        (Multiply, I32, high4.clone(), {
            let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd3, 0x49, 0xb9];
            bytes.extend(i32_maximum);
            bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1, 0x49, 0xb9]);
            bytes.extend(i32_minimum);
            bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4c, 0xd1]);
            bytes
        }),
        (
            Multiply,
            U32,
            low4.clone(),
            unsigned_narrow_multiply_low([0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]),
        ),
        // The u32 clang bytes with the u16 bound substituted.
        (
            Multiply,
            U16,
            low4.clone(),
            unsigned_narrow_multiply_low([0xff, 0xff, 0, 0, 0, 0, 0, 0]),
        ),
        // clang: mov r10, r8; imul r10, r11; movabs r9, 0xff; cmp r10, r9;
        // cmova r10, r9.
        (Multiply, U8, high4.clone(), {
            let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd3, 0x49, 0xb9];
            bytes.extend([0xff, 0, 0, 0, 0, 0, 0, 0]);
            bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x47, 0xd1]);
            bytes
        }),
        // clang: mov rcx, rdi; xor rcx, rsi; not rcx; sar rcx, 63; btc rcx,
        // 63; mov rax, rdi; imul rax, rsi; cmovo rax, rcx.
        (
            Multiply,
            I64,
            low4.clone(),
            vec![
                0x48, 0x89, 0xf9, 0x48, 0x31, 0xf1, 0x48, 0xf7, 0xd1, 0x48, 0xc1, 0xf9, 0x3f, 0x48,
                0x0f, 0xba, 0xf9, 0x3f, 0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0x0f, 0x40,
                0xc1,
            ],
        ),
        // clang: the same sequence on r8, r11, r10, r9.
        (
            Multiply,
            I64,
            high4.clone(),
            vec![
                0x4d, 0x89, 0xc1, 0x4d, 0x31, 0xd9, 0x49, 0xf7, 0xd1, 0x49, 0xc1, 0xf9, 0x3f, 0x49,
                0x0f, 0xba, 0xf9, 0x3f, 0x4d, 0x89, 0xc2, 0x4d, 0x0f, 0xaf, 0xd3, 0x4d, 0x0f, 0x40,
                0xd1,
            ],
        ),
        // clang: mul rsi; sbb rdx, rdx; or rax, rdx.
        (
            Multiply,
            U64,
            low_divide.clone(),
            vec![0x48, 0xf7, 0xe6, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xd0],
        ),
        // clang: mul r9; sbb rdx, rdx; or rax, rdx.
        (
            Multiply,
            U64,
            high_divide.clone(),
            vec![0x49, 0xf7, 0xe1, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xd0],
        ),
        // clang: mov rax, rdi; add rax, rsi; movabs rcx, 0x7f; cmp rax, rcx;
        // cmovg rax, rcx; movabs rcx, -0x80; cmp rax, rcx; cmovl rax, rcx.
        (
            Add,
            I8,
            low4.clone(),
            signed_narrow_low(i8_maximum, i8_minimum, 0x01),
        ),
        // The i8 clang bytes with the i16 bound immediates substituted.
        (
            Add,
            I16,
            low4.clone(),
            signed_narrow_low(i16_maximum, i16_minimum, 0x01),
        ),
        (
            Add,
            I32,
            low4.clone(),
            signed_narrow_low(i32_maximum, i32_minimum, 0x01),
        ),
        // The i8 clang bytes with `sub` (0x29) in place of `add` (0x01).
        (
            Subtract,
            I8,
            low4.clone(),
            signed_narrow_low(i8_maximum, i8_minimum, 0x29),
        ),
        (
            Subtract,
            I16,
            low4.clone(),
            signed_narrow_low(i16_maximum, i16_minimum, 0x29),
        ),
        (
            Subtract,
            I32,
            low4.clone(),
            signed_narrow_low(i32_maximum, i32_minimum, 0x29),
        ),
        // The i64 clang high assignment (r8, r11, r10, r9) with the narrow
        // clamp sequence in place of the overflow select.
        (
            Add,
            I8,
            high4.clone(),
            signed_narrow_high(i8_maximum, i8_minimum, 0x01),
        ),
        (
            Subtract,
            I16,
            high4.clone(),
            signed_narrow_high(i16_maximum, i16_minimum, 0x29),
        ),
        // clang: mov rax, rdi; add rax, rsi; movabs rcx, 0xffffffff; cmp rax,
        // rcx; cmovg rax, rcx.
        (
            Add,
            U32,
            low4.clone(),
            unsigned_narrow_low([0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]),
        ),
        (
            Add,
            U8,
            low4.clone(),
            unsigned_narrow_low([0xff, 0, 0, 0, 0, 0, 0, 0]),
        ),
        (
            Add,
            U16,
            low4.clone(),
            unsigned_narrow_low([0xff, 0xff, 0, 0, 0, 0, 0, 0]),
        ),
        // The clang high assignment of the signed narrow add with the u16
        // bound and no lower clamp.
        (Add, U16, high4.clone(), {
            let mut bytes = vec![0x4d, 0x89, 0xc2, 0x4d, 0x01, 0xda, 0x49, 0xb9];
            bytes.extend([0xff, 0xff, 0, 0, 0, 0, 0, 0]);
            bytes.extend([0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x4f, 0xd1]);
            bytes
        }),
        // clang: mov rcx, rdi; not rcx; sar rcx, 63; btc rcx, 63; mov rax,
        // rdi; add rax, rsi; cmovo rax, rcx.
        (Add, I64, low4.clone(), i64_overflow_select_low(0x01)),
        (Add, I64, high4.clone(), i64_overflow_select_high(0x01)),
        // clang: the i64 add with sub rax, rsi.
        (Subtract, I64, low4.clone(), i64_overflow_select_low(0x29)),
        (Subtract, I64, high4.clone(), i64_overflow_select_high(0x29)),
        // clang: cmp rdi, rsi; mov rax, rdi; cmovb rax, rsi; sub rax, rsi
        // for every unsigned carrier.
        (Subtract, U8, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U16, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U32, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U64, low3.clone(), unsigned_subtract_low.clone()),
        (Subtract, U32, high3.clone(), unsigned_subtract_high.clone()),
        (Subtract, U64, high3.clone(), unsigned_subtract_high),
        // clang: the u64 complement/borrow-select add on r8, r9, r10.
        (
            Add,
            U64,
            high3,
            vec![
                0x4d, 0x89, 0xc2, 0x49, 0xf7, 0xd2, 0x4d, 0x39, 0xca, 0x4d, 0x0f, 0x42, 0xd1, 0x4d,
                0x29, 0xca, 0x49, 0xf7, 0xd2,
            ],
        ),
        (
            Add,
            U64,
            low3,
            vec![
                0x48, 0x89, 0xf8, 0x48, 0xf7, 0xd0, 0x48, 0x39, 0xf0, 0x48, 0x0f, 0x42, 0xc6, 0x48,
                0x29, 0xf0, 0x48, 0xf7, 0xd0,
            ],
        ),
        // clang: div rsi / div r9 for every unsigned carrier.
        (Divide, U8, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
        (Divide, U64, low_divide.clone(), vec![0x48, 0xf7, 0xf6]),
        (Divide, U16, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
        (Divide, U32, high_divide.clone(), vec![0x49, 0xf7, 0xf1]),
        // clang: cqo; idiv rsi; movabs rdx, 0x7fff; cmp rax, rdx; cmovg rax, rdx.
        (
            Divide,
            I16,
            low_divide.clone(),
            signed_narrow_divide_low(i16_maximum),
        ),
        (
            Divide,
            I8,
            low_divide.clone(),
            signed_narrow_divide_low(i8_maximum),
        ),
        (
            Divide,
            I32,
            low_divide.clone(),
            signed_narrow_divide_low(i32_maximum),
        ),
        // clang: cqo; idiv r9; movabs rdx, 0x7fffffff; cmp rax, rdx; cmovg rax, rdx.
        (
            Divide,
            I32,
            high_divide.clone(),
            vec![
                0x48, 0x99, 0x49, 0xf7, 0xf9, 0x48, 0xba, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x00, 0x00,
                0x00, 0x48, 0x39, 0xd0, 0x48, 0x0f, 0x4f, 0xc2,
            ],
        ),
        // clang: cmp rsi, -1; sbb rdx, rdx; or rdx, rax; neg rdx; lea rdx,
        // [rax + 1]; cmovo rax, rdx; cqo; idiv rsi.
        (
            Divide,
            I64,
            low_divide.clone(),
            vec![
                0x48, 0x83, 0xfe, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7, 0xda, 0x48,
                0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x48, 0xf7, 0xfe,
            ],
        ),
        (
            Divide,
            I64,
            high_divide.clone(),
            vec![
                0x49, 0x83, 0xf9, 0xff, 0x48, 0x19, 0xd2, 0x48, 0x09, 0xc2, 0x48, 0xf7, 0xda, 0x48,
                0x8d, 0x50, 0x01, 0x48, 0x0f, 0x40, 0xc2, 0x48, 0x99, 0x49, 0xf7, 0xf9,
            ],
        ),
        // clang: xor rdx, rdx; div rsi; mov rax, rdx for every unsigned
        // carrier — the zeroed high half cannot overflow the quotient.
        (
            Remainder,
            U8,
            low_divide.clone(),
            vec![0x48, 0x31, 0xd2, 0x48, 0xf7, 0xf6, 0x48, 0x89, 0xd0],
        ),
        (
            Remainder,
            U32,
            low_divide.clone(),
            vec![0x48, 0x31, 0xd2, 0x48, 0xf7, 0xf6, 0x48, 0x89, 0xd0],
        ),
        // clang: xor rdx, rdx; div r9; mov rax, rdx.
        (
            Remainder,
            U16,
            high_divide.clone(),
            vec![0x48, 0x31, 0xd2, 0x49, 0xf7, 0xf1, 0x48, 0x89, 0xd0],
        ),
        (
            Remainder,
            U64,
            high_divide.clone(),
            vec![0x48, 0x31, 0xd2, 0x49, 0xf7, 0xf1, 0x48, 0x89, 0xd0],
        ),
        // clang: xor rdx, rdx; cmp rsi, -1; je over cqo/idiv; mov rax, rdx
        // for every signed carrier — x % -1 is zero for every dividend, so
        // skipping IDIV leaves the pre-cleared zero and avoids the MIN / -1
        // quotient fault.
        (
            Remainder,
            I8,
            low_divide.clone(),
            vec![
                0x48, 0x31, 0xd2, 0x48, 0x83, 0xfe, 0xff, 0x74, 0x05, 0x48, 0x99, 0x48, 0xf7, 0xfe,
                0x48, 0x89, 0xd0,
            ],
        ),
        (
            Remainder,
            I32,
            low_divide.clone(),
            vec![
                0x48, 0x31, 0xd2, 0x48, 0x83, 0xfe, 0xff, 0x74, 0x05, 0x48, 0x99, 0x48, 0xf7, 0xfe,
                0x48, 0x89, 0xd0,
            ],
        ),
        (
            Remainder,
            I16,
            high_divide.clone(),
            vec![
                0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49, 0xf7, 0xf9,
                0x48, 0x89, 0xd0,
            ],
        ),
        (
            Remainder,
            I64,
            high_divide,
            vec![
                0x48, 0x31, 0xd2, 0x49, 0x83, 0xf9, 0xff, 0x74, 0x05, 0x48, 0x99, 0x49, 0xf7, 0xf9,
                0x48, 0x89, 0xd0,
            ],
        ),
    ]
}

#[test]
fn saturating_forms_match_independent_assembler_for_every_carrier() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let mut covered = Vec::new();
    for (operation, carrier, names, expected) in independently_assembled_saturating_forms() {
        let kind = saturating_kind(operation, carrier);
        let key = saturating_key(operation, carrier);
        let operands = saturating_operands(&physical, &names);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(
            encoded.bytes(),
            expected,
            "{operation:?} {carrier:?} {names:?}"
        );
        assert_eq!(
            expected.len(),
            usize::from(SaturatingForm::of(operation, carrier).byte_count()),
            "{operation:?} {carrier:?}"
        );
        validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &expected).unwrap();
        let high = names.iter().any(|name| {
            name[1..]
                .chars()
                .all(|character| character.is_ascii_digit())
        });
        covered.push((
            operation,
            carrier,
            SaturatingForm::of(operation, carrier),
            high,
        ));
    }
    // Every carrier of every operation has an independently assembled form,
    // and every realization shape has a low and an r8-r15 assignment.
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            assert!(
                covered
                    .iter()
                    .any(|(o, c, ..)| (*o, *c) == (operation, carrier)),
                "{operation:?} {carrier:?} has no independently assembled bytes"
            );
        }
    }
    for form in [
        SaturatingForm::AddU64,
        SaturatingForm::SubtractUnsigned,
        SaturatingForm::DivideUnsigned,
        SaturatingForm::ClampSignedNarrow,
        SaturatingForm::ClampUnsignedNarrow,
        SaturatingForm::OverflowSelectI64,
        SaturatingForm::DivideSignedNarrow,
        SaturatingForm::DivideI64,
        SaturatingForm::RemainderUnsigned,
        SaturatingForm::RemainderSigned,
        SaturatingForm::MultiplySignedNarrow,
        SaturatingForm::MultiplyUnsignedNarrow,
        SaturatingForm::MultiplyI64,
        SaturatingForm::MultiplyU64,
    ] {
        for high in [false, true] {
            assert!(
                covered.iter().any(|(_, _, f, h)| (*f, *h) == (form, high)),
                "{form:?} high={high} has no independently assembled bytes"
            );
        }
    }
}

#[test]
fn saturating_forms_reject_every_mutation_substitution_and_sibling_carrier() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let substitute = physical.model().view_named("r14").unwrap().id;
    let mut canonical = Vec::new();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            for high in [false, true] {
                let kind = saturating_kind(operation, carrier);
                let key = saturating_key(operation, carrier);
                let operands = saturating_operands(
                    &physical,
                    &saturating_operand_names(operation, carrier, high),
                );
                let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
                let form = SaturatingForm::of(operation, carrier);
                assert_eq!(encoded.bytes().len(), usize::from(form.byte_count()));
                let (reads, writes) = form.operand_reads_and_writes();
                assert!(encoded.footprint().writes_rflags);
                assert_eq!(encoded.footprint().encoded.external_operand_reads, reads);
                assert_eq!(encoded.footprint().encoded.external_operand_writes, writes);
                assert_eq!(
                    encoded.footprint().register_reads,
                    reads
                        .iter()
                        .map(|&position| operands[usize::from(position)])
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    encoded.footprint().register_writes,
                    writes
                        .iter()
                        .map(|&position| operands[usize::from(position)])
                        .collect::<Vec<_>>()
                );
                let rdx = physical.model().view_named("rdx").unwrap().units.clone();
                let clobbers = &encoded.footprint().encoded.implicit_unit_clobbers;
                assert!(!clobbers.is_empty());
                // Only the divide forms implicitly clobber RDX: their row
                // declares it a Use whose incoming value CQO discards while
                // DIV still writes the pair's high half. The remainder rows
                // declare RDX an explicit Def, so the write is operand
                // custody rather than an implicit clobber.
                let rdx_is_implicit_clobber = matches!(
                    form,
                    SaturatingForm::DivideUnsigned
                        | SaturatingForm::DivideSignedNarrow
                        | SaturatingForm::DivideI64
                );
                assert_eq!(
                    rdx.iter().all(|unit| clobbers.contains(unit)),
                    rdx_is_implicit_clobber,
                    "{operation:?} {carrier:?} RDX clobber"
                );
                assert_eq!(
                    encoded.footprint().encoded.trap
                        == MachineEncodedTrapBehavior::MayArchitecturalFaultV1,
                    form.is_division()
                );
                assert_every_bit_change_rejected(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                    &format!("{operation:?} {carrier:?} high={high}"),
                );
                assert_every_operand_substitution_rejected(
                    &physical,
                    kind,
                    key,
                    &operands,
                    substitute,
                    encoded.bytes(),
                    &format!("{operation:?} {carrier:?}"),
                );
                if !high {
                    canonical.push((operation, carrier, kind, key, operands, encoded));
                }
            }
        }
    }
    // Bytes replayed under another carrier's kind (of any operation) are
    // rejected wherever that carrier's realization differs. The unsigned
    // subtract, unsigned divide, and the two remainder forms are shared
    // verbatim across carriers of their sign class, so those are the only
    // replays that can be accepted here.
    for (operation, carrier, _, _, _, encoded) in &canonical {
        for (other_operation, other_carrier, other_kind, other_key, other_operands, other) in
            &canonical
        {
            if (operation, carrier) == (other_operation, other_carrier) {
                continue;
            }
            let accepted = validate_x86_64_selected_form_encoding(
                &physical,
                *other_kind,
                *other_key,
                other_operands,
                encoded.bytes(),
            )
            .is_ok();
            let shared = operation == other_operation
                && encoded.bytes() == other.bytes()
                && matches!(
                    SaturatingForm::of(*operation, *carrier),
                    SaturatingForm::SubtractUnsigned
                        | SaturatingForm::DivideUnsigned
                        | SaturatingForm::RemainderUnsigned
                        | SaturatingForm::RemainderSigned
                );
            assert_eq!(
                accepted, shared,
                "{operation:?} {carrier:?} bytes replayed as {other_operation:?} {other_carrier:?}"
            );
        }
    }
}

#[test]
fn saturating_forms_pin_outputs_away_from_inputs_and_division_to_rax_rdx() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            let kind = saturating_kind(operation, carrier);
            let key = saturating_key(operation, carrier);
            let form = SaturatingForm::of(operation, carrier);
            let invalid: Vec<Vec<&str>> = if form.is_fixed_rax_rdx() {
                vec![
                    vec!["rax", "rdx", "rax", "rdx"],
                    vec!["rcx", "r9", "rax", "rdx"],
                    vec!["rax", "r9", "rcx", "rdx"],
                    vec!["rax", "r9", "rax", "rcx"],
                ]
            } else if form.operand_count() == 4 {
                // The result accumulates and the scratch holds the bound (or
                // the saturated value) while both inputs are still live.
                vec![
                    vec!["r9", "r10", "r9", "r12"],
                    vec!["r9", "r10", "r10", "r12"],
                    vec!["r9", "r10", "r11", "r9"],
                    vec!["r9", "r10", "r11", "r10"],
                    vec!["r9", "r10", "r11", "r11"],
                ]
            } else {
                vec![vec!["r9", "r10", "r9"], vec!["r9", "r10", "r10"]]
            };
            for names in invalid {
                let operands = saturating_operands(&physical, &names);
                assert!(
                    encode_x86_64_selected_form(&physical, kind, key, &operands).is_err(),
                    "{operation:?} {carrier:?} {names:?}"
                );
            }
            let wrong_count = saturating_operand_names(operation, carrier, false);
            let mut too_few = saturating_operands(&physical, &wrong_count);
            too_few.pop();
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &too_few),
                Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
            );
        }
    }
}

/// The RFLAGS bits the saturating and Trapping forms read. `None` is an
/// architecturally undefined flag (after SAR, BTC, a variable shift, or a
/// division), so a form that consumed it would panic here instead of passing
/// by accident.
/// Executes one decoded saturating form over a 16-register file, returning
/// the register file. Register values are 64-bit patterns.
fn execute_saturating(bytes: &[u8], registers: [u64; 16]) -> [u64; 16] {
    match execute_decoded(bytes, registers) {
        DecodedOutcome::Completed(registers) => registers,
        DecodedOutcome::Trapped => panic!("a saturating form never traps"),
    }
}

/// Executes one decoded form over a 16-register file. Every jump must land
/// on an instruction boundary inside the form or exactly at its end; a UD2
/// ends execution as a trap, and a divide the form failed to guard panics
/// instead of faulting.

fn saturating_reference(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    left: i128,
    right: i128,
) -> u64 {
    macro_rules! reference {
        ($type:ty) => {{
            let left = <$type>::try_from(left).unwrap();
            let right = <$type>::try_from(right).unwrap();
            let result = match operation {
                SaturatingOperation::Add => left.saturating_add(right),
                SaturatingOperation::Subtract => left.saturating_sub(right),
                SaturatingOperation::Divide => left.saturating_div(right),
                // A mathematical remainder already lies inside its carrier,
                // so saturating remainder is the wrapping remainder.
                SaturatingOperation::Remainder => left.wrapping_rem(right),
                SaturatingOperation::Multiply => left.saturating_mul(right),
            };
            // Truncating the lossless i128 widening to 64 bits yields the
            // sign- or zero-normalized register pattern for every carrier.
            i128::from(result) as u64
        }};
    }
    match carrier {
        SaturatingCarrier::I8 => reference!(i8),
        SaturatingCarrier::I16 => reference!(i16),
        SaturatingCarrier::I32 => reference!(i32),
        SaturatingCarrier::I64 => reference!(i64),
        SaturatingCarrier::U8 => reference!(u8),
        SaturatingCarrier::U16 => reference!(u16),
        SaturatingCarrier::U32 => reference!(u32),
        SaturatingCarrier::U64 => reference!(u64),
    }
}

#[test]
fn saturating_decoded_arithmetic_clamps_every_carrier_edge() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for operation in SATURATING_OPERATIONS {
        for carrier in SaturatingCarrier::ALL {
            let minimum = i128::from(carrier.minimum_bits() as i64);
            let maximum = i128::from(carrier.maximum_bits());
            let in_range = |value: i128| (minimum..=maximum).contains(&value);
            // MAX + 1, MIN - 1, MAX + MAX, MIN + MIN, 0 - 1, MIN / -1, and
            // in-range controls; pairs outside the carrier are skipped, as is
            // a zero divisor, which no carrier licenses.
            let inputs = [
                (maximum, 1),
                (minimum, -1),
                (maximum, maximum),
                (minimum, minimum),
                (minimum, 1),
                (maximum, -1),
                (minimum, maximum),
                (maximum, minimum),
                (0, 1),
                (0, -1),
                (1, 2),
                (2, 7),
                (7, 2),
                (-7, 2),
                (7, -2),
                (maximum, 0),
                (maximum, 2),
                (minimum, 2),
                (1, maximum),
                (maximum / 2, maximum / 2 + 1),
            ];
            let mut checked = 0;
            for high in [false, true] {
                let names = saturating_operand_names(operation, carrier, high);
                let operands = saturating_operands(&physical, &names);
                let encoded = encode_x86_64_selected_form(
                    &physical,
                    saturating_kind(operation, carrier),
                    saturating_key(operation, carrier),
                    &operands,
                )
                .unwrap();
                let registers =
                    crate::selected_form_encoding::request_validation::resolve_registers(
                        &physical, &operands,
                    )
                    .unwrap();
                let (left_home, right_home, result_home) = (
                    usize::from(registers[0]),
                    usize::from(registers[1]),
                    usize::from(registers[2]),
                );
                for (left, right) in inputs {
                    if !in_range(left)
                        || !in_range(right)
                        || (matches!(
                            operation,
                            SaturatingOperation::Divide | SaturatingOperation::Remainder
                        ) && right == 0)
                    {
                        continue;
                    }
                    let mut file = [0_u64; 16];
                    file[left_home] = left as u64;
                    file[right_home] = right as u64;
                    let file = execute_saturating(encoded.bytes(), file);
                    assert_eq!(
                        file[result_home],
                        saturating_reference(operation, carrier, left, right),
                        "{operation:?} {carrier:?} high={high} {left} {right}"
                    );
                    assert_eq!(file[right_home], right as u64, "{operation:?} {carrier:?}");
                    checked += 1;
                }
            }
            assert!(
                checked >= 16,
                "{operation:?} {carrier:?} checked only {checked} pairs"
            );
        }
    }
}

#[test]
fn saturating_i64_divide_guard_only_rewrites_the_faulting_dividend() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let operands = saturating_operands(&physical, &["rax", "rsi", "rax", "rdx"]);
    let encoded = encode_x86_64_selected_form(
        &physical,
        saturating_kind(SaturatingOperation::Divide, SaturatingCarrier::I64),
        saturating_key(SaturatingOperation::Divide, SaturatingCarrier::I64),
        &operands,
    )
    .unwrap();
    for (dividend, divisor, quotient, remainder) in [
        (i64::MIN, -1, i64::MAX, 0),
        (i64::MIN, 1, i64::MIN, 0),
        (i64::MIN, 2, i64::MIN / 2, 0),
        (i64::MAX, -1, -i64::MAX, 0),
        (i64::MIN + 1, -1, i64::MAX, 0),
        (-1, -1, 1, 0),
        (7, -2, -3, 1),
        (-7, 2, -3, -1),
    ] {
        let mut file = [0_u64; 16];
        file[0] = dividend as u64;
        file[6] = divisor as u64;
        let file = execute_saturating(encoded.bytes(), file);
        assert_eq!(file[0] as i64, quotient, "{dividend} / {divisor}");
        assert_eq!(file[2] as i64, remainder, "{dividend} % {divisor}");
    }
}

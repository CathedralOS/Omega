//! Trapping-form tests: independent-assembler agreement, mutation and
//! wrong-request rejection, row pinning and early-clobber custody,
//! same-shape replay, and decoded-form trap predicates against the
//! executed reference.

use super::*;

/// Apple clang (`clang -target x86_64-apple-macos -c`, bytes read back with
/// `otool -t`) assembled every Trapping realization from hand-written Intel
/// assembly, independently of this encoder, in `TrappingForm::ALL` order for
/// the low register assignment of `trapping_operand_names`. Every check jumps
/// to a local label just past its UD2, which the assembler emits as a
/// two-byte rel8 jump:
///
/// * i64 and u64 add, subtract, and multiply: `mov rax, rdi; add|sub|imul
///   rax, rsi; jno|jae 1f; ud2`.
/// * narrow add, subtract, and multiply: the same arithmetic, then `movsx
///   rcx, al|ax` or `movsxd rcx, eax; cmp rcx, rax` (signed) or `mov rcx,
///   rax; shr rcx, 8|16|32` (unsigned), then `je 1f; ud2`.
/// * u64 multiply: `mul rcx; jno 1f; ud2`.
/// * divide and remainder: `test rcx, rcx; jne 1f; ud2`, then `xor rdx, rdx;
///   div rcx` (unsigned) or `cmp rcx, -1; jne 2f; cmp rax, -128|-32768|
///   -2147483648; jne 3f; ud2; 3: 2: cqo; idiv rcx` (signed; i64 compares
///   `cmp rax, 1; jno 3f`), then `mov rax, rdx` for a remainder.
/// * shifts: `cmp rcx, 8|16|32|64; jb 1f; ud2; 1: mov rax, rdi`, then `sar|shr
///   rax, cl` (right), or `shl rax, cl` and either the narrow range check of
///   rax through rdx or `mov rdx, rax; sar|shr rdx, cl; cmp rdx, rdi; je 2f;
///   ud2` (i64, u64).
/// * conversions: `movsx rcx, dil|di` or `movsxd rcx, edi; cmp rcx, rdi`
///   (signed source into a narrow signed carrier) or `mov rcx, rdi; shr rcx,
///   7|8|15|16|31|32|63` (every other checked pair), then `je 1f; ud2; 1: mov
///   rax, rdi`; or the bare `mov rax, rdi`.
const CLANG_TRAPPING_LOW: [&str; 72] = [
    // Add
    "4889f84801f0480fbec84839c174020f0b", // I8
    "4889f84801f0480fbfc84839c174020f0b", // I16
    "4889f84801f04863c84839c174020f0b",   // I32
    "4889f84801f071020f0b",               // I64
    "4889f84801f04889c148c1e90874020f0b", // U8
    "4889f84801f04889c148c1e91074020f0b", // U16
    "4889f84801f04889c148c1e92074020f0b", // U32
    "4889f84801f073020f0b",               // U64
    // Subtract
    "4889f84829f0480fbec84839c174020f0b", // I8
    "4889f84829f0480fbfc84839c174020f0b", // I16
    "4889f84829f04863c84839c174020f0b",   // I32
    "4889f84829f071020f0b",               // I64
    "4889f84829f04889c148c1e90874020f0b", // U8
    "4889f84829f04889c148c1e91074020f0b", // U16
    "4889f84829f04889c148c1e92074020f0b", // U32
    "4889f84829f073020f0b",               // U64
    // Multiply
    "4889f8480fafc6480fbec84839c174020f0b", // I8
    "4889f8480fafc6480fbfc84839c174020f0b", // I16
    "4889f8480fafc64863c84839c174020f0b",   // I32
    "4889f8480fafc671020f0b",               // I64
    "4889f8480fafc64889c148c1e90874020f0b", // U8
    "4889f8480fafc64889c148c1e91074020f0b", // U16
    "4889f8480fafc64889c148c1e92074020f0b", // U32
    "48f7e171020f0b",                       // U64
    // Divide
    "4885c975020f0b4883f9ff75084883f88075020f0b489948f7f9", // I8
    "4885c975020f0b4883f9ff750a483d0080ffff75020f0b489948f7f9", // I16
    "4885c975020f0b4883f9ff750a483d0000008075020f0b489948f7f9", // I32
    "4885c975020f0b4883f9ff75084883f80171020f0b489948f7f9", // I64
    "4885c975020f0b4831d248f7f1",                           // U8
    "4885c975020f0b4831d248f7f1",                           // U16
    "4885c975020f0b4831d248f7f1",                           // U32
    "4885c975020f0b4831d248f7f1",                           // U64
    // Remainder
    "4885c975020f0b4883f9ff75084883f88075020f0b489948f7f94889d0", // I8
    "4885c975020f0b4883f9ff750a483d0080ffff75020f0b489948f7f94889d0", // I16
    "4885c975020f0b4883f9ff750a483d0000008075020f0b489948f7f94889d0", // I32
    "4885c975020f0b4883f9ff75084883f80171020f0b489948f7f94889d0", // I64
    "4885c975020f0b4831d248f7f14889d0",                           // U8
    "4885c975020f0b4831d248f7f14889d0",                           // U16
    "4885c975020f0b4831d248f7f14889d0",                           // U32
    "4885c975020f0b4831d248f7f14889d0",                           // U64
    // ShiftLeft
    "4883f90872020f0b4889f848d3e0480fbed04839c274020f0b", // I8
    "4883f91072020f0b4889f848d3e0480fbfd04839c274020f0b", // I16
    "4883f92072020f0b4889f848d3e04863d04839c274020f0b",   // I32
    "4883f94072020f0b4889f848d3e04889c248d3fa4839fa74020f0b", // I64
    "4883f90872020f0b4889f848d3e04889c248c1ea0874020f0b", // U8
    "4883f91072020f0b4889f848d3e04889c248c1ea1074020f0b", // U16
    "4883f92072020f0b4889f848d3e04889c248c1ea2074020f0b", // U32
    "4883f94072020f0b4889f848d3e04889c248d3ea4839fa74020f0b", // U64
    // ShiftRight
    "4883f90872020f0b4889f848d3f8", // I8
    "4883f91072020f0b4889f848d3f8", // I16
    "4883f92072020f0b4889f848d3f8", // I32
    "4883f94072020f0b4889f848d3f8", // I64
    "4883f90872020f0b4889f848d3e8", // U8
    "4883f91072020f0b4889f848d3e8", // U16
    "4883f92072020f0b4889f848d3e8", // U32
    "4883f94072020f0b4889f848d3e8", // U64
    // Convert { Signed }
    "480fbecf4839f974020f0b4889f8", // I8
    "480fbfcf4839f974020f0b4889f8", // I16
    "4863cf4839f974020f0b4889f8",   // I32
    "4889f8",                       // I64
    "4889f948c1e90874020f0b4889f8", // U8
    "4889f948c1e91074020f0b4889f8", // U16
    "4889f948c1e92074020f0b4889f8", // U32
    "4889f948c1e93f74020f0b4889f8", // U64
    // Convert { Unsigned }
    "4889f948c1e90774020f0b4889f8", // I8
    "4889f948c1e90f74020f0b4889f8", // I16
    "4889f948c1e91f74020f0b4889f8", // I32
    "4889f948c1e93f74020f0b4889f8", // I64
    "4889f948c1e90874020f0b4889f8", // U8
    "4889f948c1e91074020f0b4889f8", // U16
    "4889f948c1e92074020f0b4889f8", // U32
    "4889f8",                       // U64
];

/// The same assembly with every movable operand on r8 through r11 (`[r8,
/// r11, r10, r9]`, `[r8, rcx, r10, r9]`, `[r8, r10, r9]`, and the divisor on
/// r9 of `[rax, r9, rax, rdx]`), assembled by Apple clang in the same way to
/// pin REX.R and REX.B.
const CLANG_TRAPPING_HIGH: [&str; 72] = [
    // Add
    "4d89c24d01da4d0fbeca4d39d174020f0b", // I8
    "4d89c24d01da4d0fbfca4d39d174020f0b", // I16
    "4d89c24d01da4d63ca4d39d174020f0b",   // I32
    "4d89c24d01da71020f0b",               // I64
    "4d89c24d01da4d89d149c1e90874020f0b", // U8
    "4d89c24d01da4d89d149c1e91074020f0b", // U16
    "4d89c24d01da4d89d149c1e92074020f0b", // U32
    "4d89c24d01da73020f0b",               // U64
    // Subtract
    "4d89c24d29da4d0fbeca4d39d174020f0b", // I8
    "4d89c24d29da4d0fbfca4d39d174020f0b", // I16
    "4d89c24d29da4d63ca4d39d174020f0b",   // I32
    "4d89c24d29da71020f0b",               // I64
    "4d89c24d29da4d89d149c1e90874020f0b", // U8
    "4d89c24d29da4d89d149c1e91074020f0b", // U16
    "4d89c24d29da4d89d149c1e92074020f0b", // U32
    "4d89c24d29da73020f0b",               // U64
    // Multiply
    "4d89c24d0fafd34d0fbeca4d39d174020f0b", // I8
    "4d89c24d0fafd34d0fbfca4d39d174020f0b", // I16
    "4d89c24d0fafd34d63ca4d39d174020f0b",   // I32
    "4d89c24d0fafd371020f0b",               // I64
    "4d89c24d0fafd34d89d149c1e90874020f0b", // U8
    "4d89c24d0fafd34d89d149c1e91074020f0b", // U16
    "4d89c24d0fafd34d89d149c1e92074020f0b", // U32
    "49f7e171020f0b",                       // U64
    // Divide
    "4d85c975020f0b4983f9ff75084883f88075020f0b489949f7f9", // I8
    "4d85c975020f0b4983f9ff750a483d0080ffff75020f0b489949f7f9", // I16
    "4d85c975020f0b4983f9ff750a483d0000008075020f0b489949f7f9", // I32
    "4d85c975020f0b4983f9ff75084883f80171020f0b489949f7f9", // I64
    "4d85c975020f0b4831d249f7f1",                           // U8
    "4d85c975020f0b4831d249f7f1",                           // U16
    "4d85c975020f0b4831d249f7f1",                           // U32
    "4d85c975020f0b4831d249f7f1",                           // U64
    // Remainder
    "4d85c975020f0b4983f9ff75084883f88075020f0b489949f7f94889d0", // I8
    "4d85c975020f0b4983f9ff750a483d0080ffff75020f0b489949f7f94889d0", // I16
    "4d85c975020f0b4983f9ff750a483d0000008075020f0b489949f7f94889d0", // I32
    "4d85c975020f0b4983f9ff75084883f80171020f0b489949f7f94889d0", // I64
    "4d85c975020f0b4831d249f7f14889d0",                           // U8
    "4d85c975020f0b4831d249f7f14889d0",                           // U16
    "4d85c975020f0b4831d249f7f14889d0",                           // U32
    "4d85c975020f0b4831d249f7f14889d0",                           // U64
    // ShiftLeft
    "4883f90872020f0b4d89c249d3e24d0fbeca4d39d174020f0b", // I8
    "4883f91072020f0b4d89c249d3e24d0fbfca4d39d174020f0b", // I16
    "4883f92072020f0b4d89c249d3e24d63ca4d39d174020f0b",   // I32
    "4883f94072020f0b4d89c249d3e24d89d149d3f94d39c174020f0b", // I64
    "4883f90872020f0b4d89c249d3e24d89d149c1e90874020f0b", // U8
    "4883f91072020f0b4d89c249d3e24d89d149c1e91074020f0b", // U16
    "4883f92072020f0b4d89c249d3e24d89d149c1e92074020f0b", // U32
    "4883f94072020f0b4d89c249d3e24d89d149d3e94d39c174020f0b", // U64
    // ShiftRight
    "4883f90872020f0b4d89c249d3fa", // I8
    "4883f91072020f0b4d89c249d3fa", // I16
    "4883f92072020f0b4d89c249d3fa", // I32
    "4883f94072020f0b4d89c249d3fa", // I64
    "4883f90872020f0b4d89c249d3ea", // U8
    "4883f91072020f0b4d89c249d3ea", // U16
    "4883f92072020f0b4d89c249d3ea", // U32
    "4883f94072020f0b4d89c249d3ea", // U64
    // Convert { Signed }
    "4d0fbec84d39c174020f0b4d89c2", // I8
    "4d0fbfc84d39c174020f0b4d89c2", // I16
    "4d63c84d39c174020f0b4d89c2",   // I32
    "4d89c2",                       // I64
    "4d89c149c1e90874020f0b4d89c2", // U8
    "4d89c149c1e91074020f0b4d89c2", // U16
    "4d89c149c1e92074020f0b4d89c2", // U32
    "4d89c149c1e93f74020f0b4d89c2", // U64
    // Convert { Unsigned }
    "4d89c149c1e90774020f0b4d89c2", // I8
    "4d89c149c1e90f74020f0b4d89c2", // I16
    "4d89c149c1e91f74020f0b4d89c2", // I32
    "4d89c149c1e93f74020f0b4d89c2", // I64
    "4d89c149c1e90874020f0b4d89c2", // U8
    "4d89c149c1e91074020f0b4d89c2", // U16
    "4d89c149c1e92074020f0b4d89c2", // U32
    "4d89c2",                       // U64
];

fn trapping_kind(form: TrappingForm) -> SelectedInstructionKind {
    SelectedInstructionKind::TrappingInteger { form }
}

fn trapping_key(form: TrappingForm) -> MachineAlternativeKey {
    alternative(MachineAlternativeFamily::TrappingInteger(form), 0)
}

/// The register assignment the clang tables were assembled for: the low one
/// on RAX through RDI, the high one on r8 through r11 wherever the row lets
/// an operand move (the fixed divide row keeps RAX and RDX, and the shift
/// count stays on RCX).
fn trapping_operand_names(form: TrappingForm, high: bool) -> Vec<&'static str> {
    match (TrappingShape::of(form).row(), high) {
        (TrappingRow::FixedPair, _) => vec!["rax", if high { "r9" } else { "rcx" }, "rax", "rdx"],
        (TrappingRow::Binary, false) => vec!["rdi", "rsi", "rax", "rcx"],
        (TrappingRow::Binary, true) => vec!["r8", "r11", "r10", "r9"],
        (TrappingRow::Shift, false) => vec!["rdi", "rcx", "rax", "rdx"],
        (TrappingRow::Shift, true) => vec!["r8", "rcx", "r10", "r9"],
        (TrappingRow::Convert, false) => vec!["rdi", "rax", "rcx"],
        (TrappingRow::Convert, true) => vec!["r8", "r10", "r9"],
    }
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

/// Shapes whose byte sequence never names the scratch operand: the 64-bit
/// flag checks, the right shift, and the identity conversions. Their
/// scratch is still a declared definition of the row.
fn trapping_scratch_unused(form: TrappingForm) -> bool {
    matches!(
        TrappingShape::of(form),
        TrappingShape::OverflowFlag(_)
            | TrappingShape::CarryFlag(_)
            | TrappingShape::ShiftRight { .. }
            | TrappingShape::Convert(None)
    )
}

#[test]
fn trapping_forms_match_independent_assembler_for_every_form() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let constraints = crate::validate_x86_64_register_constraint_catalog(
        crate::x86_64_register_constraint_catalog(&physical),
        &physical,
    )
    .unwrap();
    let catalog =
        crate::x86_64_machine_effect_catalog(target::NativeTarget::linux_x64(), &constraints)
            .unwrap();
    let rflags = physical.model().view_named("rflags").unwrap().units.clone();
    for (index, form) in TrappingForm::ALL.into_iter().enumerate() {
        let shape = TrappingShape::of(form);
        let declared = &catalog
            .declarations
            .iter()
            .find(|row| row.semantic == MachineSemanticKind::TrappingInteger(form))
            .unwrap()
            .alternatives[0];
        for (high, table) in [(false, &CLANG_TRAPPING_LOW), (true, &CLANG_TRAPPING_HIGH)] {
            let names = trapping_operand_names(form, high);
            let operands = saturating_operands(&physical, &names);
            let expected = hex_bytes(table[index]);
            let encoded = encode_x86_64_selected_form(
                &physical,
                trapping_kind(form),
                trapping_key(form),
                &operands,
            )
            .unwrap();
            assert_eq!(encoded.bytes(), expected, "{form:?} {names:?}");
            assert_eq!(expected.len(), usize::from(shape.byte_count()), "{form:?}");
            assert_eq!(
                declared.size,
                selected_instructions::MachineSizeKnowledge::ExactBytes(shape.byte_count())
            );
            validate_x86_64_selected_form_encoding(
                &physical,
                trapping_kind(form),
                trapping_key(form),
                &operands,
                &expected,
            )
            .unwrap();
            // The footprint restates the catalog row: the row's reads and
            // writes, an RFLAGS clobber, and the inline trap surface.
            let footprint = encoded.footprint();
            let (reads, writes) = shape.operand_reads_and_writes();
            assert_eq!(
                footprint.register_reads,
                reads
                    .iter()
                    .map(|&position| operands[usize::from(position)])
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                footprint.register_writes,
                writes
                    .iter()
                    .map(|&position| operands[usize::from(position)])
                    .collect::<Vec<_>>()
            );
            assert!(footprint.writes_rflags);
            assert_eq!(footprint.encoded, declared.encoded, "{form:?}");
            assert_eq!(footprint.encoded.implicit_unit_clobbers, rflags);
            assert_eq!(
                footprint.encoded.trap,
                MachineEncodedTrapBehavior::TrappingIntegerV1
            );
            assert_eq!(
                footprint.encoded.control,
                MachineEncodedControlEffect::FallThroughOrTrapV1
            );
            assert_eq!(footprint.encoded.memory, MachineEncodedMemoryEffect::NoneV1);
            assert_eq!(
                footprint.encoded.stack,
                MachineEncodedStackEffect::UnchangedV1
            );
            // Every form that can trap carries the Crash leaf's UD2 bytes;
            // the identity conversions carry none.
            let ud2 = expected.windows(2).any(|pair| *pair == [0x0f, 0x0b]);
            assert_eq!(ud2, shape != TrappingShape::Convert(None), "{form:?}");
        }
    }
}

#[test]
fn trapping_forms_reject_every_mutation_substitution_and_wrong_request() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let view = |name: &str| physical.model().view_named(name).unwrap().id;
    let substitute = view("r14");
    for form in TrappingForm::ALL {
        let kind = trapping_kind(form);
        let key = trapping_key(form);
        for high in [false, true] {
            let operands = saturating_operands(&physical, &trapping_operand_names(form, high));
            let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
            assert_every_bit_change_rejected(
                &physical,
                kind,
                key,
                &operands,
                encoded.bytes(),
                &format!("{form:?} high={high}"),
            );
            let mut truncated = encoded.bytes().to_vec();
            truncated.pop();
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &truncated)
                    .is_err()
            );
            // Only a scratch the bytes never name may be substituted.
            let scratch = operands.len() - 1;
            for operand_position in 0..operands.len() {
                let mut changed = operands.clone();
                changed[operand_position] = substitute;
                assert_eq!(
                    validate_x86_64_selected_form_encoding(
                        &physical,
                        kind,
                        key,
                        &changed,
                        encoded.bytes()
                    )
                    .is_ok(),
                    operand_position == scratch && trapping_scratch_unused(form),
                    "{form:?} high={high} substituted operand {operand_position}"
                );
            }
        }
        let operands = saturating_operands(&physical, &trapping_operand_names(form, false));
        let bytes = encode_x86_64_selected_form(&physical, kind, key, &operands)
            .unwrap()
            .bytes()
            .to_vec();
        let sibling = TrappingForm::ALL[(usize::from(form.ordinal()) + 1) % TrappingForm::COUNT];
        for wrong in [
            alternative(MachineAlternativeFamily::TrappingInteger(form), 1),
            trapping_key(sibling),
            saturating_key(SaturatingOperation::Add, form.carrier),
        ] {
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, wrong, &operands),
                Err(X86_64SelectedFormEncodingError::AlternativeMismatch),
                "{form:?} {wrong:?}"
            );
            assert_eq!(
                validate_x86_64_selected_form_encoding(&physical, kind, wrong, &operands, &bytes),
                Err(X86_64SelectedFormEncodingError::AlternativeMismatch)
            );
        }
        let mut too_few = operands.clone();
        too_few.pop();
        let mut too_many = operands.clone();
        too_many.push(view("r12"));
        for wrong_count in [too_few, too_many] {
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &wrong_count),
                Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
            );
            assert_eq!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &wrong_count, &bytes),
                Err(X86_64SelectedFormEncodingError::OperandCountMismatch)
            );
        }
    }
}

#[test]
fn trapping_forms_pin_rows_and_keep_early_clobbers_off_inputs() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for form in TrappingForm::ALL {
        let kind = trapping_kind(form);
        let key = trapping_key(form);
        let bytes = encode_x86_64_selected_form(
            &physical,
            kind,
            key,
            &saturating_operands(&physical, &trapping_operand_names(form, false)),
        )
        .unwrap()
        .bytes()
        .to_vec();
        // Inputs may share a home (`x + x`, `x << x`, `x / x`); an output may
        // share neither an input nor the other output, and the fixed pins
        // hold.
        let (invalid, valid): (Vec<Vec<&str>>, Vec<&str>) = match TrappingShape::of(form).row() {
            TrappingRow::Binary => (
                vec![
                    vec!["r9", "r10", "r9", "r12"],
                    vec!["r9", "r10", "r10", "r12"],
                    vec!["r9", "r10", "r11", "r9"],
                    vec!["r9", "r10", "r11", "r10"],
                    vec!["r9", "r10", "r11", "r11"],
                ],
                vec!["r9", "r9", "r10", "r11"],
            ),
            TrappingRow::Shift => (
                vec![
                    vec!["r9", "r10", "r11", "r12"],
                    vec!["r9", "rcx", "r9", "r12"],
                    vec!["r9", "rcx", "rcx", "r12"],
                    vec!["r9", "rcx", "r11", "r9"],
                    vec!["r9", "rcx", "r11", "rcx"],
                    vec!["r9", "rcx", "r11", "r11"],
                ],
                vec!["rcx", "rcx", "r10", "r11"],
            ),
            TrappingRow::Convert => (
                vec![
                    vec!["r9", "r9", "r12"],
                    vec!["r9", "r10", "r9"],
                    vec!["r9", "r10", "r10"],
                ],
                vec!["r9", "r10", "r11"],
            ),
            TrappingRow::FixedPair => (
                vec![
                    vec!["rax", "rdx", "rax", "rdx"],
                    vec!["rcx", "r9", "rax", "rdx"],
                    vec!["rax", "r9", "rcx", "rdx"],
                    vec!["rax", "r9", "rax", "rcx"],
                ],
                vec!["rax", "rax", "rax", "rdx"],
            ),
        };
        for names in invalid {
            let operands = saturating_operands(&physical, &names);
            assert_eq!(
                encode_x86_64_selected_form(&physical, kind, key, &operands),
                Err(X86_64SelectedFormEncodingError::EncodedFormMismatch),
                "{form:?} {names:?}"
            );
            assert!(
                validate_x86_64_selected_form_encoding(&physical, kind, key, &operands, &bytes)
                    .is_err()
            );
        }
        let operands = saturating_operands(&physical, &valid);
        let encoded = encode_x86_64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(
            encoded.bytes().len(),
            usize::from(TrappingShape::of(form).byte_count())
        );
    }
}

#[test]
fn trapping_bytes_replay_only_under_a_form_of_the_same_shape() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let canonical = TrappingForm::ALL.map(|form| {
        let operands = saturating_operands(&physical, &trapping_operand_names(form, false));
        let bytes = encode_x86_64_selected_form(
            &physical,
            trapping_kind(form),
            trapping_key(form),
            &operands,
        )
        .unwrap()
        .bytes()
        .to_vec();
        (form, operands, bytes)
    });
    let mut accepted = Vec::new();
    for (form, _, bytes) in &canonical {
        for (other, operands, _) in &canonical {
            if form != other
                && validate_x86_64_selected_form_encoding(
                    &physical,
                    trapping_kind(*other),
                    trapping_key(*other),
                    operands,
                    bytes,
                )
                .is_ok()
            {
                accepted.push((*form, *other));
            }
        }
    }
    // Unsigned division shares one realization across carriers, and a
    // conversion check depends only on the boundary bit it tests: a signed
    // or unsigned source enters u8, u16, and u32 below the same bit, a
    // signed source into u64 and an unsigned one into i64 both test bit 63,
    // and the two identity conversions are the same copy.
    let convert = |source, carrier| TrappingForm {
        operation: TrappingOperation::Convert { source },
        carrier,
    };
    let mut groups: Vec<Vec<TrappingForm>> =
        [TrappingOperation::Divide, TrappingOperation::Remainder]
            .map(|operation| {
                [
                    SaturatingCarrier::U8,
                    SaturatingCarrier::U16,
                    SaturatingCarrier::U32,
                    SaturatingCarrier::U64,
                ]
                .map(|carrier| TrappingForm { operation, carrier })
                .to_vec()
            })
            .to_vec();
    for carrier in [
        SaturatingCarrier::U8,
        SaturatingCarrier::U16,
        SaturatingCarrier::U32,
    ] {
        groups.push(vec![
            convert(IntegerSign::Signed, carrier),
            convert(IntegerSign::Unsigned, carrier),
        ]);
    }
    groups.push(vec![
        convert(IntegerSign::Signed, SaturatingCarrier::U64),
        convert(IntegerSign::Unsigned, SaturatingCarrier::I64),
    ]);
    groups.push(vec![
        convert(IntegerSign::Signed, SaturatingCarrier::I64),
        convert(IntegerSign::Unsigned, SaturatingCarrier::U64),
    ]);
    let mut expected = Vec::new();
    for group in &groups {
        for form in group {
            for other in group {
                if form != other {
                    expected.push((*form, *other));
                }
            }
        }
    }
    accepted.sort();
    expected.sort();
    assert_eq!(accepted, expected);
    for (form, other) in &accepted {
        assert_eq!(TrappingShape::of(*form), TrappingShape::of(*other));
    }
}

/// The exact Trapping result of `form` on mathematical operands as the
/// normalized 64-bit register pattern, or `None` when the policy predicate
/// holds and the form must trap: a result outside the carrier, a zero
/// divisor, a signed `MIN / -1` or `MIN % -1`, a shift count outside
/// `0..width`, or an unrepresentable conversion.
fn trapping_reference(form: TrappingForm, left: i128, right: i128) -> Option<u64> {
    let carrier = form.carrier;
    let minimum = i128::from(carrier.minimum_bits() as i64);
    let maximum = i128::from(carrier.maximum_bits());
    let width = i128::from(carrier.bits());
    let signed_overflow = carrier.is_signed() && left == minimum && right == -1;
    let exact = match form.operation {
        TrappingOperation::Add => left + right,
        TrappingOperation::Subtract => left - right,
        // Only a u64 product can pass i128, and it lies outside every carrier.
        TrappingOperation::Multiply => left.checked_mul(right)?,
        TrappingOperation::Divide if right == 0 || signed_overflow => return None,
        TrappingOperation::Divide => left / right,
        TrappingOperation::Remainder if right == 0 || signed_overflow => return None,
        TrappingOperation::Remainder => left % right,
        TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight
            if !(0..width).contains(&right) =>
        {
            return None;
        }
        TrappingOperation::ShiftLeft => left * (1_i128 << right),
        // i128 `>>` floors like SAR; an unsigned value is non-negative.
        TrappingOperation::ShiftRight => left >> right,
        TrappingOperation::Convert { .. } => left,
    };
    (minimum..=maximum).contains(&exact).then_some(exact as u64)
}

/// The carrier's boundary values and their neighbours, including the
/// square-root pair that brackets multiplicative overflow.
fn trapping_edge_values(carrier: SaturatingCarrier) -> Vec<i128> {
    let minimum = i128::from(carrier.minimum_bits() as i64);
    let maximum = i128::from(carrier.maximum_bits());
    let mut root = (maximum as f64).sqrt() as i128;
    while root * root > maximum {
        root -= 1;
    }
    while (root + 1) * (root + 1) <= maximum {
        root += 1;
    }
    let mut values = vec![
        minimum,
        minimum + 1,
        minimum / 2,
        minimum / 2 - 1,
        -2,
        -1,
        0,
        1,
        2,
        3,
        root,
        root + 1,
        -root,
        -root - 1,
        maximum / 2,
        maximum / 2 + 1,
        maximum - 1,
        maximum,
    ];
    values.retain(|value| (minimum..=maximum).contains(value));
    values.sort_unstable();
    values.dedup();
    values
}

/// The operand pairs each form executes: the carrier's edge values against
/// each other, shifts against counts on both sides of the width (and
/// negative or huge counts), and conversions from every source boundary.
fn trapping_inputs(form: TrappingForm) -> Vec<(i128, i128)> {
    let values = trapping_edge_values(form.carrier);
    match form.operation {
        TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
            let width = i128::from(form.carrier.bits());
            let counts = [
                0,
                1,
                width - 1,
                width,
                width + 1,
                63,
                64,
                -1,
                i128::from(i64::MIN),
                i128::from(u64::MAX),
            ];
            values
                .iter()
                .flat_map(|value| counts.map(|count| (*value, count)))
                .collect()
        }
        TrappingOperation::Convert { source } => {
            let sources: Vec<i128> = if source == IntegerSign::Signed {
                [
                    i128::from(i64::MIN),
                    i128::from(i64::MIN) + 1,
                    i128::from(i32::MIN) - 1,
                    i128::from(i32::MIN),
                    i128::from(i16::MIN) - 1,
                    i128::from(i16::MIN),
                    -129,
                    -128,
                    -1,
                    0,
                    1,
                    127,
                    128,
                    255,
                    256,
                    32767,
                    32768,
                    65535,
                    65536,
                    i128::from(i32::MAX),
                    i128::from(i32::MAX) + 1,
                    i128::from(u32::MAX),
                    i128::from(u32::MAX) + 1,
                    i128::from(i64::MAX),
                ]
                .to_vec()
            } else {
                [
                    0,
                    1,
                    127,
                    128,
                    255,
                    256,
                    32767,
                    32768,
                    65535,
                    65536,
                    i128::from(i32::MAX),
                    i128::from(i32::MAX) + 1,
                    i128::from(u32::MAX),
                    i128::from(u32::MAX) + 1,
                    i128::from(i64::MAX),
                    i128::from(i64::MAX) + 1,
                    i128::from(u64::MAX),
                ]
                .to_vec()
            };
            sources.into_iter().map(|source| (source, 0)).collect()
        }
        _ => values
            .iter()
            .flat_map(|left| values.iter().map(|right| (*left, *right)))
            .collect(),
    }
}

/// One encoded Trapping form and the register homes of its operands.
struct TrappingProgram {
    form: TrappingForm,
    bytes: Vec<u8>,
    homes: Vec<usize>,
}

impl TrappingProgram {
    fn encode(
        physical: &register_model::ValidatedPhysicalRegisterModel,
        form: TrappingForm,
        high: bool,
    ) -> Self {
        let operands = saturating_operands(physical, &trapping_operand_names(form, high));
        let bytes = encode_x86_64_selected_form(
            physical,
            trapping_kind(form),
            trapping_key(form),
            &operands,
        )
        .unwrap()
        .bytes()
        .to_vec();
        let homes = crate::selected_form_encoding::request_validation::resolve_registers(
            physical, &operands,
        )
        .unwrap()
        .into_iter()
        .map(usize::from)
        .collect();
        Self { form, bytes, homes }
    }

    /// Executes the form on one operand pair, with every other register
    /// holding a poison pattern, and checks that the input homes it does not
    /// define survive. `None` is a trap.
    fn execute(&self, left: i128, right: i128) -> Option<u64> {
        let sources = self.form.source_count();
        let mut file = [0xa5a5_5a5a_a5a5_5a5a_u64; 16];
        file[self.homes[0]] = left as u64;
        if sources == 2 {
            file[self.homes[1]] = right as u64;
        }
        let initial = file;
        match execute_decoded(&self.bytes, file) {
            DecodedOutcome::Trapped => None,
            DecodedOutcome::Completed(file) => {
                for input in &self.homes[..sources] {
                    if !self.homes[sources..].contains(input) {
                        assert_eq!(file[*input], initial[*input], "{:?} input", self.form);
                    }
                }
                Some(file[self.homes[sources]])
            }
        }
    }
}

#[test]
fn trapping_decoded_forms_trap_exactly_when_the_policy_predicate_holds() {
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    for form in TrappingForm::ALL {
        let programs = [false, true].map(|high| TrappingProgram::encode(&physical, form, high));
        let (mut trapped, mut completed) = (0, 0);
        for (left, right) in trapping_inputs(form) {
            let expected = trapping_reference(form, left, right);
            for program in &programs {
                assert_eq!(
                    program.execute(left, right),
                    expected,
                    "{form:?} {:?} {left} {right}",
                    program.homes
                );
            }
            if expected.is_some() {
                completed += 1;
            } else {
                trapped += 1;
            }
        }
        // Every form both traps and completes at its edges, except the two
        // identity conversions, which every normalized source satisfies.
        let identity = TrappingShape::of(form) == TrappingShape::Convert(None);
        assert_eq!(trapped > 0, !identity, "{form:?} trapped {trapped}");
        assert!(completed > 0, "{form:?} completed {completed}");
    }
}

#[test]
fn trapping_decoded_forms_decide_the_named_boundary_cases() {
    use SaturatingCarrier::{I8, I16, I32, I64, U8, U16, U32, U64};
    use TrappingOperation::{Add, Divide, Multiply, Remainder, ShiftLeft, ShiftRight, Subtract};
    let physical = validate_physical_register_model(x86_64_physical_register_model()).unwrap();
    let form = |operation, carrier| TrappingForm { operation, carrier };
    let convert = |source, carrier| TrappingForm {
        operation: TrappingOperation::Convert { source },
        carrier,
    };
    let (signed, unsigned) = (IntegerSign::Signed, IntegerSign::Unsigned);
    let cases: Vec<(TrappingForm, i128, i128, Option<i128>)> = vec![
        // Add and subtract at MIN/MAX and one step past them.
        (form(Add, I8), 127, 0, Some(127)),
        (form(Add, I8), 127, 1, None),
        (form(Add, I8), -128, -1, None),
        (form(Subtract, I16), -32768, 0, Some(-32768)),
        (form(Subtract, I16), -32768, 1, None),
        (form(Add, I32), i128::from(i32::MAX), 1, None),
        (form(Add, I64), i128::from(i64::MAX), 1, None),
        (form(Add, I64), i128::from(i64::MIN), -1, None),
        (
            form(Add, I64),
            i128::from(i64::MIN),
            i128::from(i64::MAX),
            Some(-1),
        ),
        (form(Subtract, I64), i128::from(i64::MIN), 1, None),
        (
            form(Subtract, I64),
            -1,
            i128::from(i64::MIN),
            Some(i128::from(i64::MAX)),
        ),
        (form(Add, U8), 255, 0, Some(255)),
        (form(Add, U8), 255, 1, None),
        (form(Subtract, U32), 0, 1, None),
        (form(Subtract, U32), 1, 1, Some(0)),
        (form(Add, U64), i128::from(u64::MAX), 1, None),
        (
            form(Add, U64),
            i128::from(u64::MAX) - 1,
            1,
            Some(i128::from(u64::MAX)),
        ),
        (form(Subtract, U64), 0, 1, None),
        // Products just inside and just outside every class.
        (form(Multiply, I8), -128, 1, Some(-128)),
        (form(Multiply, I8), -128, -1, None),
        (form(Multiply, I32), 46341, 46341, None),
        (form(Multiply, I32), 46340, 46340, Some(46340 * 46340)),
        (form(Multiply, U32), 65536, 65536, None),
        (
            form(Multiply, U32),
            65535,
            65537,
            Some(i128::from(u32::MAX)),
        ),
        (form(Multiply, I64), i128::from(i64::MIN), -1, None),
        (
            form(Multiply, I64),
            i128::from(i64::MIN),
            1,
            Some(i128::from(i64::MIN)),
        ),
        (form(Multiply, U64), 1 << 32, 1 << 32, None),
        (
            form(Multiply, U64),
            (1 << 32) - 1,
            (1 << 32) + 1,
            Some(i128::from(u64::MAX)),
        ),
        // Zero divisors and the one overflowing quotient pair.
        (form(Divide, U8), 7, 0, None),
        (form(Remainder, U64), 7, 0, None),
        (form(Divide, I64), 0, 0, None),
        (form(Divide, I8), -128, -1, None),
        (form(Divide, I8), -128, 1, Some(-128)),
        (form(Divide, I8), -1, -1, Some(1)),
        (form(Remainder, I8), -128, -1, None),
        (form(Remainder, I8), -128, 1, Some(0)),
        (form(Divide, I16), -32768, -1, None),
        (form(Divide, I32), i128::from(i32::MIN), -1, None),
        (
            form(Divide, I32),
            i128::from(i32::MIN),
            1,
            Some(i128::from(i32::MIN)),
        ),
        (form(Divide, I64), i128::from(i64::MIN), -1, None),
        (
            form(Divide, I64),
            i128::from(i64::MIN),
            1,
            Some(i128::from(i64::MIN)),
        ),
        (form(Divide, I64), -1, -1, Some(1)),
        (form(Remainder, I64), i128::from(i64::MIN), -1, None),
        (form(Remainder, I64), i128::from(i64::MIN) + 1, -1, Some(0)),
        (form(Remainder, I32), -7, 2, Some(-1)),
        (
            form(Divide, U64),
            i128::from(u64::MAX),
            1,
            Some(i128::from(u64::MAX)),
        ),
        // Counts at width - 1 and width, negative counts, and value overflow.
        (form(ShiftLeft, I8), -1, 7, Some(-128)),
        (form(ShiftLeft, I8), 1, 7, None),
        (form(ShiftLeft, I8), 1, 8, None),
        (form(ShiftLeft, I8), 1, -1, None),
        (form(ShiftLeft, U8), 1, 7, Some(128)),
        (form(ShiftLeft, U8), 2, 7, None),
        (form(ShiftLeft, U16), 1, 16, None),
        (form(ShiftLeft, I32), 1, 30, Some(1 << 30)),
        (form(ShiftLeft, I32), 1, 31, None),
        (form(ShiftLeft, U32), 1, 31, Some(1 << 31)),
        (form(ShiftLeft, U32), 3, 31, None),
        (form(ShiftLeft, I64), -1, 63, Some(i128::from(i64::MIN))),
        (form(ShiftLeft, I64), 1, 63, None),
        (form(ShiftLeft, I64), 1, 64, None),
        (form(ShiftLeft, I64), 1, i128::from(i64::MIN), None),
        (form(ShiftLeft, U64), 1, 63, Some(1 << 63)),
        (form(ShiftLeft, U64), 2, 63, None),
        (form(ShiftRight, I32), -1, 31, Some(-1)),
        (form(ShiftRight, I32), -1, 32, None),
        (form(ShiftRight, I16), -32768, 15, Some(-1)),
        (form(ShiftRight, U8), 255, 7, Some(1)),
        (form(ShiftRight, U8), 255, -1, None),
        (form(ShiftRight, U64), i128::from(u64::MAX), 63, Some(1)),
        (form(ShiftRight, I64), i128::from(i64::MIN), 63, Some(-1)),
        (form(ShiftRight, I64), 1, 64, None),
        // Conversions at the destination bounds from both source signs.
        (convert(unsigned, I8), i128::from(u64::MAX), 0, None),
        (convert(unsigned, I8), 127, 0, Some(127)),
        (convert(unsigned, I8), 128, 0, None),
        (convert(signed, I8), -128, 0, Some(-128)),
        (convert(signed, I8), -129, 0, None),
        (convert(signed, I8), 128, 0, None),
        (convert(signed, I16), -32769, 0, None),
        (
            convert(signed, I32),
            i128::from(i32::MIN),
            0,
            Some(i128::from(i32::MIN)),
        ),
        (convert(signed, I32), i128::from(i32::MAX) + 1, 0, None),
        (convert(signed, U64), -1, 0, None),
        (
            convert(signed, U64),
            i128::from(i64::MAX),
            0,
            Some(i128::from(i64::MAX)),
        ),
        (convert(signed, U8), -1, 0, None),
        (convert(signed, U8), 255, 0, Some(255)),
        (convert(signed, U8), 256, 0, None),
        (convert(unsigned, U16), 65536, 0, None),
        (
            convert(unsigned, U32),
            i128::from(u32::MAX),
            0,
            Some(i128::from(u32::MAX)),
        ),
        (convert(unsigned, U32), 1 << 32, 0, None),
        (convert(unsigned, I64), 1 << 63, 0, None),
        (
            convert(unsigned, I64),
            (1 << 63) - 1,
            0,
            Some(i128::from(i64::MAX)),
        ),
        (
            convert(signed, I64),
            i128::from(i64::MIN),
            0,
            Some(i128::from(i64::MIN)),
        ),
        (
            convert(unsigned, U64),
            i128::from(u64::MAX),
            0,
            Some(i128::from(u64::MAX)),
        ),
    ];
    for (form, left, right, expected) in cases {
        let expected = expected.map(|value| value as u64);
        assert_eq!(
            trapping_reference(form, left, right),
            expected,
            "reference {form:?} {left} {right}"
        );
        for high in [false, true] {
            assert_eq!(
                TrappingProgram::encode(&physical, form, high).execute(left, right),
                expected,
                "{form:?} high={high} {left} {right}"
            );
        }
    }
}

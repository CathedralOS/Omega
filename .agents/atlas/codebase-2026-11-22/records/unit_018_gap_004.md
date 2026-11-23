# Record: unit 018, gap 004 (second pass)

Area: `11-native-stages-11-15`. Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d0559`.
One file, one range: `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`, lines 7001-13000 (6000 lines). Below, `L` means that file. The file is 21917 lines (`wc`-style count by Grep), so this range is the middle band: it begins inside the test module of `ieee_float_arithmetic` and ends inside the tests of `memory::frame`. Module starts were listed with a Grep of `mod` declarations; the headings below follow that list.

The file's own header (L:1-8) says it encodes, structurally decodes and validates all x86-64 selected forms. The two public entry points are outside my range: `encode_x86_64_selected_form` at L:21797 and `validate_x86_64_selected_form_encoding` at L:21841 (grepped, not read). Every module in my range is a per-family encoder plus an independent decoder/validator that replays the bytes; most modules also carry their own `#[cfg(test)] mod tests` that flips every bit of the encoding and expects rejection.

## 7001-7317: tail of `ieee_float_arithmetic::conversions`/`tests` (module opens at L:4855, `tests` at L:6769, `bytes_of` at L:6783)

1. Purpose: Test code only. It pins SSE2 scalar float encodings (min/max, negate, sqrt, format convert, int to float, truncate, narrow carriers, raw-bit classification) to hand-assembled byte vectors, and checks that validation rejects any one-bit change.
2. Shape: `shares_realization` (L:7071) is a helper that says two forms may legitimately share bytes. Tests: `truncation_encodings_match_the_architectural_bytes` (L:7089), `narrow_carrier_encodings_match_the_architectural_bytes` (L:7141), `raw_bit_classification_encodings_match_independently_assembled_bytes` (L:7164, hex strings for IsNan/IsInfinite/IsFinite/IsNormal/IsSubnormal/Classify, Binary32 and Binary64), `validation_binds_every_bit_operation_format_and_register` (L:7239), and `declared_lengths_match_encoded_forms` (L:7293, outside the inner module, inside `ieee_float_arithmetic`).
3. Inputs/outputs: reads `IeeeFloatArithmeticForm::ALL`, `target_x86_64::float_sizes::ieee_float_arithmetic_size` (L:7255, L:7300), the `encode`/`validate`/`canonical` functions defined earlier in the module (not in range). Produces assertions only.
4. How it decides: sharing between forms is keyed on form equality, or on both being `TruncateToInteger` with the same carrier target and the same format when neither domain is `Trapping` (L:7076-7085). Size agreement is keyed on `MachineSizeKnowledge::ExactBytes` or `EncoderResolved{min,max}` (L:7301-7313).
5. Cases by shape: none beyond the enumerated forms.
6. Same job elsewhere: the "flip every bit, expect rejection" pattern is repeated in nearly every module here (L:7382, L:7502 via `assert_every_bit_flip_rejected`, L:8259, L:8682, L:9525, L:10728). The shared helper `crate::selected_form_encoding::assert_every_bit_flip_rejected` is called at L:7502, L:10728, L:10894, L:11391; older tests inline the same loop (grepped by reading).
7. Unused code: test-only.
8. Stated limits: none.
9. Fail-open: none seen. The test at L:7276 deliberately skips the cross-form rejection for forms that share realization (`shares_realization`), so two truncate forms with different non-trapping domains are expected to be accepted as each other's bytes; that is stated by the helper's name and by the condition at L:7083-7084 (traced to the test only).

## 7318-7443: `memory_fence`

1. Purpose: Encodes and decodes the three x86 memory fence instructions.
2. Shape: `encode(kind) -> Option<Vec<u8>>` (L:7326); `decode(bytes) -> Option<MemoryFenceKind>` (L:7336). Bytes are `0f ae e8` (Load), `0f ae f8` (Store), `0f ae f0` (Full).
3. Inputs/outputs: `MemoryFenceKind` in, bytes out. Called from `register_access::encode` (L:8887) and `register_access::validate_bits` (L:9258) (grepped, two production call sites).
4. How it decides: match on the fence kind (L:7327-7332); `_ => return None` refuses any kind not listed, for example `Aarch64Full` (test at L:7411-7417 checks that refusal).
5. Cases by shape: fixed list of three kinds; the rest refused (L:7331).
6. Same job elsewhere: `Kind::AtomicFence` is encoded separately as `0f ae f0` in `instruction_bytes::encode_unchecked` (L:12155). Two sites hold the same Full-fence bytes (read).
7. Unused code: both functions are `pub(crate)` with two call sites each (grep above).
8. Stated limits: none.
9. Fail-open: none seen; unknown kinds return `None`, which both callers turn into an error (L:8887 `ok_or(AlternativeMismatch)`, L:9258 `== Some(kind)`).

## 7444-7532: `port_write`

1. Purpose: Validates the fixed-width immediate port write sequence.
2. Shape: `validate` (L:7455) only; the encoder is `x86_64_encoding::encode_immediate_port_write` (outside the file). Re-exports `effects` from `target_x86_64::form_effects::port_write` (L:7448).
3. Inputs/outputs: kind, alternative, operands, bytes in; `ValidatedX86_64SelectedFormEncoding` out with `footprint(kind, alternative, operands)` from the `decoding` module (L:7471, not in range).
4. How it decides: requires `SelectedInstructionKind::PortWrite{port,value,..}` (L:7462), then the request check (L:7465), then an exact byte comparison with the encoder output (L:7466). Keyed on the port and value in the kind, not on decoded bytes.
5. Cases by shape: none.
6. Same job elsewhere: `Kind::PortWrite` is also encoded in `encode_unchecked` (L:12152). Also the `Operation::PortWrite` register form in `register_access` (L:9016) is a different (register-operand) form.
7. Unused code: `validate` is `pub(super)`; call site is in the dispatcher outside my range (not grepped further).
8-9. No stated limits. Fail-open none seen.

## 7533-9780: `register_access` (system registers, ports, MSRs, descriptor tables, segments, task register)

Purpose: Encodes and validates x86 privileged register-access forms: control registers, RFLAGS, interrupt enable/disable, halt, `in`/`out`, `rdmsr`/`wrmsr`, GDT store/load, segment reload/read, task register read and `ltr` activation, and memory fences (delegated). Also hosts the installed-table sequences.

### 7556-8353 `descriptor_registers`
- Shape: `Label` (L:7562), `Step` enum of 30 step kinds (L:7571), `activation()` (L:7625), `table_preflight(descriptor_bytes, canonical_selector, reject_cet)` (L:7689), `reload(kind)` (L:7752), `emit(bytes, step, patches)` (L:7872), `encode(operation, registers)` (L:7977), `decode(bytes, cursor) -> Option<(Step, Option<usize>)>` (L:8006), `decode_task_read` (L:8163), `validate` (L:8180), `validate_stream(steps, bytes)` (L:8210).
- Inputs/outputs: a stream of `(Option<Label>, Step)` is assembled by `assemble_labeled_stream` (outside range, defined at L:11709) into bytes with rel32 patches. Validation decodes the supplied bytes step by step and compares each decoded step to the expected step; branch targets are compared with label positions (L:8223-8240).
- How it decides: operation plus the exact register list. `SegmentReload` needs registers `[7,2,1,8,0]` (L:7979); `TaskRegisterActivate` needs `[7,6,2,1,8,0]` (L:7994); `SegmentRead`/`TaskRegisterRead` accept one register `< 16 && != 4` (rsp is index 4) (L:7982, L:7997). Anything else is `EncodedFormMismatch` (L:8002). The generated program is a refusal-checking sequence: it checks selector privilege bits (`imm(4,0,3)`, L:7715), IF flag (`0x200`, L:7719), GDTR limit/base against registers 8 and 7 (L:7729-7735), descriptor present/type/DPL bytes (L:7649-7656, L:7761-7786), then branches to `Refused` (result 0) or `Done` (result 1).
- Cases by shape: segment reload only has `Code` and `Data` kinds (`match kind` at L:7767, L:7780, L:7787). Data reload saves FS/GS base MSRs `0xc0000100/0xc0000101` (L:7797) and restores them (L:7830). Decoder handles a fixed list of instruction patterns and refuses the rest with `_ => return None` (L:8156, L:8153).
- Same job elsewhere: `installed_table::cpu_state_stream` (L:8511) re-lists the same prefix checks as `table_preflight` (L:7714-7747) and the same TSS descriptor validation as `activation()` (L:7649-7680); the two are written out separately (read, both cited). `scan_byte_sequence` (L:9781) has its own separate labeled-stream replay (`expected_stream` L:10339, `decode_all_stream` L:10325) doing the same job as `validate_stream` (L:8210) for a different instruction set.
- Unused code: `activation` is used by `installed_table` (L:8357, L:8628). `ReadSegment`, `ReadTask` steps are used by `encode` (L:7986, L:7999) and `cpu_state_stream` (L:8558). `Step::WriteDescriptorLimit`/`WriteDescriptorBase`/`LoadIdtr` are used only by `publication_stream` (L:8666-8668).
- Fail-open: `validate_stream` is closed (every step must match in order; `cursor == bytes.len()`, L:8237). In `decode`, branch destinations decoded as `Label::Refused` placeholder (L:8061) are replaced by expected labels in `validate_stream` (L:8223-8227), so a branch whose displacement points to the wrong label is rejected via `labels.contains` (L:8240). None seen.

### 8354-8843 `installed_table`
- Shape: register contracts `X86InstalledCpuStateRegisters` (L:8363), `X86InstalledTaskCommitRegisters` (L:8398), `X86InstalledTablePublicationRegisters` (L:8421), each with a `FIXED` constant (L:8371, L:8405, L:8428) and `CLOBBERS`; `X86InstalledCpuStateGeometry{gdt_bytes}` (L:8393); error enum `X86InstalledTableEncodingError` (L:8438); public functions `encode_/validate_x86_64_installed_cpu_state_check` (L:8460, L:8468), `..._task_commit` (L:8477, L:8485), `..._table_publication` (L:8494, L:8502); stream builders `cpu_state_stream` (L:8511), `task_commit_stream` (L:8610), `publication_stream` (L:8654).
- Inputs/outputs: fixed registers and a GDT or IDT size in; byte sequences out, validated by replaying the stream.
- How it decides: registers are compared to the `FIXED` constant (L:8516, L:8614, L:8658) and any difference is `UnsupportedRegisters`. GDT size must be 8..=65536 and a multiple of 8 (L:8519); IDT size 16..=4096 and a multiple of 16 (L:8661). `task_commit_stream` takes the cpu-state stream, removes the last 4 steps (`steps.truncate(len - 4)`, L:8618), inserts the busy-bit clear (L:8620), appends `activation()` with `Refused`/`Done` relabelled to `ActivationRefused`/`ActivationDone` (L:8623-8636), and ends with `InvalidCommit` stops (L:8647-8648). Keyed on position: the truncate count 4 depends on the trailing four steps of `cpu_state_stream` being `Immediate(0,1)`, `Branch(Done)`, the Refused immediate and the Done end (L:8603-8606).
- Cases by shape: one fixed register assignment per sequence; one fixed GDT descriptor size of 16 bytes for the TSS in `cpu_state_stream` (`imm(0,0,16)` L:8566).
- Same job elsewhere: see above (cpu_state_stream vs table_preflight/activation).
- Unused code: `grep -rln "encode_x86_64_installed_\|X86InstalledTable"` over `source-rust` finds only `isa-x86_64/src/lib.rs` and this file (2 files). Inside `isa-x86_64` the callers are `installed_interrupt::encode`/`validate` (L:11471-11518). So there are no users in other crates. Line counts per name: `encode_x86_64_installed_cpu_state_check` 6, `..._task_commit` 4, `..._table_publication` 6, `validate_x86_64_installed_table_publication` 7 (Grep over `source-rust` and `tools`, `_recycling` excluded).
- Fail-open: none seen. `validate_*` rebuilds the stream and replays it.

### 8844-9490 `register_access` direct forms
- Shape: helpers `move_register` (L:8858), `swap_bytes` (L:8865); `encode(operation, registers)` (L:8873); private `Decoded` enum (L:9047); `decode(bytes) -> Option<Vec<Decoded>>` (L:9076); `validate_bits` (L:9225); `footprint(operation, operands)` (L:9452); `validate(physical, operation, alternative, operands, bytes)` (L:9474).
- Inputs/outputs: scalar register indices (u8, 0..15) in, bytes out. Validation resolves operand views to scalar registers via `request_validation::resolve_scalar_registers` (L:9483-9484), then `validate_bits` (L:9485).
- How it decides: `encode` is a chain of `if let` per operation: descriptor-register operations delegate (L:8874-8882); fence delegate (L:8883); GDT store/load uses fixed scratch r10 (L:8898); control-register read/write uses r10 and r15 as scratch and selector bits per `ControlRegister` (L:8922-8936), and only `operation.is_admitted()` control registers pass (L:8918); interrupt enable/disable and halt need zero operands (L:8939-8955); RFLAGS read/write via `pushfq`/`popfq` with r10/r15 (L:8956-8980). The remaining four (port read/write, MSR read/write) take exactly two registers (L:8981) and permute through r10/r11, rcx, rdx, rax (L:8989-9041); anything else is `AlternativeMismatch` (L:9041).
  `validate_bits` for the last four does a symbolic bit-origin trace (L:9329-9449): 16 registers by 64 bits, `ZERO = u16::MAX`, `RESULT = 1024`; each decoded move/swap/join updates origins; `Physical(actual)` must equal the operation, must occur exactly once (L:9419), checks that port/MSR operands arrive in rdx/rax/rcx (L:9382, L:9386, L:9396-9402), and checks that every register other than the declared clobber set and the result register is unchanged (L:9436-9447). The clobber set is keyed on the operation: `[0,2,10]` for PortRead, `[0,2,10,11]` for PortWrite, `[0,1,2,10,11]` otherwise (L:9437-9439).
- Cases by shape: `Operation` list is fixed. Register 4 (rsp) is refused as a result register (L:8917, L:8960, L:8985). Comment-free; the restriction is visible only in the code and tests (`operand == 4` at L:9517).
- Same job elsewhere: the symbolic origin trace is not duplicated in this unit. Other x86 replay validators read: `descriptor_registers::validate_stream` (L:8210), `scan_byte_sequence::validate` (L:10445), `zero_bytes::validate` (L:10583, hand-checked byte positions). Three different replay techniques in one file.
- Unused code: `footprint` is `pub(crate)`; the one in-range call is L:9487; others are in the dispatcher outside my range (not grepped).
- Fail-open: `footprint` (L:9452) computes `writes_rflags` from a fixed list: `SystemWrite(Rflags) | InterruptControl(_) | TaskRegisterActivate | SegmentReload(_)` (L:9463-9469) and not from the bytes, so a form such as `PortRead` is declared as not writing flags; this is a declared footprint, validated against the catalog elsewhere (not traced). Classification: open.
  `validate_bits` line `let first = *registers.first()? as usize` (L:9335) returns `None` (reject) when operands are missing. None seen otherwise.

### 9753-9780 `encoding_vectors`
Test pinning `PortRead`, `ModelSpecificWrite`, `ModelSpecificRead` to "independent llvm-mc vectors" (name at L:9757; the string is the test name, the vectors are inline).

## 9781-10540: `scan_byte_sequence`

1. Purpose: Encodes and validates a loop that scans a byte sequence for a predicate (non-empty, no NUL, ASCII only, valid UTF-8) and leaves a boolean in a result register.
2. Shape: `Label` (L:9800), generic `Instruction<T>` (L:9816, 13 variants), `layout(predicate, base, count, cursor, byte, result)` builds the labeled stream (L:9868), `emit_cont_tail` for UTF-8 continuation bytes (L:10024), `emit_instruction` (L:10093), `decode_one` (L:10153), `decode_all_stream` (L:10325), `expected_stream` (L:10339), `instruction_size` (L:10404), `encode` (L:10422), `validate` (L:10445), `request` (L:10471).
3. Inputs/outputs: five operand registers (`[base, count, cursor, byte, result]`, L:10429) and a displacement (must be 0). Output is bytes whose decoded instruction list must equal the expected list with label targets resolved to byte positions (L:10455-10458), and then `canonical_effects_encoding(..., 2, ScanByteSequenceV1 {base_pointer_operand: 0, count_operand: 1})` (L:10459-10468).
4. How it decides: keyed on `ByteSequencePredicate`. `NonEmpty` returns after a test and `setne` (L:9884-9888). `ValidUtf8` builds the UTF-8 state machine with compare ranges 0x80/0xC2/0xE0/0xF0/0xF5 and special-case paths for E0, ED, F0, F4 (L:9922-9956). `NoNul` and `AsciiOnly` share the simple loop (L:9965-10017); other predicates hit `_ => unreachable!()` (L:9995). Registers: cursor and byte must not be rsp index 4 (L:10479).
5. Cases by shape: four predicates, enumerated. Immediate operands are `u8` only; relative jumps always use rel32 (L:10138-10146). Decoder accepts only condition codes `0x82|0x83|0x84|0x85|0x87` (L:10295).
6. Same job elsewhere: `zero_bytes` (L:10541) is the fill counterpart and uses a hand-coded 25-byte layout with rel8 jumps instead of a labeled stream. Compare with `compare_byte_sequences` (L:1073) and `copy_bytes` (L:1474), outside my range (suspected same family, unread here). `append_indexed_byte_load` has 7 grep lines in one file (the definition at L:11771 plus uses at L:9788, L:10122 and others).
7. Unused code: `CC_NE` etc. are used in this module. The macro `t!` (L:9862) is used at L:9950-9956 only.
8. Stated limits: none.
9. Fail-open: `decode_one` accepts a jump whose target `== bytes.len()` (`target > bytes.len()` is the refusal test, L:10306, L:10316) which is valid for a label at the end of the stream; the comparison with `expected` still requires the same position. Traced: closed. None seen.

## 10541-10748: `zero_bytes`

1. Purpose: Encodes and validates a byte-zeroing loop (`ZeroBytes` family).
2. Shape: `encode` (L:10556) writes a fixed 25-byte block (`ENCODED_BYTES = 25`, `STORE = 8`, L:10553-10554); `validate` (L:10583); `zero_store` (L:10618); `request` (L:10631).
3. Inputs/outputs: three registers `[destination, count, cursor]` (L:10561). Validation checks positions byte by byte and then `canonical_effects_encoding(..., ZeroBytesV1 {destination_pointer_operand: 0, count_operand: 1})` (L:10606-10615).
4. How it decides: exact byte positions and rel8 offsets (`8 + bytes[7] as i8 == 25`, L:10594; `25 + bytes[24] as i8 == 8`, L:10602). Uses `byte_loop_registers::<3>` for the register rules (L:10636). Keyed on position and register identity.
5. Cases by shape: single fixed 25-byte shape; rel8 jumps are only valid because the loop body is constant size.
6. Same job elsewhere: see scan_byte_sequence above; this one hand-validates instead of replaying a stream.
7. Unused code: `pub(super)` encode/validate called from the dispatcher (not in range).
8-9. No stated limits; fail-open none seen (every byte from 0..25 is constrained except `zero_store` displacement `[0;4]` and immediate `0`, L:10619-10628).

## 10749-11423: hosted exit, read-byte, write-byte leaves

1. Purpose: Encodes and validates the Linux and macOS system-call sequences for process exit, reading one byte into the stack, and writing one byte from a register.
2. Shape: `hosted_exit_process`: `effects` (L:10761), `request` (L:10768), `encode_x86_64_selected_hosted_exit_process_form` (L:10793), `validate_...` (L:10821), decoder re-exports (L:10844-10846). `hosted_read_byte`: `request` (L:10949), encode (L:10980), validate (L:11008), decoder re-export (L:11034). `hosted_write_byte`: `request` (L:11211), encode (L:11243), validate (L:11283), decoder re-exports (L:11309-11311).
3. Inputs/outputs: native target, physical register model, kind, alternative, operands (and a displacement for read/write). Output: bytes plus a footprint with `writes_rflags: true` (L:10838, L:11028, L:11303).
4. How it decides: all three gate on the target (`hosted_exit_syscall_number(target)` L:10775, `hosted_syscall_target(target)` L:10957, L:11219), on the canonical register model identity (L:10776, L:10958, L:11220), on exact `SelectedInstructionKind` and alternative key, and on operand shape. Read-byte requires `LocalStorageSlotId::Structural` and a displacement that is non-negative, a multiple of 4, and `<= 2147483640` (L:10961-10973). Write-byte requires `LocalStorageSlotId::Boundary` and displacement in `0..=i32::MAX` (L:11224, L:11232). Validation compares the decoded value from `target_x86_64::inspection` decoders against the expected register or displacement (L:10830, L:11018-11019, L:11293-11295). The Darwin read-byte sequence is chosen by `target == NativeTarget::macos_x64()` (L:10991); every other accepted target gets the Linux sequence.
5. Cases by shape: kernel selection `hosted_kernel(target)` (L:11252) and `kernel.carry_reports_failure` adds a `jc` (L:11268). Only Linux x64 and macOS x64 pass the gates (tests at L:10889, L:11090, L:11364 list those two plus the Windows refusal at L:11418). Read-byte and write-byte use different slot kinds (Structural vs Boundary) on purpose; tests check each refuses the other (L:11128-11131, L:11405-11411).
6. Same job elsewhere: aarch64 hosted leaves exist in `isa-aarch64/src/hosted_sequences.rs` (crate map; not read). Within x86, `hosted_kernel` (324 lines, crate map) holds the syscall numbers.
7. Unused code: `encode_x86_64_selected_hosted_exit_process_form` 6 lines/3 files, `..._read_byte_form` 6/3, `..._write_byte_form` 7/3; `decode_x86_64_selected_hosted_write_byte_i32` 8 lines/4 files (Grep over `source-rust` and `tools`, excluding `_recycling`).
8. Stated limits: none.
9. Fail-open: `request` for exit and write-byte does not check that the register list has a specific class, only that exactly one register resolves (L:10787-10789, L:11237-11239). Footprint `register_reads: operands.to_vec()` (L:10836, L:11301) is therefore one register. Traced: closed by the single-register match. In `hosted_exit_process::effects`, the `_target` parameter is unused and errors are mapped to `EncodedFormMismatch` (L:10761-10766); no accepting path. None seen.

## 11424-11669: `installed_interrupt`

1. Purpose: Adapts the three installed-table sequences (CPU check, task commit, table publication) to the selected-form API.
2. Shape: `impl From<X86InstalledTableEncodingError> for Error` (L:11440), `request` (L:11446), `encode` (L:11462), `validate` (L:11493), `footprint` (L:11531), `Form` re-exported from `target_x86_64::form_effects::installed_interrupt` (L:11430).
3. Inputs/outputs: kind with `gdt_bytes` or `table_bytes`, operand views. Output: validated bytes plus footprint (reads `operands[..form.input_count()]`; writes operand 3 only for `CpuCheck`; `writes_rflags: form != TablePublication`, L:11536-11542).
4. How it decides: `Form::from_kind(kind)` else `AlternativeMismatch` (L:11452); resolved scalar registers must equal `form.registers()` (L:11456); geometry comes from the kind (L:11470-11487), so bytes are replayed against exactly the geometry in the kind.
5. Cases by shape: three kinds, enumerated (L:11469-11489); `_ => AlternativeMismatch` (L:11488, L:11523).
6. Same job elsewhere: none.
7. Unused code: `footprint` is `pub(crate)`; one call in range (L:11527).
8-9. No stated limits; none seen. Test `selected_commit_cannot_drop_its_refusal_stop` (L:11629) pins that the final stop bytes cannot be removed or replaced.

## 11670-12456: `instruction_bytes`

1. Purpose: Shared byte-building helpers and `encode_unchecked`, the encoder for all plain scalar selected forms that need no layout.
2. Shape: `integer_bits` (L:11682), `u12` (L:11694), `rex` (L:11701), `modrm` (L:11705), `assemble_labeled_stream` (L:11709), `append_*` helpers (L:11734-11786), `append_saturating` (L:11827), `append_trapping` (L:12028), `append_trapping_divide` (L:12000), `append_lea_register` (L:12089), `append_lea_immediate` (L:12107), `encode_unchecked(kind, alternative, registers)` (L:12129), `X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET = 3` (L:12443), `byte_literal_address_bytes` (L:12445).
3. Inputs/outputs: kind, alternative key, scalar register indices in; raw bytes out, no footprint. Delegates saturating arithmetic to `SaturatingForm::of(operation, carrier)` and trapping arithmetic to `TrappingShape::of(form)` (L:11833, L:12033), both in sibling modules (`saturating_forms` L:16342, `trapping_forms` L:21655, not in range).
4. How it decides: `match kind` (L:12135-12439). Alternatives for subtract and multiply are chosen by `alternative.variant` (0..3) (L:12382-12394, L:12405-12413); `MaterializeI64` variant 1 is a self-clear and only valid for value 0 (L:12158-12163). Saturating and trapping forms reject register lists that do not satisfy `accepts_registers` (L:11834, L:12034). Division and remainder forms demand `registers[0]==0 && registers[2]==0 && registers[3]==2 && registers[1]!=2` (rax, rdx fixed; L:12249, L:12256, L:12265, L:12281, L:12288, L:12296). Shifts demand `registers[1]==1` (rcx) (L:12421). `u12` accepts only `IntegerValue::Unsigned <= 4095` (L:12696).
5. Cases by shape: `Kind::InstalledInterrupt*` refused (L:12136-12140); `AccessorScalarAccess | NearestIeeeFloatArithmetic | FloatResultPolicy` refused (L:12147-12151); layout-dependent forms refused with `LayoutDependentForm` through the macros `selected_instructions::layout_dependent_selected_transfers!()` and `dedicated_selected_forms!()` (L:12435-12438). The arm list ends without a wildcard, so a new `Kind` variant forces a compile-time decision here (read).
6. Same job elsewhere: zero- and sign-extend encodings (L:12186-12231) have a separate decoder in the `decoding` module (L:1826, outside range); `register_access::move_register` (L:8858) and `append_register_binary` (L:11747) build the same `89 /r` move; the memory fence full bytes are written again at L:12155 (see `memory_fence`). The exact-divide/remainder prologue check is written four times (L:12249, L:12256, L:12265, L:12281, L:12288, L:12296; six counting both remainder forms).
7. Unused code: `assemble_labeled_stream` has 14 grep lines in one file (`isa-x86_64/src/selected_form_encoding.rs`), so no use outside this file. `append_indexed_byte_load` 7 lines/1 file.
   The comment-free `X86_64_BYTE_LITERAL_ADDRESS_FIELD_OFFSET` has 8 lines/4 files.
8. Stated limits: none.
9. Fail-open: `encode_unchecked` indexes `registers[0]`, `registers[1]`, `registers[2]`, `registers[3]` directly (for example L:12162, L:12165, L:12249). A shorter list would panic, not accept. Whether callers guarantee length is not traced here (open). `Kind::MaterializeI64` non-variant-1 path does not check register class (L:12165); the register comes from earlier request validation (outside range). `integer_bits` rejects values outside i64 bits (L:11686-11691). Not a fail-open route.

## 12457-12599: `integer_normalization_tests`

Tests for the six zero/sign-extend forms: every register pair, effects as `fallthrough_v1(reads [0], writes [1])` (L:12521), cross-form replay rejection (L:12523-12551), every single-bit flip rejected (L:12553-12563), and four clang-assembler vectors (L:12569-12597).

## 12600-12713: `jump`

1. Purpose: `jmp rel32` encoding and validation.
2. Shape: `encode_x86_64_selected_jump_form` (L:12608), `validate_x86_64_selected_jump_form` (L:12620).
3. Inputs/outputs: displacement `i64` in; five bytes `e9 d0 d1 d2 d3` out; footprint reads and writes the `rip` register units (L:12641-12646) with `UnconditionalRelativeBranchV1` (L:12662).
4. How it decides: displacement must fit `i32` (L:12613), the alternative must equal `Jump` variant 0 (L:12627-12632), bytes must match `[0xe9, a, b, c, d]` with the decoded displacement equal to the supplied one (L:12635-12640). Keyed on value.
5. Cases by shape: one near-jump form. Short jumps are not here (the file header names "short and near" branches; those are in `branch_forms`, L:690).
6. Same job elsewhere: `branch_forms` (L:690, not in range) holds conditional branch forms. `scan_byte_sequence` emits its own `e9` (L:10144).
7. Unused code: `encode_x86_64_selected_jump_form` 12 lines/6 files, `validate_x86_64_selected_jump_form` 9 lines/4 files.
8-9. No stated limits. None seen; the `expect("canonical model has RIP")` at L:12644 is a panic on a model lacking `rip`, guarded by `guard_canonical` at L:12626.

## 12714-12749: `materialization`

Two error-only modules: `mov_r32_imm32::X86_64MovR32Imm32I64MaterializationError` (L:12719) and `mov_r64_imm32_sign_extended::X86_64MovR64Imm32SignExtendedI64MaterializationError` (L:12736), each five variants and `wire::invalid_error_display!`. Re-exported at `isa-x86_64/src/lib.rs:163-164` (grepped). Each name has 5 grep lines/3 files across `source-rust` and `tools`; I did not grep for construction sites of the individual variants, so whether any code still returns these errors is not established (suspected unused, not checked).

## 12750-13000: `memory` and `memory::frame` (partial; module continues to L:15273)

1. Purpose: `frame` encodes and validates a 64-bit store to an activation frame slot and the computation of a frame address, optionally aligned.
2. Shape: `request` (L:12773), `encode(physical, kind, alternative, operands, displacement, alignment_bytes)` (L:12808), `validate` (same args plus bytes, L:12867), tests start L:12959 (`aligned_frame_forms_decode_members_and_declared_clobbers`, L:12966, cut at L:13000 by the range boundary).
3. Inputs/outputs: operands are `[value, scratch]` for `Store64` (with `value != scratch`) or `[address]` for `FrameAddress` (L:12801-12805). Frame address bytes come from `crate::frame_protocol::encode_frame_address_signed` when alignment is 1, otherwise `encode_x86_64_aligned_frame_address(physical, address, offset, alignment_bytes, member)` (L:12835-12849); a store appends `encode_x86_64_pointer_memory` (L:12850-12855). `validate` checks the pointer store suffix (`decode_pointer_store_suffix`, L:12896) and the address bytes with `validate_frame_address` (L:12901). Footprint is built from the register constraint catalog row `X86_64_STORE64` or `X86_64_FRAME_ADDRESS` (L:12913-12935).
4. How it decides: `ActivationBase` slot is refused for stores, and for frame address when `byte_offset != 0` (L:12780-12789) or when displacement != 0 or alignment != 1 (L:12816-12825, L:12876-12885). Register model must have the canonical identity (L:12795). Aligned addressing sets `writes_rflags: alignment_bytes != 1` (L:12931). Keyed on slot identity, displacement, alignment value.
5. Cases by shape: alignment 1 or "other"; any `alignment_bytes != 1` takes the aligned path and `u32::try_from(displacement)` refuses negative displacements there (L:12839-12840).
6. Same job elsewhere: `frame_protocol` crate module (660 lines, crate map; not read) encodes the same frame-address forms for the prologue; `request` duplicates the ActivationBase test between `request` (L:12780-12789), `encode` (L:12816-12825) and `validate` (L:12876-12885) (read).
7. Unused code: `pub fn encode`/`validate` inside `pub(super) mod frame`; not grepped.
8. Stated limits: none.
9. Fail-open: none seen in range. In `validate`, `ImmediateOutsideU12` is returned for a displacement that does not fit `i32` (L:12887-12888); the name is misleading but the path rejects.

## Coverage

- Read in full: `selected_form_encoding.rs` lines 7001-13000 (6000 lines), in four Read calls of 1500 lines. Boundary: line 13000 ends mid-test inside `memory::frame::tests`; the body of the `memory` module past that is another reader's range.
- Skimmed: none.
- Not read: none of the assigned range. I did not read outside it except via greps named above (module list for the whole file, counts, `lib.rs:163-164`).
- Grep commands used: a regex over the file for `mod` declarations and `fn bytes_of`; per-name counts with `grep -rn --include=*.rs "<name>" . ../tools | grep -v _recycling | wc -l` (and `-rl` for file counts) run from `source-rust/`; `grep -rln --include=*.rs "encode_x86_64_installed_\|X86InstalledTable"`; `grep -rn "register_access::encode\|register_access::validate\|memory_fence::"` inside `isa-x86_64/src`; a pattern search for TODO/FIXME/unsupported/not yet/migration/remains/legacy over lines 7001-13000 returned nothing.

## Five things a maintainer most needs to know about this range

1. Every family has two independent halves: an encoder and a decoder/validator that re-derives the expected bytes or replays them. Validation is done three different ways in this range: stream replay with label positions (`descriptor_registers::validate_stream` L:8210, `scan_byte_sequence::validate` L:10445), a symbolic bit-origin trace (`register_access::validate_bits` L:9225), and fixed-position byte checks (`zero_bytes::validate` L:10583, `port_write::validate` L:7455).
2. The privileged sequences (segment reload, `ltr` activation, installed-table CPU check and commit) are generated from `Step` streams and are tied to one fixed register assignment each (`[7,2,1,8,0]`, `[7,6,2,1,8,0]`, the `FIXED` constants at L:8371/L:8405/L:8428); any other registers are refused (L:7979, L:7994, L:8516, L:8614, L:8658). `task_commit_stream` is built by cutting four steps off `cpu_state_stream` (L:8618), so it depends on that stream's tail.
3. `instruction_bytes::encode_unchecked` (L:12129) is the encoder for the plain scalar forms; it has no wildcard arm, and refuses InstalledInterrupt, accessor, nearest-float, float-result-policy, layout-dependent and dedicated forms by name or macro (L:12136-12151, L:12435-12438). Those are encoded elsewhere in the file.
4. The hosted leaves (exit, read-byte, write-byte) accept only Linux x64 and macOS x64 (gates at L:10775, L:10957, L:11219), choose the kernel sequence by comparing the target with `macos_x64()` (L:10991) or by `hosted_kernel(target)` (L:11252), and hand decoding to `target_x86_64::inspection` functions outside the crate.
5. The installed-table encoders and their register structs have no users outside `isa-x86_64` (only `lib.rs` and this file match a grep over `source-rust`), and the two `materialization` error-only modules (L:12714-12749) are re-exported at `lib.rs:163-164` with only those two sites plus the definitions found by grep.

## What I am unsure about

- Whether `encode_unchecked` callers guarantee that the register slice is long enough; I did not read the dispatcher at L:21797 onward.
- Whether anything still constructs `X86_64MovR32Imm32I64MaterializationError` or `X86_64MovR64Imm32SignExtendedI64MaterializationError` variants; I counted name lines only.
- Whether the footprint's `writes_rflags` list at L:9463-9469 matches the catalog declarations in `target_x86_64::form_effects::register_access`; that comparison happens elsewhere.
- The body of `descriptor_registers::decode` at L:8006-8161 uses ad hoc prefix tests; I traced that unknown prefixes return `None` but did not exhaustively check each pattern against the emitter at L:7872-7975.
- The names `shares_realization`/`bytes_of` belong to a test module whose start (L:6769-6783) is outside my range, so how `bytes_of` encodes is not read by me.

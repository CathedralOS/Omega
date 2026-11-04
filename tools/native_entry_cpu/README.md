# Execute a compiled interrupt entry

This harness runs an Omega-generated x86-64 interrupt entry and its actual body in QEMU. The CPU creates the interrupt frame, enters the exact address retained by the compiler, and returns through the generated `iretq`. Eight handler-emitted `T` bytes must alternate with eight harness return markers: `BT.T.T.T.T.T.T.T.P`, with QEMU exit status 33.

After every return the harness checks all 15 non-stack GPRs, RSP, the tested RFLAGS bits (including IF and DF), all 16 XMM registers, MXCSR, and both ends of the normal and interrupt stacks. The entry runs on a dedicated IST stack. Each interrupt begins with IF and DF set; the entry must establish its promised body conditions and restore the interrupted flags. PIC interrupts stay masked. Delivery is eight software `int 0x40` instructions, not a timer.

The retained source witnesses are [the direct body](../../tests/omega/pass/build/interrupt_return_debug_marker/main.omg) and [the helper call](../../tests/omega/pass/build/interrupt_return_helper_debug_marker/main.omg). The helper artifact retains two functions and a real call. Neither test creates an acknowledgement or installed-root authority. The [Cathedral acknowledgement fixture](../../tests/omega/pass/build/interrupt_return_cathedral_ack/README.md) is compiled only; do not execute it with this harness.

## Produce and run the bytes

Use an optimized corpus runner built from the compiler under test. Ordinary fixture discovery and review supply the source inputs. On the check leg these fixtures realize their authored Linux x86-64 entry without running the hosted application. Request immutable artifacts in a fresh directory:

```sh
OMEGA_CORPUS_CAPTURE_ARTIFACTS="$PWD/build/interrupt-capture" \
OMEGA_CORPUS_CAPTURE_NATIVE_ENTRIES=1 \
python3 tools/corpus_gate.py --runner /path/to/verified/corpus_runner \
  --filter pass/build/interrupt_return_debug_marker,pass/build/interrupt_return_helper_debug_marker
```

Capture retains `native-image.bin`, `private-entry-count.txt`, each `private-entry-N.txt`, and the ordinary Terminal artifact. Entry records come from the retained private function and final image layout; they are not found by disassembly or generated-name matching. The extra native files are opt-in, so existing capture and verification records are unchanged by default.

For each positive capture directory:

```sh
python3 tools/native_entry_cpu/extract.py \
  build/interrupt-capture/pass/build/interrupt_return_debug_marker/linux_x86_64
python3 tools/native_entry_cpu/run.py --oracle debug-marker --label direct \
  --placement build/interrupt-capture/pass/build/interrupt_return_debug_marker/linux_x86_64/placement.json
```

Repeat with `interrupt_return_helper_debug_marker` and a distinct label. Extraction copies ELF load segments at their original addresses and initializes BSS; it adds no relocations or entry code. The placement receipt binds the original image, exact entry record and copied payload. The runner verifies the payload digest before loading it. Payload addresses must lie from `0x200000` through the range below `0x2000000`, clear of the harness. Results, bytes, commands, tool hashes and logs default to ignored `build/native-entry-cpu/<label>/`; `--output-root` changes that location. Labels cannot overwrite an existing run.

Required tools are Python 3, Clang with its x86-64 assembler, an existing `ld.lld` or Rust `rust-lld`, and `qemu-system-x86_64`. The image uses the bundled PC BIOS/Multiboot path, not UEFI or a downloaded firmware image. No host observer library is loaded or called.

## Failure controls and limits

The source controls reject [missing GPR preservation](../../tests/omega/fail/build/interrupt_return_missing_gpr_restore/main.omg), [unsaved vector use](../../tests/omega/fail/build/interrupt_return_vector_helper/main.omg), and [enabling interrupts under a Masked plan](../../tests/omega/fail/build/interrupt_return_masked_enable/main.omg). Only their intended diagnostics establish rejection.

Handwritten `control.S` is solely a harness bringup/falsification input. `--control marker-iret` should pass; `--control clobber-gpr` and `--control wrong-return` must fail. `--control iret --oracle debug-marker` returns correctly but omits `T`, so the runner must reject even if QEMU exits 33. The separate memory oracle uses a test-only counter at `0x300000`; it supplies no Omega memory authority. These controls never count as Omega source execution. `--control marker-iret --load-address 0x300000` also checks that marker-mode setup preserves a payload at the memory oracle's counter address.

The direct and helper Omega images both produced the exact eight-marker result. The frozen pre-repair compiler instead refused the demanded entry with `IncompatiblePlan`. Raw producer/source manifests and historical bringup records remain in the qualification build directory; copied bytes and logs are not committed.

This proves entry/body transport and safe repeated CPU return. It does not prove Cathedral boot, IDT installation authority, stack-root lifetime, live PIC/PIT delivery, `Pending` acknowledgement issuance, or waking ordinary Omega work. The separate nested Resident-layout gap is unchanged.

## Save and restore GDTR through an ordinary call

The [descriptor-table witness](../../tests/omega/pass/build/descriptor_table_round_trip/README.md)
uses an ordinary System V callback entry. Capture it with the same corpus command,
filtered to `pass/build/descriptor_table_round_trip`, then select its recorded
`CallReturn` entry:

```sh
python3 tools/native_entry_cpu/extract.py --entry-control CallReturn \
  build/gdtr-capture/pass/build/descriptor_table_round_trip/linux_x86_64
python3 tools/native_entry_cpu/run.py --oracle gdtr-call --label gdtr-round-trip \
  --placement build/gdtr-capture/pass/build/descriptor_table_round_trip/linux_x86_64/placement.json
```

The harness owns and pins the table and descriptor buffers for the entire call.
It initializes all ten output bytes to `0xA5`, then calls the actual Omega entry
eight times. After each return it independently checks the loaded base/limit,
original GDTR restoration, immediate six-byte and outer word canaries, unchanged
table contents, exact RSP, callee-saved registers and IF/DF. Ordinary caller-saved
registers and arithmetic flags may change. The required output is `B........P`
and QEMU exit33. Repeat with a distinct label and `--corrupt-neighbor`: the
harness deliberately damages one adjacent byte and must report failure.

This closed CPU test injects no asynchronous interrupts, single stepping or
hardware breakpoints during the temporary load. It does not reload segment
registers or activate a TSS/IDT/timer. The interrupt oracle above retains its
separate complete interrupted-state checks.

## Activate a retained task-state segment

The [task-register witness](../../tests/omega/pass/build/task_register_activation/README.md)
uses the same ordinary-call export path. Extract its `CallReturn` entry and run
`--oracle task-register-call`. The harness owns the GDT and task-state bytes
until shutdown. The Omega body performs the activation and all selector reads;
the harness independently checks the resulting TR and complete descriptor.

An alternate table contains an otherwise valid descriptor, so the wrong-table
control specifically tests the association with the CPU's current GDTR. A
mismatched task-state buffer, code slot, outside selector and already-busy
repetition must also refuse. Eight ordinary calls produce `B........P` and
exit33. The first activates and the other seven verify refusal of the busy
entry. `--corrupt-neighbor` must report failure using the same compiled payload.
No timer interrupt or acknowledgement is created by this test.


## Select retained code and data descriptors

The [selector witness](../../tests/omega/pass/build/segment_selector_reload/README.md)
uses the ordinary-call export path with `--oracle segment-selector-call`.
The compiled body selects new CS and DS/ES/SS entries, clears FS/GS selectors,
and preserves their active 64-bit bases. It checks refused operations before
performing the valid reloads. The harness independently checks all selectors,
full base values (including KernelGSBase), descriptor accessed bits and neighboring
storage after each of eight calls. Nonzero upper base halves detect truncation.
The bases are canonical but never dereferenced. The required output is
`B........P`, exit33; the same payload with `--corrupt-neighbor` must fail.

The table remains live until shutdown. The code transfer uses the compiler's
local continuation, with no source-supplied return address. These calls run in
a closed CPL0/IF0 environment without asynchronous or debug delivery; the code
operation refuses CET-enabled environments. They do not install interrupt
roots or demonstrate timer delivery.

QEMU 11.0.2 does not write the code descriptor’s accessed bit on its same-privilege far-return path. The first strict run observed CS=24 with the original code word unchanged and failed the mandatory accessed-bit check. The harness therefore checks CS independently and allows only that one bit to differ in the selected code descriptor; all other descriptor bits, the data accessed-bit update and neighbors remain exact. This is an emulator limitation, not permission from the ISA or proof that real hardware’s CS accessed-bit write occurred. The compiler still requires a mutable table and invalidates its prior contents. See [QEMU’s pinned return helper](https://github.com/qemu/qemu/blob/v11.0.2/target/i386/tcg/seg_helper.c) and [Intel’s descriptor contract](https://cdrdv2-public.intel.com/671190/253668-sdm-vol-3a.pdf).

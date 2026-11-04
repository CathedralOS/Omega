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

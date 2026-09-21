# RC-REPRESENTATIVE-PROGRAMS-GATE — record

Measured at `94e764a6da` on linux x86-64 (cargo; `mbx` absent on this
host). Gate leg: `cargo nextest run -p compiler --test samples_compile
--no-fail-fast`. The gate is **RED** — same three failure families the
sibling row (RC-REPRESENTATIVE-PROGRAMS-GREEN, `d8041919ad`) recorded,
plus a fourth surfaced by the gui aggregate.

## Failure families (all authored-entry selection legs)

1. **`windows_x86_64` entry-schema rejection** — every sample's
   windows_x86_64 leg fails: "target physical entry requirement and
   schema `named-callable(path(WindowsProcessEntry::enter),parameters(),
   result-dispatch())` require either the exact bundled Windows x86-64
   contract or one accepted package-owned Windows x86-64 binding, not
   `source/library/std/targets/windows_x86_64/entry.omg`". Baseline
   repair surfaces: ENTRY-CONTENT-ROOTS lane.
2. **ProgramEntry rejoin "0 Terminal attachment identities"** — the
   linux_x86_64 / linux_arm64 / macos_arm64 legs of call-bearing
   samples fail "the machine's unit plan was omitted at local
   construction at `statement sequence: call: call operation`" (or
   `structural field store: record literal field`, or "at an
   unavailable callee (`Main::add`)"). Named residuals on this board.
3. **print_number domain-field leg** —
   `named_integer_conversion_samples_reach_checked_trees` fails on
   `cli/basics/print_number`: "cannot prove default-domain field
   requirement for return from Main::main at statement 19: self.out
   requires [u8; N]::Utf8".
4. **GUI Fused-provider leg (newly surfaced here)** — the
   `gui_samples_compile_from_authored_program_entry_bindings` macos_arm64
   legs fail "selected ProgramEntry Service field `Main::<field>`
   requires a selected Fused provider for boundary `Clock|Input|Gui|
   FilesystemHost`" — GUI samples bind unprovided boundaries on the
   only non-Windows target that reaches that far.

## Passing legs observed before the red ones

dutch_flag, euclid_gcd (service-call entry plan retained), cli_mvp
(both lines + EOF + enter, 83s), generic_counter,
native_acceptance legs, sample_entry_exceptions,
standard_sample_discovery_excludes_application_submodules.

## Run shape

The suite is dominated by the per-family aggregate tests; the four
largest (`all_samples_reach_checked_trees`,
`arithmetic/algorithm/collection_samples_compile_from_authored_program_entry_bindings`)
each ran >45min on this host. Full census in the landing session's
nextest log.

macOS/Windows/QEMU host legs unavailable here per protocol.

# Shared atomic Boolean storage

Owning outcome: **ATOMIC-MEMORY-MODEL** in `TASKS.md`.

Fact: **A shared atomic Boolean store preserves its exact cell and ordering,
and its load observes the stored Boolean.** Every place copies this sentence.

This covers the already admitted `AtomicBool` load/store family used by
`atomic_global_order_operations`. On baseline
`036605466510d2361fd579cf9fd97b722e070beb`, normal package preparation of
`local` succeeds, but `omega run --both local/main.omg` refuses native target
lowering with `UnsupportedControlFlow { machine: MachineId(1) }`. Target
production required an integer cell, although source checking and Terminal
interpretation admit Boolean storage. Logical Boolean operands and results must
remain distinct from the physical byte transferred by its atomic instruction.

The nine places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each place has
`main.omg`, a four-host entry declaration, and `invalid/main.omg`. Its reject
twin changes only the store ordering from `Publish` to `Receive`, which must
report `atomic store cannot use Receive ordering` (with source backticks).
The same inputs are pinned under `tests/omega/{pass,fail}/facts/atomic_boolean_store`.
`other_state` retains the existing nested-state transport spelling during the
repository's separate nested-machine source migration.

Run each positive with `omega update --project <place> --target linux_x86_64`,
then `omega run --both <place>/main.omg`; both engines must exit 0. Check each
reject twin through the ordinary public CLI and require its ordering diagnostic.
The root repeats the local witness. `composition` additionally tests true/false
replacement, a distinct sibling cell, all three legal store/load ordering pairs,
and both receive and receive/publish fences.

The applicable requirements are:

- Establishment: only the dedicated atomic store establishes the resident;
  source-checked Boolean operands cannot become arbitrary integer bytes.
- Identity and lifetime: the event keeps the exact root, path and field;
  observation snapshots retain Boolean identity across their transport.
- Composition: fields, parameters, calls and joins transport the observation;
  target-scoped bodies select the same operation with explicit arguments.
- Soundness: a shared receiver authorizes the dedicated event without granting
  ordinary writes. Illegal orderings and Boolean arithmetic remain rejected.
- Psi 05–07: one normalized access event retains ordering and operand identity;
  the fence remains a separate location-free event. No load/store split occurs.
- Omega 00–02: exact declared atomic-cell identity supplies the logical type;
  the physical byte mapping supplies width/alignment and ordinary calling homes.
- Omega 03–06: independent legalization reconstructs that mapping; selection
  resolves Boolean operands/results and retains their proper register type.
- Omega 07–09: the existing byte-width instruction encoding and Boolean result
  normalization emit the native product. Execution must agree with Terminal Psi.

On Linux x86-64, the optimized producer at
`666e562d988fc2e8f0f32620192e7f56ec469e42` passes all nine places,
their nine ordering reject twins, the root and composition, and seven additional
source authority/type controls: 39 CLI observations including the profile check.
Every executable observation reports native exit 0 and Terminal exit 0.
The pinned placed-accessor controls independently admit permitted load/store and
reject the same operation with its exact placement permission removed.
See [validation](validation.json) for producer identity and observation commands.
Linux x86-64 is the available execution host. Other hosted targets need matching
host observations. Serial agreement proves no publication/fence synchronization,
contention behavior, memory-model axioms or target-refinement theorem.
**TR3-TR8** still gates the concurrent witnesses. Bounded atomic resident
predicates and non-observing exchanges remain separate open acceptance.

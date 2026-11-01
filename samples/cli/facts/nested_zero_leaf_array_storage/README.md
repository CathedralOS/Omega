# Nested zero-leaf array storage

Owning tasks: `NORMALIZED-ABI-LOWERING`, bounded native codec preparation and ordinary storage materialization for nested empty scalar arrays; `BUILD-OWNED-TARGET-SELECTION` and `BUILD-OWNED-NATIVE-BINDINGS`, migration of this complete row to explicit Build-owned supply.

Fact: an owned record with 512-by-512 zero-leaf array elements preserves its two stored sentinels and its last empty row through transport and copying with sixteen bytes of value storage.

The same sentence, bounds and consuming operation appear in every place. The real C provider returns `{17,31}` and transforms a copied record to `{28,60}`. Each witness checks both sentinels, the two nonzero outer extents, the last empty row's borrowed view and `size_of<Packet>() == 16`; a wrong result traps. The C declaration omits the zero-byte field. Equal geometry never supplies reference, content or claim authority.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [index512](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [index512](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [index512](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [index512](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [index512](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [index512](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [index512](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [index512](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [index512](target_body/invalid/main.omg) |

[Root](main.omg) and [ordinary zero initialization](zeroed_source/main.omg) exercise both construction routes. Every positive declares all four hosted `ProgramEntry` bindings and exit 0. The ten reject twins change only the final index 511 to 512, reaching “cannot prove index `512` is within length512”. The nine places and extra producer are pinned under `tests/omega/{pass,fail}/facts/nested_zero_leaf_array_storage_*`.

Requirements and evidence:

- **Establishment:** the foreign result constructs the exact declared typed record; ordinary zero initialization uses the existing zero-validity and erased-evidence checks. Schema validation includes declarations below zero counts before suppressing instance expansion.
- **Identity and lifetime:** initialized empty arrays retain exact type, root and path receipts. Partial retirement records vacancies, and snapshots require live initialization. The auxiliary snapshot/replacement fixture checks an untouched empty sibling and both stored sentinels; its packed-frame variation and the write-only reject control exercise the shared read path. This is not exhaustive restoration-union acceptance.
- **Composition:** fields, parameters, completed calls, constant extent names, both distinct match routes, state arrival, another machine and target body preserve the same record. No caller equality guard substitutes for construction or index evidence.
- **Soundness boundaries:** zero physical extent supplies no opaque contents, references, qualifications or linear claims. The write-only control still rejects reads. Unsupported non-scalar descendants remain outside this compact representation.
- **Psi 05–07:** every public positive runs Terminal Psi and retains a canonical `program.oterm` that reloads successfully. Typed empty receipts replace expanded empty byte allocations, preserving logical bounds and custody.
- **Omega 00–02:** the real matching-host C provider consumes and returns the two scalar lanes with the declared 16-byte value geometry; all target bodies are checked. The matching-host C ABI and runtime oracles now pass on all four hosts.
- **Omega 03–06:** native execution preserves sentinels, copying and projected borrows. Zero-byte owned copies, completed calls, observed parameters and state arrivals use funded private address anchors, retaining zero-byte ABI payloads and performing no payload loads or stores.
- **Omega 07–09:** actual Mach-O execution agrees with Terminal Psi. Retained native identities and canonical products are checked for all 13 public programs, including the distinct nonzero byte-row control; the separate matching-host qualification also verifies PE and both ELF targets.

On baseline `70f2db5eccd`, the unchanged root already exits0 in both engines but peaks at1,603,846,144 bytes. Actual optimized producer `843cfd786cc` runs the root at 356,089,856 bytes; all 13 public observations stay at or below 362,184,704 bytes against the frozen 512 MiB limit. The scoped corpus records23 authored runtime agreements and11 intended rejections. Formatting and scoped Clippy pass. [The observation record](qualification_macos_arm64.json) retains original producer identities and evidence hashes; later fixture, golden and documentation commits do not relabel that compiler.

The [four-host record](hosted_qualification.json) uses actual optimized source `84e39b1c4f5` on every host: 11 public BOTH passes, 10 corpus agreements, 10 intended bounds rejects, 12 canonical reloads and two batch cases per host. Compiler, C-provider, source/oracle, lock, native image and canonical hashes are retained. The three foreign-host archives omit only 12 temporary deletion markers each; no semantic product is missing. Linux x86-64 reported two source-checking timing slowdowns against shared pins; all functional oracles passed. No fresh memory claim is made.

Remaining: the parent task's qualified/view-backed carriers and retained-provider custody, and ordinary nested array literal results containing live borrowed-array subvalues. An auxiliary `snapshot_rows(rows: &[[u8;0];2]) -> [[u8;0];2] { [rows[0],rows[1]] }` refused before execution on both baseline70f and the first candidate (`returned scalar array`); that separate construction operation is not claimed repaired here.

Contracts: [boundary shapes](../../../../wiki/spec/build/boundary_shapes.md), [layout plans](../../../../wiki/spec/layouts/plans.md), [byte views](../../../../wiki/spec/terminal-psi/byte_views.md), and [indexing](../../../../wiki/spec/language/counts_and_addresses.md).

Build-owned supply migration: all 21 authored projects retain the same two
`ZeroLeafProvider` requirement signatures and the same four-target locator tuples.
The package Build binds those requirements directly using inferred locator lengths;
product Source no longer duplicates bodyless native declarations or binding-value
helpers. The target-body place uses ordinary `ObserverCode satisfies Observing`,
selected explicitly for each target, and preserves the original observer body.
The two scalar lanes, nested zero extents, exact bounds, `17/31 -> 28/60` sentinel
checks, copied packet operation, and both match routes remain unchanged.

Fresh migration observations use optimized producer
`e16ca0f34d840822a9f21890922e3705607b4479` with its matching immutable SDK:
all 11 public positives exit 0 in native and Terminal execution, all ten invalid
projects reject the final index 512, the 20 checking pins and ten runtime pins
pass, and the target-body project checks for all four hosted targets. The exact
C provider self-test passes. Both match inputs execute; changing only the true
branch marker, false branch marker, or expected packet size from 16 to 17 gives
three typed Terminal traps, while the unchanged control exits 0.

The original pre-migration pins also passed (ten checking positives, ten intended
bounds refusals, and ten runtime agreements) on retained optimized producer
`98708cfd91ebcf097651baaac16f72484a2f9be5`. These are separate observations.
The initial migrated target-body probe used the wrong Build selection argument
positions and refused during preparation; its corrected ordinary selection ran
in both engines. That setup failure is retained, not counted as a compiler defect.

The e16 producer predates the canonical declaration ABI publication and callable
contract implication repair in this Source base. Consumer review preserves its
applicability here: the zero/one-parameter owned Packet signatures retain the same
ordered recipes and result; the packet has no opaque/callback/static application,
its inline fields are plain u64, and its nested arrays and shared `[u8;0]`
presentation retain the same storage and borrow meaning. Executions retain the
actual e16 binary and format identities. The authored programs are the migrated
originals described above; no newer compiler execution is claimed.

The earlier four-host qualification above retains its original source and producer
identities. Fresh runtime is macOS ARM64 only; no new foreign-host or memory
measurement is inferred. The parent target/native migrations, the distinct
restoration and live borrowed-array construction gaps above, and broader ABI
orchestration remain open.

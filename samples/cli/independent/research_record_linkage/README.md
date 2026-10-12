# Offline research record linkage (NEW064)

This application interprets stdin as a bounded command stream. It maintains a
synthetic registry of at most eight two-digit record keys and links
observations against it. It performs console input/output only; it has no
equipment, network, filesystem or real-clock boundary, and its records are not
drawn from any real dataset.

Each line contains exactly an uppercase opcode and two ASCII decimal digits
(00-99). LF is the separator; CR is an ordinary invalid byte. EOF commits a
final nonempty line. Blank, wrong-length, non-digit and unknown commands are
rejected without changing linkage state. Overlong input is drained to LF with
constant memory. The 65th command produces `BUDGET_EXHAUSTED` and exit 65;
other streams exit 0.

| Command | Meaning |
| --- | --- |
| Rdd | Register a record with key dd into the next free slot; duplicates are allowed and create ambiguity; a ninth registration is `DENY capacity` |
| Odd | Observe key dd: exactly one matching registry slot is `LINK unique`, more than one is `LINK ambiguous`, none is `UNLINKED` |
| Add | Emit an `AUDIT` row with tag dd, all counters and the registry and decision checksums |

Rejected commands are atomic. Every trace row is deterministic: register rows
carry the assigned slot and key; unique links carry the first matching slot;
ambiguous links carry the match count; capacity denials carry the attempted
key and the current registration count.

The audit checksums are exact integer folds modulo 97: the registry checksum
adds `(slot + 1) * key` at each registration, and the decision checksum adds
`observation_index * code` per observation (unique 1, ambiguous 2, unlinked 3).
Two digits are sufficient for every reported quantity at the 64-command bound.
`??` is a visible report invariant violation and can never match a frozen
oracle.

The entrypoint and input framing live in `main.omg`; `linkage/model.omg` owns
the registry, matching scan and counters; `linkage/report.omg` owns trace and
audit rendering. The only dependency is the current bundled std Console and
ProgramEntry provider. The application uses no private compiler hooks or
another worker's numerics code.

Semantic comparison: `laboratory_protocol` owns a lease/interlock state graph
over single-digit commands; `uncertain_event_chronology` enumerates a finite
constraint space. This sample adds a record-linkage workload with retained
duplicate registrations, unique/ambiguous/unlinked classification, capacity
pressure and cumulative audit checksums over a two-digit record grammar.

`fixtures/*.txt` are immutable input bytes; `.stdout` and `.exit` are the
independently frozen expected results. `reference.py` uses a list scan and
`count`/`index` rather than the Omega state graph. It verifies its
calculations against those files by default; `--freeze` is an explicit
authoring action, never validation. The manifest was frozen before the first
Omega execution. All arithmetic is integer and all comparisons are exact:
tolerance **0**. Twelve fixtures cover nominal linking, unique links,
ambiguity from duplicate registration, unlinked observations, capacity
denial, audit tags and checksums, invalid runtime input, repeated
multi-matches, boundary keys 00/99, final unterminated input, hostile
framing and command-budget exhaustion. The separate `invalid/` directory is
a complete intended-invalid application that writes a registry slot without
proving the declared in-range precondition. Runtime input rejection is
distinct from this source rejection.

From the repository root, with a compiler built from this base and its
matching library selected:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=./target/release/omega
$OMEGA update --project samples/cli/independent/research_record_linkage --target linux_x86_64
# Inspect the generated review, accept the local std entry binding, then:
$OMEGA update --resume --project samples/cli/independent/research_record_linkage
$OMEGA --check --target linux_x86_64 samples/cli/independent/research_record_linkage/main.omg
$OMEGA run --both --keep samples/cli/independent/research_record_linkage/main.omg < samples/cli/independent/research_record_linkage/fixtures/nominal.txt
python3 samples/cli/independent/research_record_linkage/reference.py
python3 samples/cli/independent/research_record_linkage/validate.py --omega "$OMEGA"
```

The public `run --both` route replays the same finite stdin for Terminal and
native execution. Its comparison checks exit status; the forwarded stdout is
native. Validation checks all native trace bytes against the frozen oracle
and requires explicit Terminal exit agreement. It does not claim that this
CLI verifies Terminal stdout equality.

Validation records and generated artifacts belong in ignored `build/`.
The candidate's validation record identifies exact binary hash, base, host,
resource settings, commands and observed results. Run only one build and one
Omega execution at a time on this executor. Other targets are bound using
existing std providers, but only actually executed target evidence is
claimed.

## Provenance

This is a reconstruction, not a recovery of the original NEW064 source. The
recorded original candidate checkpoints `5a3cd26be912e035ec63144ca7d89ee8b7abe290`
and `4629179e3c34e5673258eafab84bef862881712c` are not reachable from this
checkout or from origin (`upload-pack: not our ref`), and no remote branch
carries the path. The behavior above is authored from the stated contract —
bounded synthetic record matching with uniqueness, ambiguity and audits —
against the repository's own sample conventions, with an independent
mathematical oracle. Nothing here claims original bytes, original review
decisions or original acceptance frames.

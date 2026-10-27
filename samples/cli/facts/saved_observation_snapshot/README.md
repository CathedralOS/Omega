# Saved observation of an unwritten array element

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent outcome: `STATE-LOCAL-VALUE-FRONTIER` in [TASKS.md](../../../../TASKS.md).
The nine places now read `items[0]` through an opaque generic reference result:
`capture<View>(source: View, marker: &mut u64, event: u64) -> View`, with
`View == &'source u64`. The owned `u64` receiving context performs the read
after the helper completes. The helper writes a caller-visible marker so an
omitted or repeated evaluation cannot silently satisfy the runtime oracle.

The consuming operation remains `agree(observed, current)` with
`requires observed == current && observed == 7`. Every positive starts with
`[7, 1]`, saves element zero, and writes element one to 4. Its reject twin
changes only the write destination to element zero. The field place also checks
neighbor fields; the match place runs both arms and distinguishes their marker
writes (11 and 13). Each sample has matching pass/fail fixtures in
`tests/omega/{pass,fail}/facts/saved_observation_snapshot_through_<place>_*`.

## Recorded behavior

On fixed base `62228749501b42e644eb43371f1bf38b4444baf8`, seven positive
places refused the consuming contract. The call-result and other-machine places
already passed through their explicit contracts and establish coverage only.
All nine reject twins rejected.

At source `386a5e718459964410f66780586b0c7165502810`, all nine positives
ran with native exit 0 and Terminal Psi exit 0 on `macos_arm64`. All nine twins
still rejected the affected `agree`, nested `check`, or `read_kept` contract.
The optimized producer reports opt-level 1; its SHA-256 is
`393514a74ad5457051d53e703167a17e4acc062e5785915725e1edf51704818a`.
Raw source hashes, commands, diagnostics and engine observations are retained in
`build/generic-numeric-observation/row-{baseline,candidate-3}.json` in the author
worktree. Hosted target declarations cover all four targets; these observations
establish execution on macOS ARM64 only.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Requirements and evidence

- **Establishment:** the literal array or callee input contract establishes 7.
  The checked descriptor identifies the source of the completed primitive read;
  the receiving type gives its scalar interpretation. No helper-body replay or
  caller equality guard supplies the saved fact.
- **Identity and lifetime:** the read captures its post-receiver scalar and
  bounds. Its owned expression result stays detached from later storage writes.
  A disjoint write preserves equality to the current element; an overlapping
  write removes that equality. Readable shared and mutable references are
  admitted without widening access authority.
- **Composition:** the same fact and consuming operation cross all nine places.
  Match arms receive the owned scalar context before their results join.
  Marker effects expose which arm executed and whether evaluation occurred once.
- **Soundness boundaries:** every candidate referent must have the same scalar
  interpretation; scalar equality requires agreement across all referents and
  range evidence uses their hull. Write-only or differently interpreted
  references cannot manufacture a scalar fact. All nine fact-sensitive twins
  reject; additional stale-pre-call controls also reject.
- **Checked-to-Terminal Psi (`Psi 05–07`):** generic reference calls, explicit
  loans, scalar read completion and the match join lower through the ordinary
  scalar path and execute in Terminal Psi. This row does not establish proof-tag,
  affine cleanup, dynamic dispatch or crash-route completeness.
- **Provider settlement, layout and calling plans (`Omega 00–02`):** ordinary
  Build entry bindings, array/record geometry, reference arguments and scalar
  results are realized through the normal target plans. These programs do not
  qualify foreign bindings, stack-argument boundaries or aggregate returns.
- **Instructions, register homes and activation frames (`Omega 03–06`):** the
  macOS ARM64 execution checks calls, scalar loads, indexed writes and live saved
  values, including caller-visible marker and neighbor canaries. It does not
  establish every ISA path or stack-bound calculation.
- **Encoding and image emission (`Omega 07–09`):** direct native images run and
  agree with Terminal Psi on this host. Other hosted targets remain unexecuted;
  relocation and branch-relaxation boundaries are outside this fact.

## Remaining acceptance

This repair establishes generic reference reads of existing scalar/bound facts;
the parent capability remains open. A reference-producing call that writes its
referent and guarantees `source == 7` still loses that guarantee when its result
is read into an owned scalar. In the retained `receiver_write` witness, the
initial element is 3, `observe` writes 7, and the same `agree` contract refuses;
reading the current element directly already succeeds on the baseline. The
missing connection is between the completed read and the call's post-return
storage version, not permission to reuse its pre-call value.

The retained `range_contract` witness transports a nonconstant [7, 9] bound
through a generic reference read and succeeds at a consuming contract for both
endpoints; its [6, 9] twin rejects. Using that saved value in exact subtraction
still refuses before Flow builds the completed read. The early bounds engine
must consume the evidence without admitting unproved arithmetic; this remains
under `ONE-INTEGER-BOUNDS-ENGINE`. Generic body-checking compositions,
byte-predicate observations and single-evaluation pattern destructuring also
remain open. No completed parent outcome is claimed here.

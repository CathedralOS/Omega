# NF02 numeric accuracy policy

This bounded scalar foundation reuses the core Float arithmetic, classification,
total-order, and named-conversion providers. It classifies both IEEE formats,
retains signed zero, and gives callers explicit tolerance/error conventions.
See `source/library/numerics/numeric/numeric_accuracy_policy.api.txt` for the
interface and the proposed NF01 export. NF01 owns shared numerics metadata;
this candidate changes none of it. No arrays, layout, allocation, or implicit
format conversions are involved.

The complete `classification` and `error_budget` consumer projects assert
separate frozen mathematical oracles in `EXPECTATIONS.txt`. A false predicate
reaches `crash Trap`; success exits 0. `invalid_suffix` is a separate complete
project whose explicitly suffixed f64 literal must reject at an f32 return
boundary under `wiki/spec/language/numeric_values.md`.

From the repository root, prepare the sample-only provider package:

```sh
python samples/cli/scientific/w100/nf02_numeric_accuracy_policy/prepare.py
```

This explicitly copies the exact library source without rewriting it into an
ignored sample-local provider directory. It permits independent validation before
NF01 integrates the shared package. The provider's package metadata and entrance
are tracked. There is only one authored implementation. Once NF01 supplies the
shared package, consumer dependency paths and imports can select that export.

Build and validation commands used on this executor:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/nf02-target mbx build -p omega
export OMEGA_LIBRARY_ROOT=/workspace/nf02/source/library
/workspace/nf02-target/debug/omega --check samples/cli/scientific/w100/nf02_numeric_accuracy_policy/classification/main.omg
/workspace/nf02-target/debug/omega run samples/cli/scientific/w100/nf02_numeric_accuracy_policy/classification/main.omg
/workspace/nf02-target/debug/omega run --both samples/cli/scientific/w100/nf02_numeric_accuracy_policy/classification/main.omg < /dev/null
/workspace/nf02-target/debug/omega --check samples/cli/scientific/w100/nf02_numeric_accuracy_policy/error_budget/main.omg
/workspace/nf02-target/debug/omega run samples/cli/scientific/w100/nf02_numeric_accuracy_policy/error_budget/main.omg
/workspace/nf02-target/debug/omega run --both samples/cli/scientific/w100/nf02_numeric_accuracy_policy/error_budget/main.omg < /dev/null
/workspace/nf02-target/debug/omega --check samples/cli/scientific/w100/nf02_numeric_accuracy_policy/invalid_suffix/main.omg
```

On other hosts use the locally built `omega` binary, select this checkout's
library, and use shell-appropriate empty stdin for `--both`. `prepare.py` uses
only portable Python standard-library file operations. Run one Rust build and
one Omega invocation at a time.

## Evidence and current blocker

Actual fetched base and compiler source revision:
`03e477fa424a9e6a2db7f49e17bf896ab3f76b48`.
The initial checkout was `6e4f0c37ec` on `work`; it was preserved.
Worktree `/workspace/nf02`, branch `nf02-numeric-accuracy`.
Early API checkpoint `01ecf3ca55`; implementation and frozen oracle revision
`06163d8b1f`. Evidence commit `03bb24aaca` preserves those sources.
The subsequent `nf02-policy-contract` reconciliation renames the public comparison
API and its tolerance types; the recorded validation attempts remain historical
evidence for `06163d8b1f`, not attempts against the renamed revision.

One isolated compiler build succeeded in 1m 24s (`dev`, unoptimized), with
warnings retained in `evidence/compiler-build.log`. Compiler binary SHA-256:
`09c10482a97e1a2bbf114b7f3d04a3f2a44c7748dc2ba8c63b5155d49059c246`.
The binary reports `unrecorded`; its source revision is established by the
isolated worktree build, not by that version string. No compiler source was
modified. At the recorded implementation revision, the library and staged copy both had SHA-256
`1d853ec061e7e4a64a9573cc86a7d66c55b1778cf619752b7c09b7b627c2f37c`.

All seven validation invocations stopped before source validation:

| Route | classification | error_budget | invalid_suffix |
| --- | --- | --- | --- |
| Check | blocked, exit 1 | blocked, exit 1 | blocked, exit 1 |
| Native | blocked, exit 200 | blocked, exit 200 | not requested |
| Terminal comparison | blocked, exit 200 | blocked, exit 200 | not requested |
| Intended suffix rejection | not applicable | not applicable | not observed |

The diagnostic is `cannot open private source resolver storage: git cache entry
\`/\` is invalid: cache custody ancestry is owned by an unrelated user`.
The executor runs as UID 1000; `/` is owned by UID 65534 (`nobody`). The
existing guard in `source-acquisition/src/custody/tree.rs::verify_cache_ancestry`
allows only the effective user or UID 0. No checked/native/Terminal success or
intended numeric rejection is claimed. Full commands, revisions, stderr, and
statuses are in `evidence/validation.json`; the complete reproducing sources
remain in the three projects. Automatic approval review rejected restoring `/`
ownership to `root`, citing a broad privileged system mutation outside NF02
scope. No environment ownership or guard was changed or bypassed.

Next acceptance step: use an executor whose cache ancestry satisfies the existing
custody contract, or obtain explicit approval for the environment repair, then
rerun the commands. Inspect any later compiler diagnostics against these full
sources; do not rewrite away a general compiler gap. Candidate remains for
independent review, without landing or an acceptance claim.

Measured Linux executor resources (2026-10-10): `nproc`=5; cgroup CPU quota
`400000/100000`=4 effective cores; cgroup memory limit=17,179,869,184 bytes
(16 GiB). `free -b`: total=18,882,699,264, available=17,977,204,736 bytes.
`df -B1 /workspace`: capacity=33,770,192,896, used=3,443,765,248,
available=28,591,665,152 bytes. Isolated target directory=1,486,172,080 bytes.
Commands: `nproc`, `cat /sys/fs/cgroup/cpu.max`,
`cat /sys/fs/cgroup/memory.max`, `free -b`, `df -B1 /workspace`,
`du -sb /workspace/nf02-target`. Host-visible RAM exceeds the cgroup limit.

Claim status returned coordination-unavailable with a Git deadline; no successful
claim registration is asserted. Local assignment: NF02 scalar classification,
named conversion, and accuracy conventions; acceptance is both consumer oracles
on check/native/Terminal plus the suffix rejection; current failure is cache
custody; next action is valid executor and rerun; no prerequisite wave interface
blocks implementation. The unrelated historical boards and files were untouched.

`python samples/cli/scientific/w100/nf02_numeric_accuracy_policy/derive_oracle.py`
independently checks the frozen rational midpoint and addition-error derivations
using Python Fraction and explicit IEEE packing. Its exact fractions are retained
in `evidence/oracle-derivation.txt`. This validates the oracle mathematics only;
it supplies no evidence that Omega accepted or executed the consumers.

## Comparison contract reconciliation

The public machines are now `compare_symmetric_any_f32` and
`compare_symmetric_any_f64`, accepting `SymmetricAnyTolerance32` and
`SymmetricAnyTolerance64`. These names identify the existing absolute-OR-relative
policy with symmetric maximum scaling. The source and both consumers use the
new names; the algorithm and mathematical oracle are unchanged. No generic
`compare_f32`/`compare_f64` alias is retained.

The wave template's `abs(actual-reference) <= atol + rtol*abs(reference)` is
a distinct additive, reference-directed acceptance predicate. It is now supplied
by `compare_reference_additive_f32/f64` with
`ReferenceAdditiveTolerance32/64 { atol, rtol }`. The original candidate branch
and its transferred bundle remain preserved.

## Additive reference policy

The additive machines validate finite, nonnegative `atol` and `rtol` first.
They return `Different` for every NaN or infinite operand, including equal
infinities. Finite signed zeros and subnormals are admitted. Unlike the symmetric
alternative, the reference argument alone determines relative scale.

For a finite rounded difference, compare it with the separately rounded
`atol + rtol * magnitude(reference)`. If subtraction overflows, halve actual,
reference, and the absolute budget before computing either side. This retains
the additive real inequality while making the rounded difference finite; a
rounded positive-infinite budget can then legitimately exceed it. No
infinity-to-infinity acceptance occurs. The checkpoint specifies every rounded
operation. This policy does not certify an exact-real inequality at all rounding
boundaries and introduces no subnormal tolerance floor.

The two existing consumers now cover additive finite admission, malformed
budgets, policy-distinguishing dyadic boundaries, argument reversal, largest
finite values, overflowed products and sums, signed zero, and least subnormals
in both formats. `EXPECTATIONS.txt` freezes the new results before Omega
execution. `derive_oracle.py` independently rounds exact rational values to the
IEEE format; `evidence/additive-oracle-derivation.txt` records the derivation,
including `RN(3M/4)<M` and `RN(s/2)=0`. These derivations alone do not establish
Omega source admission or execution.

The initial ownership diagnosis described the default sandbox's synthetic view.
The coordinator subsequently supplied NF01 evidence that ordinary compiler
commands under per-command escalation see real filesystem ownership and can
pass custody without mutation. The proposed ownership repair is withdrawn.
Subsequent validation therefore requests only ordinary compiler execution
outside that synthetic view, with `XDG_CACHE_HOME=/workspace/.cache`; no system
ownership, permissions, settings, or custody guards are changed.

## Additive validation frontier and integration handoff

Additive source/oracle revision: `03a1de3a873ed5fc751de97d50390ffacd88445c`.
Compiler source revision remains `03e477fa424a9e6a2db7f49e17bf896ab3f76b48`;
the prebuilt binary SHA-256 remains
`09c10482a97e1a2bbf114b7f3d04a3f2a44c7748dc2ba8c63b5155d49059c246`.
The one earlier Rust build is reused because compiler sources are unchanged.
The library and prepared provider copy have identical SHA-256
`f5ff964fb5d6d0e2ca93608be0c83469a84c5e176ea82f6f6aed08f2e74c49eb`.

Ordinary compiler invocations were approved per command with `require_escalated`,
no persistent prefix rule, and justification limited to executing outside the
synthetic ownership view. All used `XDG_CACHE_HOME=/workspace/.cache`, explicit
checkout-specific `OMEGA_LIBRARY_ROOT`, and `timeout 120s`. Exact commands,
working directories, source/compiler revisions, exits, and complete emitted
stdout/stderr are in `evidence/additive-escalated-validation.json`.

| Stage | Actual outcome |
| --- | --- |
| Cache custody | Passed under ordinary per-command escalation |
| Unchanged original candidate project check | Rejected, exit 1, 240 existing Float-provider diagnostics |
| Unchanged original candidate module check | Rejected, exit 1, full 240 diagnostics |
| Additive candidate module check | Rejected, exit 1, identical full 240 diagnostics |
| classification and error_budget checks | Both rejected, exit 1, dependency preparation failed |
| classification and error_budget native runs | Both exit 200; compilation failed, no native program executed |
| classification and error_budget Terminal comparisons | Both exit 200; compilation failed, no Terminal execution or agreement |
| Invalid-suffix control | Intended rejection observed, exit 1, exactly one literal-destination diagnostic |
| Minimal Float-import source | Checked, exit 0; import alone does not reproduce the provider failure |
| Combined Float-operations/order import source | Checked, exit 0; both imports alone also do not reproduce the failure |
| Independent oracle derivation | Completed using exact Fractions and explicit IEEE rounding; no Omega execution |

The intended diagnostic is: `literal \`1.0\` is suffixed \`f64\` but lands in a
\`f32\` place -- a width suffix chooses the literal's format at the spelling,
so it must agree with the destination's declared type (drop the suffix or fix
one side)`. The pinned diagnostic fragment now identifies this rule specifically.

Current compiler frontier: in the NF02 baseline and candidate checking context,
Float target-provider satisfaction cannot resolve against overloaded integer
conversion destination requirements. The first diagnostic is `machine satisfaction target
\`I8::from_f32\` does not resolve to an exact trait requirement or top-level
\`boundary requirement\``. All 240 emitted diagnostics concern I8/I16/I32/I64
and U8/U16/U32/U64, f32/f64 sources. Complete baseline, candidate, and minimal
import diagnostics are retained as separate evidence files. The unchanged
baseline and additive candidate direct-module logs are byte-identical.

`evidence/import_float_providers.omg` is a complete four-line check-only source
with only the core Float import and an empty main. Its direct check succeeds
(compiled 25 source files), so import alone is insufficient to reproduce the
failure. A second probe importing both Float operations and Float order also
checks successfully (compiled 26 source files); its complete source and output
are retained separately. The exact baseline and additive NF02 module sources
remain the witnessed failure reproducers; no more specific mechanism is claimed.
The source was untracked during that diagnostic run; its exact source SHA-256
is recorded in the JSON, and the final evidence commit includes those bytes.
The relevant existing declarations are in `core/float_operations.omg` (for
example lines 590, 596, 602); their satisfiers are in each target's
`core/targets/<target>/float_impl.omg` (Linux x86_64 lines 181, 185, 189).
No compiler sources or providers were rewritten around this failure.

The earlier conclusion that no supported execution route existed was limited
to the synthetic default-sandbox view and is superseded by this witnessed
per-command route. No ownership repair was attempted or performed. No guards,
system ownership, permissions, or settings were changed by this assignment.

NF01 is now the sole integration owner of `source/library/numerics/**` and the
w100 consumers, including prerequisite completion. NF02 hands off this bounded
implementation, exact interfaces, independent oracles, full sources, and actual
validation frontier. The original `nf02-numeric-accuracy` branch remains at
`03bb24aaca`; no candidate was landed. After the transfer, NF02 stops further
implementation in these consolidated paths.

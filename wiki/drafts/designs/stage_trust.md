# What each stage may trust

Architecture decision, 2026-11-10, for the whole compiler from source text to
the executable file. Not a language change. It extends
[check on the lowered form](check_on_lowered_form.md), which covers only the
checker, to every hand-off between stages.

Today a wrongly accepted program runs as native code with its safety checks
removed, and nothing between the checker and the executable would notice. The
back half takes every proof obligation as accepted without evidence and then
emits unguarded arithmetic, division and indexing on the strength of it. That
trust is what the specification asks of the back half, so the repair is at the
front: the checker must accept only what the proof kernel has verified, and
reject the rest at compile time. This record sets rules for every hand-off:
what a stage may take on faith, what counts as a check, and what the checker
owes the stages that trust it. No rule here costs runtime speed.

## What happens to a proof obligation today

Traced through the code at `457adf4a11f` (sites in
[the boundary notes](../../../.agents/atlas/compiler-2026-11-10/boundaries/back_pipeline.md)):

1. The checker accepts a contract or a bound. For integer ranges a certificate
   is built and the proof kernel decides it again, inside the proof call, and
   the certificate is then dropped: what returns is a handle and a range. When
   no certificate can be built, an interval fallback decides and the kernel
   never sees it (`proof/src/checker.rs`). Contract checks never produce an
   evidence object; they travel as a "source-checked" flag that lowering
   filters out.
2. Omega 00 turns every obligation reference in the unit into an accepted
   fact whose proof fingerprint is 32 zero bytes
   (`00_terminal-psi-to-abstract-operations/src/optimization.rs`).
3. Attaching those facts checks the program identity, a hash of the fact's own
   fields, and duplicates (`optimization-unit/src/proof.rs`). Nothing else.
4. Omega 01 deletes operations such as `x / x` and `x % x` when the fact is
   present (`01_.../src/proof_check_elision.rs`).
5. Omega 03 requires the fact and then selects unguarded forms: an exact add
   is a bare add, a division a bare divide, a byte read an indexed load with
   no bounds compare (`03_.../src/selection_scalar_graph.rs`,
   `selection_views_and_bytes.rs`).

So overflow, division by zero (apart from the hardware fault on x86), index
bounds and region bounds are absent from native code whenever the checker said
yes, for whatever reason it said yes. The interpreter is described as still
checking; that was not opened for this record.

## What is checked at each hand-off today

"Checked" means a check on that hand-off in an ordinary compile. Corpus
outcomes are not counted here; they are the end-to-end check, described below.

| Hand-off | Checked in an ordinary compile | Independent of the producer |
| --- | --- | --- |
| Tokens, syntax, names, types, specialization (Psi 00 to 04) | No | - |
| Build-time evaluation writing back into the trees | Its own receipts only | No |
| Checker to validation and proof | Integer-range certificates only | Yes, for those |
| Checked trees to lowering (Psi 05 to 06) | No | - |
| Lowering to its optimizer (06 to 07) | A hash | No |
| Publication (07 to 08) | Ordering and one storage check; the structural validator runs only when bytes are decoded | Separate code, but its byte comparison calls the producer's encoder |
| Portable program into Omega 00 | Structure not re-validated on the in-process route | - |
| Obligations into Omega 00 | Identity, self-hash, duplicates | No |
| Omega 01 rewrites | Each candidate validated | No: against stage 00's own table and the producer's own extractors |
| Omega 02 to 03 | Audit runs from the corpus runner only | - |
| Omega 03 to 06 | Receipts that hash and count; promised replays not found | No |
| Omega 06 to 07 | Validator found on no default path | - |
| Omega 07 to 09 | Validators run by default | No: one calls the producer's own encoders |
| Machine emission to object file | Stage validators run only inside the full replay | - |
| ELF static, PE | File not re-read apart from the entry | - |
| ELF dynamic | Each stage replays with the producer's builder; the final verifier is test-only | No |
| Mach-O | Whole container decoded and compared | Second decoder in the same crate |

The only end-to-end check is running the program. `omega run --both` runs the
native executable and the interpreter on the same portable program and
compares the exit status, a trap's signal, and the entry memory. It does not
compare what the program printed. With `--target` neither engine runs and the
command exits 0. A comparison that is declined keeps the native exit code, and
a failed full replay is itself reported as declined. Output and exit oracles
run in the corpus harness only.

Two documents disagree with the code. `omega-rust/pipeline.md` says default
compilation does not run stage-output validators; the code runs them for
Omega 01, 08 and 09. The specification's `--validator` and `--every-validator`
options are not implemented anywhere.

## The decision

Hand-offs are of three kinds, and each kind has one rule.

1. **A stage's own output, inside one compile.** The next stage may trust it.
   That is the current rule in `pipeline.md` and it stays: re-verifying every
   stage on every compile is not the goal. Two conditions make the trust
   safe: the producer returns a row or a refusal and never drops silently, and
   every validator that exists runs in the corpus harness on every program.
2. **A claim that removes a runtime check.** The guarantee is made real at
   the front, and the back half then trusts it. The checker accepts an
   obligation only when the proof kernel has verified a proof of it. An
   obligation it cannot prove is a compile error: the program is rejected, and
   it is never compiled with a slower guarded fallback. Once the checker has
   accepted, the back half removes every check the accepted facts license and
   re-proves nothing, exactly as the ratified `psi-execution-admission`
   decision in
   [`product.md`](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
   says: *"Reuse checked judgments for optimization and lowering under the
   selected installation contracts"*, and *"receipt does not require kernel
   replay"*.

   This costs no runtime speed and changes no specification text. It is a rule
   about how the checker is built, and it applies to the checker on the lowered
   form ([check on the lowered form](check_on_lowered_form.md)); the current
   checker does not meet it (the census found 0 of 814 accepted operation
   obligations with kernel-checked evidence). The cost moves to the prover: it
   must prove everything a valid program needs, or it rejects programs the
   current checker accepts. Route one's gate measures that.

   An earlier wording of this rule had the back half keep the runtime check
   and emit a guarded form when evidence was missing. That is withdrawn: it
   contradicted `psi-execution-admission` (*"This contract provides no guarded
   native lowering route"*), and it paid for a checker defect with runtime
   speed instead of fixing the checker.
3. **Bytes that leave the compiler.** An executable or an artifact is read
   back by a decoder that shares no builder with its writer before it is
   called validated. Where that does not exist yet, the result is called
   sealed, not validated.

Three supporting rules:

- **Words mean what they say.** "Validated" is reserved for a check that is a
  separate derivation. A receipt that hashes and counts is a seal. A replay
  that calls the producer's builder is a consistency check. Types and comments
  that claim more are renamed or corrected.
- **A declined comparison is not a pass.** `--both` reports declined as its
  own outcome, compares printed output as well as the exit status, and does
  not exit 0 when it ran nothing.
- **Pointing back to source never guesses.** A lowered construct with no
  source position says so. The debug map's fallback to "the last state" is
  removed.

## What changes, in order

1. **Measure.** The evidence census already planned for route one counts
   obligations by kind and by how they were accepted. Extend it across the
   boundary: how many accepted facts in Omega 00 have a real fingerprint
   (today: none), and how many guards rule 2 would put back, per program.
2. **Make the checker's acceptance kernel-checked.** Route one's prover emits
   a certificate for every obligation of the scalar subset and counts an
   obligation as discharged only when the kernel accepted that certificate.
   Within one compile the evidence travels in memory beside `TerminalModule`,
   so Omega 00 can fill `AcceptedObligationFact::proof_bundle_fingerprint` and
   the census can count it. An artifact re-read from bytes (including a
   compiler-storage cache read) has no in-memory evidence, which is explicit:
   never an error and never filled from an earlier build. The fingerprint is a
   record for measurement and audit; the back half does not gate on it.
3. **A failed proof is a compile error.** When the checker on the lowered form
   becomes the shipping path for a program, an obligation it cannot discharge
   rejects the program with a diagnostic naming the source construct. Nothing
   in back-half trust, optimization or instruction selection changes.
4. **Run what exists.** Every validator and replay that exists runs in the
   corpus harness for every program, including the Omega 02 to 03 audit, the
   06 to 07 validator and the full native replay. The `--validator` options
   the specification describes are implemented, or the gap is recorded as an
   owner question.
5. **Fix the oracle.** `--both` compares printed output, reports declined as
   declined, and fails when nothing ran.
6. **Correct the words.** `pipeline.md`'s statement about default validators,
   the validator headers that claim independence, and the receipt types that
   only seal.

Rebuilding the back-half stages is not part of this record. Their duplicated
derivations and per-target tables are recorded in the atlas; whether to
replace them is decided after route one's gate, with the measurements from
steps 1 and 4.

## Open questions

- **How much the prover can prove.** Rule 2 rejects what it cannot prove, so
  its strength decides how many valid programs compile. Today it cannot prove
  shift-left representability or bounds on multiplication, and relations
  between two values are being added. Unmeasured until route one's comparison.
- **What the interpreter checks.** Opened since this record was written
  ([notes](../../../.agents/atlas/compiler-2026-11-10/reviews/interpreter_checks.md)):
  it checks exact-arithmetic
  overflow, division and remainder by zero, shift counts, index and subslice
  bounds, case tags and integer width, but a failed check stops the run as a
  malformed artifact and is not a program crash, and the declared range of a
  bounded-integer field is not checked on a store into the field.
- **Constant folding.** Whether Omega 01's constant propagation refuses
  overflow and a zero divisor was not established.
- **An independent decoder for ELF and PE.** None exists; Mach-O's is in the
  writer's own crate.
- **Windows and UEFI.** They are one `NativeTarget` value, so nothing keyed on
  the target can tell them apart.
- **Whether an ordinary compile ever re-reads its own artifact** before Omega
  00 was not established.

## Method and limits

Three readers extracted every hand-off from the
[compiler atlas](../../../.agents/atlas/compiler-2026-11-10/README.md) and the
code, read-only, at a checkout a few commits after `457adf4a11f`. Each opened
the code for its most consequential claims: about 30 sites for the front half,
more than 30 for the back-half pipeline including every obligation site, and
26 for emission, images and drivers. Their notes, with a file and line for
each claim and a mark for "opened" or "from a record", are in
[`boundaries/`](../../../.agents/atlas/compiler-2026-11-10/boundaries/). The
author of this record opened three of the obligation sites: the zero
fingerprint in Omega 00, the attach checks, and the elision gate in Omega 01.
Nothing was built or run. Two atlas claims were corrected by this pass: PE
output is partly re-read (headers and entry), and program-entry binding works
for hosted targets through hosted receivers; only the physical entry is UEFI
only.

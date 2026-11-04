# Restore a record after a temporary invalid write

The compiler rejected helpers that temporarily exceeded a record's declared
bound and restored it before returning. The repair lets a helper complete those
updates while still rejecting an unfinished update or an attempt to use the
record as valid before restoration. Atomic fields keep their stricter rule:
every write must satisfy the bound.

The owning task is **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. The executable
requirement is in
[the write-only record samples](../../../samples/cli/facts/data_where_standing/write_only/README.md).
Each of the nine programs stores 61 into a field whose enclosing record requires
at most 60, restores it, and then computes an addition that needs that bound.
Each has two rejection controls: one removes the bound and one omits restoration.
Untouched nonzero fields detect misplaced writes. The two inputs exercise different
match branches, and implementation selection has distinct results.

## Cause and repair

The checker treated some record bounds as permanent restrictions on each scalar
store. For other records it noticed that an update invalidated a requirement but
could not establish that requirement again after restoration. The checker now
tracks the requirements separately, preserves facts about untouched fields, and
checks the affected requirements against the updated contents. A declaration
alone cannot prove that the current contents satisfy it.

This also fixes two unsafe acceptances found while checking the repair. A
specialized check for a helper's writes lacked the helper's call information and
skipped read checks, then suppressed the ordinary failure. It now uses the same
call information and checks reads. Its check results belong to one compilation,
so concurrent corpus programs cannot consume one another's results.

Child records that were valid before an unrelated scalar write remain valid.
Replacing the complete record also establishes those child requirements. Saved
copies retain their original value: restoring the current field cannot justify
a bound on an earlier copy of 61.

The accepted rules are
[temporary invariant violations](../../spec/language/dependent_values.md#invariant-windows),
[validity after writes](../../spec/terminal-psi/fact_validity.md), and
[write-only references](../../spec/terminal-psi/structural_access.md#write-only-authority).

A further control restores the field from another valid record. Checking this
before the storage history existed discarded the source record's current value
after the first destination write. Restoration checking now runs at the existing
later contract-checking boundary, after that history is available. The same
history proves whether the source was preserved; separate parameter names alone
do not justify preservation. The check remains mandatory before publication.
The completed storage history belongs to a new checking context. It shares the
unchanged program indexes, but recomputes answers that depend on current storage.
An answer obtained before that history existed cannot become a later proof.
The controls include an unrelated array in the source and a source promise of
at most 61 that cannot restore the destination's maximum of 60.

Borrowing a record through a second type with the same storage layout is called
[a recast](../../spec/layouts/recasts.md). A helper that returned this view while
updating a separate marker also made safe addition fail. The early checker lacked
the history needed to prove that the marker write preserved the record. It now
retains that arithmetic obligation; later checking must prove the original
operands safe before lowering receives a successful decision.

The view's fields and the original record's fields have separate identities.
The checker uses the existing recast rules to connect each viewed field to its
original storage and read its current value. Every possible source must supply
the same observed value. A view's declared type alone supplies no contents or
proof of its bound.

When match branches meet, the checker records a new storage checkpoint for
values proved unchanged through the selected branch. A later read of unchanged
entry contents used to reject that checkpoint. It now uses the same proof that
no later write changed the field, while still rejecting an overlapping write or
an unrelated checkpoint.

A write-only destination also need not expose its untouched fields to the proof.
The checker substitutes the exact current fields used by a requirement into its
ordinary predicate judgment. It creates no partial record and assumes no missing
requirement. Uses that need the whole record retain the complete-value check.
The two controls cover both `self.cell.value` and `cell.value` spellings.

## Verification

Fresh qualification on macOS ARM64 used optimized compiler source
`f78c2623a68cf93044b0b38befd42e627a98667a`:

- 37 positive programs ran with native and Terminal Psi exits of zero.
- All 44 negative controls rejected with the relevant diagnostic fragments.
- The focused corpus checked 36 valid fixtures and rejected 83 invalid fixtures,
  with no wrong-reason refusals, unsafe acceptances or lost successes.
- Ten focused Rust tests passed. Lint passed with no diagnostics on added Rust
  lines, and all 36 changed Rust files passed formatting.

The nine restored write-only programs and their eighteen twins are included.
The nine mutable-recast programs and their nine twins are included too. The
strengthened match checks markers 17 and 29 independently, so omitting its helper
or selecting the wrong branch fails at runtime. Additional arithmetic controls
retain an unsafe child even when its result is unused, multiplied by zero,
inside a match, wrapped afterward or followed by a guard. Storage controls check
that overlapping writes invalidate an old value while separate writes preserve it.

The [evidence archive](https://github.com/CathedralOS/Omega/tree/b0a0e03ebb6f31884c1527a9d5e319283646c5f2) retains inputs, producer hashes and profiles,
commands, source bundles, diagnostic expectations, actual engine observations,
reviews and failed attempts. Disk-full links and the initial cleanup-helper
failure are recorded separately from successful checks. Every edited fixture
has a fresh corpus record; every unselected original row remains unchanged.

Resolving an actual merge conflict brought in a change to scalar-field loan
checking. A [focused hosted run](https://github.com/CathedralOS/Omega/actions/runs/41756991925)
used optimized integrated source `2713116252386e98ce5e87961aca764f8760b217`.
The returned-record match program checks and runs with both exits zero; its
missing-predicate twin rejects. The dirty-forward control rejects the unproved
Pair requirement at a call. The forwarding-marker twin now reaches the intended
Pair restoration refusal on return. Their restoration obligations are therefore
observed on this source, beyond the earlier R16 loan-only errors.

That hosted job failed because the forwarding-marker expectation still named
the old loan error. The corrected expectations were checked against the actual
captured diagnostic messages with the corpus runner's fragment-matching rule.
The programs, compiler and observations were unchanged; no engine was rerun or
relabeled. The original failed report remains intact. Producer preparation took
44.7 minutes; checking took 26.8 seconds and both-engine execution 10.2 seconds.
The producer's original digests and profiles are in that run's records artifact.

Two neighboring stronger-predicate programs still stop while preparing storage
layouts; those refusals do not demonstrate the intended predicates. Those limits
remain recorded beside the programs.

## Baseline and remaining work

The initial failures were observed with optimized producer
`c5cc3d8749224ed1a76fa7abad1707dfd9f5dd1d`, whose compiler and library inputs
match accepted source `3bcdc2d9e3b81ab4af705b8c14d04a6f4021a8dd`. All nine
restored programs wrongly failed after restoration; the flat mutable and
write-only controls failed at the temporary store. After integration onto
`674184316eba783ba3d8324aa1cdf6053b3be0cb`, review verified that the checking
code responsible for those failures was unchanged. The baseline record keeps
its actual producer identity. Its comparison remains limited to the original
40 diagnostic fixtures.

Remaining parent-task work includes construction, case and array updates,
scalar-field loans, filesystem qualification, product/bootstrap migration and
retirement of range suffixes. Execution coverage is macOS ARM64; each fact
program also declares all four hosted entry bindings. GUI qualification remains
deferred at the user's request.

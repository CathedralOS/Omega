# Original scalar backing at a thin foreign boundary

Parent: **NORMALIZED-ABI-LOWERING** in [TASKS.md](../../../../TASKS.md).

Fact: a shared inline scalar-field loan exports its original backing through the
thin foreign ABI without changing adjacent fields.

The same operation writes the four stored bytes of `Cell.word` as `ABC\n`, checks
the foreign return count, and checks all three fields afterward. The row carries
its owned `Cell` through [a local](local/main.omg), [a field](field/main.omg),
[a parameter](parameter/main.omg), [a call result](call_result/main.omg),
[a match join](match_join/main.omg), [a constant name](const_name/main.omg),
[a nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [target bodies](target_body/main.omg), then forms the loan at the consumer.
Both match alternatives execute with distinct checked branch markers, so that place
writes twice and a wrongly selected route traps. Named constants
supply values to ordinary owned storage; they do not provide constant reference
storage. Windows explicitly selects CRT binary stdout before forming the loan; `_write`
must emit the stored LF without text-mode translation. Each sibling
`invalid/main.omg` adds only a write to the field while
`selected` is still live, and must reject for that loan.

This is an original-storage boundary, not a logical scalar-width rule. A shared
logical scalar reference may observe a physically narrower encoded value.
Exporting a thin pointer drops that codec, so every physical backing must retain
the foreign scalar's exact byte footprint. A wide alternative cannot excuse a
narrow or unknown one. Access, initialization, lifetime and range obligations
remain independent; the foreign calling plan supplies none of them.

The original customer remains unchanged as
`pass/host/runtime_descriptor_direct_array_exit`. Fresh main `5f1191d43438`
refused its field loan with `SourceCustodyMismatch` before either engine.
The repaired replay joins the exact projected backing to the reference using
the existing source-projection relation: inline scalar fields do not need an
invented nominal leaf declaration. Ordinary owned value copies and owned arrivals retain
the receiving storage home. Borrowed aggregate arrivals and pointer-bearing
values retain their original backing aliases. Native selection recognizes an actual returned inline scalar's home;
a descriptor sibling does not relocate that field.

## Requirement evidence

- Establishment: ordinary source checking forms a shared loan of the exact
  `Cell.word` field. No callback, retained foreign pointer or provider assertion
  manufactures the backing or its access.
- Identity and lifetime: the pointer refers to the original selected field;
  adjacent sentinel fields and the word remain unchanged. Each twin writes the
  same field before its loan's last use and must reject.
- Composition: the owned carrier reaches the consuming operation through all
  nine places. The match place executes both alternatives; named constants
  initialize owned storage before lending it.
- Soundness: all recovered physical origins must have the requested codec
  width. Narrow and mixed backings, mutation through shared access and stores
  without fit proofs remain rejection controls. No `None` result establishes
  footprint, access or lifetime authority.
- Psi 05–07: checked field loans, ordinary value transport and foreign demands
  become the same retained Terminal operation. Real Terminal execution, rather
  than source checking alone, must agree with the native program.
- Omega 00–02: exact target bindings select their own calling plans and foreign
  locators. POSIX uses `i32` descriptor, pointer and `u64` count with `i64` result;
  Windows CRT uses its distinct `u32` count and `i32` result contract. No target
  spelling substitutes for plan or import admission.
- Omega 03–06: legalization and selection preserve the actual field address and
  every physical-width alternative, followed by ordinary register, stack and
  activation placement. Pointer storage is never the pointed-to scalar bytes.
- Omega 07–09: native emission must produce the authored bytes and exit zero,
  agreeing with Terminal's result on the matching host.

Acceptance is `omega run --both <place>/main.omg`, both exits zero and exact
`ABC\n` output (twice for `match_join`), plus each twin's active-loan rejection.
The same bytes are pinned under
`tests/omega/{pass,fail}/facts/foreign_scalar_backing_through_*`.

Current qualification is recorded in [hosted_qualification.json](hosted_qualification.json).
The actual optimized producer is `6fe58fe64b3825ed91fed6a8fc4e146ae3a9d779`.
The root and all nine places run on each matching host with exact `ABC\n`
bytes (eight bytes for `match_join`), native/Terminal exit zero, nine pinned
corpus agreements and nine intended active-loan rejections. Each host also
reloads all ten canonical Terminal products and the two-input root product;
both input cases preserve the same authored output.

The recorded base `56b550d41ae` wrongly refused `const_name` before execution:
`constant initializer lost its selected construction field`. The initializer
root retained the nested `WORD` dependency, and constructor selection mistook
its `u32` carrier for the enclosing `Cell`. The repair uses only the selection
belonging to the exact constructor occurrence; it retains the complete dependency
inventory and the existing field, case and range obligations. A `const` still
has no stable address: `INITIAL` initializes ordinary owned `backing` storage.

Exact streams and sentinel checks are runtime falsifiers, not address-identity
certificates. Independent inspection of the retained four root native images
also tracks the actual `Cell` home plus four bytes through the pointer descriptor
into the target's imported `write`/`_write`, followed by reads of that same home's
three fields.
That bounded native corroboration is retained separately from engine observations.

The three thin-footprint controls are build-only publication-refusal fixtures
under `tests/omega/pass/host/scalar_reference_{narrow,mixed,mixed_index}_backing_foreign_export`.
Their logical references are Source-valid; Linux x86_64 native compilation must
reject for `thin scalar foreign pointer requires the same byte footprint in every backing`.
The matching native-refusal oracles pin that reason without promising execution;
compiling them on macOS does not execute a foreign ELF image. The unchanged
invalid `Token` constant control additionally preserves the constructor's
specific default-domain refusal.

Historical producer `5985d0f756fb` qualified only macOS. The original direct-array
customer and returned reference-sibling control retain their separately recorded
macOS evidence; this row does not promote those controls to unmeasured hosts.
Remaining descriptor carriers, retained pointers, opaque/view codecs and unrelated
C record-result ABI lanes stay on the parent task.

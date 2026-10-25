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
Both match alternatives execute, so that place writes twice. Named constants
supply values to ordinary owned storage; they do not provide constant reference
storage. Each sibling `invalid/main.omg` adds only a write to the field while
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

Actual optimized producer `5985d0f756fb` on macOS ARM64 completed the public
root, nine places, original direct-array customer and reference-sibling control:
12 native and 12 Terminal executions matched their exact streams and zero exits.
All nine public twins rejected for the intended live loan.

The bindings describe four targets. These matching-host observations qualify only
macOS ARM64; declarations or cross-compilation do not qualify another host.
This row does not close the parent's remaining descriptor carriers, all-host
ABI acceptance or indirect-result qualification. `host/runtime_returned_scalar_field_reference_sibling_exit` additionally checks
that a returned inline field remains usable beside a borrowed reference field.
`host/scalar_reference_mixed_index_backing_foreign_export` joins narrow and wide
references through an indexed descriptor array and must reject for the exact
foreign-footprint reason. The older mixed state-arrival
width control has an independent preexisting incomplete-lowering refusal;
it is retained as historical debt, not recorded as a successful width proof.

# Use a record's bound through a mutable recast

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. These programs view a
`Storage` record as a mutable `Counter`, then use its bound to prove the addition.
`Storage` declares the bound through a checked Boolean helper; `Counter` states
the same bound directly. Both have the same three i32 fields. The source bound
must justify using the `Counter`, and the loan must leave `Storage` valid when
it ends.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [missing source predicate](local/invalid/main.omg) |
| field | [program](field/main.omg) | [missing source predicate](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [missing source predicate](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [missing source predicate](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [missing source predicate](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [missing source predicate](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [missing source predicate](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [missing source predicate](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [missing source predicate](target_body/invalid/main.omg) |

Each consuming body receives existing storage or a borrowed view of it. It computes
`70 + value` before inspecting neighbor canaries, writes `before = 23`, and
observes that write through the original backing after the loan ends. Inputs 60
and minimum i32 distinguish the runtime outcomes 130 and -2147483578. Both match
choices execute. The forwarding helper changes a separate marker from 17 to
29; the direct branch leaves it at 17. The caller checks both outcomes, so
bypassing the helper or selecting the wrong branch fails at runtime.

The const-name place materializes a named portable `Storage` value into runtime
backing before forming the mutable view. It does not establish a portable
constant reference or transport an emitted runtime loan through a constant.
All positives bind the four hosted ProgramEntry slots and declare exit 0.
Actual native/Terminal agreement is recorded on macOS ARM64; other authored
target bodies are checked, not claimed as cross-host native execution.

- **Establishment:** construction and the checked field-write helper establish
  the source record's bound. That bound proves the target predicate before a
  read or call treats the view as a valid `Counter`.
- **Identity and lifetime:** exact schema field symbols and source-selected
  comparison meaning govern the domain. The emitted reference retains its
  existing exclusive loan, descriptor qualifications and backing identity.
- **Composition:** fields, parameter calls, returns, both joins, nested and
  independent machines transport the view before consuming its carried fact.
  The reject twins remove only `Storage`'s predicate. Local and const-name places
  cannot prove the addition safe; the other seven cannot establish `Counter`
  before valid-value use or transport. They add no caller guards and keep the
  same arithmetic.
- **Soundness:** the [live backing write](controls/live_backing_write/main.omg)
  rejects mutation while the exclusive view remains live. The
  [stronger-source](controls/stronger/main.omg) and
  [qualified-constant](controls/qualified_constant/main.omg) programs still fail
  while the compiler prepares their storage layouts, before it checks the
  intended record predicates. Those requirements remain open. Transporting a
  reference still grants no ownership of its contents.
- **Psi 06–08:** erased standing proofs justify the same checked arithmetic;
  reference results and match receipts retain descriptor, loan and linear-claim
  custody through Terminal lowering and interpreter execution.
- **Omega 00–02:** ordinary fixed aggregate layout, entry bindings and calling
  plans realize the exact three-field schema and mutable view.
- **Omega 03–06:** exact i32 addition, borrowed writes and returned view lanes use
  the existing instruction, register and activation-frame owners.
- **Omega 07–09:** actual native images agree with Terminal Psi and preserve
  the original backing and both neighbor canaries.

Governing contracts: [recasts](../../../../../wiki/spec/layouts/recasts.md),
[default domains](../../../../../wiki/spec/language/dependent_values.md),
[numeric meaning](../../../../../wiki/spec/language/numeric_values.md),
[loans](../../../../../wiki/spec/terminal-psi/loans.md) and
[content custody](../../../../../wiki/spec/resources/content_custody.md).

The frozen fresh-main discovery base is 6f5fa345cfa34981899f3e545d1ba6b83be742d1.
The helper source incorrectly refused recast eligibility while the literal
control ran native 0/Terminal 0; the missing-predicate twin correctly refused.
A later unchanged match source exposed a descriptor-versus-owned custody
lowering error. Exact source, profile and command observations are retained in
the qualification evidence. The qualified-constant control already refused on
the baseline and is parity evidence, not a demonstrated accepting bypass.

Fresh qualification runs all nine programs with native and Terminal exits of
zero, and rejects all nine twins. The
[record-restoration report](../../../../../wiki/drafts/measurements/record_update_restoration.md)
records the exact producer and evidence for the marker-writing match repair.

The parent task still needs construction checks, coupled and stronger predicate
proofs, collection and slice support, recursive filesystem parameters,
product/bootstrap migration and retired suffix removal.

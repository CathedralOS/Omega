# data_where_standing

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. This row establishes a
common-field default-domain proposition, transports the resulting value through
the nine places, and consumes that standing proposition in the original exact
addition. It includes the constant declaration's ordinary default-domain check:
`self.value` must select the exact completed constant's field, with the same
meaning as the canonical unqualified field spelling.

Each place now constructs its `Counter` through an ordinary helper that writes
its field through an exclusive borrow. The helper's normal-return guarantee
restores `value <= 60`; returning the `Counter` carries its declared default
domain. The receiving place must preserve that fact, rather than replaying the
initializer or adding a caller guard to prove the arithmetic. The upper and lower
runtime observations remain 130 and -2147483578. Every reject twin removes only
`Counter`'s `where` clause, so its generic consuming body cannot prove the exact
addition for an arbitrary `i32` field.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing standing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing standing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing standing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing standing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing standing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing standing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing standing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing standing predicate](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [missing standing predicate](target_body/invalid/main.omg) |

The [root program](main.omg) retains explicit `Main` storage, its field read and
`hits` write, with a [root reject](invalid/main.omg). Its entry helper performs
the establishing write before calling the independent consuming machine. All
positive projects declare exit 0 and bind all four hosted `ProgramEntry` slots.
The row and its twins are pinned under `tests/omega/{pass,fail}/facts`.

## Establishment and preservation

- **Establishment:** construction and the checked borrowed write establish the
  complete default domain before the `Counter` is returned or consumed. A
  constant validates every declaration predicate against its actual completed
  fields before publishing an identity. [Default domains](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
  and [logical subjects](../../../../wiki/spec/language/domains.md#declaration-and-membership)
  govern these checks.
- **Identity and lifetime:** `self` belongs to the selected data predicate's
  field ledger. It creates no runtime object, receiver or access authority.
  Exclusive writes and normal-return guarantees retain the ordinary
  [invariant-window fences](../../../../wiki/spec/language/dependent_values.md#invariant-windows).
- **Composition:** the produced value travels through fields, ordinary calls,
  both match choices, constants and separately checked target bodies. Each
  generic consuming body proves `70 + value` from the standing predicate,
  independently of the entry's concrete value or later result guard.
- **Soundness:** [constant eligibility and invocation admission](../../../../wiki/spec/language/constants.md)
  remain separate from predicate proof. False, missing or undecidable predicates
  do not publish a constant identity. [Focused controls](controls/README.md)
  distinguish complete conjunctions and nested projections from missing fields,
  missing members and same-spelled outer constants.
- **Psi 05–07:** the standing predicate supplies erased proof evidence; the
  ordinary record, calls, joins and nested transitions retain runtime meaning
  through Terminal Psi. Every positive requires actual Terminal/native agreement.
- **Omega 00–02:** ordinary four-target entry bindings and the existing record
  layout/calling-plan owners realize the value. The proof introduces no runtime
  argument, field or receiver.
- **Omega 03–06:** the same exact i32 addition consumes the checked bound; calls
  and field writes use ordinary register, frame and access planning.
- **Omega 07–09 and emission:** the submitted qualification records actual
  macOS ARM64 native execution and Terminal agreement. Authored bindings and
  checked target bodies do not establish native execution on other hosts.

## Evidence and remaining scope

The fixed discovery base is `02e7d90a49aeeee2b42de7bb3a53fa4b4f32505a`.
Valid explicit-subject literal and helper-produced constants refused the early
constant default-domain proof gate, while equivalent unqualified predicates and
ordinary runtime helper results executed successfully. Direct, borrowed and
indexed runtime field-write probes were already passing and establish coverage,
not a need for additional compiler edits. Exact frozen sources, diagnostics,
producer profiles and engine results belong to the qualification record and PR.
The original bounds and arithmetic remain unchanged by source migration.

This row does not close the parent task. Generic relational/guarded-case flow,
mutable recasts, collection and slice compositions, recursive filesystem
parameters, product/bootstrap migration and removal of the retired suffix
representation retain their own remaining acceptance. Closed constant controls
here do not claim arbitrary predicate-helper evaluation or cross-host runtime
qualification.

# Borrowed field production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, reference predicate read
inventories and their transport through ordinary calls. This production belongs
to [data_where_standing](../README.md): the bound and operation are unchanged.
The difference from its [predicate-helper production](../predicate_helper/README.md)
is that `Counter.value` stores a shared reference. Passing that descriptor to
`read` must connect the helper's checked result equation to the exact current
referent constrained by the containing record's default predicate.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing predicate](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [missing predicate](target_body/invalid/main.omg) |

Every twin removes only `Counter`'s default predicate. Public consuming bodies
must prove the exact addition independently of the entry's concrete values.
The match place executes both choices: 60 yields 130 and -17 yields 53. Returning
one arm for both choices traps. Entries also check neighboring fields, markers
and the unchanged backing scalars. Target-body bindings retain independent
x86/ARM tags, so selecting the wrong provider traps. The const-name place materializes named bounded `Backing` values, then an
independently checked bridge borrows each local copy into `Counter`. The
constant declaration itself supplies neither an address nor a lifetime. All positive entries declare exit 0 and bind all four
hosted targets. Exact source copies and rejection/engine oracles are pinned
under `tests/omega/{pass,fail}/facts/data_where_standing_borrowed_field_*`.

[Focused controls](controls/README.md) separate a copied scalar snapshot from
a rebound reference descriptor, a bounded field from an unrestricted sibling,
a disjoint intervening write, and a join with one unbounded incoming reference.

## Requirements and evidence

- **Establishment:** construction establishes the full default predicate by
  invoking the checked Boolean helper on the actual reference field. The
  independent consuming body imports that standing predicate; its result guard
  supplies no arithmetic premise. See [default domains](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization).
- **Identity and lifetime:** incoming descriptor identity, current pointee
  contents and detached scalar results remain separate. A saved scalar survives
  rebinding its former descriptor; a later read of the replacement receives no
  old bound. See [fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md).
- **Composition:** the record/reference crosses locals, fields, owned parameters,
  ordinary returns, both match arms and nested/independent machines. The helper's
  normal-return equation substitutes the captured actual, under the complete
  origin inventory, rather than replaying an initializer. See [invocation contracts](../../../../../wiki/spec/language/state_contracts.md).
- **Soundness:** readable authority, complete origin inventories, exact carrier
  and arithmetic policy, and invalidation by overlapping writes remain required.
  A descriptor alone supplies no numeric contents. The unbounded-arm control
  refuses even when the other arm has the standing bound. See [loans](../../../../../wiki/spec/terminal-psi/loans.md).
- **Checked-to-Terminal Psi:** the bound is proof evidence; records, descriptors,
  reads, calls and branch selection retain executable meaning in Terminal Psi.
  This production does not introduce cleanup or mutable-loan restoration.
- **Provider, layout and ABI:** existing record/reference and calling-plan owners
  realize the carrier and selected body. All four hosted Build entries are
  authored; the target-body place uses ordinary target-keyed provider rows.
- **Instructions and storage:** native execution observes the actual pointer
  load, scalar result, exact addition and nonzero neighboring fields through
  the existing frame/register owners.
- **Encoding and emission:** acceptance requires actual macOS ARM64 native exit 0
  and Terminal exit 0 for every positive. Authored bindings and checked bodies
  do not establish native execution on another host.

The fixed discovery base is `22b52bc721476d30c8596460125b3ee23f6a04ba`.
Its optimized CLI rejected all nine frozen positives at the exact i32 addition.
Direct member reads, calls borrowing an owned scalar field, and an explicitly
qualified reference formal already checked. The first repair admitted eight
places; the match join retained the complete origins but lost the selected
reference-field observation. Original diagnostics, source hashes, optimized
producer identities and actual engine observations belong to the PR evidence.

This production does not close the parent task. Recursive and remaining
case/collection reference inventories, other field/case write handback,
filesystem qualification, the remaining source migration and suffix deletion
retain their acceptance. GUI qualification remains pending separately.

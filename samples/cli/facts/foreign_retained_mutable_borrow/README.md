# Retained mutable foreign backing

Parent: **NORMALIZED-ABI-LOWERING** in `TASKS.md`.

A mutable foreign registration retains exclusive access to the exact original Buffer backing until completion consumes Pending; completion writes 73 while an earlier copied value remains 37.

The real C provider writes 41 during registration, retains the original pointer,
and writes 73 during completion. Every place checks the observed value, the
original Buffer value and the earlier scalar copy. All entries bind
`ProgramEntry` for Windows x86-64, Linux x86-64, macOS ARM64 and Linux ARM64.

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [target body](target_body/main.omg). Each has an `invalid/main.omg` twin
which inserts one read of the original Buffer before its registration completes.
The row and twins are pinned under
`tests/omega/{pass,fail}/facts/foreign_retained_mutable_borrow_*`.

## Contracts and requirements

- Establishment: the boundary accepts an exact mutable loan of qualified Buffer
  and returns a linear Pending carrying that loan. Its normal guarantee preserves
  `Buffer::Owned` on the mutable formal. A qualification on entry alone is not a
  preservation promise. The guarantee does not release Pending's exclusive debt.
- Identity and lifetime: C retains the original fixed-record cell, with its exact
  allocation, range, access and lease. Registration's first write and completion's
  later write reach that same cell. A scalar copied before registration stays 37.
- Composition: all nine places transport the debt without a reference field or
  a new spatial grant. [Mixed issuers](controls/mixed_issuers/main.omg) give the same
  Pending type shared and mutable suppliers, both through one helper and field;
  their actual access remains distinct. [Reverse completion](controls/reverse_completion/main.omg)
  keeps two disjoint registrations independent.
- Soundness: the nine twins reject live reads; [overlapping registration](controls/overlapping_registration/main.omg)
  rejects another mutable registration over live backing; [one remaining registration](controls/other_registration_live/main.omg)
  remains exclusive after the other completes. [Stale completion facts](controls/stale_completion_fact/main.omg)
  cannot reuse registration's value-41 guarantee after completion may write.
  [An uncovered qualification](controls/uncovered_qualification/main.omg) executes
  natively but Terminal refuses the mutable call because its normal guarantee
  does not cover the extra caller qualification. That refusal is an explicit
  limitation, not a positive execution result.
- Psi 05–07: checked custody retains the actual supplier access. Normal mutable
  parameter memberships have their own lowered roster, separate from result
  qualifications. Terminal wire format 188 carries that roster. Root and scalar
  leaf restoration check the original formal owner and the same coverage;
  stronger ancestor or projected qualifications cannot inherit it.
- Omega 00–02: selected foreign bindings retain their ordinary C ABI calling
  plans and fixed-record layouts. The existing admitted native provider adapters
  join the complete source contract and original backing; scalar token bits alone
  establish no issuer, pointer, storage or custody authority.
- Omega 03–06: stable original storage survives calls, helper results, field
  moves and joins. Lifetime-only transfers preserve the exact supplier debt;
  they do not expose a reference or attenuate exclusive access to shared access.
- Omega 07–09: matching-host PE, ELF and Mach-O programs and the Terminal engine
  execute the same authored operations against the actual C provider. The gate
  requires both real zero exits, intended rejection diagnostics and C lifecycle
  self-tests, including nine deliberate native traps for each provider.

## Running

Build the matching provider with
`python3 samples/cli/facts/foreign_retained_mutable_borrow/fixture/prepare.py`.
Put `build/foreign_retained_mutable_fixture` on `LD_LIBRARY_PATH` (Linux),
`DYLD_LIBRARY_PATH` (macOS), or `PATH` (Windows), then run
`omega run --both samples/cli/facts/foreign_retained_mutable_borrow/local/main.omg`.
The mixed-issuer control also needs the shared fixture and its library directory.
`tools/ci/compiler_retained_foreign.py` prepares both providers and runs both
retained cohorts on the hosted Windows/Linux jobs.

## Verification and remaining scope

Qualification is in progress. The fixed author base is `29f58e13cc0`.
The optimized producer `f8d4d2c2c65e` passes the nine places and twins on macOS
ARM64; candidate `355494c10ca` additionally passes both retained corpus cohorts,
all public CLI samples and the auxiliary controls on that host. Hosted candidate
`355494c10ca` is being qualified by runs `40271548919` and `40271551351`.

This establishes direct retained mutable loans for the exercised fixed-record
ABI, with explicit normal preservation. Transfer of owned backing into a foreign
protocol, permitted private snapshots, wider resident/view codecs, concurrent
external access and write-only retention retain their separate obligations.
Foreign behavior remains an admitted trusted provider premise. These tests do
not prove arbitrary mathematical postconditions about C implementations.

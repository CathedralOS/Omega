# Provider-dependent scalar contract endpoint

The selected provider makes `limit()` equal to 7. The input contract
`requires value <= limit()` preserves the bound needed to prove `7u64 - value`.

This row tests bounds computed by a selected provider and written in an ordinary
`requires` clause. It belongs to **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. No named
domain is needed solely to express that bound.

The [generic-function production](closed_owner/README.md) carries the same fact
when the selected provider call supplies constants inside a function that still
has its own generic parameter. It also checks both match outcomes and field
neighbors at runtime.

The [pending-call production](pending_call/README.md) keeps that same bound when
the helper first makes a call with its own requirement. Its twins require that
same pending helper result to admit the call. All nine positives run with
native/Terminal agreement, and every circular twin rejects during checking.

The nine programs above also make `limit()` admit input 1 against a calculated
prerequisite. The matching branch supplies anonymous `254 + 1 = 255`; that value
must fit `u8` before Wrapping addition produces 1. The fallback supplies 0,
so its result is 3. Each `main()` checks both results, the admitted call's result
257, and the selected provider limit 7. `limit()` itself checks the matching
calculation before choosing the provider endpoint; this is not a caller guard
used to justify an earlier partial operation.

The existing Endpoint declarations, provider 7/6 twins, input 7, and subtraction
`7u64 - value` remain. The field program checks neighbors 31 and 47. The match
program retains its original two bound-7 cases and adds bound-7/bound-3 cases
whose differences are 0 and 4. The selected-target tags remain 17 and 29.
These extended sources still need fresh qualification; the earlier separate
`closed_argument` observations do not qualify the current programs.

Two focused controls retain the missing obligations without duplicating the
nine-place row:

- [Input conditions](../../../../tests/omega/fail/generics/declared_endpoint_contract_inputs/main.omg)
  require four distinct refusals: zero below the named floor, 2^63 - 1 below
  the unsigned floor, 256 at an exclusive upper bound, and input 2 above the
  calculated prerequisite 1.
- [Initial integer fit](../../../../tests/omega/fail/arithmetic/anonymous_matched_argument_initial_fit/main.omg)
  changes 254 to 255. Anonymous addition then produces 256, which must reject
  before entering the Wrapping `u8` helper.

The [guarded-crash production](guarded_crash/README.md) keeps the same bound
when its helper has a guarded crash contract. It requires the compiler to prove
the invocation safe before running the helper; changing only the divisor to zero
must reject during admission.

The frozen [local positive](local/main.omg) is the discovery program. Its public
acceptance command is:

```sh
omega run --both samples/cli/facts/provider_contract_endpoint/local/main.omg
```

At producer `16b381462327a3f7d1eb8c7218bdb81114a8a10a`, the canonical Build
selection first exits 200: `Build declaration operand does not resolve to one
visible product declaration of the required kind` at `build.omg:3:28`. Product
suppliers are deliberately assembled after Build returns; source checking must
retain an inert authenticated description until that existing join.

Earlier discovery through the retired `TargetProviders` helper reached the
`limit` denotation refusal and symbolic result-bound loss. Those observations
remain historical only. The current witnesses use ordinary
`Build::select_provider`, nested machines with their own `Trap` effects, and
both match choices. The independently migrated `provider_const_endpoint`
customer also demonstrates the clause failure directly: changing only its
suffix to `requires value <= limit()` loses the proof of `7 <= limit()`.
At immutable producer `e89eee400dbaa88425faf8ae48554b7c020674d1`, all nine
positive places execute with native exit 0 and Terminal exit 0 on macOS ARM64.
All nine twins reject the captured input 7 against the selected endpoint 6.
The focused corpus records 38 checks (17 positives and 21 intended rejects),
17 native/Terminal agreements at exit 0, and six affected historical fixtures
with no losses or gains against the fixed baseline. A fresh public local run
also observes exit 0 in both engines. Exact-candidate review remains a separate
publication gate.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [provider returns 6](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [provider returns 6](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [provider returns 6](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [provider returns 6](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [provider returns 6](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [provider returns 6](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [provider returns 6](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [provider returns 6](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [provider returns 6](target_body/invalid/main.omg) |

Each reject changes only the selected provider's integer from 7 to 6. The
consuming value and subtraction stay identical. The missing obligation must
identify that the captured input 7 exceeds the selected closed endpoint 6.

## Requirement evidence

- **Establishment:** the exact selected ordinary checked provider computes the
  endpoint. Call admission records the captured input's relation to that recipe;
  neither the requirement nor an `asm requires` goal establishes itself.
- **Identity and lifetime:** the integer comparison names an immutable captured
  scalar. The recipe retains its exact call expression and selected target/
  provider receipt; another target's scalar is not interchangeable evidence.
- **Composition:** formal/actual substitution uses captured scalar operands.
  Result upper-bound recipes travel with the produced value across calls and
  joins into exact subtraction, without a caller guard recreating the fact.
- **Soundness:** every ordinary invocation still owes termination, empty runtime
  reach and observation freedom under the selected provider. Unclosed or false
  recipe relations refuse before Terminal production. Symbolic mathematics and
  runtime-dependent calls retain their ordinary checking route.
- **Psi 06–08:** contract comparisons remain proof facts, integer root folds
  retain their producing dependencies, and the checked result reaches canonical
  Terminal Psi. Actual interpreter/native agreement is required for each place.
- **Omega 00–02:** all four hosted `ProgramEntry` bindings and the exact provider
  defaults join the same value closure. Scalar calling plans retain normal u64
  argument/result placement; proof recipes contribute no runtime parameter.
- **Omega 03–06:** exact u64 subtraction consumes the transported integer-range
  recipe. Ordinary call activation storage and callee-saved preservation remain
  on the standard backend route.
- **Omega 07–09 and emission:** actual macOS ARM64 native execution must agree
  with Terminal execution at exit 0. Other hosted targets' bindings are authored;
  native execution on those hosts is a separate, currently unverified claim.

The nine places and their twins are pinned under `tests/omega/{pass,fail}/facts`.
[Existing soundness controls](controls/README.md) pin formation, target ownership,
write snapshots, result identity and authenticated Build request transport.
The focused corpus passes on the current captured producer; exact-candidate
review remains a publication gate.

The three named endpoint customers now use explicit free machines, selected
provider/operator evidence, and ordinary `requires` clauses. The historical
compile-only customer checks; the runtime customer retains its exact f32
rounding guard and observes native/Terminal exit 70 through the public CLI.
The invalid customer rejects its input 9 against the selected endpoint 7
during ordinary candidate checking and the pinned corpus. Its earlier missing
entry-policy refusal is not credited as that obligation. Normal dependency
review is scoped to this project, exact source and target.

Parent acceptance still includes remaining field/case/recast
and collection compositions, filesystem parameter migration, product/bootstrap
source migration, and retiring the suffix representation after its customers
are migrated. This row alone does not close that parent outcome.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.

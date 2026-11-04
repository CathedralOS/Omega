# Runtime selection before a const-only method

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A runtime-selected family calls its const-only method through the declared width list, retains its required target call, and returns the selected row's value.

`RuntimeFamily` accepts a runtime width of 16 or 32. Its `BridgeToConst`
implementation calls the const-only `Family` method. The compiler must generate
and check the closed bodies before that call; proving that an ordinary runtime
argument belongs to the list does not itself make it constant.

The current programs also require `WideRowTarget::preserve` in row 32.
`HostedWideRow` preserves the input, and explicit Build rows select it for each
hosted target. Both original widths and all four expected results remain the
same. The nine strict runtime-to-const twins retain their original failure.

The focused [uncalled-row program](../../../../tests/omega/pass/traits/finite_family_uncalled_target/main.omg)
requests only width 16 but retains the complete declared family. Its
[missing-supply control](../../../../tests/omega/fail/traits/finite_family_uncalled_target_missing_supply/main.omg)
has identical Main source and removes only the four target-provider selections.
Its unconfigured source is valid. Checking the selected target must refuse the
missing requirement before either engine runs; its diagnostic oracle names that
missing provider.

These target-call additions are awaiting qualification. The earlier const-only
bridge evidence below belongs to its recorded program revisions.

Every positive transports both `Selected` and `Other` through its named place.
It calls each at both widths. Their results are 16/17 at width 16 and 32/33 at
width 32. Choosing the wrong implementation or width fails the runtime check.
The field case also checks its surrounding values, and the match case exercises
both arms. Result comparisons observe execution; they do not establish the
static obligations needed to form the calls.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Selected target code | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each reject twin changes only the call in `selected_width`: it sends the
ordinary runtime argument directly to the const-only method. The width's
membership premise, implementations, transport and result checks remain the
same.

The target case passes the selector and width to `FamilyForward`. Four explicit
Build rows choose the checked `HostedFamily` implementation. Its requirement
has empty service reach; main records the direct boundary call with `invokes`.
Every positive binds the four hosted targets and expects exit 0.

## Requirements

- The declaring interfaces own the complete width list. The compiler must keep
  their identities when a closed call's parameters and predicates disappear.
- The provider's own complete finite declaration permits its generic inputs in
  const positions. Current callers or an unrelated finite predicate cannot
  supply that permission. Original declared carriers remain exact.
- Each required closed body must check, including width 32 when main only calls
  width 16. Correlated parameters keep their authored pairs; the compiler must
  not invent additional combinations.
- Ordinary parameters, unrestricted generic values and runtime expressions
  remain runtime inputs. A direct call to a const-only method still rejects.
- Each transport preserves the implementation selector and width. Terminal
  interpretation and native execution must both produce the authored results.
- Provider selection, argument placement, register/stack storage and final
  encoding must preserve these inputs. Successful source checking alone does
  not establish execution.

The governing rules are [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
and [Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

## Evidence and remaining work

The earlier literal-only row passed all nine places and intended controls on
Linux x86-64 at `74bbe3c7f0f173fe5063b15d085e810758a215be`:
[run 41560780946](https://github.com/CathedralOS/Omega/actions/runs/41560780946).
That result establishes existing literal-call behavior.

The [original local bridge](https://github.com/CathedralOS/Omega/blob/738740fe657619248c4085b47b376e3788e9f175/samples/cli/facts/finite_family_const_roster/bridge/local/main.omg) exposed a current compiler defect:
optimized compiler `17be13e1a9ff` refused its valid generic provider's const call.
At `d023d9db7fb8`, that frozen program, a computed const expression, correlated
parameter pairs, and an explicit nested const call execute with native and
Terminal exits 0 on macOS ARM64. Runtime-to-const, unrestricted-value,
wrong-carrier, unsupported-row and crossed-pair controls still reject for their
intended causes. The claim retains the exact producer, sources and raw outputs.

At `a50253a693d8`, a fresh optimized compiler and corpus runner pass the complete
local selection on macOS ARM64: ten positives check, eleven controls reject for
the intended reasons, and all ten corpus positives execute with native and
Terminal exits 0. All nine sample positives also execute with both exits 0;
their nine twins reject as intended. The claim retains the producer profiles,
316 input hashes, reports and raw outputs. The source-only corpus records
retain the actual `a50253a693d8` stamp. Its replay source archive is preserved
in [PR #1861](https://github.com/CathedralOS/Omega/pull/1861).

The ordinary forwarding case first exposed a second defect: the arithmetic
checker discarded a complete disjunction that was identical to the called
machine's requirement after argument substitution. The repair compares complete
predicates, preserving their selected operations and parameter pairs. It does
not infer individual pairs from unrelated conditions.

The merged compiler at `c7a12e81bd2a` passes the same complete selection on all
four matching hosts. Each checks ten positives, rejects eleven controls for
their intended causes, executes all ten corpus positives with native and
Terminal exits 0, and passes the nine sample positives and nine intended twins.
The raw reports, optimized compiler profiles and all 316 input hashes have been
audited independently.

- [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41922981066)
- [Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41922985742)
- [macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41922995281)
- [Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41922990615)

A fresh macOS ARM64 compiler at `538cc2df6785`, after the next main merge,
also passes that selection. The nine existing ARM runtime records now contain
the actual `c7a12e81bd2a` successes. Source records retain their measured
`a50253a693d8` revision. Replay archives for all three compiler revisions are
preserved in [PR #1861](https://github.com/CathedralOS/Omega/pull/1861).

The remaining parent-task requirement is target-ineligible bridge qualification.
The const-only bridge results do not close that requirement.


The focused [forwarding program](../../../../tests/omega/pass/traits/finite_family_const_forwarding/main.omg)
combines computed const arguments, the original correlated pairs, and a public
const-forwarding machine. Its twin removes only that machine's width requirement
and must fail the ordinary call-precondition proof. A separate
[missing-row control](../../../../tests/omega/fail/traits/finite_family_const_missing_row/main.omg)
keeps the runtime family's two rows while removing the kernel's second row;
main requests only the first. The repository keeps these focused controls beside
the existing transport fixtures instead of adding another fact folder.

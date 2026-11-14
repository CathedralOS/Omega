# Runtime selection before a const-only method

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A runtime-selected family calls its const-only method through the declared width list, retains its required target call, and returns the selected row's value.

`RuntimeFamily` accepts width 16 or 32 at runtime. Its `BridgeToConst`
implementation forwards that width to the const-only `Family` method. The
interfaces declare the complete width list; the compiler checks a closed body
for each declared width before runtime selection. An ordinary argument does not
become constant merely because a caller proves it belongs to that list.

Every positive carries both `Selected` and `Other` through its named place and
calls each at both widths. The results are 16/17 at width 16 and 32/33 at width
32. The wider row calls `WideRowTarget::preserve`; explicit Build choices supply
`HostedWideRow` on all four hosted targets. The field example also checks its
surrounding values, and the match example executes both arms. Result comparisons
observe execution; they do not establish the calls' static obligations.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [field/main.omg](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Selected target code | [target_body/main.omg](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Each twin changes only `selected_width`: it sends the ordinary runtime argument
directly to the const-only method. The membership premise, implementations,
transport and result checks remain. All nine pairs have identical Main and Build
copies under `tests/omega/{pass,fail}/facts/finite_family_const_roster/`.

The target example forwards through `FamilyForward`. Four explicit Build
choices select `HostedFamily`. Every positive binds all four hosted entries and
expects exit 0.

## Focused controls

- The [forwarding program](../../../../tests/omega/pass/traits/finite_family_const_forwarding/main.omg)
  uses computed const arguments, correlated parameter pairs and a public
  forwarding machine. Its [twin](../../../../tests/omega/fail/traits/finite_family_const_forwarding/main.omg)
  removes only that machine's width requirement. The ordinary call-precondition
  proof must fail.
- The [missing-row control](../../../../tests/omega/fail/traits/finite_family_const_missing_row/main.omg)
  declares both runtime widths but removes width 32 from the const-only family.
  Main calls only width 16. The complete declared family still requires the
  missing row.
- The [uncalled-row program](../../../../tests/omega/pass/traits/finite_family_uncalled_target/main.omg)
  calls only width 16 while retaining both declared rows. Its
  [missing-provider control](../../../../tests/omega/fail/traits/finite_family_uncalled_target_missing_supply/main.omg)
  has identical Main source and removes only the four provider choices.
  Unconfigured source is valid; target checking must report missing
  `WideRowTarget::preserve` before either engine runs. The target call graph
  follows actual calls in every retained family body, including the uncalled
  wider row. Advertising an invocation ceiling alone does not create this edge.

## Requirements

The interfaces own the finite width list and its declaration identities.
Only a provider's own complete finite declaration permits its generic input in
const positions. Callers and unrelated predicates cannot grant that permission;
ordinary inputs remain runtime values. Correlated parameters retain their
actual pairs without generating other combinations.

Each required body must check and retain its target calls, including rows that
main never selects. Each transported value retains the selected implementation,
width and original carrier. Terminal Psi and native execution must both produce
the authored results. Build selection, argument placement, storage and encoding
must preserve those values through execution.

These obligations follow [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
and [Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

## Evidence and remaining work

The valid const-forwarding program originally failed with optimized compiler
`17be13e1a9ff`. A second failure discarded an identical complete disjunction at
an ordinary forwarding call. The repair retains the provider's declared list and
compares whole predicates with their selected operations and substituted
arguments. It does not derive individual pairs from unrelated conditions.

The const-only bridge passed all nine places and twins on all four hosts with
compiler `c7a12e81bd2a`: ten valid checking cases, eleven intended refusals, ten
native/Terminal exit-zero agreements and nine sample execution pairs per host.
A later macOS compiler at `538cc2df6785` passed the same scope. Exact source and
raw observations are retained in [PR #1861](https://github.com/CathedralOS/Omega/pull/1861).
These earlier programs did not contain the current target-call additions.

The target-call programs at `64d08af188aa` pass on macOS ARM64 with that verified
`538cc2df6785` compiler: eleven valid checking cases, twelve intended refusals,
eleven native/Terminal exit-zero agreements, and nine sample execution pairs
with nine rejecting twins. The compiler code, bundled
library, tools and build configuration match. The twenty changed checking
records retain their actual `64d08af188aa` stamp.

The same target-call scope also passes on
[Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41937582905),
[Linux ARM64](https://github.com/CathedralOS/Omega/actions/runs/41937586127) and
[Windows x86-64](https://github.com/CathedralOS/Omega/actions/runs/41937589239).
Those runs use optimized compilers and program inputs `64d08af188aa`; each has
11 valid checking cases, 12 intended refusals, 11 actual native/Terminal zero
agreements, and nine sample execution pairs with nine intended twins.

This completes target-ineligible bridge qualification on all four matching
hosts. It establishes that every declared family body retains its required
target call, including an uncalled wider row. The broader parent acceptance remains open.
[Returned Linear tokens](../finite_family_exclusive_result/README.md#linear-result)
retain their cleanup in a separately qualified row; other result representations
and product-compiler support still need their own acceptance.
[PR #1869](https://github.com/CathedralOS/Omega/pull/1869) retains exact source,
original producer identities and raw qualification. No execution of a final
metadata commit or a later main revision is inferred.

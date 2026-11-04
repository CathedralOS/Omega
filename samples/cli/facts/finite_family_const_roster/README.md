# Runtime selection before a const-only method

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A runtime-selected family calls its const-only method through the declared width list and returns the selected row's value.

`RuntimeFamily` accepts a runtime width of 16 or 32. Its `BridgeToConst`
implementation calls the const-only `Family` method. The compiler must generate
and check the closed bodies before that call; proving that an ordinary runtime
argument belongs to the list does not itself make it constant.

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
same. The separate [runtime-const control](runtime_const/invalid/main.omg) also
retains this restriction.

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

The [local bridge](bridge/local/main.omg) exposed a current compiler defect:
optimized compiler `17be13e1a9ff` refused its valid generic provider's const call.
At `d023d9db7fb8`, that frozen program, a computed const expression, correlated
parameter pairs, and an explicit nested const call execute with native and
Terminal exits 0 on macOS ARM64. Runtime-to-const, unrestricted-value,
wrong-carrier, unsupported-row and crossed-pair controls still reject for their
intended causes. The claim retains the exact producer, sources and raw outputs.

The nine updated transport places and their corpus mirrors still need fresh
execution and intended rejections. Other-host execution and target-ineligible
bridge qualification remain open. These local results do not close the parent
task.

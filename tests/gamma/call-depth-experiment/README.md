# Gamma call-depth experiment

Exploratory evidence for `BOOTSTRAP-AUTHORING-READABILITY` in
`TASKS_BOOTSTRAP.md`. It is not accepted Gamma syntax, not a change to the
selected evaluator, and not a Delta compiler change. The customer is the
Gamma-written Delta checker's traversal: its direct-recursion form needs one
live call per nested Delta expression level, which the selected evaluator's
256 nested call contexts refuse. The item left "an evaluator-owned explicit
stack" open as an engineering comparison requiring its full containment and
failure argument; this directory supplies the measured boundary and that
argument.

Run from the repository root:

```sh
sh tests/gamma/call-depth-experiment/run.sh
sh tests/gamma/call-depth-experiment/run.sh --reference
```

The first form runs the selected Beta evaluator and needs a host Alpha seed
(macOS arm64 or Windows x64). The second runs the selected evaluator tape
under the untrusted `tests/alpha/reference/alpha_ref.py` and prints Alpha
instruction counts; it exists so a host without a seed can still reproduce
the boundary below, and its output labels itself diagnostic.

## Fixtures

`descend` in each `non_tail_*` source keeps one call context live per
recursion level because the recursive call sits inside the `+` operand, so
`descend n` reaches exactly `n` live contexts.

| Fixture | Shape | Depth | Observation |
| --- | --- | ---: | --- |
| `non_tail_256.gamma` | scalar, non-tail | 256 | status 0, byte 1 |
| `non_tail_257.gamma` | scalar, non-tail | 257 | status 3, empty stdout |
| `non_tail_1024.gamma` | scalar, non-tail | 1,024 | status 3, empty stdout |
| `non_tail_app_257.gamma` | application, non-tail | 257 | status 250, empty stdout |
| `tail_4096.gamma` | scalar, proper tail | 4,096 | status 0, byte 42 |

## Measured under `--reference` at the selected tape

macOS x86_64, untrusted reference interpreter (no Alpha seed on this host):

| Fixture | Source bytes | Alpha steps |
| --- | ---: | ---: |
| `non_tail_256.gamma` | 329 | 839,275 |
| `non_tail_257.gamma` | 302 | 800,731 |
| `non_tail_1024.gamma` | 421 | 805,059 |
| `non_tail_app_257.gamma` | 400 | 803,418 |
| `tail_4096.gamma` | 396 | 10,623,005 |

The refused cases burn their whole descent and then fail at the 257th
context allocation; the refusal is an exact boundary, not a gradual
degradation. The application form confirms the raw status 250 the rejected
direct-recursion rewrite observed. The tail control completes sixteen times
the non-tail bound in constant space, which is why the checker's explicit
continuation stack — evaluated through `typing_visit`/`typing_resume` tail
calls over pair-heap frames — is the shape that currently survives.

## Where the bound comes from

Per [the selected profile](../../../bootstrap/2_gamma/EVALUATOR_PROFILE.md),
each live non-tail call owns one three-word context row at
`0x01f00000..0x02000000` and one six-word activation row at
`0x01e00000..0x01f00000`. Neither region is the binding constraint:

| Resource | Region | Record | Physical rows | Profile cap |
| --- | --- | ---: | ---: | ---: |
| call contexts | 1 MiB | 24 B | 43,690 | 256 |
| activation frames | 1 MiB | 48 B | 21,845 | 257 reachable |

The binding constraint is the hidden Alpha call stack. Its conservative
bound is `18 * 256 * frames + 512` live return addresses — 18 per source
expression level, at most 256 levels per frame — and it must stay below the
initial `0x10000000` stack pointer, whose margin begins where buffered
output ends at `0x0efffffc` (16,777,220 bytes = 2,097,152 addresses).
Solving `18 * 256 * (N+1) + 512 <= 2,097,152` gives `N <= 454` live
contexts: the current profile already spends 9,478,144 of the 16,777,220
bytes, and the last admissible step lands within four bytes of the margin.

The customer's need is larger. Delta grammar admits expression nesting to
`parse_depth` 1,024, and a direct-recursion checker suspends about one live
call per level — the rejected rewrite passed depths 32 through 128 and
failed at 256, consistent with one context per level. Serving that depth
under the same containment constant needs `8 * (18 * 256 * 1,025 + 512)` =
37,789,696 stack-margin bytes, 2.25 times what exists. Equivalently, the
per-level constant would have to be re-audited down from 18 to at most 7,
a tightening that turns every future evaluator edit into a containment
risk. A constant increase alone cannot reach the customer; the item's
suspicion is quantitative, not rhetorical.

## Routes the comparison leaves

- **R0, status quo.** Source-level explicit continuations on the 40M-pair
  heap: `typing_frame`/`typing_visit` and the seven-kind `typing_resume`
  dispatch in `checking/types/expressions.gamma`, with per-kind payload
  encodings spread across the ~18.5-KB `checking/types/` subtree. Already
  built, audited, and passing the complete Epsilon closure; the frame
  payloads are the manual control obligation the item wants to reduce.
  No evaluator cost, no depth coupling.
- **R2, region rebalance.** Move buffered output out of the stack margin:
  the output region is a 16,777,212-**count** bound, so its two
  `0x0e000000` immediates (`buffer_output`, `flush_output`) are the only
  address uses; relocating it to the free `0x0a000000..0x0afffffc` band
  leaves an 83,886,080-byte (10,485,760-address) margin, which admits
  `N <= 2,274` under the existing constant. A 2,048-context cap covers the
  customer's 1,024 need with about 2x slack; context and frame regions
  already hold 43,690 and 21,845 rows physically, and environment/value
  regions stay fail-closed below their preflighted caps. The code change
  is three immediates; the real cost is a rebuilt tape and new identity
  re-pinned in `EVALUATOR_PROFILE.md`, `tools/bootstrap/gamma/evaluator_env.sh`,
  `tests/gamma/heap-boundary/evaluator.tsv`, and
  `bootstrap/3_delta/delta_compiler.composed`, re-derived exact/adjacent
  context and environment controls in `evaluator-development/`, and a new
  subject for the in-flight Beta-encoding proof. It also couples two
  contracts: the checker then relies on `parse_depth <= 1,024 < 2,048`, and
  a deeper admitted Delta depth would again surface as raw evaluator
  status 250 instead of a compiler-owned refusal.
- **R3, explicit eval continuations.** Reify every `eval_expression`
  recursion site into evaluator-owned continuation records: after-`if`
  condition, after-branch close, after-`let` initializer, after-`let` body,
  binary left/right, `pair` left/right, `first`/`second`/`read`/`write`
  operand, call-argument step, and call return — roughly a dozen resumption
  kinds of five to eight words each in a new bounded region, with a new
  exhaustion mapping and malformed-continuation checks. Native depth stops
  scaling with live calls, so contexts become region-bounded (millions, not
  hundreds). This is a rewrite of the ~400-line evaluation core
  (`eval_tail_expression` through `eval_call`) in addressed Beta inside the
  trusted component — the separate validation pass can stay native because
  census already bounds source nesting to 255 lists — plus every identity
  record and gate above: the largest audit-burden increase of the three.

## Finding

The rejected rewrite's failure is structural, not incidental: no context
constant consistent with the current hidden-stack bound reaches the
customer's 1,024 admitted depth. Of the remaining routes, R2 is the only
one that serves the customer without restructuring the trusted evaluator,
and its three-immediate code delta is trivial next to its record, gate, and
proof-subject costs; R3 removes the bound entirely at the highest cost.
Whether either beats retaining the checker's continuation encoding is the
customer-side half of this comparison — it needs the direct-recursion
checker measured against `bootstrap/3_delta`, which is outside this
experiment's fence — but adoption of either must first clear the identity
and contract updates above. Until then the fixtures pin the boundary the
comparison has to argue against: 256 admitted, 257 refused, tail calls
unbounded.

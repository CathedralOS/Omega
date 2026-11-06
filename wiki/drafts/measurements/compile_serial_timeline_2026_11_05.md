# The serial chain of a one-line compile

A one-line `omega --check` compiles three programs one after another: the
bundled `core/build.omg`, the user's `build.omg`, and the product. This record
names every step of that chain, what each step waits for, and which steps
could run at the same time. Two of the steps did not have to wait: typing the
core Build's Product sources waited for the core Build's check although it uses
nothing the check produces, and the calling-policy runs at the end of the
compile ran one after another although each is independent.

Measured on `a2855f01e81` (origin/main, 2026-11-05) plus scopes only, on the
shared Windows host (32 logical CPUs, other workers active; the whole compile
took 11.8 s here against 6.8 s on a quiet run, so read the milliseconds as
proportions). The scopes are the `omega.*` rows this change adds; the timeline
comes from the new `OMEGA_PHASE_PROFILE_TIMELINE` file.

```sh
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null
cd one-line   # main.omg + build.omg from .claude/sr/triv
OMEGA_PHASE_PROFILE=1 OMEGA_PHASE_PROFILE_TIMELINE=timeline.tsv \
  omega --check --timings main.omg < /dev/null
```

## Timeline

Times are milliseconds from process start on the thread that runs them.
"Waits for" lists the outputs the step consumes.

| start | ms | step (scope) | waits for |
| ---: | ---: | --- | --- |
| 0 | 74 | process start, project source resolution, parse of the user's sources | — |
| 74 | 21 | core source capture and parse (`omega.build.core_source`) | — |
| 95 | 203 | core Build: assembly + Psi 02/03 (`omega.source.assembly`, `omega.source.typed_frontier`) | core parse |
| 303 | 2093 | core Build check: Psi 04/05 (`omega.build.evaluation.check`) | core Build typing |
| 2401 | 138 | core Build admission and execution preparation, incl. its Psi 06 (`omega.build.evaluation.execution`) | check |
| 2541 | 66 | core Build run (17 `omega.build.run` slices, 64 ms) | admission |
| 2609 | 856 | **core Product sources: assembly 171 + Psi 02/03 685** (`omega.source.product_assembly`, `omega.source.product_frontier`) | **core parse only** |
| 3465 | 88 | core Product selections and defaults (`omega.build.product_selections`) | core Build return, core Product typing |
| 3568 | 288 | user Build: typing 15 + check 263 (`omega.build.source_invocation`) | core defaults |
| 3856 | 32 | user Build run, package graph | user Build check |
| 3888 | 445 | user Product sources: assembly 91 + Psi 02/03 299 + selections 54 (`omega.product.prepare`) | user Build return (package inputs) |
| 4333 | 153 | Product body completion (`omega.product.frontend`) | Product typing |
| 4486 | 5004 | Product check: Psi 04 843 + Psi 05 4117 (`omega.product.check`) | Product frontend |
| 9491 | 71 | target capture (`omega.product.capture_target`) | Product check |
| 9562 | 2111 | target settlement (`omega.product.select_target`): calling source capture 48, Psi 06 1332, then ~730 ms of optimisation, publication and calling-policy runs | Product check |

The three checks are strictly ordered by data: the user Build is typed against
the defaults the core Build returns, and the Product's sources are fixed only
by the package inputs the user Build returns. Within that order, two steps
waited on the code's shape rather than on data:

- **Core Product typing (856 ms) waited for the core Build check (2093 ms).**
  Completing the core Build always types its Product sources next, with no
  package inputs and the same execution profile. Nothing the check, admission
  or the Build run produces enters that typing; the only shared state is the
  core preparation's source catalogue and retained frontends, which the check
  never touches.
- **The calling policies ran one at a time.** Target settlement evaluates each
  boundary calling policy in its own sponsor and invocation account over the
  same immutable code, about 20 ms each in the samply profile of the same
  commit (`.claude/sr/main-now.profile.json.gz`, 10 runs, 235 ms).

Repeated work inside one compile: Terminal publication encoded the same
semantic module four times: twice to fingerprint it before and after an
optimisation phase that selected no rewriting pass, once for the artifact's
bytes and once more for the manifest's fingerprint of those same bytes.

Everything else on the chain is a check pass owned elsewhere (Psi 04, Psi 05,
Psi 06) or small (selections 54–88 ms, frontend 153 ms, capture 71 ms).
`prepare_filesystem_scope` took 1.6 ms here; the 78 ms in the samply profile
was host I/O, not compiler work.

## What changed

- The core Product sources are typed on a second compile thread while the core
  Build is checked, and offered to the existing Product retention only if no
  source or frontend was admitted to the core preparation after the
  speculation finished. Otherwise, or if it failed, completion types them as
  before.
- Calling policies run on the checking pool and are consumed in request order,
  so the first refusal in that order is still the one reported.
- Calling source capture runs beside Terminal publication.
- The optimisation phase reuses the input fingerprint when no selected pass
  runs a rule, and the artifact manifest fingerprints the bytes already
  encoded.

After the change, the same run shows core Product typing (1070 ms) inside the
core Build check (2369 ms) and the core completion step down from 946 ms to
105 ms; target settlement after Psi 06 fell from about 730 ms to 256 ms.
A/B timings are in the pull request.

# Package-declared reach of a Build-bound foreign requirement

Parent: **BUILD-OWNED-NATIVE-BINDINGS** in [TASKS.md](../../../../TASKS.md),
requirement: transport operational ceilings of Build-owned native rows through
both execution engines.

Fact: a Build-bound foreign requirement whose recorded reach names a
package-declared service runs its selected native import in Terminal and native
with the same exact result.

Psi records three service rows on each emitted boundary declaration: the fixed
reach, the owning trait service and the published ceiling. Here
`CharacterCase::upper` owns `CharacterCase` and reaches the memberless package
service `Lettering`. `Build::bind` backs it with C `toupper` on each hosted
target (msvcrt, glibc 2.2.5 and 2.17, libSystem). The Terminal native sponsor
reads that recorded list. Under the host's explicit
`TerminalNativeServiceDelegation::MatchingHost`, which `omega run` and the corpus
runner supply because the native leg runs the same product on the same host, it
performs a reached service only when that service and its whole parent closure
are declared by the package that declares the import's owning trait. A package
may perform its own declared effects on the host; reach into another package's
or the toolchain's services (std `Console`, `Clock`, `TimeHost`, core filesystem
services), whose supplies Terminal may model, still refuses. Reach grants no
capability ([service reach](../../../../wiki/spec/language/effects.md)); the
delegation is the host's consent, not a reach-derived grant.

The call is carried through [a local](local/main.omg), [a field](field/main.omg)
between canaries, [a parameter](parameter/main.omg),
[a call result](call_result/main.omg), [a match join](match_join/main.omg)
whose two arms return distinct values, [constant names](const_name/main.omg),
[a nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [a Build-selected named implementation](target_body/main.omg). Each place
converts one letter and one digit, so a dropped or misrouted call traps. Each
sibling `invalid/main.omg` removes `Lettering` from the ceiling of the machine
that performs that place's call and must reject because its checked body
reaches an undeclared service.

Before the repair, at `2174c0d10bb`, every positive place exited 0 natively
while Terminal declined with `private native sponsor supplies no additional
service capability`. The same refusal blocked the Terminal leg of the audio
playback consumers (`AudioPlayback`) and `samples/gui/windowed_calculator`.

## Requirement evidence

- Establishment: ordinary checking derives each body's reach from the
  boundary call; the authored ceiling must contain it. The twins remove the
  service and reject.
- Identity and lifetime: no storage is lent; the field place checks both
  neighbors of the stored result.
- Composition: every place transports the call through its construct; the
  match place executes both arms with distinct results.
- Soundness: the delegation admits only package-local service identities
  recorded by Psi, checked through each service's canonical parent closure.
  Another package's service, a toolchain service such as
  `FilesystemContentRead`, a parent outside the package and a missing row all
  refuse (unit tests beside `terminal-runtime`'s `service_reach`).
  A probe binding the same `toupper` with reach to `FilesystemContentRead`
  exits 0 natively while Terminal declines; the corpus cannot pin a declined
  `_exit` row, so the unit tests carry that refusal. Embedders that supply no delegation keep the previous
  refusal.
- Psi 05–07: the boundary declaration retains fixed reach, owning service and
  ceiling; Terminal reads them and executes the import through the sponsor.
- Omega 00–02: each target's Build row selects its own import; locator lengths
  are inferred.
- Omega 03–09: the native image calls the same import and exits 0.

Acceptance is `omega run --both <place>/main.omg` with both exits 0 on each
matching host, plus each twin's rejection. The fixtures are pinned under
`tests/omega/{pass,fail}/facts/foreign_package_service_reach_through_*`.
Linux x86_64 is observed; Windows, macOS ARM64 and Linux ARM64 execution remain
unverified.

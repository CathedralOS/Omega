# Qualification evidence

`routes.json` contains all seven separately attempted routes with exact argument
vectors, working directory, exit code and full diagnostic log name. They used
`OMEGA_LIBRARY_ROOT=<worktree>/source/library`, the cloud setup's XDG directories,
and closed stdin. No route reached the parser. Check exited 1 and run/run --both
exited 200 because root cache custody rejected `/`. This is an environmental
failure, not evidence that either valid source fails or the alias control rejects
as intended. System ownership, permissions and custody guards were not altered.

`rust-build.log` is the complete main-revision build failure (exit 101). The
preinstalled executable's version and SHA256 are retained in `routes.json`; its
source revision is unrecorded. Main-revision source validation remains outstanding.

`mathematical-oracles.json` uses independent 100-digit Decimal square-and-root
calculations, rather than the implemented scale/ssq recurrence. It confirms the
frozen dot=11, large norm=5e200 and small norm=5e-200 mathematical answers and
records the frozen manifest's SHA256. This is mathematical fixture validation,
not an Omega, native, Terminal or ownership check.

`environment.txt` records baseline, resources, failed ownership lookup and scope.
Full source is retained beside this evidence and in the candidate commit. Resume
with the README commands after legitimate cache configuration and shared
interfaces arrive. Descriptor/accuracy integration must precede normal landing.

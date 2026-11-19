# Portable file journal on Windows — 2026-11-02

[Matching-host run 41444385907](https://github.com/CathedralOS/Omega/actions/runs/41444385907)
qualified the unchanged `samples/cli/systems/file_journal` application at
`65e2414d0dce4b9d66cbc4ab436593c32ea4b267` on Windows x86-64.
The optimized CLI SHA-256 is
`3dc61d0498ee3b2db4a390667361d5000bf52ab510382067c82c1d1bf309328f`
(Rust nightly 2026-09-04, opt level 1).

Native, Terminal and wrapper each returned the authored exit 7. The original
program created a directory, wrote 17 bytes, inspected length 17, read and
checked every byte, renamed the file and removed both file and directory.
Neither Source, its exit oracle nor its relative Std dependency was rewritten.
Windows setup explicitly provisioned current-drive `\tmp`, as the application's
README requires. The library retained the authored rooted paths.

The existing ordinary CLI consumer now accepts exact declared normal-exit
product sample paths, preserving their repository-relative directory and input
custody. Source numeric exit comments are reused through the existing oracle
parser. Descendants remain independent projects; nonruntime roles, alternate
entries, malformed/escaping paths and extra oracles refuse. Compiler fact
selection remains restricted to `samples/cli/facts`. There is no new runner or
application coverage inside the shipped CLI.

The first observation stopped before checking: a helper copy-loop variable
shadowed its selected project path and passed a library filename to `update`.
The corrected helper retains a separate project coordinate; an actual command-
path control now checks the copied project and dependency files. All 36 focused
Python controls passed. The failed attempt and raw diagnostics remain retained,
and are not counted as compiler or program failures.

Normal update returned ReviewRequired after 391.922 seconds; 288 individual
pending decisions were accepted, and strict resume completed in 187.563 seconds.
`run --both --keep` completed in 156.469 seconds. All 198 tracked project/library
inputs were guarded before and after phases. Only the root journal application
was executed; its nested projects and completed filesystem witnesses were not
selected. Optimized compiler preparation took 157.781 seconds on the rerun.

Retained native executable SHA-256:
`f68478f894611ffc0cd914eba2c5b66f7f2c4108f22e1407d89452792f438806`.
Retained Terminal artifact SHA-256:
`708a99e63a0c630450a53500263c67a714cfcb549e0d61d5205bbf3a4200f6ed`.
Original profiles, raw reports and CRC-verified products are outside scratch in
`/Users/zcanann/Omega-build/filesystem-windows-journal-20261102/` and
`/Users/zcanann/Omega-build/artifact-read/artifact-13142881469/`.
The exact source bundle/ref is retained at
`refs/corpus-observations/65e2414d0dce4b9d66cbc4ab436593c32ea4b267`.
Note vault, remaining portable filesystem operations and occurrence-bound close
proofs remain open under **FILESYSTEM-PLATFORM-SERVICES**.

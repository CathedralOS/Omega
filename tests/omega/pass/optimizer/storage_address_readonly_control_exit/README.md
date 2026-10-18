# Storage identity and loop motion

This source enables LoopInvariantScalarMotion and declares its rollback.
The hidden-query fixture observes two simultaneously live equal arrays; the
read-only control compares contents. Both currently retain the same module
with and without the rule: explicit reference formation independently refuses
whole-component producer motion. These fixtures support behavior and retained
module coverage; they do not demonstrate a pre-repair runtime miscompile or
prove that the newly added identity guard alone blocks a source candidate.
The guard conservatively reuses transitive effects and is replayed during freeze.

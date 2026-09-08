# Content-claim production

Contract: [content conservation](../../../../wiki/spec/resources/content_custody.md).
[content_conservation.rs](src/content_conservation.rs) lowers normalized
owner projections, entry-claim bindings, identity reshuffles, and authored
partition compositions. The upstream
[source contract normalizer](../../semantics/validation/src/content_conservation.rs)
checks projection calls, entry revisions, separation, and equality; it does not
infer sealed introductions or custody exits.

Keep claim identity separate from structural path and from input/output equality.
Exact theorem/argument substitutions must survive lowering; staged derivations
outside the supported vocabulary reject. Compact report equality does not
permit changing a projection, its algebra, call, or structural subject.

Current bounded producers do not establish general runtime-indexed owned
extraction: the frontier must name the unique moved element. Static field, case,
and fixed-index paths are the available structural vocabulary. Installed-cohort
and root canaries exercise exact occurrence/epoch transactions, not an ambient
mint route or a completed native authority pipeline.

`CONSERVATION-CONTRACT / TERMINAL-CONTENT-CLAIMS` and `BOUNDARY-ISSUANCE` in
[TASKS.md](../../../../TASKS.md) own full source-to-provider/native conservation
and exact invocation supply. Do not convert passing reshuffle/partition helpers
into a completion claim for those consumers.

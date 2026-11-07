# Qualification

The independent oracle freeze preceded all sample execution. The original
19 valid/17 refusal cases retain SHA256 manifest
`0a239750266ee8f063fc038a65d7aa84bf90ea360ba8a70f12bd0f7660bec38d`.
Eight review challenges retain manifest
`3ca6a237b52fd8f73b505c72c75293b6fc40b5229de90fd470ac91f1bde7bb8a`.
The combined roster has 44 unique cases: 25 valid and 19 refused.

The oracle uses exact Fraction risk-set products and Greenwood sums. Log-rank
moments independently enumerate equally likely event subsets at each event
time; they do not reuse the sample's closed-form hypergeometric expression.
Native and public-SDK Terminal captures verify exact risk/event/censor counts,
row counts, median/zero statuses, and 985 numerical fields per route. Each route
has maximum exact decimal-vs-Fraction absolute error 6.4e-11, below 1e-9.
Terminal captures use the exact produced native report's retained Terminal
artifact and runtime provisions, with a fresh isolated private runtime for each
finite input. Native exit comparison alone does not supply those captures.

Original qualification producer: repository compiler source31988b31bfede88f3494cd4c1de20d45afe50996,
pinned nightly2026-09-04, release profile, Linuxx86_64, four build jobs.
CLI SHA256 cfe5102b0c8764051f5dab44887eb94bb0d9020b8892e09a21a76d03907d0349.
Sponsor SHA256 4e1c5120c6113e971aeb6da2472ad6b794181745ccef1a17f06757689525979e;
its full metadata and matching library identities are recorded in its link
receipt. Build14m09s; native compilation about86s; Terminal preparation101s.
Cases execute against one prepared image/artifact per route. One Rust build was
active; the original sample-only producer required no compiler/library edits. Integrated
requalification rebuilds the producer after incoming semantic repairs; its exact
identities and results are recorded in `producer.json`.

Independent read-only review identified three authored defects before acceptance:
rounded half-survival median, 4096-byte EOF boundary, and ambiguous validator
route selection. These were fixed; the median now uses four exact base10^9
limbs bounded by2*32!<10^36. Reviewed randomized risk-set sequences agree with
exact Fraction decisions. Source checking also rejected retiredif syntax,
undeclared Console effects, inadmissible exact casts, and simultaneous mutable
and read-only arguments; the authored source was corrected through the existing
language/library operations. No proof or custody guard was weakened.

The normal package review has 288 explicit rows: three Console permissions,
three dangerous-capability source disclosures, and141callable/141external-supply
rows. The user explicitly approved this exact inventory after automatic approval
review refused it. Source-risk disclosures for unused std APIs grant no selected
filesystem endpoint. Host execution preserves cache ancestry custody; sandbox
UID mapping was diagnosed, and the approved real-ownership route passed unchanged
checks. Runtime ProcessExit remains delegated only to each isolated domain.

Standard sample-gate qualification observes the positive program's empty-input
exit2 and all three intended-invalid diagnostics (negative-1, past-end32, and
unproved dynamic indexing). The latter project is checked only, never accepted
or executed. Generated Python bytecode was removed before final source-only review. Run
validation helpers with `python -B` to keep transient bytecode outside the source
identity. Source and package identity changes require normal fresh review;
adding its discovery build root correctly refused the old lock. The governing program and frozen inputs retain their recorded hashes;
compiler semantics are requalified whenever exercised operations change.

Main initially advanced only with Wire FixedVec recognition, which this fixed
array program does not exercise. A later advance changed selected scalar
intervals, case-payload casts, shared primitive projection custody, recursive Unit
call rosters, constructor float destinations and unsigned proof analysis. It also
changed package review evidence. These operations are used by this sample. Those
changes were requalified with a rebuilt integrated producer, fresh source and
negative-control checks, all 44 native and 44 captured Terminal cases, and the
standard sample gate. The numerical maximum remains 6.4e-11 across 985 fields
per route; all 288 accepted policy meanings remain unchanged.
The further compiler advancement is documented in `COMPILER_REPAIR.md`.
Incoming mutable scalar field, loan and handback repairs were integrated and
requalified with another complete 44 native/44 captured Terminal roster. The
final producer includes the explicit private machine-binder authority gate; ten
new source refusals and eight affected regressions retain their expected outcomes.
The final independent review covers the reconciled repair and qualification;
normal landing retains its exact candidate receipt separately.

Ignored build/w2-04 retains original checking/refusal logs, approval inventory,
build JSON, link receipt, actual native/Terminal stdout/stderr per case,
validation.json records, numerical-summary.json, and sample-gate records.
Recoverable source/evidence archives are retained through the supported Library
workflow, including a final archive after publication. Retained artifacts and
source remain bound by hashes rather than an unversioned preinstalled compiler.

The final numerical producer is caccecb43d7e on integrated main9f28e49398a,
release build8m59s. Fresh44native and44actualTerminal captures again agree
across985fields perroute with maximum error6.4e-11. The subsequent91135
change only rejects mixedf32/f64builtin operands; this sample and its focused
fixtures have nof32operands. Independent applicability review permits reuse
of those numerical captures, with exact integrated-candidate Rust verification.
Source checks are attributed to their recorded producer, not a later binary.

Final current-main qualification rebuilt after the selective-guard checker
repair at8945b5a1e74. Producer8b449822f7e0c31cd8959ee316fe5061e6171861
built in1m37s; fresh44native and44actualTerminal cases pass,985fields
perroute atmaximum6.4e-11. The18source controls/regressions, positive
native/Terminal exits0/0, three invalid table accesses and ten incoming guard
controls retain their expected outcomes. The later0421c3e6f7c advance changes
only two unrelated trait proof fixtures and two corpus records. Its independent
applicability review permits evidence reuse, with exact final-candidate verification.

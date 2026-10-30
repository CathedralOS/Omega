# Domain-selected cleanup

Owning item: **STATIC-MACHINE-MIGRATION**. This fact exercises
[domain-selected automatic cleanup](../../../../wiki/spec/language/domains.md#domain-selected-automatic-cleanup)
and the cleanup obligations in the ownership specification. It does not close
the entire migration item.

The selected cleanup application retains code == 7 and its exclusive output
reference through transport, then writes 7 to the original backing exactly once.
Every standard place copies this sentence. The observer traps unless the original
Counter contains value 7 and invocation count 1 after normal cleanup.

Candidate 33, built from `f2fcf548c9e66e4499ecd5621ede1ad010624262` with
optimization level 1, passes all nine positive places with native exit 0 and
Terminal interpreter exit 0 on Linux x86-64. Each corresponding invalid twin
changes the prerequisite to code 6 and rejects on the missing code-7 obligation.
The focused corpus records twelve positives checked and twenty-nine negatives
rejected with their required diagnostic fragments. All twelve executable pins
agree at exit 0 in both engines: the standard row plus predicate erasure and
literal/computed inherited indices. These rows are registered in the checking
and Linux x86-64 both-engine goldens with the exact candidate applied to recorded
base `290b093a0ecc`. Every positive build binds all four hosted ProgramEntry
targets; execution here qualifies only the actual Linux x86-64 host.

The fixed baseline `453b3c364314c5f3910d2bb0ff4051696f6313e3` runs
`erased_predicates` at exit 0 in both engines on Linux x86-64, so its earlier
main regression is no longer open. The same baseline rejects the new
[`lifetime_application`](lifetime_application/main.omg) at executable cleanup
selection: "cleanup hook has no executable selection" in `consume::entry`.
Removing that refusal exposes package review's separate rejection of
"method-local binders on a nominal cleanup hook". Independent cleanup machines
own their ordinary declaration telescope; lifetime binders carry checked loans
but contribute no runtime generic arity.

Semantic checkpoint `20e8935d3bc7b87ccd414cc362d9bdb64edd8a85` preserves
explicit lifetime binders across all nine places and their corpus pins. The
Linux and Windows x86-64 sample sweeps each observe 27 positives at native and
Terminal exit 0 and 15 discovered intended-invalid controls rejecting for their
authored reasons. Three older lexical probes have no oracle and are unscored.
The lifetime application's [reject twin](lifetime_application/invalid/main.omg)
changes only code 7 to code 6; its cleanup prerequisite still rejects. These
observations qualify these two hosts, not macOS or Linux AArch64.

| Place | Runtime result | Sensitive twin |
| --- | --- | --- |
| local | native 0, interpreter 0 | cleanup requires rejects |
| field | native 0, interpreter 0 | nested code obligation rejects |
| parameter | native 0, interpreter 0 | call prerequisite rejects |
| call_result | native 0, interpreter 0 | returned-value prerequisite rejects |
| match_join | native 0, interpreter 0 | join-producing call prerequisite rejects |
| const_name | native 0, interpreter 0 | named code prerequisite rejects |
| other_state | native 0, interpreter 0 | nested-machine prerequisite rejects |
| other_machine | native 0, interpreter 0 | independent-machine prerequisite rejects |
| target_body | native 0, interpreter 0 | target-body prerequisite rejects |

`const_name` supplies the code prerequisite from `READY_CODE`; it does not
claim compile-time construction of an owner borrowing mutable runtime storage.
`other_state` retains the historical folder name but declares a genuine nested
machine. Its declaration is resolved lexically before flattening; it is not
translated into a source `state`.

The baseline `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, freshly built in this
session, rejected `dropped by` at parsing. The repair retains an ordinary static
machine application and its exact owned subject through typing, normalization,
cleanup selection and executable specialization. Whole-subject inherited and
conjoined identical applications coalesce; fields remain separate subjects.
Different applications reject. Borrowing does not grant disposal authority.
Cleanup prerequisites are proved at the actual disposal edge, and explicit
qualification casts cannot erase or replace a retained application.

Additional productions exercise named domains, transparent aliases, repeated
and conjoined selections, generic and const machine applications, generic
containers, and deeper/sibling/terminal nested calls. Controls cover conflicting
applications, direct hook invocation, unauthorized selection, wrong carriers,
crashing, exiting, fallible, blocking and suspending hooks, and cast erasure.
The nine standard places and these controls have source pins under
`tests/omega/{pass,fail}/facts/domain_selected_cleanup_*` and registered focused
corpus outcomes.

Additional runtime evidence passes direct indexed, indexed predicate and computed
index productions, nested helpers beneath distinct result overloads, and explicit
predicate erasure retaining a default cleanup application. Candidate 33 also
passes inherited literal and computed (`3 + 4`) index productions in both
engines, with their code-6 twins rejecting on `Label` membership. It checks the
original const recipe before substituting a canonical value into the private
applied theory. The overflow control rejects the exact out-of-carrier value
`u64(18446744073709551616)`; normalization does not erase that obligation.
Historical affected-library Clippy passed with warnings. Enclosing generic telescopes,
general erased-interface transport, product-producer parity, source retirement,
and the whole item's required applications/corpus/host results remain open.
No TASKS acceptance has been subtracted. The full migration item remains open.

Immutable producers, source manifests, exact commands and raw logs live in this
worktree's ignored `build/slot3-evidence/`. Durable private source checkpoints
preserve their own recorded revisions; historical results are not relabeled as
results from a later producer.

The lifetime repair exercises these requirement layers without closing the
parent migration's broader acceptance:

| Requirement | Evidence and boundary |
| --- | --- |
| Establishment | Construction supplies code 7; code-6 twins fail the cleanup prerequisite. |
| Identity and lifetime | The explicit `'storage` binds the original exclusive Counter backing; cleanup writes that backing once. |
| Composition | The same fact and operation survive all nine linked places and pinned pass/fail fixtures. |
| Soundness boundaries | Exact cleanup owner custody, receiver type/access, prerequisites, and finite normal completion remain checked; conflict, authority and invalid-hook controls remain. |
| Psi 05–07 | Ordinary cleanup actions retain the selected machine and captured receiver through Terminal production and interpretation. |
| Omega 00–02 | Package review retains independent callable binders; ordinary provider, layout and calling-plan production consumes the result. |
| Omega 03–06 | Native execution observes the cleanup store and invocation count through ordinary instruction, register and frame realization; this is not a stack-boundary stress test. |
| Omega 07–09 and emission | Emitted PE and ELF programs execute and agree with Terminal Psi on their matching x86-64 hosts; other hosted execution remains open. |

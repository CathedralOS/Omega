# Domain-selected cleanup

Owning item: **STATIC-MACHINE-MIGRATION**. This fact exercises
[domain-selected automatic cleanup](../../../../wiki/spec/language/domains.md#domain-selected-automatic-cleanup)
and the cleanup obligations in the ownership specification. It does not close
the entire migration item.

The selected cleanup application retains code == 7 and its exclusive output
reference through transport, then writes 7 to the original backing exactly once.
Every standard place copies this sentence. The observer traps unless the original
Counter contains value 7 and invocation count 1 after normal cleanup.

Candidate 22, built from `f0a5b7ed756cf2570e7f7e15a4e260821d0242ae` with
optimization level 1, passes all nine positive places with native exit 0 and
Terminal interpreter exit 0 on Linux x86-64. Each corresponding invalid twin
changes the prerequisite to code 6 and rejects on the missing code-7 obligation.
The focused record contains 28 matching command expectations: producer profile,
nine project updates, nine both-engine runs, and nine rejects. Every positive
build binds all four hosted ProgramEntry targets; execution here qualifies only
the actual Linux x86-64 host.

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
`tests/omega/{pass,fail}/facts/domain_selected_cleanup_*`; focused corpus
qualification and golden registration remain pending.

Integrated candidate 28 passes the direct indexed, indexed predicate and computed
index productions in both engines. It also passes nested helpers beneath distinct
result overloads and explicit predicate erasure that retains a default cleanup
application. All sixteen conflict, authority, effect, erasure and scope controls
reject on their intended obligations. The inherited indexed production still
fails because its retained const-expression argument has no applied theory view;
that repair is in progress. Enclosing generic/lifetime telescopes,
general erased-interface transport, product-producer parity, source retirement,
and the whole item's required applications/corpus/host results remain open.
No TASKS acceptance has been subtracted. Reviewed source is published in draft
PR [165](https://github.com/CathedralOS/Omega/pull/165); nothing has landed.

Immutable producers, source manifests, exact commands and raw logs live in this
worktree's ignored `build/slot3-evidence/`. Durable private source checkpoints
preserve their own recorded revisions; historical results are not relabeled as
results from a later producer.

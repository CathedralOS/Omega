# NEW022 validation

The complete application passes Linux x86-64 qualification: source check, exact
intended-invalid bounds rejection, fifteen native input fixtures and fifteen
native/Terminal comparisons. Ordered contacts, every final disk component,
momentum and energy ledgers, statuses and exits match the independently frozen
oracle. Cases 0–3, 5–6 return 0; budget case 4 returns 3; invalid models 7–9 and
five malformed byte inputs return 2. Terminal consumes the same admitted artifact
and passes the Omega scientific assertions; the CLI compares exits and forwards
native stdout. It does not claim a full application Terminal stdout comparison.
The separate formatter corpus control compares stdout in both engines.

Worktree `/workspace/Omega/.codex/worktrees/new022`, branch `sample/new022`.
Historical qualified base: `2a2cbe75bf25d0f5f1b9c269dc46c5b888426167`.
Historical 2a execution checkpoint: `89f4747dceb50e856644b4a14891584508572927`.
The compiler, runner and native hashes immediately below describe only that
checkpoint. `evidence/qualified` is its retained historical evidence.

The subsequently qualified review candidate `044ae17932d330f6b12680bd062d52df255de2cc`
was integrated on main `592fcca655158cbccbd93b30a68067eb3bcfcccf`. Its compiler SHA256
is `fb668860b42cda8ae30a75409956c13be16435d794bb027f34c4deea0b75a325`,
runner `8e0315e96e3362ba51ca0abacef269eb5973298ae23fbf425793b9a27428774b`,
and native `c2016f919e77ac45c625864c84ac9cada8f971baf7c24bb6aeab29e1fe713197`.
Its pinned Library packet is `libfile_bb67c9784694819195fe7443da97f543`,
version 0, outer ZIP SHA256
`5e1a626dcea93439d4cccc4c7cf951d5910d59f310d3f553c9e95a6350434369`.

The continuation first reconciled main `4874e97f0f3aa3a174b204963a980d02aa6571b8`,
then qualified on main `6c890b3be32e839ed5e73c75ba756ef05e1e804b`.
Substantive later main changes require a fresh final qualification on
`4b5eb415f13ec0ecc49c9a8f39dd990f4063553e`; the passing 6c checkpoint is preserved.
Fresh execution identities, commands, final candidate receipt and publication
record belong to its separate final Library packet; the historical packets and
frozen expectations are preserved.
Original stock main 8fef9837644713285aa9ed9eee42b46cd3bd1da8 and stock reproduction
6121c00e4546376e663d396e443a4f20ee4d77f4 are retained in historical evidence.
`evidence/qualified/execution-source.json` binds the actual semantic and compiler
source hashes; later report edits do not change those inputs.

Historical 2a compiler SHA256: `ea8718adcede1e5f99b4ade0d7a016df0049332c9378eba05df2b4524f6db76e`.
Matching corpus runner SHA256:
`232ca503fb3ee816e7252196f8ad77368a9a36f18b0eb7fb52a5c2f0fa8afe29`.
Native program SHA256:
`dd8f3a73631fb8d7b912bea13eabf1abc11c3ef17609c8fa7313417627915c7f`.
Local `/tmp` compiler paths in recorded commands identify the verified execution
copy, not an artifact transfer. The review archive provides bundle/patch identity,
source hashes, measured commands, output artifacts and the native program.

## Frozen reference

`data/FROZEN.sha256` retains the original independent rational calculations,
frozen before Omega execution. Expected JSON SHA256 is
`374e13d2fcdb1b209bd33b1f0d26052b69300755c76f9a670422cfc5a19cd655`;
oracle Markdown SHA256 is
`dca44ca6a118aa7a5ce292ffea6071af2ccaee24e4c1c09ac782867bb6dc703f`.
Absolute tolerances remain 1e-9 internally and 1e-6 for six-decimal output.
`validate.py` consumes those values; it never generates them by replaying the
simulation. `data/expectations.omg` independently guards the computed results
before success in Omega. The delivered application is the complete Omega entry,
physics and reporting modules, not a reference script or constant witness.

## Actual compiler regressions

Stock main 6121, compiler SHA256
`0ba0652e3b2a0ad2c2634a8d35887f3f817fdd101d706d80a733c5ef468d89e8`,
rejects the event guard as not executable. Checked float builtin selection was
lost across call inventory and execution. The repair carries the existing exact
F32/F64 requirement from the exact source origin; authored names, provider/policy
selection and receiver authority remain separate checks.

That repair exposed repeated call occurrences in normalized scalar operands.
The producer retains scalar computations by source handle and contextual carrier
within a statement. Ordered expansion carries previously completed operands;
call accounting shares completed DAG visits across one invocation's roots.
Distinct authored handles remain separate and branch scope remains local.
Runtime-fed authored-name, ordered NaN/signed-zero, mutation-count and lazy-branch
controls challenge those assumptions with pre-execution promises.

At the historical 2a checkpoint with the float repair, both the full application and complete
standalone formatter reject during allocation for `UnresolvedEntryTransitions`,
register 25, count 2 (functions 16 and 3). Earlier stock main rejects the same
formatter for a fixed-view common-domain conflict. The historical bounded Omega selection
repair copies pinned inputs for Exact, Wrapping and Saturating division using the
existing selected constraint rows. The source value can survive calls in its own
home; short-lived copies serve instruction pins. Source value and definition,
proof and fuel remain attached to the same division. Free inputs are unchanged.
No allocation, borrow, bounds, custody or publication guard is weakened. The
unsuccessful optional spill-policy experiment and all temporary tracing were
removed. Published main's general allocator recovery is retained unchanged.

The intended-invalid source is a complete two-entry array indexed at 2. It rejects
with exactly one diagnostic: `cannot prove index` followed by index 2 and length 2.
No package, parser or missing-entry failure is counted as a pass. Historical
`evidence/check-final.stderr` contains earlier authoring borrow errors, corrected
by snapshotting delay/remaining before mutating Lab; it is not the final frontier.

## Commands and resources

```sh
source /workspace/cloud-setup/activate.sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
export CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true
export CARGO_BUILD_FINGERPRINT=content
mbx build --release -p omega
OMEGA="$PWD/target/release/omega"
"$OMEGA" update --project samples/cli/independent/disk_collision_lab --target linux_x86_64
# Inspect exact pending decisions; update --resume only after ordinary review.
python3 -B samples/cli/independent/disk_collision_lab/run_validation.py "$OMEGA"
```

The extended serial harness records all 33 commands and exits plus every stdout/stderr.
The invalid control is copied unchanged under excluded root build/validation
before check. Python bytecode writes are disabled, preventing generated child
files from changing the accepted source snapshot. The harness never authorizes
package review. The retained review accepted 286 exact existing declarations:
140 Std callable rows, 140 Std external supply rows, three dangerous-capability
rows, and three root Console terminal permissions. Root reaches only Console;
unchanged Std filesystem declarations do not attach filesystem authority to it.
There was no package acceptance denial and no retry of denied acceptance.

Linux x86-64, UID 1000, rustc nightly-2026-09-04 1.100.0-nightly (a69a63265),
approximately 17 GiB RAM, four Rust build jobs; one Rust build and one Omega
execution at a time. Terminal allowance is 10,000,000 fuel units. Target cache:
`/workspace/cloud-setup/cache/mbx/targets/v1/d27e61c8ff8d442f87f1d630890d9e12fd8dcbcfe4c2602c944659a897dee809`.
The default sandbox exposes synthetic root UID 65534 and fails cache custody.
Approved per-command require_escalated execution sees real root UID 0. No
ownership, permissions or custody guards were changed. Formatting and scoped
Clippy pass; final corpus records are retained separately.

Other declared hosts are not executed on this Linux executor. Claims lookup
was unavailable after a read-only timeout, so registered ownership is not asserted.
Normal main-only landing and Library materialization are recorded in the review
handoff. The independent review agent materialized the pinned original Library packet,
verified its payload and source hashes, and reviewed the application and general
compiler repair. The continuation receives a separate integration review.
Execution on other hosts is not claimed.

The historical qualified scoped corpus gate passed ten native/Terminal controls, including
formatter stdout, float selection, IEEE ordering, lazy evaluation and signed
division overflow. The separate borrow rejection gate recorded its intended
borrow diagnostic. Six pre-existing lost-success rows in the wider check golden
remain baseline observations; this is not a claim of a full corpus baseline.
Exact gate observations and logs are in `evidence/qualified/corpus-final-*`.

Final main reconciliation retains published main's division-input implementation
unchanged; the final candidate only adds the Psi float identity and shared
evaluation repair. Qualification on that integrated compiler is retained in
the outer Library review archive, separate from the historical 2a checkpoint
in `evidence/qualified`. Five malformed byte fixtures also run with Terminal.

## Continued main integration

The complete 33-command replay at source `59b6b5cac2aee21bb642e35890a96cd60a78a9d3`
on published base `4b5eb415f13ec0ecc49c9a8f39dd990f4063553e` passed in 919.453 seconds.
Its compiler SHA-256 is `7c0128cceb7057679dcada3d3c569b8399f65dbb8684f8cb81b3aea6ee10d22c`.
A fresh 19-command integration at `5906e5ec7c81720d16b433cc73d27cb5ede5639e`
on base `a6a0645ad86e108225dcb3ddd5206a4bc34cc737` passed check, exact bounds
rejection, compilation, all 15 native fixtures and the oblique comparison.
Eleven native/Terminal controls and the separate borrow rejection passed there.

The descriptor and package-discovery integration at
`6eefe97847df60ce6d48ba93bfa97a7b4ba2920a` on base
`8fe08159979f8e3b16108a819d7e2b8de72c8fbe` passed another 21 commands: check,
exact bounds rejection, compile, all 15 native fixtures, and oblique, exhausted
budget and nondigit native/Terminal comparisons. Its compiler SHA-256 is
`f736c8b896b9aab461c4fe0223b5e6c78c3b21283d42dbf9ca27cfbace77a41c`;
its matching runner SHA-256 is
`a626eebb4e5fb5de62fa3837c716206390aa525a94d23c1e8922e706579ef544`.
Both refreshed native executables are byte-identical to the retained qualified
ELF (`c2016f919e77ac45c625864c84ac9cada8f971baf7c24bb6aeab29e1fe713197`),
and every refreshed output matches its earlier captured output exactly.

Independent read-only GPT-6.1 Sol Medium review verified the source and output
hashes, rational oracles, 19 semantic repair hunks, integration boundaries,
intended rejection and all unrelated golden rows. The application does not use
main's new owned-local float store admission path; its mutable scientific float
stores target parameters. Console reporting does use byte views, so descriptor
integration receives fresh output and Terminal witnesses. The full earlier replay
is reused only within those audited boundaries; it is not relabeled as execution
by a newer binary. Frozen expectations and tolerances remain unchanged.

The final Library packet retains each executed checkpoint bundle, actual compiler
and runner identities, source manifests, commands and captured outputs, independent
review, exact-candidate receipt and normal queued publication evidence. Later
documentation, measured golden rows, metadata-only identity changes and unrelated
main changes are recorded separately from the actual compiler build checkpoint.

The 8fe integration also passed five affected native/Terminal controls, including
inline slice recasting and formatter output, plus the separate intended borrow
rejection. Unrelated golden records were preserved byte-for-byte.

The provider-roster and diagnostic-attribution integration at source
`ceb366a805e3601cc6be410f225d9671fcfa4d21` on published base
`d4fb791abfca269cf8acc5d96d6779cbfdc7b7b8` passed 19 fresh commands: check,
exact frozen bounds rejection, compile, all 15 native fixtures and the oblique
native/Terminal comparison. Compiler SHA-256:
`08a622c3496dce4e0245be570690c3a1755ee06dd62899385ca86e6154f72431`;
matching runner SHA-256:
`269230dedb5306b06d2e4bbc96b50ec91ac61fb1114a8cc9d24fc258c42633af`.
All 16 captured runtime outputs and the native ELF remain byte-identical to 8fe.
Three affected native/Terminal controls and the separate borrow rejection passed;
all unrelated golden records were preserved. Concrete ConsoleBinding uses no
inherited generic receiver roster, as independently reviewed.

An initial private integration wrapper predicted a third diagnostic location line
and failed its format assertion after successful check and intended rejection.
The bounds producer has no source span; incoming attribution leaves the original
frozen two-line diagnostic unchanged. The original matcher was restored, and the
complete aborted attempt and successful replay are retained in the outer packet.
No application, compiler, oracle or tolerance was changed for that correction.

The requirement-discovery/provider-reader refresh at source
`0b56de12720a854a48885975d2d9ed7b916ca901` on published base
`8336093c996532367fa08aa72774cba531e28a84` passed five fresh commands: check,
exact bounds rejection, compile, oblique native and native/Terminal comparison.
Compiler SHA-256:
`11be35ca1ba07896434f7b284f19254d465c149d62742ec7eaa730848d9411aa`;
matching runner SHA-256:
`be87a0472a8b6b32c21a9ae4c690ef8ae7c7e4a1f5a1ac11da2f830770d69eb1`.
The native ELF and both oblique outputs match the retained d4 qualification exactly.
Four affected native/Terminal controls passed, including incoming structural
initializer requirement discovery, followed by the intended borrow rejection.
Prior full replay and complete native fixtures remain separately identified;
this bounded refresh does not relabel them as new executions. Unrelated golden
records were preserved. A verified public no-reply identity replaced unpublished
commit metadata only after an actual GH007 queue rejection; trees, messages and
metadata-rewrite timestamps remained unchanged, with the original refs preserved.

The fixed-array/result-custody/debug refresh at source
`49f020d3e5b8830551506bb86c0f82a640e4edc1` on published base
`4777c4138956b38cef8a80d3aa8064e27c7d4e68` passed five fresh commands:
check, exact bounds rejection, compile, oblique native and native/Terminal comparison.
Compiler SHA-256:
`132db6a6cae15557d71cee33cf2c72c8a7a7151369dd55cac5450eaae9d9cdba`.
The native ELF and both oblique outputs remain byte-identical to the prior replay.
Five positive native/Terminal controls passed, including fixed-array slice state
arguments, computed enum effects and complete-module debug inspections. Separate
borrow and immutable-array negatives reject for their intended reasons. New
numerics files on published main are not imported by this application.

An inherited CaseMembership Rust layout failed scoped formatting. One pattern/call
block was formatted without changing tokens; before/after hashes and the failed
check are retained. Formatting and Clippy then passed. The compiler and runner
remain identified at their actual pre-format build checkpoint, with the formatting
delta and final receipt recorded separately. A read-only volume inventory found
ample workspace disk/inode headroom and a distinct memory-backed /tmp filesystem;
no build, ownership, mounts or unrelated files were changed for that inventory.

The provider-identity/integer-fixed-use refresh at actual source
`8d1e588c07dacec4b5ec3688958852ef57ff7736` on published base
`bc230f034c3835183188528b72b30dc23f144306` passed five fresh commands: check, exact frozen bounds
rejection, compile, oblique native and native/Terminal comparison.
Compiler SHA-256:
`5ef363db3a7e4bb85a7aee01765c6b95ad01756a27b67c20bf6e3fa633686b3c`;
matching runner SHA-256:
`39103a0e399af467401a0dabd60c19abaf99582c3c55b0527be0d7cc66adeba9`.
The native ELF and both ledger captures are byte-identical to the prior 477
qualification. Six affected native/Terminal controls passed with exact stdout in
both engines: formatter, hosted clobbers, cyclic fixed uses, Boolean and floating
call-result recovery, and nested-record requirements. Separate trapping-coverage
and fixed-input definedness controls reject for their intended reasons, with zero
wrong-reason, oracle or infrastructure failures. Unrelated golden rows and ordered
diagnostics remain byte-identical to the published base.

Independent review finds no Atomic expression in this application or the earlier
borrow fixture; non-Atomic executable traversal edges are unchanged. Earlier
structural/debug, borrow/immutable and complete application qualification remain
explicitly identified at their original producers. This is a bounded affected-path
refresh, not a new full replay. The package update succeeded with zero pending
decisions. Its private evidence-copy wrapper initially used an incorrect emitted
file path; the successful update and corrected review inspection are retained,
and neither update nor package acceptance was retried for that correction.

The collection-view custody/IEEE-arrival refresh at actual source
`f7aaa83016dae88d8b680ec40a0d929d082ab89b` on published base
`12ba12dd88f46048c6baa573e89ddaa01346305d` passed five fresh commands: check, exact bounds rejection,
compile, oblique native and native/Terminal comparison. Compiler SHA-256:
`ea01d815a7f214e45015b88634d4761f291da4809921f5d97e3a77eaa5ee4e98`;
matching runner SHA-256:
`ea868a7e796407dfa6dfb7f64f61c40750b8b7f4b9a3bac291ac990df10522f7`.
The native ELF and both ledger captures remain byte-identical to bc23. Three
native/Terminal controls passed: exact formatter output, shared fixed-byte state
loans, and IEEE state-arrival predicates. Separate byte-extent and unsatisfied
arrival controls reject for their intended reasons, without wrong-reason, oracle
or infrastructure failures. Unrelated golden rows and diagnostics were preserved.

The latest main removes one unused private syscall digest helper; independent
review confirms no caller or digest behavior changes. Prior qualification remains
identified at its actual binary producers. A closed-stdin expected-exit/output
header was added for current sample-gate policy. This comment-only delta preserves
the complete executable body; its proof and a fresh source check, package review
and native EOF witness are retained separately from the pre-header native build.
The completed unpublished outcome is consolidated for main's per-commit build
policy, while every original and executed checkpoint remains archived.

The scalar/resultless ABI refresh at actual source
`ed8ca78ac1ebbec3e3cfdcb346d774e1abee5ee2` on published base
`f729e578ef001a16351575983b8f490171e8073b` passed five fresh commands: check, exact bounds rejection,
compile, oblique native and native/Terminal comparison. Compiler SHA-256:
`b1c8f96f4a980d25502ecbaecc87e96fa26ba5d838810c149796c09488d46e65`;
matching runner SHA-256:
`6fdb8d58951568d6d5d3b2fceeae3b70bf9a62b167ef367bd1c487c876a446e1`.
This compiler builds the current header-containing source. The native ELF and
both ledger captures remain byte-identical to the earlier qualification. Formatter
and resultless scalar-parameter controls passed in both engines, with exact
stdout/stderr expectations; the separate return-contract control rejected for its
intended reason. Wrong-reason, oracle and infrastructure failure counts are zero.
Independent review retains earlier structural, IEEE, fixed-use and complete
application evidence at its actual producers, and unrelated golden records remain
byte-identical to published main.

The finite-family/provider refresh at actual source `fbcaaf7bfec5b8c06f35b2ab09a00a65c665092b` on
published base `5f0e10adfb515b43bbeb9e8be29e80f0cc97fd24` passed five application commands, with exact
bounds rejection and unchanged native ELF/oblique ledger captures. Compiler
SHA-256 `37241ae18922daaadc6c40c1cc906f6f9ed90880664c0e2b2c62c156d7b33703`; matching runner
`3b60195250d84f34e7a1eef404bae49bee372dac0e0fb82d65c746a170cb0240`. Four native/Terminal controls passed: float guard,
authored max, shared operand evaluation and exact console formatter. The inherited
provider roster retained one Terminal artifact for macOS exit0 and Windows Trap;
its declared targets have no Linux native entry. Const-roster, crossed-tuple and
borrow negatives reject for their intended reasons. Wrong-reason, oracle and
infrastructure failures are zero. Foreign golden rows retain main's bytes.

The calling-plan closure refresh at actual source `1c1b6d2c5afe34c654a8f1c2265d21e0b709efc2` on
published base `3970e023b994c27ba62cf93dfb842b1c684bd750` passed five application commands with exact bounds
rejection. Compiler SHA-256 `36aaca07fdac9b9861f93dc4dc4f202d225b11257ac5a319600a5760e68b1b43`; runner
`df4dc258b8f621821417040b7cde71ced753f7114eb86416cbb1408cd58b0277`. The exact formatter passed native/Terminal
comparison, and two private-callback layout negatives rejected for their intended
reasons. Wrong-reason, oracle and infrastructure failures are zero. The standard
Application entry-policy path was freshly rebuilt; no authored callback or Linux
pthread fixture execution is claimed. The native ELF and oblique ledger outputs
match prior qualification byte for byte. Earlier scientific replay and selected
float/provider/borrow evidence retain their actual producer identities.

Independent review of the subsequent published range through
`70e586e35f92cf722d87756dd55b5a74209ab99b` supports reuse: its sole Rust branch
admits zero-byte owned structural arrivals, which this application's positive-size
Disk/Lab, borrowed bindings and scalar operands do not exercise. Incoming probes
and golden records are preserved; the exact final receipt covers the integrated
source without relabeling the earlier compiler producer.

The c715 indexed-store/descriptor integration at actual source
`ccf0156931e7d5d0ad48c15e0ef5edf73a12a205` passed five fresh application commands.
Its native ELF changed to SHA-256
`74db06c5f98be730e01e80b358b8a3ab7334c41b03f9a2fa2fbabe150f9544cf`; all 15
native fixtures were therefore freshly validated, with every stdout matching the
prior full replay. Six frozen corpus controls were deferred and consolidated with
the next shared-result integration; no c715 corpus execution is claimed.

The completed shared-result qualification at actual source `744cc46631b7aed5f6ae1c73fedc9ba098dfdc3f`
on published base `916d3ac1d2963d9e656cbb99d1b5de8e7beae1d9` uses compiler
`3647bea68ee79e06d2b3128d9d0732b8ef8858a4323fef6cc07d10b2120a716c` and matching runner
`0ee5c43738ab6f0da8154ace8da9360bc08f41c0f672517db43ea19c81669c1c`. Five application commands passed (165.244 seconds),
followed by all 15 native fixtures and four additional native/Terminal comparisons
for grazing, equal-time collisions, exhausted budget and invalid nondigit input.
Eight consolidated native/Terminal controls passed: formatter, shared float
evaluation, indexed array-field and local-view stores, incoming bounded-byte
calls, narrow signed case payloads, nested dynamic owned returns and linear result
claims. Two ownership negatives reject for their intended reasons. Wrong-reason,
oracle, infrastructure and lost-success counts are zero. The release build took
5m59s; the two corpus commands took 241.044 and 1.371 seconds. Actual commands,
per-command durations, outputs and stage transitions are retained.

This qualification remained pinned until completion before observing subsequent
main. Earlier full33/finite-family/borrow evidence retains its actual producer;
changed-ELF behavior is established by fresh fixtures rather than an identity claim.
The current ELF equals c715's 74db artifact, and all 19 additional stdout captures
match the original full replay. Foreign golden rows are preserved verbatim.

The completed coalesced IEEE/provider qualification at actual source
`fd6cd6367cac6758b2d6a323661dd3ca07928215` on published base `93483ae50e172fc0096c9c2f5c16732b67c09da4` uses compiler
`afca2c53a55e6f06c99b7e04796e3fd450042643b9b5d6493eb04aaf05655f45` and matching runner
`b7f7f5593cde68cbd9f3db1b041e311cc2d4c81d493ad5753f1d2805ea544a0a`. Five application commands passed in
172.181 seconds; all 15 native fixtures passed with stdout
byte-identical to the prior full replay. The ELF remains 74db, so representative
Terminal branches retain reviewed 916 evidence while oblique both6 runs freshly.
Four native/Terminal controls passed: exact formatter, builtin IEEE order, runtime
float guard and all six NaN comparison relations. Error/lost-success counts are
zero. The release build took 6m49s; the corpus run took 241.958
seconds. Stage transitions were recorded, and main was observed only after this
pinned qualification completed. The same warm cache/profile and reusable working
binary paths are retained; independent review verified actual bytes before reuse.

The completed generalized-assignment/integer-carrier qualification at actual source
`c0680c720bca657ce96cf69d7855c629fec4c205` on published base `0e7af1e5722c8ff56db5a5c1824610ce99c444b2` uses compiler
`088217d1579ab3a5cc4cb0b0e6650bc5f69b94c33cd0685d144af1902f475e08` and matching runner
`9ae253c874740613c07f46619559f6cc911df983784f9122de33087bd8167d89`. Five application commands passed in
172.656 seconds, followed by all 15 native fixtures. The native ELF
changed to `3dfbfba1cc06a25d5d15d554e7bf22ee33a3fd050f02f378c9c4cb893f5072ba`, so grazing contact, equal-time ordering,
budget exhaustion and invalid nondigit input were freshly checked in both engines.
All 19 additional stdout captures equal the earlier full replay. Six focused
native/Terminal controls passed: exact formatter, shared float operand identity,
hydrostatic formatting, returned-record computed fields, fixed division and unsigned
multiplication. Shared float array writes and an unguarded scalar-state contract
rejected for their intended reasons. Wrong-reason, lost-success, oracle and
infrastructure counts are zero. The release build took 3m14s; the two corpus
commands took 251.327 and 15.052 seconds.
The same warm target/profile and working binary paths were retained. All 6,380
compiler inputs and 29 control artifacts were frozen before execution and verified
after building. Pinned qualification completed before observing further main;
actual producer revisions, outputs and stage durations remain distinct.

The completed computed-backing/current-ownership qualification at actual source
`8d023c25305c188c7198ff588a86f029adfeb90b` on published base `0b802d0a87174780f2908bee44da19ce721525a2` uses compiler
`21a0ddf3316c9ef3957064763b00664c499ab3ffd3803084d6285ab4ebe27eb5` and matching runner
`027de3645bbfeac5b444c39e8049244a934dcc6c01efd97cdcc070f4869c4b74`. Five application commands passed in
177.782 seconds; all 15 native fixtures passed with original full-replay stdout.
The native artifact is `3dfbfba1cc06a25d5d15d554e7bf22ee33a3fd050f02f378c9c4cb893f5072ba`. Additional Terminal coverage or
identity-based reuse is explicitly recorded in the qualification directory;
0 additional Terminal branches ran freshly. Six native/Terminal
controls passed: formatter, shared float operand, computed backing reference copies,
restored owned parameters, canonical builtin guard and authored max selection.
Five checked negatives preserve nested loans, temporary backing lifetime, selected
reference lifetime, strict-child restoration and builtin operand borrow authority.
Wrong-reason, lost-success, oracle and infrastructure counts are zero. The release
build took 6m 22s; the two corpus commands took 241.038 and
27.879 seconds. All 6,379 compiler inputs and 40 control
artifacts were frozen before execution and verified after building. Independent
review verified the merge's complete PreparedExpansion storage threading and
ordered scalar snapshots. Earlier unaffected results retain their actual producer.
Qualification remained pinned until completion, using the same warm profile/target
and working compiler/runner paths, before observing further main.

The e259 integration failed before application execution: equality occurrence
1850 selected both BuiltinOperator and Float::equal. The unmodified stock-e259
compiler reproduced the same diagnostic with the complete application. Private
traces established that specialized copies of the resultless
CommutativeSemiring.add_identity law lacked their checked contract roots and
reconstructed an incompatible runtime meaning from partially unknown operands.
The original selection consistency guard remained intact. No package acceptance
command was executed. Complete source, producer hashes, diagnostics and traces
are preserved separately from successful evidence.

A temporary private provenance repair at actual source
`ee27a6212b2460931d06f9806fc9dcdbc035f883` on base
`3b5282df452f7a15b37daafddcdd1e3213d2ea91` passed ordinary package update, five
application commands and all 15 native inputs. Its broader six-control gate
failed: three current controls built, while three older law inputs rejected for
unresolved termination/constant-materialization obligations. That gate is not
reported as successful. Supplemental complete law probes established the intended
unproved-law rejection; the positive probe stopped at ordinary pending package
review, without acceptance or runtime execution. Their complete sources and
proposal remain diagnostic evidence outside the shipped application.

Published main repair `783a1cde856bf35b023f8c1f0a34f8ee0497098e` retains rooted
operator meaning in detached copies while independently resolving rooted copies
and preserving the disagreement guard. Inspection of the actual occurrence and
owner evidence established that it subsumes the temporary repair. All three
private provenance changes were removed before integrating published main
`12953abbc60fd1ac37ddc79b3d9aa0a804ba522a`. The application retains its original
bounded float evaluation and ordered argument fixes; it consumes no worker branch
or unfinished numerical library.

Completed integrated qualification uses actual source `4f41664b0e41c0659491e4c7d58c7410935e216d`,
base `12953abbc60fd1ac37ddc79b3d9aa0a804ba522a`, compiler
`2e4bf24be58c8561c716b52d22d6cf250bff32b9eed4ac98cc87b5eb8c769c88` and matching runner
`4c893d7c5658c27beff9f40530c7b0d48a40000b7b64a705a389ab6124072ed7`. Ordinary package update passed with zero pending
decisions and zero acceptance commands. Five application commands passed in
168.472 seconds, including exact frozen bounds rejection,
production, native oblique contact and Native/Terminal comparison. All 15 native
inputs passed and their stdout equals the earlier full replay. The native ELF
`3dfbfba1cc06a25d5d15d554e7bf22ee33a3fd050f02f378c9c4cb893f5072ba` is byte-identical to the prior qualified artifact.
Additional Terminal branch evidence retains actual producer
`c0680c720bca657ce96cf69d7855c629fec4c205`, with intermediate identity reviewed at
`8d023c25305c188c7198ff588a86f029adfeb90b`; oblique Native/Terminal execution ran
freshly against the current compiler.

Six focused Native/Terminal controls passed: fixed-point formatter, shared float
operand evaluation, computed backing reference copies, restored owned parameters,
canonical builtin guard and authored max selection. Five checked negatives
preserve nested loans, temporary backing lifetime, selected reference lifetime,
strict-child restoration and builtin operand borrow authority. Wrong-reason,
lost-success, oracle and infrastructure counts are zero. These are distinct from
the failed broader historical law gate. The release build took
313.003 seconds; the two corpus commands took
251.437 and 29.589 seconds. All 6,385
compiler inputs and 40 control artifacts were frozen before execution and verified
afterward. Carrier and callback changes on the integrated base were reviewed for
reachability: disk/lab transport and application production ran freshly; there is
no authored callback in this application. No callback-host execution is claimed.
Qualification stayed pinned until completion. Earlier full replay, failed runs,
and unaffected branch evidence retain their actual producer identities. Execution
is Linux x86-64; other declared host providers require separate qualification.

At the completed main129 boundary, main advanced with invocation shared loans,
opaque storage inventory and closed entry contexts. The candidate integrated
published base `791331135359e88f666b6601323bbfba63f8d554` once. Independent review identified shared-argument
loan collection and common entry planning as affected application routes.
Completed fresh qualification uses actual source `b7ce6495681491577415563dbcfb939fb5dd4fcf`,
compiler `62e635945ca34d6856705026a4f0e09af0d0950c3370d9da40ccfadc5ef0c694` and runner
`bc773efa4689b96bb09404b43ed03c84ebda393e5c5f40e021e2b6d0fb6c9b4f`. Ordinary update passed with zero pending decisions
and zero acceptance commands. Five application commands passed in
173.177 seconds, including exact intended bounds rejection,
production, oblique native execution and Native/Terminal comparison. All 15 native
inputs passed against frozen oracles and original full-replay stdout. Native ELF
`3dfbfba1cc06a25d5d15d554e7bf22ee33a3fd050f02f378c9c4cb893f5072ba` and separately recorded Terminal branch replay or identity
reuse retain exact producer identities.

Seven Native/Terminal controls passed: the prior six formatter/float/carrier
controls plus bounded carrier copy with its projected implicit shared arguments.
Six checked negatives passed: the prior five ownership controls plus a local-backed
stored-view escape beside an opaque sibling. All intended diagnostics, interpreter
agreements and exact formatter output checks passed; wrong-reason, oracle,
infrastructure and lost-success counts are zero. Foreign ordered golden rows are
preserved verbatim from the incoming base. Release build took
252.697 seconds and corpus commands took
259.415 and 31.289 seconds. All
6,385 compiler inputs and 46 control
artifacts were frozen before execution and audited unchanged afterward. This
qualification stayed pinned until completion; earlier evidence is not relabeled.

At the completed791 boundary, published main advanced to `b5b869460e9862a29d4aecf7a2cc4a0d3a7f5dc5`.
Independent review found four query relocations preserve their predicates and
that this application has no callback address. Seeded typing retains fresh symbol
suffixes, and the common provider artifact envelope advances to version 63;
these affected package/build routes received matching fresh qualification.
Actual source `cacb7c7a55e2544e7b9bc22b7b196df0647ee6d4` uses compiler
`eed23b1cf26c843f7842680ec1cfbd2d651ca048d28f781bc2c9336752ccddba` and runner `0f44bd1fa4e46eaec3899685051aad716134129daabc2d213953fba656822fd4`.
Ordinary package update passed with zero pending decisions and zero acceptance
commands. Five application commands passed in 184.300 seconds:
source check, exact intended bounds rejection, production, oblique native and
Native/Terminal execution. All 15 native inputs passed against frozen scientific
oracles and original full-replay stdout. Native ELF is `3dfbfba1cc06a25d5d15d554e7bf22ee33a3fd050f02f378c9c4cb893f5072ba`;
additional Terminal branch replay or reviewed identity reuse is separately explicit.

Three fresh Native/Terminal controls passed: exact formatter, shared float operand
evaluation and canonical builtin guard. The separate builtin operand borrow
negative rejected for its intended reason. Wrong-reason, lost-success, oracle and
infrastructure counts are zero. Unchanged seven-comparison/six-negative results
retain actual791 producer `b7ce6495681491577415563dbcfb939fb5dd4fcf`; they are not
relabeled as this build. Release took 338.316 seconds and corpus
commands took 179.217 and 27.156 seconds.
All 6,386 compiler inputs and 46 frozen
control artifacts were audited unchanged afterward. No callback-host execution,
other-host native execution or whole-baseline qualification is claimed.

Final published-main integration base is `6c11822dceeb28f45995bce847ab298a0a4c7ed5`. Independent review and a
comment/string-excluding scan of all 128 authored core, Std and application
Omega sources found no named lifetime tokens. The added conformance admission
only iterates such lifetime operands; generated copies retain their empty roster.
The region identity helper preserves the previous owner/ordinal loop, and the
product-operand query move preserves operand filters, candidate order and the
same typed product-declaration projection. New unrelated sample/test sources are
not imported. These changes leave this application's exercised checking and runtime routes unchanged.
Execution evidence therefore retains actual producer
`cacb7c7a55e2544e7b9bc22b7b196df0647ee6d4` and its b5 base; no release or Omega
execution is relabeled as the integrated candidate. The final exact candidate
receives the normal Rust build receipt, covering the integrated API moves.

The normal FIFO claim returned final landing base
`d6fde68f40392c62a8cce2beeb3710f3a968986c`. Independent review of the intervening
atomic-place changes found their semantic branches restricted to AtomicAccess and
AtomicEvent. This application and its selected controls have no atomic cells or
calls. The ordinary Terminal field reader preserves the same backing lookup and
extracted observation body; other common changes only broaden helper visibility.
Subsequent changes add unrelated samples/tests. Qualification retains its actual
b5 and791 producers; no new release or engine execution is claimed for this base.
The exact final Rust receipt covers the integrated API threading.

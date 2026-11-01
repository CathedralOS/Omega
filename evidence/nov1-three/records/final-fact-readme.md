# Open const-expression implementation arguments

**Fact and operation:** Selected<Base + 4> at Base = 3 supplies at least seven before exact value - 7.

This row advances `STATIC-MACHINE-MIGRATION` in
[TASKS.md](../../../../TASKS.md). Its distinct mechanism is an open, typed const
expression in a selected named implementation's explicit argument tuple. The
compiler must preserve the expected `u64` carrier, exact binder declarations,
complete selected application, and public result contract through outer
substitution. An expression is not a structural type or a guessed argument.

The witnesses are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[a nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [another target body](target_body/main.omg). Each has `invalid/main.omg`
beside it. Every twin changes only the construct's nested `Selected<Base + 4>`
to `Selected<Base + 3>`; the required minimum, construct guarantee, and consuming
subtraction remain unchanged. The [root](main.omg) retains its original source.
Each positive binds all four hosted `ProgramEntry` targets and expects exit 0.

The [field place](field/main.omg) surrounds the carried value with nonzero
neighbor canaries and checks both neighbors after projection. The
[match join](match_join/main.omg) invokes both Boolean paths; each carries the
same value and lower bound, while distinct tags (17 and 29) make inverted or
wildcard dispatch observable. Both still consume the unchanged `value - 7`
operation directly.

Two additional productions stay in this fact:

- [Private package projection](const_name/package_projection/main.omg) evaluates
  the selected implementation through a package that retains a private helper
  and a copyable record constant's `amount` projection. Its
  [twin](const_name/package_projection/invalid/main.omg) changes only the
  implementation's const argument in the package's `construct` call.
- [Statement result](const_name/statement_result/main.omg) calls the selected
  member in statement position. Its exact public write guarantee establishes
  the scalar later returned and evaluated as a constant. Its
  [twin](const_name/statement_result/invalid/main.omg) changes the same const
  argument, preserving the bound and subtraction. The
  [saved-output composition](const_name/statement_result/saved_output/main.omg)
  copies the completed scalar before overwriting the original; its
  [twin](const_name/statement_result/saved_output/invalid/main.omg) changes the
  same selected argument.

The [private result guarantee control](controls/private_result_guarantee/main.omg)
asks a generic wrapper for equality where its public requirement promises only
a lower bound. It must reject even though the selected concrete member has a
stronger equality guarantee. A prepared executable target does not donate that
private contract to the generic caller. The
[current-output write](controls/changed_statement_output/main.omg) and
[outgoing return write](controls/return_write_output/main.omg) controls reject
at the public postcondition after a write retires its current-storage truth.
The saved scalar keeps its historical guarantee independently of that truth.

Applicable requirements:

- **Establishment:** the checked `Selected` member and its complete `Reading`
  application establish the lower bound. `Base <= 100` establishes arithmetic
  formation; a caller guard supplies no subtraction premise.
- **Identity and lifetime:** immutable const binders and completed scalar values
  keep their declaration/occurrence identities. No storage or borrow authority
  follows from an equal scalar value. Normal mutable-output guarantees name
  the exact call-return version; copies retain it, while overlapping writes
  retire its association with current storage.
- **Composition:** simultaneous actual substitution transports `Base + 4`;
  calls, joins, and state arrivals retain the same lower bound. The operation
  `value - 7` consumes that bound directly.
- **Soundness boundaries:** complete tuple kinds/carriers, unused operand
  formation, public contract refinement, and normal-call completion remain
  obligations. Invocation-owned logical signatures do not become declaration
  evidence or provider-selection authority.
- **Psi 05–07:** the checked inline member selection and scalar contract must
  reach target-neutral Terminal Psi and execute there.
- **Omega 00–02:** ordinary `u64` layout and validated hosted entry/call plans
  realize the selected member. This row introduces no new ABI placement lane.
- **Omega 03–06:** scalar instructions, register homes, and activation frames
  must preserve the authored result.
- **Omega 07–09 and emission:** the native image must execute with exit 0 and
  agree with Terminal Psi. Hosted declarations alone do not establish execution
  on another host.

The governing clauses are [generics](../../../../wiki/spec/language/generics.md),
[conformances](../../../../wiki/spec/language/conformances.md),
[dependent values](../../../../wiki/spec/language/dependent_values.md),
[evaluation](../../../../wiki/spec/language/evaluation.md), and
[constants](../../../../wiki/spec/language/constants.md), and
[normal return contracts](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use).

At fixed base `bfcffe6bcfcedb6fd60713e223408566c6c4e10d`, the unchanged
local witness wrongly refuses at `selected`'s `result >= Minimum` guarantee.
Borrow analysis names the exact applied public requirement signature, while
capture preservation compared the authored evidence child. After that join is
repaired, the unchanged const-name place exposes a second refusal: semantic
evaluation executes the original map-row template and loses `Value`, although
the call already retains its prepared capture entry. Both failures were recorded
before their respective compiler edits, with unchanged source and oracles.

Optimized producer `2de5d6af94b2460e5c85e5162a72a1fbd568eee1` observes native
and Terminal exit 0 for the unchanged root and all nine original places; all ten
original reject controls still refuse for their authored tuple mismatch. Those
original sources and observations remain in the claim evidence, before the
field/join runtime oracles were strengthened. The additional productions and
private-guarantee boundary retain their own current observations there. This is
Linux x86-64 evidence; declarations for the other hosted targets do not supply
new execution observations.

The statement production exposed two further obligations before edits: readable
scalar output equality must imply the published lower bound during declaration
refinement, and the caller must join its returned scalar to that exact completed
output version. Current-storage association cannot outlive a later write, even
when a saved copy retains the old proposition. The frozen direct statement and
saved-output sources passed checking before runtime calls were added to their
oracles. All intermediate refusals, corrected probe formation errors, and the
stale return-write acceptance are retained with their actual producer identities
in the qualification record.

CURRENT_ACCEPTANCE_EVIDENCE


The optimized reference producer `5df98d50e149` has actual Linux x86-64
native and Terminal exit 0 for the unchanged root and all nine places, with
all ten construct-sensitive reject twins refusing. The same frozen qualification
also observes private selected helper/constant projection, nested aliases,
floating constant dependencies, effect isolation, distinct implementation
binders and the completed-call controls. The exact nine positive/reject source
and Build mirrors live under
`tests/omega/pass/facts/open_const_implementation_through_<place>_exit` (nine
positives, byte-identical to their place program and Build pair) and
`tests/omega/fail/facts/open_const_implementation_through_<place>_rejected`
(nine twins, byte-identical to `<place>/invalid`). The reject family was
renamed from the legacy `open_const_implementation_bound_through_<place>`
spelling in the same change; the rename touched no fixture byte, verdict,
timing, stamp or diagnostic line.

Measured on macOS arm64 with the `factsr1` shared opt-level 1 producer (built
at `9ff26a870215d1a351e830cacd1255a852e3dd1e`): the root and all nine places run
`omega run --both` with native exit 0 and Terminal exit 0 (10/10 agree), and all
nine `<place>/invalid` twins exit 1 at checking with the verbatim diagnostic
"generic machine `selected` cannot bind `Implementation` to conformance
`Selected`: expected a complete `Reading` implementation with the instantiated
trait arguments". Each twin differs from its positive on exactly one line: the
selected application's own const argument `Selected<Base + 4>` becomes
`Selected<Base + 3>`; the required bound, the consuming `value - 7` and every
other line stay identical. Recorded on macOS arm64 at `ab30b42820ea+wt`: 18 rows
in `corpus_outcomes.txt` (9 `checked check-target:macos_arm64`, 9 `rejected
expected` carrying both diagnostics), 9 rows in `corpus_native_macos_arm64.txt`
(`built ... exit:0`) and 9 rows in `corpus_both_macos_arm64.txt` (`exit:0
interpreter-exit:0 interpreter:agrees`). The nine `<place>/invalid` twins and the
root invalid control also carry their intended-invalid `sample.json`; the sample
check gate reports 40/40 checked with 20/20 intended rejection controls
matched.

After integration, the optimized `8b23a13f8b1c` producer reobserves all 21 pinned
positive programs with native/Terminal agreement and checks the 13 selected
existing consumers against their authored oracles and exact baseline diagnostics.
Root program source is identical to the local place; only its Build application
label differs. The retained 5df public/negative observations remain labelled with
their actual producer. The selected current refresh covers changed checking and
backend consumers; it does not relabel historical executions.

The normal qualification records distinguish raw unpinned NEW outcomes and three
baseline-attributed diagnostic changes from the strict source/engine companion
checks. No unrelated golden is changed. Detailed source hashes, actual streams,
ordinary accepted-input identities and optimized producer profiles are retained
in the existing swarm claim evidence. Native execution on the other three hosted
targets and the full parent capability remain unqualified.

The existing named-NaN program currently reaches its accepted-entry authority
boundary; no NaN engine observation is claimed here.

This row does not close the parent. Other calling policies and member kinds,
complete inline reference-row migration, product-producer parity, the parent's
remaining contract/authority negatives, and cross-host execution remain open.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.

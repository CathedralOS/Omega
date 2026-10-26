# Static callback identity through conformance invocation

Parent: **STATIC-CALLBACK-REACH-RETENTION**, `TASKS.md`.
Contract: [static callback reach dependencies](../../../../wiki/spec/language/effects.md#static-callback-reach-dependencies).

A static callback retains its exact selection and bounded reach through a
conformance invocation; its transported result discharges `37 - result`.
Every place copies that sentence, the bound 37 and that consuming operation.
The public callback and traversal contracts establish `result <= 37`; no
caller equality guard establishes the bound. Runtime equality is only the
exit oracle.

The same named conformance member selects `quiet`, a same-shaped second pure
callback, and `console_step`. The latter declares Console reach despite an
inert body; body inspection cannot narrow its public promise. The ordinary
traversal also selects both pure and Console callbacks. `pure_observer` has an
explicit empty reach ceiling and consumes the pure specialization. Later
applications must retain the original generic definition and its bound.

The nine transport places are [local](local/main.omg),
[field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[const name](const_name/main.omg), [nested machine](other_state/main.omg),
[another machine](other_machine/main.omg), and
[target body](target_body/main.omg). The const place supplies the exact input
through a named constant; the completed call still establishes its result
contract. The nested-machine place uses the currently implemented `state`
spelling, matching the existing fact rows. Source spelling migration remains
on NESTED-MACHINE-UNIFICATION.

Each `invalid/main.omg` changes only `quiet`'s declared reach to Extra. This
exceeds StepContract's Console ceiling. The selection must reject even though
the callback's body, numeric result and downstream subtraction are unchanged.
The diagnostic includes `does not refine` / `service reach Extra exceeds its
authored ceiling`, and the independently checked satisfaction also rejects.
Nine pass/fail fixtures are pinned under
`tests/omega/{pass,fail}/facts/static_callback_reach_through_*`.
The positive fixtures additionally request `main closed-conformance-controls`:
source-produced artifacts must survive canonical reload, and the corpus
checks missing/changed invocation tuples, original schemas, commitments and
same-shaped alternative selections through existing external-ingress controls.

## Baseline and repair

At `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, a valid generic conformance
callback runs with native exit 0 and Terminal exit 0, but its saved artifact
rejects source-free reload:

```text
Semantic(MalformedStructuralFoundation("static conformance method tuple differs from its retained executable invocation"))
```

Reach telescopes retained a package-qualified owner, overload and exact selected
entry. Conformance method tuples instead retained the callable evidence
identity. The two independently produced records therefore disagreed about the
same callback. The repair makes reach telescopes, nested schema call arguments
and conformance method arguments use one exact selected-machine identity
operation. Checked contract commitments and the external-ingress comparison
remain unchanged. Historical malformed artifacts remain rejected; they are
not relabeled as accepted candidate products.

Evidence is retained in the task's ignored `build/static-callback/` directory,
with exact command, source/producer hashes, diagnostics and saved artifacts.
The source fixture is the durable replay input. The selected-identity, terminal
guarantee and recursive-entry repairs landed in PR #145, published as
`3c1b7104e4f62b22ddd31502f076fb3bf5ab7bef`. Follow-on retention changes
remain under qualification in PR #168.

On the Linux x86-64 executor, the ordinary CLI sample sequence produced nine
native exits 0, nine Terminal exits 0, nine successful saved-artifact reloads,
and nine intended reach rejections. After integrating actual interpreter changes
at `3353916f943`, the filtered corpus run recorded nine `built` rows with
`exit:0 interpreter-exit:0 interpreter:agrees terminal-inspections:match`.
Its checked leg recorded nine `checked` rows and nine `rejected` rows, with
zero wrong expected fragments, failed runtime oracles or lost successes.
Goldens retain the actual `3353916f9430+wt` observation stamp. The earlier CLI
observations retain their original producer identity; this is no other-host
or published-main execution claim.

## Supplemental productions

[Selected schema](controls/selected_schema/main.omg) calls two exact const
applications (`2` and `3`) through one selected nominal generic callback,
retains a second unused Console-reaching selection, and observes result 3.
Its twin changes only the second application to 38, exceeding the public
`Count <= 37` requirement. The pure observer keeps empty reach even though
its unused selection publishes Console.

[Recursive component](controls/recursive_component/main.omg) invokes a pure
callback before each measured self-entry transition. The callback decrements
its bounded input, so input 37 and rank 3 must produce 33 after four calls;
`37 - result == 4` is the runtime oracle. Its twin removes the rank decrease.
Both public CLI programs run with native/Terminal exit 0 and reload their
saved artifacts. The filtered corpus additionally records two dual-engine
positives with successful Terminal inspections and their two intended rejects.
These are supplemental productions of this fact, not another nine-place matrix.

[Terminal binder](controls/terminal_binder/main.omg) retains the same bound
through two named terminal arms. Its twin removes the public result guarantee;
the provider's stronger private promise cannot prove the generic body's exit.
The ordinary traversal in every transport place also uses a named terminal
callback call. All nine updated places execute with native/Terminal exit 0,
reload their artifacts, and reject their reach twins on Linux x86-64.

The original bare terminal probe first rejected its `result <= 37` exit
obligation. Contract calls used the binder as both executable target and logical
signature state, while published facts retained the binder owner and original
signature state. Keeping those frames distinct exposes the exact public promise.
The shared normal-result reader joins that invocation to the caller's exit
without creating an expression for a named arm, and keeps input capture, write
preservation and dependency-cycle checks. Dependency traversal also consumes
ordinary and captured applications' retained typed selection ledgers.

After checking succeeded, executable planning still classified specialized
terminal calls by their retained static-binder marker. It now follows the
resolved target, while keeping that marker for reach retention. Both the bare
and guarded probes execute in both engines and reload successfully. Their
historical checking/lowering refusals retain their original producer identities.

The focused checked corpus records 17 checked positives and 14 intended rejects,
including the required existing effects fixtures, an independent-result cycle
control, and overwritten-field guarantee rejection. Twelve callback positives
also pass dual-engine execution and Terminal inspections. An additional normal
return integration witness runs natively but refuses Terminal execution at
machine 3/block 20/operation 19. That operation copies a borrowed byte descriptor
from a fixed-array element; the unchanged interpreter reader omits its existing
full-home descriptor map and refuses the field-only fallback. The matching
reader omission is present at the base, but baseline emitted bytes/execution
have not been compared. This control is not counted as a dual-engine pass.

The recursive terminal-entry control now executes with both engines and reloads
its artifact. Abstract callback leaves consume only an unconditional public
termination contract; the missing-public-promise and nondecreasing-rank twins
reject independently. Concrete recursive edges still owe strict descent.

## Requirements and limits

- Establishment: the callback and requirement bodies prove their public entry
  and result contracts. The selected requirement bounds service reach.
- Identity and lifetime: selected owner, overload, entry and checked contract
  commitment are exact; a same-shaped callback is a distinct selection.
  This scalar row introduces no borrowed-storage lifetime claim.
- Composition: the bound survives the nine listed transports, and generic
  invocations retain their complete static machine argument tuple.
- Soundness: Extra-reaching twins reject. Existing external-ingress controls
  challenge substituted or incomplete conformance invocation records. No
  contract, authority or custody check is disabled by this repair.
- Psi 05–07: retained conformance and reach records agree after source-free
  artifact reload. Terminal execution is measured separately from reload.
- Omega 00–02: all four hosted ProgramEntry bindings are authored. This row
  uses ordinary scalar calls and a record field, not foreign/provider ABI.
- Omega 03–06: the runtime oracle checks both selected callback results,
  transported subtraction and pure/declared-reach calls after ordinary calls
  and state transfer. It does not claim new register/stack boundary coverage.
- Omega 07–09 and emission: native/Terminal agreement is measured on the
  executing Linux x86-64 host only. Other hosted targets remain unexecuted.

The follow-on controls exercise the remaining retained-data distinctions in this
same fact folder:

- `captured_selection`, `captured_schema` and `captured_scope` preserve an
  outer selected callback while a conformance method adds its own invocation
  telescope. Original and cloned binders keep exact correspondence.
- `requirement_telescope` preserves unused proposition and declaration-identity
  arguments in a nominal requirement's complete trait tuple. Same-shaped
  declaration replacements and wrong-kind lexical shadows reject.
- `evidence_scope` preserves the exact selected conformance application for a
  nested proof-static requirement argument; another conformance rejects.
- `private_helper` keeps both empty ordinary caller reach and a helper's fixed
  Console contribution. Omitting the latter from the caller rejects.
- `captured_unused` retains the whole capture even when its callback has no
  emitted callee or invocation. Removing its application, outer telescope or
  independent checked capture header rejects artifact ingress.
- `captured_shadow` keeps outer and method-local const binders distinct despite
  equal spelling. `boundary_schema` retains selected generic const tuples and
  their bounds; its checked body is not evidence of an opaque foreign leaf.

All nine follow-on controls execute with native/Terminal exit 0 and reload on
Linux x86-64. Their eleven source reject controls still reject. The original
nine-place row also passes both engines and reload, with nine Extra-reaching
reject twins. The target-body source realizes all four authored hosted targets;
foreign-host execution has not been observed. These observations retain their
actual source and producer hashes in `build/static-callback/`.

The focused fixture inspections now pass, with nine new dual-engine outcomes
and nine checked positives plus eleven intended rejects recorded through the
published-base-plus-staged-source procedure. The three required existing effects
fixtures pass their applicable checks; nominal and generic-schema witnesses also
execute in both engines. Scoped Clippy and independent semantic source review
pass. Full outcome acceptance still requires verified landing of the follow-on
source. An open PR does not supply main acceptance.

Run a place after ordinary package preparation:

```sh
omega update --project samples/cli/facts/static_callback_reach/local --target linux_x86_64
omega run --both --keep samples/cli/facts/static_callback_reach/local/main.omg
omega inspect-terminal --artifact <kept-directory>/program.oterm
python3 tools/corpus_gate.py --both --filter facts/static_callback_reach
```

The source-free ingress validator now replays finite unions, requirement bounds,
complete call tuples and exact operation markers. Corrupting the selected reach
on a real saved artifact was accepted before the validator and now rejects.
Thirteen callback corpus positives execute with both engines; the updated local
and schema controls reject malformed retained data. A subsequent duplicate
operation identity guard was checked using the rebuilt codec and the same saved
source-produced artifacts; the earlier full CLI producer predates only that guard.

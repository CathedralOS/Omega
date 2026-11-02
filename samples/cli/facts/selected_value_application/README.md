# Selected runtime Value application

Owning tasks: **BUILD-OWNED-TARGET-SELECTION** and
**RUNTIME-VALUE-GENERICS** in [TASKS.md](../../../../TASKS.md).
Requirement: an implementation's captured type telescope and its member's
independent Value and explicit evidence telescopes compose without treating
realized Value arguments as extra public formals, losing their exact subjects,
replacing a caller's evidence with a Build provider, or dropping returned facts.

The current Build-selection migration is unfinished. The optimized source
probe at `c83230e4576` admits the new evidence declaration but wrongly refuses
the valid target witness's exact subtraction; its index-11 twin still refuses
with the intended bound diagnostic. Native/Terminal qualification of this
migration is pending. Earlier evidence for the ordinary-wrapper row does not
qualify these changed sources.

Fact: A selected member preserves runtime index 9 below bound 10, so exact
10 - right yields 1, while returning the second Cell and closed left 7.

Acceptance: run each positive below with `omega run --both <main.omg>` on a
matching hosted target; each must produce native and Terminal exit 0. Every
positive binds all four hosted targets. Runtime evidence is host-specific;
the qualification for this repair executes on `macos_arm64`, not all four hosts.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| A nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The same sources and Build companions are pinned in
`tests/omega/{pass,fail}/facts/selected_value_application_through_<place>_*`.
Each twin changes only the supplied index from 9 to 11. Parameter, state and
machine transports reject at their preceding `requires`; the other six reject
the unproved exact subtraction. The separate [bound control](probes/invalid_bound/main.omg)
removes the subtraction and still rejects the invalid selected call.

[Distinct private/public binder names](probes/distinct_binder_names/main.omg)
and [two Value binders sharing one runtime subject](probes/shared_runtime_subject/main.omg)
challenge name-based or one-to-one-actual correspondence. Source-only formation
controls under `tests/omega/{pass,fail}/generics/unused_selected_value_*` check
unused member carriers, proved/unproved `where` facts and Value-to-Const refusal
even when the generic wrapper has no executable instantiation.

Fresh optimized baseline `1b2dbda4df7b59894492bda731238f4338f4b3fe`
rejects the unchanged source-only reproduction with `state choose expects 2
argument(s), got 3`; exact subtraction also lacks the returned-bound proof.
The baseline reproduction and authored oracle are retained unchanged. Raw
producer-bound runs, controls and final review belong to this repair's pinned
qualification and claim record, not a claim that every host has run.

The selected application establishes the index obligation before execution;
the consuming subtraction cannot be licensed by a post-call equality guard.
The two same-carrier ordinary arguments distinguish parameter identity from
arity, while one closed and one runtime Value binder distinguish the runtime
roster from the whole independent member telescope. Declaration identity,
actual subject identity, full contracts, ownership and erased/runtime relevance
must survive the source-to-Terminal-to-native path. There is no implicit
provider value, per-index specialized body, policy authority or OS structure here.

The nine places call the same `SelectedValue::selected` contract. Ordinary
places select `SelectionAdapter` in Build; the target place selects four named
implementations through four explicit target rows. The caller still supplies
`Second<Cell>` independently. Every implementation retains the same declared
`Choose<Cell>` promise and runtime Value index.

The additional producers stay in this fact folder:

- [Public evidence](probes/public_evidence/main.omg) retains the exact bound in
  source-free review and recovery.
- [Two independent binders](probes/two_evidence/main.omg) distinguishes the
  supplied maps; its unused first binder still owes its declared application.
- [Unused evidence](probes/unused_evidence/main.omg) has a provider-bound reject
  control, independently of whether the body invokes that evidence.
- [A default body](probes/default_evidence/main.omg) preserves its declaration's
  evidence namespace through synthesis.
- [A nominal callable](probes/nominal_evidence/main.omg) retains the same full
  telescope through an explicitly selected callable parameter.
- [Wrong argument](probes/wrong_evidence_argument/main.omg) and
  [wrong nominal trait](probes/wrong_evidence_trait/main.omg) controls retain
  complete static admission.

The strengthened match place transports index 9 alongside route markers 17
and 23 and executes both routes. With the retained pre-migration producer,
the marker-preserving ordinary wrapper exits 0 in both engines; swapping only
the markers causes native trap signal 5 and Terminal `Trap`. This isolates the
oracle change from the new requirement declaration.

## Requirement scope

- Establishment: the selected call owes `Right <= 10`; its postcondition binds
  the returned field to that exact Value argument. Exact subtraction consumes
  the transported fact before the runtime result checks.
- Identity and lifetime: public and private binders are joined through the
  admitted realization lineage, not names or equal carriers. Completed return
  occurrences are not repeatable arithmetic calls. This row does not establish
  indexed storage compatibility across mutation or policy/loan authority.
- Composition: captured `Cell` and independent closed/runtime member arguments
  compose through all nine places; both match arms carry the same fact.
  Unused-template controls retain complete source tuple admission.
- Soundness: runtime Value is not Const. Mathematical facts do not authorize
  access, erase call formation, or establish repeated effectful-call equality.
- Psi 06–08: ordinary Value actuals and the returned record survive lowering
  into Terminal Psi and interpreter execution. No new proof-only, cleanup or
  loan construct is claimed by these programs.
- Omega 00–02: existing Build providers, applied record layouts and calling
  plans realize the same explicit values. No hidden provider state is added.
- Omega 03–06: the chosen host executes the shared member body, record return,
  scalar fields, exact arithmetic and call-preserved values.
- Omega 07–09/emission: actual native execution agrees with Terminal Psi and
  the authored exit oracle on the recorded host. Other hosts remain unverified
  by this session; no new encoding/relocation mechanism is claimed.

The parent task remains open for dependent data/storage qualifications,
reassignment identity, other constructor/forwarding routes and linear custody.
Cathedral's installed-policy composition is a separate remaining customer gap;
this fact establishes its selected-call argument-correspondence prerequisite.

## Build-owned target-body migration

The `target_body` place still spells the retired target prefix. Its family
member is generic over an implementation map, and a boundary requirement's
signature does not admit an evidence binder, so the place cannot yet move to
named implementations selected by Build rows without changing the fact. The
owner decision is
[OWNER_QUESTIONS.md question 6](../../../../OWNER_QUESTIONS.md#6-may-a-boundary-requirements-own-signature-bind-a-generic-evidence-parameter).

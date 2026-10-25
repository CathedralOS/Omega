# Selected runtime Value application

Owning task: **RUNTIME-VALUE-GENERICS** in [TASKS.md](../../../../TASKS.md).
Requirement: an implementation's captured type telescope and its member's
independent Value telescope compose without treating realized Value arguments as
extra public formals, losing their exact subjects, or dropping returned facts.

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
- Psi 05–07: ordinary Value actuals and the returned record survive lowering
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

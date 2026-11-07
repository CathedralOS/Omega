# Psi claim admission migration

The ratified [Psi product contract](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
keeps source proof checking, removes transported producer derivations, and requires
incoming artifacts to establish their retained claims. Internal compiler stages
reuse checked judgments. This migration is **PSI-CLAIM-ADMISSION**, tracked with
**PROOF-KERNEL-CORE** in [TASKS.md](../../../TASKS.md).

## Existing implementation and scope

At `5c34da0c7bf7c`, source lowering emits mathematical certificate bytes in proof
terms and indexed theorem roots. The semantic codec embeds and decodes them.
`verify_module_mathematical_actuals` and `verify_indexed_theorem_roots` supply
receiver-replay helpers exercised by tests. Canonical artifact decoding explicitly
does not establish semantic proof validity; the source `validate_program` function
is not an incoming-Psi verifier.

Keep the proof admission kernel and local proof objects. Source checking uses
them, and payload-entry semantics constructs and checks local proofs. Bounded
source contract automation also has trusted routes without kernel evidence;
removing transport does not complete or remove that source-checking work.

## Implementation work

1. **Separate semantics from derivations.** Inventory certificate-bearing proof
   terms, indexed theorem roots and carrier keys. Preserve mathematical values,
   definitions, propositions, exact subjects, substitutions, law selections and
   assumption dependencies needed by consumers. A serialized mathematical value
   or type is not expendable merely because the old format wrapped it in a
   certificate. Replace that wrapping with semantic representation where needed.
   Preserve progress measures and custody lineage needed to reconstruct obligations,
   even where existing names call these relationships evidence or derivations.

2. **Retire proof shipping.** Remove producer derivation fields and emission,
   their wire codec paths, and receiver helpers used only to replay those bytes.
   Preserve local kernel checking and oracle calls. Migrate semantic format
   markers, identities, decoders and fixtures together; reject old proof-bearing
   encodings rather than interpreting their bytes as trusted facts. Assign exact
   replacement tags with the implementation, not this document.

3. **Complete receiving admission.** Reconstruct operation, call, return, arrival,
   ownership and authority obligations from the artifact. Prove required claims
   using the receiver's procedures and admitted work budget. Calls may use a
   callee contract only under the existing checked body or explicitly admitted
   boundary rules. Producer success flags, hashes and theorem names cannot
   establish premises. Missing justification, unsupported reasoning and exhausted
   resources refuse admission. Apply this to interpreted and native consumption,
   including caller-supplied in-memory modules.

4. **Preserve theorem-dependent justifications.** Trace erased theorem-only
   citations into every affected published obligation. Retain enough semantic
   definitions and dependencies for independent checking. A theorem conclusion
   alone, or a retained requirement with no way to establish it, is insufficient.
   Publication and lowering must preserve this information through optimization.
   Do not resolve a missing justification by granting implicit producer trust.

5. **Keep internal reuse cheap.** Trusted immutable outputs within one compilation
   may reuse their established judgments and exact subjects. Incoming artifacts
   cannot manufacture that status. After external admission, consumers may reuse
   that exact immutable admitted product under its bound assumptions and authority.

## Acceptance

The existing **PROOF-KERNEL-CORE** work remains: finish identity and metatheory;
source mutual, nested and derivation families; higher indices and open carrier
transport; polymorphic-identity storage receipts; and UIP/extensionality rejection
controls. Retain its existing `--both` acceptance for
`samples/cli/facts/{closed_integer_bound,negated_mathematical_order,source_indexed_*,erased_reference_contract}`
and `proofs/quotient_*`. The receipt migration adds these controls:

- A directly loaded Psi artifact establishes its required claims before either
  interpreter execution or native realization. Proving may construct and check
  local proof objects; no producer derivation payload is required.
- A well-formed, canonically encoded artifact with a fabricated fact, mismatched
  subject, unproved callee requirement, invalid arrival or invalidated loan rejects
  for the relevant obligation. Structural decoding alone must not pass it.
- A source theorem establishes a call-site requirement; its published artifact
  retains sufficient semantics for admission. Removing or changing the necessary
  definition/dependency rejects rather than trusting the erased citation.
- Exhaustion and unsupported reasoning refuse admission with distinct reasons.
  A smaller receiving budget may refuse a valid claim without changing its meaning
  or canonical identity. Existing source specialization still reuses checked facts.
- Mismatched or unaccepted external assumptions reject. Provenance and package review
  preserve their own authority checks without substituting for claim verification.
- Existing local kernel, payload-entry and source proof tests continue to pass.
  Codec tests cover stale proof-bearing formats and canonical replacement forms;
  normal internal compilation does not add receiver proof replay.

Extend existing suites with focused positive/rejection controls and compare
admitted artifact execution through both engines. This work proves the artifact's
retained claims; source correspondence and faithful native lowering remain trusted
compiler responsibilities in the [verification table](../../../omega-rust/pipeline.md#verification-placement).

## Separate tooling work

The [named stage-validator options](../../spec/build/command_line.md#independent-stage-validators)
remain CLI implementation work under **RC-RELEASE-RECORD-AND-CLOSURE**. They run
optional independent checks on stage outputs and report exact coverage. They
neither enable nor disable required incoming-Psi admission and introduce no
assurance profile. Keep their acceptance separate from this migration.

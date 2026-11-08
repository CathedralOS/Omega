# Callback composition and explicit field omissions

Ratified 2026-11-06. Wrappers retain the contracts of callbacks they forward,
and synchronous selections avoid unnecessary suspension/blocking requirements.
Construction sites explicitly opt into zero defaults so new fields cannot
silently acquire values at exhaustive literals.

This record covers three source-design changes. The normative rules live in
the specification links below; compiler acceptance is separate from this
documentation ratification.

## Decisions

1. [Explicit field omission](../../spec/language/data_and_literals.md#explicit-field-omission):
   a final standalone `..` opts into zero-valued omissions in records and case
   literals, including common fields and active payloads. Without it, provide
   every runtime field. Prove complete validity and custody either way.
   Erased construction and storage zero-initialization keep their own rules.
2. [Forwarded callback requirements](../../spec/language/generics.md#forwarded-callback-requirements):
   an explicitly kinded machine binder can inherit exact requirements from
   selected callees. This includes public wrappers. Origins stay explicit;
   multiple uses require all applicable contracts. No trait search,
   implementation choice, callable-shape guessing or invented authority.
3. [Operational dependencies](../../spec/language/effects.md#static-callback-operational-dependencies):
   derive finite OR summaries for suspension/blocking from static callbacks
   and fixed calls. Public generics declare suspension/blocking ceilings and
   publish the summaries.
   Bodies use upper bounds; callers substitute selected published contracts.
   Service reach follows the later [deferred callback reach](deferred_callback_reach.md)
   decision for structural, nominal and conformance-member selections.

A public wrapper's inferred requirements and dependencies are interface data.
Changes invalidate dependent checking without requiring private-source
inspection. The normalized identity retains exact origin requirements and
substitutions. A generic's authored contract cannot be silently strengthened
to make an invalid body pass.

No conditional carry checking is added. Ordinary loans can cross suspension
with the required preservation evidence, but a synchronous specialization does
not excuse a generic body that violates its upper-bound carry or window rules.

Niche layouts, error-propagation shorthand, loop syntax, anonymous machines and
captures receive no design change. The invalid-payload-niche prohibition stays
in force. Existing some packages and machine values retain their contracts.

## Implementation acceptance

| Area | Required behavior and refusal |
| --- | --- |
| Exhaustive literals | Accept all supplied runtime fields; reject a missing zero-valid field without `..`. Adding a field requires an update at exhaustive sites. |
| Defaulted literals | Accept omitted zero-valid fields with `..`; reject invalid zero values, failed aggregate couplings, missing establishment authority and invalid callable/borrow defaults. |
| Cases and nesting | Apply the marker to common and active payload fields; require explicit case selection; do not initialize inactive payloads or authorize omissions in an authored nested literal. |
| Syntax and custody | Reject duplicate/nonfinal markers and update/spread operands. Preserve authored initializer order, partial cleanup, linear establishment and erased-field rules. |
| Forwarding | Derive the same requirement through private/public and multi-layer wrappers, independent of body statement order. Preserve exact substitutions and nested callable contracts. |
| Conjunction | Enforce every forwarded requirement; reject incompatible callable shapes, unsatisfied contracts and public requirements violating visibility. Do not silently strengthen an explicit origin. |
| Missing origins | Reject a bare callback invocation without a contract, a forwarding cycle with no origin, implementation search and attempts to invoke identity-only binders. |
| Public effects | Require suspension/blocking ceilings covering the abstract body. Specialize those axes independently; retain fixed contributions. Service reach may be inferred. |
| Markers | Require markers covering body upper bounds and substituted caller envelopes; reject missing markers and permit redundant ones under [conservative acknowledgements](../../spec/language/effects.md#call-site-acknowledgements). |
| Safety and evaluation | Preserve upper-bound carry, windows, termination and outcome checks. Admit evaluation only when the specialized contract meets all existing evaluation conditions. |
| Representation | Carry requirement/dependency identities through checked artifacts and specialization; lower the specialized operational envelope in both interpreter and native paths. |
| Evolution | Retain summaries without private-source lookup; invalidate dependent artifacts when their originating requirement, forwarding use or fixed operational contribution changes. |

Implementation must migrate affected library/sample construction and callback
surfaces together with their positive and negative coverage. An unchanged
compiler is not evidence that these ratified source rules execute. Corpus pins
must not be weakened to accommodate the source migration.

## Documentation coverage

The sweep updates data/establishment/evaluation rules, generic and machine
contracts, effect and carry rules, guides, the README example and the library
iteration guidance. The anonymous-machine proposal only updates its references
to accepted named-machine rules; its status stays proposed.

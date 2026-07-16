# `Versioned<T>` Stage-3 Placement

Current as of 2026-07-18. Chapter 22 is the language authority.

## Implemented role

Stage 3 implements an era-bearing `Versioned<T>` container for data types with
declared historical shapes. Version-match arms lower to era tests and payload
reads. Only boundary/decode machinery can mint the container.

This representation belongs to persisted/wire history:

| Concern | Semantic owner | `Versioned<T>` role |
|---|---|---|
| Persisted/external data with multiple historical shapes | Chapter 21 wire/schema model | Era-tagged decode result and match surface |
| Live in-memory state during replacement | Chapter 22 replacement plan | None; use typed upgrade plans and normalized component contracts |

The numeric era is a decode discriminator, not component identity. Durable
shape identity comes from the normalized schema/layout artifact. Component
identity comes from normalized machine contracts.

## Engineering rule

Do not extend `Versioned<T>` into the live hot-swap implementation. Stage-4
work for live replacement is separate:

- `Upgradable<Old, New, Context>` checking;
- owned `replace` plan validation;
- quiescence/capture/upgrade/install obligations;
- bounded version-liveness pins;
- normalized import-slot admission; and
- the still-open coexistence/outbound-call policy.

The existing era container remains useful, but its provenance comments and
future constructors should point to the wire-decode role rather than implying
that it is the component hot-swap mechanism.

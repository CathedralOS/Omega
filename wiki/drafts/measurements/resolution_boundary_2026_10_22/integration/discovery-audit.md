# Placement-discovery dependency audit, 2026-10-22

This follows the [rejected bypass](../placed_discovery/README.md). The bounded
question is whether the early full resolver can be replaced by a smaller product
without losing placement requests or invocation authority. This is a source audit
and experiment design, not a measured implementation or a saturation claim.

## Consumer and current cost

The existing measurements found one early full resolver per compile, resolving
5,559–5,615 expressions for the five positive workloads. The three ordinary
workloads produced no placement selections. The returned/forwarded view programs
produced six/nine selection records and four/seven invocation locations. Those
historical counts remain bound to their recorded producer; this audit does not
relabel them as measurements of the integrated compiler.

`compile-time/src/layouts/placed_views/applied_results.rs::retain_applied_results`
uses `resolve_numeric_probe`, scans resolved calls, validates sealed borrowed
View prototypes, substitutes exact type actuals into declared return types, and
writes selected policy/schema/template references back into syntax. Its downstream
consumer joins declarations and invokes `require_common_floor_for_invocation` for
every use location. The accepted [placed-access contract](../../../../../wiki/spec/resources/placed_access.md)
requires visibility and direct-dependency authority before synthesis.

## What cannot be omitted

| Input | Current owner | Consequence for a smaller product |
| --- | --- | --- |
| Declaration identities, signatures, source custody | Resolver declaration indexing and `placed_borrow_factory` | Retain exact machine/state symbols and prototype clauses, including resident backing domains. |
| Preceding locals and their declared types | `lowering/state/lexical_inputs.rs`, `symbols/statements.rs` | Call selection must preserve statement order and local scope. |
| Receiver structure | `symbols/expression_paths/calls.rs` and `projected_receivers.rs` | Indexed/member/borrowed receivers need their structure and local/parameter types. A call-result receiver intentionally remains unresolved until typing. |
| Static actuals and selected call target | `symbols/expressions/traversal.rs`, `references.rs::assign_call_symbol` | Reuse ordinary selection; names alone do not distinguish declarations or authority. |
| Return-type wrappers and use locations | `applied_results.rs::retain_result` | Retain reference, constraint, array, slice, generic and dynamic-trait traversal and every invocation coordinate. |
| Normalization and routing | `resolution.rs::finish_resolution` and `finish_resolved_tables` | These do more than compact storage: constant settlement, conformance routing and target siblings can affect selected calls. Their dependency must be demonstrated before skipping them. |

The current initial guard asks whether *any* call has static machine arguments.
Once entered, discovery scans *all* calls. Filtering the scan to generic calls
would miss concrete-return wrappers with no static arguments.

The existing `resolve_const_argument_selection` is not an adequate replacement.
It indexes declarations and runs `assign_lexical_inputs`; the latter lowers local
headers and type/index expressions while explicitly discarding runtime initializers.
It never selects the ordinary call-result occurrences needed here. Conversely,
completing every deferred machine body recreates the work we want to remove.

## Next decisive experiment

Extend the existing resolver-owned lexical-input construction to retain call
occurrences with their receiver/static-argument dependency and preceding-local
scope, using the existing call selection. Return one preparation-only selected
call-result product. Keep this private to preparation; it grants no body execution,
typing, placement admission or cache validity. Do not create an independent
name resolver in compile-time or select declarations by spelling.

First run the full resolver and candidate side by side in a diagnostic producer.
Compare selected declaration source identities, closed actual identities,
invocation spans, normalized placement applications, and admission diagnostics.
Fresh handles can differ between independent allocations: comparison must join
through their exact declaration/source identity, not ignore identity or compare
only names. Preserve multiplicity and order of invocation observations.

Start with the existing three ordinary workloads, returned and forwarded views,
and five rejection controls. Add the existing policy-owner-selection and
placed-view-call-custody fixtures after recording their current baseline behavior.
Receiver projection, same-spelling declarations, concrete-return wrappers, nested
return types and unresolved call-result receivers define the additional dependency
cases to inspect; they are not an arbitrary combinatorial matrix.

Reject the candidate on any lost use location, changed declaration/actual,
prototype or authority refusal, or native/Terminal disagreement. Measure rows
materialized and work actually skipped before making timing claims. If lexical
selection still demands full bodies or routing, record that result and retain the
current full route. The earlier inline-result fixture has a recorded lowering
failure; it must not be presented as a passing runtime witness.

## Research log

2026-10-22. New question relative to the earlier query-cache research: how do
existing compilers separate declaration resolution, body-local selection and source
mapping? Query: `site.rust-analyzer.github.io book architecture ItemTree body
lowering name resolution`. Read the [architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)
sections for `hir-def`/`hir-ty`, plus the [2024-01-01 guide](https://rust-analyzer.github.io/book/contributing/guide.html)
sections on name resolution, source maps and body inference. The guide is explicitly
historical. Their declaration summaries and body/source-map separation suggest
retaining semantic identity separately from body materialization. They do not
establish that Omega's ordinary calls can be selected from headers alone. No
Salsa/query cache or Rust-specific import rule is proposed. The local receiver
and routing dependencies above control the experiment.

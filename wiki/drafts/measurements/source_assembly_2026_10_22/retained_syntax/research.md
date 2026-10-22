# Retained syntax ownership research — 2026-10-22

Question: remove reference-count work for each parsed name and the decoded-token-to-syntax literal allocation while preserving independently copied syntax and generated spellings.

Queries: `site.github.com/rust-lang/rust compiler rustc_span source_map SourceMap source_file`; `site.github.com/oxc-project/oxc allocator arena ast lifetime`.

Read https://raw.githubusercontent.com/oxc-project/oxc/main/crates/oxc_allocator/README.md in full. Oxc groups AST allocations under one arena lifetime. Its borrowed AST model supports eliminating per-node destruction, but Omega retains and copies independently produced syntax pages, so transplanting that lifetime directly would change public ownership boundaries. The requested follow-up str.rs URL was 404 and supplies no evidence. No performance claim is imported from the README.

Reuse the existing source-working-set design (source-working-set-design-2026-10-21/DESIGN.md in the measurement archive). Its proposed source-owner/explicit-access-context boundary applies here. Inspection confirms contextless Identifier::as_str, Display, PartialEq, generic expansion clones and checkpoint cloning must be migrated consistently. A raw pointer or global text registry would hide the missing lifetime and is rejected. A span never substitutes for generated semantic spelling. No hashing experiment is proposed.

Acceptance: retain exact source and decoded storage at their real lifetime boundary; preserve logical spelling, raw byte literals, source spans, diagnostics, generated custody, clone/adoption behavior, serialized snapshots and source/native outcomes. Count retained references and allocation traffic, then alternate frozen optimized producers and report spread. Outcome: retain the implementation; ownership, output, allocation and timing evidence is in README.md.

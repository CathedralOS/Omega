# Bounded tentative scans — 2026-10-21

Question: can failed static-call lookahead stop before later declarations without changing the existing grammar or its ambiguity decisions?

Previous research: [probe storage](../probe_storage/research.md). This is a changed question: token work rather than scratch lifetime.

Query: `site.llvm.org clang parser tentative parsing lookahead TryParseSimpleDeclaration`.
Read [Clang ParseTentative.cpp](https://clang.llvm.org/doxygen/ParseTentative_8cpp_source.html), isCXXSimpleDeclaration (source lines 115–176) and TryParseSimpleDeclaration (246–272). Its early disambiguation and explicit incomplete-lookahead caveat reinforce the need to justify a stopping condition against the entire admitted grammar. C++ rules do not establish Omega's grammar.

Omega trace: static argument parsing reaches type references, const expressions, nested symbol applications, and grouped ordinary expressions. A single colon, assignment equals, or arrow is not an ungrouped operator or argument delimiter on these routes. Record field colons, match arrows, call-lane semicolons and array-extent semicolons occur inside the existing group boundary. `::`, `==`, `>=` and `>>` remain distinct tokens. Thus once one of these three tokens is reached at group depth zero, a later candidate prefix still includes it and cannot complete the pack.

Rejected: stopping at braces or equality/logical operators would require extra reasoning about match subjects and grouped expressions; arbitrary first-candidate rejection would change shift/closer ambiguity. Neither is implemented. No memo table, token cache, new arena region or second recognizer is introduced.

Experiment: retain the existing ordered candidate recognizer, extend its unconditional top-level terminator set, then compare complete syntax/error hashes on the corpus and measure instrumented work separately from timing. Reject on any output difference or unsupported grammar premise.

Publication-copy audit: `SyntaxTrees::new` uses generation 1; `PagedArenaSet::insert_page` requires an adopted generation strictly later than the retained first page and not already present. Whole-page adoption also retains every speculative row, unlike the existing returned-argument traversal. Moving the current scratch forest is therefore not a drop-in replacement for `transfer_static_machine_arguments`. A separate successful-pack transfer harness measures that unchanged operation with argument clones and destination construction outside the timed interval; its results do not estimate the cost of hypothetical generation allocation, garbage retention, or new page management.

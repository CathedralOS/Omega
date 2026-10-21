# Parser work-reduction research — 2026-10-21

Question: can a necessary token-prefix condition avoid constructing scratch syntax for a tentative type parse without replacing the ambiguous grammar?

Queries: `site.clang.llvm.org doxygen ParseTentative.cpp isCXXSimpleDeclaration lookahead`; `site.doc.rust-lang.org std hint black_box benchmarks`.

Read https://clang.llvm.org/doxygen/ParseTentative_8cpp_source.html, isCXXSimpleDeclaration lines 133–175: Clang classifies the declaration prefix before doing a reverting tentative parse. This supports conservative prefix dispatch as an implementation technique; C++ grammar and its classifications are not applicable Omega rules. Omega's parse_static_name and parse_type_reference_handle supply the necessary `<` or `::` condition for a name-led Generic node. Existing full parsing stays responsible for plausible applications and ambiguities.

Read https://doc.rust-lang.org/std/hint/fn.black_box.html: guard both inputs and outputs; the optimizer barrier is best effort, so inspect emitted ARM64 code too. The benchmark retains actual parser calls inside the timed loop and hashes complete syntax outside it.

Next experiment: retain the full applied-requirement probe after name prefixes followed by `<` or `::`; bypass it for the measured 1,456 impossible cases among 1,982 probes. Compare full syntax digests, allocations, bytes, stable repetition minima, averages and maxima, page faults, and real assembly timing. No new source search is needed until that comparison changes the question.

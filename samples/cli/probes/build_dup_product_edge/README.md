# build_dup_product_edge

Two `Build::dependency` registrations select one local dependency with
different aliases. The Product imports both package-root modules and adds
`k1::value()` and `k2::value()`. The dependency's result contract exposes 7
for both calls so the caller can prove the exact sum. The Build-side
`build_dup_build_edge` probe selects an occurrence for Build scope.

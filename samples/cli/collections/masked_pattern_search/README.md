# Masked pattern search

Search a borrowed byte snapshot using exact, wildcard and nibble-masked bytes.
Results carry the actual matched slice and its position, not copied bytes.
Searching one byte after a match finds overlapping windows. The authored
oracle checks three overlapping windows, a masked rejection, short/empty input,
an empty pattern, a start beyond the input, and untouched boundary sentinels.

```sh
omega update --project samples/cli/collections/masked_pattern_search --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/masked_pattern_search/main.omg
```

Expected exit: 0, no output. Empty patterns deliberately produce no match.
Native and Terminal Psi execution both return 0 on macOS arm64. The program
exercises cross-state returned-window custody and temporary view arguments to
structural-result calls; neither requires copying the matched bytes.

# Alias RHS declaration scope

Fact: A transparent alias keeps its declaration's constant separate from a same-named caller value while computing its array extent.

The imported declaration owns public constant `Bias=1`; the caller owns a
local `Bias=9` and `Count=17`. `Padded<3>` must remain the ordinary four-byte
array and retain its 17/42 values through explicit annotations and borrows.
The twin changes only 3 to maximum u64 and must reject Exact addition overflow.

Current-source observation is pending. This follows the qualified Const-formal
place; no alias wrapper, nominal identity or initializer inlining is permitted.

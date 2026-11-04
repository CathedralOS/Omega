# Call a selected implementation with fixed array lengths

The caller selects `Runner<u64>` and passes a nominal `Reader<u64>` callback plus two constant array lengths. Calls with values 37 and 91 must return those distinct values; `main` traps if either selected call is lowered incorrectly.

The compiler used to save the lengths as constant expressions and later compare them with normalized literals as types. It rejected the valid call as a stale provider tuple. Constant arguments must retain their exact value identity through requirement closure; types, callback declarations, ownership and lifetimes remain separate obligations.

The reject twin changes only the second array’s length from 4,096 to 4,095 while both calls still require 4,096. It must reject that argument mismatch. Existing nominal-constant and region controls separately retain exact callback and lifetime checks.

This is a prerequisite exposed while compiling Cathedral’s static interrupt installer. It demonstrates an ordinary generic callback call, not installed-root authority, acknowledgement construction, or timer delivery.

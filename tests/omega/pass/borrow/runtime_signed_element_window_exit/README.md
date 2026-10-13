Signed field endpoints retain i32 arithmetic before a checked exact conversion
into the u64 element-view coordinate. The mutable f64 window selects elements
[1,4); an ordinary helper subtracts 0.5, and both adjacent elements remain intact.
Canonical contracts: language/numeric_values.md collection bounds and literal
landing; terminal-psi/byte_views.md source-bound ranges; terminal-psi/loans.md
captured places and access. Expected native and Terminal exit 0.

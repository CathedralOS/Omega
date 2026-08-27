# Omega bootstrap checked IR v16

CKIR16 is the minimal direct, pure, same-carrier full-width `u64 Less` slice.
It preserves every CKIR15 row shape and opcode number. Major version 16 is the
only wire identity that enables the new carrier.

Type kind `8` denotes ordinary unqualified `u64`; its flags byte and reserved
word are zero. The four existing type words encode the inclusive interval as
`lower-low32`, `lower-high32`, `upper-low32`, `upper-high32`. The interval is
ordered as an unsigned 64-bit pair. This kind is CKIR-local; a later OMGRSW
kind number need not match it.

Opcode `1 Const` uses immediate 0 as the low half and immediate 1 as the high
half for kind 8. Opcode `9 Less` accepts two visible kind-8 values and returns
the canonical bool. The operation is pure, nontrapping, direct and same
carrier. At least one such selected row is required. Kind-8 `LessEqual`,
`ScalarEqual`, `Greater`, `GreaterEqual`, arithmetic and array indexing remain
outside CKIR16.

Kind 8 has size and alignment eight. It is a scalar through fields, loads,
stores, exact call parameters/results, block parameters/edges, returns,
record/case constructor fields and case payload bindings. Narrow destination
intervals are checked at every runtime custody boundary: store, callee
parameter receipt, edge commit, return, and constructor placement. Constants
are checked statically. Constant-DAG scalar nodes remain one-word and therefore
cannot carry kind 8.

CKIR15 generalized shared-byte-view control and CKIR14 complete arithmetic
families remain optional inherited families. A CKIR16 module containing no
view type or view operation does not need synthetic nonempty-edge blocks. If
the view family is selected, its complete inherited relation is enforced.

The true branch may target a narrower kind-8 parameter whose interval records
the fact established by direct `Less`; CKIR itself retains the constrained
target type and performs the edge range check. It does not serialize a second
proof-fact language. The resolver/lowerer’s directness and true-edge fact
construction are separate producer obligations.

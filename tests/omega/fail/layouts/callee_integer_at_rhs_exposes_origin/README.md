An earlier RHS operand passes a captured reference carrier to an opaque mutable
callee. The assignment's final value is provably 7, which fits the signed-byte
layout, but that does not preserve the captured destination slot. The compiler
must reject missing destination provenance rather than reusing the old loan.

# Fixed-width SIMD values

A SIMD value contains a fixed number of scalar elements called lanes. Each lane has the same element type and its own value. The lane count and element type are part of the type. The first element in memory corresponds to lane zero. A lane operation has the same observable result in Psi and native execution.

## Integer operations

Wrapping addition and subtraction apply Omega's wrapping arithmetic independently at the element width. For example, adding one to a signed 16-bit lane containing 32767 produces −32768 in that lane. Every other lane uses its own two operands. An arithmetic right shift by a constant preserves the sign of each lane; for example, −3 shifted right by one produces −2. The count must be smaller than the element width for these immediate shifts.

A fixed shuffle selects result lanes from its inputs by constant index. Each index must identify a lane in the specified inputs. Widening an unsigned byte to a signed 16-bit lane preserves its value. Saturating narrowing from signed 16-bit values to unsigned bytes maps values below zero to zero, values above 255 to 255, and every other value to the corresponding byte. A constant vector supplies its declared value at each lane.

## Memory and realization

A load borrows the elements it reads. A store requires an exclusive borrow covering every element it writes. Each operation accesses exactly its declared extent, including at the edge of a buffer. Four pixel bytes may be read from a four-byte row or stored into it independently of the size of a physical vector register. Use of an aligned instruction requires the corresponding proven alignment. Lane operations alone require no special memory alignment.

The source compiler checks types, arithmetic policies, lane indexes and access bounds. Psi preserves lane order and element semantics. The native compiler chooses target instructions under the selected deployment features and obeys the target's register preservation rules. A sequence of dependent operations can share a physical vector register. Library machines compose these operations into such algorithms as a small matrix transpose. The codec owns the transform coefficients, rounding, pixel locations and output contract.

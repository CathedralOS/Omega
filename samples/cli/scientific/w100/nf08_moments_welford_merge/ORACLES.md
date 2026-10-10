# Frozen NF08 mathematical oracles

Frozen before Omega execution. Exact rational reference calculation is independent
of Welford: mean = sum(x)/n; M2 = sum((x-mean)^2).

Streaming observations: [2,4,4,4,5,5,7,9]. Count 8; mean 5; M2 32;
population variance 4; sample variance 32/7. Absolute tolerance 1e-12.
Singleton [42]: count 1, mean 42, M2 0, population variance 0 exactly,
sample variance InsufficientSample. Empty mean/population return Empty;
empty sample returns InsufficientSample.

Uneven chunks: [1000000001,1000000002], [1000000003],
[1000000004,1000000005]. Count 5; mean 1000000003; M2 10;
population variance 2; sample variance 5/2. Absolute tolerance 1e-6
for all floating outputs; count exact. Both merge orders ((A+B)+C) and
((B+C)+A) and the sequential stream must satisfy this fixed oracle.
An empty chunk is an identity. Source chunk summaries remain unchanged.
The large offset makes raw E[x^2]-E[x]^2 cancellation-prone.

These local acceptance tolerances are not NF02's shared accuracy policy.
No post-run tolerance adjustment is permitted.

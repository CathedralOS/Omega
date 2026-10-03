# Guarded terminal boundary requirement calls

Both guarded arms invoke an ordinary selected boundary requirement and return
its scalar result to the caller. The program exercises the selected and fallback
arms, preserving their distinct operands, and exits 70 only if both results match.

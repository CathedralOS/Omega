# A later copy cannot recover an overwritten IEEE value

The source is written after its Boolean range guard completes and before the
copy. The copy receives the new out-of-range value; the earlier saved truth
cannot justify its unqualified conversion. This pins the write boundary in the
same conditional observation transport used by the runtime positive.

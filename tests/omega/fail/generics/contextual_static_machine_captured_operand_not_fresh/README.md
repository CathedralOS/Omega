# Captured operands do not bind a callback's fresh lifetime

The caller-owned Holder carries a captured region. A provider whose Holder
uses its fresh invocation region cannot refine this contract, even when
both binders have the same spelling. The paired fixture alpha-renames only
the required fresh binder; the judgment must remain unchanged.

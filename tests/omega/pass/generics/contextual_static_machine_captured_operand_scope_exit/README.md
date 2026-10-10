# Captured operand and fresh callback region remain separate

A caller-owned Holder and the callback's fresh invocation lifetime use the
same spelling. Simultaneous type substitution keeps Holder in its captured
caller scope; the callback reads the independently fresh item. Native and
Terminal execution must both select the 42-valued item, not the 17-valued
carried reference.

//! Shared admission for the in-block commuting run interchange: locate the
//! four named members bounding two disjoint runs inside one block's body —
//! the earlier run's first and last and the later run's first and last —
//! and prove the window they bound independent: no register or
//! condition-state hazard between any member and any instruction outside
//! its own run inside the window, every roster row that newly trades order
//! commuting with every row of the position it crosses, no call,
//! hosted-effect, or terminator barrier anywhere in the window, and no
//! boundary settlement inside its span.

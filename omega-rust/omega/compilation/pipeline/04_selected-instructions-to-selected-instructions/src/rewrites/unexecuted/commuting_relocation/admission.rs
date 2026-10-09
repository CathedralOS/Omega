//! Shared admission for the in-block commuting run relocation: locate the
//! named `first_member` and `last_member` bounding one contiguous run
//! inside a block's body, locate the named destination instruction outside
//! the run in the same block, and prove the window they bound independent —
//! no register or condition-state hazard between any member and any crossed
//! position, every roster row that newly trades order commuting with every
//! row of the position it crosses, no call, hosted-effect, or terminator
//! barrier anywhere in the window, and no boundary settlement inside its
//! span.

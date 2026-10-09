//! Keyed function reconstruction for independent replay.
// Mirrors the compute admission: early-clobber participants partition
// identically because no `early_clobber_points` row enters a source domain;
// the hazard is enforced on the reanalyzed plan. Only the use count feeds
// work accounting.

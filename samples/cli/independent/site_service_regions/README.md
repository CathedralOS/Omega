# Bounded site service regions (ADD12)

This complete Omega console application constructs the clipped Euclidean Voronoi
partition of a supplied strictly convex counterclockwise boundary. Each site owns
its nearest-point cell. It emits cell vertices in boundary order, cell areas,
positive-length shared-edge adjacency, nearest-site checks and the partition area.
All numeric algorithms and storage are private to this application. Its only
product dependency is the repository's existing standard console provider.

Input is whitespace-separated signed decimal integers: boundary vertex count
(3–12), that many `x y` pairs, site count (1–8), then site `x y` pairs. Coordinates
must be in [-100,100]. Sites must be distinct and inside or on the boundary.
Malformed, truncated, extra, nonconvex, clockwise, collinear and out-of-range
inputs print `INVALID_INPUT` and exit 64. Successful output exits 0. A capacity or
geometry consistency failure prints `PARTITION_FAILED` and exits 70. There is no
interactive pause and EOF ends the supplied input.

Clipping uses ordinary IEEE f64 arithmetic and the exact closed predicate
`a*x+b*y <= c`. A convex 12-edge polygon clipped by seven lines has at most 19
vertices; invocation-private capacity is 32 vertices per cell and eight cells.
Coincident clipping endpoints may repeat in the output. Adjacency ignores zero
length edges and excludes self relationships. Ties belong to both cell closures;
a vertex-only contact is not adjacency. No runtime geometry is replaced with
fixture constants. Internal distance-squared and area-partition tolerances are
1e-6; positive edge length-squared must exceed 1e-12. Output integers represent
micro-units (1,000,000 per coordinate or area unit), rounded away from zero at
half units. Quantization affects presentation only.

## Independent expected results

`data/expected.json` was frozen before the first Omega execution, SHA-256
`d7f9565bd692122304bbb18814334fccbe560ba1e019047daa78b03eda82e6b2`.
`reference.py` independently enumerates feasible pairwise line intersections with
exact rational arithmetic, orders their hull, and computes rational shoelace
areas. It does not implement the application's clipping algorithm.
`validate.py` verifies that identity, fixture hashes, vertex sets, signed areas,
exact adjacency, symmetry, vertex/edge-midpoint/centroid distance comparisons,
boundary containment and total partition area. The frozen output tolerances are
2e-6 coordinates, 4e-6 cell areas, 2e-5 squared distances and 2e-5 partition area.
Do not regenerate the frozen oracle in a validation run.

Eight variants include the exact vertical bisector (two 50-unit cells), diagonal
bisector (two 50-unit triangles), four quadrants (four 25-unit cells with only
edge neighbors), triangular boundary (25, 23.5, 23.5), nonrectangular translated
pentagon, uneven collinear sites, a single site and eight sites including four
boundary corners. Fourteen runtime invalid controls exercise distinct input and
geometry failures. `invalid/main.omg` is a separate complete source program
whose negative constrained quota must be rejected by the compiler.

## Reproduction

From repository root, set `OMEGA_LIBRARY_ROOT` to this checkout's
`source/library` and use a compiler built from its recorded base. First prepare
and review the existing std package through the ordinary documented workflow:

```text
omega update --project samples/cli/independent/site_service_regions --target linux_x86_64
omega update --resume --project samples/cli/independent/site_service_regions
omega --check --target linux_x86_64 samples/cli/independent/site_service_regions/main.omg
omega --target linux_x86_64 samples/cli/independent/site_service_regions/main.omg
omega run --both samples/cli/independent/site_service_regions/main.omg < samples/cli/independent/site_service_regions/data/split.txt
omega --check samples/cli/independent/site_service_regions/invalid/main.omg
```

Only resume after reviewing and accepting the precise pending package decisions.
`exercise.py --omega /absolute/compiler/path` owns the sequential checks,
retained native output, all valid/invalid runtime fixtures and Terminal comparison.
Generated binaries, outputs, diagnostic logs and run receipts go in ignored
`build/`. Terminal `AGREEMENT` must be observed explicitly; a `DECLINED` is
reported as an unsupported comparison, never accepted as Terminal validation.

## Semantic overlap review

At fetched main `eac95f5bc8ef4123fd7815bafe188df2a6f71a07`, `samples/` has
scalar `vector_distance`, `stats_compute`, `particle_sim` and `grid_walk` workflows
but no Voronoi, polygon clipping, nearest-neighbor prediction or operating-envelope
sample. The assigned earlier-wave nearest-neighbor and admitted-envelope work
returns a query result or one feasible region. ADD12 constructs the entire spatial
partition with all per-site polygons, global area accounting and adjacency.
It neither imports those candidates nor relies on the in-progress numerics library.

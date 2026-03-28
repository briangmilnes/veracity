# veracity-type-subst: N→usize and B→bool substitution plan

## Validated

| Chap | N→usize | B→bool | Total | Status |
|------|---------|--------|-------|--------|
| 21   | 128     | 12     | 140   | validated (isolate Chap21, 1207 verified, 0 errors) |

## To do

| Chap | N→usize | B→bool | Total | Notes |
|------|---------|--------|-------|-------|
| 05   | 12      | 12     | 24    | Foundation: Set, Mapping, Relation |
| 06   | 40      | 20     | 60    | Foundation: DirGraph, UnDirGraph, LabDirGraph, LabUnDirGraph |
| 26   | 310     | 48     | 358   | DivConReduce, MergeSort, ScanDC, ETSP |
| 37   | 134     | 62     | 196   | AVLTreeSeq, BST variants — compare-par-mut flagged N/B here |
| 38   | 0       | 9      | 9     | BSTPara |
| 41   | 0       | 16     | 16    | AVLTreeSet, ArraySet |
| 42   | 0       | 6      | 6     | Table |
| 43   | 0       | 36     | 36    | OrderedSet, OrderedTable, AugOrderedTable |
| 52   | 288     | 38     | 326   | AdjMatrixGraph, AdjSeqGraph, AdjTableGraph, EdgeSetGraph |
| 53   | 0       | 9      | 9     | GraphSearch, PQMin |
| 54   | 247     | 0      | 247   | BFS |
| 55   | 149     | 19     | 168   | DFS, TopoSort, CycleDetect, SCC — memory pressure |
| 61   | 2       | 3      | 5     | EdgeContraction, VertexMatching |
| 64   | 4       | 4      | 8     | SpanTree, TSPApprox |
| 65   | 1       | 4      | 5     | Kruskal, Prim, UnionFind |

## Totals

- **16 chapters** need substitutions
- **1315 N→usize** substitutions
- **298 B→bool** substitutions
- **1613 total** substitutions

## Execution

### Dry run (preview changes without modifying files)

```bash
~/projects/veracity/target/release/veracity-type-subst usize N -c -n
~/projects/veracity/target/release/veracity-type-subst bool B -c -n
```

### Apply and validate (full crate)

```bash
cd ~/projects/APAS-VERUS
~/projects/veracity/target/release/veracity-type-subst usize N -c
~/projects/veracity/target/release/veracity-type-subst bool B -c
scripts/validate.sh
ls -t logs/validate.*.log | head -1 | xargs cat
```

### Notes

- The tool skips `N`/`B` when they are generic type parameters (`fn foo<N>(x: N)`).
  Only concrete type-alias usages are substituted.

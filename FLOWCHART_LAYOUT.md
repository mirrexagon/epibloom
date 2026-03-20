# Chat Flowchart Layout — Issue & Options

## Problem

The flowchart (`epibloom/ui/src/components/ChatFlowchart.tsx`) places branch nodes
far to the sides of their parent in long, mostly-linear chats.

### Root cause

The current algorithm assigns **global, non-overlapping column slots** to every
subtree. A branch leaf gets 1 slot; the continuing trunk gets N slots. Those slots
are placed sequentially starting from the *left edge of the parent's total span*.

At every user node that has a branch, the branch takes one column from the left and
pushes the continuing trunk one unit to the right. After many such branch points the
trunk has drifted far to the right and the discarded branches are scattered to the
left. There is also a secondary effect: parent nodes are *centred over all their
children*, so even the trunk node itself drifts leftward toward its branch, creating
a diagonal staircase pattern.

The symptoms match exactly: in a long chat with each user message having one
discarded branch, those branch nodes are spread far to the left at increasing
distances from their parent, rather than sitting right next to it.

---

## Options

### Option 1 — Add `d3-hierarchy` and replace the layout function

[`d3-hierarchy`](https://github.com/d3/d3-hierarchy) (~20 KB, no transitive
dependencies) includes a proper Reingold–Tilford tree layout (`d3.tree()`). It is
battle-tested and produces compact trees where sibling subtrees are positioned as
tightly as possible without overlapping. Every identical subtree shape gets the same
relative layout regardless of where it appears in the overall tree.

**What changes:** install `d3-hierarchy`, rewrite only `computeLayout()` (~80 lines
→ ~30 lines). All SVG rendering, pan/zoom, viewport culling, `centerOnNode`, etc.
stay identical.

**Pros:** minimal change to the codebase, correct algorithm, well-known library.  
**Cons:** small new dependency.

---

### Option 2 — Rewrite `computeLayout` with the Buchheim/Walker algorithm (no new deps)

Walker's algorithm (refined by Buchheim et al.) is what `d3-hierarchy` implements
under the hood. It is implementable in ~100 lines of TypeScript. The key improvement
over the current code: it tracks the *contour* of each subtree so siblings can be
packed tightly together, with a separate shift-accumulation pass to keep the
algorithm O(n) rather than O(n²).

**What changes:** replace `computeLayout()` only. Everything else stays.

**Pros:** zero new dependencies.  
**Cons:** subtle to implement correctly — small mistakes produce hard-to-spot
asymmetries or overlaps.

---

### Option 3 — Replace the canvas with React Flow + dagre

[React Flow (xyflow)](https://reactflow.dev/) is a full React node-graph component
with pan, zoom, selection, minimap, and animated edge routing all built in.
[@dagrejs/dagre](https://github.com/dagrejs/dagre) provides proper hierarchical
(Sugiyama-style) layout on top of it.

**What changes:** remove the custom SVG canvas and all pan/zoom/culling code, install
`@xyflow/react` + `@dagrejs/dagre`, rewrite the component to use React Flow
nodes/edges with dagre layout.

**Pros:** professional out-of-the-box result, much less layout and interaction code
to maintain, many free features (minimap, edge routing, etc.).  
**Cons:** large new dependency (~300 KB), substantial rewrite of the component, some
current fine-grained control (custom culling, exact node appearance) would need to be
re-expressed through React Flow's API.

---

## Recommendation

**Option 1** is the lowest-risk, smallest-change fix. The layout algorithm is the
only broken part, and `d3-hierarchy` is the standard tool for exactly this problem.

Option 3 is appealing for the long term if a richer graph UI (minimap, animated
edges, etc.) is wanted, but it is a larger investment.

Option 2 is viable but more error-prone to implement manually when a correct
reference implementation already exists as a small library.

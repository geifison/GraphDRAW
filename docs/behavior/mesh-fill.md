# Mesh fill

A grid of nodes over an object, each node with a colour and an opacity;
the colours flow between the nodes and the object's outline clips them.

## Mesh Fill tool

- A click on an object gives it a mesh fill of the bar's rows and
  columns, spread over its bounds, every node in the colour its fill
  had (white when it had none); a click on an object that has a mesh
  selects it. A click on nothing clears the selection.
- A click on a node selects it; dragging a node moves it (one undo step
  for the drag).
- With a node selected, a click on a colour in the colour palette
  colours that node (the fill stays a mesh); with the Pick tool the same
  click fills the whole object as usual.
- Double-click inside the mesh adds a grid line through the nearest node
  (a column or a row, whichever is nearer), its nodes mixed from their
  neighbours.
- Delete with a node selected removes the grid lines through it that
  are not on the edge (its column, its row or both); a node on the edge
  keeps its lines and the status bar says so.
- The Shape tool drags and colours the nodes the same way.
- The grid is drawn over the selected object; the selection handles are
  not.

## Property bar

| Control | Default | Range |
|---|---|---|
| Rows, Columns | 2 x 2 (the selected mesh's when there is one) | 1 to 50 each |
| Smooth colour | off | |
| Transparency (the selected node) | 0 % | 0 to 100 % |
| Clear Mesh | | the selected objects lose their mesh fill (no fill) |

New meshes take the rows and columns, and a selected mesh changes to
them keeping its colours: the new nodes are spread evenly over the old
nodes' bounds and each takes the colour and opacity the old grid has
there (mixed from the four old nodes around it). The Properties panel's
fill page changes the grid the same way. The rows and columns are kept
in the preferences (`tools.mesh_rows`, `tools.mesh_cols`).

## Drawing

Each cell is cut into small quads (`k x k`, `k` from the cell's size on
screen: one per 6 pixels of its diagonal, 2 to 24), each painted with
the colour at its middle:

- Without Smooth colour, the mix of the cell's four nodes (bilinear).
- With Smooth colour, the Catmull-Rom mix of the 4 x 4 nodes around the
  cell (rows and columns past the edge repeat the edge), in RGB, so
  colours flow through the nodes without creases.

## Checks

- `graphdraw_core::style::tests::meshes_regrid_and_lose_lines_keeping_their_colours`
- `graphdraw_render::tests::mesh_fill_blends_corner_colours`
- `graphdraw_render::tests::smooth_mesh_colour_flows_through_the_nodes`
- `mesh_tool::tests::the_mesh_tool_builds_colours_and_clears_meshes`
- `ui::fill_tool_bars::tests::the_fill_tool_bars_draw`

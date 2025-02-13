# Dot graph linker
A utility to link dot graphs and make some transformations on them

# Dot format
Graph nodes have a name, which is the same as node id in dot graph

# Arguments
List of all dot files is written in file `dots`

Config is written in file `config`

To store result in a specific file, output file should be passed as `-s` argument

To link all graphs into one, use `-l` or `--link` argument

# Config
Config is a file with the list of modifications(passes) to be applied to the graph.

_Note: if `link` is provided, all graphs are linked before any modifications_

Currently supported operations:
- `term_nodes file` - remove all nodes with names listed in `file`
- `regex_edge_gen file` - create edges by provided rules
  - `"regex" -> name`: create nodes from every matching node to v
  - `"regex" <- name`: create nodes from v to every matching node
- `cut_deg (+deg_in) (-deg_out)`: filter nodes that have no more than `deg_out` outgoing edges and `deg_in` incoming ones
- `unique_edges` - deduplicate edges
- `extract_subgraph file` - leave only listed in file nodes
- `reverse` - reverse edges
- `reparent file` - reparent all nodes listed in file. If a node `s` is in file, all chains `v -> s -> u` create edge `v -> u`

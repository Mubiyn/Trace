import type { GraphNode, Subgraph } from "./types";

/** Map legacy / serde variants to canonical node kinds. */
export function normalizeNodeKind(kind: string): GraphNode["kind"] {
  if (kind === "uielement") {
    return "ui_element";
  }
  return kind as GraphNode["kind"];
}

export function normalizeSubgraph(subgraph: Subgraph): Subgraph {
  return {
    ...subgraph,
    nodes: subgraph.nodes.map((node) => ({
      ...node,
      kind: normalizeNodeKind(node.kind),
    })),
  };
}

import type { EdgeKind, GraphEdge, GraphNode, Subgraph, ViewMode } from "./types";

const SYMBOL_KINDS = new Set<GraphNode["kind"]>([
  "function",
  "class",
  "import",
  "ui_element",
  "route",
  "branch",
]);

const STRUCTURE_KINDS = new Set<GraphNode["kind"]>(["directory", "file"]);

const BROAD_EDGES = new Set<EdgeKind>(["Contains"]);
const ISOLATION_EDGES = new Set<EdgeKind>([
  "Calls",
  "Imports",
  "Triggers",
  "Handles",
  "Fetches",
  "BranchesTo",
]);

export function filterSubgraphForMode(
  subgraph: Subgraph,
  mode: ViewMode,
): Subgraph {
  if (mode === "tree") {
    return filterSubgraphForMode(subgraph, "isolation");
  }

  if (mode === "broad") {
    const nodes = subgraph.nodes.filter((node) => STRUCTURE_KINDS.has(node.kind));
    const nodeIds = new Set(nodes.map((node) => node.id));
    return {
      nodes,
      edges: subgraph.edges.filter(
        (edge) =>
          BROAD_EDGES.has(edge.kind) &&
          nodeIds.has(edge.from_id) &&
          nodeIds.has(edge.to_id),
      ),
      ghosts: [],
    };
  }

  const nodes = subgraph.nodes.filter((node) => SYMBOL_KINDS.has(node.kind));
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = subgraph.edges.filter(
    (edge) =>
      ISOLATION_EDGES.has(edge.kind) &&
      nodeIds.has(edge.from_id) &&
      nodeIds.has(edge.to_id),
  );

  return {
    nodes,
    edges,
    ghosts: subgraph.ghosts.filter((ghost) =>
      nodeIds.has(ghost.via.from_id) || nodeIds.has(ghost.via.to_id),
    ),
  };
}

export function nodeFilePath(node: GraphNode): string {
  return node.parent_file ?? node.relative_path;
}

export function cytoscapeStyleForKind(kind: GraphNode["kind"]): {
  color: string;
  shape: string;
} {
  switch (kind) {
    case "directory":
      return { color: "#64748b", shape: "round-rectangle" };
    case "file":
      return { color: "#0ea5e9", shape: "round-rectangle" };
    case "function":
      return { color: "#22c55e", shape: "ellipse" };
    case "class":
      return { color: "#a855f7", shape: "diamond" };
    case "import":
      return { color: "#f59e0b", shape: "triangle" };
    case "ui_element":
      return { color: "#ec4899", shape: "round-rectangle" };
    case "route":
      return { color: "#14b8a6", shape: "hexagon" };
    case "branch":
      return { color: "#eab308", shape: "diamond" };
    default:
      return { color: "#94a3b8", shape: "ellipse" };
  }
}

/** Entry nodes for top-down decision-tree layout (UI roots, routes, branchless functions). */
export function pickLayoutRoots(subgraph: Subgraph): string[] {
  const incoming = new Set(subgraph.edges.map((edge) => edge.to_id));
  const preferred = new Set<GraphNode["kind"]>([
    "ui_element",
    "route",
    "branch",
    "function",
  ]);

  const roots = subgraph.nodes
    .filter((node) => preferred.has(node.kind) && !incoming.has(node.id))
    .sort((a, b) => a.name.localeCompare(b.name));

  if (roots.length > 0) {
    return roots.map((node) => node.id);
  }

  return subgraph.nodes
    .filter((node) => !incoming.has(node.id))
    .map((node) => node.id)
    .slice(0, 4);
}

export function toCytoscapeElements(subgraph: Subgraph) {
  const elements: Array<{
    group: "nodes" | "edges";
    data: Record<string, unknown>;
  }> = [];

  const overlay = subgraph.overlay;
  const observedNodes = new Set(overlay?.observedNodeIds ?? []);
  const observedEdges = new Set(overlay?.observedEdgeKeys ?? []);
  const nodeHits = overlay?.nodeHits ?? {};

  for (const node of subgraph.nodes) {
    const style = cytoscapeStyleForKind(node.kind);
    const hits = nodeHits[node.id];
    const observed = observedNodes.has(node.id);
    const label =
      hits !== undefined ? `${node.name} (${hits})` : node.name;
    elements.push({
      group: "nodes",
      data: {
        id: node.id,
        label,
        kind: node.kind,
        path: nodeFilePath(node),
        line: node.line ?? null,
        color: observed ? "#f97316" : style.color,
        shape: style.shape,
        observed,
        hits: hits ?? null,
      },
    });
  }

  for (const edge of subgraph.edges) {
    const observed = observedEdges.has(edge.id);
    elements.push({
      group: "edges",
      data: {
        id: edge.id,
        source: edge.from_id,
        target: edge.to_id,
        kind: edge.kind,
        label: edge.kind,
        observed,
      },
    });
  }

  for (const ghost of subgraph.ghosts) {
    elements.push({
      group: "nodes",
      data: {
        id: ghost.id,
        label: ghost.name,
        kind: "ghost",
        path: ghost.name,
        line: null,
        color: "#f97316",
        shape: "hexagon",
        ghost: true,
        direction: ghost.direction,
      },
    });
    elements.push({
      group: "edges",
      data: {
        id: `ghost-edge:${ghost.id}`,
        source:
          ghost.direction === "ingress" ? ghost.id : ghost.via.from_id,
        target:
          ghost.direction === "ingress" ? ghost.via.to_id : ghost.id,
        kind: "boundary",
        label: ghost.direction,
      },
    });
  }

  return elements;
}

export function buildSyntheticSubgraph(count: number): Subgraph {
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  nodes.push({
    id: "dir:.",
    kind: "directory",
    name: ".",
    relative_path: ".",
    language_id: "directory",
  });

  for (let i = 0; i < count; i++) {
    const file = `file_${i}.py`;
    const fileId = `file:${file}`;
    nodes.push({
      id: fileId,
      kind: "file",
      name: file,
      relative_path: file,
      language_id: "python",
    });
    edges.push({
      id: `Contains:dir:.:${fileId}`,
      from_id: "dir:.",
      to_id: fileId,
      kind: "Contains",
    });

    const symId = `sym:${file}:function:fn_${i}:1`;
    nodes.push({
      id: symId,
      kind: "function",
      name: `fn_${i}`,
      relative_path: file,
      parent_file: file,
      line: 1,
      language_id: "python",
    });
    edges.push({
      id: `Contains:${fileId}:${symId}`,
      from_id: fileId,
      to_id: symId,
      kind: "Contains",
    });

    if (i > 0) {
      const prev = `sym:file_${i - 1}.py:function:fn_${i - 1}:1`;
      edges.push({
        id: `Calls:${prev}:${symId}`,
        from_id: prev,
        to_id: symId,
        kind: "Calls",
        confidence: 0.9,
      });
    }
  }

  return { nodes, edges, ghosts: [] };
}

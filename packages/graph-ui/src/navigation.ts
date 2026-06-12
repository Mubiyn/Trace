import type { GraphNode, Subgraph, ViewMode } from "./types";
import { nodeFilePath } from "./modes";

export interface NavFrame {
  scope: string;
  focusId: string | null;
  selectedId: string | null;
  mode: ViewMode;
  label: string;
}

export function rootNavFrame(scope = ".", mode: ViewMode = "isolation"): NavFrame {
  return {
    scope,
    focusId: null,
    selectedId: null,
    mode,
    label: scope === "." ? "repository" : scope,
  };
}

const STRUCTURE_KINDS = new Set<GraphNode["kind"]>(["directory", "file"]);

const DRILL_EDGES = new Set([
  "Contains",
  "Calls",
  "Imports",
  "Triggers",
  "Handles",
  "Fetches",
  "BranchesTo",
]);

/** Focused neighborhood around a node for folder-style drill-down. */
export function neighborhoodSubgraph(
  subgraph: Subgraph,
  focusId: string,
  depth = 2,
): Subgraph {
  if (!subgraph.nodes.some((node) => node.id === focusId)) {
    return subgraph;
  }

  const included = new Set<string>([focusId]);
  let frontier = [focusId];

  for (let level = 0; level < depth; level += 1) {
    const next: string[] = [];
    for (const edge of subgraph.edges) {
      if (!DRILL_EDGES.has(edge.kind)) {
        continue;
      }
      for (const id of frontier) {
        if (edge.from_id === id && !included.has(edge.to_id)) {
          included.add(edge.to_id);
          next.push(edge.to_id);
        }
        if (edge.to_id === id && !included.has(edge.from_id)) {
          included.add(edge.from_id);
          next.push(edge.from_id);
        }
      }
    }
    frontier = next;
    if (frontier.length === 0) {
      break;
    }
  }

  const focusNode = subgraph.nodes.find((node) => node.id === focusId);
  if (focusNode && STRUCTURE_KINDS.has(focusNode.kind)) {
    for (const edge of subgraph.edges) {
      if (edge.kind === "Contains" && edge.from_id === focusId) {
        included.add(edge.to_id);
      }
    }
  }

  const nodes = subgraph.nodes.filter((node) => included.has(node.id));
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = subgraph.edges.filter(
    (edge) => nodeIds.has(edge.from_id) && nodeIds.has(edge.to_id),
  );

  return {
    nodes,
    edges,
    ghosts: subgraph.ghosts.filter(
      (ghost) =>
        nodeIds.has(ghost.via.from_id) || nodeIds.has(ghost.via.to_id),
    ),
  };
}

export function drillTargetScope(node: GraphNode): string | null {
  if (node.kind === "directory" || node.kind === "file") {
    return nodeFilePath(node);
  }
  return null;
}

export function frameLabelForNode(node: GraphNode): string {
  return node.name;
}

import type { Core } from "cytoscape";

type CyNodeCollection = ReturnType<Core["nodes"]>;
import type { EdgeKind, Subgraph } from "./types";
import { pickLayoutRoots } from "./modes";

const TRIM_EDGE_KINDS = new Set<EdgeKind>([
  "Calls",
  "Imports",
  "Triggers",
  "Handles",
  "Fetches",
  "BranchesTo",
]);

/** Node count above which we focus on entry points instead of rendering everything. */
export const LARGE_GRAPH_NODE_THRESHOLD = 120;

/** Minimum zoom so nodes stay readable after auto-fit. */
export const MIN_READABLE_ZOOM = 0.35;

/** Maximum nodes to render when the graph is large. */
export const LARGE_GRAPH_FOCUS_LIMIT = 80;

/** Maximum root seeds when expanding a large graph neighborhood. */
export const LARGE_GRAPH_ROOT_SEED_LIMIT = 4;

export interface DisplaySubgraph {
  subgraph: Subgraph;
  trimmed: boolean;
  totalNodes: number;
}

export function trimSubgraphForDisplay(
  subgraph: Subgraph,
  maxNodes = LARGE_GRAPH_FOCUS_LIMIT,
): DisplaySubgraph {
  if (subgraph.nodes.length <= LARGE_GRAPH_NODE_THRESHOLD) {
    return { subgraph, trimmed: false, totalNodes: subgraph.nodes.length };
  }

  const roots = pickLayoutRoots(subgraph).slice(0, LARGE_GRAPH_ROOT_SEED_LIMIT);
  const fallbackRoots = subgraph.nodes
    .filter((node) => !subgraph.edges.some((edge) => edge.to_id === node.id))
    .map((node) => node.id)
    .slice(0, LARGE_GRAPH_ROOT_SEED_LIMIT);
  const seeds = roots.length > 0 ? roots : fallbackRoots;
  const expandableEdges = subgraph.edges.filter((edge) =>
    TRIM_EDGE_KINDS.has(edge.kind),
  );

  const included = new Set<string>();
  const queue: string[] = [];

  for (const id of seeds) {
    if (!included.has(id)) {
      included.add(id);
      queue.push(id);
    }
  }

  while (queue.length > 0 && included.size < maxNodes) {
    const id = queue.shift()!;
    for (const edge of expandableEdges) {
      const neighbors: string[] = [];
      if (edge.from_id === id) {
        neighbors.push(edge.to_id);
      }
      if (edge.to_id === id) {
        neighbors.push(edge.from_id);
      }
      for (const next of neighbors) {
        if (!included.has(next)) {
          included.add(next);
          queue.push(next);
          if (included.size >= maxNodes) {
            queue.length = 0;
            break;
          }
        }
      }
      if (included.size >= maxNodes) {
        break;
      }
    }
  }

  const nodes = subgraph.nodes
    .filter((node) => included.has(node.id))
    .slice(0, maxNodes);
  const nodeIds = new Set(nodes.map((node) => node.id));
  const edges = subgraph.edges.filter(
    (edge) => nodeIds.has(edge.from_id) && nodeIds.has(edge.to_id),
  );

  return {
    subgraph: {
      nodes,
      edges,
      ghosts: subgraph.ghosts.filter(
        (ghost) =>
          nodeIds.has(ghost.via.from_id) || nodeIds.has(ghost.via.to_id),
      ),
    },
    trimmed: true,
    totalNodes: subgraph.nodes.length,
  };
}

export function focusElementsForLargeGraph(
  cy: Core,
  subgraph: Subgraph,
): CyNodeCollection {
  const roots = pickLayoutRoots(subgraph);
  const rootNodes = cy.nodes().filter((node) => roots.includes(node.id()));

  if (rootNodes.length > 0) {
    const neighborhood = rootNodes
      .closedNeighborhood()
      .nodes()
      .slice(0, LARGE_GRAPH_FOCUS_LIMIT);
    return neighborhood.length > 0 ? neighborhood : rootNodes;
  }

  return cy.nodes().slice(0, Math.min(LARGE_GRAPH_FOCUS_LIMIT, cy.nodes().length));
}

export function applyGraphViewport(
  cy: Core,
  subgraph: Subgraph,
  padding = 40,
): void {
  const nodeCount = cy.nodes().length;
  if (nodeCount === 0) {
    return;
  }

  cy.fit(undefined, padding);

  if (nodeCount > LARGE_GRAPH_NODE_THRESHOLD && cy.zoom() < MIN_READABLE_ZOOM) {
    cy.zoom(MIN_READABLE_ZOOM);
    const focus = focusElementsForLargeGraph(cy, subgraph);
    if (focus.length > 0) {
      cy.center(focus);
    } else {
      cy.center();
    }
  }
}

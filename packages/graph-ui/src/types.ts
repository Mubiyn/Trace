export type NodeKind =
  | "directory"
  | "file"
  | "function"
  | "class"
  | "import"
  | "ui_element"
  | "route"
  | "branch";

export type EdgeKind =
  | "Contains"
  | "Imports"
  | "Calls"
  | "Triggers"
  | "Handles"
  | "Fetches"
  | "BranchesTo";

export interface TraceHop {
  node: GraphNode;
  via?: GraphEdge | null;
  siblings: GraphNode[];
}

export interface TraceResult {
  hops: TraceHop[];
}

export interface GraphNode {
  id: string;
  kind: NodeKind;
  name: string;
  relative_path: string;
  parent_file?: string | null;
  line?: number | null;
  extension?: string | null;
  language_id: string;
  size_bytes?: number | null;
}

export interface GraphEdge {
  id: string;
  from_id: string;
  to_id: string;
  kind: EdgeKind;
  confidence?: number | null;
}

export interface GhostNode {
  id: string;
  name: string;
  direction: "ingress" | "egress";
  via: GraphEdge;
}

export interface ObservedPath {
  label?: string | null;
  nodeIds: string[];
  hits?: number;
}

export interface SubgraphOverlay {
  nodeHits: Record<string, number>;
  paths: ObservedPath[];
  observedNodeIds: string[];
  observedEdgeKeys: string[];
}

export interface Subgraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  ghosts: GhostNode[];
  overlay?: SubgraphOverlay | null;
}

export type ViewMode = "broad" | "isolation" | "tree";

export interface GraphHostBridge {
  openFile?: (path: string, line?: number) => void;
}

declare global {
  interface Window {
    __GRAPH_HOST__?: GraphHostBridge;
  }
}

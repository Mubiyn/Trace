import type { GraphNode } from "../types";
import { nodeFilePath } from "../modes";

interface SidePanelProps {
  node: GraphNode | null;
  onOpenFile?: (path: string, line?: number) => void;
}

export function SidePanel({ node, onOpenFile }: SidePanelProps) {
  if (!node) {
    return (
      <aside className="graph-side-panel graph-side-panel--empty">
        <p>Select a node to inspect details.</p>
      </aside>
    );
  }

  const path = nodeFilePath(node);

  return (
    <aside className="graph-side-panel">
      <h2>{node.name}</h2>
      <dl>
        <dt>Kind</dt>
        <dd>{node.kind}</dd>
        <dt>Path</dt>
        <dd>{path}</dd>
        {node.line != null && (
          <>
            <dt>Line</dt>
            <dd>{node.line}</dd>
          </>
        )}
        <dt>Language</dt>
        <dd>{node.language_id}</dd>
        <dt>ID</dt>
        <dd className="graph-mono">{node.id}</dd>
      </dl>
      {onOpenFile && path && (
        <button
          type="button"
          className="graph-button"
          onClick={() => onOpenFile(path, node.line ?? undefined)}
        >
          Open file
        </button>
      )}
    </aside>
  );
}

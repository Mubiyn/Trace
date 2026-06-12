import type { TraceResult } from "../types";

interface TracePanelProps {
  trace: TraceResult | null;
  loading: boolean;
  error: string | null;
  activeNodeId?: string | null;
  canGoBack?: boolean;
  onGoBack?: () => void;
  onSelectHop?: (nodeId: string) => void;
}

export function TracePanel({
  trace,
  loading,
  error,
  activeNodeId = null,
  canGoBack = false,
  onGoBack,
  onSelectHop,
}: TracePanelProps) {
  if (loading) {
    return (
      <section className="graph-trace-panel">
        <h3>Trace</h3>
        <p className="graph-trace-muted">Following branches…</p>
      </section>
    );
  }

  if (error) {
    return (
      <section className="graph-trace-panel">
        <h3>Trace</h3>
        <p className="graph-trace-error">{error}</p>
      </section>
    );
  }

  if (!trace || trace.hops.length === 0) {
    return (
      <section className="graph-trace-panel">
        <h3>Trace</h3>
        <p className="graph-trace-muted">
          Click a node to drill in. Trace shows roots → branches from the
          current focus.
        </p>
      </section>
    );
  }

  return (
    <section className="graph-trace-panel">
      <div className="graph-trace-header">
        <h3>Trace — roots &amp; branches</h3>
        {canGoBack && (
          <button
            type="button"
            className="graph-button graph-button--secondary graph-button--compact"
            onClick={() => onGoBack?.()}
          >
            Back
          </button>
        )}
      </div>
      <ol className="graph-trace-hops">
        {trace.hops.map((hop, index) => (
          <li key={hop.node.id}>
            <button
              type="button"
              className={
                hop.node.id === activeNodeId
                  ? "graph-trace-hop graph-trace-hop--active"
                  : "graph-trace-hop"
              }
              onClick={() => onSelectHop?.(hop.node.id)}
            >
              <span className="graph-trace-step">{index + 1}</span>
              <span className="graph-trace-name">{hop.node.name}</span>
              <span className="graph-trace-kind">{hop.node.kind}</span>
            </button>
            {hop.siblings.length > 0 && (
              <ul className="graph-trace-siblings" role="group" aria-label="Alternate branches">
                {hop.siblings.map((sibling) => (
                  <li key={sibling.id} className="graph-trace-sibling-item">
                    <span className="graph-trace-fork" aria-hidden>
                      └─
                    </span>
                    <button
                      type="button"
                      className="graph-trace-sibling"
                      onClick={() => onSelectHop?.(sibling.id)}
                    >
                      {sibling.kind === "branch" ? "decision" : "alternate"} → {sibling.name}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ol>
    </section>
  );
}

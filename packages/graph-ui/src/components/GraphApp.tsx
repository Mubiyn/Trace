import { useCallback, useEffect, useMemo, useState } from "react";
import { GraphClient } from "../api";
import { postOpenFile } from "../bridge";
import { filterSubgraphForMode, nodeFilePath } from "../modes";
import {
  drillTargetScope,
  frameLabelForNode,
  neighborhoodSubgraph,
  rootNavFrame,
  type NavFrame,
} from "../navigation";
import type { GraphNode, Subgraph, TraceResult, ViewMode } from "../types";
import {
  LARGE_GRAPH_NODE_THRESHOLD,
  trimSubgraphForDisplay,
} from "../viewport";
import { GraphCanvas } from "./GraphCanvas";
import { SidePanel } from "./SidePanel";
import { TracePanel } from "./TracePanel";

export interface GraphAppProps {
  apiBaseUrl?: string;
  initialRepoPath?: string;
  /** When true, index `initialRepoPath` once on mount (VS Code panel). */
  autoIndex?: boolean;
  onOpenFile?: (path: string, line?: number) => void;
}

function pushFrame(
  stack: NavFrame[],
  frame: Omit<NavFrame, "label"> & { label?: string },
): NavFrame[] {
  const label =
    frame.label ??
    (frame.focusId ? "focus" : frame.scope === "." ? "repository" : frame.scope);
  return [...stack, { ...frame, label } as NavFrame];
}

export function GraphApp({
  apiBaseUrl,
  initialRepoPath = "",
  autoIndex = false,
  onOpenFile = postOpenFile,
}: GraphAppProps) {
  const client = useMemo(() => new GraphClient(apiBaseUrl), [apiBaseUrl]);
  const [repoPath, setRepoPath] = useState(initialRepoPath);
  const [scope, setScope] = useState(".");
  const [mode, setMode] = useState<ViewMode>("isolation");
  const [focusId, setFocusId] = useState<string | null>(null);
  const [navStack, setNavStack] = useState<NavFrame[]>(() => [rootNavFrame()]);
  const [traceStack, setTraceStack] = useState<string[]>([]);
  const [rawSubgraph, setRawSubgraph] = useState<Subgraph | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [status, setStatus] = useState<string>("Connect to graph-server and index a repo.");
  const [busy, setBusy] = useState(false);
  const [trace, setTrace] = useState<TraceResult | null>(null);
  const [traceLoading, setTraceLoading] = useState(false);
  const [traceError, setTraceError] = useState<string | null>(null);

  const selectedNode = useMemo(() => {
    if (!selectedId || !rawSubgraph) {
      return null;
    }
    return rawSubgraph.nodes.find((node) => node.id === selectedId) ?? null;
  }, [rawSubgraph, selectedId]);

  const display = useMemo(() => {
    if (!rawSubgraph) {
      return null;
    }
    let filtered = filterSubgraphForMode(rawSubgraph, mode);
    if (focusId) {
      filtered = neighborhoodSubgraph(filtered, focusId, 2);
    }
    if (mode === "broad") {
      return {
        subgraph: filtered,
        trimmed: false,
        totalNodes: filterSubgraphForMode(rawSubgraph, mode).nodes.length,
      };
    }
    return trimSubgraphForDisplay(filtered);
  }, [rawSubgraph, mode, focusId]);

  const visibleSubgraph = display?.subgraph ?? null;

  const loadGraph = useCallback(
    async (options?: { scopeOverride?: string; keepSelection?: boolean }) => {
      setBusy(true);
      setStatus("Loading subgraph…");
      try {
        const queryScope = (options?.scopeOverride ?? scope).trim() || ".";
        const subgraph = await client.fetchSubgraph(queryScope);
        setRawSubgraph(subgraph);
        if (!options?.keepSelection) {
          setSelectedId(null);
          setFocusId(null);
          setTraceStack([]);
        }
        let message = `Loaded ${subgraph.nodes.length} nodes, ${subgraph.edges.length} edges`;
        if (queryScope !== ".") {
          message += ` (scope: ${queryScope})`;
        }
        if (subgraph.nodes.length > LARGE_GRAPH_NODE_THRESHOLD) {
          message +=
            ". Large graph — narrow scope, use Broad mode, or drill into a node.";
        }
        setStatus(`${message}.`);
      } catch (error) {
        setStatus(error instanceof Error ? error.message : "Failed to load graph");
      } finally {
        setBusy(false);
      }
    },
    [client, scope],
  );

  const restoreFrame = useCallback((frame: NavFrame) => {
    setScope(frame.scope);
    setFocusId(frame.focusId);
    setSelectedId(frame.selectedId);
    setMode(frame.mode);
    setTraceStack([]);
  }, []);

  const goBack = useCallback(() => {
    setNavStack((stack) => {
      if (stack.length <= 1) {
        return stack;
      }
      const next = stack.slice(0, -1);
      const frame = next[next.length - 1]!;
      restoreFrame(frame);
      void loadGraph({ scopeOverride: frame.scope, keepSelection: true });
      return next;
    });
  }, [restoreFrame, loadGraph]);

  const drillInto = useCallback(
    (node: GraphNode) => {
      setNavStack((stack) =>
        pushFrame(stack, {
          scope,
          focusId,
          selectedId,
          mode,
          label: focusId
            ? rawSubgraph?.nodes.find((n) => n.id === focusId)?.name ?? "focus"
            : scope === "."
              ? "repository"
              : scope,
        }),
      );

      const targetScope = drillTargetScope(node);
      if (targetScope) {
        setScope(targetScope);
        setFocusId(null);
        setSelectedId(node.id);
        setMode(node.kind === "directory" ? "broad" : "isolation");
        setTraceStack([]);
        void loadGraph({ scopeOverride: targetScope, keepSelection: true });
        return;
      }

      setFocusId(node.id);
      setSelectedId(node.id);
      setTraceStack([]);
    },
    [scope, focusId, selectedId, mode, rawSubgraph, loadGraph],
  );

  const handleNodeActivate = useCallback(
    (nodeId: string | null) => {
      if (!nodeId || !rawSubgraph) {
        setSelectedId(null);
        return;
      }
      const node = rawSubgraph.nodes.find((entry) => entry.id === nodeId);
      if (!node) {
        return;
      }
      drillInto(node);
    },
    [rawSubgraph, drillInto],
  );

  const handleTraceHop = useCallback(
    (nodeId: string) => {
      if (selectedId && selectedId !== nodeId) {
        setTraceStack((stack) => [...stack, selectedId]);
      }
      setSelectedId(nodeId);
      setFocusId(nodeId);
    },
    [selectedId],
  );

  const traceGoBack = useCallback(() => {
    setTraceStack((stack) => {
      const next = [...stack];
      const previous = next.pop();
      if (previous) {
        setSelectedId(previous);
        setFocusId(previous);
      }
      return next;
    });
  }, []);

  useEffect(() => {
    if (!selectedId || !rawSubgraph) {
      setTrace(null);
      setTraceError(null);
      return;
    }

    let cancelled = false;
    setTraceLoading(true);
    setTraceError(null);
    void client
      .trace(selectedId, 8)
      .then((result) => {
        if (!cancelled) {
          setTrace(result);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setTrace(null);
          setTraceError(
            error instanceof Error ? error.message : "Trace failed",
          );
        }
      })
      .finally(() => {
        if (!cancelled) {
          setTraceLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [client, selectedId, rawSubgraph]);

  const clearOverlay = useCallback(async () => {
    setBusy(true);
    setStatus("Clearing runtime overlay…");
    try {
      await client.clearOverlay();
      await loadGraph();
      setStatus("Runtime overlay cleared.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Clear overlay failed");
    } finally {
      setBusy(false);
    }
  }, [client, loadGraph]);

  const onOverlayFile = useCallback(
    async (file: File) => {
      setBusy(true);
      setStatus("Importing runtime overlay…");
      try {
        const text = await file.text();
        const payload = JSON.parse(text) as unknown;
        await client.importOverlay(payload);
        await loadGraph();
        setStatus("Runtime overlay applied.");
      } catch (error) {
        setStatus(error instanceof Error ? error.message : "Overlay import failed");
      } finally {
        setBusy(false);
      }
    },
    [client, loadGraph],
  );

  const indexAndLoad = useCallback(async () => {
    if (!repoPath.trim()) {
      setStatus("Enter a repository path or GitHub URL to index.");
      return;
    }
    setBusy(true);
    setStatus("Indexing repository…");
    try {
      await client.health();
      await client.indexRepo(repoPath.trim());
      setNavStack([rootNavFrame()]);
      setScope(".");
      setFocusId(null);
      setSelectedId(null);
      setTraceStack([]);
      await loadGraph();
      setStatus("Index complete.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Index failed");
    } finally {
      setBusy(false);
    }
  }, [client, repoPath, loadGraph]);

  useEffect(() => {
    if (!autoIndex || !initialRepoPath.trim()) {
      return;
    }
    void indexAndLoad();
    // eslint-disable-line react-hooks/exhaustive-deps -- mount-only auto index
  }, []);

  const breadcrumbs = useMemo(() => {
    const trail = [...navStack];
    if (focusId) {
      const focusNode = rawSubgraph?.nodes.find((node) => node.id === focusId);
      if (focusNode) {
        trail.push({
          scope,
          focusId,
          selectedId,
          mode,
          label: frameLabelForNode(focusNode),
        });
      }
    } else if (selectedNode && scope !== ".") {
      trail.push({
        scope,
        focusId,
        selectedId,
        mode,
        label: frameLabelForNode(selectedNode),
      });
    }
    return trail;
  }, [navStack, focusId, rawSubgraph, selectedNode, scope, selectedId, mode]);

  const canGoBack = navStack.length > 1 || traceStack.length > 0;

  return (
    <div className="graph-app">
      <header className="graph-toolbar">
        <div className="graph-brand">Graph</div>
        <input
          className="graph-input graph-input--path"
          type="text"
          placeholder="/path/to/repo or https://github.com/org/repo"
          value={repoPath}
          onChange={(event) => setRepoPath(event.target.value)}
          aria-label="Repository path"
        />
        <input
          className="graph-input graph-input--scope"
          type="text"
          placeholder="scope (e.g. frontend)"
          value={scope}
          onChange={(event) => setScope(event.target.value)}
          aria-label="Subgraph scope"
        />
        <button
          type="button"
          className="graph-button graph-button--secondary"
          disabled={!canGoBack || busy}
          onClick={() => {
            if (traceStack.length > 0) {
              traceGoBack();
            } else {
              goBack();
            }
          }}
        >
          Back
        </button>
        <button
          type="button"
          className="graph-button"
          disabled={busy}
          onClick={() => void indexAndLoad()}
        >
          Index
        </button>
        <button
          type="button"
          className="graph-button graph-button--secondary"
          disabled={busy}
          onClick={() => void loadGraph()}
        >
          Refresh
        </button>
        <label className="graph-button graph-button--secondary graph-file-input">
          Import trace
          <input
            type="file"
            accept="application/json,.json"
            disabled={busy}
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void onOverlayFile(file);
              }
              event.target.value = "";
            }}
          />
        </label>
        <button
          type="button"
          className="graph-button graph-button--secondary"
          disabled={busy}
          onClick={() => void clearOverlay()}
        >
          Clear overlay
        </button>
        <div className="graph-mode-toggle" role="group" aria-label="View mode">
          <button
            type="button"
            className={mode === "broad" ? "active" : ""}
            onClick={() => setMode("broad")}
          >
            Broad
          </button>
          <button
            type="button"
            className={mode === "isolation" ? "active" : ""}
            onClick={() => setMode("isolation")}
          >
            Isolation
          </button>
          <button
            type="button"
            className={mode === "tree" ? "active" : ""}
            onClick={() => setMode("tree")}
          >
            Tree
          </button>
        </div>
        {breadcrumbs.length > 0 && (
          <nav className="graph-breadcrumbs" aria-label="Graph navigation">
            {breadcrumbs.map((frame, index) => (
              <span key={`${frame.label}-${index}`} className="graph-crumb">
                {index > 0 && <span className="graph-crumb-sep">›</span>}
                <span
                  className={
                    index === breadcrumbs.length - 1
                      ? "graph-crumb-current"
                      : "graph-crumb-link"
                  }
                >
                  {frame.label}
                </span>
              </span>
            ))}
          </nav>
        )}
        <p className="graph-status">{status}</p>
      </header>
      <div className="graph-main">
        <GraphCanvas
          subgraph={visibleSubgraph}
          layoutMode={mode}
          onSelectNode={handleNodeActivate}
        />
        <div className="graph-side-stack">
          <SidePanel
            node={selectedNode as GraphNode | null}
            onOpenFile={onOpenFile}
          />
          <TracePanel
            trace={trace}
            loading={traceLoading}
            error={traceError}
            activeNodeId={selectedId}
            canGoBack={traceStack.length > 0}
            onGoBack={traceGoBack}
            onSelectHop={handleTraceHop}
          />
        </div>
      </div>
      {visibleSubgraph && display && (
        <footer className="graph-footer">
          Showing {visibleSubgraph.nodes.length}
          {display.trimmed ? ` of ${display.totalNodes}` : ""} nodes ·{" "}
          {visibleSubgraph.edges.length} edges ({mode}
          {focusId ? ", drilled in" : ""}
          {display.trimmed ? ", focused view" : ""})
        </footer>
      )}
    </div>
  );
}

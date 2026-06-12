import type { GraphHostBridge } from "./types";

interface VsCodeApi {
  postMessage(message: unknown): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

export function postOpenFile(path: string, line?: number): void {
  const payload = { type: "openFile" as const, path, line };

  try {
    if (typeof acquireVsCodeApi === "function") {
      acquireVsCodeApi().postMessage(payload);
      return;
    }
  } catch {
    // not in a VS Code webview
  }

  if (window.__GRAPH_HOST__?.openFile) {
    window.__GRAPH_HOST__.openFile(path, line);
    return;
  }

  console.info("[graph-ui] openFile", payload);
}

export function createHostBridge(bridge: GraphHostBridge): GraphHostBridge {
  window.__GRAPH_HOST__ = { ...window.__GRAPH_HOST__, ...bridge };
  return window.__GRAPH_HOST__;
}

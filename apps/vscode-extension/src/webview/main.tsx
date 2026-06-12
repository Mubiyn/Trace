import { GraphApp } from "@graph/ui";
import "../../../../packages/graph-ui/src/styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

declare function acquireVsCodeApi(): { postMessage(message: unknown): void };

const vscode = acquireVsCodeApi();
const initialRepoPath =
  (window as Window & { __GRAPH_REPO__?: string }).__GRAPH_REPO__ ?? "";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <GraphApp
      apiBaseUrl="http://127.0.0.1:9847"
      initialRepoPath={initialRepoPath}
      autoIndex={Boolean(initialRepoPath)}
      onOpenFile={(path, line) =>
        vscode.postMessage({ type: "openFile", path, line })
      }
    />
  </StrictMode>,
);

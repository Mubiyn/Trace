import { GraphApp } from "@graph/ui";
import "../../../packages/graph-ui/src/styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

const apiBaseUrl = import.meta.env.VITE_GRAPH_API ?? "http://127.0.0.1:9847";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <GraphApp apiBaseUrl={apiBaseUrl} />
  </StrictMode>,
);

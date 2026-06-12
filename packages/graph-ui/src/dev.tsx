import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { GraphApp } from "./components/GraphApp";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <GraphApp />
  </StrictMode>,
);

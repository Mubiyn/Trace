import cytoscape, {
  type Core,
  type ElementDefinition,
  type StylesheetStyle,
} from "cytoscape";
import { useEffect, useRef } from "react";
import type { Subgraph } from "../types";
import { pickLayoutRoots, toCytoscapeElements } from "../modes";
import type { ViewMode } from "../types";
import { applyGraphViewport } from "../viewport";

export interface GraphCanvasProps {
  subgraph: Subgraph | null;
  layoutMode?: ViewMode;
  onSelectNode?: (nodeId: string | null) => void;
}

export function GraphCanvas({
  subgraph,
  layoutMode = "isolation",
  onSelectNode,
}: GraphCanvasProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const onSelectRef = useRef(onSelectNode);
  onSelectRef.current = onSelectNode;

  useEffect(() => {
    if (!containerRef.current) {
      return;
    }

    const stylesheet: StylesheetStyle[] = [
      {
        selector: "node",
        style: {
          label: "data(label)",
          "background-color": "data(color)",
          shape: "data(shape)",
          color: "#e2e8f0",
          "font-size": "11px",
          "text-wrap": "wrap",
          "text-max-width": "120px",
          "border-width": "1px",
          "border-color": "#334155",
          padding: "8px",
          width: 28,
          height: 28,
        } as StylesheetStyle["style"],
      },
      {
        selector: "edge",
        style: {
          width: 1.5,
          "line-color": "#475569",
          "target-arrow-color": "#475569",
          "target-arrow-shape": "triangle",
          "curve-style": "bezier",
          label: "data(label)",
          "font-size": "8px",
          color: "#94a3b8",
        },
      },
      {
        selector: "node:selected",
        style: {
          "border-width": "3px",
          "border-color": "#38bdf8",
        },
      },
      {
        selector: "node[observed]",
        style: {
          "border-width": "3px",
          "border-color": "#fb923c",
        },
      },
      {
        selector: "edge[observed]",
        style: {
          width: 3,
          "line-color": "#fb923c",
          "target-arrow-color": "#fb923c",
        },
      },
    ];

    const cy = cytoscape({
      container: containerRef.current,
      style: stylesheet,
      layout: { name: "breadthfirst", directed: true, padding: 24 },
      wheelSensitivity: 0.2,
      minZoom: 0.08,
      maxZoom: 3,
    });

    cy.on("tap", "node", (event) => {
      onSelectRef.current?.(event.target.id());
    });
    cy.on("tap", (event) => {
      if (event.target === cy) {
        onSelectRef.current?.(null);
      }
    });

    cyRef.current = cy;
    return () => {
      cy.destroy();
      cyRef.current = null;
    };
  }, []);

  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) {
      return;
    }

    cy.elements().remove();
    if (!subgraph || subgraph.nodes.length === 0) {
      return;
    }

    const elements = toCytoscapeElements(subgraph) as ElementDefinition[];
    cy.add(elements);

    const roots =
      layoutMode === "tree" ? pickLayoutRoots(subgraph) : [];
    const layoutOptions =
      roots.length > 0
        ? {
            name: "breadthfirst" as const,
            directed: true,
            padding: 32,
            spacingFactor: 1.25,
            roots: cy.nodes().filter((node) => roots.includes(node.id())),
          }
        : { name: "breadthfirst" as const, directed: true, padding: 24 };

    let active = true;
    const onLayoutStop = () => {
      if (active) {
        applyGraphViewport(cy, subgraph, 40);
      }
    };
    cy.on("layoutstop", onLayoutStop);

    const layout = cy.layout(layoutOptions);
    layout.run();

    requestAnimationFrame(() => {
      if (active && cy.nodes().length > 0) {
        applyGraphViewport(cy, subgraph, 40);
      }
    });

    return () => {
      active = false;
      cy.off("layoutstop", onLayoutStop);
      layout.stop();
    };
  }, [subgraph, layoutMode]);

  return <div ref={containerRef} className="graph-canvas" data-testid="graph-canvas" />;
}

import { describe, expect, it } from "vitest";
import { buildSyntheticSubgraph, filterSubgraphForMode } from "./modes";
import { neighborhoodSubgraph } from "./navigation";
import { normalizeNodeKind, normalizeSubgraph } from "./normalize";

describe("normalizeSubgraph", () => {
  it("maps uielement to ui_element", () => {
    expect(normalizeNodeKind("uielement")).toBe("ui_element");
    const subgraph = normalizeSubgraph({
      nodes: [
        {
          id: "ui:1",
          kind: "uielement" as never,
          name: "button.onClick",
          relative_path: "App.tsx",
          language_id: "typescript",
        },
      ],
      edges: [],
      ghosts: [],
    });
    expect(subgraph.nodes[0]?.kind).toBe("ui_element");
  });
});

describe("neighborhoodSubgraph", () => {
  it("expands around a focus node", () => {
    const raw = filterSubgraphForMode(buildSyntheticSubgraph(4), "isolation");
    const focus = raw.nodes.find((node) => node.name === "fn_0")?.id;
    expect(focus).toBeTruthy();
    const focused = neighborhoodSubgraph(raw, focus!, 1);
    expect(focused.nodes.length).toBeGreaterThan(1);
  });
});

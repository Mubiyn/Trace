import { describe, expect, it } from "vitest";
import { buildSyntheticSubgraph, filterSubgraphForMode } from "./modes";
import {
  LARGE_GRAPH_FOCUS_LIMIT,
  LARGE_GRAPH_NODE_THRESHOLD,
  MIN_READABLE_ZOOM,
  trimSubgraphForDisplay,
} from "./viewport";

describe("viewport constants", () => {
  it("uses sensible thresholds for readable graphs", () => {
    expect(LARGE_GRAPH_NODE_THRESHOLD).toBeGreaterThan(50);
    expect(MIN_READABLE_ZOOM).toBeGreaterThan(0.1);
    expect(LARGE_GRAPH_FOCUS_LIMIT).toBeLessThan(LARGE_GRAPH_NODE_THRESHOLD);
  });
});

describe("trimSubgraphForDisplay", () => {
  it("keeps small graphs intact", () => {
    const raw = buildSyntheticSubgraph(5);
    const result = trimSubgraphForDisplay(raw);
    expect(result.trimmed).toBe(false);
    expect(result.subgraph.nodes.length).toBe(raw.nodes.length);
  });

  it("trims large graphs to a focused neighborhood", () => {
    const raw = filterSubgraphForMode(buildSyntheticSubgraph(200), "isolation");
    const result = trimSubgraphForDisplay(raw);
    expect(result.trimmed).toBe(true);
    expect(result.subgraph.nodes.length).toBeLessThanOrEqual(
      LARGE_GRAPH_FOCUS_LIMIT,
    );
    expect(result.totalNodes).toBe(raw.nodes.length);
  });
});

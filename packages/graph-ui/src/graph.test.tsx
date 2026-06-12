import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GraphCanvas } from "./components/GraphCanvas";
import {
  buildSyntheticSubgraph,
  filterSubgraphForMode,
  toCytoscapeElements,
} from "./modes";

const layoutMock = {
  run: vi.fn(),
  stop: vi.fn(),
};

const cytoscapeMock = vi.fn(() => ({
  on: vi.fn(),
  off: vi.fn(),
  elements: vi.fn(() => ({ remove: vi.fn() })),
  add: vi.fn(),
  layout: vi.fn(() => layoutMock),
  fit: vi.fn(),
  zoom: vi.fn(() => 1),
  center: vi.fn(),
  nodes: vi.fn(() => ({
    filter: vi.fn(() => ({ length: 0, union: vi.fn() })),
    slice: vi.fn(() => ({ length: 0 })),
  })),
  destroy: vi.fn(),
}));

vi.mock("cytoscape", () => ({
  default: (...args: unknown[]) => cytoscapeMock(...args),
}));

describe("GraphCanvas", () => {
  beforeEach(() => {
    cytoscapeMock.mockClear();
  });

  it("mounts with 100 nodes", () => {
    const subgraph = filterSubgraphForMode(buildSyntheticSubgraph(100), "broad");
    const nodeElements = toCytoscapeElements(subgraph).filter(
      (element) => element.group === "nodes",
    );
    expect(nodeElements.length).toBeGreaterThanOrEqual(100);

    render(<GraphCanvas subgraph={subgraph} />);
    expect(screen.getByTestId("graph-canvas")).toBeTruthy();
    expect(cytoscapeMock).toHaveBeenCalled();
  });
});

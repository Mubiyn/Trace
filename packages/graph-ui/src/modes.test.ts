import { describe, expect, it } from "vitest";
import {
  buildSyntheticSubgraph,
  filterSubgraphForMode,
  pickLayoutRoots,
  toCytoscapeElements,
} from "./modes";

describe("filterSubgraphForMode", () => {
  it("broad mode keeps file tree nodes only", () => {
    const raw = buildSyntheticSubgraph(5);
    const broad = filterSubgraphForMode(raw, "broad");
    const nodeIds = new Set(broad.nodes.map((node) => node.id));

    expect(broad.nodes.every((n) => n.kind === "directory" || n.kind === "file")).toBe(
      true,
    );
    expect(broad.edges.every((e) => e.kind === "Contains")).toBe(true);
    expect(broad.edges.every((e) => nodeIds.has(e.from_id) && nodeIds.has(e.to_id))).toBe(
      true,
    );
    expect(broad.nodes.length).toBeGreaterThan(0);
  });

  it("isolation mode keeps symbols and semantic edges", () => {
    const raw = buildSyntheticSubgraph(5);
    const isolation = filterSubgraphForMode(raw, "isolation");

    expect(
      isolation.nodes.every((n) =>
        ["function", "class", "import"].includes(n.kind),
      ),
    ).toBe(true);
    expect(
      isolation.edges.every((e) => e.kind === "Calls" || e.kind === "Imports"),
    ).toBe(true);
    expect(isolation.edges.length).toBeGreaterThan(0);
  });

  it("mode switch changes visible node counts", () => {
    const raw = buildSyntheticSubgraph(10);
    const broad = filterSubgraphForMode(raw, "broad");
    const isolation = filterSubgraphForMode(raw, "isolation");

    expect(broad.nodes.length).not.toBe(isolation.nodes.length);
  });

  it("tree mode uses isolation semantics", () => {
    const raw = buildSyntheticSubgraph(5);
    const tree = filterSubgraphForMode(raw, "tree");
    const isolation = filterSubgraphForMode(raw, "isolation");

    expect(tree.nodes.map((n) => n.id).sort()).toEqual(
      isolation.nodes.map((n) => n.id).sort(),
    );
  });

  it("pickLayoutRoots prefers nodes without incoming edges", () => {
    const raw = buildSyntheticSubgraph(3);
    const isolation = filterSubgraphForMode(raw, "isolation");
    const roots = pickLayoutRoots(isolation);

    expect(roots.length).toBeGreaterThan(0);
    expect(toCytoscapeElements(isolation).length).toBeGreaterThan(0);
  });
});

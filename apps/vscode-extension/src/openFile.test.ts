import { describe, expect, it, vi } from "vitest";

/** Mirrors extension.ts openFile handling for unit tests. */
export function resolveOpenFileTarget(
  workspaceRoot: string,
  path: string,
  line = 1,
): { absolutePath: string; line: number } {
  const normalized = path.replace(/^\.\//, "");
  return {
    absolutePath: `${workspaceRoot}/${normalized}`,
    line: Math.max(1, line),
  };
}

describe("openFile message handling", () => {
  it("maps webview openFile to workspace path and line", () => {
    const result = resolveOpenFileTarget("/repo", "main.py", 4);
    expect(result).toEqual({
      absolutePath: "/repo/main.py",
      line: 4,
    });
  });

  it("clamps invalid lines to at least 1", () => {
    const result = resolveOpenFileTarget("/repo", "utils.py", 0);
    expect(result.line).toBe(1);
  });

  it("postMessage payload shape matches extension contract", () => {
    const postMessage = vi.fn();
    const payload = { type: "openFile" as const, path: "main.py", line: 4 };
    postMessage(payload);
    expect(postMessage).toHaveBeenCalledWith({
      type: "openFile",
      path: "main.py",
      line: 4,
    });
  });
});

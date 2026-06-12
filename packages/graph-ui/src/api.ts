import type { Subgraph, TraceResult } from "./types";
import { normalizeSubgraph } from "./normalize";

const DEFAULT_BASE = "http://127.0.0.1:9847";
const GIT_URL_RE = /^https?:\/\//i;

export class GraphApiError extends Error {
  constructor(
    message: string,
    readonly code?: string,
    readonly status?: number,
  ) {
    super(message);
    this.name = "GraphApiError";
  }
}

export class GraphClient {
  constructor(private readonly baseUrl = DEFAULT_BASE) {}

  get base() {
    return this.baseUrl.replace(/\/$/, "");
  }

  async health(): Promise<{ status: string; version: string }> {
    return this.getJson("/health");
  }

  async indexRepo(pathOrUrl: string): Promise<void> {
    const trimmed = pathOrUrl.trim();
    const body = GIT_URL_RE.test(trimmed)
      ? {
          path: ".",
          gitUrl: trimmed,
          gitRef: "main",
          persist: true,
        }
      : { path: trimmed };
    const accepted = await this.postJson<{ job_id: string; status: string }>(
      "/index",
      body,
    );
    await this.waitForJob(accepted.job_id);
  }

  async fetchSubgraph(scope = "."): Promise<Subgraph> {
    const subgraph = await this.postJson<Subgraph>("/query", {
      op: "subgraph",
      scope,
      boundary: true,
    });
    return normalizeSubgraph(subgraph);
  }

  async trace(id: string, depth = 8): Promise<TraceResult> {
    return this.postJson<TraceResult>("/query", {
      op: "trace",
      id,
      depth,
    });
  }

  async importOverlay(payload: unknown): Promise<{ status: string }> {
    return this.postJson("/overlay", payload);
  }

  async clearOverlay(): Promise<void> {
    const res = await fetch(`${this.base}/overlay`, { method: "DELETE" });
    if (!res.ok && res.status !== 204) {
      throw new GraphApiError(`HTTP ${res.status}`, undefined, res.status);
    }
  }

  private async waitForJob(jobId: string): Promise<void> {
    for (let i = 0; i < 120; i++) {
      const status = await this.getJson<{
        status: string;
        error?: string;
      }>(`/index/${jobId}`);
      if (status.status === "complete") {
        return;
      }
      if (status.status === "failed") {
        throw new GraphApiError(status.error ?? "index failed", "index_failed");
      }
      await sleep(250);
    }
    throw new GraphApiError("index timed out", "index_timeout");
  }

  private async getJson<T>(path: string): Promise<T> {
    const res = await fetch(`${this.base}${path}`);
    return this.parseJson<T>(res);
  }

  private async postJson<T>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`${this.base}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    return this.parseJson<T>(res);
  }

  private async parseJson<T>(res: Response): Promise<T> {
    const payload = (await res.json()) as T & { error?: string; code?: string };
    if (!res.ok) {
      throw new GraphApiError(
        payload.error ?? `HTTP ${res.status}`,
        payload.code,
        res.status,
      );
    }
    return payload;
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export { DEFAULT_BASE };

export const GRAPH_SERVER_HOST = "127.0.0.1";
export const GRAPH_SERVER_PORT = 9847;

export function graphServerUrl(): string {
  return `http://${GRAPH_SERVER_HOST}:${GRAPH_SERVER_PORT}`;
}

/** Candidate binaries tried when the desktop shell spawns graph-server. */
export function graphServerSpawnCandidates(): string[] {
  return [
    "graph-server",
    "../../target/debug/graph-server",
    "../../../target/debug/graph-server",
    "../../../../target/debug/graph-server",
  ];
}

import express from "express";

const app = express();

export function health() {
  return { ok: true };
}

app.get("/health", health);

export { app };

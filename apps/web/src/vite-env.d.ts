/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_GRAPH_API?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

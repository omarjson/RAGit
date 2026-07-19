export const DEFAULT_LIBRARY_ID = "default";
export const TEAM_PORT = 11436;
export const LEVELS = [1, 2, 3, 4, 5] as const;
export type IndexLevel = (typeof LEVELS)[number];
export const LEVEL_LABELS: Record<IndexLevel, string> = {
  1: "Raw",
  2: "Structure",
  3: "Summaries",
  4: "Dense (embeddings)",
  5: "Rerank",
};

export const DEFAULT_GPU_LAYERS = -1;
export const DEFAULT_CONTEXT_SIZE = 8192;
export const MAX_RERANK_CANDIDATES = 12;

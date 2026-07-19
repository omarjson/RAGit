export interface HardwareInfo {
  os: string;
  arch: string;
  cpuCores: number;
  cpuBrand: string;
  totalRamBytes: number;
  gpuBackend: string;
  gpuName: string | null;
  vramBytes: number | null;
}

export type Fitness = "Fits" | "Tight" | "TooBig";

export interface ModelVariant {
  quant: string;
  sizeGb: number;
  fitness: Fitness;
}

export interface CatalogModel {
  id: string;
  name: string;
  repo: string;
  defaultFile: string;
  modalities: string[];
  context: number;
  embed: boolean;
  variants: ModelVariant[];
}

export interface SearchHit {
  id: string;
  repo: string;
  defaultFile: string;
  modalities: string[];
  context: number;
  embed: boolean;
  sizeGb: number;
  fitness: Fitness;
  downloads: number;
}

export interface EngineStatus {
  running: boolean;
  modelPath: string | null;
  port: number | null;
  measuredTps: number | null;
  backend: string | null;
  embedModel: string | null;
  embedPort: number | null;
}

export type ChatRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatRole;
  content: string;
}

export type ChatEvent =
  | { event: "start"; data: { model: string } }
  | { event: "token"; data: { text: string } }
  | { event: "done"; data: { tokens: number } }
  | { event: "error"; data: { message: string } };

export type DownloadEvent =
  | { event: "started"; data: { url: string; totalBytes: number } }
  | { event: "progress"; data: { downloaded: number; totalBytes: number } }
  | { event: "verified"; data: { sha256: string } }
  | { event: "finished"; data: { path: string } }
  | { event: "error"; data: { message: string } };

export interface IndexProgress {
  libraryId: string;
  total: number;
  processed: number;
  currentFile: string;
  level: number;
  status: string;
  message: string;
}

export interface IndexedFile {
  id: number;
  libraryId: string;
  path: string;
  fileName: string;
  contentHash: string;
  status: string;
  level: number;
  chunks: number;
  error: string | null;
}

export interface TeamUser {
  id: string;
  username: string;
  role: string;
}

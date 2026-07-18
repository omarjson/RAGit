import { invoke, Channel } from "@tauri-apps/api/core";

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

export function detectHardware(): Promise<HardwareInfo> {
  return invoke("detect_hardware");
}

export function listModels(): Promise<CatalogModel[]> {
  return invoke("list_models");
}

export type DownloadEvent =
  | { event: "started"; data: { url: string; totalBytes: number } }
  | { event: "progress"; data: { downloaded: number; totalBytes: number } }
  | { event: "verified"; data: { sha256: string } }
  | { event: "finished"; data: { path: string } }
  | { event: "error"; data: { message: string } };

export function downloadModel(
  repo: string,
  filename: string,
  expectedSha256: string | null,
  onEvent: (e: DownloadEvent) => void
): Promise<void> {
  const channel = new Channel<DownloadEvent>();
  channel.onmessage = (msg) => onEvent(msg);
  return invoke("download_model", {
    repo,
    filename,
    expectedSha256,
    onEvent: channel,
  });
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

export function startEngine(
  modelPath: string,
  port?: number,
  gpuLayers?: number,
  embedModelPath?: string,
  embedPort?: number
): Promise<EngineStatus> {
  return invoke("start_engine", {
    modelPath,
    port,
    gpuLayers,
    embedModelPath,
    embedPort,
  });
}

export function exportLibrary(path: string, libraryId?: string): Promise<string> {
  return invoke("export_library", { path, libraryId });
}

export function importLibrary(path: string, libraryId?: string): Promise<string> {
  return invoke("import_library", { path, libraryId });
}

export function ragChat(
  message: string,
  libraryId?: string,
  history?: ChatMessage[],
  rerank?: boolean
): Promise<string> {
  return invoke("rag_chat", {
    message,
    libraryId,
    history: history ?? [],
    rerank,
  });
}

export function stopEngine(): Promise<void> {
  return invoke("stop_engine");
}

export function engineStatus(): Promise<EngineStatus> {
  return invoke("engine_status");
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

export function chatStream(
  messages: ChatMessage[],
  onEvent: (e: ChatEvent) => void
): Promise<void> {
  const channel = new Channel<ChatEvent>();
  channel.onmessage = (msg) => onEvent(msg);
  return invoke("chat_stream", { messages, onEvent: channel });
}

export function indexLibrary(
  path: string,
  libraryId?: string,
  level?: number,
  onProgress?: (p: IndexProgress) => void
): Promise<string> {
  const channel = new Channel<IndexProgress>();
  if (onProgress) channel.onmessage = (msg) => onProgress(msg);
  return invoke("index_library", {
    path,
    libraryId,
    level,
    onProgress: channel,
  });
}

export interface IndexProgress {
  libraryId: string;
  total: number;
  processed: number;
  currentFile: string;
  level: number;
  status: string;
  message: string;
}

export function pauseIndex(libraryId?: string): Promise<void> {
  return invoke("pause_index", { libraryId });
}

export function resumeIndex(libraryId?: string): Promise<void> {
  return invoke("resume_index", { libraryId });
}

export function cancelIndex(libraryId?: string): Promise<void> {
  return invoke("cancel_index", { libraryId });
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

export function listIndexedFiles(
  libraryId?: string
): Promise<IndexedFile[]> {
  return invoke("list_indexed_files", { libraryId });
}

export function setScheduler(
  libraryId: string | undefined,
  enabled: boolean,
  intervalSecs?: number
): Promise<string> {
  return invoke("set_scheduler", { libraryId, enabled, intervalSecs });
}

// ---- Team Mode ----

export function startTeamServer(port?: number): Promise<string> {
  return invoke("start_team_server_cmd", { port });
}

export function stopTeamServer(): Promise<string> {
  return invoke("stop_team_server_cmd");
}

export function teamStatus(): Promise<boolean> {
  return invoke("team_status_cmd");
}

export interface TeamUser {
  id: string;
  username: string;
  role: string;
}

export async function teamApi(
  method: "GET" | "POST",
  path: string,
  body?: unknown,
  token?: string | null
): Promise<unknown> {
  const base = `http://localhost:11436${path}`;
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const res = await fetch(base, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error((data as any).error || res.statusText);
  return data;
}

export function openFolderDialog(): Promise<string | null> {
  return import("@tauri-apps/plugin-dialog")
    .then((m) => m.open({ directory: true, multiple: false }))
    .then((r) => (Array.isArray(r) ? (r[0] ?? null) : (r as string | null)));
}

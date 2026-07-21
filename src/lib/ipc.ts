import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  HardwareInfo,
  Fitness,
  ModelVariant,
  CatalogModel,
  SearchHit,
  DownloadEvent,
  EngineStatus,
  ChatRole,
  ChatMessage,
  ChatEvent,
  IndexProgress,
  IndexedFile,
  TeamUser,
} from "./ipc/types";
import { TEAM_PORT } from "./ipc/constants";

export type {
  HardwareInfo,
  Fitness,
  ModelVariant,
  CatalogModel,
  SearchHit,
  DownloadEvent,
  EngineStatus,
  ChatRole,
  ChatMessage,
  ChatEvent,
  IndexProgress,
  IndexedFile,
  TeamUser,
};

export function runningInTauri(): boolean {
  try {
    const w = window as unknown as Record<string, unknown>;
    return Boolean(w["__TAURI_INTERNALS__"] || w["__TAURI__"]);
  } catch {
    return false;
  }
}

export function detectHardware(): Promise<HardwareInfo> {
  return invoke("detect_hardware");
}

export function listModels(): Promise<CatalogModel[]> {
  return invoke("list_models");
}

export function searchModels(query: string): Promise<SearchHit[]> {
  return invoke("search_hf_models", { query });
}

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

export function pauseIndex(libraryId?: string): Promise<void> {
  return invoke("pause_index", { libraryId });
}

export function resumeIndex(libraryId?: string): Promise<void> {
  return invoke("resume_index", { libraryId });
}

export function cancelIndex(libraryId?: string): Promise<void> {
  return invoke("cancel_index", { libraryId });
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

export function startTeamServer(port?: number): Promise<string> {
  return invoke("start_team_server_cmd", { port });
}

export function stopTeamServer(): Promise<string> {
  return invoke("stop_team_server_cmd");
}

export function teamStatus(): Promise<boolean> {
  return invoke("team_status_cmd");
}

export async function teamApi(
  method: "GET" | "POST",
  path: string,
  body?: unknown,
  token?: string | null
): Promise<unknown> {
  const base = `http://localhost:${TEAM_PORT}${path}`;
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

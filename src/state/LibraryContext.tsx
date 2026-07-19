import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import {
  indexLibrary as invokeIndex,
  pauseIndex as invokePause,
  resumeIndex as invokeResume,
  cancelIndex as invokeCancel,
  listIndexedFiles as invokeListFiles,
  setScheduler as invokeScheduler,
  type IndexProgress,
  type IndexedFile,
} from "../lib/ipc";
import { DEFAULT_LIBRARY_ID } from "../lib/ipc/constants";

interface IndexJob {
  libraryId: string;
  total: number;
  processed: number;
  currentFile: string;
  level: number;
  status: "running" | "paused" | "done" | "canceled" | "error";
  message: string;
}

interface LibraryContextValue {
  files: IndexedFile[];
  job: IndexJob | null;
  loading: boolean;
  error: string | null;
  startIndex: (path: string, libraryId?: string, level?: number) => Promise<void>;
  pauseIndex: (libraryId?: string) => Promise<void>;
  resumeIndex: (libraryId?: string) => Promise<void>;
  cancelIndex: (libraryId?: string) => Promise<void>;
  refreshFiles: (libraryId?: string) => Promise<void>;
  setScheduler: (libraryId: string | undefined, enabled: boolean, intervalSecs?: number) => Promise<string>;
}

const LibraryContext = createContext<LibraryContextValue | null>(null);

export function LibraryProvider({ children }: { children: ReactNode }) {
  const [files, setFiles] = useState<IndexedFile[]>([]);
  const [job, setJob] = useState<IndexJob | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refreshFiles = useCallback(async (libraryId?: string) => {
    try {
      const f = await invokeListFiles(libraryId);
      setFiles(f);
    } catch (e) {
      setFiles([]);
    }
  }, []);

  const startIndex = useCallback(async (path: string, libraryId?: string, level?: number) => {
    const lib = libraryId ?? DEFAULT_LIBRARY_ID;
    setLoading(true);
    setError(null);
    setJob({ libraryId: lib, total: 0, processed: 0, currentFile: "", level: level ?? 4, status: "running", message: "Starting..." });
    try {
      await invokeIndex(path, lib, level, (p: IndexProgress) => {
        setJob({
          libraryId: p.libraryId,
          total: p.total,
          processed: p.processed,
          currentFile: p.currentFile,
          level: p.level,
          status: p.status as IndexJob["status"],
          message: p.message,
        });
        if (p.status === "done" || p.status === "canceled" || p.status === "error") {
          refreshFiles(lib);
        }
      });
    } catch (e) {
      setError(String(e));
      setJob((prev) => prev ? { ...prev, status: "error", message: String(e) } : null);
    } finally {
      setLoading(false);
    }
  }, [refreshFiles]);

  const pauseIndex = useCallback(async (libraryId?: string) => {
    try { await invokePause(libraryId); }
    catch (e) { setError(String(e)); }
  }, []);

  const resumeIndex = useCallback(async (libraryId?: string) => {
    try { await invokeResume(libraryId); }
    catch (e) { setError(String(e)); }
  }, []);

  const cancelIndex = useCallback(async (libraryId?: string) => {
    try { await invokeCancel(libraryId); }
    catch (e) { setError(String(e)); }
  }, []);

  const setScheduler = useCallback(async (libraryId: string | undefined, enabled: boolean, intervalSecs?: number) => {
    return invokeScheduler(libraryId, enabled, intervalSecs);
  }, []);

  return (
    <LibraryContext.Provider value={{
      files, job, loading, error,
      startIndex, pauseIndex, resumeIndex, cancelIndex, refreshFiles, setScheduler,
    }}>
      {children}
    </LibraryContext.Provider>
  );
}

export function useLibrary(): LibraryContextValue {
  const ctx = useContext(LibraryContext);
  if (!ctx) throw new Error("useLibrary must be used within LibraryProvider");
  return ctx;
}

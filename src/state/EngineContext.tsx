import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from "react";
import { engineStatus, startEngine as invokeStart, stopEngine as invokeStop, type EngineStatus } from "../lib/ipc";

interface EngineContextValue {
  status: EngineStatus | null;
  loading: boolean;
  error: string | null;
  start: (modelPath: string, port?: number, gpuLayers?: number, embedModelPath?: string, embedPort?: number) => Promise<EngineStatus>;
  stop: () => Promise<void>;
  refresh: () => Promise<void>;
}

const EngineContext = createContext<EngineContextValue | null>(null);

export function EngineProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const s = await engineStatus();
      setStatus(s);
    } catch (e) {
      setStatus(null);
      setError(String(e));
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const start = useCallback(async (
    modelPath: string,
    port?: number,
    gpuLayers?: number,
    embedModelPath?: string,
    embedPort?: number,
  ) => {
    setLoading(true);
    setError(null);
    try {
      const s = await invokeStart(modelPath, port, gpuLayers, embedModelPath, embedPort);
      setStatus(s);
      return s;
    } catch (e) {
      const msg = String(e);
      setError(msg);
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  const stop = useCallback(async () => {
    setLoading(true);
    try {
      await invokeStop();
      setStatus(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  return (
    <EngineContext.Provider value={{ status, loading, error, start, stop, refresh }}>
      {children}
    </EngineContext.Provider>
  );
}

export function useEngine(): EngineContextValue {
  const ctx = useContext(EngineContext);
  if (!ctx) throw new Error("useEngine must be used within EngineProvider");
  return ctx;
}

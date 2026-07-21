import { createContext, useContext, useState, useCallback, type ReactNode } from "react";
import { startTeamServer as invokeStart, stopTeamServer as invokeStop, teamStatus as invokeStatus, teamApi, type TeamUser } from "../lib/ipc";

interface TeamContextValue {
  running: boolean;
  token: string | null;
  role: string | null;
  users: TeamUser[];
  me: string | null;
  loading: boolean;
  error: string | null;
  start: (port?: number) => Promise<string>;
  stop: () => Promise<string>;
  refresh: () => Promise<void>;
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<void>;
  logout: () => void;
  setUserRole: (userId: string, role: string) => Promise<void>;
}

const TeamContext = createContext<TeamContextValue | null>(null);

export function TeamProvider({ children }: { children: ReactNode }) {
  const [running, setRunning] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [role, setRole] = useState<string | null>(null);
  const [users, setUsers] = useState<TeamUser[]>([]);
  const [me, setMe] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchUsers = useCallback(async (tok: string) => {
    try {
      const data = (await teamApi("GET", "/api/users", undefined, tok)) as TeamUser[];
      setUsers(data);
    } catch { /* ignore */ }
  }, []);

  const refresh = useCallback(async () => {
    const st = await invokeStatus();
    setRunning(st);
  }, []);

  const start = useCallback(async (port?: number) => {
    setLoading(true);
    setError(null);
    try {
      const msg = await invokeStart(port);
      setRunning(true);
      return msg;
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  const stop = useCallback(async () => {
    setLoading(true);
    try {
      const msg = await invokeStop();
      setRunning(false);
      setToken(null);
      setRole(null);
      setMe(null);
      setUsers([]);
      return msg;
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  const login = useCallback(async (username: string, password: string) => {
    setLoading(true);
    setError(null);
    try {
      const data = (await teamApi("POST", "/api/login", { username, password })) as { token: string; role: string; user_id: string };
      setToken(data.token);
      setRole(data.role);
      setMe(data.user_id);
      await fetchUsers(data.token);
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setLoading(false);
    }
  }, [fetchUsers]);

  const register = useCallback(async (username: string, password: string) => {
    setLoading(true);
    setError(null);
    try {
      await teamApi("POST", "/api/register", { username, password });
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  const logout = useCallback(() => {
    setToken(null);
    setRole(null);
    setMe(null);
    setUsers([]);
  }, []);

  const setUserRole = useCallback(async (userId: string, newRole: string) => {
    if (!token) return;
    try {
      await teamApi("POST", "/api/users/role", { userId, role: newRole }, token);
      await fetchUsers(token);
    } catch (e) {
      setError(String(e));
    }
  }, [token, fetchUsers]);

  return (
    <TeamContext.Provider value={{
      running, token, role, users, me, loading, error,
      start, stop, refresh, login, register, logout, setUserRole,
    }}>
      {children}
    </TeamContext.Provider>
  );
}

export function useTeam(): TeamContextValue {
  const ctx = useContext(TeamContext);
  if (!ctx) throw new Error("useTeam must be used within TeamProvider");
  return ctx;
}

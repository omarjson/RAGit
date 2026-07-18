import { useEffect, useState } from "react";
import {
  startTeamServer,
  stopTeamServer,
  teamStatus,
  teamApi,
  TeamUser,
} from "../lib/ipc";

export function Team() {
  const [running, setRunning] = useState(false);
  const [token, setToken] = useState<string | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [users, setUsers] = useState<TeamUser[]>([]);
  const [msg, setMsg] = useState("");
  const [myRole, setMyRole] = useState<string>("");
  const [chatInput, setChatInput] = useState("");
  const [chatOut, setChatOut] = useState("");

  const refresh = async () => {
    const up = await teamStatus();
    setRunning(up);
    if (up && token) {
      try {
        const me = (await teamApi("GET", "/api/me", undefined, token)) as any;
        setMyRole(me.role);
        if (me.role === "admin") {
          const list = (await teamApi("GET", "/api/users", undefined, token)) as TeamUser[];
          setUsers(list);
        }
      } catch {
        /* ignore */
      }
    }
  };

  useEffect(() => {
    refresh();
  }, [token]);

  const start = async () => {
    try {
      setMsg(await startTeamServer(11436));
      await refresh();
    } catch (e) {
      setMsg(`⚠ ${String(e)}`);
    }
  };
  const stop = async () => {
    try {
      setMsg(await stopTeamServer());
      setToken(null);
      await refresh();
    } catch (e) {
      setMsg(`⚠ ${String(e)}`);
    }
  };

  const login = async () => {
    try {
      const r = (await teamApi("POST", "/api/login", { username, password })) as any;
      setToken(r.token);
      setMsg(`Logged in as ${username} (${r.role})`);
    } catch (e) {
      setMsg(`⚠ ${String(e)}`);
    }
  };

  const register = async () => {
    try {
      const r = (await teamApi("POST", "/api/register", { username, password })) as any;
      setMsg(`Registered ${username} (${r.role})`);
      await login();
    } catch (e) {
      setMsg(`⚠ ${String(e)}`);
    }
  };

  const setRole = async (userId: string, role: string) => {
    try {
      await teamApi("POST", "/api/users/role", { user_id: userId, role }, token);
      await refresh();
    } catch (e) {
      setMsg(`⚠ ${String(e)}`);
    }
  };

  const sendChat = async () => {
    if (!chatInput.trim()) return;
    try {
      const r = (await teamApi("POST", "/api/chat", { message: chatInput }, token)) as any;
      setChatOut(r.answer);
    } catch (e) {
      setChatOut(`⚠ ${String(e)}`);
    }
    setChatInput("");
  };

  return (
    <div className="flex h-full flex-col p-6">
      <div className="mb-4 flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Team Mode</h1>
        <span className={running ? "text-emerald-400" : "text-zinc-600"}>
          {running ? "● server live (0.0.0.0:11436)" : "○ server off"}
        </span>
      </div>

      {!running ? (
        <div className="max-w-md space-y-3 rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
          <p className="text-sm text-zinc-400">
            Start the LAN server to share RAGit with your team. Auth + RBAC are enforced
            (admin / editor / viewer). Anyone on the network can reach this port.
          </p>
          <button
            onClick={start}
            className="rounded-md bg-brand-fg px-3 py-2 text-sm font-medium text-white hover:bg-brand-fg/80"
          >
            Start Team Server
          </button>
        </div>
      ) : (
        <div className="grid flex-1 grid-cols-2 gap-4 overflow-auto">
          <div className="space-y-3 rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
            <h2 className="font-medium">Account</h2>
            <input
              placeholder="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
            />
            <input
              placeholder="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
            />
            <div className="flex gap-2">
              <button onClick={login} className="rounded-md bg-zinc-800 px-3 py-2 text-sm hover:bg-zinc-700">
                Login
              </button>
              <button onClick={register} className="rounded-md bg-zinc-800 px-3 py-2 text-sm hover:bg-zinc-700">
                Register
              </button>
              <button onClick={stop} className="rounded-md bg-red-900/60 px-3 py-2 text-sm hover:bg-red-900">
                Stop
              </button>
            </div>
            {token && <div className="text-xs text-emerald-400">session active · role: {myRole}</div>}

            {myRole === "admin" && (
              <div className="space-y-2 border-t border-zinc-800 pt-3">
                <h3 className="text-sm font-medium text-zinc-300">Users (admin)</h3>
                {users.map((u) => (
                  <div key={u.id} className="flex items-center justify-between text-sm">
                    <span>{u.username}</span>
                    <select
                      value={u.role}
                      onChange={(e) => setRole(u.id, e.target.value)}
                      className="rounded border border-zinc-700 bg-zinc-900 px-2 py-1 text-xs"
                    >
                      <option value="viewer">viewer</option>
                      <option value="editor">editor</option>
                      <option value="admin">admin</option>
                    </select>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="space-y-3 rounded-lg border border-zinc-800 bg-zinc-900/30 p-4">
            <h2 className="font-medium">Team Chat (RBAC-enforced)</h2>
            <div className="min-h-[120px] rounded bg-zinc-950 p-3 text-sm text-zinc-200">
              {chatOut || <span className="text-zinc-600">answers appear here…</span>}
            </div>
            <div className="flex gap-2">
              <input
                placeholder="Ask the shared library…"
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && sendChat()}
                className="flex-1 rounded-md border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
              />
              <button onClick={sendChat} className="rounded-md bg-brand-fg px-3 py-2 text-sm font-medium text-white">
                Send
              </button>
            </div>
          </div>
        </div>
      )}

      {msg && <div className="mt-3 text-xs text-zinc-500">{msg}</div>}
    </div>
  );
}

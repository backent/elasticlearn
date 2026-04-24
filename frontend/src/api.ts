const BASE = (import.meta.env.VITE_API_BASE as string | undefined) ?? "/api";

export interface Session {
  session_id: string;
  expires_at: string;
  ttl_seconds: number;
}

export interface IndexInfo {
  name: string;
  full_name: string;
  docs: string | null;
  size: string | null;
}

export interface UploadResult {
  index: string;
  doc_count: number;
  errors: number;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    credentials: "include",
    ...init,
    headers: {
      Accept: "application/json",
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const detail = await res.text();
    throw new ApiError(res.status, detail);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

export class ApiError extends Error {
  constructor(public status: number, public detail: string) {
    super(`HTTP ${status}: ${detail}`);
  }
}

export const api = {
  startSession: () =>
    request<Session>("/sessions", { method: "POST" }),

  currentSession: () => request<Session>("/sessions/me"),

  endSession: () =>
    request<void>("/sessions/me", { method: "DELETE" }),

  listIndices: () => request<IndexInfo[]>("/indices"),

  getMapping: (name: string) =>
    request<unknown>(`/mapping/${encodeURIComponent(name)}`),

  upload: async (
    index: string,
    format: "ndjson" | "csv",
    file: File,
  ): Promise<UploadResult> => {
    const form = new FormData();
    form.append("file", file);
    const qs = new URLSearchParams({ index, format });
    return request<UploadResult>(`/datasets?${qs.toString()}`, {
      method: "POST",
      body: form,
    });
  },

  search: (index: string, body: unknown) =>
    request<unknown>("/query", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ index, body }),
    }),
};

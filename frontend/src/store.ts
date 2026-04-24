import { create } from "zustand";
import { api, IndexInfo, Session } from "./api";

interface AppStore {
  session: Session | null;
  indices: IndexInfo[];
  activeIndex: string | null;
  results: unknown;
  error: string | null;
  loading: boolean;

  init: () => Promise<void>;
  startNew: () => Promise<void>;
  endSession: () => Promise<void>;
  refreshIndices: () => Promise<void>;
  setActive: (name: string | null) => void;
  setResults: (r: unknown) => void;
  setError: (e: string | null) => void;
  setLoading: (b: boolean) => void;
}

export const useStore = create<AppStore>((set, get) => ({
  session: null,
  indices: [],
  activeIndex: null,
  results: null,
  error: null,
  loading: false,

  init: async () => {
    try {
      const s = await api.currentSession();
      set({ session: s });
      await get().refreshIndices();
    } catch {
      const s = await api.startSession();
      set({ session: s, indices: [] });
    }
  },

  startNew: async () => {
    const s = await api.startSession();
    set({ session: s, indices: [], activeIndex: null, results: null, error: null });
  },

  endSession: async () => {
    await api.endSession().catch(() => {});
    set({ session: null, indices: [], activeIndex: null, results: null });
    await get().startNew();
  },

  refreshIndices: async () => {
    const indices = await api.listIndices();
    set({ indices });
    if (!get().activeIndex && indices.length > 0) {
      set({ activeIndex: indices[0].name });
    }
  },

  setActive: (name) => set({ activeIndex: name }),
  setResults: (r) => set({ results: r }),
  setError: (e) => set({ error: e }),
  setLoading: (b) => set({ loading: b }),
}));

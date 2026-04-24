import Editor from "@monaco-editor/react";
import { useState } from "react";
import { api, ApiError } from "../api";
import { useStore } from "../store";

const DEFAULT_QUERY = `{
  "query": { "match_all": {} },
  "size": 10
}`;

/** Monaco-backed JSON editor for the ES query body, plus a Run button. */
export function QueryEditor() {
  const [value, setValue] = useState(DEFAULT_QUERY);
  const activeIndex = useStore((s) => s.activeIndex);
  const setResults = useStore((s) => s.setResults);
  const setError = useStore((s) => s.setError);
  const setLoading = useStore((s) => s.setLoading);
  const loading = useStore((s) => s.loading);
  const session = useStore((s) => s.session);

  const expired =
    session &&
    new Date(session.expires_at).getTime() - Date.now() <= 0;

  async function run() {
    if (!activeIndex) {
      setError("Pick an index first.");
      return;
    }
    let body: unknown;
    try {
      body = JSON.parse(value);
    } catch (e) {
      setError(`Invalid JSON: ${String(e)}`);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const res = await api.search(activeIndex, body);
      setResults(res);
    } catch (e) {
      if (e instanceof ApiError) setError(e.detail);
      else setError(String(e));
      setResults(null);
    } finally {
      setLoading(false);
    }
  }

  return (
    <>
      <div className="editor-toolbar">
        <button
          className="primary"
          onClick={run}
          disabled={loading || expired || !activeIndex}
        >
          {loading ? "Running…" : "Run query"}
        </button>
        <span style={{ alignSelf: "center", color: "var(--muted)", fontSize: 12 }}>
          {activeIndex ? `POST /${activeIndex}/_search` : "select an index →"}
        </span>
      </div>
      <div className="editor-body">
        <Editor
          height="100%"
          defaultLanguage="json"
          theme="vs-dark"
          value={value}
          onChange={(v) => setValue(v ?? "")}
          options={{ minimap: { enabled: false }, fontSize: 13 }}
        />
      </div>
    </>
  );
}

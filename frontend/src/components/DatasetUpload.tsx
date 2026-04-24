import { useState } from "react";
import { api, ApiError } from "../api";
import { useStore } from "../store";

/** Upload a dataset file and register the new index in the sidebar. */
export function DatasetUpload() {
  const [indexName, setIndexName] = useState("movies");
  const [format, setFormat] = useState<"ndjson" | "csv">("ndjson");
  const [file, setFile] = useState<File | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshIndices = useStore((s) => s.refreshIndices);
  const setActive = useStore((s) => s.setActive);

  async function onUpload() {
    if (!file) return;
    setBusy(true);
    setMsg(null);
    try {
      const res = await api.upload(indexName, format, file);
      setMsg(`Indexed ${res.doc_count} docs (${res.errors} errors).`);
      await refreshIndices();
      setActive(res.index);
    } catch (e) {
      if (e instanceof ApiError) setMsg(`Error: ${e.detail}`);
      else setMsg(`Error: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="upload-box">
      <div style={{ fontWeight: 600, fontSize: 13 }}>Upload dataset</div>
      <input
        placeholder="index name (a-z0-9_-)"
        value={indexName}
        onChange={(e) => setIndexName(e.target.value.toLowerCase())}
      />
      <select value={format} onChange={(e) => setFormat(e.target.value as any)}>
        <option value="ndjson">NDJSON</option>
        <option value="csv">CSV</option>
      </select>
      <input
        type="file"
        accept=".ndjson,.json,.csv,.txt"
        onChange={(e) => setFile(e.target.files?.[0] ?? null)}
      />
      <button className="primary" onClick={onUpload} disabled={!file || busy}>
        {busy ? "Uploading…" : "Upload"}
      </button>
      {msg && <div style={{ fontSize: 12, color: "var(--muted)" }}>{msg}</div>}
    </div>
  );
}

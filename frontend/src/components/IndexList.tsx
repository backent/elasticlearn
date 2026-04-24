import { useStore } from "../store";

/** Sidebar list of indices owned by the current session. */
export function IndexList() {
  const indices = useStore((s) => s.indices);
  const active = useStore((s) => s.activeIndex);
  const setActive = useStore((s) => s.setActive);
  const refresh = useStore((s) => s.refreshIndices);

  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 6,
        }}
      >
        <div style={{ fontWeight: 600, fontSize: 13 }}>Indices</div>
        <button onClick={() => refresh()} title="Refresh">
          ⟳
        </button>
      </div>
      {indices.length === 0 ? (
        <div className="empty-state">No indices yet. Upload a dataset.</div>
      ) : (
        <div className="index-list">
          {indices.map((ix) => (
            <div
              key={ix.name}
              className={`index-item ${active === ix.name ? "active" : ""}`}
              onClick={() => setActive(ix.name)}
            >
              <span>{ix.name}</span>
              <span className="meta">
                {ix.docs ?? "–"} docs · {ix.size ?? "–"}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

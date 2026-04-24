import { useStore } from "../store";

/** Right-hand pane: pretty-prints the raw Elasticsearch response or error. */
export function ResultsPane() {
  const results = useStore((s) => s.results);
  const error = useStore((s) => s.error);

  if (error) {
    return (
      <pre style={{ color: "var(--danger)" }}>{error}</pre>
    );
  }
  if (!results) {
    return (
      <div className="empty-state">
        Run a query to see the raw Elasticsearch response here.
      </div>
    );
  }
  return <pre>{JSON.stringify(results, null, 2)}</pre>;
}

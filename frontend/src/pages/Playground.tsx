import { DatasetUpload } from "../components/DatasetUpload";
import { IndexList } from "../components/IndexList";
import { QueryEditor } from "../components/QueryEditor";
import { ResultsPane } from "../components/ResultsPane";
import { useStore } from "../store";

/** Main three-pane layout: sidebar | editor | results. */
export function Playground() {
  const session = useStore((s) => s.session);
  const expired =
    session &&
    new Date(session.expires_at).getTime() - Date.now() <= 0;

  return (
    <>
      {expired && (
        <div className="banner">
          Session ended. Start a new one to continue.
        </div>
      )}
      <div className="layout">
        <aside className="sidebar">
          <div className="panel-header">Workspace</div>
          <div className="sidebar-body">
            <DatasetUpload />
            <IndexList />
          </div>
        </aside>
        <section className="editor">
          <div className="panel-header">Query</div>
          <QueryEditor />
        </section>
        <section className="results">
          <div className="panel-header">Response</div>
          <ResultsPane />
        </section>
      </div>
    </>
  );
}

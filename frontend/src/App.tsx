import { useEffect } from "react";
import { Playground } from "./pages/Playground";
import { useStore } from "./store";
import { SessionTimer } from "./components/SessionTimer";

/** Root component: bootstraps a session on load. */
export default function App() {
  const init = useStore((s) => s.init);
  const session = useStore((s) => s.session);
  const startNew = useStore((s) => s.startNew);
  const endSession = useStore((s) => s.endSession);

  useEffect(() => {
    init();
  }, [init]);

  return (
    <div className="app">
      <div className="topbar">
        <h1>ElasticLearn</h1>
        <div className="right">
          <SessionTimer />
          {session && (
            <>
              <span title={session.session_id}>
                session {session.session_id.slice(0, 8)}…
              </span>
              <button onClick={() => endSession()} className="danger">
                End & restart
              </button>
            </>
          )}
          {!session && (
            <button className="primary" onClick={() => startNew()}>
              Start practice
            </button>
          )}
        </div>
      </div>
      <Playground />
    </div>
  );
}

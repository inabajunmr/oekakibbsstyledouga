import { EditorShell } from "../editor/EditorShell";
import { ProjectPanel } from "./ProjectPanel";

export function App() {
  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="topbar__group">
          <strong>oekakibbsstyledouga</strong>
          <span>frame paint editor skeleton</span>
        </div>
        <div className="topbar__group">
          <span>Tauri-first scaffold</span>
        </div>
      </header>
      <ProjectPanel />
      <EditorShell />
    </div>
  );
}

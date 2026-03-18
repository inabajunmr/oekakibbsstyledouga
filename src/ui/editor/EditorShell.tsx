import { CanvasStage } from "../canvas/CanvasStage";
import { Timeline } from "../timeline/Timeline";
import { useEditorStore } from "../../state/editor-store/useEditorStore";

const tools = ["Pen", "Fill", "Eyedropper"];
const swatches = ["#0a0908", "#d1495b", "#edae49", "#00798c", "#30638e"];

export function EditorShell() {
  const { activeTool, currentFrame, frameBundle, project, setActiveTool } =
    useEditorStore();

  return (
    <div className="workspace">
      <aside className="sidebar">
        <section className="sidebar__section">
          <h2 className="sidebar__title">Tools</h2>
          <div className="tool-grid">
            {tools.map((tool) => (
              <button
                key={tool}
                className={`tool-button${activeTool === tool ? " tool-button--active" : ""}`}
                onClick={() => setActiveTool(tool)}
                type="button"
              >
                {tool}
              </button>
            ))}
          </div>
        </section>
        <section className="sidebar__section">
          <h2 className="sidebar__title">Palette</h2>
          <div className="swatch-list">
            {swatches.map((color) => (
              <button
                key={color}
                className="swatch"
                style={{ background: color }}
                type="button"
                aria-label={color}
              />
            ))}
          </div>
        </section>
        <section className="sidebar__section">
          <h2 className="sidebar__title">Project API</h2>
          <div className="meta-list">
            <code>create_project(video_path, project_root)</code>
            <code>get_frame_bundle(project_root, frame_index)</code>
            <code>fill_region(project_root, frame, x, y, color)</code>
            <code>export_video(project_root, output_path)</code>
          </div>
        </section>
        <section className="sidebar__section">
          <h2 className="sidebar__title">Status</h2>
          <div className="meta-list">
            <code>tool: {activeTool}</code>
            <code>currentFrame: {currentFrame}</code>
            <code>project loaded: {project ? "yes" : "no"}</code>
            <code>source mode: {project?.sourceMode ?? "none"}</code>
            <code>line frame: {frameBundle?.lineFramePath ?? "none"}</code>
            <code>render mode: canvas-only</code>
          </div>
        </section>
      </aside>
      <div className="editor-layout">
        <CanvasStage />
        <Timeline />
      </div>
    </div>
  );
}

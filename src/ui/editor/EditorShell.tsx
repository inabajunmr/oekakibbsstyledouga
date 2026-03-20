import { useEffect } from "react";
import { CanvasStage } from "../canvas/CanvasStage";
import { Timeline } from "../timeline/Timeline";
import { useEditorStore } from "../../state/editor-store/useEditorStore";

const tools = ["Pen", "Fill", "Eyedropper"];

function toHex(value: number) {
  return value.toString(16).padStart(2, "0");
}

function colorToHex(r: number, g: number, b: number) {
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

export function EditorShell() {
  const {
    activeTool,
    currentFrame,
    frameBundle,
    isPlaying,
    project,
    setActiveTool,
    setCurrentFrame,
    selectedColor,
    setIsPlaying,
    setSelectedColor
  } =
    useEditorStore();

  const colorHex = colorToHex(selectedColor.r, selectedColor.g, selectedColor.b);

  useEffect(() => {
    if (!isPlaying || !project) {
      return;
    }

    const fps = project.fps > 0 ? project.fps : 24;
    const timer = window.setInterval(() => {
      setCurrentFrame((currentFrame + 1) % Math.max(project.frameCount, 1));
    }, Math.max(16, Math.round(1000 / fps)));

    return () => {
      window.clearInterval(timer);
    };
  }, [currentFrame, isPlaying, project, setCurrentFrame]);

  return (
    <div className="workspace">
      <aside className="sidebar">
        <section className="sidebar__section">
          <h2 className="sidebar__title">Playback</h2>
          <div className="tool-grid">
            <button className="tool-button" onClick={() => setIsPlaying(!isPlaying)} type="button">
              {isPlaying ? "Pause" : "Play"}
            </button>
            <button
              className="tool-button"
              onClick={() => setCurrentFrame(0)}
              type="button"
            >
              To Start
            </button>
          </div>
        </section>
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
          <div className="color-picker">
            <label className="color-picker__field">
              <span>Custom Color</span>
              <input
                className="color-picker__input"
                type="color"
                value={colorHex}
                onChange={(event) => {
                  const value = event.target.value;
                  setSelectedColor({
                    ...selectedColor,
                    r: Number.parseInt(value.slice(1, 3), 16),
                    g: Number.parseInt(value.slice(3, 5), 16),
                    b: Number.parseInt(value.slice(5, 7), 16)
                  });
                }}
              />
            </label>
            <label className="color-picker__field">
              <span>Opacity</span>
              <div className="color-picker__alpha">
                <input
                  className="color-picker__slider"
                  type="range"
                  min="0"
                  max="255"
                  value={selectedColor.a}
                  onChange={(event) => {
                    setSelectedColor({
                      ...selectedColor,
                      a: Number.parseInt(event.target.value, 10)
                    });
                  }}
                />
                <span>{selectedColor.a}</span>
              </div>
            </label>
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
            <code>isPlaying: {isPlaying ? "yes" : "no"}</code>
            <code>currentFrame: {currentFrame}</code>
            <code>project loaded: {project ? "yes" : "no"}</code>
            <code>source mode: {project?.sourceMode ?? "none"}</code>
            <code>color: rgba({selectedColor.r}, {selectedColor.g}, {selectedColor.b}, {selectedColor.a})</code>
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

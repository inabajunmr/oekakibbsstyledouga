import { useEffect } from "react";
import { CanvasStage } from "../canvas/CanvasStage";
import { Timeline } from "../timeline/Timeline";
import { useEditorStore } from "../../state/editor-store/useEditorStore";

const tools = ["Pen", "Fill", "Eyedropper"];
const swatches = [
  { hex: "#0a0908", rgba: { r: 10, g: 9, b: 8, a: 255 } },
  { hex: "#d1495b", rgba: { r: 209, g: 73, b: 91, a: 255 } },
  { hex: "#edae49", rgba: { r: 237, g: 174, b: 73, a: 255 } },
  { hex: "#00798c", rgba: { r: 0, g: 121, b: 140, a: 255 } },
  { hex: "#30638e", rgba: { r: 48, g: 99, b: 142, a: 255 } }
];

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
          <div className="swatch-list">
            {swatches.map((swatch) => (
              <button
                key={swatch.hex}
                className={`swatch${selectedColor.r === swatch.rgba.r &&
                selectedColor.g === swatch.rgba.g &&
                selectedColor.b === swatch.rgba.b
                  ? " swatch--active"
                  : ""}`}
                onClick={() => setSelectedColor(swatch.rgba)}
                style={{ background: swatch.hex }}
                type="button"
                aria-label={swatch.hex}
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

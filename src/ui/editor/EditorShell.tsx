import { useEffect } from "react";
import {
  getPaintedFrames,
  redoPaint,
  undoPaint
} from "../../infra/tauri-api/client";
import { CanvasStage } from "../canvas/CanvasStage";
import { Timeline } from "../timeline/Timeline";
import { isTauriApp } from "../../infra/tauri-api/platform";
import { useEditorStore } from "../../state/editor-store/useEditorStore";

const tools = ["Pen", "Fill"];

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
    canRedo,
    canUndo,
    markFramesPainted,
    setActiveTool,
    setCurrentFrame,
    setHistoryState,
    syncPaintedFrames,
    setStatusMessage,
    selectedColor,
    setIsPlaying,
    setSelectedColor
  } =
    useEditorStore();

  const colorHex = colorToHex(selectedColor.r, selectedColor.g, selectedColor.b);

  async function applyHistoryAction(action: "undo" | "redo") {
    if (!project || !isTauriApp()) {
      return;
    }

    try {
      const result =
        action === "undo"
          ? await undoPaint(project.projectRoot)
          : await redoPaint(project.projectRoot);
      markFramesPainted(result.updatedFrames);
      setHistoryState(result.canUndo, result.canRedo);
      syncPaintedFrames(await getPaintedFrames(project.projectRoot));
      setStatusMessage(
        result.updatedFrames.length > 0
          ? `${action === "undo" ? "Undid" : "Redid"} ${result.updatedFrames.length} frame update(s)`
          : `${action === "undo" ? "Undo" : "Redo"} skipped`
      );
    } catch (error) {
      setStatusMessage(`${action === "undo" ? "Undo" : "Redo"} failed: ${String(error)}`);
    }
  }

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

  useEffect(() => {
    if (!project || !isTauriApp()) {
      return;
    }

    function isEditableTarget(target: EventTarget | null) {
      return (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement
      );
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (isEditableTarget(event.target)) {
        return;
      }

      const acceleratorPressed = event.metaKey || event.ctrlKey;
      if (!acceleratorPressed) {
        return;
      }

      const key = event.key.toLowerCase();
      const wantsUndo = key === "z" && !event.shiftKey;
      const wantsRedo = (key === "z" && event.shiftKey) || key === "y";

      if (wantsUndo && canUndo) {
        event.preventDefault();
        void applyHistoryAction("undo");
      } else if (wantsRedo && canRedo) {
        event.preventDefault();
        void applyHistoryAction("redo");
      }
    }

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [canRedo, canUndo, project]);

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
          <h2 className="sidebar__title">History</h2>
          <div className="tool-grid">
            <button
              className="tool-button"
              disabled={!project || !canUndo}
              onClick={() => {
                void applyHistoryAction("undo");
              }}
              type="button"
            >
              Undo
            </button>
            <button
              className="tool-button"
              disabled={!project || !canRedo}
              onClick={() => {
                void applyHistoryAction("redo");
              }}
              type="button"
            >
              Redo
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
            <code>canUndo: {canUndo ? "yes" : "no"}</code>
            <code>canRedo: {canRedo ? "yes" : "no"}</code>
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

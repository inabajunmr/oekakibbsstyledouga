import { useEffect, useState } from "react";
import {
  open as openDialog,
  save as saveDialog
} from "@tauri-apps/plugin-dialog";
import {
  createProject,
  ensureFfmpegTools,
  exportVideo,
  getFrameBundle,
  getPaintedFrames,
  openProject,
  preprocessProject
} from "../../infra/tauri-api/client";
import { isTauriApp } from "../../infra/tauri-api/platform";
import { useEditorStore } from "../../state/editor-store/useEditorStore";

const demoVideoPath = "test.mp4";
const demoProjectRoot = "/Users/juninaba/Desktop/test";
const demoOutputPath = "/Users/juninaba/Desktop/test/test.mp4";

export function ProjectPanel() {
  const {
    currentFrame,
    project,
    setPaintedFrames,
    setFrameBundle,
    setProject,
    setStatusMessage,
    statusMessage
  } = useEditorStore();
  const [videoPath, setVideoPath] = useState(demoVideoPath);
  const [projectRoot, setProjectRoot] = useState(demoProjectRoot);
  const [outputPath, setOutputPath] = useState(demoOutputPath);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isTauriApp()) {
      return;
    }

    let cancelled = false;

    void ensureFfmpegTools()
      .then((result) => {
        if (cancelled) {
          return;
        }

        if (result.source === "downloaded") {
          setStatusMessage(`Downloaded ffmpeg tools to ${result.toolDir}`);
          return;
        }

        if (result.source === "managed") {
          setStatusMessage(`Using managed ffmpeg tools from ${result.toolDir}`);
          return;
        }

        setStatusMessage(`Using system ffmpeg tools from ${result.toolDir}`);
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }

        setStatusMessage(`ffmpeg setup skipped: ${String(error)}`);
      });

    return () => {
      cancelled = true;
    };
  }, [setStatusMessage]);

  async function pickVideoFile() {
    if (!isTauriApp()) {
      setStatusMessage("Video picker is available in Tauri runtime only.");
      return;
    }

    try {
      const selection = await openDialog({
        multiple: false,
        filters: [
          {
            name: "Video",
            extensions: ["mp4", "mov", "m4v"]
          }
        ]
      });

      if (typeof selection === "string") {
        setVideoPath(selection);
        setStatusMessage(`Selected video: ${selection}`);
      }
    } catch (error) {
      setStatusMessage(`Video picker failed: ${String(error)}`);
    }
  }

  async function pickProjectFolder() {
    if (!isTauriApp()) {
      setStatusMessage("Project folder picker is available in Tauri runtime only.");
      return;
    }

    try {
      const selection = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose project folder"
      });

      if (typeof selection === "string") {
        setProjectRoot(selection);
        setStatusMessage(`Selected project folder: ${selection}`);
      }
    } catch (error) {
      setStatusMessage(`Project folder picker failed: ${String(error)}`);
    }
  }

  async function pickOutputFile() {
    if (!isTauriApp()) {
      setStatusMessage("Output picker is available in Tauri runtime only.");
      return;
    }

    try {
      const selection = await saveDialog({
        defaultPath: outputPath,
        filters: [
          {
            name: "MP4",
            extensions: ["mp4"]
          }
        ],
        title: "Choose mp4 output path"
      });

      if (typeof selection === "string") {
        setOutputPath(selection);
        setStatusMessage(`Selected output path: ${selection}`);
      }
    } catch (error) {
      setStatusMessage(`Output picker failed: ${String(error)}`);
    }
  }

  async function handleCreate() {
    if (!isTauriApp()) {
      setStatusMessage("Create project is available in Tauri runtime only.");
      return;
    }

    setProject(null);
    setFrameBundle(null);
    setBusy(true);
    setStatusMessage("Creating project...");

    try {
      const startedAt = performance.now();
      const nextProject = await createProject(videoPath, projectRoot);
      setStatusMessage(
        `Created project metadata in ${(performance.now() - startedAt).toFixed(0)}ms, preprocessing fill data...`
      );
      setProject(nextProject);
      const preprocessStartedAt = performance.now();
      await preprocessProject(nextProject.projectRoot);
      setStatusMessage(
        `Preprocessed fill data in ${(performance.now() - preprocessStartedAt).toFixed(0)}ms, loading frames...`
      );
      const paintedFramesStartedAt = performance.now();
      const paintedFrames = await getPaintedFrames(nextProject.projectRoot);
      setPaintedFrames(paintedFrames);
      setStatusMessage(
        `Loaded painted state in ${(performance.now() - paintedFramesStartedAt).toFixed(0)}ms, opening frame 0...`
      );
      const frameBundleStartedAt = performance.now();
      const bundle = await getFrameBundle(nextProject.projectRoot, currentFrame);
      setFrameBundle(bundle);
      setStatusMessage(
        nextProject.sourceMode === "video"
          ? `Created project at ${nextProject.projectRoot} in ${(performance.now() - startedAt).toFixed(0)}ms`
          : `Created placeholder project at ${nextProject.projectRoot} because ffmpeg/ffprobe were not available`
      );
      console.info("create_project_timing", {
        createProjectMs: Math.round(preprocessStartedAt - startedAt),
        preprocessProjectMs: Math.round(paintedFramesStartedAt - preprocessStartedAt),
        getPaintedFramesMs: Math.round(frameBundleStartedAt - paintedFramesStartedAt),
        getFrameBundleMs: Math.round(performance.now() - frameBundleStartedAt),
        totalMs: Math.round(performance.now() - startedAt)
      });
    } catch (error) {
      setStatusMessage(`Create project failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleOpen() {
    if (!isTauriApp()) {
      setStatusMessage("Open project is available in Tauri runtime only.");
      return;
    }

    setProject(null);
    setFrameBundle(null);
    setBusy(true);
    setStatusMessage("Opening project...");

    try {
      const startedAt = performance.now();
      const nextProject = await openProject(projectRoot);
      setProject(nextProject);
      setStatusMessage(
        `Opened project metadata in ${(performance.now() - startedAt).toFixed(0)}ms, preprocessing fill data...`
      );
      const preprocessStartedAt = performance.now();
      await preprocessProject(nextProject.projectRoot);
      setStatusMessage(
        `Preprocessed fill data in ${(performance.now() - preprocessStartedAt).toFixed(0)}ms, loading frames...`
      );
      const paintedFramesStartedAt = performance.now();
      const paintedFrames = await getPaintedFrames(nextProject.projectRoot);
      setPaintedFrames(paintedFrames);
      setStatusMessage(
        `Loaded painted state in ${(performance.now() - paintedFramesStartedAt).toFixed(0)}ms, opening frame 0...`
      );
      const frameBundleStartedAt = performance.now();
      const bundle = await getFrameBundle(nextProject.projectRoot, currentFrame);
      setFrameBundle(bundle);
      setStatusMessage(
        nextProject.sourceMode === "video"
          ? `Opened project at ${nextProject.projectRoot} in ${(performance.now() - startedAt).toFixed(0)}ms`
          : `Opened placeholder project at ${nextProject.projectRoot}`
      );
      console.info("open_project_timing", {
        openProjectMs: Math.round(preprocessStartedAt - startedAt),
        preprocessProjectMs: Math.round(paintedFramesStartedAt - preprocessStartedAt),
        getPaintedFramesMs: Math.round(frameBundleStartedAt - paintedFramesStartedAt),
        getFrameBundleMs: Math.round(performance.now() - frameBundleStartedAt),
        totalMs: Math.round(performance.now() - startedAt)
      });
    } catch (error) {
      setStatusMessage(`Open project failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleExport() {
    if (!isTauriApp() || !project) {
      setStatusMessage("Export is available in Tauri runtime with a loaded project.");
      return;
    }

    setBusy(true);
    setStatusMessage("Exporting mp4...");

    try {
      const result = await exportVideo(project.projectRoot, outputPath);
      setStatusMessage(
        `Exported ${result.frameCount} frame(s) to ${result.outputPath}`
      );
    } catch (error) {
      setStatusMessage(`Export failed: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="project-panel">
      <div className="project-panel__fields">
        <label className="project-panel__field">
          <span>Video Path</span>
          <div className="project-panel__row">
            <input
              value={videoPath}
              onChange={(event) => setVideoPath(event.target.value)}
              type="text"
            />
            <button className="project-panel__browse" onClick={pickVideoFile} type="button">
              Browse
            </button>
          </div>
        </label>
        <label className="project-panel__field">
          <span>Project Folder</span>
          <div className="project-panel__row">
            <input
              value={projectRoot}
              onChange={(event) => setProjectRoot(event.target.value)}
              type="text"
            />
            <button className="project-panel__browse" onClick={pickProjectFolder} type="button">
              Browse
            </button>
          </div>
        </label>
        <label className="project-panel__field">
          <span>Output Path</span>
          <div className="project-panel__row">
            <input
              value={outputPath}
              onChange={(event) => setOutputPath(event.target.value)}
              type="text"
            />
            <button className="project-panel__browse" onClick={pickOutputFile} type="button">
              Browse
            </button>
          </div>
        </label>
      </div>
      <div className="project-panel__actions">
        <button className="topbar__button" disabled={busy} onClick={handleCreate} type="button">
          Create Project
        </button>
        <button className="topbar__button" disabled={busy} onClick={handleOpen} type="button">
          Open Project
        </button>
        <button className="topbar__button" disabled={busy || !project} onClick={handleExport} type="button">
          Export MP4
        </button>
      </div>
      <div className="project-panel__status">
        <div>{statusMessage}</div>
        <div>{project ? `Loaded: ${project.projectRoot}` : "Loaded: none"}</div>
      </div>
    </section>
  );
}

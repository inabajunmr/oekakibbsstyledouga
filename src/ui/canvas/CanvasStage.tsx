import { useEffect, useRef, useState } from "react";
import { drawStroke, fillRegion, getFrameBundle } from "../../infra/tauri-api/client";
import { toAssetUrl } from "../../infra/tauri-api/assets";
import { isTauriApp } from "../../infra/tauri-api/platform";
import { useEditorStore } from "../../state/editor-store/useEditorStore";
import type { StrokeInput } from "../../infra/tauri-api/types";

type ScreenPoint = {
  x: number;
  y: number;
};

export function CanvasStage() {
  const {
    activeTool,
    currentFrame,
    frameBundle,
    paintRevisions,
    project,
    markFramesPainted,
    selectedColor,
    setHistoryState,
    setRecentFillFrames,
    setFrameBundle,
    setStatusMessage,
    updateFrameBundle,
    zoom
  } = useEditorStore();
  const [frameError, setFrameError] = useState<string | null>(null);
  const [localStrokes, setLocalStrokes] = useState<Record<number, StrokeInput[]>>({});
  const [draftPoints, setDraftPoints] = useState<ScreenPoint[]>([]);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef<HTMLDivElement | null>(null);
  const isDrawingRef = useRef(false);

  useEffect(() => {
    if (!project || !isTauriApp()) {
      return;
    }

    let cancelled = false;

    void getFrameBundle(project.projectRoot, currentFrame)
      .then((bundle) => {
        if (cancelled) {
          return;
        }

        setFrameBundle(bundle);
        setFrameError(null);
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }

        setFrameError(String(error));
        setStatusMessage(`Frame load failed: ${String(error)}`);
      });

    return () => {
      cancelled = true;
    };
  }, [currentFrame, project, setFrameBundle, setStatusMessage]);

  const lineFrameUrl = toAssetUrl(frameBundle?.lineFramePath);
  const paintFrameBaseUrl = toAssetUrl(frameBundle?.paintFramePath);
  const paintFrameUrl = paintFrameBaseUrl
    ? `${paintFrameBaseUrl}?v=${paintRevisions[currentFrame] ?? 0}`
    : undefined;
  const strokesForFrame = localStrokes[currentFrame] ?? [];

  useEffect(() => {
    const frame = frameRef.current;
    const canvas = canvasRef.current;

    if (!frame || !canvas) {
      return;
    }

    const frameElement = frame;
    const canvasElement = canvas;

    function resizeCanvas() {
      const bounds = frameElement.getBoundingClientRect();
      canvasElement.width = Math.max(1, Math.floor(bounds.width));
      canvasElement.height = Math.max(1, Math.floor(bounds.height));
    }

    resizeCanvas();

    const observer = new ResizeObserver(() => {
      resizeCanvas();
    });
    observer.observe(frameElement);

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;

    if (!canvas) {
      return;
    }

    const context = canvas.getContext("2d");

    if (!context) {
      return;
    }

    const drawingContext = context;
    const lineWidth = frameBundle?.width ?? project?.width ?? 1;
    const lineHeight = frameBundle?.height ?? project?.height ?? 1;

    drawingContext.clearRect(0, 0, canvas.width, canvas.height);
    drawingContext.lineCap = "round";
    drawingContext.lineJoin = "round";
    drawingContext.strokeStyle = `rgba(${selectedColor.r}, ${selectedColor.g}, ${selectedColor.b}, ${selectedColor.a / 255})`;
    drawingContext.lineWidth = 4;

    function drawPath(points: ScreenPoint[]) {
      if (points.length === 0) {
        return;
      }

      drawingContext.beginPath();
      drawingContext.moveTo(points[0].x, points[0].y);

      for (const point of points.slice(1)) {
        drawingContext.lineTo(point.x, point.y);
      }

      if (points.length === 1) {
        drawingContext.lineTo(points[0].x + 0.01, points[0].y + 0.01);
      }

      drawingContext.stroke();
    }

    for (const stroke of strokesForFrame) {
      const projected = stroke.points.map((point) => ({
        x: (point.x / lineWidth) * canvas.width,
        y: (point.y / lineHeight) * canvas.height
      }));
      drawPath(projected);
    }

    drawPath(draftPoints);
  }, [draftPoints, frameBundle, project, selectedColor, strokesForFrame]);

  function projectPoint(event: React.PointerEvent<HTMLCanvasElement>) {
    const canvas = canvasRef.current;

    if (!canvas || !project) {
      return null;
    }

    const bounds = canvas.getBoundingClientRect();
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;

    return {
      screen: { x, y },
      image: {
        x: Math.max(0, Math.min(project.width, (x / bounds.width) * project.width)),
        y: Math.max(0, Math.min(project.height, (y / bounds.height) * project.height))
      }
    };
  }

  async function commitStroke(points: ScreenPoint[], imagePoints: StrokeInput["points"]) {
    if (!project || imagePoints.length === 0) {
      return;
    }

    const stroke: StrokeInput = {
      color: selectedColor,
      size: 4,
      points: imagePoints
    };

    if (!isTauriApp()) {
      setLocalStrokes((current) => ({
        ...current,
        [currentFrame]: [...(current[currentFrame] ?? []), stroke]
      }));
      setStatusMessage("Stroke preview updated in browser mode.");
      return;
    }

    try {
      const result = await drawStroke(project.projectRoot, currentFrame, stroke);
      markFramesPainted([currentFrame]);
      setHistoryState(result.canUndo, result.canRedo);
      setRecentFillFrames([currentFrame]);
      updateFrameBundle((currentBundle) =>
        currentBundle && currentBundle.frameIndex === currentFrame
          ? {
              ...currentBundle,
              paintFramePath: result.updatedPaintFramePath
            }
          : currentBundle
      );
      setStatusMessage(`Saved stroke for frame ${currentFrame}`);
    } catch (error) {
      setStatusMessage(`Draw stroke failed: ${String(error)}`);
      setFrameError(String(error));
    }
  }

  async function commitFill(x: number, y: number) {
    if (!project) {
      return;
    }

    if (!isTauriApp()) {
      setStatusMessage("Fill preview is available in Tauri runtime only.");
      return;
    }

    try {
      const result = await fillRegion(project.projectRoot, currentFrame, x, y, selectedColor);

      if (result.updatedFrames.length > 0) {
        markFramesPainted(result.updatedFrames);
        setRecentFillFrames(result.updatedFrames);
        setHistoryState(result.canUndo, result.canRedo);
        setStatusMessage(
          `Filled ${result.updatedFrames.length} frame(s) on track ${result.trackId}`
        );
      } else {
        setHistoryState(result.canUndo, result.canRedo);
        setRecentFillFrames([]);
        setStatusMessage("Fill skipped because the clicked point is already filled or blocked.");
      }
    } catch (error) {
      setStatusMessage(`Fill failed: ${String(error)}`);
      setFrameError(String(error));
    }
  }

  function handlePointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    if (!project) {
      return;
    }

    const point = projectPoint(event);

    if (!point) {
      return;
    }

    if (activeTool === "Fill") {
      void commitFill(Math.round(point.image.x), Math.round(point.image.y));
      return;
    }

    if (activeTool !== "Pen") {
      return;
    }

    isDrawingRef.current = true;
    setDraftPoints([point.screen]);
    (event.currentTarget as HTMLCanvasElement).setPointerCapture(event.pointerId);
    (event.currentTarget as HTMLCanvasElement).dataset.strokePoints = JSON.stringify([
      point.image
    ]);
  }

  function handlePointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
    if (!isDrawingRef.current) {
      return;
    }

    const point = projectPoint(event);

    if (!point) {
      return;
    }

    setDraftPoints((current) => [...current, point.screen]);
    const currentPoints = JSON.parse(
      (event.currentTarget as HTMLCanvasElement).dataset.strokePoints ?? "[]"
    ) as StrokeInput["points"];
    currentPoints.push(point.image);
    (event.currentTarget as HTMLCanvasElement).dataset.strokePoints =
      JSON.stringify(currentPoints);
  }

  async function finishPointer(event: React.PointerEvent<HTMLCanvasElement>) {
    if (!isDrawingRef.current) {
      return;
    }

    isDrawingRef.current = false;
    const imagePoints = JSON.parse(
      (event.currentTarget as HTMLCanvasElement).dataset.strokePoints ?? "[]"
    ) as StrokeInput["points"];
    (event.currentTarget as HTMLCanvasElement).dataset.strokePoints = "[]";
    const points = draftPoints;
    setDraftPoints([]);
    await commitStroke(points, imagePoints);
  }

  return (
    <section className="canvas-stage">
      <div className="canvas-frame" ref={frameRef}>
        {lineFrameUrl ? (
          <img
            alt={`frame ${currentFrame}`}
            className="canvas-frame__image"
            onError={() => {
              setFrameError(
                `Failed to load line frame image: ${frameBundle?.lineFramePath ?? "unknown path"}`
              );
            }}
            onLoad={() => {
              setFrameError(null);
            }}
            src={lineFrameUrl}
          />
        ) : (
          <div className="canvas-frame__empty">
            {project ? "No frame image yet" : "Create or open a project"}
          </div>
        )}
        {paintFrameUrl ? (
          <img
            alt={`paint ${currentFrame}`}
            className="canvas-frame__paint-image"
            onError={() => {
              setFrameError(
                `Failed to load paint frame image: ${frameBundle?.paintFramePath ?? "unknown path"}`
              );
            }}
            src={paintFrameUrl}
          />
        ) : null}
        <canvas
          className={`canvas-frame__paint${activeTool === "Pen" ? " canvas-frame__paint--active" : ""}`}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={finishPointer}
          onPointerLeave={finishPointer}
          ref={canvasRef}
        />
        <div className="canvas-frame__overlay">
          frame {currentFrame} / zoom {zoom.toFixed(2)}x /{" "}
          {project ? `${project.width}x${project.height}` : "no project"}
        </div>
        {frameError ? <div className="canvas-frame__error">{frameError}</div> : null}
      </div>
    </section>
  );
}

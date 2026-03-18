import { useEditorStore } from "../../state/editor-store/useEditorStore";

export function Timeline() {
  const { currentFrame, paintedFrames, project, recentFillFrames, setCurrentFrame } =
    useEditorStore();
  const frameCount = Math.max(project?.frameCount ?? 0, 1);
  const visibleCount = Math.min(frameCount, 12);
  const start = Math.max(0, currentFrame - 5);
  const frames = Array.from({ length: visibleCount }, (_, index) => start + index).filter(
    (frame) => frame < frameCount
  );

  return (
    <section className="timeline">
      <div className="timeline__track">
        {frames.map((frame) => (
          <button
            key={frame}
            className={`timeline__frame${currentFrame === frame ? " timeline__frame--active" : ""}`}
            onClick={() => setCurrentFrame(frame)}
            type="button"
          >
            <div className="timeline__thumb" />
            <div>Frame {frame}</div>
            {paintedFrames[frame] ? <div className="timeline__badge">painted</div> : null}
            {recentFillFrames[frame] ? <div className="timeline__badge timeline__badge--recent">track</div> : null}
          </button>
        ))}
      </div>
    </section>
  );
}

import type { FrameBundle, ProjectSummary } from "../../infra/tauri-api/types";

export type EditorState = {
  activeTool: string;
  currentFrame: number;
  isPlaying: boolean;
  zoom: number;
  project: ProjectSummary | null;
  frameBundle: FrameBundle | null;
  paintRevisions: Record<number, number>;
  paintedFrames: Record<number, boolean>;
  recentFillFrames: Record<number, boolean>;
  statusMessage: string;
};

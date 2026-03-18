import type { FrameBundle, ProjectSummary } from "../../infra/tauri-api/types";

export type EditorColor = {
  r: number;
  g: number;
  b: number;
  a: number;
};

export type EditorState = {
  activeTool: string;
  currentFrame: number;
  isPlaying: boolean;
  zoom: number;
  selectedColor: EditorColor;
  project: ProjectSummary | null;
  frameBundle: FrameBundle | null;
  paintRevisions: Record<number, number>;
  paintedFrames: Record<number, boolean>;
  recentFillFrames: Record<number, boolean>;
  statusMessage: string;
};

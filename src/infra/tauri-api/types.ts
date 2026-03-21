export type ProjectSummary = {
  version: number;
  projectRoot: string;
  sourceVideoPath: string;
  fps: number;
  width: number;
  height: number;
  frameCount: number;
  sourceMode: string;
};

export type SaveResult = {
  frameIndex: number;
  updatedPaintFramePath: string;
  canUndo: boolean;
  canRedo: boolean;
};

export type ExportResult = {
  outputPath: string;
  frameCount: number;
};

export type ToolSetupResult = {
  toolDir: string;
  source: string;
  downloaded: boolean;
};

export type PreprocessResult = {
  frameCount: number;
  lineFramesDir: string;
  thumbFramesDir: string;
};

export type FrameBundle = {
  frameIndex: number;
  lineFramePath: string;
  paintFramePath: string;
  prevFramePath?: string;
  nextFramePath?: string;
  thumbnailPath?: string;
  width: number;
  height: number;
};

export type StrokeInput = {
  color: { r: number; g: number; b: number; a: number };
  size: number;
  points: Array<{ x: number; y: number }>;
};

export type FillResult = {
  trackId: number;
  updatedFrames: number[];
  canUndo: boolean;
  canRedo: boolean;
};

export type HistoryApplyResult = {
  updatedFrames: number[];
  canUndo: boolean;
  canRedo: boolean;
};

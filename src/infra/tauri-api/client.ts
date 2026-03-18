import { invoke } from "@tauri-apps/api/core";
import type {
  ExportResult,
  FillResult,
  FrameBundle,
  PreprocessResult,
  ProjectSummary,
  SaveResult,
  StrokeInput,
  ToolSetupResult
} from "./types";

export async function ensureFfmpegTools() {
  return invoke<ToolSetupResult>("ensure_ffmpeg_tools");
}

export async function createProject(videoPath: string, projectRoot: string) {
  return invoke<ProjectSummary>("create_project", {
    videoPath,
    projectRoot
  });
}

export async function openProject(projectRoot: string) {
  return invoke<ProjectSummary>("open_project", { projectRoot });
}

export async function getPaintedFrames(projectRoot: string) {
  return invoke<number[]>("get_painted_frames", { projectRoot });
}

export async function preprocessProject(projectRoot: string) {
  return invoke<PreprocessResult>("preprocess_project", { projectRoot });
}

export async function getFrameBundle(projectRoot: string, frameIndex: number) {
  return invoke<FrameBundle>("get_frame_bundle", {
    projectRoot,
    frameIndex
  });
}

export async function drawStroke(
  projectRoot: string,
  frameIndex: number,
  stroke: StrokeInput
) {
  return invoke<SaveResult>("draw_stroke", {
    projectRoot,
    frameIndex,
    stroke
  });
}

export async function fillRegion(
  projectRoot: string,
  frameIndex: number,
  x: number,
  y: number,
  color: { r: number; g: number; b: number; a: number }
) {
  return invoke<FillResult>("fill_region", {
    projectRoot,
    frameIndex,
    x,
    y,
    color
  });
}

export async function exportVideo(projectRoot: string, outputPath: string) {
  return invoke<ExportResult>("export_video", {
    projectRoot,
    outputPath
  });
}

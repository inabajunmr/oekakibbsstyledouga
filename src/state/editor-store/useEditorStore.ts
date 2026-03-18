import { useSyncExternalStore } from "react";
import type { FrameBundle, ProjectSummary } from "../../infra/tauri-api/types";
import type { EditorColor, EditorState } from "./types";

type Listener = () => void;

let state: EditorState = {
  activeTool: "Pen",
  currentFrame: 0,
  isPlaying: false,
  zoom: 1,
  selectedColor: { r: 209, g: 73, b: 91, a: 255 },
  project: null,
  frameBundle: null,
  paintRevisions: {},
  paintedFrames: {},
  recentFillFrames: {},
  statusMessage: "No project loaded"
};

const listeners = new Set<Listener>();

function emit() {
  listeners.forEach((listener) => listener());
}

function subscribe(listener: Listener) {
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return state;
}

const actions = {
  setActiveTool(activeTool: string) {
    state = {
      ...state,
      activeTool
    };
    emit();
  },
  setCurrentFrame(currentFrame: number | ((currentFrame: number) => number)) {
    state = {
      ...state,
      currentFrame:
        typeof currentFrame === "function"
          ? currentFrame(state.currentFrame)
          : currentFrame
    };
    emit();
  },
  setZoom(zoom: number) {
    state = {
      ...state,
      zoom
    };
    emit();
  },
  setSelectedColor(selectedColor: EditorColor) {
    state = {
      ...state,
      selectedColor
    };
    emit();
  },
  setIsPlaying(isPlaying: boolean) {
    state = {
      ...state,
      isPlaying
    };
    emit();
  },
  setProject(project: ProjectSummary | null) {
    state = {
      ...state,
      currentFrame: 0,
      isPlaying: false,
      project,
      frameBundle: null,
      paintRevisions: {},
      paintedFrames: {},
      recentFillFrames: {}
    };
    emit();
  },
  setFrameBundle(frameBundle: FrameBundle | null) {
    state = {
      ...state,
      frameBundle
    };
    emit();
  },
  updateFrameBundle(
    updater: (frameBundle: FrameBundle | null) => FrameBundle | null
  ) {
    state = {
      ...state,
      frameBundle: updater(state.frameBundle)
    };
    emit();
  },
  markFramesPainted(frames: number[]) {
    const paintRevisions = { ...state.paintRevisions };
    const paintedFrames = { ...state.paintedFrames };

    for (const frame of frames) {
      paintRevisions[frame] = (paintRevisions[frame] ?? 0) + 1;
      paintedFrames[frame] = true;
    }

    state = {
      ...state,
      paintRevisions,
      paintedFrames
    };
    emit();
  },
  setPaintedFrames(frames: number[]) {
    const paintedFrames: Record<number, boolean> = {};
    const paintRevisions: Record<number, number> = {};

    for (const frame of frames) {
      paintedFrames[frame] = true;
      paintRevisions[frame] = 1;
    }

    state = {
      ...state,
      paintedFrames,
      paintRevisions
    };
    emit();
  },
  setRecentFillFrames(frames: number[]) {
    const recentFillFrames: Record<number, boolean> = {};

    for (const frame of frames) {
      recentFillFrames[frame] = true;
    }

    state = {
      ...state,
      recentFillFrames
    };
    emit();
  },
  setStatusMessage(statusMessage: string) {
    state = {
      ...state,
      statusMessage
    };
    emit();
  }
};

export function useEditorStore() {
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    ...snapshot,
    ...actions
  };
}

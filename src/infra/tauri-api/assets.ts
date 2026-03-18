import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriApp } from "./platform";

export function toAssetUrl(filePath?: string) {
  if (!filePath) {
    return undefined;
  }

  if (isTauriApp()) {
    return convertFileSrc(filePath);
  }

  return undefined;
}


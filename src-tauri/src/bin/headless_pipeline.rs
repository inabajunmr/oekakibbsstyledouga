use std::{
    env, fs,
    path::{Path, PathBuf},
};

use oekakibbsstyledouga::{
    create_project_headless, export_video_headless, fill_region_headless,
    preprocess_project_headless, RgbaColor,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FillSpec {
    frame_index: u32,
    x: u32,
    y: u32,
    color: RgbaColor,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipelineSpec {
    video_path: String,
    project_root: String,
    output_path: String,
    #[serde(default)]
    fills: Vec<FillSpec>,
}

fn resolve_path(base_dir: &Path, value: &str) -> PathBuf {
    let candidate = PathBuf::from(value);

    if candidate.is_absolute() {
        return candidate;
    }

    base_dir.join(candidate)
}

fn main() -> Result<(), String> {
    let spec_arg = env::args()
        .nth(1)
        .ok_or_else(|| String::from("usage: cargo run --bin headless_pipeline -- <spec.json>"))?;
    let spec_path = PathBuf::from(&spec_arg);
    let spec_dir = spec_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let content = fs::read_to_string(&spec_path).map_err(|error| error.to_string())?;
    let spec = serde_json::from_str::<PipelineSpec>(&content).map_err(|error| error.to_string())?;

    let video_path = resolve_path(&spec_dir, &spec.video_path);
    let project_root = resolve_path(&spec_dir, &spec.project_root);
    let output_path = resolve_path(&spec_dir, &spec.output_path);

    if let Some(parent) = project_root.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let project = create_project_headless(
        video_path.display().to_string(),
        project_root.display().to_string(),
    )?;
    let preprocess = preprocess_project_headless(project.project_root.clone())?;

    println!(
        "project_ready root={} frames={} size={}x{} source_mode={}",
        project.project_root,
        project.frame_count,
        project.width,
        project.height,
        project.source_mode
    );
    println!(
        "preprocess_complete frames={} line_dir={} thumb_dir={}",
        preprocess.frame_count, preprocess.line_frames_dir, preprocess.thumb_frames_dir
    );

    for fill in spec.fills {
        let result = fill_region_headless(
            project.project_root.clone(),
            fill.frame_index,
            fill.x,
            fill.y,
            fill.color,
        )?;
        println!(
            "fill_complete frame={} x={} y={} track={} updated_frames={}",
            fill.frame_index,
            fill.x,
            fill.y,
            result.track_id,
            result.updated_frames.len()
        );
    }

    let export = export_video_headless(
        project.project_root.clone(),
        output_path.display().to_string(),
    )?;
    println!(
        "export_complete output={} frames={}",
        export.output_path, export.frame_count
    );

    Ok(())
}

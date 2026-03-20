use serde::{Deserialize, Serialize};

fn default_source_mode() -> String {
    String::from("placeholder")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub version: u32,
    pub project_root: String,
    pub source_video_path: String,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub source_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreprocessResult {
    pub frame_count: u32,
    pub line_frames_dir: String,
    pub thumb_frames_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPaths {
    pub source_frames_dir: String,
    pub line_frames_dir: String,
    pub paint_frames_dir: String,
    pub thumb_frames_dir: String,
    pub region_metadata_dir: String,
    #[serde(default)]
    pub region_track_index_path: String,
    #[serde(default)]
    pub region_label_maps_dir: String,
    #[serde(default)]
    pub blocked_region_metadata_dir: String,
    #[serde(default)]
    pub blocked_region_track_index_path: String,
    #[serde(default)]
    pub blocked_region_label_maps_dir: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub version: u32,
    pub source_video_path: String,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    #[serde(default = "default_source_mode")]
    pub source_mode: String,
    pub paths: ProjectPaths,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrameBundle {
    pub frame_index: u32,
    pub line_frame_path: String,
    pub paint_frame_path: String,
    pub prev_frame_path: Option<String>,
    pub next_frame_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StrokeInput {
    pub color: RgbaColor,
    pub size: f32,
    pub points: Vec<Point>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub frame_index: u32,
    pub updated_paint_frame_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FillResult {
    pub track_id: u32,
    pub updated_frames: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output_path: String,
    pub frame_count: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolSetupResult {
    pub tool_dir: String,
    pub source: String,
    pub downloaded: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegionBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegionMetadata {
    pub region_id: u32,
    pub track_id: u32,
    pub area: u32,
    pub centroid_x: f32,
    pub centroid_y: f32,
    pub bounds: RegionBounds,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrameRegionMetadata {
    pub frame_index: u32,
    pub width: u32,
    pub height: u32,
    pub regions: Vec<RegionMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrackFrameEntry {
    pub frame_index: u32,
    pub region_id: u32,
    pub centroid_x: u32,
    pub centroid_y: u32,
    pub sample_x: u32,
    pub sample_y: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RegionTrackIndex {
    pub tracks: std::collections::BTreeMap<u32, Vec<TrackFrameEntry>>,
}

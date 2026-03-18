mod commands;
mod models;

use commands::{
    create_project, draw_stroke, ensure_ffmpeg_tools, export_video, fill_region, get_frame_bundle,
    get_painted_frames, open_project, preprocess_project,
};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_project,
            ensure_ffmpeg_tools,
            preprocess_project,
            open_project,
            get_painted_frames,
            get_frame_bundle,
            draw_stroke,
            fill_region,
            export_video
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

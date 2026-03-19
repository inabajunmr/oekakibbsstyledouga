use std::{
    collections::BTreeMap,
    collections::HashMap,
    collections::VecDeque,
    env, fs,
    fs::File,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use image::{
    codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder},
    imageops::FilterType,
    DynamicImage, GrayImage, ImageBuffer, ImageEncoder, Luma, Rgba, RgbaImage,
};
use reqwest::blocking::Client;
use tauri::command;
use tauri::Manager;
use zip::ZipArchive;

use crate::models::{
    ExportResult, FillResult, FrameBundle, FrameRegionMetadata, PreprocessResult, ProjectFile,
    ProjectPaths, ProjectSummary, RegionBounds, RegionMetadata, RegionTrackIndex, RgbaColor,
    SaveResult, StrokeInput, ToolSetupResult, TrackFrameEntry,
};

const PROJECT_FILE_NAME: &str = "project.json";

#[derive(Debug, Clone, Copy)]
struct VideoMetadata {
    fps: f32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct DownloadSpec {
    ffmpeg_url: &'static str,
    ffprobe_url: &'static str,
}

fn line_image_cache() -> &'static Mutex<HashMap<String, GrayImage>> {
    static CACHE: OnceLock<Mutex<HashMap<String, GrayImage>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn paint_image_cache() -> &'static Mutex<HashMap<String, RgbaImage>> {
    static CACHE: OnceLock<Mutex<HashMap<String, RgbaImage>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn label_map_cache() -> &'static Mutex<HashMap<String, Vec<u32>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<u32>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn clear_image_caches() {
    if let Ok(mut cache) = line_image_cache().lock() {
        cache.clear();
    }

    if let Ok(mut cache) = paint_image_cache().lock() {
        cache.clear();
    }

    if let Ok(mut cache) = label_map_cache().lock() {
        cache.clear();
    }
}

fn log_file_path() -> PathBuf {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if current_dir.file_name().and_then(|name| name.to_str()) == Some("src-tauri") {
        return current_dir.parent().unwrap_or(&current_dir).join("log.txt");
    }

    current_dir.join("log.txt")
}

fn log_message(message: impl AsRef<str>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = format!(
        "{}.{}",
        now.as_secs(),
        format!("{:03}", now.subsec_millis())
    );
    let line = format!("[{timestamp}] {}\n", message.as_ref());

    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path())
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn frame_file_name(frame_index: u32) -> String {
    format!("{frame_index:06}.png")
}

fn png_file_paths(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
        .collect::<Vec<_>>();

    paths.sort();
    Ok(paths)
}

fn frame_path_for_index(dir: &Path, frame_index: u32) -> Result<PathBuf, String> {
    let exact = dir.join(frame_file_name(frame_index));

    if exact.exists() {
        return Ok(exact);
    }

    let paths = png_file_paths(dir)?;
    paths
        .get(frame_index as usize)
        .cloned()
        .ok_or_else(|| format!("Frame {frame_index} was not found in {}", dir.display()))
}

fn source_frame_path_for_index(dir: &Path, frame_index: u32) -> Result<PathBuf, String> {
    let paths = png_file_paths(dir)?;
    paths.get(frame_index as usize).cloned().ok_or_else(|| {
        format!(
            "Source frame {frame_index} was not found in {}",
            dir.display()
        )
    })
}

fn project_file_path(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_FILE_NAME)
}

fn build_project_paths() -> ProjectPaths {
    ProjectPaths {
        source_frames_dir: String::from("frames/source"),
        line_frames_dir: String::from("frames/line"),
        paint_frames_dir: String::from("frames/paint"),
        thumb_frames_dir: String::from("frames/thumb"),
        region_metadata_dir: String::from("regions"),
        region_track_index_path: String::from("regions/track-index.json"),
        region_label_maps_dir: String::from("regions/labels"),
    }
}

fn managed_tools_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base_dir = app
        .path()
        .app_local_data_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|error| error.to_string())?;

    Ok(base_dir.join("tools").join("ffmpeg"))
}

fn configured_tools_dir() -> Option<PathBuf> {
    env::var_os("OEKAKI_TOOLS_DIR").map(PathBuf::from)
}

fn managed_binary(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

fn resolve_in_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;

    env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.exists())
}

fn resolve_binary(names: &[&str]) -> Option<PathBuf> {
    let explicit_root = configured_tools_dir();

    let mut candidates = Vec::new();

    if let Some(root) = explicit_root {
        for name in names {
            candidates.push(root.join(name));
        }
    }

    for name in names {
        if let Some(found) = resolve_in_path(name) {
            candidates.push(found);
        }
        candidates.push(PathBuf::from("/opt/homebrew/bin").join(name));
        candidates.push(PathBuf::from("/usr/local/bin").join(name));
        candidates.push(PathBuf::from("/usr/bin").join(name));
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn current_download_spec() -> Result<DownloadSpec, String> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Ok(DownloadSpec {
            ffmpeg_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/snapshot/ffmpeg.zip",
            ffprobe_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/snapshot/ffprobe.zip",
        }),
        ("macos", "x86_64") => Ok(DownloadSpec {
            ffmpeg_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/snapshot/ffmpeg.zip",
            ffprobe_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/snapshot/ffprobe.zip",
        }),
        ("linux", "x86_64") => Ok(DownloadSpec {
            ffmpeg_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/linux/amd64/snapshot/ffmpeg.zip",
            ffprobe_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/linux/amd64/snapshot/ffprobe.zip",
        }),
        ("linux", "aarch64") => Ok(DownloadSpec {
            ffmpeg_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/linux/arm64/snapshot/ffmpeg.zip",
            ffprobe_url:
                "https://ffmpeg.martin-riedl.de/redirect/latest/linux/arm64/snapshot/ffprobe.zip",
        }),
        (os, arch) => Err(format!(
            "Automatic ffmpeg download is not supported on {os}/{arch} yet."
        )),
    }
}

fn archive_entry_matches(name: &str, expected_binary: &str) -> bool {
    Path::new(name).file_name().and_then(|part| part.to_str()) == Some(expected_binary)
}

fn download_zip(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|error| error.to_string())
}

fn extract_binary_from_zip(
    zip_bytes: &[u8],
    binary_name: &str,
    destination: &Path,
) -> Result<(), String> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|error| error.to_string())?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;

        if !archive_entry_matches(entry.name(), binary_name) {
            continue;
        }

        let mut output = File::create(destination).map_err(|error| error.to_string())?;
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|error| error.to_string())?;
        output
            .write_all(&buffer)
            .map_err(|error| error.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = output
                .metadata()
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(destination, permissions).map_err(|error| error.to_string())?;
        }

        return Ok(());
    }

    Err(format!(
        "Downloaded archive did not contain `{binary_name}`"
    ))
}

fn ensure_ffmpeg_tools_internal(app: &tauri::AppHandle) -> Result<ToolSetupResult, String> {
    if let (Some(ffmpeg), Some(_ffprobe)) =
        (resolve_binary(&["ffmpeg"]), resolve_binary(&["ffprobe"]))
    {
        let tool_dir = ffmpeg
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        return Ok(ToolSetupResult {
            tool_dir: tool_dir.display().to_string(),
            source: String::from("system"),
            downloaded: false,
        });
    }

    let tool_dir = managed_tools_dir(app)?;
    fs::create_dir_all(&tool_dir).map_err(|error| error.to_string())?;

    let ffmpeg_path = managed_binary(&tool_dir, "ffmpeg");
    let ffprobe_path = managed_binary(&tool_dir, "ffprobe");

    if ffmpeg_path.exists() && ffprobe_path.exists() {
        env::set_var("OEKAKI_TOOLS_DIR", &tool_dir);

        return Ok(ToolSetupResult {
            tool_dir: tool_dir.display().to_string(),
            source: String::from("managed"),
            downloaded: false,
        });
    }

    let spec = current_download_spec()?;
    let client = Client::builder()
        .build()
        .map_err(|error| error.to_string())?;

    let ffmpeg_zip = download_zip(&client, spec.ffmpeg_url)?;
    let ffprobe_zip = download_zip(&client, spec.ffprobe_url)?;
    extract_binary_from_zip(&ffmpeg_zip, "ffmpeg", &ffmpeg_path)?;
    extract_binary_from_zip(&ffprobe_zip, "ffprobe", &ffprobe_path)?;
    env::set_var("OEKAKI_TOOLS_DIR", &tool_dir);

    Ok(ToolSetupResult {
        tool_dir: tool_dir.display().to_string(),
        source: String::from("downloaded"),
        downloaded: true,
    })
}

fn ensure_project_dirs(project_root: &Path, paths: &ProjectPaths) -> Result<(), String> {
    fs::create_dir_all(project_root).map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.source_frames_dir))
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.line_frames_dir))
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.paint_frames_dir))
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.thumb_frames_dir))
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.region_metadata_dir))
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(project_root.join(&paths.region_label_maps_dir))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn clear_png_files(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) == Some("png") {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn clear_json_files(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn clear_directory(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.is_file() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn write_placeholder_frame(path: &Path, width: u32, height: u32) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    let image = ImageBuffer::from_pixel(
        width.max(1),
        height.max(1),
        Rgba([255u8, 255u8, 255u8, 255u8]),
    );
    image.save(path).map_err(|error| error.to_string())
}

fn write_transparent_placeholder_frame(path: &Path, width: u32, height: u32) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    let image = ImageBuffer::from_pixel(width.max(1), height.max(1), Rgba([0u8, 0u8, 0u8, 0u8]));
    image.save(path).map_err(|error| error.to_string())
}

fn composite_frame(line_image: &GrayImage, paint_image: &RgbaImage) -> RgbaImage {
    let (width, height) = line_image.dimensions();
    let mut output = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    for y in 0..height {
        for x in 0..width {
            let tone = line_image.get_pixel(x, y).0[0];
            output.put_pixel(x, y, Rgba([tone, tone, tone, 255]));
        }
    }

    for y in 0..height {
        for x in 0..width {
            let paint = *paint_image.get_pixel(x, y);

            if paint[3] == 0 {
                continue;
            }

            let pixel = output.get_pixel_mut(x, y);
            blend_pixel(pixel, paint);
            pixel[3] = 255;
        }
    }

    output
}

fn export_frames_dir(project_root: &Path) -> PathBuf {
    project_root.join("frames").join("export")
}

fn export_video_with_ffmpeg(
    ffmpeg: &Path,
    input_dir: &Path,
    output_path: &Path,
    fps: f32,
) -> Result<(), String> {
    let input_pattern = input_dir.join("%06d.png");
    let fps_value = if fps.is_finite() && fps > 0.0 {
        format!("{fps:.3}")
    } else {
        String::from("24")
    };
    let output = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-framerate",
            fps_value.as_str(),
            "-i",
        ])
        .arg(input_pattern)
        .args(["-pix_fmt", "yuv420p"])
        .arg(output_path)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(())
}

fn grayscale_at(image: &GrayImage, x: u32, y: u32) -> i32 {
    image.get_pixel(x, y).0[0] as i32
}

fn build_lineart_image(source: DynamicImage) -> GrayImage {
    let grayscale = source.grayscale().to_luma8();
    let (width, height) = grayscale.dimensions();

    if width < 3 || height < 3 {
        return ImageBuffer::from_pixel(width, height, Luma([255]));
    }

    let mut result = ImageBuffer::from_pixel(width, height, Luma([255]));

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let gx = -grayscale_at(&grayscale, x - 1, y - 1)
                + grayscale_at(&grayscale, x + 1, y - 1)
                - 2 * grayscale_at(&grayscale, x - 1, y)
                + 2 * grayscale_at(&grayscale, x + 1, y)
                - grayscale_at(&grayscale, x - 1, y + 1)
                + grayscale_at(&grayscale, x + 1, y + 1);

            let gy = -grayscale_at(&grayscale, x - 1, y - 1)
                - 2 * grayscale_at(&grayscale, x, y - 1)
                - grayscale_at(&grayscale, x + 1, y - 1)
                + grayscale_at(&grayscale, x - 1, y + 1)
                + 2 * grayscale_at(&grayscale, x, y + 1)
                + grayscale_at(&grayscale, x + 1, y + 1);

            let edge_strength = gx.abs() + gy.abs();
            let tone = grayscale_at(&grayscale, x, y);
            let is_edge = edge_strength > 160 || tone < 48;
            let value = if is_edge { 0 } else { 255 };

            result.put_pixel(x, y, Luma([value]));
        }
    }

    result
}

fn preprocess_source_frame(
    source_path: &Path,
    line_path: &Path,
    thumb_path: &Path,
) -> Result<(), String> {
    let image = image::open(source_path).map_err(|error| error.to_string())?;
    let lineart = build_lineart_image(image.clone());
    lineart.save(line_path).map_err(|error| error.to_string())?;

    let thumbnail = image.thumbnail(240, 135);
    thumbnail
        .save(thumb_path)
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn seed_placeholder_frames(
    dir: &Path,
    frame_count: u32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    for frame_index in 0..frame_count {
        let frame_name = frame_file_name(frame_index);
        write_placeholder_frame(&dir.join(frame_name.as_str()), width, height)?;
    }

    Ok(())
}

fn seed_blank_paint_frames(
    dir: &Path,
    frame_count: u32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    for frame_index in 0..frame_count {
        let frame_name = frame_file_name(frame_index);
        write_transparent_placeholder_frame(&dir.join(frame_name.as_str()), width, height)?;
    }

    Ok(())
}

fn normalize_gray_image(image: GrayImage, width: u32, height: u32) -> GrayImage {
    if image.width() == width && image.height() == height {
        return image;
    }

    DynamicImage::ImageLuma8(image)
        .resize_exact(width, height, FilterType::Nearest)
        .to_luma8()
}

fn normalize_rgba_image(image: RgbaImage, width: u32, height: u32) -> RgbaImage {
    if image.width() == width && image.height() == height {
        return image;
    }

    DynamicImage::ImageRgba8(image)
        .resize_exact(width, height, FilterType::Nearest)
        .to_rgba8()
}

fn load_line_image(path: &Path, width: u32, height: u32) -> Result<GrayImage, String> {
    let key = path.display().to_string();

    if let Ok(cache) = line_image_cache().lock() {
        if let Some(image) = cache.get(&key) {
            return Ok(image.clone());
        }
    }

    let image = normalize_gray_image(
        image::open(path)
            .map_err(|error| error.to_string())?
            .to_luma8(),
        width,
        height,
    );

    if let Ok(mut cache) = line_image_cache().lock() {
        cache.insert(key, image.clone());
    }

    Ok(image)
}

fn blank_paint_image(width: u32, height: u32) -> RgbaImage {
    ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]))
}

fn ensure_paint_image(path: &Path, width: u32, height: u32) -> Result<RgbaImage, String> {
    let key = path.display().to_string();

    if let Ok(cache) = paint_image_cache().lock() {
        if let Some(image) = cache.get(&key) {
            return Ok(image.clone());
        }
    }

    if path.exists() {
        let rgba = normalize_rgba_image(
            image::open(path)
                .map_err(|error| error.to_string())?
                .to_rgba8(),
            width,
            height,
        );

        if rgba.width() == width && rgba.height() == height {
            if let Ok(mut cache) = paint_image_cache().lock() {
                cache.insert(key, rgba.clone());
            }
            return Ok(rgba);
        }
    }

    let blank = blank_paint_image(width, height);

    if let Ok(mut cache) = paint_image_cache().lock() {
        cache.insert(key, blank.clone());
    }

    Ok(blank)
}

fn save_paint_image(path: &Path, image: &RgbaImage) -> Result<(), String> {
    let file = File::create(path).map_err(|error| error.to_string())?;
    let encoder =
        PngEncoder::new_with_quality(file, CompressionType::Fast, PngFilterType::NoFilter);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;

    if let Ok(mut cache) = paint_image_cache().lock() {
        cache.insert(path.display().to_string(), image.clone());
    }

    Ok(())
}

fn paint_image_has_visible_pixels(path: &Path) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }

    let key = path.display().to_string();
    let image = if let Ok(cache) = paint_image_cache().lock() {
        cache.get(&key).cloned()
    } else {
        None
    };
    let image = match image {
        Some(image) => image,
        None => image::open(path)
            .map_err(|error| error.to_string())?
            .to_rgba8(),
    };
    Ok(image.pixels().any(|pixel| pixel[3] > 0))
}

fn blend_pixel(dest: &mut Rgba<u8>, src: Rgba<u8>) {
    let alpha = src[3] as f32 / 255.0;
    let inv_alpha = 1.0 - alpha;

    for channel in 0..3 {
        dest[channel] = ((src[channel] as f32 * alpha) + (dest[channel] as f32 * inv_alpha))
            .round()
            .clamp(0.0, 255.0) as u8;
    }

    dest[3] = ((src[3] as f32) + (dest[3] as f32 * inv_alpha))
        .round()
        .clamp(0.0, 255.0) as u8;
}

fn paint_circle(image: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
    let width = image.width() as i32;
    let height = image.height() as i32;

    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }

            let dx = x - cx;
            let dy = y - cy;

            if dx * dx + dy * dy > radius * radius {
                continue;
            }

            let pixel = image.get_pixel_mut(x as u32, y as u32);
            blend_pixel(pixel, color);
        }
    }
}

fn draw_stroke_on_image(image: &mut RgbaImage, stroke: &StrokeInput) {
    if stroke.points.is_empty() {
        return;
    }

    let color = Rgba([
        stroke.color.r,
        stroke.color.g,
        stroke.color.b,
        stroke.color.a,
    ]);
    let radius = (stroke.size.max(1.0) / 2.0).round() as i32;

    if stroke.points.len() == 1 {
        let point = &stroke.points[0];
        paint_circle(
            image,
            point.x.round() as i32,
            point.y.round() as i32,
            radius,
            color,
        );
        return;
    }

    for segment in stroke.points.windows(2) {
        let start = &segment[0];
        let end = &segment[1];
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let steps = dx.abs().max(dy.abs()).max(1.0).ceil() as i32;

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = start.x + dx * t;
            let y = start.y + dy * t;
            paint_circle(image, x.round() as i32, y.round() as i32, radius, color);
        }
    }
}

fn line_blocks_fill(line_image: &GrayImage, x: u32, y: u32) -> bool {
    line_image.get_pixel(x, y).0[0] < 64
}

fn color_matches(pixel: &Rgba<u8>, color: &Rgba<u8>) -> bool {
    pixel.0 == color.0
}

fn region_metadata_path(dir: &Path, frame_index: u32) -> PathBuf {
    dir.join(format!("{frame_index:06}.json"))
}

fn region_track_index_path(project_root: &Path, paths: &ProjectPaths) -> PathBuf {
    if paths.region_track_index_path.is_empty() {
        return project_root.join("regions").join("track-index.json");
    }

    project_root.join(&paths.region_track_index_path)
}

fn region_label_maps_dir(project_root: &Path, paths: &ProjectPaths) -> PathBuf {
    if paths.region_label_maps_dir.is_empty() {
        return project_root.join("regions").join("labels");
    }

    project_root.join(&paths.region_label_maps_dir)
}

fn region_label_map_path(project_root: &Path, paths: &ProjectPaths, frame_index: u32) -> PathBuf {
    region_label_maps_dir(project_root, paths).join(format!("{frame_index:06}.bin"))
}

fn write_region_label_map(path: &Path, label_map: &[u32]) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(label_map.len() * 4);

    for label in label_map {
        bytes.extend_from_slice(&label.to_le_bytes());
    }

    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn load_region_label_map(path: &Path, width: u32, height: u32) -> Result<Vec<u32>, String> {
    let key = path.display().to_string();

    if let Ok(cache) = label_map_cache().lock() {
        if let Some(label_map) = cache.get(&key) {
            return Ok(label_map.clone());
        }
    }

    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixel_count| pixel_count.checked_mul(4))
        .ok_or_else(|| String::from("Region label map dimensions overflowed."))?;

    if bytes.len() != expected_len {
        return Err(format!(
            "Region label map at {} has invalid length {} (expected {}).",
            path.display(),
            bytes.len(),
            expected_len
        ));
    }

    let mut label_map = Vec::with_capacity((width * height) as usize);

    for chunk in bytes.chunks_exact(4) {
        label_map.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    if let Ok(mut cache) = label_map_cache().lock() {
        cache.insert(key, label_map.clone());
    }

    Ok(label_map)
}

fn build_region_metadata(
    frame_index: u32,
    line_image: &GrayImage,
) -> (FrameRegionMetadata, Vec<u32>) {
    let width = line_image.width();
    let height = line_image.height();
    let mut visited = vec![false; (width * height) as usize];
    let mut label_map = vec![0; (width * height) as usize];
    let mut regions = Vec::new();
    let mut next_region_id = 1_u32;

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;

            if visited[index] || line_blocks_fill(line_image, x, y) {
                continue;
            }

            let mut queue = VecDeque::from([(x, y)]);
            visited[index] = true;
            let mut area = 0_u32;
            let mut sum_x = 0_f32;
            let mut sum_y = 0_f32;
            let mut min_x = x;
            let mut max_x = x;
            let mut min_y = y;
            let mut max_y = y;

            while let Some((cx, cy)) = queue.pop_front() {
                let current_index = (cy * width + cx) as usize;
                area += 1;
                sum_x += cx as f32;
                sum_y += cy as f32;
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);
                label_map[current_index] = next_region_id;

                let neighbors = [
                    (cx.wrapping_sub(1), cy, cx > 0),
                    (cx + 1, cy, cx + 1 < width),
                    (cx, cy.wrapping_sub(1), cy > 0),
                    (cx, cy + 1, cy + 1 < height),
                ];

                for (nx, ny, valid) in neighbors {
                    if !valid || line_blocks_fill(line_image, nx, ny) {
                        continue;
                    }

                    let nindex = (ny * width + nx) as usize;

                    if visited[nindex] {
                        continue;
                    }

                    visited[nindex] = true;
                    queue.push_back((nx, ny));
                }
            }

            regions.push(RegionMetadata {
                region_id: next_region_id,
                track_id: next_region_id,
                area,
                centroid_x: sum_x / area as f32,
                centroid_y: sum_y / area as f32,
                bounds: RegionBounds {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x + 1,
                    height: max_y - min_y + 1,
                },
            });
            next_region_id += 1;
        }
    }

    (
        FrameRegionMetadata {
            frame_index,
            width,
            height,
            regions,
        },
        label_map,
    )
}

fn write_region_metadata(
    region_dir: &Path,
    frame_index: u32,
    metadata: &FrameRegionMetadata,
) -> Result<(), String> {
    let content = serde_json::to_string_pretty(metadata).map_err(|error| error.to_string())?;
    fs::write(region_metadata_path(region_dir, frame_index), content)
        .map_err(|error| error.to_string())
}

fn read_region_metadata(
    region_dir: &Path,
    frame_index: u32,
) -> Result<FrameRegionMetadata, String> {
    let content = fs::read_to_string(region_metadata_path(region_dir, frame_index))
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn build_region_track_index(all_region_metadata: &[FrameRegionMetadata]) -> RegionTrackIndex {
    let mut tracks: BTreeMap<u32, Vec<TrackFrameEntry>> = BTreeMap::new();

    for metadata in all_region_metadata {
        for region in &metadata.regions {
            tracks
                .entry(region.track_id)
                .or_default()
                .push(TrackFrameEntry {
                    frame_index: metadata.frame_index,
                    region_id: region.region_id,
                    centroid_x: region.centroid_x.round().max(0.0) as u32,
                    centroid_y: region.centroid_y.round().max(0.0) as u32,
                    sample_x: region.bounds.x,
                    sample_y: region.bounds.y,
                });
        }
    }

    RegionTrackIndex { tracks }
}

fn write_region_track_index(
    project_root: &Path,
    paths: &ProjectPaths,
    index: &RegionTrackIndex,
) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index).map_err(|error| error.to_string())?;
    fs::write(region_track_index_path(project_root, paths), content)
        .map_err(|error| error.to_string())
}

fn read_region_track_index(
    project_root: &Path,
    paths: &ProjectPaths,
) -> Result<RegionTrackIndex, String> {
    let content = fs::read_to_string(region_track_index_path(project_root, paths))
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn find_region_at_point<'a>(
    metadata: &'a FrameRegionMetadata,
    label_map: &[u32],
    x: u32,
    y: u32,
) -> Option<&'a RegionMetadata> {
    if x >= metadata.width || y >= metadata.height {
        return None;
    }

    let index = (y * metadata.width + x) as usize;
    let region_id = label_map.get(index).copied().unwrap_or_default();

    if region_id == 0 {
        return None;
    }

    metadata
        .regions
        .iter()
        .find(|region| region.region_id == region_id)
}

fn fill_region_using_label_map(
    paint_image: &mut RgbaImage,
    label_map: &[u32],
    region_id: u32,
    sample_x: u32,
    sample_y: u32,
    color: Rgba<u8>,
) -> bool {
    let width = paint_image.width();
    let height = paint_image.height();

    if width == 0 || height == 0 {
        return false;
    }

    let sample_index = if sample_x < width && sample_y < height {
        let index = (sample_y * width + sample_x) as usize;
        if label_map.get(index).copied() == Some(region_id) {
            Some(index)
        } else {
            None
        }
    } else {
        None
    };

    let sample_index =
        sample_index.or_else(|| label_map.iter().position(|label| *label == region_id));

    let Some(sample_index) = sample_index else {
        return false;
    };

    let target_x = (sample_index as u32) % width;
    let target_y = (sample_index as u32) / width;
    let target = *paint_image.get_pixel(target_x, target_y);

    if color_matches(&target, &color) {
        return false;
    }

    let mut changed = false;

    for (index, label) in label_map.iter().enumerate() {
        if *label != region_id {
            continue;
        }

        let x = (index as u32) % width;
        let y = (index as u32) / width;
        let current = *paint_image.get_pixel(x, y);

        if !color_matches(&current, &target) {
            continue;
        }

        paint_image.put_pixel(x, y, color);
        changed = true;
    }

    changed
}

fn region_match_score(current: &RegionMetadata, previous: &RegionMetadata) -> f32 {
    let dx = current.centroid_x - previous.centroid_x;
    let dy = current.centroid_y - previous.centroid_y;
    let centroid_distance = (dx * dx + dy * dy).sqrt();
    let area_ratio =
        (current.area as f32 - previous.area as f32).abs() / previous.area.max(1) as f32;

    centroid_distance + area_ratio * 120.0
}

fn propagate_track_ids(frames: &mut [FrameRegionMetadata]) {
    let mut next_track_id = 1_u32;

    if let Some(first) = frames.first_mut() {
        for region in &mut first.regions {
            region.track_id = next_track_id;
            next_track_id += 1;
        }
    }

    for index in 1..frames.len() {
        let previous = frames[index - 1].regions.clone();
        let current = &mut frames[index].regions;
        let mut assigned_previous = vec![false; previous.len()];

        for region in current {
            let mut best_match: Option<(usize, f32)> = None;

            for (previous_index, previous_region) in previous.iter().enumerate() {
                if assigned_previous[previous_index] {
                    continue;
                }

                let score = region_match_score(region, previous_region);

                if score > 80.0 {
                    continue;
                }

                match best_match {
                    Some((_, best_score)) if score >= best_score => {}
                    _ => {
                        best_match = Some((previous_index, score));
                    }
                }
            }

            if let Some((previous_index, _)) = best_match {
                region.track_id = previous[previous_index].track_id;
                assigned_previous[previous_index] = true;
            } else {
                region.track_id = next_track_id;
                next_track_id += 1;
            }
        }
    }
}

fn parse_fps(raw: &str) -> Option<f32> {
    let (numerator, denominator) = raw.split_once('/')?;
    let numerator = numerator.parse::<f32>().ok()?;
    let denominator = denominator.parse::<f32>().ok()?;

    if denominator == 0.0 {
        return None;
    }

    Some(numerator / denominator)
}

fn probe_video_metadata(ffprobe: &Path, video_path: &Path) -> Result<VideoMetadata, String> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate",
            "-of",
            "json",
        ])
        .arg(video_path)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let stream = value
        .get("streams")
        .and_then(|streams| streams.as_array())
        .and_then(|streams| streams.first())
        .ok_or_else(|| String::from("ffprobe returned no video streams"))?;

    let width = stream
        .get("width")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| String::from("ffprobe width missing"))? as u32;
    let height = stream
        .get("height")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| String::from("ffprobe height missing"))? as u32;
    let fps = stream
        .get("r_frame_rate")
        .and_then(|value| value.as_str())
        .and_then(parse_fps)
        .unwrap_or(24.0);

    Ok(VideoMetadata { fps, width, height })
}

fn extract_source_frames(
    ffmpeg: &Path,
    video_path: &Path,
    output_dir: &Path,
) -> Result<(), String> {
    let output_pattern = output_dir.join("%06d.png");
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(video_path)
        .args(["-start_number", "0"])
        .arg(output_pattern)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(())
}

fn create_project_file_from_video(
    project_root: &Path,
    video_path: &Path,
) -> Result<ProjectFile, String> {
    log_message(format!(
        "create_project_file_from_video start project_root={} video_path={}",
        project_root.display(),
        video_path.display()
    ));
    let paths = build_project_paths();
    ensure_project_dirs(project_root, &paths)?;
    log_message("create_project_file_from_video ensured_dirs");

    let ffmpeg = resolve_binary(&["ffmpeg"]);
    let ffprobe = resolve_binary(&["ffprobe"]);

    if let (Some(ffmpeg), Some(ffprobe)) = (ffmpeg, ffprobe) {
        log_message(format!(
            "create_project_file_from_video using_video_tools ffmpeg={} ffprobe={}",
            ffmpeg.display(),
            ffprobe.display()
        ));
        let source_dir = project_root.join(&paths.source_frames_dir);
        clear_png_files(&source_dir)?;
        log_message("create_project_file_from_video cleared_source_dir");
        let metadata = probe_video_metadata(&ffprobe, video_path)?;
        log_message(format!(
            "create_project_file_from_video probed_video width={} height={} fps={}",
            metadata.width, metadata.height, metadata.fps
        ));
        extract_source_frames(&ffmpeg, video_path, &source_dir)?;
        let frame_count = png_file_paths(&source_dir)?.len() as u32;
        log_message(format!(
            "create_project_file_from_video extracted_frames count={}",
            frame_count
        ));

        seed_blank_paint_frames(
            &project_root.join(&paths.paint_frames_dir),
            frame_count,
            metadata.width,
            metadata.height,
        )?;
        log_message("create_project_file_from_video seeded_blank_paint_frames");

        return Ok(ProjectFile {
            version: 1,
            source_video_path: video_path.display().to_string(),
            fps: metadata.fps,
            width: metadata.width,
            height: metadata.height,
            frame_count,
            source_mode: String::from("video"),
            paths,
        });
    }

    log_message("create_project_file_from_video fallback_placeholder");
    let project_file = ProjectFile {
        version: 1,
        source_video_path: video_path.display().to_string(),
        fps: 24.0,
        width: 1280,
        height: 720,
        frame_count: 12,
        source_mode: String::from("placeholder"),
        paths,
    };

    seed_placeholder_frames(
        &project_root.join(&project_file.paths.source_frames_dir),
        project_file.frame_count,
        project_file.width,
        project_file.height,
    )?;
    seed_blank_paint_frames(
        &project_root.join(&project_file.paths.paint_frames_dir),
        project_file.frame_count,
        project_file.width,
        project_file.height,
    )?;
    Ok(project_file)
}

fn write_project_file(project_root: &Path, project_file: &ProjectFile) -> Result<(), String> {
    let content = serde_json::to_string_pretty(project_file).map_err(|error| error.to_string())?;
    fs::write(project_file_path(project_root), content).map_err(|error| error.to_string())
}

fn read_project_file(project_root: &Path) -> Result<ProjectFile, String> {
    let content =
        fs::read_to_string(project_file_path(project_root)).map_err(|error| error.to_string())?;
    serde_json::from_str::<ProjectFile>(&content).map_err(|error| error.to_string())
}

fn project_summary_from_file(project_root: &Path, project_file: &ProjectFile) -> ProjectSummary {
    ProjectSummary {
        version: project_file.version,
        project_root: project_root.display().to_string(),
        source_video_path: project_file.source_video_path.clone(),
        fps: project_file.fps,
        width: project_file.width,
        height: project_file.height,
        frame_count: project_file.frame_count,
        source_mode: project_file.source_mode.clone(),
    }
}

fn optional_frame_path(dir: &Path, frame_index: Option<u32>) -> Option<String> {
    frame_index
        .and_then(|index| frame_path_for_index(dir, index).ok())
        .map(|path| path.display().to_string())
}

fn preprocess_project_frames(
    project_root: &Path,
    paths: &ProjectPaths,
) -> Result<PreprocessResult, String> {
    let source_dir = project_root.join(&paths.source_frames_dir);
    let line_dir = project_root.join(&paths.line_frames_dir);
    let thumb_dir = project_root.join(&paths.thumb_frames_dir);
    let paint_dir = project_root.join(&paths.paint_frames_dir);
    let region_dir = project_root.join(&paths.region_metadata_dir);
    let label_map_dir = region_label_maps_dir(project_root, paths);

    clear_png_files(&line_dir)?;
    clear_png_files(&thumb_dir)?;
    clear_json_files(&region_dir)?;
    clear_directory(&label_map_dir)?;
    fs::create_dir_all(&line_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&thumb_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paint_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&region_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&label_map_dir).map_err(|error| error.to_string())?;

    let mut frame_count = 0;
    let mut all_region_metadata = Vec::new();

    for entry in fs::read_dir(&source_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();

        if source_path.extension().and_then(|ext| ext.to_str()) != Some("png") {
            continue;
        }

        let file_name = entry.file_name();
        preprocess_source_frame(
            &source_path,
            &line_dir.join(&file_name),
            &thumb_dir.join(&file_name),
        )?;
        let line_image = image::open(line_dir.join(&file_name))
            .map_err(|error| error.to_string())?
            .to_luma8();
        let (metadata, label_map) = build_region_metadata(frame_count, &line_image);
        write_region_label_map(
            &region_label_map_path(project_root, paths, frame_count),
            &label_map,
        )?;
        all_region_metadata.push(metadata);
        write_transparent_placeholder_frame(
            &paint_dir.join(&file_name),
            line_image.width(),
            line_image.height(),
        )?;
        frame_count += 1;
    }

    propagate_track_ids(&mut all_region_metadata);

    for metadata in &all_region_metadata {
        write_region_metadata(&region_dir, metadata.frame_index, metadata)?;
    }

    let track_index = build_region_track_index(&all_region_metadata);
    write_region_track_index(project_root, paths, &track_index)?;

    Ok(PreprocessResult {
        frame_count,
        line_frames_dir: line_dir.display().to_string(),
        thumb_frames_dir: thumb_dir.display().to_string(),
    })
}

fn materialize_frame_assets(
    project_root: &Path,
    project_file: &ProjectFile,
    frame_index: u32,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let source_dir = project_root.join(&project_file.paths.source_frames_dir);
    let line_dir = project_root.join(&project_file.paths.line_frames_dir);
    let thumb_dir = project_root.join(&project_file.paths.thumb_frames_dir);
    let paint_dir = project_root.join(&project_file.paths.paint_frames_dir);
    let frame_name = frame_file_name(frame_index);
    let source_path = source_frame_path_for_index(&source_dir, frame_index)?;
    let line_path = line_dir.join(&frame_name);
    let thumb_path = thumb_dir.join(&frame_name);
    let paint_path = paint_dir.join(&frame_name);

    fs::create_dir_all(&line_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&thumb_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(&paint_dir).map_err(|error| error.to_string())?;

    if !line_path.exists() || !thumb_path.exists() {
        preprocess_source_frame(&source_path, &line_path, &thumb_path)?;
    }

    if !paint_path.exists() {
        write_transparent_placeholder_frame(&paint_path, project_file.width, project_file.height)?;
    }

    Ok((line_path, paint_path, thumb_path))
}

#[derive(Debug)]
struct FillFrameMetrics {
    frame_index: u32,
    changed: bool,
    materialize_ms: u128,
    load_ms: u128,
    fill_ms: u128,
    save_ms: Option<u128>,
    total_ms: u128,
}

#[command]
pub fn ensure_ffmpeg_tools(app: tauri::AppHandle) -> Result<ToolSetupResult, String> {
    ensure_ffmpeg_tools_internal(&app)
}

#[command]
pub fn create_project(
    app: tauri::AppHandle,
    video_path: String,
    project_root: String,
) -> Result<ProjectSummary, String> {
    clear_image_caches();
    log_message(format!(
        "create_project start video_path={} project_root={}",
        video_path, project_root
    ));
    let _ = ensure_ffmpeg_tools_internal(&app);
    log_message("create_project tools_ready");
    let project_root = PathBuf::from(project_root);
    let source_video_path = PathBuf::from(video_path);

    if !source_video_path.exists() {
        return Err(format!(
            "Video file was not found: {}",
            source_video_path.display()
        ));
    }

    if !source_video_path.is_file() {
        return Err(format!(
            "Video path is not a file: {}",
            source_video_path.display()
        ));
    }

    if project_root.exists() && !project_root.is_dir() {
        return Err(format!(
            "Project folder is not a directory: {}",
            project_root.display()
        ));
    }

    let project_file = create_project_file_from_video(&project_root, &source_video_path)?;
    log_message(format!(
        "create_project project_file_ready frame_count={} source_mode={}",
        project_file.frame_count, project_file.source_mode
    ));
    write_project_file(&project_root, &project_file)?;
    log_message(format!(
        "create_project complete project_root={} frame_count={} source_mode={}",
        project_root.display(),
        project_file.frame_count,
        project_file.source_mode
    ));

    Ok(project_summary_from_file(&project_root, &project_file))
}

#[command]
pub fn preprocess_project(project_root: String) -> Result<PreprocessResult, String> {
    let project_root = PathBuf::from(project_root);
    let project_file = read_project_file(&project_root)?;
    preprocess_project_frames(&project_root, &project_file.paths)
}

#[command]
pub fn open_project(project_root: String) -> Result<ProjectSummary, String> {
    let project_root = PathBuf::from(project_root);
    let project_file = read_project_file(&project_root)?;
    Ok(project_summary_from_file(&project_root, &project_file))
}

#[command]
pub fn get_painted_frames(project_root: String) -> Result<Vec<u32>, String> {
    let root = PathBuf::from(project_root);
    let project_file = read_project_file(&root)?;
    let paint_dir = root.join(&project_file.paths.paint_frames_dir);
    let mut painted_frames = Vec::new();

    for frame_index in 0..project_file.frame_count {
        let frame_path = paint_dir.join(frame_file_name(frame_index));

        if paint_image_has_visible_pixels(&frame_path)? {
            painted_frames.push(frame_index);
        }
    }

    Ok(painted_frames)
}

#[command]
pub fn export_video(project_root: String, output_path: String) -> Result<ExportResult, String> {
    let started_at = Instant::now();
    let root = PathBuf::from(project_root);
    let project_file = read_project_file(&root)?;
    let ffmpeg = resolve_binary(&["ffmpeg"]).ok_or_else(|| {
        String::from("ffmpeg was not found. Set OEKAKI_TOOLS_DIR or install ffmpeg.")
    })?;
    let export_dir = export_frames_dir(&root);
    let output_path = PathBuf::from(output_path);

    fs::create_dir_all(&export_dir).map_err(|error| error.to_string())?;
    clear_directory(&export_dir)?;

    for frame_index in 0..project_file.frame_count {
        let (line_path, paint_path, _) =
            materialize_frame_assets(&root, &project_file, frame_index)?;
        let frame_name = frame_file_name(frame_index);
        let line_image = load_line_image(&line_path, project_file.width, project_file.height)?;
        let paint_image = normalize_rgba_image(
            ensure_paint_image(&paint_path, project_file.width, project_file.height)?,
            project_file.width,
            project_file.height,
        );
        let composed = composite_frame(&line_image, &paint_image);
        composed
            .save(export_dir.join(&frame_name))
            .map_err(|error| error.to_string())?;
    }

    export_video_with_ffmpeg(&ffmpeg, &export_dir, &output_path, project_file.fps)?;
    log_message(format!(
        "export_video project_root={} output_path={} frame_count={} elapsed_ms={}",
        root.display(),
        output_path.display(),
        project_file.frame_count,
        started_at.elapsed().as_millis()
    ));

    Ok(ExportResult {
        output_path: output_path.display().to_string(),
        frame_count: project_file.frame_count,
    })
}

#[command]
pub fn get_frame_bundle(project_root: String, frame_index: u32) -> Result<FrameBundle, String> {
    let started_at = Instant::now();
    let root = PathBuf::from(project_root);
    let project_file = read_project_file(&root)?;
    let line_dir = root.join(&project_file.paths.line_frames_dir);
    let (line_frame_path, paint_frame_path, thumb_frame_path) =
        materialize_frame_assets(&root, &project_file, frame_index)?;
    log_message(format!(
        "get_frame_bundle frame={} project_root={} elapsed_ms={}",
        frame_index,
        root.display(),
        started_at.elapsed().as_millis()
    ));

    Ok(FrameBundle {
        frame_index,
        line_frame_path: line_frame_path.display().to_string(),
        paint_frame_path: paint_frame_path.display().to_string(),
        prev_frame_path: optional_frame_path(&line_dir, frame_index.checked_sub(1)),
        next_frame_path: if frame_index + 1 < project_file.frame_count {
            optional_frame_path(&line_dir, Some(frame_index + 1))
        } else {
            None
        },
        thumbnail_path: Some(thumb_frame_path.display().to_string()),
        width: project_file.width,
        height: project_file.height,
    })
}

#[command]
pub fn draw_stroke(
    project_root: String,
    frame_index: u32,
    stroke: StrokeInput,
) -> Result<SaveResult, String> {
    let started_at = Instant::now();
    let root = PathBuf::from(project_root);
    let project_file = read_project_file(&root)?;
    let (_, paint_path, _) = materialize_frame_assets(&root, &project_file, frame_index)?;
    let mut image = ensure_paint_image(&paint_path, project_file.width, project_file.height)?;
    draw_stroke_on_image(&mut image, &stroke);
    save_paint_image(&paint_path, &image)?;
    log_message(format!(
        "draw_stroke frame={} points={} project_root={} elapsed_ms={}",
        frame_index,
        stroke.points.len(),
        root.display(),
        started_at.elapsed().as_millis()
    ));

    Ok(SaveResult {
        frame_index,
        updated_paint_frame_path: paint_path.display().to_string(),
    })
}

#[command]
pub fn fill_region(
    project_root: String,
    frame_index: u32,
    x: u32,
    y: u32,
    color: RgbaColor,
) -> Result<FillResult, String> {
    let started_at = Instant::now();
    let root = PathBuf::from(project_root);
    let project_file = read_project_file(&root)?;
    let region_dir = root.join(&project_file.paths.region_metadata_dir);
    log_message(format!(
        "fill_region start frame={} x={} y={} color=rgba({},{},{},{}) project_root={}",
        frame_index,
        x,
        y,
        color.r,
        color.g,
        color.b,
        color.a,
        root.display()
    ));

    if !region_metadata_path(&region_dir, frame_index).exists()
        || !region_track_index_path(&root, &project_file.paths).exists()
        || !region_label_map_path(&root, &project_file.paths, frame_index).exists()
    {
        let preprocess_started_at = Instant::now();
        preprocess_project_frames(&root, &project_file.paths)?;
        log_message(format!(
            "fill_region preprocess frame={} elapsed_ms={}",
            frame_index,
            preprocess_started_at.elapsed().as_millis()
        ));
    }

    let read_metadata_started_at = Instant::now();
    let fill_color = Rgba([color.r, color.g, color.b, color.a]);
    let region_metadata = read_region_metadata(&region_dir, frame_index)?;
    let clicked_label_map = load_region_label_map(
        &region_label_map_path(&root, &project_file.paths, frame_index),
        project_file.width,
        project_file.height,
    )?;
    let track_index = read_region_track_index(&root, &project_file.paths)?;
    log_message(format!(
        "fill_region read_metadata frame={} elapsed_ms={}",
        frame_index,
        read_metadata_started_at.elapsed().as_millis()
    ));
    let region = find_region_at_point(&region_metadata, &clicked_label_map, x, y);
    let Some(region) = region else {
        log_message(format!(
            "fill_region no_region frame={} elapsed_ms={}",
            frame_index,
            started_at.elapsed().as_millis()
        ));
        return Ok(FillResult {
            track_id: frame_index,
            updated_frames: Vec::new(),
        });
    };

    let Some(track_frames) = track_index.tracks.get(&region.track_id) else {
        log_message(format!(
            "fill_region missing_track_index track_id={} frame={} elapsed_ms={}",
            region.track_id,
            frame_index,
            started_at.elapsed().as_millis()
        ));
        return Ok(FillResult {
            track_id: region.track_id,
            updated_frames: Vec::new(),
        });
    };

    log_message(format!(
        "fill_region track={} candidate_frames={}",
        region.track_id,
        track_frames.len()
    ));

    let worker_count = std::thread::available_parallelism()
        .map(|count| count.get().saturating_mul(2))
        .unwrap_or(4)
        .max(4)
        .min(track_frames.len().max(1))
        .min(24);
    let chunk_size = track_frames.len().max(1).div_ceil(worker_count);
    let track_frames = track_frames.clone();
    let mut updated_frames = Vec::new();
    let mut metrics = Vec::new();

    std::thread::scope(|scope| -> Result<(), String> {
        let mut handles = Vec::new();

        for chunk in track_frames.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let root = root.clone();
            let project_file = project_file.clone();

            handles.push(
                scope.spawn(move || -> Result<Vec<FillFrameMetrics>, String> {
                    let mut chunk_metrics = Vec::with_capacity(chunk.len());

                    for entry in chunk {
                        let frame_started_at = Instant::now();
                        let materialize_started_at = Instant::now();
                        let (_, paint_path, _) =
                            materialize_frame_assets(&root, &project_file, entry.frame_index)?;
                        let materialize_elapsed = materialize_started_at.elapsed().as_millis();

                        let load_started_at = Instant::now();
                        let mut paint_image = ensure_paint_image(
                            &paint_path,
                            project_file.width,
                            project_file.height,
                        )?;
                        let label_map = load_region_label_map(
                            &region_label_map_path(&root, &project_file.paths, entry.frame_index),
                            project_file.width,
                            project_file.height,
                        )?;
                        let load_elapsed = load_started_at.elapsed().as_millis();

                        let fill_started_at = Instant::now();
                        let changed = fill_region_using_label_map(
                            &mut paint_image,
                            &label_map,
                            entry.region_id,
                            entry.sample_x,
                            entry.sample_y,
                            fill_color,
                        );
                        let fill_elapsed = fill_started_at.elapsed().as_millis();

                        let save_elapsed = if changed {
                            let save_started_at = Instant::now();
                            save_paint_image(&paint_path, &paint_image)?;
                            Some(save_started_at.elapsed().as_millis())
                        } else {
                            None
                        };

                        chunk_metrics.push(FillFrameMetrics {
                            frame_index: entry.frame_index,
                            changed,
                            materialize_ms: materialize_elapsed,
                            load_ms: load_elapsed,
                            fill_ms: fill_elapsed,
                            save_ms: save_elapsed,
                            total_ms: frame_started_at.elapsed().as_millis(),
                        });
                    }

                    Ok(chunk_metrics)
                }),
            );
        }

        for handle in handles {
            let chunk_metrics = handle
                .join()
                .map_err(|_| String::from("A fill worker thread panicked."))??;
            metrics.extend(chunk_metrics);
        }

        Ok(())
    })?;

    metrics.sort_by_key(|entry| entry.frame_index);

    for entry in metrics {
        if entry.changed {
            updated_frames.push(entry.frame_index);
            log_message(format!(
                "fill_region frame={} changed=yes materialize_ms={} load_ms={} fill_ms={} save_ms={} total_ms={}",
                entry.frame_index,
                entry.materialize_ms,
                entry.load_ms,
                entry.fill_ms,
                entry.save_ms.unwrap_or_default(),
                entry.total_ms
            ));
        } else {
            log_message(format!(
                "fill_region frame={} changed=no materialize_ms={} load_ms={} fill_ms={} total_ms={}",
                entry.frame_index,
                entry.materialize_ms,
                entry.load_ms,
                entry.fill_ms,
                entry.total_ms
            ));
        }
    }

    log_message(format!(
        "fill_region complete frame={} track_id={} updated_frames={} workers={} elapsed_ms={}",
        frame_index,
        region.track_id,
        updated_frames.len(),
        worker_count,
        started_at.elapsed().as_millis()
    ));

    Ok(FillResult {
        track_id: region.track_id,
        updated_frames,
    })
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use which::which;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

#[cfg(windows)]
impl NoWindow for Command {
    fn no_window(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(windows)]
impl NoWindow for std::process::Command {
    fn no_window(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }
}

#[cfg(not(windows))]
trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

#[cfg(not(windows))]
impl<T> NoWindow for T {
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoInfo {
    fps: f64,
    width: i32,
    height: i32,
    duration: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertOptions {
    input: String,
    format: String,
    fps: f64,
    max_width: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConvertResult {
    output_path: String,
    duration: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    status: String,
    percent: u8,
}

fn locate_binary(name: &str) -> Result<PathBuf, String> {
    if let Ok(p) = which(name) {
        return Ok(p);
    }
    let roots = [PathBuf::from("C:\\ffmpeg-gpl"), PathBuf::from("C:\\ffmpeg")];
    for root in &roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let exe = path.join("bin").join(format!("{name}.exe"));
                    if exe.is_file() {
                        return Ok(exe);
                    }
                }
            }
        }
        let direct = root.join("bin").join(format!("{name}.exe"));
        if direct.is_file() {
            return Ok(direct);
        }
    }
    let mut extras = vec![
        PathBuf::from("C:\\ProgramData\\chocolatey\\bin").join(format!("{name}.exe")),
        PathBuf::from("C:\\Program Files\\ffmpeg\\bin").join(format!("{name}.exe")),
        PathBuf::from("C:\\Program Files (x86)\\ffmpeg\\bin").join(format!("{name}.exe")),
    ];
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        extras.push(
            PathBuf::from(&local)
                .join("Microsoft\\WinGet\\Packages\\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe\\ffmpeg-master-latest-win64-gpl\\bin")
                .join(format!("{name}.exe")),
        );
    }
    for p in extras {
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(format!("{name} not found. Install ffmpeg and make sure it is on PATH."))
}

fn parse_rational(s: &str) -> f64 {
    if let Some((num, den)) = s.split_once('/') {
        let num: f64 = num.trim().parse().unwrap_or(0.0);
        let den: f64 = den.trim().parse().unwrap_or(1.0);
        if den == 0.0 {
            0.0
        } else {
            num / den
        }
    } else {
        s.trim().parse().unwrap_or(0.0)
    }
}

async fn probe_path(input: &Path) -> Result<VideoInfo, String> {
    let ffprobe = locate_binary("ffprobe")?;
    let output = Command::new(ffprobe)
        .no_window()
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate,width,height,duration",
            "-of",
            "csv=p=0",
        ])
        .arg(input)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let line = line.trim();
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() != 4 {
        return Err("ffprobe output format unexpected".into());
    }
    Ok(VideoInfo {
        width: parts[0].parse().unwrap_or(0),
        height: parts[1].parse().unwrap_or(0),
        fps: parse_rational(parts[2]),
        duration: parts[3].parse().unwrap_or(0.0),
    })
}

#[tauri::command]
async fn probe_video(input: String) -> Result<VideoInfo, String> {
    probe_path(Path::new(&input)).await
}

fn unique_output_path(input: &Path, ext: &str) -> PathBuf {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let mut candidate = parent.join(format!("{stem}.{ext}"));
    if candidate == input {
        candidate = parent.join(format!("{stem}_gifdrop.{ext}"));
    }
    let mut n = 1;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}_{n}.{ext}"));
        n += 1;
    }
    candidate
}

#[tauri::command]
async fn convert_video(options: ConvertOptions, app: AppHandle) -> Result<ConvertResult, String> {
    let input = Path::new(&options.input);
    if !input.is_file() {
        return Err("Input file not found".into());
    }
    let info = probe_path(input).await?;
    let ext = match options.format.as_str() {
        "apng" => "apng",
        _ => "gif",
    };
    let output = unique_output_path(input, ext);
    let fps = if options.fps > 0.0 {
        options.fps
    } else if info.fps > 0.0 {
        info.fps
    } else {
        30.0
    };
    let scale = options
        .max_width
        .map_or(false, |w| w > 0 && (info.width as u32) > w);

    let ffmpeg = locate_binary("ffmpeg")?;
    let mut args: Vec<String> = vec![
        "-loglevel".into(),
        "warning".into(),
        "-nostats".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-i".into(),
        input.to_string_lossy().to_string(),
    ];

    if ext == "gif" {
        let mut filter = String::from("[0:v]");
        if scale {
            filter.push_str(&format!(
                "scale='min(iw,{}):-1:flags=lanczos',",
                options.max_width.unwrap()
            ));
        }
        filter.push_str(&format!("fps=fps={fps},"));
        filter.push_str("split[s0][s1];[s0]palettegen=max_colors=256[p];[s1][p]paletteuse=dither=bayer");
        args.push("-filter_complex".into());
        args.push(filter);
        args.push("-loop".into());
        args.push("0".into());
    } else {
        let mut vf_parts: Vec<String> = Vec::new();
        if scale {
            vf_parts.push(format!(
                "scale='min(iw,{}):-1:flags=lanczos'",
                options.max_width.unwrap()
            ));
        }
        vf_parts.push(format!("fps=fps={fps}"));
        if !vf_parts.is_empty() {
            args.push("-vf".into());
            args.push(vf_parts.join(","));
        }
        args.push("-c:v".into());
        args.push("apng".into());
        args.push("-pred".into());
        args.push("mixed".into());
        args.push("-plays".into());
        args.push("0".into());
    }

    args.push("-y".into());
    args.push(output.to_string_lossy().to_string());

    let _ = app.emit(
        "convert-progress",
        ProgressPayload {
            status: "Starting conversion".into(),
            percent: 0,
        },
    );

    let mut child = Command::new(ffmpeg)
        .no_window()
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let stdout = child.stdout.take().ok_or("failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("failed to capture stderr")?;

    let app_clone = app.clone();
    let duration = info.duration;
    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line.starts_with("out_time_us=") {
                if let Ok(us) = line[12..].parse::<i64>() {
                    if duration > 0.0 && us > 0 {
                        let pct = ((us as f64 / 1_000_000.0) / duration * 100.0) as u8;
                        let pct = pct.min(99);
                        let _ = app_clone.emit(
                            "convert-progress",
                            ProgressPayload {
                                status: "Converting".into(),
                                percent: pct,
                            },
                        );
                    }
                }
            } else if line == "progress=end" {
                let _ = app_clone.emit(
                    "convert-progress",
                    ProgressPayload {
                        status: "Finalizing".into(),
                        percent: 99,
                    },
                );
            }
        }
        Ok::<(), String>(())
    });

    let stderr_handle = tokio::spawn(async move {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(stderr);
        let _ = reader.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).to_string()
    });

    let status = child.wait().await.map_err(|e| e.to_string())?;
    let _ = stdout_handle.await.map_err(|e| e.to_string())?;
    let stderr_str = stderr_handle.await.map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!("ffmpeg failed: {}", stderr_str));
    }

    let _ = app.emit(
        "convert-progress",
        ProgressPayload {
            status: "Done".into(),
            percent: 100,
        },
    );

    Ok(ConvertResult {
        output_path: output.to_string_lossy().to_string(),
        duration: info.duration,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![probe_video, convert_video])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffprobe_reports_test_video_metadata() {
        let ffmpeg = locate_binary("ffmpeg").unwrap();
        let ffprobe = locate_binary("ffprobe").unwrap();
        assert!(ffmpeg.is_file());
        assert!(ffprobe.is_file());

        let video = std::env::temp_dir().join("gifdrop_test.mp4");
        let _ = std::fs::remove_file(&video);
        let status = std::process::Command::new(ffmpeg)
            .no_window()
            .args([
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=24",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(&video)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(video.is_file());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let info = rt.block_on(probe_path(&video)).unwrap();
        assert_eq!(info.width, 320);
        assert_eq!(info.height, 240);
        assert!((info.fps - 24.0).abs() < 0.01);
        assert!(info.duration >= 1.9 && info.duration <= 2.1);
    }
}

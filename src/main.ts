import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

interface VideoInfo {
  fps: number;
  width: number;
  height: number;
  duration: number;
}

interface ProgressPayload {
  status: string;
  percent: number;
}

interface ConvertResult {
  outputPath: string;
  duration: number;
}

const dropZone = document.getElementById("drop-zone")!;
const panel = document.getElementById("panel")!;
const fileNameEl = document.getElementById("file-name")!;
const fileMetaEl = document.getElementById("file-meta")!;
const fpsInput = document.getElementById("fps-input") as HTMLInputElement;
const maxWidthInput = document.getElementById("max-width-input") as HTMLInputElement;
const formatSelect = document.getElementById("format-select")!;
const convertBtn = document.getElementById("convert-btn") as HTMLButtonElement;
const progress = document.getElementById("progress")!;
const progressBar = document.getElementById("progress-bar")!;
const progressStatus = document.getElementById("progress-status")!;
const progressPercent = document.getElementById("progress-percent")!;
const result = document.getElementById("result")!;
const outputPathEl = document.getElementById("output-path")!;
const openBtn = document.getElementById("open-btn") as HTMLButtonElement;
const sourcePreview = document.getElementById("source-preview") as HTMLVideoElement;
const outputPreview = document.getElementById("output-preview") as HTMLImageElement;
const outputPlaceholder = document.getElementById("output-placeholder")!;

let currentInput: string | null = null;
let selectedFormat = "gif";

// --- Haptic-style feedback: click sound + ripple ---

let audioCtx: AudioContext | null = null;

function playClick(freq = 220, duration = 0.04, volume = 0.06) {
  try {
    if (!audioCtx) {
      audioCtx = new AudioContext();
    }
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    osc.frequency.value = freq;
    osc.type = "sine";
    gain.gain.setValueAtTime(volume, audioCtx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, audioCtx.currentTime + duration);
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    osc.start();
    osc.stop(audioCtx.currentTime + duration);
  } catch {
    // AudioContext not available — silently skip
  }
}

function spawnRipple(el: HTMLElement, x: number, y: number) {
  const rect = el.getBoundingClientRect();
  const size = Math.max(rect.width, rect.height) * 0.6;
  const ripple = document.createElement("span");
  ripple.className = "ripple";
  ripple.style.width = `${size}px`;
  ripple.style.height = `${size}px`;
  ripple.style.left = `${x - rect.left - size / 2}px`;
  ripple.style.top = `${y - rect.top - size / 2}px`;
  el.appendChild(ripple);
  setTimeout(() => ripple.remove(), 500);
}

function tactileClick(el: HTMLElement, freq = 220) {
  return (e: MouseEvent) => {
    playClick(freq);
    spawnRipple(el, e.clientX, e.clientY);
  };
}

// Attach feedback to all interactive buttons
[convertBtn, openBtn].forEach((btn) => {
  btn.addEventListener("mousedown", tactileClick(btn, 200));
});

dropZone.addEventListener("mousedown", (e) => {
  playClick(180);
  spawnRipple(dropZone, e.clientX, e.clientY);
});

// Segment buttons get a higher-pitched click
formatSelect.addEventListener("mousedown", (e) => {
  const target = e.target as HTMLElement;
  if (target.classList.contains("segment")) {
    playClick(320);
    spawnRipple(target, e.clientX, e.clientY);
  }
});

function pulseDropZone() {
  dropZone.classList.remove("pulse");
  void dropZone.offsetWidth; // force reflow to restart animation
  dropZone.classList.add("pulse");
  playClick(440, 0.08, 0.08);
}

function basename(path: string): string {
  const idx = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

function setActive(active: boolean) {
  if (active) {
    dropZone.classList.add("active");
  } else {
    dropZone.classList.remove("active");
  }
}

async function loadVideo(input: string) {
  currentInput = input;
  fileNameEl.textContent = basename(input);
  panel.classList.remove("hidden");
  result.classList.add("hidden");
  progress.classList.add("hidden");
  convertBtn.disabled = false;
  convertBtn.textContent = "Convert";

  sourcePreview.src = convertFileSrc(input);
  sourcePreview.load();
  outputPreview.classList.add("hidden");
  outputPlaceholder.classList.remove("hidden");
  outputPreview.src = "";

  try {
    const info = await invoke<VideoInfo>("probe_video", { input });
    const sourceFps = info.fps > 0 ? info.fps : 30;
    fpsInput.value = sourceFps.toFixed(2);
    fileMetaEl.textContent = `${info.width}×${info.height} · ${info.duration.toFixed(2)}s · ${sourceFps.toFixed(2)} fps`;
  } catch (err) {
    fileMetaEl.textContent = "Could not read video info";
    console.error(err);
  }
}

formatSelect.addEventListener("click", (e) => {
  const target = e.target as HTMLElement;
  if (!target.classList.contains("segment")) return;
  for (const btn of formatSelect.querySelectorAll(".segment")) {
    btn.classList.remove("active");
  }
  target.classList.add("active");
  selectedFormat = target.getAttribute("data-value") || "gif";
});

const VIDEO_EXTENSIONS = ["mp4", "mov", "avi", "mkv", "webm", "m4v", "wmv", "flv", "ts", "3gp"];

async function pickFile() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "Video", extensions: VIDEO_EXTENSIONS }],
    });
    if (typeof selected === "string" && selected.length > 0) {
      loadVideo(selected);
    }
  } catch (err) {
    console.error(err);
  }
}

dropZone.addEventListener("click", () => {
  pickFile();
});

getCurrentWebview().onDragDropEvent((event) => {
  const type = event.payload.type;
  if (type === "enter" || type === "over") {
    setActive(true);
  } else if (type === "leave") {
    setActive(false);
  } else if (type === "drop") {
    setActive(false);
    pulseDropZone();
    const paths: string[] = (event.payload as any).paths || [];
    if (paths.length > 0) {
      loadVideo(paths[0]);
    }
  }
});

document.addEventListener("dragover", (e) => e.preventDefault());
document.addEventListener("drop", (e) => e.preventDefault());

listen("convert-progress", (event: { payload: ProgressPayload }) => {
  const { status, percent } = event.payload;
  progress.classList.remove("hidden");
  progressBar.style.width = `${percent}%`;
  progressStatus.textContent = status;
  progressPercent.textContent = `${percent}%`;
});

convertBtn.addEventListener("click", async () => {
  if (!currentInput) return;

  const fps = parseFloat(fpsInput.value);
  const maxWidth = parseInt(maxWidthInput.value, 10);
  convertBtn.disabled = true;
  convertBtn.textContent = "Converting...";
  result.classList.add("hidden");
  progress.classList.remove("hidden");

  try {
    const res = await invoke<ConvertResult>("convert_video", {
      options: {
        input: currentInput,
        format: selectedFormat,
        fps,
        maxWidth: maxWidth > 0 ? maxWidth : null,
      },
    });
    outputPathEl.textContent = res.outputPath;
    result.classList.remove("hidden");
    progressStatus.textContent = "Done";
    progressPercent.textContent = "100%";
    progressBar.style.width = "100%";
    outputPreview.src = convertFileSrc(res.outputPath);
    outputPreview.classList.remove("hidden");
    outputPlaceholder.classList.add("hidden");
    // Success chime: two ascending notes
    playClick(523, 0.08, 0.07);
    setTimeout(() => playClick(784, 0.12, 0.07), 80);
  } catch (err) {
    progressStatus.textContent = `Error: ${err}`;
    // Error buzz: low harsh note
    playClick(110, 0.15, 0.08);
    progressPercent.textContent = "";
    console.error(err);
  } finally {
    convertBtn.disabled = false;
    convertBtn.textContent = "Convert";
  }
});

openBtn.addEventListener("click", async () => {
  const path = outputPathEl.textContent?.trim();
  if (!path) return;
  try {
    await revealItemInDir(path);
  } catch (err) {
    console.error(err);
  }
});

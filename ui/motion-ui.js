import { animate, animateLayout } from "./vendor/motion.js";
import createGlobe from "./vendor/cobe.js";

const reduced = matchMedia("(prefers-reduced-motion: reduce)");
const queue = document.getElementById("queue-list");
let selectedId;
function select(id) {
  if (!id || id === selectedId) return;
  selectedId = id;
  const update = () => {
    queue.querySelector(".selected-indicator")?.remove();
    const button = [...queue.querySelectorAll(".qbtn")].find(b => b.dataset.id === id);
    if (!button) return;
    const indicator = document.createElement("div");
    indicator.className = "selected-indicator";
    indicator.dataset.layoutId = "case-selection";
    indicator.setAttribute("aria-hidden", "true");
    button.before(indicator);
  };
  if (reduced.matches) update();
  else Promise.resolve(animateLayout(queue, update, { duration: 0.3, ease: [0.4, 0, 0.2, 1] })).catch(update);
}
globalThis.CasefileMotion = { select };

function illustration() {
  const source = document.createElement("canvas");
  source.width = 640; source.height = 320;
  const c = source.getContext("2d");
  c.fillStyle = "#eeeae1"; c.fillRect(0, 0, 640, 320);
  c.fillStyle = "#c9d2c3"; c.fillRect(0, 0, 160, 320);
  c.fillStyle = "#c3cfcb"; c.beginPath(); c.moveTo(0, 130); c.bezierCurveTo(180, 90, 160, 280, 0, 260); c.fill();
  c.strokeStyle = "#faf9f5"; c.lineWidth = 23;
  for (let x = 220; x < 640; x += 120) { c.beginPath(); c.moveTo(x, 0); c.lineTo(x - 75, 320); c.stroke(); }
  for (let y = 70; y < 320; y += 110) { c.beginPath(); c.moveTo(140, y); c.lineTo(640, y + 25); c.stroke(); }
  for (let x = 260; x < 640; x += 120) for (let y = 16; y < 320; y += 110) {
    c.fillStyle = "#b5b0a4"; c.fillRect(x - y / 5, y, 39, 29);
    c.fillStyle = "#d6c7b3"; c.fillRect(x - y / 5 + 48, y + 3, 28, 26);
  }
  c.fillStyle = "#3e4c42"; c.beginPath(); c.arc(345, 165, 10, 0, Math.PI * 2); c.fill();
  c.strokeStyle = "#ffffff"; c.lineWidth = 4; c.stroke();
  return source;
}

let dispose = () => {};
function mount() {
  dispose();
  const preview = document.querySelector(".location-preview");
  if (!preview) return;
  const canvas = preview.querySelector(".globe-canvas");
  const scene = preview.querySelector(".scene-canvas");
  const replay = preview.querySelector(".globe-replay");
  const context = scene.getContext("2d");
  const source = illustration();
  context.drawImage(source, 0, 0);
  let globe, controls, rotation, timer, alive = true, dragging = false, lastX = 0;
  let phi = 4.8, progress = 0, width = 400, height = 320;
  const offset = { value: 0 };
  const resize = new ResizeObserver(entries => {
    const bounds = entries[0].contentRect;
    width = Math.max(bounds.width, 1); height = Math.max(bounds.height, 1);
    globe?.toggle(true);
  });
  resize.observe(canvas);
  const show = t => {
    progress = t;
    const blend = Math.max(0, Math.min(1, (t - 0.65) / 0.3));
    canvas.style.opacity = String(1 - blend);
    scene.style.opacity = String(blend);
    scene.style.transform = `scale(${1.12 - blend * 0.12})`;
    if (t >= 1) globe?.toggle(false);
  };
  try {
    if (!canvas.getContext("webgl2") && !canvas.getContext("webgl")) throw new Error("WebGL unavailable");
    globe = createGlobe(canvas, {
      width: 800, height: 800, devicePixelRatio: Math.min(devicePixelRatio, 2),
      phi, theta: 0.3, dark: 0, diffuse: 0.4, mapSamples: 16000, mapBrightness: 1.2,
      baseColor: [1, 1, 1], markerColor: [251 / 255, 100 / 255, 21 / 255], glowColor: [1, 1, 1],
      markers: [{ location: [47.6, -122.3], size: 0.055 }],
      onRender: state => {
        if (!alive) return;
        state.phi = phi + offset.value;
        state.theta = 0.3;
        state.width = Math.round(width * 2); state.height = Math.round(height * 2);
        const zoom = Math.max(0, (progress - 0.25) / 0.75);
        state.scale = 0.88 + zoom * zoom * 3.2;
        if (reduced.matches || progress >= 1) globe?.toggle(false);
      },
    });
  } catch {
    canvas.hidden = true;
    replay.textContent = "Show illustration";
    replay.disabled = true;
    show(1);
    dispose = () => { alive = false; resize.disconnect(); globe?.destroy(); };
    return;
  }
  const play = () => {
    clearTimeout(timer); controls?.stop(); rotation?.stop();
    dragging = false; offset.value = 0;
    if (reduced.matches) { show(1); return; }
    show(0); globe.toggle(true);
    controls = animate(0, 1, { duration: 3.4, ease: [0.4, 0, 0.2, 1], onUpdate: t => { phi = 4.8 + Math.min(t / 0.45, 1) * 0.45; show(t); } });
  };
  const begin = event => {
    if (event.button !== 0) return;
    clearTimeout(timer); controls?.stop(); rotation?.stop();
    show(0); globe.toggle(true); dragging = true; lastX = event.clientX;
    canvas.setPointerCapture(event.pointerId); canvas.style.cursor = "grabbing";
  };
  const move = event => {
    if (!dragging) return;
    const delta = event.clientX - lastX; lastX = event.clientX;
    rotation?.stop();
    rotation = animate(offset, { value: offset.value + delta / 1400 }, { type: "spring", mass: 1, damping: 30, stiffness: 100 });
  };
  const end = () => { dragging = false; canvas.style.cursor = "grab"; };
  canvas.addEventListener("pointerdown", begin);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", end);
  canvas.addEventListener("pointercancel", end);
  canvas.addEventListener("lostpointercapture", end);
  canvas.addEventListener("keydown", event => {
    if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault(); controls?.stop(); clearTimeout(timer); show(0);
    offset.value += event.key === "ArrowRight" ? 0.15 : -0.15;
    globe.toggle(true);
  });
  replay.addEventListener("click", play);
  const visibility = new IntersectionObserver(entries => {
    if (!entries[0].isIntersecting) globe.toggle(false);
    else if (progress < 1) globe.toggle(true);
  });
  visibility.observe(canvas);
  show(0);
  if (!reduced.matches) timer = setTimeout(play, 900);
  dispose = () => {
    alive = false; clearTimeout(timer); controls?.stop(); rotation?.stop();
    resize.disconnect(); visibility.disconnect(); globe.destroy();
  };
}
new MutationObserver(mount).observe(document.getElementById("case-card"), { childList: true });
new MutationObserver(() => {
  selectedId = undefined;
  select(queue.querySelector('[aria-current="true"]')?.dataset.id);
}).observe(queue, { childList: true });
select(queue.querySelector('[aria-current="true"]')?.dataset.id);
mount();

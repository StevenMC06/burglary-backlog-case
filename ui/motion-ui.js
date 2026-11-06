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
const surfaceAnimations = new WeakMap();
function revealSurface(node, distance = 12) {
  surfaceAnimations.get(node)?.stop();
  if (reduced.matches) return;
  surfaceAnimations.set(node, animate(node, { opacity: [0, 1], transform: [`translateY(${distance}px)`, "translateY(0px)"] }, { duration: 0.32, ease: [0.2, 0.8, 0.2, 1] }));
}
globalThis.CasefileMotion = {
  select,
  reveal: revealSurface,
  caseTransition: () => document.querySelectorAll('.case-heading, .case-body').forEach(node => revealSurface(node, 8)),
  drawers: () => document.querySelectorAll('.drawer:not([hidden])').forEach(node => revealSurface(node, 16)),
};
const caseMenu = document.getElementById("case-switcher");
caseMenu.addEventListener("toggle", () => { if (caseMenu.open) revealSurface(caseMenu.querySelector(".queue"), -6); });

function enhanceTabs(group, index) {
  const buttons = [...group.querySelectorAll('[role="tab"]')];
  if (!buttons.length) return;
  group.classList.add("motion-tabs");
  for (const button of buttons) {
    const slot = document.createElement("div");
    slot.className = "tab-slot";
    slot.setAttribute("role", "presentation");
    button.before(slot); slot.appendChild(button);
  }
  let active, generation = 0, panelAnimation;
  const sync = (initial = false) => {
    const selected = buttons.find(button => button.getAttribute("aria-selected") === "true");
    if (!selected || selected === active) return;
    active = selected;
    const request = ++generation;
    const update = () => {
      if (request !== generation) return;
      group.querySelectorAll(".tab-indicator").forEach(node => node.remove());
      const indicator = document.createElement("span");
      indicator.className = "tab-indicator";
      indicator.dataset.layoutId = "tab-selection-" + index;
      indicator.setAttribute("aria-hidden", "true");
      selected.parentElement.prepend(indicator);
    };
    if (initial || reduced.matches) update();
    else Promise.resolve(animateLayout(group, update, { duration: 0.3, ease: [0.4, 0, 0.2, 1] })).catch(update);
    panelAnimation?.stop();
    const panel = document.getElementById(selected.getAttribute("aria-controls"));
    if (panel && !initial && !reduced.matches) {
      panelAnimation = animate(panel, { opacity: [0, 1], transform: ["translateY(5px)", "translateY(0px)"] }, { duration: 0.22, ease: [0.2, 0.8, 0.2, 1] });
    }
  };
  sync(true);
  new MutationObserver(() => sync()).observe(group, { attributes: true, subtree: true, attributeFilter: ["aria-selected"] });
  reduced.addEventListener("change", () => {
    if (reduced.matches) panelAnimation?.complete();
  });
}
document.querySelectorAll('[role="tablist"]').forEach(enhanceTabs);

function illustration() {
  const source = document.createElement("canvas");
  source.width = 640; source.height = 320;
  const c = source.getContext("2d");
  c.fillStyle = "#e6ecef"; c.fillRect(0, 0, 640, 320);
  c.fillStyle = "#c7d9d2"; c.fillRect(0, 0, 145, 320);
  c.fillStyle = "#8fb2c0";
  c.beginPath(); c.moveTo(0, 40); c.bezierCurveTo(135, 70, 30, 155, 112, 320); c.lineTo(0, 320); c.fill();
  c.strokeStyle = "#f5f8f5"; c.lineWidth = 5;
  c.beginPath(); c.moveTo(105, 0); c.bezierCurveTo(150, 100, 60, 180, 145, 320); c.stroke();
  c.fillStyle = "#f6f8f9"; c.fillRect(151, 0, 45, 320); c.fillRect(145, 138, 495, 48);
  c.fillStyle = "#84949e"; c.fillRect(158, 0, 31, 320); c.fillRect(145, 145, 495, 34);
  c.strokeStyle = "#eef3f4"; c.lineWidth = 1.5; c.setLineDash([10, 10]);
  c.beginPath(); c.moveTo(174, 0); c.lineTo(174, 320); c.moveTo(196, 162); c.lineTo(640, 162); c.stroke(); c.setLineDash([]);
  for (let i = 0; i < 5; i++) { c.fillStyle = "#f7fafb"; c.fillRect(159 + i * 6, 128, 3, 12); c.fillRect(196, 147 + i * 6, 12, 3); }
  const tree = (x, y, r = 9) => {
    c.fillStyle = "#536b6025"; c.beginPath(); c.ellipse(x + 5, y + 5, r + 2, r, 0, 0, Math.PI * 2); c.fill();
    c.fillStyle = "#688c78"; c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); c.fill();
    c.fillStyle = "#8bad94"; c.beginPath(); c.arc(x - 2, y - 2, r * .65, 0, Math.PI * 2); c.fill();
  };
  for (const [x, y] of [[125,28],[119,81],[132,230],[90,280],[35,25],[48,188]]) tree(x,y,11);
  function house(x,y,lower,index) {
    c.fillStyle = index % 2 ? "#d1ded4" : "#d9e4dc"; c.fillRect(x-10,y-10,111,112);
    c.strokeStyle = "#bccbc3"; c.lineWidth = 1; c.strokeRect(x-10,y-10,111,112);
    c.fillStyle = "#dfe4e7"; c.fillRect(x+58,lower?y-4:y+64,22,lower?18:35);
    c.fillStyle = "#34465325"; c.fillRect(x+5,y+8,72,57);
    c.fillStyle = "#ecf0f2"; c.fillRect(x,y+4,70,55);
    c.fillStyle = "#697f8d"; c.beginPath(); c.moveTo(x-4,y+26); c.lineTo(x+35,y-1); c.lineTo(x+74,y+26); c.closePath(); c.fill();
    c.fillStyle = "#465e70"; c.beginPath(); c.moveTo(x-4,y+26); c.lineTo(x+35,y+12); c.lineTo(x+74,y+26); c.lineTo(x+35,y+39); c.closePath(); c.fill();
    c.strokeStyle = "#90a2ac"; c.beginPath(); c.moveTo(x+35,y+12); c.lineTo(x+35,y+39); c.stroke();
    c.fillStyle = "#b9ccd6"; c.fillRect(x+9,y+42,10,8); c.fillRect(x+48,y+42,10,8);
    c.fillStyle = "#354f5c"; c.fillRect(x+29,y+44,11,15);
    c.fillStyle = "#dbe3e7"; c.fillRect(x+26,y+60,17,4);
    c.fillStyle = "#9aabb1"; c.fillRect(x+53,y+8,7,12);
    tree(x+84,y+72,7); tree(x+3,y+77,6);
    c.fillStyle = index % 2 ? "#e8eced" : "#526c80"; c.fillRect(x+60,lower?y-4:y+78,16,9);
    c.fillStyle = "#a6bbc8"; c.fillRect(x+64,lower?y-3:y+79,7,7);
  }
  [225,360,495].forEach((x,i)=>{house(x,20,false,i);house(x,215,true,i+3);});
  c.save(); c.translate(301,153); c.rotate(Math.PI/2); c.fillStyle="#f0f3f5";c.fillRect(-10,-4,21,8);c.fillStyle="#8ea3af";c.fillRect(-3,-3,8,6);c.restore();
  c.strokeStyle = "#173b63"; c.lineWidth = 2; c.beginPath(); c.arc(395,75,15,0,Math.PI*2); c.stroke();
  c.fillStyle = "#173b63"; c.beginPath(); c.arc(395,75,5,0,Math.PI*2); c.fill();
  c.fillStyle = "#f9fbfcdd"; c.fillRect(215,191,159,18); c.fillStyle="#4b6170"; c.font="10px system-ui"; c.fillText("ILLUSTRATIVE RESIDENTIAL BLOCK",222,204);
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
  let phi = 4.8, progress = 0;
  const offset = { value: 0 };
  const resize = new ResizeObserver(() => globe?.toggle(true));
  resize.observe(canvas);
  const show = t => {
    progress = t;
    const blend = Math.max(0, Math.min(1, (t - 0.76) / 0.2));
    canvas.style.opacity = String(1 - blend);
    scene.style.opacity = String(blend);
    scene.style.transform = `scale(${1.8 - blend * 0.8})`;
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
        state.width = canvas.width; state.height = canvas.height;
        const zoom = Math.max(0, (progress - 0.25) / 0.75);
        state.scale = 0.78 + zoom * zoom * 3.4;
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
    controls = animate(0, 1, { duration: 6.4 / 0.75, ease: [0.4, 0, 0.2, 1], onUpdate: t => { phi = 4.8 + t * (0.45 + Math.PI * 3); show(t); } });
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

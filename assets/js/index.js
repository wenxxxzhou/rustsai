const projectNames = [
  "edl2srt @GLM-5.2",
  "WhisperLiveKitp @Claude Sonnet 5",
  "qinfoss @GLM-5.2",
];

const canvas = document.getElementById("matrixCanvas");
const ctx = canvas.getContext("2d");
const projectNameEl = document.getElementById("projectName");

const glyphSets = [
  "01",
  "0123456789",
  "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
  "アイウエオカキクケコサシスセソタチツテト",
  "$#@%&*+=<>?/|"
];

let width = 0;
let height = 0;
let animationId = null;
let lastTime = 0;
let streams = [];
let fontSize = 20;
let columnGap = 20;
let projectIndex = 0;
let projectTimer = null;

function pickRandom(source) {
  return source[Math.floor(Math.random() * source.length)];
}

function randomGlyph() {
  const set = pickRandom(glyphSets);
  return pickRandom(set);
}

function updateProjectName() {
  if (!projectNames.length) {
    projectNameEl.textContent = "no project";
    return;
  }

  projectNameEl.style.opacity = "0.35";

  setTimeout(() => {
    projectNameEl.textContent = projectNames[projectIndex];
    projectNameEl.style.opacity = "1";
    projectIndex = (projectIndex + 1) % projectNames.length;
  }, 180);
}

function startProjectTicker() {
  updateProjectName();

  if (projectTimer) {
    clearInterval(projectTimer);
  }

  projectTimer = setInterval(updateProjectName, 2200);
}

function createStream(x) {
  return {
    x,
    y: Math.random() * -height,
    speed: 0.9 + Math.random() * 1.2,
    length: 14 + Math.floor(Math.random() * 16),
    alpha: 0.72 + Math.random() * 0.22,
    chars: Array.from({ length: 48 }, () => randomGlyph())
  };
}

function resizeCanvas() {
  width = window.innerWidth;
  height = window.innerHeight;

  canvas.width = width * devicePixelRatio;
  canvas.height = height * devicePixelRatio;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.scale(devicePixelRatio, devicePixelRatio);
  ctx.textBaseline = "top";
  ctx.textAlign = "left";

  fontSize = width < 768 ? 16 : 20;
  columnGap = fontSize;

  const count = Math.ceil(width / columnGap);
  streams = [];

  for (let i = 0; i < count; i++) {
    streams.push(createStream(i * columnGap));
  }
}

function drawChar(stream, index, headY) {
  const y = headY - index * fontSize;
  if (y < -fontSize || y > height + fontSize) return;

  const fade = 1 - index / stream.length;
  const alpha = fade * stream.alpha;

  const changeRate =
    index === 0 ? 0.75 :
    index < 4 ? 0.22 :
    index < 10 ? 0.08 :
    0.03;

  const glyph = Math.random() < changeRate
    ? randomGlyph()
    : stream.chars[index % stream.chars.length];

  ctx.font = `${fontSize}px monospace`;

  if (index === 0) {
    ctx.fillStyle = `rgba(235,255,235,${Math.min(1, alpha + 0.18)})`;
    ctx.shadowColor = "rgba(160,255,180,0.55)";
    ctx.shadowBlur = 6;
  } else if (index < 4) {
    ctx.fillStyle = `rgba(150,255,170,${Math.min(1, alpha)})`;
    ctx.shadowColor = "rgba(60,255,100,0.18)";
    ctx.shadowBlur = 2;
  } else {
    ctx.fillStyle = `rgba(20,255,70,${alpha})`;
    ctx.shadowBlur = 0;
  }

  ctx.fillText(glyph, stream.x, y);
}

function updateStream(stream, deltaFactor) {
  stream.y += stream.speed * fontSize * 0.075 * deltaFactor;

  for (let n = 0; n < 2; n++) {
    if (Math.random() < 0.12) {
      const idx = Math.floor(Math.random() * Math.min(12, stream.chars.length));
      stream.chars[idx] = randomGlyph();
    }
  }

  if (Math.random() < 0.04) {
    const idx = Math.floor(Math.random() * stream.chars.length);
    stream.chars[idx] = randomGlyph();
  }

  if (stream.y - stream.length * fontSize > height && Math.random() > 0.985) {
    stream.y = Math.random() * -height * 0.3;
    stream.speed = 0.9 + Math.random() * 1.2;
    stream.length = 14 + Math.floor(Math.random() * 16);
    stream.alpha = 0.72 + Math.random() * 0.22;
    stream.chars = Array.from({ length: 48 }, () => randomGlyph());
  }
}

function draw(timestamp = 0) {
  const delta = timestamp - lastTime;
  lastTime = timestamp;
  const deltaFactor = Math.max(0.75, Math.min(1.6, delta / 16.67));

  ctx.fillStyle = "rgba(0, 0, 0, 0.16)";
  ctx.fillRect(0, 0, width, height);

  for (const stream of streams) {
    for (let i = 0; i < stream.length; i++) {
      drawChar(stream, i, stream.y);
    }
    updateStream(stream, deltaFactor);
  }

  ctx.shadowBlur = 0;
  animationId = requestAnimationFrame(draw);
}

function startMatrix() {
  if (animationId) {
    cancelAnimationFrame(animationId);
  }

  resizeCanvas();
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, width, height);
  lastTime = performance.now();
  draw(lastTime);
}

let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(startMatrix, 120);
});

startProjectTicker();
startMatrix();
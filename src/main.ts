import "./styles.css";

type TauriGlobal = {
  core?: {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  };
};

type PetAsset = {
  id: string;
  display_name: string;
  description: string;
  image_data_url: string;
};

type WindowPosition = {
  x: number;
  y: number;
};

const FRAME_WIDTH = 192;
const FRAME_HEIGHT = 208;
const MAX_FRAME_COUNT = 8;
const FRAME_MS = 140;
const EXIT_ANIMATION_MS = 900;
const STATE_ROWS = {
  idle: 0,
  "running-right": 1,
  "running-left": 2,
  waving: 3,
  jumping: 4,
  failed: 5,
  waiting: 6,
  running: 7,
  review: 8
} as const;

type PetState = keyof typeof STATE_ROWS;

function requiredNode<T extends Element>(selector: string) {
  const node = document.querySelector<T>(selector);
  if (!node) {
    throw new Error(`Missing required DOM node: ${selector}`);
  }
  return node;
}

function invoke<T>(cmd: string, args?: Record<string, unknown>) {
  const tauri = (window as Window & { __TAURI__?: TauriGlobal }).__TAURI__;
  if (!tauri?.core?.invoke) {
    throw new Error("Tauri runtime is not available");
  }
  return tauri.core.invoke<T>(cmd, args);
}

const canvas = requiredNode<HTMLCanvasElement>("#pet");
const message = requiredNode<HTMLDivElement>("#message");
const maybeContext = canvas.getContext("2d", { alpha: true });

if (!maybeContext) {
  throw new Error("Canvas rendering is not available");
}

const context = maybeContext;
context.imageSmoothingEnabled = false;

let sprite: HTMLImageElement | null = null;
let state: PetState = "idle";
let frame = 0;
let lastFrameAt = 0;
let temporaryStateUntil = 0;
let animationRequest = 0;
let quitTimer: number | null = null;
let pendingDragPointerId: number | null = null;
let drag: {
  pointerId: number;
  startScreenX: number;
  startScreenY: number;
  startWindowX: number;
  startWindowY: number;
  lastScreenX: number;
  moved: boolean;
} | null = null;
let suppressClickUntil = 0;
let visibleFramesByRow = new Map<number, number[]>([
  [STATE_ROWS.idle, [0]]
]);

function setMessage(text: string) {
  message.textContent = text;
  message.classList.toggle("visible", text.length > 0);
}

function setState(nextState: PetState, durationMs = 0) {
  if (state === nextState && durationMs === 0) {
    return;
  }
  state = nextState;
  frame = 0;
  temporaryStateUntil = durationMs > 0 ? performance.now() + durationMs : 0;
}

function requestQuit() {
  if (quitTimer !== null) {
    return;
  }

  drag = null;
  pendingDragPointerId = null;
  suppressClickUntil = performance.now() + EXIT_ANIMATION_MS;
  setState("failed");

  quitTimer = window.setTimeout(() => {
    void invoke<void>("quit_app");
  }, EXIT_ANIMATION_MS);
}

function draw(now: number) {
  if (temporaryStateUntil > 0 && now > temporaryStateUntil) {
    temporaryStateUntil = 0;
    setState("idle");
  }

  const image = sprite;
  if (!image) {
    animationRequest = requestAnimationFrame(draw);
    return;
  }

  const row = STATE_ROWS[state];
  const visibleFrames = visibleFramesByRow.get(row) ?? [0];

  if (now - lastFrameAt >= FRAME_MS) {
    frame = (frame + 1) % visibleFrames.length;
    lastFrameAt = now;
  }

  const column = visibleFrames[frame] ?? 0;
  context.clearRect(0, 0, FRAME_WIDTH, FRAME_HEIGHT);
  context.drawImage(
    image,
    column * FRAME_WIDTH,
    row * FRAME_HEIGHT,
    FRAME_WIDTH,
    FRAME_HEIGHT,
    0,
    0,
    FRAME_WIDTH,
    FRAME_HEIGHT
  );

  animationRequest = requestAnimationFrame(draw);
}

function countVisiblePixels(
  context2d: CanvasRenderingContext2D,
  image: HTMLImageElement,
  row: number,
  column: number
) {
  context2d.clearRect(0, 0, FRAME_WIDTH, FRAME_HEIGHT);
  context2d.drawImage(
    image,
    column * FRAME_WIDTH,
    row * FRAME_HEIGHT,
    FRAME_WIDTH,
    FRAME_HEIGHT,
    0,
    0,
    FRAME_WIDTH,
    FRAME_HEIGHT
  );

  const pixels = context2d.getImageData(0, 0, FRAME_WIDTH, FRAME_HEIGHT).data;
  let visible = 0;
  for (let index = 3; index < pixels.length; index += 4) {
    if (pixels[index] > 16) {
      visible += 1;
    }
  }
  return visible;
}

function findVisibleFrames(image: HTMLImageElement) {
  const probe = document.createElement("canvas");
  probe.width = FRAME_WIDTH;
  probe.height = FRAME_HEIGHT;
  const probeContext = probe.getContext("2d", { alpha: true, willReadFrequently: true });

  if (!probeContext) {
    return new Map<number, number[]>([[STATE_ROWS.idle, [0]]]);
  }

  const frames = new Map<number, number[]>();
  for (const row of Object.values(STATE_ROWS)) {
    const rowFrames: number[] = [];
    for (let column = 0; column < MAX_FRAME_COUNT; column += 1) {
      if (countVisiblePixels(probeContext, image, row, column) > 24) {
        rowFrames.push(column);
      }
    }
    frames.set(row, rowFrames.length > 0 ? rowFrames : [0]);
  }
  return frames;
}

async function loadPet() {
  try {
    const asset = await invoke<PetAsset>("load_default_pet");
    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      sprite = image;
      visibleFramesByRow = findVisibleFrames(image);
      canvas.title = `${asset.display_name}: ${asset.description}`;
      setMessage("");
    };
    image.onerror = () => setMessage("Failed to load pet");
    image.src = asset.image_data_url;
  } catch (error) {
    console.error(error);
    setMessage("No pet found");
  }
}

canvas.addEventListener("click", () => {
  if (performance.now() < suppressClickUntil) {
    return;
  }
  setState("waving", 1600);
});

canvas.addEventListener("dblclick", () => {
  if (performance.now() < suppressClickUntil) {
    return;
  }
  setState("jumping", 1300);
});

canvas.addEventListener("pointerdown", (event) => {
  if (event.button !== 0) {
    return;
  }

  event.preventDefault();
  canvas.setPointerCapture(event.pointerId);
  pendingDragPointerId = event.pointerId;

  void invoke<WindowPosition>("pet_window_position")
    .then((position) => {
      if (pendingDragPointerId !== event.pointerId) {
        return;
      }

      drag = {
        pointerId: event.pointerId,
        startScreenX: event.screenX,
        startScreenY: event.screenY,
        startWindowX: position.x,
        startWindowY: position.y,
        lastScreenX: event.screenX,
        moved: false
      };
    })
    .catch((error) => {
      console.error(error);
    });
});

canvas.addEventListener("pointermove", (event) => {
  if (!drag || event.pointerId !== drag.pointerId) {
    if (pendingDragPointerId === event.pointerId) {
      pendingDragPointerId = null;
    }
    return;
  }

  const deltaX = event.screenX - drag.startScreenX;
  const deltaY = event.screenY - drag.startScreenY;
  if (Math.abs(deltaX) > 2 || Math.abs(deltaY) > 2) {
    drag.moved = true;
  }

  const directionDelta = event.screenX - drag.lastScreenX;
  if (directionDelta > 0) {
    setState("running-right");
  } else if (directionDelta < 0) {
    setState("running-left");
  }
  drag.lastScreenX = event.screenX;

  void invoke<void>("move_pet_window", {
    x: Math.round(drag.startWindowX + deltaX),
    y: Math.round(drag.startWindowY + deltaY)
  });
});

function finishDrag(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.pointerId) {
    return;
  }

  if (drag.moved) {
    suppressClickUntil = performance.now() + 250;
  }

  drag = null;
  pendingDragPointerId = null;
  setState("idle");

  if (canvas.hasPointerCapture(event.pointerId)) {
    canvas.releasePointerCapture(event.pointerId);
  }
}

canvas.addEventListener("pointerup", finishDrag);
canvas.addEventListener("pointercancel", finishDrag);

canvas.addEventListener("contextmenu", (event) => {
  event.preventDefault();
});

window.addEventListener("keydown", (event) => {
  if (event.key === "Escape" || event.key.toLowerCase() === "q") {
    requestQuit();
    return;
  }
  if (event.key === "1") {
    setState("idle");
    return;
  }
  if (event.key === "2") {
    setState("running");
    return;
  }
  if (event.key === "3") {
    setState("waiting");
    return;
  }
  if (event.key === "4") {
    setState("review");
    return;
  }
  if (event.key === "5") {
    setState("failed");
  }
});

void loadPet();
animationRequest = requestAnimationFrame(draw);

window.addEventListener("beforeunload", () => {
  cancelAnimationFrame(animationRequest);
});

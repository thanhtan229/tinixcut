export interface CanvasPosition {
  x: number;
  y: number;
}

export interface CanvasTransform {
  position_x: number;
  position_y: number;
  scale_x: number;
  scale_y: number;
  rotation: number;
}

export type CanvasTransformArrow = "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown";
export type CanvasCropEdge = "top" | "right" | "bottom" | "left";
export interface CanvasCrop {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export function canvasPositionFromPointerDelta(
  start: CanvasPosition,
  deltaX: number,
  deltaY: number,
  scaleRatio: number
): CanvasPosition {
  if (!Number.isFinite(scaleRatio) || scaleRatio <= 0) return start;
  return {
    x: start.x + deltaX / scaleRatio,
    y: start.y + deltaY / scaleRatio,
  };
}

export function canvasTransformFromKeyboard(
  transform: CanvasTransform,
  key: CanvasTransformArrow,
  modifiers: { control: boolean; alt: boolean; shift: boolean }
): CanvasTransform | null {
  if (modifiers.control === modifiers.alt) return null;
  const direction = key === "ArrowRight" || key === "ArrowUp" ? 1 : -1;
  if (modifiers.control) {
    if (!Number.isFinite(transform.scale_x) || !Number.isFinite(transform.scale_y) || transform.scale_x <= 0 || transform.scale_y <= 0) return null;
    const requestedRatio = 1 + direction * (modifiers.shift ? 0.1 : 0.05);
    const minimumRatio = Math.max(0.05 / transform.scale_x, 0.05 / transform.scale_y);
    const maximumRatio = Math.min(20 / transform.scale_x, 20 / transform.scale_y);
    const ratio = Math.min(maximumRatio, Math.max(minimumRatio, requestedRatio));
    return {
      ...transform,
      scale_x: transform.scale_x * ratio,
      scale_y: transform.scale_y * ratio,
    };
  }
  return {
    ...transform,
    rotation: transform.rotation + direction * (modifiers.shift ? 10 : 1),
  };
}

export function canvasScaleFromPointerDelta(transform: CanvasTransform, deltaX: number): CanvasTransform {
  if (
    !Number.isFinite(deltaX) ||
    !Number.isFinite(transform.scale_x) ||
    !Number.isFinite(transform.scale_y) ||
    transform.scale_x <= 0 ||
    transform.scale_y <= 0
  ) return transform;
  const requestedRatio = Math.exp(deltaX / 180);
  const minimumRatio = Math.max(0.05 / transform.scale_x, 0.05 / transform.scale_y);
  const maximumRatio = Math.min(20 / transform.scale_x, 20 / transform.scale_y);
  const ratio = Math.min(maximumRatio, Math.max(minimumRatio, requestedRatio));
  return { ...transform, scale_x: transform.scale_x * ratio, scale_y: transform.scale_y * ratio };
}

export function canvasRotationFromPointerDelta(transform: CanvasTransform, deltaX: number): CanvasTransform {
  if (!Number.isFinite(deltaX)) return transform;
  return { ...transform, rotation: transform.rotation + deltaX * 0.5 };
}

export function canvasCropFromPointerDelta(
  crop: CanvasCrop,
  edge: CanvasCropEdge,
  deltaX: number,
  deltaY: number,
  width: number,
  height: number,
  rotationDegrees: number
): CanvasCrop {
  if (
    ![deltaX, deltaY, width, height, rotationDegrees, crop.top, crop.right, crop.bottom, crop.left].every(Number.isFinite) ||
    width <= 0 || height <= 0 ||
    crop.top < 0 || crop.right < 0 || crop.bottom < 0 || crop.left < 0 ||
    crop.top + crop.bottom >= 1 || crop.left + crop.right >= 1
  ) return crop;
  const radians = (rotationDegrees * Math.PI) / 180;
  const localX = deltaX * Math.cos(radians) + deltaY * Math.sin(radians);
  const localY = -deltaX * Math.sin(radians) + deltaY * Math.cos(radians);
  const next = { ...crop };
  const minimumVisibleFraction = 0.001;
  if (edge === "left" || edge === "right") {
    const proposed = edge === "left" ? crop.left + localX / width : crop.right - localX / width;
    const opposite = edge === "left" ? crop.right : crop.left;
    next[edge] = Math.min(Math.max(0, 1 - opposite - minimumVisibleFraction), Math.max(0, proposed));
  } else {
    const proposed = edge === "top" ? crop.top + localY / height : crop.bottom - localY / height;
    const opposite = edge === "top" ? crop.bottom : crop.top;
    next[edge] = Math.min(Math.max(0, 1 - opposite - minimumVisibleFraction), Math.max(0, proposed));
  }
  return next;
}

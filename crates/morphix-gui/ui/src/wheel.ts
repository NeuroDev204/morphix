// Wheel geometry for the Menu window. Pure math, no DOM, so vitest can run it in node.
// Values mirror the approved wireframe (menuA in morphix-wireframe.html, lines 648-656):
// a 320 px square, a ring between radius 58 and 150, and sector 0 centred at the top.

export interface Point {
  readonly x: number;
  readonly y: number;
}

export const WHEEL = { size: 320, outer: 150, inner: 58, center: 160 } as const;

// Sector 0 is centred on the top; sectors then run clockwise (SVG y grows downward,
// so increasing angle is clockwise on screen).
const TOP_ANGLE: number = -Math.PI / 2;

function sectorStep(count: number): number {
  return (2 * Math.PI) / count;
}

function pointOnCircle(angle: number, radius: number): Point {
  return {
    x: WHEEL.center + radius * Math.cos(angle),
    y: WHEEL.center + radius * Math.sin(angle),
  };
}

function formatPoint(point: Point): string {
  return `${point.x.toFixed(1)} ${point.y.toFixed(1)}`;
}

// WHY: one item owns the whole ring. A single sector would have coincident start and end
// points, which SVG drops, so the lone item would have no clickable area at all.
// The inner circle runs the opposite way so the hole stays unfilled under the default nonzero rule.
function fullRingPath(): string {
  const outerTop: Point = pointOnCircle(TOP_ANGLE, WHEEL.outer);
  const outerBottom: Point = pointOnCircle(TOP_ANGLE + Math.PI, WHEEL.outer);
  const innerTop: Point = pointOnCircle(TOP_ANGLE, WHEEL.inner);
  const innerBottom: Point = pointOnCircle(TOP_ANGLE + Math.PI, WHEEL.inner);
  return [
    `M ${formatPoint(outerTop)}`,
    `A ${WHEEL.outer} ${WHEEL.outer} 0 0 1 ${formatPoint(outerBottom)}`,
    `A ${WHEEL.outer} ${WHEEL.outer} 0 0 1 ${formatPoint(outerTop)}`,
    "Z",
    `M ${formatPoint(innerTop)}`,
    `A ${WHEEL.inner} ${WHEEL.inner} 0 0 0 ${formatPoint(innerBottom)}`,
    `A ${WHEEL.inner} ${WHEEL.inner} 0 0 0 ${formatPoint(innerTop)}`,
    "Z",
  ].join(" ");
}

export function sectorPath(index: number, count: number): string {
  if (count === 1) {
    return fullRingPath();
  }
  const step: number = sectorStep(count);
  // Each sector is centred on its label angle, so it starts half a step before it.
  const startAngle: number = TOP_ANGLE - step / 2 + index * step;
  const endAngle: number = startAngle + step;
  // A sector wider than half the circle needs the large-arc flag.
  const largeArc: 0 | 1 = step > Math.PI ? 1 : 0;
  const outerStart: Point = pointOnCircle(startAngle, WHEEL.outer);
  const outerEnd: Point = pointOnCircle(endAngle, WHEEL.outer);
  const innerEnd: Point = pointOnCircle(endAngle, WHEEL.inner);
  const innerStart: Point = pointOnCircle(startAngle, WHEEL.inner);
  return [
    `M ${formatPoint(outerStart)}`,
    `A ${WHEEL.outer} ${WHEEL.outer} 0 ${largeArc} 1 ${formatPoint(outerEnd)}`,
    `L ${formatPoint(innerEnd)}`,
    `A ${WHEEL.inner} ${WHEEL.inner} 0 ${largeArc} 0 ${formatPoint(innerStart)}`,
    "Z",
  ].join(" ");
}

export function labelPosition(index: number, count: number): Point {
  const angle: number = TOP_ANGLE + index * sectorStep(count);
  return pointOnCircle(angle, (WHEEL.outer + WHEEL.inner) / 2);
}

export function sectorIndexAt(x: number, y: number, count: number): number | null {
  // An empty wheel has no sector to hit. Without this guard the step is Infinity and
  // the result becomes -1, which would index sectors[-1] in the caller.
  if (count < 1) {
    return null;
  }
  const dx: number = x - WHEEL.center;
  const dy: number = y - WHEEL.center;
  const radius: number = Math.hypot(dx, dy);
  // The hole holds the file summary and the area outside is not part of the wheel.
  if (radius < WHEEL.inner || radius > WHEEL.outer) {
    return null;
  }
  const step: number = sectorStep(count);
  // Shift by half a step so sector boundaries fall between label angles, then wrap into [0, 2π).
  const shifted: number = (Math.atan2(dy, dx) - TOP_ANGLE + step / 2 + 2 * Math.PI) % (2 * Math.PI);
  return Math.min(Math.floor(shifted / step), count - 1);
}

// WHY: a sector is a narrow column, so a long label wraps onto several short lines.
// A single word longer than the limit stays whole rather than being cut mid-word.
export function wrapLabel(text: string, maxChars: number): string[] {
  const lines: string[] = [];
  let current: string = "";
  for (const word of text.split(" ")) {
    const candidate: string = current === "" ? word : `${current} ${word}`;
    if (current === "" || Array.from(candidate).length <= maxChars) {
      current = candidate;
    } else {
      lines.push(current);
      current = word;
    }
  }
  if (current !== "") {
    lines.push(current);
  }
  return lines;
}

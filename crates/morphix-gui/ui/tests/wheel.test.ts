import { describe, expect, it } from "vitest";
import { WHEEL, labelPosition, sectorIndexAt, sectorPath, wrapLabel, type Point } from "../src/wheel";

interface ParsedSector {
  readonly outerStart: Point;
  readonly outerEnd: Point;
  readonly innerEnd: Point;
  readonly innerStart: Point;
  readonly outerRadius: number;
  readonly innerRadius: number;
  readonly outerLargeArc: number;
  readonly outerSweep: number;
  readonly innerLargeArc: number;
  readonly innerSweep: number;
}

// Parses the fixed token layout that sectorPath emits:
// M x y A r r rot large sweep x y L x y A r r rot large sweep x y Z
function parseSector(path: string): ParsedSector {
  const t: string[] = path.split(" ");
  const point = (ix: number, iy: number): Point => ({ x: Number(t[ix]), y: Number(t[iy]) });
  return {
    outerStart: point(1, 2),
    outerRadius: Number(t[4]),
    outerLargeArc: Number(t[7]),
    outerSweep: Number(t[8]),
    outerEnd: point(9, 10),
    innerEnd: point(12, 13),
    innerRadius: Number(t[15]),
    innerLargeArc: Number(t[18]),
    innerSweep: Number(t[19]),
    innerStart: point(20, 21),
  };
}

// Expected corners come from the wireframe formula (menuA, lines 650-653), not from wheel.ts.
function wireframeAngles(index: number, count: number): { start: number; end: number } {
  const step: number = (2 * Math.PI) / count;
  const start: number = -Math.PI / 2 - step / 2 + index * step;
  return { start, end: start + step };
}

function onCircle(angle: number, radius: number): Point {
  return { x: 160 + radius * Math.cos(angle), y: 160 + radius * Math.sin(angle) };
}

function distanceFromCentre(p: Point): number {
  return Math.hypot(p.x - 160, p.y - 160);
}

// Path coordinates are rounded to one decimal, so allow 0.1 px of drift.
const COORD_TOLERANCE: number = 0.1;

function expectDistanceNear(p: Point, radius: number): void {
  expect(Math.abs(distanceFromCentre(p) - radius)).toBeLessThan(COORD_TOLERANCE);
}

function expectPointNear(actual: Point, expected: Point): void {
  expect(Math.abs(actual.x - expected.x)).toBeLessThan(COORD_TOLERANCE);
  expect(Math.abs(actual.y - expected.y)).toBeLessThan(COORD_TOLERANCE);
}

describe("wheel geometry", () => {
  it("first sector is centred at the top", () => {
    const c = labelPosition(0, 8);
    expect(c.x).toBeCloseTo(WHEEL.center, 1);
    expect(c.y).toBeLessThan(WHEEL.center);
  });
  it("each label point maps back to its own sector", () => {
    for (const count of [3, 7]) {
      for (let i = 0; i < count; i++) {
        const p = labelPosition(i, count);
        expect(sectorIndexAt(p.x, p.y, count)).toBe(i);
      }
    }
  });
  it("the hole in the middle is not a sector", () => {
    expect(sectorIndexAt(WHEEL.center, WHEEL.center, 8)).toBeNull();
  });
  it("points outside the ring are not a sector", () => {
    expect(sectorIndexAt(0, 0, 8)).toBeNull();
  });
  it("an empty wheel has no sector at any point", () => {
    expect(sectorIndexAt(WHEEL.center + WHEEL.outer - 1, WHEEL.center, 0)).toBeNull();
    expect(sectorIndexAt(WHEEL.center, WHEEL.center - 100, 0)).toBeNull();
  });
  it("sector path is a closed shape", () => {
    expect(sectorPath(3, 6)).toMatch(/Z$/);
  });
  it("a lone item is a full ring: its arcs run on the outer and inner circles", () => {
    const path: string = sectorPath(0, 1);
    // Decimal pairs are the path's points; radii and flags are integers, so they never match.
    const points: Point[] = [...path.matchAll(/(-?\d+\.\d) (-?\d+\.\d)/g)].map((m: RegExpMatchArray) => ({
      x: Number(m[1]),
      y: Number(m[2]),
    }));
    const radii: number[] = [...path.matchAll(/A (\d+) \d+ 0 \d \d/g)].map((m: RegExpMatchArray) => Number(m[1]));

    expect(radii).toEqual([WHEEL.outer, WHEEL.outer, WHEEL.inner, WHEEL.inner]);
    for (const p of points) {
      const onOuter: boolean = Math.abs(distanceFromCentre(p) - WHEEL.outer) < COORD_TOLERANCE;
      const onInner: boolean = Math.abs(distanceFromCentre(p) - WHEEL.inner) < COORD_TOLERANCE;
      expect(onOuter || onInner).toBe(true);
    }
    expect(points.some((p: Point) => Math.abs(distanceFromCentre(p) - WHEEL.outer) < COORD_TOLERANCE)).toBe(true);
    expect(points.some((p: Point) => Math.abs(distanceFromCentre(p) - WHEEL.inner) < COORD_TOLERANCE)).toBe(true);
  });
  it("a click on the lone item's ring selects it", () => {
    expect(sectorIndexAt(WHEEL.center, WHEEL.center - 100, 1)).toBe(0);
    expect(sectorIndexAt(WHEEL.center, WHEEL.center + 100, 1)).toBe(0);
  });
  it("a lone item's label sits at the top of the ring", () => {
    expectPointNear(labelPosition(0, 1), { x: WHEEL.center, y: WHEEL.center - 104 });
  });
  it.each([
    [2, 0, "M 10.0 160.0 A 150 150 0 0 1 310.0 160.0 L 218.0 160.0 A 58 58 0 0 0 102.0 160.0 Z"],
    [2, 1, "M 310.0 160.0 A 150 150 0 0 1 10.0 160.0 L 102.0 160.0 A 58 58 0 0 0 218.0 160.0 Z"],
    [3, 0, "M 30.1 85.0 A 150 150 0 0 1 289.9 85.0 L 210.2 131.0 A 58 58 0 0 0 109.8 131.0 Z"],
    [3, 1, "M 289.9 85.0 A 150 150 0 0 1 160.0 310.0 L 160.0 218.0 A 58 58 0 0 0 210.2 131.0 Z"],
    [3, 2, "M 160.0 310.0 A 150 150 0 0 1 30.1 85.0 L 109.8 131.0 A 58 58 0 0 0 160.0 218.0 Z"],
  ])("%i-sector path %i is byte-identical to the pre-ring output", (count: number, index: number, expected: string) => {
    expect(sectorPath(index, count)).toBe(expected);
  });
  it.each([2, 3, 5, 8])("sector path geometry matches the wireframe for %i sectors", (count: number) => {
    for (let i = 0; i < count; i++) {
      const sector: ParsedSector = parseSector(sectorPath(i, count));
      const angles = wireframeAngles(i, count);

      expect(sector.outerRadius).toBe(WHEEL.outer);
      expect(sector.innerRadius).toBe(WHEEL.inner);
      expectDistanceNear(sector.outerStart, WHEEL.outer);
      expectDistanceNear(sector.outerEnd, WHEEL.outer);
      expectDistanceNear(sector.innerEnd, WHEEL.inner);
      expectDistanceNear(sector.innerStart, WHEEL.inner);

      expectPointNear(sector.outerStart, onCircle(angles.start, WHEEL.outer));
      expectPointNear(sector.outerEnd, onCircle(angles.end, WHEEL.outer));
      expectPointNear(sector.innerEnd, onCircle(angles.end, WHEEL.inner));
      expectPointNear(sector.innerStart, onCircle(angles.start, WHEEL.inner));

      // Step > pi needs the large arc. Exactly pi (count 2) is a half circle and must not set it.
      const expectedLargeArc: number = 2 * Math.PI / count > Math.PI ? 1 : 0;
      expect(sector.outerLargeArc).toBe(expectedLargeArc);
      expect(sector.innerLargeArc).toBe(expectedLargeArc);
      // Outer arc runs clockwise (sweep 1), inner arc runs back anticlockwise (sweep 0).
      expect(sector.outerSweep).toBe(1);
      expect(sector.innerSweep).toBe(0);
    }
  });
  it.each([2, 3, 5, 8])("label sits at the ring midline for %i sectors", (count: number) => {
    for (let i = 0; i < count; i++) {
      const label: Point = labelPosition(i, count);
      expect(distanceFromCentre(label)).toBeCloseTo((WHEEL.outer + WHEEL.inner) / 2, 9);
      const angles = wireframeAngles(i, count);
      expectPointNear(label, onCircle((angles.start + angles.end) / 2, 104));
    }
  });
});

describe("wrapLabel", () => {
  it("wraps words so each line fits the column", () => {
    expect(wrapLabel("Gộp thành 1 PDF", 9)).toEqual(["Gộp thành", "1 PDF"]);
  });
  it("keeps a single long word on its own line instead of cutting it", () => {
    expect(wrapLabel("bao-cao-quy-III-phien-ban-cuoi-cung", 9)).toEqual([
      "bao-cao-quy-III-phien-ban-cuoi-cung",
    ]);
  });
  it("returns no lines for empty text", () => {
    expect(wrapLabel("", 9)).toEqual([]);
  });
});

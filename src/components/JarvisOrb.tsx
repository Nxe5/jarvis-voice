import { useMemo, useRef } from "react";
import { Canvas, useFrame } from "@react-three/fiber";
import * as THREE from "three";
import { useVoiceStore } from "../state/voiceStore";
import type { VoiceState } from "../types";

/* ------------------------------------------------------------------ *
 * Geometry helpers
 * ------------------------------------------------------------------ */

/** Distribute `n` points evenly over a unit sphere (Fibonacci lattice). */
function fibonacciSphere(n: number, radius: number): THREE.Vector3[] {
  const pts: THREE.Vector3[] = [];
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < n; i++) {
    const y = 1 - (i / (n - 1)) * 2;
    const r = Math.sqrt(1 - y * y);
    const theta = i * golden;
    pts.push(
      new THREE.Vector3(Math.cos(theta) * r, y, Math.sin(theta) * r).multiplyScalar(radius),
    );
  }
  return pts;
}

/** Connect every node to its `k` nearest neighbours → a clean network mesh. */
function nearestNeighbourEdges(pts: THREE.Vector3[], k: number): Float32Array {
  const seen = new Set<string>();
  const segs: number[] = [];
  for (let i = 0; i < pts.length; i++) {
    const dists = pts
      .map((p, j) => ({ j, d: pts[i].distanceToSquared(p) }))
      .filter((x) => x.j !== i)
      .sort((a, b) => a.d - b.d)
      .slice(0, k);
    for (const { j } of dists) {
      const key = i < j ? `${i}-${j}` : `${j}-${i}`;
      if (seen.has(key)) continue;
      seen.add(key);
      segs.push(pts[i].x, pts[i].y, pts[i].z, pts[j].x, pts[j].y, pts[j].z);
    }
  }
  return new Float32Array(segs);
}

/** A soft round glow sprite used as the point texture. */
function makeGlowTexture(): THREE.Texture {
  const size = 64;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d")!;
  const g = ctx.createRadialGradient(size / 2, size / 2, 0, size / 2, size / 2, size / 2);
  g.addColorStop(0, "rgba(255,255,255,1)");
  g.addColorStop(0.25, "rgba(255,255,255,0.85)");
  g.addColorStop(0.55, "rgba(255,255,255,0.25)");
  g.addColorStop(1, "rgba(255,255,255,0)");
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, size, size);
  const tex = new THREE.CanvasTexture(canvas);
  tex.needsUpdate = true;
  return tex;
}

/* ------------------------------------------------------------------ *
 * State → visual mapping (architecture §7.2)
 * ------------------------------------------------------------------ */

interface Style {
  color: THREE.Color;
  energyBase: number;
  spin: number;
  /** pulse kind so different states "read" distinctly */
  pulse: "breathe" | "flash" | "audio" | "think" | "ripple" | "alarm";
}

const STYLES: Record<VoiceState, Style> = {
  IDLE: { color: new THREE.Color("#31c7ff"), energyBase: 0.16, spin: 1.0, pulse: "breathe" },
  WAKE_DETECTED: { color: new THREE.Color("#eaffff"), energyBase: 1.0, spin: 1.7, pulse: "flash" },
  LISTENING: { color: new THREE.Color("#33d9ff"), energyBase: 0.34, spin: 1.25, pulse: "audio" },
  PROCESSING: { color: new THREE.Color("#8f7bff"), energyBase: 0.48, spin: 1.9, pulse: "think" },
  DISPATCHING: { color: new THREE.Color("#7466ff"), energyBase: 0.42, spin: 1.5, pulse: "think" },
  RESPONDING: { color: new THREE.Color("#57ffb0"), energyBase: 0.7, spin: 1.1, pulse: "ripple" },
  ERROR: { color: new THREE.Color("#ff5b5b"), energyBase: 0.55, spin: 0.7, pulse: "alarm" },
};

/* ------------------------------------------------------------------ *
 * Orbit definitions — nodes that travel in circular patterns
 * ------------------------------------------------------------------ */

interface OrbitDef {
  radius: number;
  count: number;
  tilt: [number, number, number];
  speed: number;
}

const ORBITS: OrbitDef[] = [
  { radius: 1.5, count: 16, tilt: [0.42, 0, 0.2], speed: 0.35 },
  { radius: 1.74, count: 20, tilt: [1.2, 0, -0.5], speed: -0.27 },
  { radius: 1.33, count: 12, tilt: [-0.6, 0, 0.95], speed: 0.46 },
  { radius: 1.95, count: 24, tilt: [0.95, 0, 1.4], speed: -0.2 },
];

/* ------------------------------------------------------------------ *
 * The rig — core globe + orbiting node rings
 * ------------------------------------------------------------------ */

function OrbRig() {
  const glow = useMemo(makeGlowTexture, []);

  // Core globe: nodes on a sphere + a nearest-neighbour network.
  const core = useMemo(() => {
    const pts = fibonacciSphere(110, 1.0);
    const nodes = new Float32Array(pts.length * 3);
    pts.forEach((p, i) => {
      nodes[i * 3] = p.x;
      nodes[i * 3 + 1] = p.y;
      nodes[i * 3 + 2] = p.z;
    });
    return { nodes, edges: nearestNeighbourEdges(pts, 3) };
  }, []);

  // Per-orbit geometry: the ring path, the traveling nodes, and spokes to centre.
  const orbits = useMemo(() => {
    return ORBITS.map((o) => {
      const nodes = new Float32Array(o.count * 3);
      const spokes = new Float32Array(o.count * 2 * 3);
      for (let i = 0; i < o.count; i++) {
        const a = (i / o.count) * Math.PI * 2;
        const x = Math.cos(a) * o.radius;
        const z = Math.sin(a) * o.radius;
        nodes[i * 3] = x;
        nodes[i * 3 + 1] = 0;
        nodes[i * 3 + 2] = z;
        // spoke: centre -> node
        spokes[i * 6 + 3] = x;
        spokes[i * 6 + 5] = z;
      }
      const ringSeg = 96;
      const ring = new Float32Array(ringSeg * 3);
      for (let i = 0; i < ringSeg; i++) {
        const a = (i / ringSeg) * Math.PI * 2;
        ring[i * 3] = Math.cos(a) * o.radius;
        ring[i * 3 + 2] = Math.sin(a) * o.radius;
      }
      return { def: o, nodes, spokes, ring };
    });
  }, []);

  // Shared materials (updated once per frame, reused across meshes).
  const nodeMat = useMemo(
    () =>
      new THREE.PointsMaterial({
        map: glow,
        size: 0.09,
        sizeAttenuation: true,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      }),
    [glow],
  );
  const edgeMat = useMemo(
    () =>
      new THREE.LineBasicMaterial({
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        opacity: 0.2,
      }),
    [],
  );
  const ringMat = useMemo(
    () =>
      new THREE.LineBasicMaterial({
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        opacity: 0.1,
      }),
    [],
  );
  const haloMat = useMemo(
    () =>
      new THREE.SpriteMaterial({
        map: glow,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        opacity: 0.4,
      }),
    [glow],
  );

  const rig = useRef<THREE.Group>(null);
  const coreGroup = useRef<THREE.Group>(null);
  const spinRefs = useRef<(THREE.Group | null)[]>([]);
  const halo = useRef<THREE.Sprite>(null);

  // Smoothed animation state (kept off React to avoid re-renders).
  const energy = useRef(0.16);
  const curColor = useRef(new THREE.Color("#31c7ff"));
  const dimColor = useRef(new THREE.Color("#31c7ff"));

  useFrame((_, rawDt) => {
    const dt = Math.min(rawDt, 0.05);
    const t = performance.now() / 1000;
    const { state, amplitude } = useVoiceStore.getState();
    const style = STYLES[state];

    // Target energy varies by pulse "personality".
    let target = style.energyBase;
    switch (style.pulse) {
      case "breathe":
        target += 0.06 * Math.sin(t * 1.1);
        break;
      case "audio":
        target += amplitude * 0.85;
        break;
      case "think":
        target += 0.22 * Math.sin(t * (style.pulse === "think" ? 5 : 3));
        break;
      case "ripple":
        target += 0.18 * Math.sin(t * 4);
        break;
      case "alarm":
        target += 0.3 * Math.abs(Math.sin(t * 9));
        break;
      case "flash":
        target = 1.0;
        break;
    }
    energy.current += (target - energy.current) * Math.min(1, dt * 8);
    const e = energy.current;

    // Colour lerp.
    curColor.current.lerp(style.color, Math.min(1, dt * 6));
    dimColor.current.copy(curColor.current).multiplyScalar(0.6);

    nodeMat.color.copy(curColor.current);
    nodeMat.size = 0.05 + e * 0.11;
    nodeMat.opacity = 0.7 + e * 0.3;

    edgeMat.color.copy(curColor.current);
    edgeMat.opacity = 0.1 + e * 0.35;

    ringMat.color.copy(dimColor.current);
    ringMat.opacity = 0.05 + e * 0.16;

    haloMat.color.copy(curColor.current);
    haloMat.opacity = 0.22 + e * 0.5;
    if (halo.current) {
      const s = 4.2 + e * 1.6;
      halo.current.scale.setScalar(s);
    }

    // Breathing scale of the whole rig.
    if (rig.current) {
      const s = 1 + e * 0.06;
      rig.current.scale.setScalar(s);
    }

    // Rotations — the network turns, the orbit nodes travel their circles.
    const spinMul = style.spin * (0.55 + e);
    if (coreGroup.current) {
      coreGroup.current.rotation.y += dt * 0.09 * spinMul;
      coreGroup.current.rotation.x += dt * 0.02 * spinMul;
    }
    spinRefs.current.forEach((g, i) => {
      if (g) g.rotation.y += dt * ORBITS[i].speed * spinMul;
    });
  });

  return (
    <group ref={rig}>
      <sprite ref={halo} material={haloMat} />

      {/* Core globe: connected nodes */}
      <group ref={coreGroup}>
        <lineSegments material={edgeMat}>
          <bufferGeometry>
            <bufferAttribute attach="attributes-position" args={[core.edges, 3]} />
          </bufferGeometry>
        </lineSegments>
        <points material={nodeMat}>
          <bufferGeometry>
            <bufferAttribute attach="attributes-position" args={[core.nodes, 3]} />
          </bufferGeometry>
        </points>
      </group>

      {/* Orbiting rings of connected, traveling nodes */}
      {orbits.map((orb, i) => (
        <group key={i} rotation={orb.def.tilt}>
          <group ref={(el) => (spinRefs.current[i] = el)}>
            <lineLoop material={ringMat}>
              <bufferGeometry>
                <bufferAttribute attach="attributes-position" args={[orb.ring, 3]} />
              </bufferGeometry>
            </lineLoop>
            <lineSegments material={edgeMat}>
              <bufferGeometry>
                <bufferAttribute attach="attributes-position" args={[orb.spokes, 3]} />
              </bufferGeometry>
            </lineSegments>
            <points material={nodeMat}>
              <bufferGeometry>
                <bufferAttribute attach="attributes-position" args={[orb.nodes, 3]} />
              </bufferGeometry>
            </points>
          </group>
        </group>
      ))}
    </group>
  );
}

/**
 * The Jarvis orb. A glowing globe of connected nodes with rings of nodes
 * orbiting it in circular patterns, all reacting to the voice-engine state and
 * live audio levels streamed from the Rust core.
 */
export function JarvisOrb() {
  return (
    <Canvas
      camera={{ position: [0, 0, 5.2], fov: 45 }}
      gl={{ antialias: true, alpha: true }}
      dpr={[1, 2]}
    >
      <OrbRig />
    </Canvas>
  );
}

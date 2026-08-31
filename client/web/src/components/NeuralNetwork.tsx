// NeuralNetwork.tsx — canvas interactivo de grafo tipo red neuronal con
// espacio infinito + floating toolbar.
//
// INTERACCIONES
//   - arrastrar un nodo → moverlo
//   - arrastrar un nodo ENCIMA de otro → conectarlos
//   - doble-click en espacio vacío → crear un nodo nuevo
//   - shift + click sobre un nodo → eliminarlo (y sus conexiones)
//   - arrastrar el FONDO vacío → pan del mundo infinito (con grid de
//     referencia para no perder la noción espacial)
//   - hover sobre un nodo → solo se ilumina su ego-network (1-hop)
//
// TOOLBAR FLOTANTE (bottom-center)
//   - + Add Node (IconPlus)
//   - Recenter (IconHome) → reset pan a 0,0
//   - Clear All (IconTrash) → wipe graph
//
// Pulses animados siguen viajando por cada conexión. Sin libs. Canvas 2D
// vanilla. Respeta tokens (--accent, --border, --text-primary,
// --surface-translucent). Estado vive solo en memoria: al cambiar de tab y
// volver, el grafo se reinicia.

import { useEffect, useRef, useState } from "react";
import {
  IconArrow,
  IconGrid,
  IconHand,
  IconHome,
  IconMenu,
  IconMinus,
  IconPin,
  IconPinFilled,
  IconPlus,
  IconTrash,
} from "../shared/components/Icons";

interface Node {
  id: number;
  x: number;
  y: number;
  r: number;
  name: string;
  role: AgentRole;
  /** Per-agent override; empty string means "use role default". */
  color: string;
}

type AgentRole = "assistant" | "coder" | "data" | "voice" | "shield";

const ROLE_COLORS: Record<AgentRole, string> = {
  assistant: "#efb05e",
  coder: "#5eb8ef",
  data: "#8aef5e",
  voice: "#ef5e9a",
  shield: "#c45eef",
};

const COLOR_SWATCHES = [
  "#efb05e",
  "#5eb8ef",
  "#8aef5e",
  "#ef5e9a",
  "#c45eef",
  "#f5f5f5",
  "#1a1a1a",
];

const AGENT_NAME_POOL = [
  "Atlas",
  "Nova",
  "Echo",
  "Cipher",
  "Iris",
  "Onyx",
  "Sage",
  "Vega",
  "Orion",
  "Lyra",
];

function pickAgentName(usedNames: Set<string>, fallbackId: number): string {
  for (const n of AGENT_NAME_POOL) {
    if (!usedNames.has(n)) return n;
  }
  return `Agent ${fallbackId}`;
}

function pickRole(id: number): AgentRole {
  const roles: AgentRole[] = ["assistant", "coder", "data", "voice", "shield"];
  return roles[id % roles.length] ?? "assistant";
}

function agentColor(n: Node): string {
  return n.color || ROLE_COLORS[n.role];
}

function snapToGrid(v: number): number {
  return Math.round(v / GRID_SIZE) * GRID_SIZE;
}

interface Pulse {
  t: number;
  speed: number;
}

interface Connection {
  a: number;
  b: number;
  pulses: Pulse[];
}

const GRID_SIZE = 32;

type Tool = "select" | "move" | "add";

export function NeuralNetwork() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [counts, setCounts] = useState({ nodes: 1, conns: 0 });
  const [tool, setTool] = useState<Tool>("move");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [pinned, setPinned] = useState(false);
  const [toolbarHovered, setToolbarHovered] = useState(false);
  const [gridVisible, setGridVisible] = useState(true);
  const expanded = pinned || toolbarHovered;
  const selectedRef = useRef<number | null>(null);
  const gridVisibleRef = useRef(true);
  selectedRef.current = selectedId;
  gridVisibleRef.current = gridVisible;

  // Mutables refs (canvas state). Updated in-place by handlers / RAF loop.
  const nodesRef = useRef<Node[]>([]);
  const connectionsRef = useRef<Connection[]>([]);
  const nextIdRef = useRef(0);
  const panRef = useRef({ x: 0, y: 0 });
  const zoomRef = useRef(1);
  const panStartRef = useRef<{ mx: number; my: number; px: number; py: number } | null>(null);
  const dragSourceRef = useRef<number | null>(null);
  const dragOffsetXRef = useRef(0);
  const dragOffsetYRef = useRef(0);
  const hoveredRef = useRef<number | null>(null);
  const toolRef = useRef<Tool>("select");
  const downRef = useRef<{
    mx: number;
    my: number;
    target: Node | null;
    moved: boolean;
  } | null>(null);
  // Keep toolRef in sync with React state for the RAF/handlers.
  toolRef.current = tool;

  // Reads world coords (mouse viewport pos - current pan). Mutates graph.
  function hitTest(mx: number, my: number): Node | null {
    const pan = panRef.current;
    const wx = mx - pan.x;
    const wy = my - pan.y;
    const nodes = nodesRef.current;
    for (let i = nodes.length - 1; i >= 0; i--) {
      const n = nodes[i];
      if (!n) continue;
      const dx = n.x - wx;
      const dy = n.y - wy;
      const d = Math.sqrt(dx * dx + dy * dy);
      if (d < n.r + 10) return n;
    }
    return null;
  }

function usedNames(): Set<string> {
  return new Set(nodesRef.current.map((n) => n.name));
}

function setAgentColor(id: number, color: string) {
  const idx = nodesRef.current.findIndex((n) => n.id === id);
  if (idx < 0) return;
  const existing = nodesRef.current[idx];
  if (!existing) return;
  nodesRef.current[idx] = { ...existing, color };
}

function addNodeAt(wx: number, wy: number) {
    const newId = nextIdRef.current++;
    nodesRef.current.push({
      id: newId,
      x: snapToGrid(wx),
      y: snapToGrid(wy),
      r: 12,
      name: pickAgentName(usedNames(), newId),
      role: pickRole(newId),
      color: "",
    });
    setCounts({
      nodes: nodesRef.current.length,
      conns: connectionsRef.current.length,
    });
  }

  function recenter() {
    panRef.current = { x: 0, y: 0 };
    zoomRef.current = 1;
    setSelectedId(null);
  }

  function clearAll() {
    nodesRef.current = [];
    connectionsRef.current = [];
    nextIdRef.current = 0;
    panRef.current = { x: 0, y: 0 };
    zoomRef.current = 1;
    hoveredRef.current = null;
    dragSourceRef.current = null;
    panStartRef.current = null;
    setSelectedId(null);
    setCounts({ nodes: 0, conns: 0 });
  }

  function setZoomAt(newZoom: number, mx: number, my: number) {
    const clamped = Math.max(0.2, Math.min(5, newZoom));
    const ratio = clamped / zoomRef.current;
    // Keep the world point under the cursor fixed.
    panRef.current = {
      x: mx - (mx - panRef.current.x) * ratio,
      y: my - (my - panRef.current.y) * ratio,
    };
    zoomRef.current = clamped;
  }

  function zoomIn() {
    const c = canvasRef.current;
    if (!c) return;
    const rect = c.getBoundingClientRect();
    setZoomAt(zoomRef.current * 1.2, rect.width / 2, rect.height / 2);
  }

  function zoomOut() {
    const c = canvasRef.current;
    if (!c) return;
    const rect = c.getBoundingClientRect();
    setZoomAt(zoomRef.current / 1.2, rect.width / 2, rect.height / 2);
  }

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // No seed: canvas starts empty. The crosshair drawn in draw() is the only
    // hint at "this is the world origin".

    const dpr = window.devicePixelRatio || 1;

    function readVar(name: string, fallback: string): string {
      return (
        getComputedStyle(document.documentElement).getPropertyValue(name).trim() ||
        fallback
      );
    }
    let accent = readVar("--accent", "#efb05e");
    let border = readVar("--border", "rgba(255, 255, 255, 0.08)");
    let textPrimary = readVar("--text-primary", "#fff");

    function resize() {
      const parent = canvas!.parentElement;
      if (!parent) return;
      const rect = parent.getBoundingClientRect();
      const w = rect.width;
      const h = rect.height;
      if (w === 0 || h === 0) return;
      canvas!.width = w * dpr;
      canvas!.height = h * dpr;
      canvas!.style.width = `${w}px`;
      canvas!.style.height = `${h}px`;
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);

      accent = readVar("--accent", "#efb05e");
      border = readVar("--border", "rgba(255, 255, 255, 0.08)");
      textPrimary = readVar("--text-primary", "#fff");
    }

    // Defensive cleanup: drop any refs pointing at deleted-agent ids so we
    // never render a ghost halo for a node that no longer exists.
    function pruneOrphanRefs() {
      const ids = new Set(nodesRef.current.map((n) => n.id));
      if (hoveredRef.current !== null && !ids.has(hoveredRef.current)) {
        hoveredRef.current = null;
      }
      if (dragSourceRef.current !== null && !ids.has(dragSourceRef.current)) {
        dragSourceRef.current = null;
      }
      // selectedId is React state and will be cleared via useEffect-friendly
      // paths from the input handlers; nothing to do here.
    }

    resize();

    function onMouseMove(e: MouseEvent) {
      const rect = canvas!.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      if (downRef.current !== null && !downRef.current.moved) {
        const dx = mx - downRef.current.mx;
        const dy = my - downRef.current.my;
        if (Math.hypot(dx, dy) > 4) {
          downRef.current.moved = true;
        }
      }

      if (panStartRef.current !== null) {
        const ps = panStartRef.current;
        panRef.current.x = ps.px + (mx - ps.mx);
        panRef.current.y = ps.py + (my - ps.my);
        return;
      }

      const drag = dragSourceRef.current;
      if (drag !== null) {
        const src = nodesRef.current.find((n) => n.id === drag);
        if (src) {
          const pan = panRef.current;
          src.x = snapToGrid(mx - pan.x + dragOffsetXRef.current);
          src.y = snapToGrid(my - pan.y + dragOffsetYRef.current);
          hoveredRef.current = hitTest(mx, my)?.id ?? null;
        }
      } else {
        hoveredRef.current = hitTest(mx, my)?.id ?? null;
      }

      const cur = toolRef.current;
      let cursor: string;
      if (drag !== null) cursor = "grabbing";
      else if (panStartRef.current !== null) cursor = "grabbing";
      else if (hoveredRef.current !== null)
        cursor = cur === "select" ? "default" : "grab";
      else cursor = cur === "add" ? "crosshair" : "default";
      canvas!.style.cursor = cursor;
    }

    function tryConnect(aId: number, bId: number) {
      if (aId === bId) return;
      const a = nodesRef.current.find((n) => n.id === aId);
      const b = nodesRef.current.find((n) => n.id === bId);
      if (!a || !b) return;
      const exists = connectionsRef.current.some(
        (c) =>
          (c.a === a.id && c.b === b.id) ||
          (c.a === b.id && c.b === a.id),
      );
      if (exists) return;
      const numPulses = Math.random() < 0.7 ? 1 : 2;
      const pulses: Pulse[] = [];
      for (let i = 0; i < numPulses; i++) {
        pulses.push({ t: Math.random(), speed: 0.18 + Math.random() * 0.32 });
      }
      connectionsRef.current.push({ a: a.id, b: b.id, pulses });
      setCounts({
        nodes: nodesRef.current.length,
        conns: connectionsRef.current.length,
      });
    }

    function onMouseDown(e: MouseEvent) {
      const rect = canvas!.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;

      if (e.shiftKey) {
        const target = hitTest(mx, my);
        if (target) {
          const idx = nodesRef.current.findIndex((n) => n.id === target.id);
          if (idx >= 0) nodesRef.current.splice(idx, 1);
          for (let i = connectionsRef.current.length - 1; i >= 0; i--) {
            const c = connectionsRef.current[i];
            if (!c) continue;
            if (c.a === target.id || c.b === target.id) {
              connectionsRef.current.splice(i, 1);
            }
          }
          if (selectedRef.current === target.id) {
            setSelectedId(null);
          }
          setCounts({
            nodes: nodesRef.current.length,
            conns: connectionsRef.current.length,
          });
        }
        return;
      }

      const target = hitTest(mx, my);
      const cur = toolRef.current;
      downRef.current = { mx, my, target, moved: false };

      if (cur === "select") {
        // Select tool: clicks select, drag background pans. NEVER drags nodes.
        if (target) {
          const sel = selectedRef.current;
          if (sel !== null && sel !== target.id) {
            // Connect prior selection to this target, then deselect.
            tryConnect(sel, target.id);
            setSelectedId(null);
          } else if (sel === target.id) {
            // Toggle off.
            setSelectedId(null);
          } else {
            setSelectedId(target.id);
          }
          // No drag-source — we don't move selected nodes; we only select.
        } else {
          // Click empty space: deselect, and prepare to pan on drag.
          setSelectedId(null);
          panStartRef.current = {
            mx,
            my,
            px: panRef.current.x,
            py: panRef.current.y,
          };
          canvas!.style.cursor = "grabbing";
        }
        return;
      }

      if (cur === "move") {
        // Move tool: drag node = move; drag bg = pan; click is no-op (no node pos change).
        if (target) {
          const pan = panRef.current;
          dragSourceRef.current = target.id;
          dragOffsetXRef.current = target.x - (mx - pan.x);
          dragOffsetYRef.current = target.y - (my - pan.y);
        } else {
          panStartRef.current = {
            mx,
            my,
            px: panRef.current.x,
            py: panRef.current.y,
          };
          canvas!.style.cursor = "grabbing";
        }
        return;
      }

      if (cur === "add") {
        // Add tool: click adds, drag bg = pan.
        if (!target) {
          panStartRef.current = {
            mx,
            my,
            px: panRef.current.x,
            py: panRef.current.y,
          };
          canvas!.style.cursor = "grabbing";
        }
        // If user clicked on a node, we don't do anything — let mouseUp
        // handle it via the click path (which won't add in this case).
      }
    }

    function onMouseUp(_e: MouseEvent) {
      const drag = dragSourceRef.current;
      const down = downRef.current;
      const cur = toolRef.current;

      if (cur === "move" && drag !== null && down?.moved) {
        // Drag completed on move tool — connect if dropped on another node.
        const dropTarget = nodesRef.current.find(
          (n) => n.id === hoveredRef.current,
        );
        const src = nodesRef.current.find((n) => n.id === drag);
        if (dropTarget && src && dropTarget.id !== src.id) {
          tryConnect(src.id, dropTarget.id);
        }
      } else if (
        cur === "add" &&
        down &&
        !down.moved &&
        !down.target
      ) {
        addNodeAt(down.mx - panRef.current.x, down.my - panRef.current.y);
      }
      // select tool handled in mousedown — nothing to do on mouseup

      dragSourceRef.current = null;
      panStartRef.current = null;
      downRef.current = null;
    }

    function onDoubleClick(e: MouseEvent) {
      const rect = canvas!.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      if (!hitTest(mx, my)) {
        addNodeAt(mx - panRef.current.x, my - panRef.current.y);
      }
    }

    function onWheel(e: WheelEvent) {
      e.preventDefault();
      const rect = canvas!.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const factor = Math.exp(-e.deltaY * 0.0015);
      setZoomAt(zoomRef.current * factor, mx, my);
    }

    canvas.addEventListener("mousemove", onMouseMove);
    canvas.addEventListener("mousedown", onMouseDown);
    canvas.addEventListener("mouseup", onMouseUp);
    canvas.addEventListener("mouseleave", () => {
      dragSourceRef.current = null;
      panStartRef.current = null;
      downRef.current = null;
    });
    canvas.addEventListener("dblclick", onDoubleClick);
    canvas.addEventListener("wheel", onWheel, { passive: false });

    let lastTs = performance.now();
    let raf = 0;
    function draw(ts: number) {
      const dt = Math.min(0.05, (ts - lastTs) / 1000);
      lastTs = ts;
      const w = canvas!.width / dpr;
      const h = canvas!.height / dpr;
      ctx!.clearRect(0, 0, w, h);

      pruneOrphanRefs();

      const pan = panRef.current;
      const zoom = zoomRef.current;
      ctx!.save();
      ctx!.translate(pan.x, pan.y);
      ctx!.scale(zoom, zoom);

      const nodes = nodesRef.current;
      const connections = connectionsRef.current;

      // 1-hop ego-network: hovered + its direct neighbors.
      const ego = new Set<number>();
      const hovered = hoveredRef.current;
      if (hovered !== null) {
        ego.add(hovered);
        for (const c of connections) {
          if (c.a === hovered) ego.add(c.b);
          if (c.b === hovered) ego.add(c.a);
        }
      }

      // Reference grid in world coords (gives a sense of motion when panning).
      if (gridVisibleRef.current) {
        ctx!.strokeStyle = border;
        ctx!.globalAlpha = 1;
        ctx!.lineWidth = 1;
        const startWX = -pan.x / zoom;
        const startWY = -pan.y / zoom;
        const endWX = w / zoom - pan.x / zoom;
        const endWY = h / zoom - pan.y / zoom;
        const firstX = Math.floor(startWX / GRID_SIZE) * GRID_SIZE;
        const firstY = Math.floor(startWY / GRID_SIZE) * GRID_SIZE;
        ctx!.beginPath();
        for (let x = firstX; x <= endWX; x += GRID_SIZE) {
          ctx!.moveTo(x, startWY);
          ctx!.lineTo(x, endWY);
        }
        for (let y = firstY; y <= endWY; y += GRID_SIZE) {
          ctx!.moveTo(startWX, y);
          ctx!.lineTo(endWX, y);
        }
        ctx!.stroke();
        ctx!.globalAlpha = 1;
      }

      // Connections + pulses
      for (const conn of connections) {
        const a = nodes.find((n) => n.id === conn.a);
        const b = nodes.find((n) => n.id === conn.b);
        if (!a || !b) continue;
        const inEgo = ego.has(conn.a) && ego.has(conn.b);
        const isDimmed = hovered !== null && !inEgo;
        ctx!.strokeStyle = inEgo ? accent : border;
        ctx!.globalAlpha = isDimmed ? 0.08 : inEgo ? 0.85 : 0.35;
        ctx!.lineWidth = inEgo ? 1.4 : 0.7;
        ctx!.beginPath();
        ctx!.moveTo(a.x, a.y);
        ctx!.lineTo(b.x, b.y);
        ctx!.stroke();

        if (!isDimmed) {
          ctx!.globalAlpha = 1;
          for (const p of conn.pulses) {
            p.t += p.speed * dt;
            if (p.t >= 1) {
              p.t -= 1;
              p.speed = 0.18 + Math.random() * 0.32;
            }
            const px = a.x + (b.x - a.x) * p.t;
            const py = a.y + (b.y - a.y) * p.t;
            ctx!.shadowColor = accent;
            ctx!.shadowBlur = 14;
            ctx!.fillStyle = accent;
            ctx!.beginPath();
            ctx!.arc(px, py, 1.6, 0, Math.PI * 2);
            ctx!.fill();
          }
        }
      }
      ctx!.shadowBlur = 0;

      // Preview connection while dragging onto a target
      const drag = dragSourceRef.current;
      if (
        drag !== null &&
        hovered !== null &&
        hovered !== drag
      ) {
        const src = nodes.find((n) => n.id === drag);
        const tgt = nodes.find((n) => n.id === hovered);
        if (src && tgt) {
          ctx!.setLineDash([4, 4]);
          ctx!.strokeStyle = accent;
          ctx!.globalAlpha = 0.6;
          ctx!.lineWidth = 1.2;
          ctx!.beginPath();
          ctx!.moveTo(src.x, src.y);
          ctx!.lineTo(tgt.x, tgt.y);
          ctx!.stroke();
          ctx!.setLineDash([]);
          ctx!.globalAlpha = 1;
        }
      }

      // Agents — each drawn at its fixed r. The dot is a single filled circle
      // in the agent's color. Hover/select change glow intensity and stroke,
      // not size. Labels fade with the dimming rule and use textPrimary.
      for (const n of nodes) {
        const isH = hovered === n.id;
        const isDrag = drag === n.id;
        const isSel = selectedRef.current === n.id;
        const inEgo = ego.has(n.id);
        const isDimmed = hovered !== null && !inEgo;
        const c = agentColor(n);

        ctx!.globalAlpha = isDimmed ? 0.18 : 1;
        ctx!.shadowColor = c;
        ctx!.shadowBlur = isSel ? 26 : isH || isDrag ? 22 : inEgo ? 14 : 8;

        // Body fill: agent's own color.
        ctx!.fillStyle = c;
        ctx!.beginPath();
        ctx!.arc(n.x, n.y, n.r, 0, Math.PI * 2);
        ctx!.fill();

        // Solid outline ring on hover/select/ego (no size change).
        const outlineR = n.r * 1.45;
        if (isSel) {
          ctx!.shadowBlur = 0;
          ctx!.globalAlpha = isDimmed ? 0.18 : 1;
          ctx!.strokeStyle = c;
          ctx!.lineWidth = 1.6;
          ctx!.beginPath();
          ctx!.arc(n.x, n.y, outlineR, 0, Math.PI * 2);
          ctx!.stroke();
        } else if (isH || isDrag || inEgo) {
          ctx!.shadowBlur = 0;
          ctx!.globalAlpha = isDimmed ? 0.18 : 0.55;
          ctx!.strokeStyle = c;
          ctx!.lineWidth = 1.2;
          ctx!.beginPath();
          ctx!.arc(n.x, n.y, outlineR, 0, Math.PI * 2);
          ctx!.stroke();
        }

        // Name label below — single tone, brighter on hover/ego/select.
        ctx!.shadowBlur = 0;
        ctx!.font = "10px ui-monospace, SFMono-Regular, Menlo, monospace";
        ctx!.textAlign = "center";
        ctx!.textBaseline = "top";
        ctx!.fillStyle = isSel
          ? c
          : inEgo || isH
            ? textPrimary
            : textPrimary;
        ctx!.globalAlpha = isDimmed ? 0.22 : isSel || inEgo || isH ? 1 : 0.8;
        ctx!.fillText(n.name, n.x, n.y + n.r + 6);
      }
      ctx!.shadowBlur = 0;
      ctx!.globalAlpha = 1;

      ctx!.restore();

      raf = requestAnimationFrame(draw);
    }
    raf = requestAnimationFrame(draw);

    window.addEventListener("resize", resize);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("mousemove", onMouseMove);
      canvas.removeEventListener("mousedown", onMouseDown);
      canvas.removeEventListener("mouseup", onMouseUp);
      canvas.removeEventListener("mouseleave", () => {
        dragSourceRef.current = null;
        panStartRef.current = null;
      });
      canvas.removeEventListener("dblclick", onDoubleClick);
      canvas.removeEventListener("wheel", onWheel);
    };
  }, []);

  return (
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <canvas
        ref={canvasRef}
        data-testid="neural-network-canvas"
        style={{ width: "100%", height: "100%", display: "block" }}
      />

      {/* Compact counter (top-left) — no descriptions, just node count */}
      <div
        data-testid="neural-network-hud"
        style={{
          position: "absolute",
          top: 12,
          left: 12,
          color: "var(--text-muted)",
          fontFamily: "monospace",
          fontSize: 12,
          opacity: 0.7,
          pointerEvents: "none",
          userSelect: "none",
        }}
      >
        {counts.nodes} agents · {counts.conns} conns
      </div>

      {/* Tool panel (bottom-right). Layout uses `grid-template-columns` so we
          can animate the container width between a single-hamburger column
          and the full tools row without leaving trailing whitespace. */}
      <div
        data-testid="neural-network-toolbar"
        onMouseEnter={() => setToolbarHovered(true)}
        onMouseLeave={() => setToolbarHovered(false)}
        style={{
          position: "absolute",
          right: 12,
          bottom: 24,
          display: "grid",
          gridTemplateColumns: expanded
            ? "32px minmax(0, auto)"
            : "32px minmax(0, 0px)",
          alignItems: "center",
          background: "var(--surface-translucent)",
          backdropFilter: "var(--chrome-blur)",
          border: "1px solid var(--border)",
          boxShadow: "0 8px 24px rgba(0, 0, 0, 0.3)",
          borderRadius: 18,
          padding: 6,
          gap: 4,
          height: 44,
          overflow: "hidden",
          transition:
            "grid-template-columns 0.22s cubic-bezier(0.4, 0, 0.2, 1)",
        }}
      >
        {/* Column 1: hamburger. Always in DOM; visible only when
            collapsed. */}
        <button
          type="button"
          onClick={() => setPinned(true)}
          aria-label="Open tools"
          title="Open tools"
          data-testid="nn-tool-menu"
          style={{
            width: 32,
            height: 32,
            display: "grid",
            placeItems: "center",
            border: "none",
            background: "transparent",
            color: "var(--text-primary)",
            borderRadius: 12,
            cursor: "pointer",
            padding: 0,
            opacity: expanded ? 0 : 1,
            transform: expanded
              ? "translateX(-16px) scale(0.55)"
              : "translateX(0) scale(1)",
            pointerEvents: expanded ? "none" : "auto",
            transition:
              "opacity 0.22s cubic-bezier(0.4, 0, 0.2, 1), transform 0.22s cubic-bezier(0.4, 0, 0.2, 1)",
          }}
        >
          <IconMenu />
        </button>

        {/* Column 2: tools. Width animates from 0 to natural when expanded. */}
        <div
          data-testid="nn-tool-group"
          style={{
            display: "inline-flex",
            flexDirection: "row",
            alignItems: "center",
            gap: 4,
            minWidth: 0,
            opacity: expanded ? 1 : 0,
            transform: expanded
              ? "translateX(0) scale(1)"
              : "translateX(-16px) scale(0.85)",
            transformOrigin: "left center",
            pointerEvents: expanded ? "auto" : "none",
            transition:
              "opacity 0.22s cubic-bezier(0.4, 0, 0.2, 1), transform 0.22s cubic-bezier(0.4, 0, 0.2, 1)",
          }}
        >
          {/* Pin toggle — leftmost item in the expanded group. */}
          <ToolButton
            label={pinned ? "Unpin (auto-collapse on leave)" : "Pin open"}
            active={pinned}
            onClick={() => setPinned((p) => !p)}
            testId="nn-tool-pin"
          >
            {pinned ? <IconPinFilled /> : <IconPin />}
          </ToolButton>
          <ToolButton
            label="Select (click to select & connect)"
            active={tool === "select"}
            onClick={() => setTool("select")}
            testId="nn-tool-select"
          >
            <IconArrow />
          </ToolButton>
          <ToolButton
            label="Move (drag nodes / pan canvas)"
            active={tool === "move"}
            onClick={() => setTool("move")}
            testId="nn-tool-move"
          >
            <IconHand />
          </ToolButton>
          <ToolButton
            label="Add (click to place a node)"
            active={tool === "add"}
            onClick={() => setTool("add")}
            testId="nn-tool-add"
          >
            <IconPlus />
          </ToolButton>
          <ToolButton
            label="Recenter"
            onClick={recenter}
            testId="nn-tool-recenter"
          >
            <IconHome />
          </ToolButton>
          <ToolButton
            label={gridVisible ? "Hide grid" : "Show grid"}
            active={gridVisible}
            onClick={() => setGridVisible((v) => !v)}
            testId="nn-tool-grid"
          >
            <IconGrid />
          </ToolButton>
          <ToolButton
            label="Zoom out"
            onClick={zoomOut}
            testId="nn-tool-zoom-out"
          >
            <IconMinus />
          </ToolButton>
          <ToolButton
            label="Zoom in"
            onClick={zoomIn}
            testId="nn-tool-zoom-in"
          >
            <IconPlus />
          </ToolButton>
          <ToolButton
            label="Clear all"
            onClick={clearAll}
            testId="nn-tool-clear"
          >
            <IconTrash />
          </ToolButton>
        </div>
      </div>

      {/* Color swatches (bottom-center) — only when an agent is selected. */}
      {selectedId !== null && (
        <ColorSwatches
          selectedId={selectedId}
          agents={nodesRef.current}
          onPick={(color) => setAgentColor(selectedId, color)}
          onClose={() => setSelectedId(null)}
        />
      )}
    </div>
  );
}

function ColorSwatches({
  selectedId,
  agents,
  onPick,
  onClose,
}: {
  selectedId: number;
  agents: Node[];
  onPick: (color: string) => void;
  onClose: () => void;
}) {
  const sel = agents.find((a) => a.id === selectedId);
  if (!sel) return null;
  const currentColor = agentColor(sel);
  return (
    <div
      data-testid="nn-color-swatches"
      style={{
        position: "absolute",
        bottom: 24,
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "8px 12px",
        borderRadius: 16,
        background: "var(--surface-translucent)",
        backdropFilter: "var(--chrome-blur)",
        border: "1px solid var(--border)",
        boxShadow: "0 8px 24px rgba(0, 0, 0, 0.3)",
        color: "var(--text-primary)",
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: 12,
      }}
    >
      <span style={{ opacity: 0.7 }}>{sel.name}</span>
      <span style={{ width: 1, height: 16, background: "var(--border)" }} />
      {COLOR_SWATCHES.map((c) => (
        <button
          key={c}
          type="button"
          onClick={() => onPick(c)}
          aria-label={`Set color ${c}`}
          data-testid={`nn-swatch-${c}`}
          style={{
            width: 18,
            height: 18,
            borderRadius: 9,
            border: c === currentColor ? "2px solid #fff" : "1px solid var(--border)",
            background: c,
            cursor: "pointer",
            padding: 0,
          }}
        />
      ))}
      <button
        type="button"
        onClick={() => onPick("")}
        aria-label="Reset to role color"
        data-testid="nn-swatch-reset"
        title="Reset to role color"
        style={{
          width: 18,
          height: 18,
          borderRadius: 9,
          border: "1px solid var(--border)",
          background: "transparent",
          cursor: "pointer",
          padding: 0,
          color: "var(--text-muted)",
          fontSize: 11,
          lineHeight: 1,
        }}
      >
        ×
      </button>
      <span style={{ width: 1, height: 16, background: "var(--border)" }} />
      <button
        type="button"
        onClick={onClose}
        aria-label="Close"
        data-testid="nn-swatches-close"
        style={{
          appearance: "none",
          background: "transparent",
          border: "none",
          color: "var(--text-muted)",
          cursor: "pointer",
          fontSize: 14,
          lineHeight: 1,
          padding: 0,
        }}
      >
        ✕
      </button>
    </div>
  );
}

function ToolButton({
  children,
  onClick,
  label,
  testId,
  active,
}: {
  children: React.ReactNode;
  onClick: () => void;
  label: string;
  testId: string;
  active?: boolean;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      aria-label={label}
      aria-pressed={active ? "true" : undefined}
      data-testid={testId}
      data-active={active ? "true" : undefined}
      style={{
        width: 32,
        height: 32,
        display: "grid",
        placeItems: "center",
        border: "none",
        background: active
          ? "var(--accent-soft)"
          : hover
            ? "var(--bg-glass)"
            : "transparent",
        color: active ? "var(--accent)" : "var(--text-primary)",
        borderRadius: 18,
        cursor: "pointer",
        transition: "background 0.15s ease, color 0.15s ease",
      }}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      <span style={{ display: "grid", placeItems: "center" }}>{children}</span>
    </button>
  );
}

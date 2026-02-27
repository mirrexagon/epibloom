import { useMemo, useRef, useState, useCallback, useEffect } from "react";
import { ChatMessage } from "../types/api";

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const NODE_W = 160;
const NODE_H = 54;
/** Horizontal space (in pixels) allocated per unit of subtree width. */
const COL_UNIT = 180;
/** Vertical distance between depth levels. */
const ROW_HEIGHT = 90;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Returns the set of node IDs on the path from leafId to the root. */
function getPathIds(
    messages: Record<string, ChatMessage>,
    leafId: string | null,
): Set<string> {
    const ids = new Set<string>();
    let currentId: string | null = leafId;
    while (currentId !== null) {
        const node = messages[currentId];
        if (!node) break;
        ids.add(currentId);
        currentId = node.parent_id ?? null;
    }
    return ids;
}

// ---------------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------------

interface LayoutResult {
    positions: Record<string, { x: number; y: number }>;
    childrenMap: Record<string, string[]>;
    rootIds: string[];
}

function computeLayout(messages: Record<string, ChatMessage>): LayoutResult {
    // Build children map, treating null parent as "__root__".
    const childrenMap: Record<string, string[]> = {};
    for (const id of Object.keys(messages)) {
        const node = messages[id];
        const parentKey = node.parent_id ?? "__root__";
        if (!childrenMap[parentKey]) childrenMap[parentKey] = [];
        childrenMap[parentKey].push(id);
    }
    // Sort by UUID for stable UUIDv7 insertion order.
    for (const key of Object.keys(childrenMap)) {
        childrenMap[key].sort((a, b) => a.localeCompare(b));
    }

    const rootIds = (childrenMap["__root__"] ?? []).slice();

    // --- Bottom-up pass: compute subtree width in units ---
    const subtreeWidth: Record<string, number> = {};

    function computeWidth(id: string): number {
        const children = childrenMap[id] ?? [];
        if (children.length === 0) {
            subtreeWidth[id] = 1;
            return 1;
        }
        let total = 0;
        for (const child of children) {
            total += computeWidth(child);
        }
        subtreeWidth[id] = Math.max(total, 1);
        return subtreeWidth[id];
    }
    for (const rootId of rootIds) {
        computeWidth(rootId);
    }

    // --- Top-down pass: assign pixel positions ---
    const positions: Record<string, { x: number; y: number }> = {};

    function assignPositions(id: string, leftEdge: number, depth: number) {
        const w = subtreeWidth[id];
        // Centre the node over its allocated column span.
        const centreUnit = leftEdge + w / 2;
        positions[id] = {
            x: centreUnit * COL_UNIT - NODE_W / 2,
            y: depth * ROW_HEIGHT,
        };
        const children = childrenMap[id] ?? [];
        let childLeft = leftEdge;
        for (const child of children) {
            assignPositions(child, childLeft, depth + 1);
            childLeft += subtreeWidth[child];
        }
    }

    let offset = 0;
    for (const rootId of rootIds) {
        assignPositions(rootId, offset, 0);
        offset += subtreeWidth[rootId];
    }

    return { positions, childrenMap, rootIds };
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export interface ChatFlowchartProps {
    messages: Record<string, ChatMessage>;
    selectedLeafId: string | null;
    onSelectNode: (nodeId: string) => void;
}

export function ChatFlowchart({
    messages,
    selectedLeafId,
    onSelectNode,
}: ChatFlowchartProps) {
    const [pan, setPan] = useState({ x: 16, y: 16 });
    const [scale, setScale] = useState(1);

    const isPanning = useRef(false);
    const lastMouse = useRef({ x: 0, y: 0 });
    // Track whether the mouse actually moved during a press (to distinguish
    // panning from a click).
    const didMove = useRef(false);

    const svgRef = useRef<SVGSVGElement>(null);

    // Recompute layout whenever messages change.
    const { positions, childrenMap } = useMemo(
        () => computeLayout(messages),
        [messages],
    );

    // Set of node IDs on the current path (for highlight ring).
    const pathIds = useMemo(
        () => getPathIds(messages, selectedLeafId),
        [messages, selectedLeafId],
    );

    // ---------------------------------------------------------------------------
    // Pan & zoom event handlers
    // ---------------------------------------------------------------------------

    const handleMouseDown = useCallback(
        (e: React.MouseEvent<SVGSVGElement>) => {
            isPanning.current = true;
            didMove.current = false;
            lastMouse.current = { x: e.clientX, y: e.clientY };
        },
        [],
    );

    const handleMouseMove = useCallback(
        (e: React.MouseEvent<SVGSVGElement>) => {
            if (!isPanning.current) return;
            const dx = e.clientX - lastMouse.current.x;
            const dy = e.clientY - lastMouse.current.y;
            if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
                didMove.current = true;
            }
            lastMouse.current = { x: e.clientX, y: e.clientY };
            setPan((prev) => ({ x: prev.x + dx, y: prev.y + dy }));
        },
        [],
    );

    const handleMouseUp = useCallback(() => {
        isPanning.current = false;
    }, []);

    // Attach the wheel listener directly (non-passive) so we can call
    // e.preventDefault() and prevent the page from scrolling.
    useEffect(() => {
        const svg = svgRef.current;
        if (!svg) return;

        const handleWheel = (e: WheelEvent) => {
            e.preventDefault();
            const scaleFactor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
            const rect = svg.getBoundingClientRect();
            const mx = e.clientX - rect.left;
            const my = e.clientY - rect.top;
            // Zoom around the mouse pointer.
            setPan((prev) => ({
                x: mx + (prev.x - mx) * scaleFactor,
                y: my + (prev.y - my) * scaleFactor,
            }));
            setScale((prev) => Math.max(0.15, Math.min(4, prev * scaleFactor)));
        };

        svg.addEventListener("wheel", handleWheel, { passive: false });
        return () => svg.removeEventListener("wheel", handleWheel);
    }, []);

    // ---------------------------------------------------------------------------
    // SVG content
    // ---------------------------------------------------------------------------

    const edges = useMemo(() => {
        const result: React.ReactNode[] = [];
        for (const id of Object.keys(messages)) {
            const node = messages[id];
            if (!node.parent_id) continue;
            const parentPos = positions[node.parent_id];
            const childPos = positions[id];
            if (!parentPos || !childPos) continue;
            const x1 = parentPos.x + NODE_W / 2;
            const y1 = parentPos.y + NODE_H;
            const x2 = childPos.x + NODE_W / 2;
            const y2 = childPos.y;
            // Curved bezier for a nicer look.
            const cy = (y1 + y2) / 2;
            result.push(
                <path
                    key={`edge-${id}`}
                    d={`M ${x1} ${y1} C ${x1} ${cy}, ${x2} ${cy}, ${x2} ${y2}`}
                    stroke="#4b5563"
                    strokeWidth={1.5}
                    fill="none"
                />,
            );
        }
        return result;
    }, [messages, positions]);

    const nodeEls = useMemo(() => {
        return Object.keys(messages).map((id) => {
            const node = messages[id];
            const pos = positions[id];
            if (!pos) return null;

            const isSelected = id === selectedLeafId;
            const isOnPath = pathIds.has(id);

            const bgColor =
                node.role === "user"
                    ? "#1d4ed8"
                    : node.role === "system"
                      ? "#6d28d9"
                      : "#374151";

            const rawText = node.content.join("");
            const previewMaxLength = 30;
            const preview =
                rawText.length > previewMaxLength
                    ? rawText.slice(0, previewMaxLength - 3) + "…"
                    : rawText || (node.role === "assistant" ? "…" : "");

            const strokeColor = isSelected
                ? "#facc15"
                : isOnPath
                  ? "#60a5fa"
                  : "#6b7280";
            const strokeWidth = isSelected ? 2.5 : isOnPath ? 2 : 1;

            const roleLabel =
                node.role === "user"
                    ? "User"
                    : node.role === "assistant"
                      ? "Assistant"
                      : "System";

            const hasChildren = (childrenMap[id] ?? []).length > 0;
            const childCount = (childrenMap[id] ?? []).length;

            return (
                <g
                    key={id}
                    onClick={(e) => {
                        e.stopPropagation();
                        if (!didMove.current) {
                            onSelectNode(id);
                        }
                    }}
                    style={{ cursor: "pointer" }}
                >
                    <rect
                        x={pos.x}
                        y={pos.y}
                        width={NODE_W}
                        height={NODE_H}
                        rx={8}
                        fill={bgColor}
                        stroke={strokeColor}
                        strokeWidth={strokeWidth}
                    />
                    {/* Role label */}
                    <text
                        x={pos.x + 8}
                        y={pos.y + 15}
                        fill="#e5e7eb"
                        fontSize={10}
                        fontWeight="bold"
                        fontFamily="system-ui, sans-serif"
                    >
                        {roleLabel}
                        {hasChildren && childCount > 1
                            ? ` (${childCount} branches)`
                            : ""}
                    </text>
                    {/* Content preview */}
                    <text
                        x={pos.x + 8}
                        y={pos.y + 31}
                        fill="#d1d5db"
                        fontSize={10}
                        fontFamily="system-ui, sans-serif"
                    >
                        {preview}
                    </text>
                    {/* UUID snippet */}
                    <text
                        x={pos.x + 8}
                        y={pos.y + 46}
                        fill="#6b7280"
                        fontSize={9}
                        fontFamily="monospace"
                    >
                        {id.slice(0, 8)}…
                    </text>
                </g>
            );
        });
    }, [
        messages,
        positions,
        selectedLeafId,
        pathIds,
        childrenMap,
        onSelectNode,
    ]);

    if (Object.keys(messages).length === 0) {
        return (
            <div className="flex items-center justify-center h-full text-gray-500 text-sm">
                No messages yet
            </div>
        );
    }

    return (
        <svg
            ref={svgRef}
            className="w-full h-full select-none"
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
            onMouseLeave={handleMouseUp}
            style={{ cursor: isPanning.current ? "grabbing" : "grab" }}
        >
            <g transform={`translate(${pan.x}, ${pan.y}) scale(${scale})`}>
                {/* Draw edges underneath nodes */}
                {edges}
                {nodeEls}
            </g>
        </svg>
    );
}

import {
    useMemo,
    useRef,
    useState,
    useCallback,
    useEffect,
    forwardRef,
    useImperativeHandle,
} from "react";
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

/** Extra screen-space margin (px) for viewport culling — keeps edges from
 *  popping in/out right at the viewport boundary. */
const CULL_BUFFER = 200;

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

function computeLayout(
    messages: Record<string, ChatMessage>,
    pathIds?: Set<string>,
): LayoutResult {
    // Build children map, treating null parent as "__root__".
    const childrenMap: Record<string, string[]> = {};
    for (const id of Object.keys(messages)) {
        const node = messages[id];
        const parentKey = node.parent_id ?? "__root__";
        if (!childrenMap[parentKey]) childrenMap[parentKey] = [];
        childrenMap[parentKey].push(id);
    }
    // Sort by UUID for stable UUIDv7 insertion order, then reorder so the
    // on-path child sits in the middle index.  This makes the active trunk go
    // roughly straight down instead of drifting sideways at every branch point.
    for (const key of Object.keys(childrenMap)) {
        childrenMap[key].sort((a, b) => a.localeCompare(b));

        if (pathIds && pathIds.size > 0 && childrenMap[key].length > 1) {
            const kids = childrenMap[key];
            const pathIdx = kids.findIndex((c) => pathIds.has(c));
            if (pathIdx !== -1) {
                const pathChild = kids[pathIdx];
                const others = kids.filter((c) => c !== pathChild);
                // Split non-path siblings: half on the left, half on the right.
                const leftCount = Math.floor(others.length / 2);
                childrenMap[key] = [
                    ...others.slice(0, leftCount),
                    pathChild,
                    ...others.slice(leftCount),
                ];
            }
        }
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
// Public handle type (exposed via forwardRef)
// ---------------------------------------------------------------------------

export interface ChatFlowchartHandle {
    /** Pan the view to centre on nodeId without changing the zoom level. */
    centerOnNode: (nodeId: string) => void;
}

// ---------------------------------------------------------------------------
// Component props
// ---------------------------------------------------------------------------

export interface ChatFlowchartProps {
    messages: Record<string, ChatMessage>;
    selectedLeafId: string | null;
    onSelectNode: (nodeId: string) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ChatFlowchart = forwardRef<ChatFlowchartHandle, ChatFlowchartProps>(
    function ChatFlowchart({ messages, selectedLeafId, onSelectNode }, ref) {
        const [pan, setPan] = useState({ x: 16, y: 16 });
        const [scale, setScale] = useState(1);

        // Mirror scale into a ref so centerOnNode can read it without being
        // listed as a dep (avoids unnecessary re-renders and effect cycles).
        const scaleRef = useRef(scale);
        useEffect(() => {
            scaleRef.current = scale;
        }, [scale]);

        // Track SVG viewport size for culling and auto-fit.
        const [svgSize, setSvgSize] = useState({ width: 0, height: 0 });

        const isPanning = useRef(false);
        const lastMouse = useRef({ x: 0, y: 0 });
        // Track whether the mouse actually moved during a press (to distinguish
        // panning from a click).
        const didMove = useRef(false);

        // Guard: only auto-fit once per mount.
        const hasAutoFitted = useRef(false);

        const svgRef = useRef<SVGSVGElement>(null);

        // Set of node IDs on the current path (computed first so layout can use it).
        const pathIds = useMemo(
            () => getPathIds(messages, selectedLeafId),
            [messages, selectedLeafId],
        );

        // Recompute layout whenever messages or the active path changes.
        // pathIds is passed so the on-path child is placed in the middle at each
        // branch point, keeping the active trunk roughly straight down.
        const { positions, childrenMap } = useMemo(
            () => computeLayout(messages, pathIds),
            [messages, pathIds],
        );

        // ---------------------------------------------------------------------------
        // SVG resize observation (for culling bounds and auto-fit)
        // ---------------------------------------------------------------------------

        useEffect(() => {
            const svg = svgRef.current;
            if (!svg) return;
            const ro = new ResizeObserver(([entry]) => {
                setSvgSize({
                    width: entry.contentRect.width,
                    height: entry.contentRect.height,
                });
            });
            ro.observe(svg);
            return () => ro.disconnect();
        }, []);

        // ---------------------------------------------------------------------------
        // centerOnNodeInternal — the single function that moves the view.
        //
        //   targetScale  provided → also zoom to that scale
        //   targetScale  omitted  → keep current zoom (reads scaleRef)
        // ---------------------------------------------------------------------------

        const centerOnNodeInternal = useCallback(
            (nodeId: string, targetScale?: number) => {
                const svg = svgRef.current;
                const pos = positions[nodeId];
                if (!pos || !svg) return;
                const w = svg.clientWidth;
                const h = svg.clientHeight;
                if (w === 0 || h === 0) return;
                const s = targetScale ?? scaleRef.current;
                setPan({
                    x: w / 2 - (pos.x + NODE_W / 2) * s,
                    y: h / 2 - (pos.y + NODE_H / 2) * s,
                });
                if (targetScale !== undefined) {
                    setScale(targetScale);
                }
            },
            [positions],
        );

        // Expose the "pan to node at current zoom" method to the parent.
        useImperativeHandle(
            ref,
            () => ({
                centerOnNode: (nodeId: string) =>
                    centerOnNodeInternal(nodeId),
            }),
            [centerOnNodeInternal],
        );

        // ---------------------------------------------------------------------------
        // Auto-fit: centre on the selected leaf once on initial load
        // ---------------------------------------------------------------------------

        useEffect(() => {
            if (hasAutoFitted.current) return;
            if (!selectedLeafId || !positions[selectedLeafId]) return;

            const tryFit = () => {
                const svg = svgRef.current;
                if (!svg || svg.clientWidth === 0) return;
                hasAutoFitted.current = true;
                centerOnNodeInternal(selectedLeafId, 1.0);
            };

            // Defer one frame so the SVG has been laid out and has real dimensions.
            requestAnimationFrame(tryFit);
        }, [selectedLeafId, positions, centerOnNodeInternal]);

        // ---------------------------------------------------------------------------
        // Viewport culling: only render nodes (and their edges) that are within the
        // visible area + a small buffer.
        // ---------------------------------------------------------------------------

        const visibleIds = useMemo((): Set<string> => {
            // Before the SVG has been measured, show everything.
            if (svgSize.width === 0 && svgSize.height === 0) {
                return new Set(Object.keys(messages));
            }
            const { width, height } = svgSize;
            const result = new Set<string>();
            for (const id of Object.keys(messages)) {
                const pos = positions[id];
                if (!pos) continue;
                // Transform: screen = canvas * scale + pan
                const sx = pos.x * scale + pan.x;
                const sy = pos.y * scale + pan.y;
                const sw = NODE_W * scale;
                const sh = NODE_H * scale;
                if (
                    sx + sw >= -CULL_BUFFER &&
                    sx <= width + CULL_BUFFER &&
                    sy + sh >= -CULL_BUFFER &&
                    sy <= height + CULL_BUFFER
                ) {
                    result.add(id);
                }
            }
            return result;
        }, [messages, positions, scale, pan, svgSize]);

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
                setScale((prev) =>
                    Math.max(0.05, Math.min(4, prev * scaleFactor)),
                );
            };

            svg.addEventListener("wheel", handleWheel, { passive: false });
            return () => svg.removeEventListener("wheel", handleWheel);
        }, []);

        // ---------------------------------------------------------------------------
        // SVG content (culled)
        // ---------------------------------------------------------------------------

        const edges = useMemo(() => {
            const result: React.ReactNode[] = [];
            for (const id of Object.keys(messages)) {
                const node = messages[id];
                if (!node.parent_id) continue;
                // Draw an edge if either endpoint is visible.
                if (
                    !visibleIds.has(id) &&
                    !visibleIds.has(node.parent_id)
                )
                    continue;
                const parentPos = positions[node.parent_id];
                const childPos = positions[id];
                if (!parentPos || !childPos) continue;
                const x1 = parentPos.x + NODE_W / 2;
                const y1 = parentPos.y + NODE_H;
                const x2 = childPos.x + NODE_W / 2;
                const y2 = childPos.y;
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
        }, [messages, positions, visibleIds]);

        const nodeEls = useMemo(() => {
            return Object.keys(messages)
                .filter((id) => visibleIds.has(id))
                .map((id) => {
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
            visibleIds,
        ]);

        // ---------------------------------------------------------------------------
        // "Re-centre" button — snaps view back to the selected node at scale 1
        // ---------------------------------------------------------------------------

        const handleRecentre = useCallback(() => {
            if (!selectedLeafId) return;
            centerOnNodeInternal(selectedLeafId, 1.0);
        }, [selectedLeafId, centerOnNodeInternal]);

        if (Object.keys(messages).length === 0) {
            return (
                <div className="flex items-center justify-center h-full text-gray-500 text-sm">
                    No messages yet
                </div>
            );
        }

        return (
            <div className="relative w-full h-full">
                <svg
                    ref={svgRef}
                    className="w-full h-full select-none"
                    onMouseDown={handleMouseDown}
                    onMouseMove={handleMouseMove}
                    onMouseUp={handleMouseUp}
                    onMouseLeave={handleMouseUp}
                    style={{
                        cursor: isPanning.current ? "grabbing" : "grab",
                    }}
                >
                    <g
                        transform={`translate(${pan.x}, ${pan.y}) scale(${scale})`}
                    >
                        {/* Draw edges underneath nodes */}
                        {edges}
                        {nodeEls}
                    </g>
                </svg>

                {/* Re-centre button overlay */}
                {selectedLeafId && (
                    <button
                        onClick={handleRecentre}
                        title="Re-centre on selected node (zoom reset to 100%)"
                        className="absolute bottom-2 right-2 px-2 py-1 text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 rounded shadow transition-colors"
                    >
                        ⌖ Re-centre
                    </button>
                )}

                {/* Visible node count */}
                <div className="absolute bottom-2 left-2 text-xs text-gray-600 pointer-events-none">
                    {visibleIds.size} / {Object.keys(messages).length}
                </div>
            </div>
        );
    },
);

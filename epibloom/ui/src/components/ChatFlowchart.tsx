import {
    useMemo,
    forwardRef,
    useImperativeHandle,
    useCallback,
    useRef,
    useEffect,
    memo,
} from "react";
import {
    ReactFlow,
    ReactFlowProvider,
    useReactFlow,
    Handle,
    Position,
    Background,
    BackgroundVariant,
    type Node,
    type NodeProps,
    type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import Dagre from "@dagrejs/dagre";
import { ChatMessage } from "../types/api";

// ---------------------------------------------------------------------------
// Layout constants (must be consistent between dagre and the node component)
// ---------------------------------------------------------------------------

const NODE_W = 160;
const NODE_H = 54;

// ---------------------------------------------------------------------------
// Data type for custom nodes
// ---------------------------------------------------------------------------

// NodeData must extend Record<string, unknown> for React Flow's type system.
interface ChatNodeData extends Record<string, unknown> {
    message: ChatMessage;
    isSelected: boolean;
    isOnPath: boolean;
    childCount: number;
}

// Concrete node type used throughout this file.
type ChatNode = Node<ChatNodeData, "chatMessage">;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
// Dagre layout
// ---------------------------------------------------------------------------

function computeLayout(
    messages: Record<string, ChatMessage>,
): Record<string, { x: number; y: number }> {
    const g = new Dagre.graphlib.Graph();
    g.setDefaultEdgeLabel(() => ({}));
    g.setGraph({
        rankdir: "TB",
        // Horizontal gap between sibling nodes.
        nodesep: 20,
        // Vertical gap between ranks.
        ranksep: 36,
    });

    for (const id of Object.keys(messages)) {
        g.setNode(id, { width: NODE_W, height: NODE_H });
    }
    for (const id of Object.keys(messages)) {
        const msg = messages[id];
        if (msg.parent_id) {
            g.setEdge(msg.parent_id, id);
        }
    }

    Dagre.layout(g);

    // dagre gives the centre of each node; React Flow wants the top-left.
    const positions: Record<string, { x: number; y: number }> = {};
    for (const id of Object.keys(messages)) {
        const n = g.node(id);
        if (n) {
            positions[id] = { x: n.x - NODE_W / 2, y: n.y - NODE_H / 2 };
        }
    }
    return positions;
}

// ---------------------------------------------------------------------------
// Custom node component
//
// Must be defined outside the main component so the reference is stable
// across renders (React Flow unmounts/remounts nodes when nodeTypes changes).
// ---------------------------------------------------------------------------

const ChatNodeComponent = memo(function ChatNodeComponent({ id, data }: NodeProps<ChatNode>) {
    const { message, isSelected, isOnPath, childCount } = data;

    const bgColor =
        message.role === "user"
            ? "#1d4ed8"
            : message.role === "system"
              ? "#6d28d9"
              : "#374151";

    const borderColor = isSelected
        ? "#facc15"
        : isOnPath
          ? "#60a5fa"
          : "#6b7280";
    const borderWidth = isSelected ? 2.5 : isOnPath ? 2 : 1;

    const roleLabel =
        message.role === "user"
            ? "User"
            : message.role === "assistant"
              ? "Assistant"
              : "System";

    const rawText = message.content.join("");
    const previewMaxLength = 30;
    const preview =
        rawText.length > previewMaxLength
            ? rawText.slice(0, previewMaxLength - 3) + "…"
            : rawText || (message.role === "assistant" ? "…" : "");

    return (
        <div
            style={{
                width: NODE_W,
                height: NODE_H,
                background: bgColor,
                border: `${borderWidth}px solid ${borderColor}`,
                borderRadius: 8,
                padding: "5px 8px",
                cursor: "pointer",
                boxSizing: "border-box",
                fontFamily: "system-ui, sans-serif",
                userSelect: "none",
                overflow: "hidden",
            }}
        >
            {/* Invisible handles so React Flow can draw edges */}
            <Handle
                type="target"
                position={Position.Top}
                style={{ opacity: 0, pointerEvents: "none" }}
            />

            <div
                style={{
                    fontSize: 10,
                    fontWeight: "bold",
                    color: "#e5e7eb",
                    lineHeight: "1.4",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                }}
            >
                {roleLabel}
                {childCount > 1 ? ` (${childCount} branches)` : ""}
            </div>
            <div
                style={{
                    fontSize: 10,
                    color: "#d1d5db",
                    lineHeight: "1.4",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                }}
            >
                {preview}
            </div>
            <div
                style={{
                    fontSize: 9,
                    color: "#6b7280",
                    fontFamily: "monospace",
                    lineHeight: "1.4",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                }}
            >
                {id.slice(0, 8)}…
            </div>

            <Handle
                type="source"
                position={Position.Bottom}
                style={{ opacity: 0, pointerEvents: "none" }}
            />
        </div>
    );
});

// Stable reference — defined once, outside of any component.
const nodeTypes = { chatMessage: ChatNodeComponent };

// ---------------------------------------------------------------------------
// Public handle / props types
// ---------------------------------------------------------------------------

export interface ChatFlowchartHandle {
    centerOnNode: (nodeId: string) => void;
}

export interface ChatFlowchartProps {
    messages: Record<string, ChatMessage>;
    selectedLeafId: string | null;
    onSelectNode: (nodeId: string) => void;
}

// ---------------------------------------------------------------------------
// Inner component — must be rendered inside ReactFlowProvider to use
// useReactFlow().
// ---------------------------------------------------------------------------

const ChatFlowchartInner = forwardRef<ChatFlowchartHandle, ChatFlowchartProps>(
    function ChatFlowchartInner({ messages, selectedLeafId, onSelectNode }, ref) {
        const { setCenter } = useReactFlow();

        // Which nodes are on the active path (root → selectedLeaf).
        const pathIds = useMemo(
            () => getPathIds(messages, selectedLeafId),
            [messages, selectedLeafId],
        );

        // Re-run dagre only when the set of node IDs changes, not on every
        // content/selection update (those go through node data instead).
        const messageIdKey = useMemo(
            () => Object.keys(messages).sort().join(","),
            [messages],
        );
        // eslint-disable-next-line react-hooks/exhaustive-deps
        const positions = useMemo(() => computeLayout(messages), [messageIdKey]);

        // Children map — used to derive childCount for the branch label.
        const childrenMap = useMemo(() => {
            const map: Record<string, string[]> = {};
            for (const id of Object.keys(messages)) {
                const parentKey = messages[id].parent_id ?? "__root__";
                if (!map[parentKey]) map[parentKey] = [];
                map[parentKey].push(id);
            }
            return map;
        }, [messages]);

        // React Flow node array.
        const nodes = useMemo<ChatNode[]>(
            () =>
                Object.keys(messages).map((id) => ({
                    id,
                    type: "chatMessage" as const,
                    position: positions[id] ?? { x: 0, y: 0 },
                    // Telling React Flow the size avoids a measurement round-trip
                    // and keeps dagre's edge attachment points accurate.
                    width: NODE_W,
                    height: NODE_H,
                    data: {
                        message: messages[id],
                        isSelected: id === selectedLeafId,
                        isOnPath: pathIds.has(id),
                        childCount: (childrenMap[id] ?? []).length,
                    },
                    selectable: false,
                    draggable: false,
                })),
            [messages, positions, selectedLeafId, pathIds, childrenMap],
        );

        // React Flow edge array.
        const edges = useMemo(
            () =>
                Object.keys(messages)
                    .filter((id) => messages[id].parent_id)
                    .map((id) => ({
                        id: `e-${id}`,
                        source: messages[id].parent_id!,
                        target: id,
                    })),
            [messages],
        );

        // ---------------------------------------------------------------------------
        // Viewport helpers
        // ---------------------------------------------------------------------------

        const centerOn = useCallback(
            (nodeId: string, duration: number) => {
                const pos = positions[nodeId];
                if (!pos) return;
                setCenter(pos.x + NODE_W / 2, pos.y + NODE_H / 2, {
                    zoom: 1,
                    duration,
                });
            },
            [positions, setCenter],
        );

        // Auto-fit: once the selected leaf is known and positions are ready,
        // snap to it instantly (no animation on first load).
        const hasAutoFitted = useRef(false);
        useEffect(() => {
            if (hasAutoFitted.current || !selectedLeafId) return;
            if (!positions[selectedLeafId]) return;
            // requestAnimationFrame defers until after React Flow has painted its
            // viewport, so setCenter has valid dimensions to work with.
            requestAnimationFrame(() => {
                centerOn(selectedLeafId, 0);
                hasAutoFitted.current = true;
            });
        }, [selectedLeafId, positions, centerOn]);

        // Expose the imperative handle to the parent (ChatView).
        useImperativeHandle(
            ref,
            () => ({
                centerOnNode: (nodeId: string) => centerOn(nodeId, 300),
            }),
            [centerOn],
        );

        // ---------------------------------------------------------------------------
        // Event handlers
        // ---------------------------------------------------------------------------

        const handleNodeClick = useCallback<NodeMouseHandler>(
            (_, node) => {
                onSelectNode(node.id);
            },
            [onSelectNode],
        );

        const handleRecentre = useCallback(() => {
            if (!selectedLeafId) return;
            centerOn(selectedLeafId, 300);
        }, [selectedLeafId, centerOn]);

        // ---------------------------------------------------------------------------
        // Render
        // ---------------------------------------------------------------------------

        return (
            <div style={{ width: "100%", height: "100%", position: "relative" }}>
                <ReactFlow
                    nodes={nodes}
                    edges={edges}
                    nodeTypes={nodeTypes}
                    onNodeClick={handleNodeClick}
                    // Appearance
                    colorMode="dark"
                    defaultEdgeOptions={{
                        style: { stroke: "#4b5563", strokeWidth: 1.5 },
                        // Cubic-bezier curves, matching the previous SVG style.
                        type: "default",
                    }}
                    // Interaction
                    nodesDraggable={false}
                    nodesConnectable={false}
                    elementsSelectable={false}
                    // Zoom limits
                    minZoom={0.05}
                    maxZoom={4}
                    // Don't auto-fit on mount — we handle that ourselves via
                    // centerOn so we can target the selected leaf.
                    fitView={false}
                >
                    <Background
                        variant={BackgroundVariant.Dots}
                        color="#1f2937"
                        gap={16}
                    />
                </ReactFlow>

                {/* Re-centre button */}
                {selectedLeafId && (
                    <button
                        onClick={handleRecentre}
                        title="Re-centre on selected node (zoom reset to 100%)"
                        className="absolute bottom-2 right-2 px-2 py-1 text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 rounded shadow transition-colors"
                    >
                        ⌖ Re-centre
                    </button>
                )}

                {/* Node count */}
                <div className="absolute bottom-2 left-2 text-xs text-gray-600 pointer-events-none">
                    {Object.keys(messages).length}
                </div>
            </div>
        );
    },
);

// ---------------------------------------------------------------------------
// Outer component — wraps the inner in a ReactFlowProvider so that
// useReactFlow() works inside ChatFlowchartInner.
// ---------------------------------------------------------------------------

export const ChatFlowchart = forwardRef<ChatFlowchartHandle, ChatFlowchartProps>(
    function ChatFlowchart(props, ref) {
        if (Object.keys(props.messages).length === 0) {
            return (
                <div className="flex items-center justify-center h-full text-gray-500 text-sm">
                    No messages yet
                </div>
            );
        }
        return (
            <ReactFlowProvider>
                <ChatFlowchartInner {...props} ref={ref} />
            </ReactFlowProvider>
        );
    },
);

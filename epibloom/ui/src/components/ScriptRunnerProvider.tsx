import {
    ReactNode,
    useCallback,
    useEffect,
    useMemo,
    useRef,
    useState,
} from "react";
import { useStateObjects } from "../hooks/useStateObjects";
import {
    RunScript,
    ScriptError,
    ScriptRunnerContext,
} from "../hooks/useScriptRunner";
import { ScriptStatus } from "../types/api";

const STATUS_PREFIX = "script-status:";

interface PendingRun {
    context: string;
    index: number;
    resolve: (result: unknown) => void;
    reject: (error: Error) => void;
}

/**
 * Provides `useScriptRunner()`.
 *
 * Running a script via the API only returns an index into the context's
 * `script-status:<context>` state object; the result arrives there later. This
 * subscribes (over a single connection) to the status objects of every context
 * that has been used, and resolves pending runs as they finish.
 */
export function ScriptRunnerProvider({ children }: { children: ReactNode }) {
    const [contexts, setContexts] = useState<string[]>([]);

    const objectNames = useMemo(
        () => contexts.map((context) => `${STATUS_PREFIX}${context}`),
        [contexts],
    );
    const { objects } = useStateObjects(objectNames);

    const pendingRef = useRef<Map<string, PendingRun>>(new Map());
    const historiesRef = useRef<Map<string, ScriptStatus[]>>(new Map());

    const settlePending = useCallback(() => {
        for (const [key, pending] of pendingRef.current) {
            const status = historiesRef.current.get(pending.context)?.[
                pending.index
            ];
            if (!status?.end_time) continue;

            pendingRef.current.delete(key);
            if (status.error) {
                pending.reject(new ScriptError(status.error));
            } else {
                pending.resolve(status.result);
            }
        }
    }, []);

    useEffect(() => {
        historiesRef.current = new Map(
            objects.map((obj) => [
                obj.name.slice(STATUS_PREFIX.length),
                (obj.value as ScriptStatus[] | null) ?? [],
            ]),
        );
        settlePending();
    }, [objects, settlePending]);

    const run: RunScript = useCallback(
        async (context, scriptPath, input) => {
            setContexts((prev) =>
                prev.includes(context) ? prev : [...prev, context],
            );

            const response = await fetch(
                `/api/script/${context}/${scriptPath}`,
                {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify(input),
                },
            );
            if (!response.ok) {
                throw new Error(
                    `Failed to run script ${scriptPath} on context ${context}: ${response.status} ${await response.text()}`,
                );
            }
            const index: number = await response.json();

            return new Promise<unknown>((resolve, reject) => {
                pendingRef.current.set(`${context}#${index}`, {
                    context,
                    index,
                    resolve,
                    reject,
                });
                // It may have already finished.
                settlePending();
            });
        },
        [settlePending],
    );

    return (
        <ScriptRunnerContext.Provider value={run}>
            {children}
        </ScriptRunnerContext.Provider>
    );
}

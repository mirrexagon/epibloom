import { useState } from "react";

interface UseScriptResult<T> {
    data: T | null;
    error: string | null;
    loading: boolean;
    runScript: (input: Record<string, unknown>) => Promise<T | null>;
}

export function useScript<T = unknown>(
    contextName: string,
    scriptPath: string,
): UseScriptResult<T> {
    const [data, setData] = useState<T | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);

    const runScript = async (
        input: Record<string, unknown>,
    ): Promise<T | null> => {
        setLoading(true);
        setError(null);

        try {
            const response = await fetch(
                `/api/script/${contextName}/${scriptPath}`,
                {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                    },
                    body: JSON.stringify(input),
                },
            );

            if (!response.ok) {
                const errorText = await response.text();
                throw new Error(
                    `Script execution failed: ${response.status} ${errorText}`,
                );
            }

            const result: T = await response.json();
            setData(result);
            return result;
        } catch (err) {
            const errorMessage =
                err instanceof Error ? err.message : "Unknown error";
            setError(errorMessage);
            console.error("Script execution error:", err);
            return null;
        } finally {
            setLoading(false);
        }
    };

    return { data, error, loading, runScript };
}

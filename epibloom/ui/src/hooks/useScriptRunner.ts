import { createContext, useContext } from "react";

/**
 * Run a script on the given context and wait for it to finish.
 *
 * Resolves with the script's return value, or rejects with a `ScriptError` if
 * the script raised an error.
 */
export type RunScript = (
    context: string,
    scriptPath: string,
    input: unknown,
) => Promise<unknown>;

export class ScriptError extends Error {
    /** The full error message, including the Lua stack traceback. */
    readonly fullMessage: string;

    constructor(fullMessage: string) {
        super(summarizeScriptError(fullMessage));
        this.name = "ScriptError";
        this.fullMessage = fullMessage;
    }
}

/**
 * Strip the boilerplate prefix, source location, and stack traceback from a
 * script error message, leaving just the message itself.
 */
export function summarizeScriptError(message: string): string {
    let summary = message.split("\nstack traceback:")[0];
    summary = summary.replace(
        /^Error executing script .*? on context .*?: (runtime error: )?/,
        "",
    );
    summary = summary.replace(/^[^\s:]+\.lua:\d+: /, "");
    return summary.trim();
}

export const ScriptRunnerContext = createContext<RunScript | null>(null);

/** Get the `RunScript` function. Must be used inside a `ScriptRunnerProvider`. */
export function useScriptRunner(): RunScript {
    const run = useContext(ScriptRunnerContext);
    if (!run) {
        throw new Error(
            "useScriptRunner() must be used inside a ScriptRunnerProvider",
        );
    }
    return run;
}

import { useState, useCallback } from "react";
import { TraceList } from "./components/TraceList";
import { TraceView } from "./components/TraceView";
import { TraceMetadata } from "./types/api";

function App() {
    const [selectedTrace, setSelectedTrace] = useState<TraceMetadata | null>(
        null,
    );

    const handleSelectTrace = useCallback((trace: TraceMetadata) => {
        console.log(`Selected ${trace.id}`);
        setSelectedTrace(trace);
    }, []);

    return (
        <div className="h-screen flex flex-col bg-gray-900">
            {/* App header */}
            <header className="bg-gray-800 text-white p-4 shadow-lg">
                <h1 className="text-2xl font-bold">Epibloom</h1>
            </header>

            {/* Main content */}
            <div className="flex-1 flex overflow-hidden">
                {/* Sidebar */}
                <div className="w-80 flex-shrink-0">
                    <TraceList
                        selectedTraceId={selectedTrace?.id || null}
                        onSelectTrace={handleSelectTrace}
                    />
                </div>

                {/* Main view */}
                <div className="flex-1">
                    <TraceView
                        traceId={selectedTrace?.id || null}
                        traceName={selectedTrace?.name || null}
                    />
                </div>
            </div>
        </div>
    );
}

export default App;

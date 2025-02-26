import { useState } from "react";
import "./App.css";

function App() {
    const [prompt, setPrompt] = useState("");

    const handleSubmit = () => {
        // TODO: Implement LLM interaction
        console.log("Submitted prompt:", prompt);
    };

    return (
        <div className="container">
            <header>
                <h1>AI Chat Assistant</h1>
            </header>
            <main>
                <textarea
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    placeholder="Enter your prompt here..."
                    rows={10}
                />
                <button onClick={handleSubmit}>
                    Submit
                </button>
            </main>
        </div>
    );
}

export default App;

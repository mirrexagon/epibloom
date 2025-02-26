import { useState } from "react";
import "./App.css";

function App() {
    const [prompt, setPrompt] = useState("");

    const handleSubmit = () => {
        // TODO: Implement LLM interaction
        console.log("Submitted prompt:", prompt);
    };

    return (
        <div id="app-container" className="container">
            <header id="app-header">
                <h1 id="app-title">Epibloom</h1>
            </header>
            <main id="app-main">
                <textarea
                    id="prompt-input"
                    value={prompt}
                    onChange={(e) => setPrompt(e.target.value)}
                    placeholder="Enter your prompt here..."
                    rows={10}
                />
                <button id="submit-button" onClick={handleSubmit}>Submit</button>
            </main>
        </div>
    );
}

export default App;

@react.component
let make = () => {
  let (prompt, setPrompt) = React.useState(_ => "")

  let handleSubmit = e => {
    e->ReactEvent.Form.preventDefault
    Js.Console.log(`Submitted prompt: ${prompt}`)
  }

  <div className="p-6 max-w-2xl mx-auto">
    <div className="flex items-center justify-center mb-6">
      <img src="/epibloom-logo.svg" alt="Epibloom logo" className="h-12 mr-3" />
      <h1 className="text-3xl font-bold text-center"> {"Epibloom"->React.string} </h1>
    </div>
    
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <textarea
          className="w-full p-3 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Enter your prompt here..."
          rows={5}
          value={prompt}
          onChange={e => setPrompt(ReactEvent.Form.target(e)["value"])}
        />
      </div>
      
      <div className="text-right">
        <Button type_="submit">
          {React.string("Submit")}
        </Button>
      </div>
    </form>
  </div>
}

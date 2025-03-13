@react.component
let make = () => {
  <div className="p-6 max-w-2xl mx-auto">
    <div className="flex items-center justify-center mb-6">
      <img src="/epibloom-logo.svg" alt="Epibloom logo" className="h-12 mr-3" />
      <h1 className="text-3xl font-bold text-center"> {"Epibloom"->React.string} </h1>
    </div>

    <ChatInterface />
  </div>
}

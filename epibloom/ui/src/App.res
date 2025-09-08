@react.component
let make = () => {
  let (selectedChatId, setSelectedChatId) = React.useState(() => None)

  <div className="flex h-screen bg-gray-900 text-white">
    <ChatList selectedChatId onChatSelect={chatId => setSelectedChatId(_ => Some(chatId))} />
    <div className="flex-1 p-6 bg-gray-900">
      <div className="flex items-center justify-center mb-6">
        <img src="/epibloom-logo.svg" alt="Epibloom logo" className="h-12 mr-3" />
        <h1 className="text-3xl font-bold text-center text-white"> {React.string("Epibloom")} </h1>
      </div>
      <ChatInterface selectedChatId />
    </div>
    <RenderedChatView selectedChatId />
  </div>
}

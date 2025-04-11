type message = {
  text: string,
  sender: [#User | #AI]
}

@react.component
let make = () => {
  let (messages, setMessages) = React.useState(_ => [])
  let (inputValue, setInputValue) = React.useState(_ => "")

  let handleSubmit = e => {
    ReactEvent.Form.preventDefault(e)

    if String.trim(inputValue) != "" {
      // Add user message
      let userMessage = {text: inputValue, sender: #User}
      setMessages(prev => prev->Array.concat([userMessage]))

      // Simulate AI response (in a real app, this would be an API call)
      let aiResponse = {text: `Response to: ${inputValue}`, sender: #AI}
      Js.Global.setTimeout(() => {
        setMessages(prev => prev->Array.concat([aiResponse]))
      }, 500)->ignore

      // Clear input
      setInputValue(_ => "")
    }
  }

  <div className="flex flex-col h-[600px] border border-gray-300 rounded-md overflow-hidden">
    // Chat history
    <div className="flex-1 p-4 overflow-y-auto">
      <div className="space-y-4">
        {messages->Array.mapWithIndex((message, index) => {
          let isUser = message.sender == #User
          <div
            key={index->Int.toString}
            className={`flex ${isUser ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-[80%] p-3 rounded-lg ${
                isUser
                  ? "bg-blue-600 text-white"
                  : "bg-gray-200 text-gray-800"
              }`}
            >
              {React.string(message.text)}
            </div>
          </div>
        })->React.array}
      </div>
    </div>

    // Input area
    <form onSubmit={handleSubmit} className="border-t border-gray-300 p-3">
      <div className="flex items-center">
        <input
          type_="text"
          className="flex-1 p-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder="Type a message..."
          value={inputValue}
          onChange={e => setInputValue(ReactEvent.Form.target(e)["value"])}
        />
        <Button type_="submit" className="ml-2">
          {React.string("Send")}
        </Button>
      </div>
    </form>
  </div>
}

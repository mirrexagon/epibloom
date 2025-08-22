@react.component
let make = (~selectedChatId: option<string>) => {
  let (chat, setChat) = React.useState(_ => None)
  let (loading, setLoading) = React.useState(_ => false)
  let (error, setError) = React.useState(_ => None)
  let (inputValue, setInputValue) = React.useState(_ => "")

  React.useEffect(() => {
    switch selectedChatId {
    | None => {
        setChat(_ => None)
        setError(_ => None)
      }

    | Some(chatId) => {
        let loadChat = async () => {
          setLoading(_ => true)
          setError(_ => None)

          let result = await Api.fetchChat(chatId)
          switch result {
          | Ok(chatData) => setChat(_ => Some(chatData))
          | Error(errorMsg) => setError(_ => Some(errorMsg))
          }

          setLoading(_ => false)
        }
        loadChat()->ignore
      }
    }
    None
  }, [selectedChatId])

  let handleSubmit = e => {
    ReactEvent.Form.preventDefault(e)

    switch selectedChatId {
    | None => ()
    | Some(chatId) =>
      if String.trim(inputValue) != "" {
        let sendMessage = async () => {
          let result = await Api.postMessage(chatId, User, inputValue)
          switch result {
          | Ok() => {
              setInputValue(_ => "")

              // Reload the chat to show the new message
              let chatResult = await Api.fetchChat(chatId)
              switch chatResult {
              | Ok(updatedChat) => {
                  setChat(_ => Some(updatedChat))
                  
                  // Generate assistant response
                  let generateResult = await Api.generateMessage(chatId)
                  switch generateResult {
                  | Ok() => {
                      // Reload chat again to show generated message
                      let finalChatResult = await Api.fetchChat(chatId)
                      switch finalChatResult {
                      | Ok(finalChat) => setChat(_ => Some(finalChat))
                      | Error(_) => ()
                      }
                    }
                  | Error(errorMsg) => setError(_ => Some(`Failed to generate response: ${errorMsg}`))
                  }
                }
              | Error(_) => ()
              }
            }
          | Error(errorMsg) => setError(_ => Some(`Failed to send message: ${errorMsg}`))
          }
        }
        sendMessage()->ignore
      }
    }
  }

  <div className="flex flex-col h-[600px] border border-gray-300 rounded-md overflow-hidden">
    // Chat history
    <div className="flex-1 p-4 overflow-y-auto">
      {switch (selectedChatId, chat, loading, error) {
      | (None, _, _, _) =>
        <div className="flex items-center justify-center h-full text-gray-500">
          {React.string("Select a chat to view messages")}
        </div>

      | (Some(_), _, true, _) =>
        <div className="flex items-center justify-center h-full text-gray-500" />

      | (Some(_), _, false, Some(errorMsg)) =>
        <div className="flex items-center justify-center h-full text-red-500">
          {React.string(`Error: ${errorMsg}`)}
        </div>

      | (Some(_), Some(chatData), false, None) =>
        <div className="space-y-4">
          {if chatData.messages->Array.length > 0 {
            chatData.messages
            ->Array.mapWithIndex((message, index) => {
              let isUser = message.role == User
              <div
                key={index->Int.toString}
                className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
                <div
                  className={`max-w-[80%] p-3 rounded-lg ${isUser
                      ? "bg-blue-600 text-white"
                      : "bg-gray-200 text-gray-800"}`}>
                  {React.string(message.text)}
                </div>
              </div>
            })
            ->React.array
          } else {
            <div className="flex items-center justify-center h-full text-gray-500">
              {React.string("No messages")}
            </div>
          }}
        </div>
      | _ =>
        <div className="flex items-center justify-center h-full text-red-500">
          {React.string("Unknown error")}
        </div>
      }}
    </div>
    // Input area
    <form onSubmit={handleSubmit} className="border-t border-gray-300 p-3">
      <div className="flex items-center">
        <input
          type_="text"
          className="flex-1 p-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={selectedChatId->Option.isSome
            ? "Type a message..."
            : "Select a chat to start messaging"}
          value={inputValue}
          onChange={e => setInputValue(ReactEvent.Form.target(e)["value"])}
          disabled={selectedChatId->Option.isNone}
        />
        <Button type_="submit" className="ml-2" disabled={selectedChatId->Option.isNone}>
          {React.string("Send")}
        </Button>
      </div>
    </form>
  </div>
}

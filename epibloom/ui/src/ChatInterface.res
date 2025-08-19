@val external fetch: string => Promise.t<'response> = "fetch"
@send external json: 'response => Promise.t<JSON.t> = "json"

let fetchChat = async (chatId: string): result<ApiTypes.chat, string> => {
  try {
    let response = await fetch(`/api/chat/${chatId}`)
    let jsonData = await response->json
    let chatObj = jsonData->JSON.Decode.object->Option.getExn
    let messagesData = chatObj->Dict.get("messages")->Option.flatMap(JSON.Decode.array)->Option.getExn

    let messages = messagesData->Array.map(msgJson => {
      let msgObj = msgJson->JSON.Decode.object->Option.getExn
      {
        ApiTypes.role: msgObj->Dict.get("role")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        text: msgObj->Dict.get("text")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        created: msgObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        modified: msgObj->Dict.get("modified")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      }
    })

    let chat = {
      ApiTypes.id: chatObj->Dict.get("id")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      name: chatObj->Dict.get("name")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      created: chatObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      modified: chatObj->Dict.get("modified")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      messages: messages,
    }
    Ok(chat)
  } catch {
  | _ => Error("Failed to fetch chat")
  }
}

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
          let result = await fetchChat(chatId)
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
    // TODO: Implement sending messages to the selected chat
    if String.trim(inputValue) != "" {
      setInputValue(_ => "")
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
        <div className="flex items-center justify-center h-full text-gray-500">
          {React.string("Loading chat...")}
        </div>
      | (Some(_), _, false, Some(errorMsg)) =>
        <div className="flex items-center justify-center h-full text-red-500">
          {React.string(`Error: ${errorMsg}`)}
        </div>
      | (Some(_), Some(chatData), false, None) =>
        <div className="space-y-4">
          {chatData.messages->Array.mapWithIndex((message, index) => {
            let isUser = message.role == "user"
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
      | _ =>
        <div className="flex items-center justify-center h-full text-gray-500">
          {React.string("No messages")}
        </div>
      }}
    </div>

    // Input area
    <form onSubmit={handleSubmit} className="border-t border-gray-300 p-3">
      <div className="flex items-center">
        <input
          type_="text"
          className="flex-1 p-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={selectedChatId->Option.isSome ? "Type a message..." : "Select a chat to start messaging"}
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

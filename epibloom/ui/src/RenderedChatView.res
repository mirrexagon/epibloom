@react.component
let make = (~selectedChatId: option<string>) => {
  let (renderedChat, setRenderedChat) = React.useState(_ => None)
  let (loading, setLoading) = React.useState(_ => false)
  let (error, setError) = React.useState(_ => None)

  // TODO: Refresh the rendered chat after send, and later when changing anything that might change the rendered chat (eg. lorebooks)
  React.useEffect(() => {
    switch selectedChatId {
    | None => {
        setRenderedChat(_ => None)
        setError(_ => None)
      }

    | Some(chatId) => {
        let loadRenderedChat = async () => {
          setLoading(_ => true)
          setError(_ => None)

          let result = await Api.fetchRenderedChat(chatId)
          switch result {
          | Ok(chatData) => setRenderedChat(_ => Some(chatData))
          | Error(errorMsg) => setError(_ => Some(errorMsg))
          }

          setLoading(_ => false)
        }

        loadRenderedChat()->ignore
      }
    }

    None
  }, [selectedChatId])

  <div className="w-80 border-l border-gray-600 bg-gray-900 p-4">
    <div className="mb-4">
      <h3 className="text-lg font-bold text-white"> {React.string("Rendered Chat")} </h3>
    </div>
    <div className="h-full overflow-y-auto">
      {switch (selectedChatId, renderedChat, loading, error) {
      | (None, _, _, _) =>
        <div className="flex items-center justify-center h-32 text-gray-400 text-sm">
          {React.string("Select a chat to view rendered data")}
        </div>

      | (Some(_), _, true, _) =>
        <div className="flex items-center justify-center h-32 text-gray-400 text-sm">
          {React.string("Loading...")}
        </div>

      | (Some(_), _, false, Some(errorMsg)) =>
        <div className="text-red-400 text-sm"> {React.string(`Error: ${errorMsg}`)} </div>

      | (Some(_), Some(chatData), false, None) =>
        <div className="font-mono text-xs text-gray-300 space-y-3">
          {if chatData.messages->Array.length > 0 {
            chatData.messages
            ->Array.mapWithIndex((message, index) => {
              let roleColor = switch message.role {
              | User => "text-blue-400"
              | Assistant => "text-green-400"
              }
              let roleText = switch message.role {
              | User => "user"
              | Assistant => "assistant"
              }

              <div key={index->Int.toString} className="border border-gray-700 rounded p-2">
                <div className={`font-bold mb-1 ${roleColor}`}> {React.string(roleText)} </div>
                <div className="text-gray-200 whitespace-pre-wrap break-words">
                  {React.string(message.text)}
                </div>
              </div>
            })
            ->React.array
          } else {
            <div className="text-gray-400 text-center"> {React.string("No messages")} </div>
          }}
        </div>

      | _ => <div className="text-red-400 text-sm"> {React.string("Unknown error")} </div>
      }}
    </div>
  </div>
}

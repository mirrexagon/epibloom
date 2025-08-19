
@val external fetch: string => Promise.t<'response> = "fetch"
@send external json: 'response => Promise.t<JSON.t> = "json"

let fetchChats = async (): result<array<ApiTypes.chatMetadata>, string> => {
  try {
    let response = await fetch("/api/chat")
    let jsonData = await response->json
    let chatsData = jsonData->JSON.Decode.object->Option.flatMap(obj =>
      obj->Dict.get("chats")->Option.flatMap(JSON.Decode.array)
    )

    switch chatsData {
    | Some(chatsArray) =>
      let chats = chatsArray->Array.map(chatJson => {
        let chatObj = chatJson->JSON.Decode.object->Option.getExn
        {
          ApiTypes.id: chatObj->Dict.get("id")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          name: chatObj->Dict.get("name")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          created: chatObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          modified: chatObj->Dict.get("modified")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        }
      })
      Ok(chats)
    | None => Error("Failed to parse chats data")
    }
  } catch {
  | _ => Error("Failed to fetch chats")
  }
}

@react.component
let make = (~selectedChatId: option<string>, ~onChatSelect: string => unit) => {
  let (chats, setChats) = React.useState(() => [])
  let (loading, setLoading) = React.useState(() => true)
  let (error, setError) = React.useState(() => None)

  React.useEffect(() => {
    let loadChats = async () => {
      setLoading(_ => true)
      let result = await fetchChats()
      switch result {
      | Ok(chatsData) => {
          setChats(_ => chatsData)
          setError(_ => None)
        }
      | Error(errorMsg) => setError(_ => Some(errorMsg))
      }
      setLoading(_ => false)
    }

    loadChats()->ignore
    None
  }, [])

  <div className="w-64 bg-gray-50 border-r border-gray-200 p-4">
    <h2 className="text-lg font-semibold mb-4"> {React.string("Chats")} </h2>

    {loading ?
      <div className="text-gray-500"> {React.string("Loading...")} </div>
    : switch error {
      | Some(errorMsg) =>
        <div className="text-red-500"> {React.string(`Error: ${errorMsg}`)} </div>
      | None =>
        <div className="space-y-2">
          {chats->Array.map(chat => {
            let isSelected = selectedChatId->Option.mapOr(false, id => id == chat.id)
            <div
              key={chat.id}
              className={`p-3 rounded-lg border cursor-pointer transition-colors ${
                isSelected
                  ? "bg-blue-100 border-blue-300"
                  : "bg-white border-gray-200 hover:bg-gray-50"
              }`}
              onClick={_ => onChatSelect(chat.id)}
            >
              <div className="font-medium text-sm truncate">
                {React.string(chat.name)}
              </div>
              <div className="text-xs text-gray-500">
                {React.string(chat.created)}
              </div>
            </div>
          })->React.array}
        </div>
    }}
  </div>
}

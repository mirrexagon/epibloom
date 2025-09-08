@react.component
let make = (~selectedChatId: option<string>, ~onChatSelect: string => unit) => {
  let (chats, setChats) = React.useState(() => [])
  let (loading, setLoading) = React.useState(() => true)
  let (error, setError) = React.useState(() => None)

  React.useEffect(() => {
    let loadChats = async () => {
      setLoading(_ => true)
      let result = await Api.fetchChats()
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

  <div className="w-64 bg-gray-800 border-r border-gray-700 p-4">
    <h2 className="text-lg font-semibold mb-4 text-white"> {React.string("Chats")} </h2>
    {loading
      ? <div className="text-gray-400"> {React.string("Loading...")} </div>
      : switch error {
        | Some(errorMsg) =>
          <div className="text-red-400"> {React.string(`Error: ${errorMsg}`)} </div>
        | None =>
          <div className="space-y-2">
            {chats
            ->Array.map(chat => {
              let isSelected = selectedChatId->Option.mapOr(false, id => id == chat.id)
              <div
                key={chat.id}
                className={`p-3 rounded-lg border cursor-pointer transition-colors ${isSelected
                    ? "bg-blue-900 border-blue-600"
                    : "bg-gray-700 border-gray-600 hover:bg-gray-600"}`}
                onClick={_ => onChatSelect(chat.id)}>
                <div className="font-medium text-sm truncate text-white">
                  {React.string(chat.name)}
                </div>
                <div className="text-xs text-gray-400"> {React.string(chat.created)} </div>
              </div>
            })
            ->React.array}
          </div>
        }}
  </div>
}

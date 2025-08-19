type chatMetadata = {
  id: string,
  name: string,
  created: string,
  modified: string,
}

type chatMessage = {
  role: string,
  text: string,
  created: string,
  modified: string,
}

type chat = {
  id: string,
  name: string,
  created: string,
  modified: string,
  messages: array<chatMessage>,
}

type chatsResponse = {chats: array<chatMetadata>}

@val external fetch: string => Promise.t<'response> = "fetch"
@send external json: 'response => Promise.t<JSON.t> = "json"

let fetchChats = async (): result<array<chatMetadata>, string> => {
  try {
    let response = await fetch("/api/chat")
    let jsonData = await response->json
    let chatsData =
      jsonData
      ->JSON.Decode.object
      ->Option.flatMap(obj => obj->Dict.get("chats")->Option.flatMap(JSON.Decode.array))

    switch chatsData {
    | Some(chatsArray) =>
      let chats = chatsArray->Array.map(chatJson => {
        let chatObj = chatJson->JSON.Decode.object->Option.getExn

        {
          id: chatObj->Dict.get("id")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          name: chatObj->Dict.get("name")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          created: chatObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
          modified: chatObj
          ->Dict.get("modified")
          ->Option.flatMap(JSON.Decode.string)
          ->Option.getExn,
        }
      })
      Ok(chats)

    | None => Error("Failed to parse chats data")
    }
  } catch {
  | _ => Error("Failed to fetch chats")
  }
}

let fetchChat = async (chatId: string): result<chat, string> => {
  try {
    let response = await fetch(`/api/chat/${chatId}`)
    let jsonData = await response->json
    let chatObj = jsonData->JSON.Decode.object->Option.getExn
    let messagesData =
      chatObj->Dict.get("messages")->Option.flatMap(JSON.Decode.array)->Option.getExn

    let messages = messagesData->Array.map(msgJson => {
      let msgObj = msgJson->JSON.Decode.object->Option.getExn
      {
        role: msgObj->Dict.get("role")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        text: msgObj->Dict.get("text")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        created: msgObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
        modified: msgObj->Dict.get("modified")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      }
    })

    let chat = {
      id: chatObj->Dict.get("id")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      name: chatObj->Dict.get("name")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      created: chatObj->Dict.get("created")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      modified: chatObj->Dict.get("modified")->Option.flatMap(JSON.Decode.string)->Option.getExn,
      messages,
    }
    Ok(chat)
  } catch {
  | _ => Error("Failed to fetch chat")
  }
}

type role =
  | @as("user") User
  | @as("assistant") Assistant

type chatMetadata = {
  id: string,
  name: string,
  created: string,
  modified: string,
}

type chatMessage = {
  role: role,
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

let fetchChats = async (): result<array<chatMetadata>, string> => {
  try {
    let response = await Fetch.get("/api/chat")
    let jsonData = await response->Fetch.Response.json
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
    let response = await Fetch.get(`/api/chat/${chatId}`)
    let jsonData = await response->Fetch.Response.json
    let chatObj = jsonData->JSON.Decode.object->Option.getExn
    let messagesData =
      chatObj->Dict.get("messages")->Option.flatMap(JSON.Decode.array)->Option.getExn

    let messages = messagesData->Array.map(msgJson => {
      let msgObj = msgJson->JSON.Decode.object->Option.getExn

      let roleString = msgObj->Dict.get("role")->Option.flatMap(JSON.Decode.string)->Option.getExn
      let role = switch roleString {
      | "user" => User
      | "assistant" => Assistant
      | _ => raise(Failure(`Invalid role for message: ${roleString}`))
      }

      {
        role,
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

let postMessage = async (chatId: string, role: role, text: string): result<unit, string> => {
  try {
    let data = {
      "role": role,
      "text": text,
    }

    let _response = await Fetch.fetch(
      `/api/chat/${chatId}/message`,
      {method: #POST, body: data->Js.Json.stringifyAny->Option.getExn->Fetch.Body.string},
    )

    Ok()
  } catch {
  | _ => Error("Failed to post message")
  }
}

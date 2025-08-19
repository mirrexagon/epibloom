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

type chatMetadata = {
  id: string,
  name: string,
  created: string,
  modified: string,
}

type chatsResponse = {chats: array<chatMetadata>}

@react.component
let make: unit => Jsx.element
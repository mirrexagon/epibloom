type message = {
  text: string,
  sender: [#User | #AI]
}

@react.component
let make: unit => Jsx.element
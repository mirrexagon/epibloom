# Epibloom

A RPG simulator, using LLMs.

## Name

I discovered the word "epicalyx" through a random word picker. It seems to be an extra part of some flowers beneath the main bloom and main calyx (which I think are green pointy bits under the flower). It sounds cool, but the position of the epicalyx on the flower is more like a backend than a frontend. I would consider the petals to be the "frontend" of the flower in this analogy, but "epipetal" is still somewhat common when I do an internet search for it. "Epibloom" is probably not quite right, since the bloom is the whole flower, including the calyx, but I get much less existing content with that word, and I think it sounds better anyway.

## Design

Epibloom is made up of a Rust + Rocket backend and a ReScript + React frontend. The backend stores all data, chats, does all LLM API requests, etc., while the frontend is just the interface into that. As much as possible, the backend does everything functional, such that you can submit a chat message, watch it start to stream the response, then close your browser and the request continues in the background. When you reopen the UI, it continues right where it currently is, including when in the middle of streaming the response.

## TODO
- https://tailwindcss.com/docs/editor-setup#class-sorting-with-prettier

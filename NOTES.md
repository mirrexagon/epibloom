## Good things about SillyTavern
- World info is mostly good
- Support for many APIs, including model picker for hosted services
- Hiding messages from inclusion
- Connection profiles make managing all the different preset categories easier, but it seems somewhat like a patch
- Downloading cards from Chub, etc.

## Problems with SillyTavern
- Story string often mixes system prompt and other things
- Can't control summary/example message injection well, arbitrary injection would be nice - want to keep neat Markdown format
    -  Settings for introduction of examples are not in a preset (eg. with story string), not much control over how examples are sent (setting for whether to mark up with start/end sequences is part of one of the presets but didn't seem super coherent with that category)
    - Summary template item from summary extension doesn't work correctly with the story string (if statement doesn't see it, but text still gets inserted?)
- Writing JSON arrays etc. for eg. DRY sequence breakers, with no syntax highlighting
- Some settings that should cross domains are tricky to manage, they end up in just one, eg.
    - DRY sequence breakers, you probably want the special prompt format tokens in there automatically from the definition of the prompt format
    - Prompt format gets weirdly split across story string and instruct template
        - though perhaps the instruct template has system prompt markers, but does that only apply for the system prompt and not the entire story string?
    - When I have a setup such as the Wayfarer adventure, it's annoying to switch between prompt formats - as far as I know, I need to adjust the story string so that the entire story string is wrapped in the system prompt tags
- No syntax highlighting for eg. Jinja2 constructs
- In prompts, I want newlines between my Markdown headers and their bodies on both sides, and having an optional section is a bit tricky to get that right
- Persona locking to chats is awkward, I would like a persona field per chat to tailor it for each card - though now in ST personas can be linked to characters
    - Would be nice to have separate persona name and in-chat name, so I can categorize like "Mirage for Inos" and the in-chat name is still "Mirage"
        - Persona display names have since been implemented in ST
- When using System Prompt -> Miscellaneous -> Start Reply With, this gets applied to Chat Completion models even though the rest of the section isn't used
- It's unclear to me how summary is injected in Chat Completion prompt manager mode. With Pixijb, it is near the start, before most things.
- No progress or task completion indications when doing things like generating a prompt for image generation, or image generation itself
- No input bar history, see https://github.com/LenAnderson/SillyTavern-InputHistory
- See other features added by extensions, eg. Better continues and others from https://reddit.com/comments/1j7xzdh/comment/mh0oxis
- Chat-type LLM interaction is probably better done via chat completion.
- navigating away from the tab or browser on my phone closes connections - if the backend was fully responsible for things and the UI could always be restored to the current state, that would help a lot.

## Features
- Seeing raw LLM input before it is sent, live - useful for things like seeing what the summary is and making sure it is in there
- Templating
    - probably Handlebars, which is what ST uses
    - There is also Vento by the creator of Lume: https://vento.js.org
    - Or, don't use traditional templating for overall response building, build in order based on roles like chat completion, including for system prompt, etc.
    - Use Lua, see somewhere below
- Chat interface and flat interface - probably can't mix both but having both is nice, and converting from chat to flat might be good
- Swipes
- Separating reasoning from main part
- Multiple system prompt parts, eg. like for https://github.com/p-e-w/sorcery
- World info is good, possibly more control over newlines for Markdown formatting, etc.
- Markdown features
    - Structured document that gets turned into Markdown with nice formatting? Aware of header depth, world info can be nested correctly, etc.?
- Lots of syntax highlighting, eg.
    - Templating language, highlighting `{{char}}` and so on
    - In final LLM input preview, highlight which sections come from what, eg. system prompt, chat, world info
- NovelAI-style edit and author tracking?
- Load prompt format stuff based on model detected on API (by name or metadata)? Can ST do this somehow?
    - Maybe associate lots of things with the specific chat?
- Chat completion as well as text completion
- Seeing logits/token probabilities for each token, see top few tokens, how sampler selected (if possible)
- Binding template, prompt format, sampler settings, etc. to a chat, character, tagged in some way, etc.
- Being able to hover and highlight individual tokens in every text field that it makes sense in (may require API support to tokenize)
- Use chat completion wherever possible (including possibly for free text if using system prompt, etc.) to push chat tokenization to backend - build Epicalyx with this in mind
- MCP support, PoC with blender-mcp for example
- Image generation as tool as well as just asking for prompt from LLM
- Notes per chat, not sent to LLM
- Lore/lore book per chat, eg. for characters made up in that chat - ideally don't mix with other lore books but it can be useful, maybe UI to make creating lore books easier
- Fancy summarization, eg. https://reddit.com/comments/1jkckum
- Expose primitives to a scripting language which then assembles the prompt? Lua? Implement most features this way? Or templating language is good enough?
    - Reload scripts on disk as soon as they are changed
    - Expose scripts to UI editor, probably still store on disk, or in DB
    - Lua as templating language: https://nachtimwald.com/2014/08/06/using-lua-as-a-templating-engine/
    - https://github.com/mlua-rs/mlua/tree/v0.10?tab=readme-ov-file#serialization-serde-support
- Add tool calls for things like adding lorebook entries, adding notes and character changes to bios, summarizing, adjusting timeline, so Claude can add them when requested instead of me having to copy and paste and then delete the messages.
- If enabled/received, show thinking for actions like summarizing, generating images, etc. as well as normal messages
- Branching chats natively supported

## Other notes
- Use Aider/Claude Code to help make this
- If some features need support from the backend (eg. token probabilities, how sampler chose tokens), make a new backend based on llama.cpp called Epicalyx (see Epibloom README for why)

# RPG Ideas
Based on AI RPG.

Start by making a simpler text adventure game with locations and exits, inventory, items in locations, and NPCs in locations.

- Save prompts and other inputs used to generate and do things and allow them to be viewed in the UI (probably bring them up in the debug panel) - things like ComfyUI workflows and prompts used for images
- Tracking what the player should know/has found out, instead of just having NPCs have descriptions and knowing their names immediately.

# Inspirations
- Freely-editable markdown plain text as chat: https://github.com/rusiaaman/chat.md
- More complex templates/thinking message selection: https://huggingface.co/blog/qwen-3-chat-template-deep-dive

# Tools for development
- https://docs.rs/utoipa/latest/utoipa/index.html to export OpenAPI definitions for internal API - has Rocket support


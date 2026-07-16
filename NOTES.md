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
- Swipes
    - If not doing a full tree structure, at least be able to change swipes of any message in the history and generate a new one at any depth.
- Separating reasoning from main part
- Multiple system prompt parts, eg. like for https://github.com/p-e-w/sorcery
- World info is good, possibly more control over newlines for Markdown formatting, etc.
- Markdown features
    - Structured document that gets turned into Markdown with nice formatting? Aware of header depth, world info can be nested correctly, etc.?
    - World info can have folders of Markdown files for organization instead of just being flat
- Lots of syntax highlighting, eg.
    - Templating language, highlighting `{{char}}` and so on
    - In final LLM input preview, highlight which sections come from what, eg. system prompt, chat, world info
        - This will have to be some kind of other trace other than the raw API one
- NovelAI-style edit and author tracking?
- Associate lots of things with the specific chat - I seem to prefer each chat to have its own separate lorebook, etc. so that every chat can evolve independently
    - Binding template, prompt format, sampler settings, etc. to a chat, character, tagged in some way, etc.
- Use chat completion wherever possible (including possibly for free text if using system prompt, etc.) to push chat tokenization to backend - build Epicalyx with this in mind
- MCP support, PoC with blender-mcp for example
- Image generation as tool as well as just asking for prompt from LLM
- Notes per chat and per message, not sent to LLM
    - For each swipe/tree branch, be able to tag it in some way. Maybe just be able to tag messages, add notes, etc. eg. for which swipes I like and what about them I like.
- Fancy summarization, eg. https://reddit.com/comments/1jkckum
- Add tool calls for things like adding lorebook entries, adding notes and character changes to bios, summarizing, adjusting timeline, so Claude can add them when requested instead of me having to copy and paste and then delete the messages.
- If enabled/received, show thinking for actions like summarizing, generating images, etc. as well as normal messages
- Branching chats natively supported

## Other notes
Dedicated guy making own story/RP interface with anti slop https://reddit.com/comments/1sh9tz7/comment/ofb7mhp?context=3

# Chat

## Personas

Each scenario has characters in a lorebook like usual, but it would be great to be able to switch to playing any listed character for a new chat. There wouldn't really be personas, but instead just a note about which character the user is playing.

Think about whether character names should be in the messages as a prefix or not, see what my ST text scenario preset does

# RPG Ideas
Based on AI RPG.

Start by making a simpler text adventure game with locations and exits, inventory, items in locations, and NPCs in locations.

- Save prompts and other inputs used to generate and do things and allow them to be viewed in the UI (probably bring them up in the debug panel) - things like ComfyUI workflows and prompts used for images
- Tracking what the player should know/has found out, instead of just having NPCs have descriptions and knowing their names immediately.
    - https://www.reddit.com/r/aiRPGofficial/comments/1rjrdup/what_i_have_done_to_eliminate_spoilers_in_ai_rpg/
        - Implement things like this with Anthropic batching API, use batching for parallel things where possible
- Map: https://www.reddit.com/r/SillyTavernAI/comments/1uxstd6/a_ddstyle_interactive_map_for_visual_rp_with/

# Inspirations
- Freely-editable markdown plain text as chat: https://github.com/rusiaaman/chat.md
- More complex templates/thinking message selection: https://huggingface.co/blog/qwen-3-chat-template-deep-dive

# Scripting
- Make sure that scripts that are started and then the client disconnects are still run to completion. It doesn't matter if the response is not heard, as the client needs to be able to get the current state at any time anyway.

- Implement lorebooks with scripting, have lorebooks be folders and entries be Markdown files with YAML front matter for any metadata
- Implement normal chat, trees with scripting:
    - Streaming can be viewed in the trace panel, and each message can have a reference to the trace used to generate it. The UI can expose a button to open the trace associated with a message.
    - Cancel can be done by running a script to cancel the generation on another context.

- Lua as templating language: https://nachtimwald.com/2014/08/06/using-lua-as-a-templating-engine/

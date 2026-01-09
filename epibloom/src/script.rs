// TODO: See NOTES.md for ideas like templating, reloading scripts, etc.
//
// Use cases:
// - Press the button to submit a chat message, submit an action in the game, etc. which runs a script that does things like add messages to the chat and starts generation, etc. Needs to be able to return things for the UI to view? Updates the game state which is reflected in the UI?
// - Defining the UI in scripts? As in, behaviour of buttons should be scriptable but should game UI/the API also be as flexible? eg. scripting inventory system instead of having it written in Rust.
//
// Scripts can go in global and local config locations, eg. /etc/epibloom/scripts and ~/.config/epibloom/scripts, and the user one is higher in package.path
//
// There is a POST API endpoint /api/script/<script name> which looks up scripts, passes the body JSON as Lua values to the script, runs it.
// - How do results come back? Via a global event endpoint that streams events? (should this endpoint also just send the current state of the application as the first thing, like chat streams do?)
// - Each script in its own Lua state or all use the same one? If same one, then can't run scripts in background waiting for things unless using some coroutine async thing - I want to try coroutine and async (mlua has support) where possible.
//
// Chat objects in Lua wrap a Uuid and internally the methods call back to the chat store.

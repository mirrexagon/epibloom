--- @meta

--- Reading from and writing to the scripts data directory.
--- @class filesystemlib
local filesystem = {}

--- List files matching a glob pattern, relative to the scripts data directory.
--- @param pattern string # Glob pattern to search for.
--- @return string[] # List of file paths that match the pattern.
function filesystem.files(pattern) end

--- Save a Lua value to a JSON file. Raises an error if this fails.
--- @param file_path string # The path to save the file to, relative to the script data directory. The extension will be forced to `.json`.
--- @param content any # A Lua value to save as JSON.
function filesystem.save_json(file_path, content) end

--- Load the contents of a JSON file as a Lua value. Raises an error if this fails.
--- @param file_path string # The path to the file to load from, relative to the script data directory. The extension will be forced to `.json`.
--- @return any # The loaded value.
function filesystem.load_json(file_path) end

return filesystem

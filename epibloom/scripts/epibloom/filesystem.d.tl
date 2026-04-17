--- Reading from and writing to the scripts data directory.
local record filesystem
    --- List files matching a glob pattern, relative to the scripts data directory.
    files: function(pattern: string): {string}

    --- Save a Lua value to a JSON file. Raises an error if this fails.
    --- The path to save the file to is relative to the script data directory.
    --- The extension will be forced to `.json`.
    save_json: function(file_path: string, content: any)

    --- Load the contents of a JSON file as a Lua value. Raises an error if this fails.
    --- The path to the file to load from is relative to the script data directory.
    --- The extension will be forced to `.json`.
    load_json: function(file_path: string): any
end

return filesystem

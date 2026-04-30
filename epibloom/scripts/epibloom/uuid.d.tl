local record uuid
    --- A UUID, represented as a hyphenated lowercase hex string.
    type Uuid = string

    --- Generate a new UUIDv7 based on the current time.
    new: function(): Uuid
end

return uuid

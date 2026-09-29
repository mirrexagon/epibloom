--- @meta

--- @alias uuid string # A UUID in hyphenated string form, eg. `01890a5d-ac96-774b-bcce-b302099a8057`.

--- Generate UUIDs.
--- @class EpibloomUuidModule
local uuid = {}

--- Generate a new UUIDv7 based on the current time.
--- @return uuid
--- @nodiscard
function uuid.now_v7() end

return uuid

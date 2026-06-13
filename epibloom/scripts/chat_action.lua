local arg = ...

assert(type(arg) == "table", "expected object, got " .. type(arg))
assert(type(arg.action), "field 'action' missing")

if arg.action == "new_chat" then
end

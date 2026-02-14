# State Objects

## Example Usage

### Example 1: Simple Chat

**Script: `scripts/chat/send-message.lua`**
```lua
local state = require("epibloom.state")
local trace = require("epibloom.trace")

local chat_id = input.chat_id
local user_message = input.message

-- Get messages state object
local messages = state.get("chat:" .. chat_id .. ":messages")

-- Add user message
messages:update(function(msgs)
    msgs = msgs or {}
    table.insert(msgs, {
        role = "user",
        content = user_message,
        timestamp = os.time()
    })
    return msgs
end)

-- Generate assistant response
local t = trace.new()
t[1] = {role = "user", content = user_message}
t:generate("anthropic", {model = "claude-3-5-sonnet-20241022"})
t:wait_for_generate()

-- Add assistant message
messages:update(function(msgs)
    table.insert(msgs, {
        role = "assistant",
        content = t[2].content,
        trace_id = t.id,
        timestamp = os.time()
    })
    return msgs
end)

return {success = true}
```

**Client usage:**
```javascript
// Subscribe to chat updates
const eventSource = new EventSource('/api/state?objects=chat:123:messages');

eventSource.addEventListener('message', (e) => {
    const event = JSON.parse(e.data);

    if (event.type === 'snapshot') {
        // Initial state
        displayMessages(event.value);
    } else if (event.type === 'update') {
        // State changed
        displayMessages(event.value);
    }
});

// Send message
fetch('/api/script/alice/chat/send-message', {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body: JSON.stringify({
        chat_id: '123',
        message: 'Hello!'
    })
});
```

### Example 2: Chat Tree with Concurrent Generation

**Different branches use different state objects:**

```lua
-- Branch 4 messages
local branch4 = state.get("chat:123:branch:4:messages")

-- Branch 6 messages
local branch6 = state.get("chat:123:branch:6:messages")

-- Can be modified concurrently by different script contexts
```

**Tree structure itself is also a state object:**
```lua
local tree = state.get("chat:123:tree")

tree:update(function(t)
    t = t or {nodes = {}}
    table.insert(t.nodes, {
        id = 7,
        parent = 4,
        children = {}
    })
    return t
end)
```

**Client subscribes to multiple objects:**
```
GET /api/state?objects=chat:123:tree,chat:123:branch:4:messages,chat:123:branch:6:messages
```

### Example 3: Game State

**Player stats, inventory, and quest log as separate objects:**

```lua
local stats = state.get("player:456:stats")
local inventory = state.get("player:456:inventory")
local quests = state.get("player:456:quests")

-- Take damage
stats:update(function(s)
    s.health = math.max(0, s.health - 10)
    return s
end)

-- Add item
inventory:update(function(inv)
    table.insert(inv, {item = "health_potion", count = 1})
    return inv
end)
```

**Client gets all player state in one stream:**
```
GET /api/state?objects=player:456:stats,player:456:inventory,player:456:quests
```


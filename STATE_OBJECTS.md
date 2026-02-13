# State Objects Implementation Plan

## Background

Epibloom is a system for working with LLM conversations through **traces** (conversation history with streaming token generation) and **Lua scripts** (which orchestrate interactions with traces and application state).

Current architecture:
- **Traces** (`epibloom/src/trace.rs`) - LLM conversation sessions with SSE streaming of message deltas
- **TraceStore** - Manages traces, provides SSE streams for clients to subscribe and get current state + future updates
- **Lua scripts** (`epibloom/src/script.rs`) - Run in isolated contexts, can create/manipulate traces
- **Script contexts** - Each context has its own Lua state, scripts queue and run sequentially per context
- Scripts are invoked via REST API: `POST /api/script/<context>/<script-path>` with JSON input, returns JSON output

### The Problem

We want to build **chat** functionality (and eventually a text adventure game) on top of this system. Key requirements:

1. **Client reentrancy** - Clients can disconnect and rejoin at any time, getting current state + future updates
2. **Concurrent operations** - Multiple clients can trigger scripts that modify shared state (e.g., generating messages on different branches of a chat tree)
3. **Real-time updates** - All connected clients see state changes as they happen
4. **Flexible state structure** - Support chat messages, game state, player stats, etc.
5. **Minimal Rust abstractions** - Keep domain logic in Lua scripts, not Rust

Traces solve #1 for LLM streaming, but we need a general solution for application state (chat history, game state, etc.).

### Design Decision: State Objects

Add a new generic primitive: **StateObject** - a named, lockable JSON value that broadcasts changes to subscribed clients.

Key properties:
- Each StateObject has a **unique name** (e.g., `"chat:123:messages"`)
- Contains a **single JSON value** (can be object, array, primitive)
- Can be **locked by scripts** for atomic read-modify-write operations
- **Broadcasts full value** to subscribers whenever lock is released
- Clients can **subscribe to multiple state objects** in one SSE stream
- Get **snapshot of current values + stream of future updates**

This enables:
- Chat messages as `["msg1", "msg2", ...]` in state object `"chat:123:messages"`
- Player stats as `{health: 100, inventory: [...]}` in state object `"player:456:stats"`
- Concurrent generation on different chat branches using different state object names
- Scripts orchestrate everything, state objects just provide storage + notification

### Future Enhancement: JSON Diffs

Current plan: broadcast **full value** on every change (simple, no diff computation).

Future optimization: broadcast **JSON Patch (RFC 6902) diffs** instead of full values for large objects. This would reduce bandwidth for big state objects with small changes.

Implementation would:
1. Store previous value alongside current value
2. Compute JSON Patch on lock release
3. Send diff events instead of full values
4. Clients apply patches to their local copy

Not implementing now to keep initial version simple.

## Implementation Plan

### 1. Create `epibloom/src/state.rs`

New module for state object functionality.

#### Data Structures

```rust
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use tokio::sync::{broadcast, RwLock};
use time::OffsetDateTime;
use utoipa::ToSchema;

/// Manages all state objects in the system.
pub struct StateStore {
    objects: RwLock<HashMap<String, Arc<StateObject>>>,
}

impl StateStore {
    pub fn new() -> Self {
        Self {
            objects: RwLock::new(HashMap::new()),
        }
    }
    
    /// Get or create a state object by name.
    pub async fn get_or_create(&self, name: &str) -> Arc<StateObject> {
        let mut objects = self.objects.write().await;
        
        if let Some(obj) = objects.get(name) {
            Arc::clone(obj)
        } else {
            let new_obj = Arc::new(StateObject::new(name));
            objects.insert(name.to_string(), Arc::clone(&new_obj));
            new_obj
        }
    }
    
    /// Check if a state object exists.
    pub async fn has(&self, name: &str) -> bool {
        let objects = self.objects.read().await;
        objects.contains_key(name)
    }
}

/// A single state object with a JSON value.
pub struct StateObject {
    name: String,
    
    /// The current JSON value.
    value: RwLock<serde_json::Value>,
    
    /// Lock information (None = unlocked).
    lock: RwLock<Option<LockInfo>>,
    
    /// Broadcast channel for state changes.
    event_broadcast: broadcast::Sender<StateEvent>,
}

impl StateObject {
    fn new(name: &str) -> Self {
        let (broadcast, _) = broadcast::channel(128);
        
        Self {
            name: name.to_string(),
            value: RwLock::new(serde_json::Value::Null),
            lock: RwLock::new(None),
            event_broadcast: broadcast,
        }
    }
    
    /// Read the current value (does not require lock).
    pub async fn read(&self) -> serde_json::Value {
        self.value.read().await.clone()
    }
    
    /// Try to acquire the lock. Returns true if successful.
    pub async fn try_lock(&self, holder: &str) -> bool {
        let mut lock = self.lock.write().await;
        
        if lock.is_some() {
            false
        } else {
            *lock = Some(LockInfo {
                holder: holder.to_string(),
                acquired: OffsetDateTime::now_utc(),
            });
            true
        }
    }
    
    /// Write a new value. Must hold the lock.
    pub async fn write(&self, holder: &str, new_value: serde_json::Value) -> Result<(), Error> {
        {
            let lock = self.lock.read().await;
            match lock.as_ref() {
                Some(info) if info.holder == holder => {},
                Some(_) => return Err(Error::NotLockHolder),
                None => return Err(Error::NotLocked),
            }
        }
        
        let mut value = self.value.write().await;
        *value = new_value;
        
        Ok(())
    }
    
    /// Release the lock and broadcast the current value to all subscribers.
    pub async fn unlock(&self, holder: &str) -> Result<(), Error> {
        let current_value = {
            let mut lock = self.lock.write().await;
            
            match lock.as_ref() {
                Some(info) if info.holder == holder => {},
                Some(_) => return Err(Error::NotLockHolder),
                None => return Err(Error::NotLocked),
            }
            
            *lock = None;
            
            self.value.read().await.clone()
        };
        
        // Broadcast the new value
        let _ = self.event_broadcast.send(StateEvent::Update {
            object: self.name.clone(),
            value: current_value,
        });
        
        Ok(())
    }
    
    /// Check if currently locked.
    pub async fn is_locked(&self) -> bool {
        self.lock.read().await.is_some()
    }
    
    /// Subscribe to state changes. Returns current value + stream of future updates.
    pub async fn subscribe(&self) -> (serde_json::Value, impl futures::Stream<Item = StateEvent>) {
        use async_stream::stream;
        
        let current_value = self.read().await;
        let mut rx = self.event_broadcast.subscribe();
        
        let event_stream = stream! {
            use tokio::sync::broadcast::error::RecvError;
            
            loop {
                match rx.recv().await {
                    Ok(event) => yield event,
                    Err(RecvError::Lagged(n)) => {
                        log::error!("State object '{}' stream lagged {} messages", self.name, n);
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        };
        
        (current_value, event_stream)
    }
}

struct LockInfo {
    holder: String,
    acquired: OffsetDateTime,
}

/// Event sent to subscribers when state changes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateEvent {
    /// Initial snapshot of current state.
    Snapshot {
        object: String,
        value: serde_json::Value,
    },
    
    /// State was updated (sent when lock is released).
    Update {
        object: String,
        value: serde_json::Value,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("State object is not locked")]
    NotLocked,
    
    #[error("Caller does not hold the lock")]
    NotLockHolder,
}
```

### 2. Add Lua Bindings in `epibloom/src/script.rs`

Add a new module to the Lua environment for state objects.

```rust
// In Context::new(), after other modules:

// State module
{
    let state_module = lua.create_table()?;
    
    state_module.set(
        "get",
        lua.create_async_function(|lua, name: String| async move {
            let state_store: Arc<StateStore> = lua.named_registry_value("epibloom.state_store")?;
            let state_obj = state_store.get_or_create(&name).await;
            
            Ok(LuaStateObject {
                name: name.clone(),
                state_object: state_obj,
            })
        })?,
    )?;
    
    let package_loaded: Table = globals.get_path("package.loaded")?;
    package_loaded.set("epibloom.state", state_module)?;
}

// Store reference to StateStore in registry
lua.set_named_registry_value("epibloom.state_store", Arc::clone(&state_store))?;
```

Add UserData implementation for state objects:

```rust
struct LuaStateObject {
    name: String,
    state_object: Arc<StateObject>,
}

impl UserData for LuaStateObject {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        // Read current value (no lock required)
        methods.add_async_method("read", |lua, obj, ()| async move {
            let value = obj.state_object.read().await;
            lua.to_value(&value)
        });
        
        // Try to acquire lock
        methods.add_async_method("lock", |lua, obj, ()| async move {
            let context_name: String = lua.named_registry_value("epibloom.context.name")?;
            let holder = format!("context:{}", context_name);
            
            let success = obj.state_object.try_lock(&holder).await;
            Ok(success)
        });
        
        // Write new value (must hold lock)
        methods.add_async_method("write", |lua, obj, value: Value| async move {
            let context_name: String = lua.named_registry_value("epibloom.context.name")?;
            let holder = format!("context:{}", context_name);
            
            let json_value: serde_json::Value = lua.from_value(value)?;
            obj.state_object.write(&holder, json_value).await
                .map_err(mlua::Error::external)?;
            
            Ok(())
        });
        
        // Release lock and broadcast
        methods.add_async_method("unlock", |lua, obj, ()| async move {
            let context_name: String = lua.named_registry_value("epibloom.context.name")?;
            let holder = format!("context:{}", context_name);
            
            obj.state_object.unlock(&holder).await
                .map_err(mlua::Error::external)?;
            
            Ok(())
        });
        
        // Check if locked
        methods.add_async_method("is_locked", |_lua, obj, ()| async move {
            Ok(obj.state_object.is_locked().await)
        });
        
        // Atomic read-modify-write helper
        methods.add_async_method("update", |lua, obj, func: mlua::Function| async move {
            let context_name: String = lua.named_registry_value("epibloom.context.name")?;
            let holder = format!("context:{}", context_name);
            
            // Acquire lock
            if !obj.state_object.try_lock(&holder).await {
                return Err(mlua::Error::runtime("Failed to acquire lock"));
            }
            
            // Read current value
            let current_value = obj.state_object.read().await;
            let lua_value = lua.to_value(&current_value)?;
            
            // Call function to modify value
            let new_value: Value = func.call_async(lua_value).await?;
            let json_value: serde_json::Value = lua.from_value(new_value)?;
            
            // Write new value
            obj.state_object.write(&holder, json_value).await
                .map_err(mlua::Error::external)?;
            
            // Release lock (broadcasts automatically)
            obj.state_object.unlock(&holder).await
                .map_err(mlua::Error::external)?;
            
            Ok(())
        });
    }
}
```

**Lua API exposed:**

```lua
local state = require("epibloom.state")

-- Get or create state object
local obj = state.get("chat:123:messages")

-- Read current value (no lock needed)
local value = obj:read()

-- Manual lock/write/unlock
if obj:lock() then
    local value = obj:read()
    -- ... modify value ...
    obj:write(new_value)
    obj:unlock()  -- Broadcasts to all subscribers
else
    -- Object is locked by another script
end

-- Check if locked
if obj:is_locked() then
    -- ...
end

-- Atomic update helper
obj:update(function(current_value)
    -- Modify and return new value
    table.insert(current_value, "new item")
    return current_value
end)
```

### 3. Add HTTP API in `epibloom/src/api/state.rs`

New API module for subscribing to state object updates.

```rust
use std::sync::Arc;
use futures::StreamExt;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    State,
};
use utoipa::OpenApi;

use crate::state::{StateStore, StateEvent};
use super::ApiErrorResponse;

#[derive(OpenApi)]
#[openapi(paths(stream_state_objects))]
pub struct StateApiDoc;

/// Subscribe to one or more state objects.
/// Returns SSE stream with snapshots of current values, then updates as they change.
///
/// Example: GET /api/state?objects=chat:123:messages,player:456:stats
#[utoipa::path(
    responses(
        (status = 200, description = "SSE stream of state snapshots and updates", body = StateEvent),
        (status = 500, description = "Error starting stream", body = String)
    ),
    params(
        ("objects" = String, Query, description = "Comma-separated list of state object names")
    )
)]
#[get("/?<objects>")]
pub async fn stream_state_objects(
    objects: String,
    store: &State<Arc<StateStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let object_names: Vec<&str> = objects.split(',').collect();
    
    // Get or create all requested state objects and subscribe
    let mut subscriptions = Vec::new();
    for name in &object_names {
        let obj = store.get_or_create(name).await;
        let (current_value, stream) = obj.subscribe().await;
        
        subscriptions.push((
            name.to_string(),
            current_value,
            stream,
        ));
    }
    
    Ok(EventStream! {
        // Send snapshots first
        for (name, value, _) in &subscriptions {
            yield Event::json(&StateEvent::Snapshot {
                object: name.clone(),
                value: value.clone(),
            });
        }
        
        // Merge all streams and forward updates
        let streams: Vec<_> = subscriptions.into_iter()
            .map(|(_, _, stream)| stream)
            .collect();
        
        let mut merged = futures::stream::select_all(streams);
        
        while let Some(event) = merged.next().await {
            yield Event::json(&event);
        }
    })
}
```

### 4. Wire Everything Together in `epibloom/src/main.rs`

Add state module and API:

```rust
mod state;
// ... existing mods ...

use state::StateStore;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing setup ...
    
    // Create state store
    let state_store = Arc::new(StateStore::new());
    
    // ... existing rocket setup ...
    
    rocket::build()
        // ... existing config ...
        .manage(state_store)
        .mount("/api/state", routes![
            api::state::stream_state_objects
        ])
        // ... existing mounts ...
}
```

Pass state_store to script contexts:

```rust
// In Context::new() signature:
pub fn new(name: &str, trace_store: Arc<TraceStore>, state_store: Arc<StateStore>) -> Result<Self, Error>

// Store in registry:
lua.set_named_registry_value("epibloom.state_store", Arc::clone(&state_store))?;
```

Update ContextStore::get_context() to pass state_store:

```rust
let new_context = Arc::new(Context::new(
    name,
    Arc::clone(&self.trace_store),
    Arc::clone(&self.state_store),  // Add this
)?);
```

### 5. Update OpenAPI Documentation

In `epibloom/src/api.rs`:

```rust
#[derive(OpenApi)]
#[openapi(
    nest(
        (path = "/api/state", api = state::StateApiDoc),
        // ... existing nests ...
    ),
    components(
        schemas(state::StateEvent),
        // ... existing schemas ...
    )
)]
pub struct ApiDoc;
```

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

## Testing Plan

1. **Unit tests** for StateObject locking logic
2. **Integration tests** for Lua API (lock/write/unlock)
3. **Manual testing** with HTTPie:
   ```bash
   # Subscribe to state
   http --stream :8000/api/state objects==chat:123:messages
   
   # In another terminal, run script to modify state
   http POST :8000/api/script/main/chat/send-message chat_id=123 message="Hello"
   
   # First terminal should show update event
   ```
4. **Frontend test** with simple chat UI

## Future Enhancements

### JSON Patch Diffs

When state objects grow large, sending full values on every change wastes bandwidth.

**Enhancement plan:**
1. Store previous value alongside current value in StateObject
2. When unlocking, compute JSON Patch (RFC 6902) between old and new values
3. Send diff event instead of full value:
   ```rust
   pub enum StateEvent {
       Snapshot { object: String, value: serde_json::Value },
       Update { object: String, value: serde_json::Value },
       Patch { object: String, patch: Vec<PatchOperation> },  // New
   }
   ```
4. Clients apply patches using a JSON Patch library

**Libraries:**
- Rust: `json-patch` crate
- JavaScript: `fast-json-patch`

**When to use:**
- Heuristic: if `patch_size < 0.5 * full_value_size`, send patch; else send full value
- Or: always send patch, clients can handle it

This is a nice-to-have optimization, not needed for initial implementation.

### State Persistence

State objects currently exist only in memory. For production:

1. Auto-persist to disk (like traces do)
2. Store as `<STATE_DATA_DIR>/<object-name>.json`
3. Load all state objects on startup
4. Debounce writes (e.g., persist 1 second after unlock)

### Lock Timeouts

Add automatic lock release after timeout to prevent deadlocks:

```rust
pub struct LockInfo {
    holder: String,
    acquired: OffsetDateTime,
    timeout: Duration,  // New
}

// Background task that checks for expired locks
async fn release_expired_locks(state_store: Arc<StateStore>) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        // Check all locks, release expired ones
    }
}
```

### Conditional Updates

Allow optimistic locking with version checks:

```lua
local version = obj:version()
-- ... compute new value ...
obj:update_if_version(new_value, version)  -- Fails if version changed
```

This enables more concurrent scripts without explicit locking.

## Questions / Design Decisions

- **Lock timeout duration?** Default to 30 seconds? Configurable per object?
- **Persistence strategy?** Auto-persist like traces, or explicit save/load?
- **Object naming conventions?** Enforce patterns like `<type>:<id>:<field>`?
- **Initial value for new objects?** Default to `null`, `{}`, or `[]`?
  - Recommendation: `null`, scripts decide structure

## Summary

This design adds a minimal, flexible primitive (StateObject) that solves the client reentrancy problem while keeping all domain logic in Lua scripts. It naturally extends to support chat, text adventures, and any other stateful application.

The implementation is:
- **~400 lines** of Rust code (state.rs + bindings + API)
- **Generic** - no chat or game-specific concepts
- **Consistent** - mirrors the trace/session pattern already in Epibloom
- **Extensible** - JSON diffs, persistence, etc. can be added later

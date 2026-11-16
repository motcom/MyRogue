-- Lua callbacks only; Rust tests cover input, visibility, occupancy and rollback.
local door = assert(loadfile("data/entity_templates/static/wooden_door.lua"))()
local chest = assert(loadfile("data/entity_templates/item/treasure_chest.lua"))()
local count = 0
local function check(value)
    assert(value)
    count = count + 1
end
local function allowed(definition, state)
    local result = {}
    for _, action in ipairs(definition.interactions) do
        local enabled = true
        for key, expected in pairs(action.when_state or {}) do
            if state[key] ~= expected then enabled = false end
        end
        if enabled then table.insert(result, action.id) end
    end
    return result
end
local function effect(result, name)
    local matches = {}
    for _, item in ipairs(result.effects or {}) do
        if item.type == name then table.insert(matches, item) end
    end
    return matches
end
local instance = { id = 10, state = door.initial_state, config = door }
check(allowed(door, instance.state)[1] == "open")
local out = door.hooks.on_interact(instance, {}, { interaction = "open", user = 1 })
check(out.state.opened == true)
check(effect(out, "set_collision")[1].movement == false)
check(effect(out, "set_collision")[1].sight == false)
check(effect(out, "set_glyph")[1].glyph == "/")
instance.state = out.state
check(allowed(door, instance.state)[1] == "close")
out = door.hooks.on_interact(instance, {}, { interaction = "close", user = 1 })
check(out.state.opened == false)
check(effect(out, "set_collision")[1].movement == true)
check(effect(out, "set_collision")[1].sight == true)
check(effect(out, "set_glyph")[1].glyph == "+")
check(next(door.hooks.on_interact(instance, {}, { interaction = "invalid" })) == nil)
instance = { id = 11, state = chest.initial_state, config = chest }
out = chest.hooks.on_interact(instance, {}, { interaction = "open", user = 1 })
check(out.state.opened)
check(#effect(out, "spawn") == 0)
check(effect(out, "open_container")[1].target == instance.id)
check(#chest.item.contents == 3)
instance.state = out.state
out = chest.hooks.on_interact(instance, {}, { interaction = "contents", user = 1 })
check(effect(out, "open_container")[1].target == instance.id)
check(out.state == nil)
out = chest.hooks.on_interact(instance, {}, { interaction = "close", user = 1 })
check(not out.state.opened)
instance.state = out.state
out = chest.hooks.on_interact(instance, {}, { interaction = "open", user = 1 })
check(out.state.opened)
check(#effect(out, "spawn") == 0)
check(chest.category == "item" and chest.item.stack_limit == 1)
print("OK: " .. count .. " Lua interaction checks")

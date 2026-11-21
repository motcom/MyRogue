-- Lua 単体の形式・定義間参照の検査。ゲームのローダーではない。
local files = { ... }
local validation = require("tools.entity_validation")
if #files == 0 then
    files = assert(loadfile("data/entity_templates/manifest.lua"))()
end
local function integer(n, min)
    return type(n) == "number" and n == math.floor(n) and n < math.huge and n >= min
end
local function damage(value)
    assert(type(value) == "table", "damage must use schema 2 dice format")
    assert(integer(value.dice, 1) and value.dice <= 20)
    assert(integer(value.sides, 1) and value.sides <= 100)
    assert(integer(value.bonus, -1000) and value.bonus <= 1000)
end
local function combat(value)
    assert(type(value) == "table" and integer(value.base_av, 0) and value.base_av <= 100)
    for _, key in ipairs({"dv_bonus", "accuracy_bonus"}) do
        assert(value[key] == nil or (integer(value[key], -100) and value[key] <= 100))
    end
end
local function one_of(value, values)
    for _, v in ipairs(values) do if value == v then return true end end
    return false
end
local function strings(values, allowed)
    assert(type(values) == "table", "array expected")
    for _, value in ipairs(values) do
        assert(type(value) == "string", "string expected")
        if allowed then assert(one_of(value, allowed), "unknown value: " .. value) end
    end
end
local function amounts(values)
    assert(type(values) == "table")
    for name, amount in pairs(values) do
        assert(one_of(name, { "fuel", "electricity", "welding_material" }), "unknown resource")
        assert(integer(amount, 0), "resource amount must be a nonnegative integer")
    end
end
local function rgb(values)
    assert(type(values) == "table" and #values == 3)
    for _, n in ipairs(values) do assert(integer(n, 0) and n <= 255, "invalid RGB") end
end
local function serializable(value, seen)
    local t = type(value)
    if t == "table" then
        assert(not seen[value], "cyclic state")
        seen[value] = true
        for key, item in pairs(value) do
            assert(type(key) == "string" or integer(key, 1), "invalid state key")
            serializable(item, seen)
        end
        seen[value] = nil
    else
        assert(t == "boolean" or t == "string" or
            (t == "number" and value == value and math.abs(value) < math.huge), "invalid state value")
    end
end
local ids = {}
for _, path in ipairs(files) do
    local config = assert(loadfile(path))()
    assert(type(config) == "table" and config.schema_version == 2, "invalid schema")
    assert(type(config.id) == "string" and #config.id > 0 and not ids[config.id], "invalid/duplicate id")
    ids[config.id] = config
    assert(one_of(config.category, { "movable", "static", "map", "item" }), "unknown category")
    local common = assert(config.common)
    assert(type(common.name) == "string" and #common.name > 0)
    assert(common.description == nil or type(common.description) == "string", "invalid description")
    local appearance = assert(common.appearance)
    assert(type(appearance.glyph) == "string" and #appearance.glyph == 1 and
        appearance.glyph:byte() >= 32 and appearance.glyph:byte() <= 126, "ASCII glyph expected")
    rgb(appearance.fg); rgb(appearance.bg)
    assert(integer(appearance.layer, 0))
    assert(type(common.collision.movement) == "boolean" and type(common.collision.sight) == "boolean")
    assert(common.durability == false or
        (type(common.durability) == "table" and integer(common.durability.max, 1)), "invalid durability")
    strings(common.tags)
    assert(type(config.initial_state) == "table")
    serializable(config.initial_state, {})
    for _, category in ipairs({ "movable", "static", "map", "item" }) do
        if category == config.category then
            assert(type(config[category]) == "table", "missing category data")
        else
            assert(config[category] == nil, "multiple category tables")
        end
    end
    if config.category == "movable" then
        local m = config.movable
        assert(integer(m.tier or 1, 1) and (m.tier or 1) <= 10, "invalid movable tier")
        assert(one_of(m.role, { "monster", "animal", "npc", "player" }))
        assert(one_of(m.controller, { "lua", "input" }))
        assert(m.dominant_hand == nil or one_of(m.dominant_hand,{"right","left"}))
        if m.hostile_factions then strings(m.hostile_factions) end
        assert(type(m.faction) == "string" and #m.faction > 0)
        assert(common.durability == false)
        assert(type(m.body) == "table" and next(m.body))
        assert(integer(m.hp, 1), "positive whole HP required")
        assert(#m.body == 6, "six body parts required")
        local parts = {}
        for _, part in ipairs(m.body) do
            assert(one_of(part, {"head", "body", "left_arm", "right_arm", "left_leg", "right_leg"}) and not parts[part])
            parts[part] = true
        end
        for _, name in ipairs({ "kinryoku", "subayasa", "kiyousa", "seishinryoku" }) do
            assert(integer(m.attributes[name], 0), "missing attribute: " .. name)
        end
        for key in pairs(m.attributes) do assert(one_of(key,{"kinryoku","subayasa","kiyousa","seishinryoku"}), "unknown attribute") end
        assert(m.skills.syakou == nil, "social skill removed")
        assert(type(m.skills) == "table")
        for name, level in pairs(m.skills) do assert(type(name) == "string" and integer(level, 0)) end
        for _, name in ipairs({ "max_length_mm", "max_volume_ml", "max_mass_g" }) do
            assert(integer(m.inventory[name], 0))
        end
        assert(integer(m.senses.sight_range, 0) and m.senses.sight_range <= 100)
        damage(m.attack.damage)
        combat(m.combat)
        assert(integer(m.attack.base_pv, 0) and m.attack.base_pv <= 30)
        assert(integer(m.attack.strength_cap, 0) and m.attack.strength_cap <= 30)
        assert(integer(m.sanity, 1) and m.sanity <= 1000000)
        assert(type(m.psykick) == "table" and type(m.status) == "table")
        assert(integer(m.attack.range, 1))
        assert(type(m.loadout) == "table" and type(m.needs) == "table")
        for _, entry in ipairs(m.loadout) do
            assert(type(entry.definition_id) == "string" and integer(entry.quantity, 1) and entry.quantity <= 1000)
            assert(entry.equipped == nil or type(entry.equipped) == "boolean")
            assert(entry.grip == nil or one_of(entry.grip,{"right","left","both"}))
            if entry.equipped or entry.grip then assert(entry.quantity == 1) end
        end
        for name in pairs(m.needs) do assert(name=="hydration", "unknown need") end
        for _, name in ipairs({ "hydration" }) do
            local need = assert(m.needs[name])
            assert(integer(need.max, 1) and integer(need.initial, 0) and need.initial <= need.max)
        end
        if m.controller == "input" then
            assert(type(m.controls) == "table")
            for key, binding in pairs(m.controls) do
                assert(type(key) == "string" and #key == 1)
                assert(one_of(binding.type, { "move", "open_crafting", "open_inventory", "open_map", "shoot", "reload", "pickup", "open_inspect", "interact", "open", "smash" }))
                if binding.type == "move" then
                    assert(integer(binding.dx, -1) and binding.dx <= 1)
                    assert(integer(binding.dy, -1) and binding.dy <= 1 and binding.dz == 0)
                    assert(binding.dx ~= 0 or binding.dy ~= 0)
                end
            end
        end
    elseif config.category == "map" then
        local m = config.map
        assert(m.movement_cost >= 0.01 and m.movement_cost <= 1000)
        assert(config.common.durability == false)
        assert(#(m.spawns or {}) <= 32)
        for _, rule in ipairs(m.spawns or {}) do
            assert(type(rule.definition_id) == "string" and type(rule.chance) == "number" and rule.chance >= 0 and rule.chance <= 1)
            assert(rule.quantity == nil or integer(rule.quantity, 1))
            assert(rule.with_item == nil or type(rule.with_item) == "string")
        end
    elseif config.category == "static" then
        local s = config.static
        assert(s.placement == nil or one_of(s.placement, { "map_gen", "spawn" }))
        assert(s.placement ~= "spawn" or config.common.durability == false or type(s.destroyed_map) == "string", "spawned destructible static requires destroyed_map")
        assert(s.movement_cost == nil or (type(s.movement_cost) == "number"
            and s.movement_cost >= 0.01 and s.movement_cost <= 1000),
            "static.movement_cost must be a finite number between 0.01 and 1000")
        combat(s.combat)
        assert(one_of(s.role, { "terrain", "obstacle", "facility" }))
        strings(s.facilities, { "work_surface", "clamping", "anvil", "forge_heat" })
        amounts(s.resources)
    else
        local item = config.item
        local tier = item.tier or 1
        assert(integer(tier, 1) and tier <= 10, "invalid item tier")
        for _, name in ipairs({ "length_mm", "volume_ml", "mass_g" }) do assert(integer(item[name], 0)) end
        assert(integer(item.stack_limit, 1))
        assert(#(item.contents or {}) <= 100)
        assert((#(item.contents or {}) == 0 and not item.loot) or item.stack_limit == 1)
        for _, entry in ipairs(item.contents or {}) do
            assert(type(entry.definition_id) == "string" and integer(entry.quantity, 1) and entry.quantity <= 1000)
        end
        if item.loot then
            local loot = item.loot
            assert(#(item.contents or {}) == 0, "container must use fixed contents or loot")
            assert(integer(loot.min_rolls, 1) and integer(loot.max_rolls, loot.min_rolls)
                and loot.max_rolls <= #loot.entries and #loot.entries <= 100, "invalid loot roll range")
            assert(#loot.tier_weights >= 1 and #loot.tier_weights <= 10, "invalid loot tier weights")
            for _, weight in ipairs(loot.tier_weights) do
                assert(integer(weight, 0) and weight <= 10000, "invalid loot tier weight")
            end
            local seen = {}
            for _, entry in ipairs(loot.entries) do
                local min, max = entry.min_quantity or 1, entry.max_quantity or 1
                assert(type(entry.definition_id) == "string" and not seen[entry.definition_id]
                    and integer(min, 1) and integer(max, min) and max <= 1000, "invalid or duplicate loot entry")
                seen[entry.definition_id] = true
            end
        end
        strings(item.materials, { "wood", "metal", "cloth", "leather", "stone" })
        strings(item.tool_functions, { "cutting", "wood_cutting", "hammering", "drilling", "fastening",
            "prying", "digging", "sewing", "cooking", "butchering", "metal_cutting", "grinding", "welding" })
        for function_name, level in pairs(item.tool_levels or {}) do
            local present = false
            for _, name in ipairs(item.tool_functions) do if name == function_name then present = true end end
            assert(present and integer(level,1) and level <= 20, "invalid tool level")
        end
        amounts(item.resources)
        if item.nutrition ~= nil then validation.validate_nutrition(item.nutrition) end
        if item.weapon then
            local weapon = item.weapon
            assert(one_of(weapon.mode, { "melee", "ranged" }))
            assert(one_of(weapon.hands,{"one","two","either"}), "weapon hands required")
            assert(integer(weapon.two_hand_pv_bonus or 0,0) and (weapon.two_hand_pv_bonus or 0) <= 30)
            assert((weapon.two_hand_pv_bonus or 0) == 0 or (weapon.mode == "melee" and weapon.hands == "either"))
            assert(integer(weapon.one_hand_range_penalty or 0,0) and (weapon.one_hand_range_penalty or 0) < weapon.range)
            assert(integer(weapon.one_hand_accuracy_penalty or 0,0) and (weapon.one_hand_accuracy_penalty or 0) <= 100)
            assert(weapon.hands == "either" or ((weapon.one_hand_range_penalty or 0) == 0 and (weapon.one_hand_accuracy_penalty or 0) == 0))
            damage(weapon.damage)
            assert(integer(weapon.critical_value, 3) and weapon.critical_value <= 18, "critical_value must be 3..18")
            assert(integer(weapon.range, 1) and weapon.range <= 100)
            assert(integer(weapon.base_pv, 0) and weapon.base_pv <= 30)
            assert(integer(weapon.strength_cap, 0) and weapon.strength_cap <= 30)
            if weapon.mode == "ranged" then
                assert(type(weapon.ammo_definition_id) == "string" and integer(weapon.ammo_per_attack, 1))
                if weapon.magazine_capacity ~= nil then
                    assert(integer(weapon.magazine_capacity, weapon.ammo_per_attack) and weapon.magazine_capacity <= 1000)
                end
            else
                assert(weapon.range >= 1 and weapon.range <= 100 and weapon.ammo_definition_id == nil)
            end
        end
        if item.throwable then
            local t = item.throwable
            assert(integer(t.range, 1) and t.range <= 20 and t.radius == 2)
            assert(integer(t.power, 1) and t.power <= 100 and one_of(t.kind, {"explosive", "flash"}))
            assert(item.weapon == nil)
        end
        if item.armor then
            assert(item.armor.slot == nil or one_of(item.armor.slot,{"head","body","arms","hands","legs","feet"}))
            assert(integer(item.armor.av, 0) and item.armor.av <= 100)
            assert(integer(item.armor.dv, -100) and item.armor.dv <= 100)
        end
        if item.use then
            local use = item.use
            assert(one_of(use.kind, { "remove_status", "heal", "restore_need", "bandage", "nutrition" }))
            if use.kind == "remove_status" then
                assert(one_of(use.status_id, { "bleeding" }))
            elseif use.kind == "nutrition" then
                assert(item.nutrition ~= nil, "nutrition required")
            else
                assert(integer(use.amount, 1))
                if use.kind == "restore_need" then
                    assert(one_of(use.need, { "hydration" }))
                    assert(item.nutrition ~= nil, "food and drink must define nutrition")
                end
            end
        end
    end
    if config.interactions ~= nil then
        assert(type(config.interactions) == "table" and #config.interactions <= 32)
        local interaction_ids = {}
        for _, interaction in ipairs(config.interactions) do
            assert(type(interaction.id) == "string" and #interaction.id > 0 and not interaction_ids[interaction.id], "invalid interaction id")
            interaction_ids[interaction.id] = true
            assert(type(interaction.name) == "string" and interaction.name:match("%S") and not interaction.name:find("%c"), "invalid interaction name")
            assert(interaction.consumes_turn == nil or type(interaction.consumes_turn) == "boolean")
            if interaction.when_state then
                for key, value in pairs(interaction.when_state) do
                    assert(type(key) == "string" and #key > 0 and
                        (type(value) == "boolean" or type(value) == "string" or type(value) == "number"), "invalid interaction condition")
                end
            end
        end
        assert(#config.interactions == 0 or type(config.hooks.on_interact) == "function", "interactions require on_interact")
    end
    assert(type(config.hooks) == "table")
    for name, fn in pairs(config.hooks) do
        assert(one_of(name, { "on_spawn", "on_turn", "on_use", "on_damage", "on_destroy", "on_ability", "on_status", "on_interact" }), "unknown hook")
        assert(type(fn) == "function", "hook must be function")
    end
    print("OK " .. path)
end
local function item_reference(id)
    local definition = assert(ids[id], "missing definition: " .. id)
    assert(definition.category == "item", "reference must point to item: " .. id)
    return definition
end
for _, config in pairs(ids) do
    if config.static and config.static.destroyed_map then
        assert(ids[config.static.destroyed_map] and ids[config.static.destroyed_map].category == "map", "destroyed_map must reference map_entity")
    end
    for _, rule in ipairs(config.map and config.map.spawns or {}) do
        local target = assert(ids[rule.definition_id], "missing map spawn definition")
        if rule.with_item then item_reference(rule.with_item) end
        assert(target.category ~= "map", "map_entity cannot spawn")
        assert(not target.static or target.static.placement == "spawn", "map spawn static requires placement=spawn")
        if target.item then assert((rule.quantity or 1) <= target.item.stack_limit)
        else assert((rule.quantity or 1) == 1) end
    end
    for _, entry in ipairs(config.item and config.item.contents or {}) do
        local content = item_reference(entry.definition_id)
        assert(#(content.item.contents or {}) == 0 and not content.item.loot, "nested containers are not supported")
    end
    local loot = config.item and config.item.loot
    for _, entry in ipairs(loot and loot.entries or {}) do
        local content = item_reference(entry.definition_id)
        assert(#(content.item.contents or {}) == 0 and not content.item.loot, "nested containers are not supported")
        assert((loot.tier_weights[content.item.tier or 1] or 0) > 0, "missing positive loot tier weight")
    end
    if config.category == "movable" then
        local occupied = {}
        local armor_slots = {}
        for _, entry in ipairs(config.movable.loadout) do
            local item = item_reference(entry.definition_id)
            if item.item.armor and not item.item.weapon and not one_of("shield",item.common.tags) then
                assert(entry.grip == nil,"body armor cannot use hand grip")
                if entry.equipped then
                    local slot=item.item.armor.slot or "body"
                    assert(not armor_slots[slot],"loadout armor slot conflict");armor_slots[slot]=true
                end
            elseif entry.equipped or entry.grip then
                assert(item.item.weapon or one_of("shield",item.common.tags),"loadout item is not equippable")
                local hands = item.item.weapon and item.item.weapon.hands or "one"
                local grip = entry.grip or (hands ~= "one" and "both" or config.movable.dominant_hand or "right")
                assert(hands == "either" or (hands == "two" and grip == "both") or (hands == "one" and grip ~= "both"))
                for _, hand in ipairs({"right","left"}) do
                    if grip == hand or grip == "both" then assert(not occupied[hand],"loadout hand conflict"); occupied[hand] = true end
                end
            end
        end
    elseif config.category == "item" and config.item.weapon and config.item.weapon.mode == "ranged" then
        local ammo = item_reference(config.item.weapon.ammo_definition_id)
        assert(one_of("ammunition", ammo.common.tags), "ammo reference is not ammunition")
    end
end
print("OK: " .. #files .. " definitions and cross-references")

-- 模擬 ctx で Lua の判断と返却内容を検証。Rust のゲーム処理は実行しない。
local definitions = {}
for _, path in ipairs(assert(loadfile("data/entity_templates/manifest.lua"))()) do
    local definition = assert(loadfile(path))()
    assert(not definitions[definition.id], "duplicate definition")
    definitions[definition.id] = definition
end
local count = 0
local function check(value, message)
    assert(value, message)
    count = count + 1
end
local function instance(id)
    return { id = 100, config = assert(definitions[id]), state = {}, position = { x = 2, y = 3, z = 0 }, quantity = 1 }
end
local ctx = { target = nil, attackable = false, next_position = nil, actor = nil }
function ctx:nearest_hostile(id, range)
    check(id == 100 and range >= 1, "invalid hostile query")
    return self.target
end
function ctx:can_reload(actor) return self.reloadable == true end
function ctx:can_attack(actor, target, weapon)
    check(actor ~= nil and target ~= nil, "invalid attack query")
    return self.attackable
end
function ctx:step_towards(actor, target)
    return self.next_position
end
function ctx:entity(id)
    if self.actor and id == self.actor.id then return self.actor end
    return nil
end
for _, id in ipairs({ "monster.ghoul_worker", "monster.ghoul_warrior", "monster.ghoul_archer" }) do
    local self = instance(id)
    local turn = self.config.hooks.on_turn
    ctx.target = nil; ctx.attackable = false; ctx.next_position = nil
    check(turn(self, ctx, {turn=1}).action.type == "wait", "no enemy should wait")
    ctx.target = { id = 200 }; ctx.attackable = true
    local out = turn(self, ctx, {turn=2})
    check(out.action.type == "attack" and out.action.target == 200, "attack enemy in range")
    ctx.attackable = false; ctx.next_position = { x = 3, y = 3, z = 0 }
    out = turn(self, ctx, {turn=3})
    check(out.action.type == "move" and out.action.position == ctx.next_position, "approach enemy")
    ctx.next_position = nil
    check(turn(self, ctx, {turn=4}).action.type == "wait", "no path should wait")
end
local player = definitions["player.survivor"]
check(player.movable.controller == "input" and player.hooks.on_turn == nil, "player must use input")
for key, expected in pairs({h={-1,0},j={0,1},k={0,-1},l={1,0},y={-1,-1},u={1,-1},b={-1,1},n={1,1}}) do
    local binding = player.movable.controls[key]
    check(binding.type == "move" and binding.dx == expected[1] and binding.dy == expected[2], "wrong movement key")
end
check(player.movable.controls.p.type == "open_crafting", "wrong crafting key")
check(player.movable.controls.i.type == "open_inventory", "wrong inventory key")
check(player.movable.controls.m.type == "open_map", "wrong map key")
check(player.movable.controls.f.type == "shoot", "wrong shoot key")
check(player.movable.controls.r.type == "reload", "wrong reload key")
check(player.movable.controls.g.type == "pickup", "wrong pickup key")
check(player.movable.controls.o.type == "open", "wrong open key")
check(player.movable.controls[";"].type == "open_inspect", "wrong inspect key")
for _, definition in pairs(definitions) do
    check(type(definition.common.description) == "string" and #definition.common.description > 0, "missing description")
end
ctx.reloadable = true
ctx.target = { id = 200 }
local archer = instance("monster.ghoul_archer")
check(archer.config.hooks.on_turn(archer, ctx, {turn=5}).action.type == "reload", "archer reload")
ctx.reloadable = false
for _, id in ipairs({"terrain.field", "terrain.road"}) do
    check(not definitions[id].common.collision.movement, "ground should be walkable")
end
check(definitions["terrain.pond"].common.collision.movement, "pond must block walking for now")
local tree = instance("obstacle.tree")
local drop = tree.config.hooks.on_destroy(tree, ctx, {}).effects[1]
check(drop.type == "spawn" and definitions[drop.definition_id] and drop.quantity == 2, "tree must drop defined wood")
tree.position = nil
check(next(tree.config.hooks.on_destroy(tree, ctx, {})) == nil, "no drop without location")
for _, id in ipairs({"item.knife", "item.club", "item.glock", "item.crossbow", "item.sks"}) do
    local self = instance(id)
    ctx.attackable = false
    check(next(self.config.hooks.on_use(self, ctx, {user=1,target=2})) == nil, "reject unusable attack")
    ctx.attackable = true
    check(next(self.config.hooks.on_use(self, ctx, {user=1})) == nil, "reject missing attack target")
    local out = self.config.hooks.on_use(self, ctx, {user=1,target=2})
    check(#out.effects == 1 and out.effects[1].type == "weapon_attack" and out.effects[1].weapon == 100, "weapon request")
end
local function actor()
    return { id=1, category="movable", statuses={}, hp={current=20,max=60}, body={body="healthy"},
        nutrition={protein=30,carbohydrate=30,vitamin=30}, needs={hydration={current=30,max=100}} }
end
local function use(id, event)
    local self = instance(id)
    return self.config.hooks.on_use(self, ctx, event or {user=1})
end
ctx.actor = actor()
ctx.actor.bleeding = {left_arm=1,body=0}
ctx.actor.bandages = {}
check(next(use("item.bandage")) == nil,"bandage needs body part selection")
check(next(use("item.bandage",{user=1,body_part="body"})) == nil,"healthy part cannot be bandaged")
local dressed=use("item.bandage",{user=1,body_part="left_arm"}).effects
check(dressed[1].type == "apply_bandage" and dressed[1].body_part == "left_arm","bandage selected bleeding part")
check(dressed[2].type == "consume_item" and dressed[2].quantity == 1,"bandage consumes one")
ctx.actor.bandages.left_arm = 100
check(next(use("item.bandage",{user=1,body_part="left_arm"})) == nil,"already bandaged part is excluded")
ctx.actor = actor()
check(use("item.syringe").effects[1].type == "heal", "injection heal request")
ctx.actor.hp.current = 60
check(next(use("item.syringe")) == nil, "full health should not consume")
ctx.actor.hp.current = 0
check(next(use("item.syringe")) == nil, "dead actor should not heal")
for _, id in ipairs({"item.bread","item.mineral_water"}) do
    ctx.actor = actor()
    local effects = use(id).effects
    local index = id == "item.bread" and 1 or 2
    check(effects[index].type == "gain_nutrition" and effects[index].nutrition == definitions[id].item.nutrition, "configured nutrition")
    check(effects[#effects].type == "consume_item" and effects[#effects].quantity == 1, "consume one")
    ctx.actor.nutrition={protein=100,carbohydrate=100,vitamin=100}
    ctx.actor.needs.hydration.current=100
    check(next(use(id)) == nil, "no benefit should not consume")
    check(next(use(id,{user=2,target=1})) == nil, "affect user only")
end
for _, id in ipairs({"item.bandage","item.syringe","item.bread","item.mineral_water"}) do
    ctx.actor = nil
    check(next(use(id)) == nil, "missing target should not consume")
    ctx.actor = {id=1,category="static"}
    check(next(use(id)) == nil, "fixed object should not consume medicine/food")
end
print("OK: " .. count .. " Lua behavior checks (mock ctx)")

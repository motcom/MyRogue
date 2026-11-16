-- 栄養の入力範囲と、食品 Lua から返される摂取要求を検証する。
local validation = require("tools.entity_validation")
local count = 0
local function check(value, message)
    assert(value, message)
    count = count + 1
end
for _, key in ipairs({"protein", "carbohydrate", "vitamin"}) do
    for _, value in ipairs({0, 1, 4294967295}) do
        local nutrition = {protein=0, carbohydrate=0, vitamin=0}
        nutrition[key] = value
        check(pcall(validation.validate_nutrition, nutrition), "reject valid u32: " .. key)
    end
    for _, value in ipairs({-1, 0.5, 4294967296, math.huge, -math.huge, 0/0, "8", true}) do
        local nutrition = {protein=0, carbohydrate=0, vitamin=0}
        nutrition[key] = value
        check(not pcall(validation.validate_nutrition, nutrition), "accept invalid u32: " .. key)
    end
    local nutrition = {protein=0, carbohydrate=0, vitamin=0}
    nutrition[key] = nil
    check(not pcall(validation.validate_nutrition, nutrition), "accept missing property")
end
check(not pcall(validation.validate_nutrition, {protein=0, carbohydrate=0, vitamin=0, vitamins=2}), "unknown key")
check(not pcall(validation.validate_nutrition, false), "non-table")
for _, name in ipairs({"bread", "mineral_water"}) do
    local config = assert(loadfile("data/entity_templates/item/" .. name .. ".lua"))()
    validation.validate_nutrition(config.item.nutrition)
    local actor = {id=1,category="movable",nutrition={protein=50,carbohydrate=50,vitamin=50},needs={hydration={current=50,max=100}}}
    local ctx = {}
    function ctx:entity(id) if id == 1 then return actor end end
    local self = {id=2,quantity=10,config=config,state={}}
    local result = config.hooks.on_use(self, ctx, {user=1})
    local index = name == "bread" and 1 or 2
    check(#result.effects == index+1, "coordinated effects")
    if name == "mineral_water" then check(result.effects[1].type == "restore_need", "water recovery") end
    check(result.effects[index].type == "gain_nutrition" and result.effects[index].target == 1, "nutrition target")
    for _, key in ipairs({"protein", "carbohydrate", "vitamin"}) do
        check(result.effects[index].nutrition[key] == config.item.nutrition[key], "one unit, not whole stack")
    end
    check(result.effects[index+1].type == "consume_item" and result.effects[index+1].quantity == 1, "consume one unit")
    actor.nutrition={protein=100,carbohydrate=100,vitamin=100}
    actor.needs.hydration.current=100
    check(next(config.hooks.on_use(self, ctx, {user=1})) == nil, "full: no effect")

end
print("OK: " .. count .. " food/nutrition checks")

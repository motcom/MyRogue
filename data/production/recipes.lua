-- 制作の調整用データ。再起動時に読み込む。値の範囲・参照・消費はRustで検査する。
local function recipe(id, name, ingredients, tools, skill, recommended, turns, output, count, quality, workbench)
    local materials, functions = {}, {}
    for _, material in ipairs(ingredients) do
        table.insert(materials, {kind="item."..material[1], quantity=material[2]})
    end
    for _, tool in ipairs(tools) do
        table.insert(functions, {function_name=tool[1], level=tool[2]})
    end
    return {
        id="craft."..id, name=name, ingredients=materials, required_tools=functions,
        required_facilities=workbench and {"work_surface"} or {},
        required_skills={{skill=skill, recommended_level=recommended}}, resource_costs={},
        outputs={{kind=output, quantity=count}}, work_turns=turns, quality_enabled=quality,
    }
end
return {
    rules = {
        success_base=60, skill_step=10, dexterity_step=5, success_min=5, success_max=100,
        tool_step=5, tool_cap=10, material_step=5, material_cap=10,
        quality_thresholds={10,30,75,90,100}, failure_material_percent=50,
    },
    recipes = {
        recipe("club", "棍棒を削る", {{"wood",1}}, {{"cutting",1}}, "wood_craft",0,3,"item.club",1,true,true),
        recipe("wood_handle", "木柄を削る", {{"wood",1}}, {{"cutting",1}}, "wood_craft",1,3,"item.wood_handle",1,true,true),
        recipe("bandage", "布を切って包帯を作る", {{"cloth",1}}, {{"cutting",1}}, "saihou",0,2,"item.bandage",2,false,false),
        recipe("spear", "穂先付きの槍を組み立てる", {{"wood",2},{"metal_stock",1},{"thread",1}}, {{"cutting",1},{"drilling",2},{"grinding",2}}, "wood_craft",2,6,"item.spear",1,true,true),
        recipe("mace", "鉄材からメイスを組み立てる", {{"wood_handle",1},{"metal_stock",3}}, {{"hammering",2},{"drilling",2},{"metal_cutting",2}}, "metal_craft",3,8,"item.mace",1,true,true),
        recipe("knife", "鉄材を削りナイフを作る", {{"metal_stock",2},{"wood_handle",1}}, {{"metal_cutting",2},{"grinding",2},{"drilling",2}}, "metal_craft",2,7,"item.knife",1,true,true),
        recipe("shield", "木と革で盾を作る", {{"wood",2},{"leather_piece",2},{"thread",2}}, {{"cutting",1},{"drilling",2},{"sewing",1}}, "wood_craft",2,6,"item.shield",1,true,true),
        recipe("leather_armor", "革の鎧を縫う", {{"leather_piece",4},{"thread",3}}, {{"cutting",1},{"sewing",1}}, "saihou",3,8,"item.leather_armor",1,true,true),
        recipe("bolts", "木材からボルトを作る", {{"wood",1},{"metal_stock",1}}, {{"cutting",1},{"grinding",2}}, "wood_craft",1,4,"ammo.bolt",5,false,true),
        recipe("cooked_meat", "肉を焼く", {{"raw_meat",1},{"charcoal",1}}, {{"cooking",2}}, "cooking",1,3,"item.cooked_meat",1,false,false),
        recipe("clean_water", "汚染水を煮沸する", {{"dirty_water",1},{"charcoal",1}}, {{"cooking",2}}, "cooking",1,3,"item.clean_water",1,false,false),
        recipe("treasure_chest", "木の宝箱を組み立てる", {{"wood",4},{"metal_stock",2}}, {{"wood_cutting",2},{"hammering",2},{"drilling",2}}, "wood_craft",3,10,"item.treasure_chest",1,true,true),
    },
}

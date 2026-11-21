-- 制作試作の道具・材料。機能レベルと寸法はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.metal_saw", category = "item",
    common = {
        name = "金属のこぎり", description = "鉄材の切り分けに使う。金属切断Lv2。",
        appearance = { glyph = "(", fg = {180,180,150}, bg = {0,0,0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 }, tags = { "tool" },
    },
    initial_state = {},
    item = {
        tier = 2,
        length_mm = 300, volume_ml = 300, mass_g = 500, stack_limit = 1,
        materials = { "metal" }, tool_functions = { "metal_cutting" },
        tool_levels = { metal_cutting = 2 }, resources = {},
    }, hooks = {},
}

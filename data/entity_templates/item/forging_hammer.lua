-- 制作試作の道具・材料。機能レベルと寸法はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.forging_hammer", category = "item",
    common = {
        name = "金工ハンマー", description = "鉄材の打ち込みと組み立てに使う。打撃Lv2。",
        appearance = { glyph = "(", fg = {180,180,150}, bg = {0,0,0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 }, tags = { "tool" },
    },
    initial_state = {},
    item = {
        tier = 3,
        length_mm = 300, volume_ml = 300, mass_g = 500, stack_limit = 1,
        materials = { "metal", "wood" }, tool_functions = { "hammering" },
        tool_levels = { hammering = 2 }, resources = {},
    }, hooks = {},
}

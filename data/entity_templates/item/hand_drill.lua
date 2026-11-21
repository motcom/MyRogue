-- 制作試作の道具・材料。機能レベルと寸法はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.hand_drill", category = "item",
    common = {
        name = "手回しドリル", description = "木や薄い鉄材の穴あけに使う。穴あけLv2。",
        appearance = { glyph = "(", fg = {180,180,150}, bg = {0,0,0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 }, tags = { "tool" },
    },
    initial_state = {},
    item = {
        tier = 2,
        length_mm = 300, volume_ml = 300, mass_g = 500, stack_limit = 1,
        materials = { "metal", "wood" }, tool_functions = { "drilling" },
        tool_levels = { drilling = 2 }, resources = {},
    }, hooks = {},
}

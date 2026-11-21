-- 制作試作の道具・材料。機能レベルと寸法はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.cooking_kit", category = "item",
    common = {
        name = "携帯調理セット", description = "小型こんろと鍋の組。燃料の木炭を別に使う。調理Lv2。",
        appearance = { glyph = "(", fg = {180,180,150}, bg = {0,0,0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = { max = 100 }, tags = { "tool" },
    },
    initial_state = {},
    item = {
        tier = 2,
        length_mm = 300, volume_ml = 300, mass_g = 500, stack_limit = 1,
        materials = { "metal" }, tool_functions = { "cooking" },
        tool_levels = { cooking = 2 }, resources = {},
    }, hooks = {},
}

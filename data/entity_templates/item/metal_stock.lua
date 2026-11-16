-- 制作試作の道具・材料。機能レベルと寸法はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.metal_stock", category = "item",
    common = {
        name = "加工用鉄材", description = "切断・研削・穴あけで加工できる鉄材。武器や箱の材料。",
        appearance = { glyph = "%", fg = {180,180,150}, bg = {0,0,0}, layer = 20 },
        collision = { movement = false, sight = false },
        durability = false, tags = { "material" },
    },
    initial_state = {},
    item = {
        length_mm = 150, volume_ml = 100, mass_g = 100, stack_limit = 100,
        materials = { "metal" }, tool_functions = {  },
        tool_levels = {  }, resources = {},
    }, hooks = {},
}

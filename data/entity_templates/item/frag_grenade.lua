-- 注射器は回復薬入りの架空のゲームアイテムとして仮定。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.frag_grenade",
    category = "item",
    common = {
        name = "爆発手榴弾",
        description = "所持品でEnterを押して投擲先を選ぶ。5×5マスに中心から減衰する被害。自分や仲間も巻き込む。",
        appearance = {
            glyph = "!",
            fg = { 220, 200, 140 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = false,
        tags = { "consumable" },
    },
    initial_state = {},
    item = {
        length_mm = 150,
        volume_ml = 30,
        mass_g = 30,
        stack_limit = 10,
        materials = {  },
        tool_functions = {  },
        resources = {},
        throwable = { range = 6, radius = 2, power = 24, kind = "explosive" },
    },
    hooks = {},
}

-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "obstacle.stone_wall",
    category = "static",
    common = {
        name = "石壁",
        description = "石でできた壁。移動と視線を遮る。",
        appearance = {
            glyph = "#",
            fg = { 150, 150, 150 },
            bg = { 0, 0, 0 },
            layer = 10,
        },
        collision = {
            movement = true,
            sight = true,
        },
        durability = {
            max = 150,
        },
        tags = { "obstacle" },
    },
    initial_state = {},
    static = {
        movement_cost = 1.0,
        combat = { base_av = 5 },
        role = "obstacle",
        facilities = {  },
        resources = {},
    },
    hooks = {
    },
}

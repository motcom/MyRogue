-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "obstacle.tree",
    category = "static",
    common = {
        name = "木",
        description = "枝葉を広げる木。破壊すると木材を得られる。",
        appearance = {
            glyph = "T",
            fg = { 100, 112, 75 },
            bg = { 0, 0, 0 },
            layer = 10,
        },
        collision = {
            movement = true,
            sight = true,
        },
        durability = {
            max = 60,
        },
        tags = { "obstacle" },
    },
    initial_state = {},
    static = {
        placement = "spawn",
        destroyed_map = "terrain.field",
        movement_cost = 1.0,
        combat = { base_av = 2 },
        role = "obstacle",
        facilities = {  },
        resources = {},
    },
    hooks = {
        on_destroy = function(self, ctx, event)
            if not self.position then return {} end
            return { effects = {
                { type = "spawn", definition_id = "item.wood", position = self.position, quantity = 2 },
            } }
        end,
    },
}

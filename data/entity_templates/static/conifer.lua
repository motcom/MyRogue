-- 地面・固定物も全て Entity として配置する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "obstacle.conifer",
    category = "static",
    common = {
        name = "針葉樹",
        description = "固定マップ要素。通行と視線は個別に設定する。",
        appearance = {
            glyph = "Y",
            fg = { 40, 130, 40 },
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

-- 閉じている扉は移動・視線を遮る。操作メニューと開閉処理をLuaに登録する。
return {
    schema_version = 2,
    id = "obstacle.wooden_door",
    category = "static",
    common = {
        name = "木の扉",
        description = "木でできた扉。eで方向を選び、行動メニューから開閉できる。開くと通行と視線が通る。",
        appearance = { glyph = "+", fg = {180, 130, 70}, bg = {0, 0, 0}, layer = 12 },
        collision = { movement = true, sight = true },
        durability = { max = 60 },
        tags = { "door" },
    },
    initial_state = { opened = false },
    static = {
        movement_cost = 1.0,
        role = "obstacle", combat = { base_av = 1 }, facilities = {}, resources = {},
    },
    interactions = {
        { id = "open", name = "開く", when_state = { opened = false } },
        { id = "close", name = "閉じる", when_state = { opened = true } },
    },
    hooks = {
        on_interact = function(self, ctx, event)
            local opened = event.interaction == "open"
            if event.interaction ~= "open" and event.interaction ~= "close" then return {} end
            return {
                state = { opened = opened },
                effects = {
                    { type = "set_collision", target = self.id, movement = not opened, sight = not opened },
                    { type = "set_glyph", target = self.id, glyph = opened and "/" or "+" },
                    { type = "message", message = opened and "扉を開けました" or "扉を閉じました" },
                },
            }
        end,
    },
}

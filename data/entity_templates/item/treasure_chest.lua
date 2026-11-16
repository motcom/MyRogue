-- 宝箱自体も拾えるItem。中身は個体ごとに保持し、選んだ品だけ取り出す。
return {
    schema_version = 2,
    id = "item.treasure_chest",
    category = "item",
    common = {
        name = "宝箱",
        description = "持ち運べる木の宝箱。開くと中身を選んで取得できる。Spaceで複数選択、Enterで取得。残りは箱に残る。",
        appearance = { glyph = "C", fg = {210, 160, 60}, bg = {0, 0, 0}, layer = 21 },
        collision = { movement = false, sight = false },
        durability = { max = 100 },
        tags = { "container" },
    },
    initial_state = { opened = false },
    item = {
        length_mm = 600, volume_ml = 5000, mass_g = 3000, stack_limit = 1,
        contents = {
            { definition_id = "item.bread", quantity = 2 },
            { definition_id = "item.bandage", quantity = 2 },
            { definition_id = "ammo.pistol", quantity = 10 },
        },
        materials = { "wood", "metal" }, tool_functions = {}, resources = {},
    },
    interactions = {
        { id = "open", name = "開く", when_state = { opened = false } },
        { id = "contents", name = "中身を見る", when_state = { opened = true }, consumes_turn = false },
        { id = "close", name = "閉じる", when_state = { opened = true } },
    },
    hooks = {
        on_interact = function(self, ctx, event)
            if event.interaction == "open" then
                local effects = {
                    { type = "set_glyph", target = self.id, glyph = "c" },
                    { type = "message", message = "宝箱を開けました" },
                }
                table.insert(effects, { type = "open_container", target = self.id })
                return { state = { opened = true }, effects = effects }
            elseif event.interaction == "contents" then
                return { effects = { { type = "open_container", target = self.id } } }
            elseif event.interaction == "close" then
                return { state = { opened = false }, effects = {
                    { type = "set_glyph", target = self.id, glyph = "C" },
                    { type = "message", message = "宝箱を閉じました" },
                } }
            end
            return {}
        end,
    },
}

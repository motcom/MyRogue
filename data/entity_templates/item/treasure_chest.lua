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
        tier = 2,
        length_mm = 600, volume_ml = 5000, mass_g = 3000, stack_limit = 1,
        loot = {
            min_rolls = 2, max_rolls = 4,
            -- Tier 1〜4の抽選重み。品目のティアは各アイテム定義から読む。
            tier_weights = { 65, 26, 8, 1 },
            entries = {
                { definition_id = "item.knife" },
                { definition_id = "item.club" },
                { definition_id = "item.spear" },
                { definition_id = "ammo.bolt", min_quantity = 4, max_quantity = 8 },
                { definition_id = "item.bandage", min_quantity = 1, max_quantity = 3 },
                { definition_id = "item.bread", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.apple", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.cooked_meat", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.carrot", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.cabbage" },
                { definition_id = "item.mineral_water", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.clean_water", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.hammer" },
                { definition_id = "item.wood", min_quantity = 2, max_quantity = 4 },
                { definition_id = "item.metal_stock", min_quantity = 2, max_quantity = 4 },
                { definition_id = "item.cloth", min_quantity = 2, max_quantity = 4 },
                { definition_id = "item.leather_piece", min_quantity = 1, max_quantity = 3 },
                { definition_id = "item.thread", min_quantity = 2, max_quantity = 5 },
                { definition_id = "item.charcoal", min_quantity = 2, max_quantity = 5 },
                { definition_id = "item.wood_handle", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.short_sword" },
                { definition_id = "item.mace" },
                { definition_id = "item.crossbow" },
                { definition_id = "item.leather_armor" },
                { definition_id = "item.shield" },
                { definition_id = "ammo.pistol", min_quantity = 6, max_quantity = 12 },
                { definition_id = "item.vitamins", min_quantity = 1, max_quantity = 2 },
                { definition_id = "item.hand_drill" },
                { definition_id = "item.metal_saw" },
                { definition_id = "item.metal_file" },
                { definition_id = "item.sewing_kit" },
                { definition_id = "item.cooking_kit" },
                { definition_id = "item.wood_saw" },
                { definition_id = "item.long_sword" },
                { definition_id = "item.bastard_sword" },
                { definition_id = "item.cleaver" },
                { definition_id = "item.glock" },
                { definition_id = "item.sks" },
                { definition_id = "item.flash_grenade" },
                { definition_id = "ammo.rifle", min_quantity = 5, max_quantity = 10 },
                { definition_id = "item.syringe" },
                { definition_id = "item.forging_hammer" },
                { definition_id = "item.ak" },
                { definition_id = "item.m16" },
                { definition_id = "item.frag_grenade" },
            },
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

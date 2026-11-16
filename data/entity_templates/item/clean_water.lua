-- 効果がある場合だけ一個を消費する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.clean_water",
    category = "item",
    common = {
        name = "水（清潔）",
        description = "摂取で水分と栄養を回復する。数値はゲーム用の仮設定。",
        appearance = {
            glyph = "%",
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
        volume_ml = 400,
        mass_g = 500,
        stack_limit = 10,
        materials = {  },
        tool_functions = {  },
        resources = {},
        -- 一個あたりの栄養ポイント。各値は Rust の u32 範囲。
        nutrition = {
            protein = 0,
            carbohydrate = 0,
            vitamin = 0,
        },
        use = {
            kind = "restore_need",
            need = "hydration",
            amount = 40,
        },
    },
    hooks = {
        on_use = function(self, ctx, event)
            local target_id = event.target or event.user
            local target = ctx:entity(target_id)
            if not target or target.category ~= "movable" then return {} end
            -- 飲食は使用者自身に限定。水分は栄養量と別に扱う。
            if target_id ~= event.user then return {} end
            local use = self.config.item.use
            local need = target.needs and target.needs[use.need]
            if not need or need.current >= need.max then return {} end
            return { effects = {
                { type = "restore_need", target = target_id, need = use.need, amount = use.amount },
                { type = "gain_nutrition", target = target_id, nutrition = self.config.item.nutrition },
                { type = "consume_item", target = self.id, quantity = 1 },
            } }
        end,
    },
}

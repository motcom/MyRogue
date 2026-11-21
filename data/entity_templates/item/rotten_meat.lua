-- 効果がある場合だけ一個を消費する。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "item.rotten_meat",
    category = "item",
    common = {
        name = "肉（腐り）",
        description = "摂取で炭水化物などの栄養を回復する。数値はゲーム用の仮設定。摂取するとHPに3の被害を受ける。",
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
        tier = 1,
        length_mm = 150,
        volume_ml = 400,
        mass_g = 250,
        stack_limit = 10,
        materials = {  },
        tool_functions = {  },
        resources = {},
        -- 一個あたりの栄養ポイント。各値は Rust の u32 範囲。
        nutrition = {
            protein = 5,
            carbohydrate = 0,
            vitamin = 0,
        },
        use = {kind="nutrition"},
    },
    hooks = {
        on_use = function(self, ctx, event)
            local target_id = event.target or event.user
            local target = ctx:entity(target_id)
            if not target or target.category ~= "movable" then return {} end
            -- 飲食は使用者自身に限定。水分とは別に三種類の栄養を補う。
            if target_id ~= event.user then return {} end
            local n = self.config.item.nutrition
            local current = target.nutrition
            if not current then return {} end
            if not ((n.protein > 0 and current.protein < 100)
                or (n.carbohydrate > 0 and current.carbohydrate < 100)
                or (n.vitamin > 0 and current.vitamin < 100)) then return {} end
            return { effects = {
                { type = "gain_nutrition", target = target_id, nutrition = self.config.item.nutrition },
                { type = "damage", target = target_id, amount = 3 },
                { type = "consume_item", target = self.id, quantity = 1 },
            } }
        end,
    },
}

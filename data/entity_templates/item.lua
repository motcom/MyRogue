-- 射程・威力・寸法は現実の製品仕様ではなくゲーム用の仮設定。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "item.knife",

    category = "item",
    common = {
        name = "ナイフ",
        description = "片手で扱う近接武器。隣接した相手を攻撃できる。",
        appearance = {
            glyph = "/",
            fg = { 200, 200, 200 },
            bg = { 0, 0, 0 },
            layer = 20,
        },
        collision = {
            movement = false,
            sight = false,
        },
        durability = {
            max = 100,
        },
        tags = { "weapon" },
    },
    initial_state = {},
    item = {
        length_mm = 250,
        volume_ml = 150,
        mass_g = 200,
        stack_limit = 1,
        materials = { "metal" },
        tool_functions = { "cutting", "butchering" },
        resources = {},
        weapon = {
            skill = "melee_weapon",
            hands = "one",
            critical_value = 10,
            base_pv = 4, strength_cap = 2, accuracy_bonus = 0,
            mode = "melee",
            damage = { dice = 1, sides = 4, bonus = 0 },
            range = 1,
        },
    },
    hooks = {
        on_use = function(self, ctx, event)
            if not event.target or not ctx:can_attack(event.user, event.target, self.id) then return {} end
            return { effects = {
                { type = "weapon_attack", attacker = event.user, target = event.target, weapon = self.id },
            } }
        end,
    },
}

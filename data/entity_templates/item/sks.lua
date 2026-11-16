-- 射程・威力・寸法は現実の製品仕様ではなくゲーム用の仮設定。
-- 数値はゲーム用の仮設定。Rust 側のホスト API は未接続。
return {
    schema_version = 2,
    id = "item.sks",
    category = "item",
    common = {
        name = "SKS",
        description = "ライフル弾を使う遠距離武器。装備し、rで装填してからfで射撃する。",
        appearance = {
            glyph = ")",
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
        length_mm = 1000,
        volume_ml = 4000,
        mass_g = 3800,
        stack_limit = 1,
        materials = { "wood", "metal" },
        tool_functions = {  },
        resources = {},
        weapon = {
            skill = "rifle",
            hands = "two",
            critical_value = 11,
            base_pv = 7, strength_cap = 0, accuracy_bonus = 0,
            mode = "ranged",
            damage = { dice = 1, sides = 8, bonus = 2 },
            range = 10,
            ammo_definition_id = "ammo.rifle",
            ammo_per_attack = 1,
            magazine_capacity = 10, -- ゲーム用の仮設定
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

-- 武器と射程に応じて攻撃・追跡・待機するグール。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "monster.ghoul_guard",
    category = "movable",
    common = {
        name = "グール・ガード",
        description = "設計に対応したグール・ガード。行動・能力値はゲーム用の仮設定。",
        appearance = {
            glyph = "G",
            fg = { 180, 90, 90 },
            bg = { 0, 0, 0 },
            layer = 30,
        },
        collision = {
            movement = true,
            sight = false,
        },
        durability = false,
        tags = { "ghoul" },
    },
    initial_state = {},
    movable = {
        tier = 2,
        experience_reward = 70,
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 4, dv_bonus = 0, accuracy_bonus = 0 },
        role = "monster",
        controller = "lua",
        faction = "ghoul",
        hp = 60,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 10,
            subayasa = 6,
            kiyousa = 8,
            seishinryoku = 4,
        },
        skills = {
            melee_weapon = 2,
            bow = 0,
        },
        inventory = {
            max_length_mm = 1500,
            max_volume_ml = 20000,
            max_mass_g = 30000,
        },
        senses = {
            sight_range = 10,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = {{definition_id="item.club",quantity=1,equipped=true,grip="right"},{definition_id="item.shield",quantity=1,equipped=true,grip="left"}},
        needs = {
            hydration = {
                max = 100,
                initial = 80,
            },
        },
    },
    hooks = {
        on_turn = function(self, ctx, event)
            return { action = { type = "ghoul_turn", behavior = "guard" } }
        end,
    },
}

-- 武器と射程に応じて攻撃・追跡・待機するグール。
-- 数値はゲーム用の仮設定。
return {
    schema_version = 2,
    id = "monster.ghoul_rifleman",
    category = "movable",
    common = {
        name = "グール・ライフルマン",
        description = "設計に対応したグール・ライフルマン。行動・能力値はゲーム用の仮設定。",
        appearance = {
            glyph = "a",
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
        experience_reward = 120,
        dominant_hand = "right",
        sanity = 100,
        psykick = {},
        status = {},
        combat = { base_av = 0, dv_bonus = 0, accuracy_bonus = 0 },
        role = "monster",
        controller = "lua",
        faction = "ghoul",
        hp = 32,
        body = { "head", "body", "left_arm", "right_arm", "left_leg", "right_leg" },
        attributes = {
            kinryoku = 6,
            tairyoku = 7,
            subayasa = 6,
            kiyousa = 8,
            tiryoku = 4,
            seishinryoku = 4,
        },
        skills = {
            melee_weapon = 0,
            bow = 2,
        },
        inventory = {
            max_length_mm = 1500,
            max_volume_ml = 20000,
            max_mass_g = 30000,
        },
        senses = {
            sight_range = 12,
        },
        attack = {
            damage = { dice = 1, sides = 3, bonus = 0 },
            base_pv = 3, strength_cap = 2,
            range = 1,
        },
        loadout = {{definition_id="item.sks",quantity=1,equipped=true},{definition_id="ammo.rifle",quantity=20}},
        needs = {
            hydration = {
                max = 100,
                initial = 80,
            },
        },
    },
    hooks = {
        on_turn = function(self, ctx, event)
            return { action = { type = "ghoul_turn", behavior = "rifleman" } }
        end,
    },
}
